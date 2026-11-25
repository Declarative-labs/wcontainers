//! The host code of every route (docs/native.md sections 3.2 to 3.4 and 4): the withheld registers and their
//! stand-ins, [`access`] as the one place a guest access is emitted, and each route's sequence with its
//! position map.

use std::mem::offset_of;
use std::ops::Range;

use wcontainers_abi::PAGE_SIZE;
use wcontainers_guest_cpu::{
    AddressPlace, BranchTable, Fixup, Guest, Held, Holds, HostCode, InsideCall, Native,
    NativeReason, Registers, Resume, Route, Span, effects,
};

use crate::encode::{self, Register, ZERO, field, form};
use crate::instruction::{
    Address, BranchRegister, Instruction as I, MemoryAccess, Operand, Reg, Shift, SystemRegister,
    Width,
};
use crate::meaning::Host;
use crate::mirror::{
    self, LAST_TARGET_BRANCH, LAST_TARGET_CALL, LAST_TARGET_MISSED, LAST_TARGET_TEST, MIRROR,
    REACH_BITS, SLOT_MOVES, SLOT_SHIFT,
};
pub use crate::thread_state::{
    BORROWED, COUNT, PAGE, RegisterPage, SCRATCH, STACK_ALIGNMENT, TAG_LOW, WITHHELD,
};
use crate::{Arm64, FPCR_BITS, decode};
use crate::{forms, habits};

/// The bytes of a 64-bit, a 32-bit and a 128-bit register.
const X_BYTES: u8 = size_of::<u64>() as u8;
const W_BYTES: u8 = size_of::<u32>() as u8;
use crate::simd::Q_BYTES;
/// The bytes of a half-word access.
const H_BYTES: u8 = size_of::<u16>() as u8;

/// The bytes of one instruction.
const INSTRUCTION: u64 = Arm64::MAX_LENGTH;

/// A call into the inside: `brk` with the call's code.
pub fn call_word(call: InsideCall) -> u32 {
    encode::exception(form::BRK, call.code())
}
/// The low bits of a branch target that an aligned one has clear.
const ALIGNMENT_BITS: u32 = INSTRUCTION.trailing_zeros();
/// `adrp`'s pages, which are 4 KiB whatever the translation granule (Arm ARM, ADRP).
const ADRP_PAGE: u64 = 1 << 12;
/// What the hardware computes for the float meanings: fused multiply-adds and Arm's not-a-numbers.
const HARDWARE: Host = Host {
    fused_multiply_add: true,
    arm_nans: true,
};

/// The byte offset of a register page word.
fn page_word(offset: usize, index: usize) -> u64 {
    (offset + index * size_of::<u64>()) as u64
}

fn withheld_slot(register: Register) -> u64 {
    let index = WITHHELD.iter().position(|&w| w == register);
    page_word(
        offset_of!(RegisterPage, withheld),
        index.expect("a withheld register"),
    )
}

fn saved_slot(index: usize) -> u64 {
    page_word(offset_of!(RegisterPage, saved), index)
}

fn held_page(offset: u64) -> Held {
    Held::Page(u16::try_from(offset).expect("the register page is small"))
}

fn load_page(rt: Register, offset: u64) -> u32 {
    encode::load_store(form::LDR, rt, PAGE, offset, size_of::<u64>() as u64)
        .expect("register page words are in reach")
}

fn store_page(rt: Register, offset: u64) -> u32 {
    encode::load_store(form::STR, rt, PAGE, offset, size_of::<u64>() as u64)
        .expect("register page words are in reach")
}

fn number(r: Reg) -> Register {
    match r {
        Reg::X(n) => n,
        Reg::Zero | Reg::Sp => ZERO,
    }
}

fn bit(n: Register) -> u32 {
    Arm64::reg_bit(Reg::X(n)).expect("a general register has a bit")
}

fn only(n: Register) -> Registers {
    Registers::of_bits(&[bit(n)])
}

/// What an instruction's meaning reads and writes.
fn touched(i: &I) -> (Registers, Registers) {
    let e = effects::<Arm64>(i, HARDWARE, ());
    (e.uses, e.defs)
}

/// A guest register the instruction does not touch and that is not withheld.
fn free(avoid: Registers) -> Register {
    (0..ZERO)
        .find(|&n| !WITHHELD.contains(&n) && !avoid.has_bit(bit(n)))
        .expect("an instruction touches few registers")
}

/// An even guest register that, with the next one, the instruction does not touch.
fn free_pair(avoid: Registers) -> Register {
    (0..ZERO)
        .step_by(2)
        .find(|&n| {
            [n, n + 1]
                .iter()
                .all(|&r| !WITHHELD.contains(&r) && !avoid.has_bit(bit(r)))
        })
        .expect("an instruction touches few registers")
}

/// Host code written for one guest instruction.
struct Out<'a> {
    code: &'a mut HostCode,
    address: u64,
    /// The bases a test before the instruction's loop found aligned.
    aligned: &'a AlignedBases,
}

/// Base registers of a loop's accesses, each with the size it was found aligned to by one test before the loop;
/// no instruction of the loop writes them, so the test holds for every turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AlignedBases(Vec<(Register, u32)>);

impl AlignedBases {
    fn holds(&self, base: Reg, bytes: u32) -> bool {
        matches!(base, Reg::X(n) if self.0.iter().any(|&(r, b)| r == n && b >= bytes))
    }
}

/// The bases whose page test an access in `part` makes as a test of the base's alignment alone (an access of one
/// element at its base's alignment), that no instruction of `part` writes, with the largest size tested.
pub(crate) fn aligned_bases(part: &[(u64, I)]) -> AlignedBases {
    let written = part
        .iter()
        .fold(Registers::NONE, |w, (_, i)| w.union(touched(i).1));
    let mut bases: Vec<(Register, u32)> = Vec::new();
    for (_, i) in part {
        if !matches!(Arm64::route(i), Route::Copy | Route::Split) || !one_element(i) {
            continue;
        }
        let Some(m) = i.memory_access() else {
            continue;
        };
        let Some(Reg::X(base)) = base_of(&m.address) else {
            continue;
        };
        if m.bytes > 1
            && may_cross_page(&m)
            && aligned_with_base(&m.address, m.bytes)
            && !WITHHELD.contains(&base)
            && !written.has_bit(bit(base))
        {
            match bases.iter_mut().find(|(r, _)| *r == base) {
                Some((_, bytes)) => *bytes = (*bytes).max(m.bytes),
                None => bases.push((base, m.bytes)),
            }
        }
    }
    AlignedBases(bases)
}

/// The test of `aligned`'s bases in t, ending in a branch taken when every one is aligned (pointed later), whose
/// offset it gives; when one is not, the code goes on after it.
pub(crate) fn test_aligned_bases(code: &mut HostCode, aligned: &AlignedBases) -> Option<usize> {
    let (&(last, last_bytes), rest) = aligned.0.split_last()?;
    let test = |base: Register, bytes: u32| {
        encode::and_immediate(SCRATCH, base, u64::from(bytes) - 1)
            .expect("a power of two less one is a logical immediate")
    };
    // Each earlier base's branch skips to the word after the last branch.
    let words_left = |k: usize| ((rest.len() - k) * 2 + 1) as i64;
    for (k, &(base, bytes)) in rest.iter().enumerate() {
        code.push(&test(base, bytes).to_le_bytes());
        let skip = encode::compare_branch(true, SCRATCH, words_left(k) * INSTRUCTION as i64)
            .expect("fits");
        code.push(&skip.to_le_bytes());
    }
    code.push(&test(last, last_bytes).to_le_bytes());
    let at = code.offset();
    code.push(
        &encode::compare_branch(false, SCRATCH, 0)
            .expect("fits")
            .to_le_bytes(),
    );
    Some(at)
}

/// Code out of line: a test's other way, entered from the branch at `from` in the state it had there.
struct Tail {
    state: (Span, bool),
    from: usize,
    words: Vec<u32>,
    /// What a stop in it means, when it differs from the branch's state.
    resume: Option<Resume>,
    fixups: Vec<Fixup>,
    call: Option<InsideCall>,
    /// Where it goes back to in the main line.
    back: Option<usize>,
    /// The words that are branches to [`Self::back`], placed with offset 0.
    back_from: Vec<usize>,
}

/// Points the branch at `from` at the next byte.
fn land(code: &mut HostCode, from: usize) {
    let here = code.offset();
    let bytes = &code.bytes[from..from + INSTRUCTION as usize];
    let word = u32::from_le_bytes(bytes.try_into().expect("a word"));
    let moved = encode::retarget(word, here as i64 - from as i64).expect("a piece is short");
    code.patch(from, &moved.to_le_bytes());
}

impl Tail {
    fn place(self, code: &mut HostCode) {
        let after = code.snapshot();
        code.restore(self.state);
        land(code, self.from);
        if let Some(resume) = self.resume {
            code.after(resume);
        }
        for f in self.fixups {
            code.fixup(f);
        }
        for (k, w) in self.words.into_iter().enumerate() {
            let w = if self.back_from.contains(&k) {
                let back = self.back.expect("a branch back has a place to go");
                encode::retarget(w, back as i64 - code.offset() as i64).expect("a piece is short")
            } else {
                w
            };
            code.push(&w.to_le_bytes());
        }
        if let Some(call) = self.call {
            Arm64::call(code, call);
        }
        if let Some(back) = self.back {
            let here = code.offset() as i64;
            let word = encode::branch(false, back as i64 - here).expect("a piece is short");
            code.push(&word.to_le_bytes());
        }
        code.restore(after);
    }
}

impl Out<'_> {
    fn put(&mut self, word: u32) {
        self.code.push(&word.to_le_bytes());
    }

    fn put_all(&mut self, words: &[u32]) {
        for &w in words {
            self.put(w);
        }
    }

    fn commit(&mut self, word: u32, resume: Resume) {
        self.code.commit(&word.to_le_bytes(), resume);
    }

    fn next(&self) -> Resume {
        Resume::At(self.address.wrapping_add(INSTRUCTION))
    }

    fn call(&mut self, call: InsideCall) {
        Arm64::call(self.code, call);
    }

    /// A branch to be pointed at code placed later: written with offset 0, patched by [`Self::land`].
    fn branch(&mut self, word: u32) -> usize {
        let at = self.code.offset();
        self.put(encode::retarget(word, 0).expect("a branch"));
        at
    }

    fn land(&mut self, from: usize) {
        land(self.code, from);
    }

    /// A test's branch to code placed out of line after the piece.
    fn tail(&mut self, test: u32, tail: impl FnOnce(&mut Tail)) {
        let state = self.code.snapshot();
        let from = self.branch(test);
        let mut t = Tail {
            state,
            from,
            words: Vec::new(),
            resume: None,
            fixups: Vec::new(),
            call: None,
            back: None,
            back_from: Vec::new(),
        };
        tail(&mut t);
        self.code.defer(move |code| t.place(code));
    }

    /// `rd` built from `value` in place; the first move is the commit point.
    fn build(&mut self, rd: Reg, value: u64, resume: Resume) {
        if rd == Reg::Zero {
            self.code.after(resume);
            return;
        }
        let rd = number(rd);
        let moves = encode::move_constant(rd, value);
        self.commit(moves[0], resume);
        self.code.fixup(Fixup::Constant {
            register: rd,
            value,
        });
        self.put_all(&moves[1..]);
        self.code.fixed();
    }

    /// `rd` built from guest `address` in place, each word an address place; the first is the commit point.
    fn build_address(&mut self, rd: Reg, address: u64, resume: Resume) {
        if rd == Reg::Zero {
            self.code.after(resume);
            return;
        }
        let rd = number(rd);
        for (part, word) in address_words(rd, address, Holds::Address)
            .into_iter()
            .enumerate()
        {
            note_place(self.code, rd, address, Holds::Address, part);
            if part == 0 {
                self.commit(word, resume);
                self.code.fixup(Fixup::Address {
                    register: rd,
                    value: address,
                });
            } else {
                self.put(word);
            }
        }
        self.code.fixed();
    }
}

/// Every route but these takes stand-ins for the withheld registers its instruction names: a transfer reads
/// them itself, and the inside reads and writes them where they are.
const OWN_WAY: [Route; 4] = [Route::Transfer, Route::Kernel, Route::Trap, Route::Nothing];

/// Routes whose sequence needs t, which then cannot be a stand-in.
fn uses_scratch(route: Route) -> bool {
    !matches!(route, Route::Copy | Route::Constant)
}

/// `word` with registers `from` named `to` instead (a pair moves by its first register's field, and any other
/// field naming one of the pair moves with it): the register fields that name them as general registers are
/// found by trying them against the meaning's effects.
fn substitute(word: u32, from: &[Register], to: &[Register]) -> u32 {
    let form = forms::form(word).expect("a decoded word has a form");
    let fields: Vec<(encode::FieldName, Register)> = form
        .fields
        .iter()
        .zip(encode::field_names(form))
        .filter(|((name, _, width), _)| {
            name.starts_with(field::REGISTER) && *width == encode::REGISTER_BITS
        })
        .filter_map(|((name, ..), field)| {
            let named = form.field(word, name)?;
            let k = from.iter().position(|&f| u32::from(f) == named)?;
            Some((field, to[k]))
        })
        .collect();
    let original = decode(word);
    let (uses, defs) = touched(&original);
    let swap = |r: Registers| {
        from.iter().zip(to).fold(r, |r, (&f, &t)| {
            if r.has_bit(bit(f)) {
                r.minus(only(f)).union(only(t))
            } else {
                r
            }
        })
    };
    let all = (1u32 << fields.len()) - 1;
    (1..=all)
        .rev()
        .map(|subset| {
            fields
                .iter()
                .enumerate()
                .filter(|(k, _)| subset >> k & 1 != 0)
                .fold(word, |w, (_, &(f, r))| {
                    encode::replace(w, f, r.into()).expect("a register fits")
                })
        })
        .find(|&w| {
            let i = decode(w);
            Arm64::route(&i) == Arm64::route(&original) && touched(&i) == (swap(uses), swap(defs))
        })
        .unwrap_or_else(|| panic!("no stand-in for x{from:?} in {word:#010x}"))
}

/// A guest register moved to a stand-in around one instruction, and the saved slot of the stand-in's own value.
struct StandIn {
    from: Register,
    to: Register,
    slot: usize,
}

/// The host code of one guest instruction (docs/native.md section 3.4).
pub(crate) fn carry(i: &I, word: u32, address: u64, aligned: &AlignedBases, code: &mut HostCode) {
    let route = Arm64::route(i);
    let (uses, defs) = touched(i);
    let named = |r: Register| uses.union(defs).has_bit(bit(r));
    let mut out = Out {
        code,
        address,
        aligned,
    };
    if !WITHHELD.into_iter().any(named) || OWN_WAY.contains(&route) {
        carry_route(&mut out, route, i, word);
        return;
    }
    // What moves together: a register pair `casp` names by its first, else each withheld register named.
    let mut groups: Vec<Vec<Register>> = Vec::new();
    if let I::CompareSwap {
        pair: true, rs, rt, ..
    } = *i
    {
        for first in [number(rs), number(rt)] {
            let pair = vec![first, first + 1];
            if pair.iter().any(|r| WITHHELD.contains(r)) && !groups.contains(&pair) {
                groups.push(pair);
            }
        }
    }
    // Each other withheld register named moves alone (a `casp`'s base among them).
    for w in WITHHELD {
        if named(w) && !groups.iter().any(|g| g.contains(&w)) {
            groups.push(vec![w]);
        }
    }
    let mut avoid = uses.union(defs);
    let mut scratch_free = !uses_scratch(route);
    let mut stand_ins = Vec::new();
    let mut changed = word;
    // t stands in for the guest's own t first, so that no other register is moved onto a name it has.
    groups.sort_by_key(|g| !g.contains(&SCRATCH));
    for group in groups {
        let to: Vec<Register> = if group.len() > 1 {
            let first = free_pair(avoid);
            vec![first, first + 1]
        } else if scratch_free && (group[0] == SCRATCH || !named(SCRATCH)) {
            scratch_free = false;
            vec![SCRATCH]
        } else {
            vec![free(avoid)]
        };
        for &r in &to {
            avoid = avoid.union(only(r));
        }
        if group != to {
            changed = substitute(changed, &group, &to);
        }
        for (&from, &to) in group.iter().zip(&to) {
            let slot = stand_ins.len();
            stand_ins.push(StandIn { from, to, slot });
        }
    }
    out.code.slow(NativeReason::WithheldRegisterNamed);
    for s in &stand_ins {
        if s.to != SCRATCH {
            out.put(store_page(s.to, saved_slot(s.slot)));
            out.code.relocate(s.to, held_page(saved_slot(s.slot)));
        }
    }
    for s in &stand_ins {
        out.put(if WITHHELD.contains(&s.from) {
            load_page(s.to, withheld_slot(s.from))
        } else {
            encode::move_register(s.to, s.from)
        });
        out.code.relocate(s.from, Held::Register(s.to));
    }
    carry_route(&mut out, route, &decode(changed), changed);
    for s in &stand_ins {
        if defs.has_bit(bit(s.from)) {
            out.put(if WITHHELD.contains(&s.from) {
                store_page(s.to, withheld_slot(s.from))
            } else {
                encode::move_register(s.from, s.to)
            });
        }
        out.code.home(s.from);
    }
    for s in &stand_ins {
        if s.to != SCRATCH {
            out.put(load_page(s.to, saved_slot(s.slot)));
            out.code.home(s.to);
        }
    }
}

fn carry_route(out: &mut Out, route: Route, i: &I, word: u32) {
    let next = out.next();
    match route {
        Route::Copy => match i.memory_access() {
            Some(m) => access(out, &m, one_element(i), None, &[], word),
            None => out.commit(word, next),
        },
        Route::Split => access(
            out,
            &i.memory_access().expect("a load or store"),
            one_element(i),
            None,
            &[],
            word,
        ),
        Route::Aligned => {
            let m = i.memory_access().expect("an access");
            access(out, &m, false, Some(m.bytes), &[], word);
        }
        Route::Collision => collision(out, i, word),
        Route::Nothing => out.code.after(next),
        Route::Rebase => rebase(out, i),
        Route::Transfer => transfer(out, i, word),
        Route::Kernel => {
            out.call(InsideCall::Answer);
            out.code.after(next);
        }
        Route::Constant => match *i {
            I::ReadSystem {
                rt,
                register: SystemRegister::Fixed(value),
            } => out.build(rt, value, next),
            _ => unreachable!("only fixed registers are answered with a constant"),
        },
        Route::FloatStatus => float_status(out, i, word),
        Route::Trap => out.call(InsideCall::Trap),
        Route::Exclusive => exclusive(out, i),
    }
}

/// The address an access's first byte is at, built in t.
fn first_byte(address: &Address) -> Vec<u32> {
    match *address {
        Address::Offset { base, offset } | Address::PreIndex { base, offset } => {
            let mut words = encode::move_constant(SCRATCH, offset as u64);
            words.push(
                encode::add_extended(false, SCRATCH, number(base), SCRATCH, false, u64::BITS, 0)
                    .expect("registers fit"),
            );
            words
        }
        Address::PostIndex { base, .. } => {
            vec![encode::add_immediate(SCRATCH, number(base), 0).expect("zero fits")]
        }
        Address::Indexed {
            base,
            index,
            extend,
            shift,
        } => vec![
            encode::add_extended(
                false,
                SCRATCH,
                number(base),
                number(index),
                extend.signed,
                extend.from_bits.into(),
                shift.into(),
            )
            .expect("a load's extend fits an add's"),
        ],
        Address::Literal { .. } => unreachable!("a literal access is rebased"),
    }
}

/// The first byte's offset in its page, built in t.
fn page_offset(address: &Address) -> u32 {
    let page = PAGE_SIZE as i64;
    match *address {
        Address::Offset { base, offset } | Address::PreIndex { base, offset } => {
            encode::add_immediate(SCRATCH, number(base), offset.rem_euclid(page))
                .expect("an offset within a page fits")
        }
        _ => first_byte(address)[0],
    }
}

/// The page test in front of an access of `bytes` at `address`, built in t, its last word the branch to the
/// probes (offset 0) taken when the access may cross a page. An immediate offset of a power-of-two size takes
/// three words: the access crosses when its first byte's page offset plus `bytes - 1` reaches the page, that is
/// when the last byte's page offset is below `bytes - 1`, so the branch is taken when it is below `bytes` (a
/// first byte at a page's start is probed too, harmlessly).
fn page_test_words(address: &Address, bytes: u32) -> Vec<u32> {
    let last = i64::from(bytes) - 1;
    let page = PAGE_SIZE as i64;
    let offset = match *address {
        Address::Offset { offset, .. } | Address::PreIndex { offset, .. } => Some(offset),
        Address::PostIndex { .. } => Some(0),
        _ => None,
    };
    match (offset, base_of(address)) {
        (Some(offset), Some(base)) if bytes.is_power_of_two() => vec![
            encode::add_immediate(SCRATCH, number(base), (offset + last).rem_euclid(page))
                .expect("an offset within a page fits"),
            encode::and_immediate(SCRATCH, SCRATCH, (PAGE_SIZE - 1) & !(u64::from(bytes) - 1))
                .expect("the page offset's bits above the access's are contiguous"),
            encode::compare_branch(false, SCRATCH, 0).expect("fits"),
        ],
        _ => vec![
            page_offset(address),
            encode::and_immediate(SCRATCH, SCRATCH, PAGE_SIZE - 1).expect("a page mask"),
            encode::add_immediate(SCRATCH, SCRATCH, last).expect("an access is short"),
            encode::test_branch_nonzero(SCRATCH, PAGE_SIZE.trailing_zeros(), 0).expect("fits"),
        ],
    }
}

/// The base register of an access with one.
fn base_of(address: &Address) -> Option<Reg> {
    match *address {
        Address::Offset { base, .. }
        | Address::PreIndex { base, .. }
        | Address::PostIndex { base, .. }
        | Address::Indexed { base, .. } => Some(base),
        Address::Literal { .. } => None,
    }
}

/// Whether an access may cross a page. A stack pointer base is aligned to [`STACK_ALIGNMENT`] wherever an access
/// is made through it (the hardware checks it, SA0, before any byte is touched), so an immediate offset from it
/// stays in one page when it stays in one aligned run of that many bytes.
fn may_cross_page(m: &MemoryAccess) -> bool {
    let offset = match m.address {
        Address::Offset {
            base: Reg::Sp,
            offset,
        }
        | Address::PreIndex {
            base: Reg::Sp,
            offset,
        } => Some(offset),
        Address::PostIndex { base: Reg::Sp, .. } => Some(0),
        _ => None,
    };
    let run = STACK_ALIGNMENT as i64;
    offset.is_none_or(|offset| offset.rem_euclid(run) + i64::from(m.bytes) > run)
}

/// The slow path a page test in front of an access of `bytes` is.
fn page_test(bytes: u32) -> NativeReason {
    if bytes > u32::from(Q_BYTES) {
        NativeReason::AccessMayCrossPage
    } else {
        NativeReason::HostModelResultRouted
    }
}

/// Whether an access of `bytes` at `address` is at the address its base has modulo `bytes`, so that a base aligned
/// to `bytes` keeps an access of one element in one aligned run of that many bytes, which no page boundary splits.
fn aligned_with_base(address: &Address, bytes: u32) -> bool {
    let run = i64::from(bytes);
    bytes.is_power_of_two()
        && match *address {
            Address::Offset { base, offset } | Address::PreIndex { base, offset } => {
                base != Reg::Sp && offset.rem_euclid(run) == 0
            }
            Address::PostIndex { base, .. } => base != Reg::Sp,
            Address::Indexed { base, shift, .. } => {
                base != Reg::Sp && u32::from(shift) >= bytes.trailing_zeros()
            }
            Address::Literal { .. } => false,
        }
}

/// Whether an instruction accesses one element, not a pair.
fn one_element(i: &I) -> bool {
    matches!(
        i,
        I::Load { .. } | I::Store { .. } | I::LoadVector { .. } | I::StoreVector { .. }
    )
}

/// The one place a guest access is emitted (section 3.1): the alignment test when the form must be aligned
/// (`align` bytes), the page test and probes when it may cross a page, then `prefix` and the copied instruction
/// `word`, the commit point. An access of one element (`one_element`) at its base's alignment tests only that
/// the base is aligned to its size, and the exact page test runs out of line when it is not.
fn access(
    out: &mut Out,
    m: &MemoryAccess,
    one_element: bool,
    align: Option<u32>,
    prefix: &[u32],
    word: u32,
) {
    let base = base_of(&m.address).expect("a literal access is rebased");
    // The stack pointer's alignment is the hardware's to check (SA0).
    if let Some(align) = align.filter(|&a| a > 1 && base != Reg::Sp) {
        out.put(
            encode::and_immediate(SCRATCH, number(base), u64::from(align) - 1)
                .expect("a power of two less one is a logical immediate"),
        );
        out.tail(
            encode::compare_branch(true, SCRATCH, 0).expect("fits"),
            |t| t.call = Some(InsideCall::Misaligned),
        );
    }
    // A base tested once before the loop needs no test here.
    let tested_before =
        one_element && aligned_with_base(&m.address, m.bytes) && out.aligned.holds(base, m.bytes);
    if align.is_none() && m.bytes > 1 && may_cross_page(m) && !tested_before {
        out.code.slow(page_test(m.bytes));
        let test = page_test_words(&m.address, m.bytes);
        let (crossing, before) = test.split_last().expect("a test ends in its branch");
        let mut probes = Vec::new();
        // The probes go through t, so the check SA0 makes of a stack pointer base comes before them.
        if base == Reg::Sp {
            let skip = 2 * INSTRUCTION as i64;
            probes.extend([
                encode::add_immediate(SCRATCH, number(base), 0).expect("zero fits"),
                encode::and_immediate(SCRATCH, SCRATCH, STACK_ALIGNMENT - 1)
                    .expect("a power of two less one is a logical immediate"),
                encode::compare_branch(false, SCRATCH, skip).expect("fits"),
                call_word(InsideCall::Misaligned),
            ]);
        }
        probes.extend(first_byte(&m.address));
        let last = i64::from(m.bytes) - 1;
        if m.write {
            probes.push(encode::load_add(form::LDADDB, ZERO, ZERO, SCRATCH));
            probes.push(encode::add_immediate(SCRATCH, SCRATCH, last).expect("an access is short"));
            probes.push(encode::load_add(form::LDADDB, ZERO, ZERO, SCRATCH));
        } else {
            probes.push(encode::load_store(form::LDRB, ZERO, SCRATCH, 0, 1).expect("fits"));
            probes.push(
                encode::load_store(form::LDRB, ZERO, SCRATCH, last as u64, 1)
                    .expect("an access is short"),
            );
        }
        if one_element && aligned_with_base(&m.address, m.bytes) {
            out.put(
                encode::and_immediate(SCRATCH, number(base), u64::from(m.bytes) - 1)
                    .expect("a power of two less one is a logical immediate"),
            );
            let resume_at = out.code.offset() + INSTRUCTION as usize;
            // Out of line: the exact test, whose branch skips the way back when the access crosses.
            let skip = encode::retarget(*crossing, 2 * INSTRUCTION as i64).expect("fits");
            out.tail(
                encode::compare_branch(true, SCRATCH, 0).expect("fits"),
                |t| {
                    t.words.extend_from_slice(before);
                    t.words.push(skip);
                    t.back_from.push(t.words.len());
                    t.words.push(encode::branch(false, 0).expect("fits"));
                    t.words.extend(probes);
                    t.back = Some(resume_at);
                },
            );
        } else {
            out.put_all(before);
            let resume_at = out.code.offset() + INSTRUCTION as usize;
            out.tail(*crossing, |t| {
                t.words = probes;
                t.back = Some(resume_at);
            });
        }
    }
    out.put_all(prefix);
    let next = out.next();
    out.commit(word, next);
}

/// A load or store whose register fields collide where Arm leaves the result open: the colliding transfer
/// register becomes t, which gives the meaning's result (section 3.4).
fn collision(out: &mut Out, i: &I, word: u32) {
    let m = i.memory_access().expect("a load or store");
    let base = match m.address {
        Address::PreIndex { base, .. } | Address::PostIndex { base, .. } => Some(base),
        _ => None,
    };
    let replace = |w: u32, f: encode::FieldName, r: Register| {
        encode::replace(w, f, r.into()).expect("a register fits")
    };
    match *i {
        I::Load { .. } => access(out, &m, true, None, &[], replace(word, field::RT, SCRATCH)),
        I::Store { rt, .. } => {
            let save = encode::move_register(SCRATCH, number(rt));
            access(
                out,
                &m,
                true,
                None,
                &[save],
                replace(word, field::RT, SCRATCH),
            );
        }
        // The meaning writes rt2, then rt, then the base: into one register the first word lands.
        I::LoadPair { rt, rt2, .. } => {
            let first = if base == Some(rt) || (rt == rt2 && rt == Reg::Zero) {
                SCRATCH
            } else {
                number(rt)
            };
            let second = if rt == rt2 {
                ZERO
            } else if base == Some(rt2) {
                SCRATCH
            } else {
                number(rt2)
            };
            let changed = replace(replace(word, field::RT, first), field::RT2, second);
            access(out, &m, false, None, &[], changed);
        }
        I::LoadVectorPair { bytes, rt, .. } => vector_pair_into_one(out, &m, bytes, rt),
        I::StorePair { rt, rt2, .. } => {
            let base = base.expect("a store pair collides only with its written back base");
            let save = encode::move_register(SCRATCH, number(base));
            let pick = |r: Reg| if r == base { SCRATCH } else { number(r) };
            let changed = replace(replace(word, field::RT, pick(rt)), field::RT2, pick(rt2));
            access(out, &m, false, None, &[save], changed);
        }
        _ => unreachable!("only loads and stores collide"),
    }
}

/// A vector load pair into one register, which no host instruction does: the first element alone is loaded
/// from t, after the pair's page test, and a written back base gets its new value from t after it (a fix-up).
fn vector_pair_into_one(out: &mut Out, m: &MemoryAccess, bytes: u8, rt: u8) {
    let (base, offset, post) = match m.address {
        Address::Offset { base, offset } => (base, offset, None),
        Address::PreIndex { base, offset } => (base, offset, Some(false)),
        Address::PostIndex { base, offset } => (base, offset, Some(true)),
        _ => unreachable!("a pair has an immediate offset"),
    };
    let mut prefix = Vec::new();
    // The load goes through t unless the base is written back after it, so a stack pointer base gets the check
    // SA0 would make.
    if base == Reg::Sp && post != Some(true) {
        let skip = 2 * INSTRUCTION as i64;
        prefix.extend([
            encode::add_immediate(SCRATCH, ZERO, 0).expect("zero fits"),
            encode::and_immediate(SCRATCH, SCRATCH, STACK_ALIGNMENT - 1)
                .expect("a power of two less one is a logical immediate"),
            encode::compare_branch(false, SCRATCH, skip).expect("fits"),
            call_word(InsideCall::Misaligned),
        ]);
    }
    prefix.extend(first_byte(&Address::Offset { base, offset }));
    let name = match bytes {
        Q_BYTES => form::LDR_Q,
        X_BYTES => form::LDR_D,
        _ => form::LDR_S,
    };
    let from = if post == Some(true) {
        number(base)
    } else {
        SCRATCH
    };
    let load = encode::load_store(name, rt, from, 0, 1).expect("fits");
    access(out, m, false, None, &prefix, load);
    if post.is_some() {
        let base = number(base);
        out.code.fixup(Fixup::Move {
            register: base,
            from: SCRATCH,
        });
        out.put(encode::add_immediate(base, SCRATCH, 0).expect("zero fits"));
        out.code.fixed();
    }
}

/// A pc-relative instruction rebuilt on its guest address.
fn rebase(out: &mut Out, i: &I) {
    let at = out.address;
    let next = out.next();
    let (rt, bytes, offset, name) = match *i {
        I::PcRelative { rd, page, offset } => {
            let from = if page { at & !(ADRP_PAGE - 1) } else { at };
            return out.build_address(rd, from.wrapping_add_signed(offset), next);
        }
        I::Load {
            bytes,
            signed,
            rt,
            address: Address::Literal { offset },
        } => {
            let name = match (bytes, signed) {
                (_, Some(_)) => form::LDRSW,
                (X_BYTES, None) => form::LDR,
                _ => form::LDR_32,
            };
            (number(rt), bytes, offset, name)
        }
        I::LoadVector {
            bytes,
            rt,
            address: Address::Literal { offset },
        } => {
            let name = match bytes {
                Q_BYTES => form::LDR_Q,
                X_BYTES => form::LDR_D,
                _ => form::LDR_S,
            };
            (rt, bytes, offset, name)
        }
        _ => unreachable!("only pc-relative forms are rebased"),
    };
    let target = at.wrapping_add_signed(offset);
    for (part, word) in address_words(SCRATCH, target, Holds::Address)
        .into_iter()
        .enumerate()
    {
        note_place(out.code, SCRATCH, target, Holds::Address, part);
        out.put(word);
    }
    // Whether the access crosses a page is known here, from the guest address.
    let last = u64::from(bytes) - 1;
    if target % PAGE_SIZE + last >= PAGE_SIZE {
        out.code.slow(page_test(bytes.into()));
        out.put(encode::load_store(form::LDRB, ZERO, SCRATCH, 0, 1).expect("fits"));
        out.put(encode::load_store(form::LDRB, ZERO, SCRATCH, last, 1).expect("fits"));
    }
    let load = encode::load_store(name, rt, SCRATCH, 0, 1).expect("fits");
    out.commit(load, next);
}

/// The exit to the direct guest `target` (section 3.2): its entry slot's address built in t and a jump there,
/// the first word being the one a loader links. With `commit` that word is the current instruction's commit
/// point, so that a stop before it is at the instruction. A target beyond the mirror goes to the inside, which
/// reports it as its jump's would be.
pub(crate) fn exit(code: &mut HostCode, target: u64, commit: Option<Resume>) {
    if target >> REACH_BITS != 0 {
        if let Some(resume) = commit {
            code.after(resume);
        }
        Arm64::call(code, InsideCall::Exit);
        return;
    }
    let moves = address_words(SCRATCH, target, Holds::EntrySlot);
    let lead = code.lead_after_commit();
    if lead > 0 {
        let settle = encode::add_immediate(COUNT, COUNT, -i64::from(lead)).expect("a lead fits");
        code.settling_exit(&settle.to_le_bytes(), target, commit);
        note_place(code, SCRATCH, target, Holds::EntrySlot, 0);
        code.push(&moves[0].to_le_bytes());
    } else {
        note_place(code, SCRATCH, target, Holds::EntrySlot, 0);
        code.exit(&moves[0].to_le_bytes(), target, commit);
    }
    for (part, w) in moves.iter().enumerate().skip(1) {
        note_place(code, SCRATCH, target, Holds::EntrySlot, part);
        code.push(&w.to_le_bytes());
    }
    code.push(&encode::branch_register(form::BR, SCRATCH).to_le_bytes());
}

/// The words that build guest `address`, or its entry slot's address, in `rd`: as many for every address of a
/// movable page.
fn address_words(rd: Register, address: u64, holds: Holds) -> Vec<u32> {
    encode::move_address(
        rd,
        match holds {
            Holds::Address => address,
            Holds::EntrySlot => mirror::slot(address),
        },
    )
}

/// Notes that the next word written is word `part` of those building `address` in `rd`.
fn note_place(code: &mut HostCode, rd: Register, address: u64, holds: Holds, part: usize) {
    code.place(AddressPlace {
        start: 0,
        register: rd,
        part: u8::try_from(part).expect("a value builds in few words"),
        address,
        holds,
    });
}

/// The guest pages between which translated code moves by its address places alone (section 3.2): from one of
/// them `adrp`'s furthest page is still at or above zero, and a direct branch's furthest target from anywhere on
/// it is still under the mirror's reach, so no address the code names wraps or leaves the mirror, and each builds
/// in as many words.
pub(crate) fn movable() -> Range<u64> {
    encode::page_reach(ADRP_PAGE)..(1 << REACH_BITS) - encode::branch_reach() - PAGE_SIZE
}

/// Writes the word at `place` for its address.
pub(crate) fn write_place(code: &mut [u8], place: &AddressPlace) {
    let words = address_words(place.register, place.address, place.holds);
    let start = usize::try_from(place.start).expect("a host offset fits usize");
    code[start..start + size_of::<u32>()]
        .copy_from_slice(&words[usize::from(place.part)].to_le_bytes());
}

/// The words a loader fills: a slot's moves and jump, and a direct branch (section 3.2).
pub struct LinkWords {
    /// Each move of a slot, from the lowest part up: the word with its immediate clear, and its immediate's bits.
    pub moves: [(u32, u32); SLOT_MOVES],
    /// The slot's jump through t.
    pub jump: u32,
    /// A direct branch with no offset, and its offset's bits.
    pub branch: (u32, u32),
    /// An empty slot's first word.
    pub empty: u32,
    /// The first move of a jump's last target, as `moves`; the others are a slot's.
    pub last_target_first: (u32, u32),
    /// A jump's test once its last target is made: it goes past the call.
    pub last_target_test: u32,
    /// A jump's direct branch to its last target's piece before it is linked.
    pub last_target_branch: u32,
}

/// The words a loader fills, from the encoder.
pub fn link_words() -> LinkWords {
    let moves = std::array::from_fn(|k| {
        let half = k as u32;
        let clear = encode::movk(SCRATCH, 0, half);
        (clear, clear ^ encode::movk(SCRATCH, u16::MAX, half))
    });
    let clear = encode::branch(false, 0).expect("a branch");
    let back = encode::branch(false, -(INSTRUCTION as i64)).expect("a branch");
    LinkWords {
        moves,
        jump: encode::branch_register(form::BR, SCRATCH),
        branch: (clear, clear ^ back),
        empty: encode::exception(form::UDF, 0),
        last_target_first: {
            let clear = last_target_move(0, 0);
            (clear, clear ^ last_target_move(0, u16::MAX))
        },
        last_target_test: encode::compare_branch(
            true,
            SCRATCH,
            between(LAST_TARGET_TEST, LAST_TARGET_MISSED),
        )
        .expect("in reach"),
        last_target_branch: encode::branch(false, between(LAST_TARGET_BRANCH, LAST_TARGET_MISSED))
            .expect("in reach"),
    }
}

/// A branch, call or return (section 3.2): a direct target is left through its exit, a jump through a register
/// through the target's entry slot.
fn transfer(out: &mut Out, i: &I, word: u32) {
    let at = out.address;
    let next = out.next();
    let return_address = at.wrapping_add(INSTRUCTION);
    match *i {
        I::Branch { link, offset } => {
            let target = at.wrapping_add_signed(offset);
            if link {
                out.build_address(Reg::X(LINK), return_address, Resume::At(target));
                exit(out.code, target, None);
            } else {
                exit(out.code, target, Some(Resume::At(target)));
            }
        }
        I::BranchCond { offset, .. }
        | I::CompareBranch { offset, .. }
        | I::TestBranch { offset, .. } => {
            let mut word = word;
            if let I::CompareBranch { rt, .. } | I::TestBranch { rt, .. } = *i
                && WITHHELD.contains(&number(rt))
            {
                out.code.slow(NativeReason::WithheldRegisterNamed);
                out.put(load_page(SCRATCH, withheld_slot(number(rt))));
                word = encode::replace(word, field::RT, SCRATCH.into()).expect("a register fits");
            }
            let taken = out.code.offset();
            let unlinked = encode::retarget(word, 0).expect("a branch");
            out.put(unlinked);
            out.code.after(next);
            let target = at.wrapping_add_signed(offset);
            // The test's own branch goes straight to the target's piece once it is linked.
            if target >> REACH_BITS == 0 {
                let back = encode::retarget(word, -(INSTRUCTION as i64)).expect("a branch");
                out.code.branch_exit(taken, target, unlinked ^ back);
            }
            // The taken side's exit goes out of line; the run goes on at the next instruction.
            let state = out.code.snapshot();
            out.code.defer(move |code| {
                let after = code.snapshot();
                code.restore(state);
                land(code, taken);
                code.after(Resume::At(target));
                exit(code, target, None);
                code.restore(after);
            });
        }
        I::BranchRegister { op, rn } => jump(out, op, rn),
        _ => unreachable!("only transfers are routed as transfers"),
    }
}

/// The link register.
pub const LINK: Register = 30;

/// A jump through a register to the entry slot of its target (section 3.2).
fn jump(out: &mut Out, op: BranchRegister, rn: Reg) {
    let n = number(rn);
    let withheld = WITHHELD.contains(&n);
    let (source, held) = if withheld {
        (SCRATCH, held_page(withheld_slot(n)))
    } else {
        (n, Held::Register(n))
    };
    let return_address = out.address.wrapping_add(INSTRUCTION);
    // A call through any register but x30 builds x30 first, so that its last target's hit is one direct branch.
    let linked_first = op == BranchRegister::Blr && !withheld && n != LINK;
    if withheld {
        out.code.slow(NativeReason::WithheldRegisterNamed);
        out.put(load_page(SCRATCH, withheld_slot(n)));
    } else if op == BranchRegister::Ret {
        last_target(out, n);
    } else if linked_first {
        out.build_address(Reg::X(LINK), return_address, Resume::Through(held));
        last_target(out, n);
    }
    out.put(encode::ubfx(
        SCRATCH,
        source,
        REACH_BITS,
        TAG_LOW - REACH_BITS,
    ));
    out.tail(
        encode::compare_branch(true, SCRATCH, 0).expect("fits"),
        |t| {
            t.resume = Some(Resume::Through(held));
            if op == BranchRegister::Blr && !linked_first {
                t.fixups.push(Fixup::Address {
                    register: LINK,
                    value: return_address,
                });
            }
            t.call = Some(InsideCall::Unreachable);
        },
    );
    if withheld {
        out.put(load_page(SCRATCH, withheld_slot(n)));
    }
    out.put(encode::ubfiz(SCRATCH, source, SLOT_SHIFT, REACH_BITS));
    out.put(encode::bfxil(SCRATCH, SCRATCH, SLOT_SHIFT, ALIGNMENT_BITS));
    out.put(
        encode::eor_immediate(SCRATCH, SCRATCH, MIRROR)
            .expect("the mirror's base is a logical immediate"),
    );
    let slot = Resume::Slot(Held::Register(SCRATCH));
    match op {
        // A return goes by `br` too until calls push the return address's slot (the call stub): a `ret` would be
        // foreseen from a return-address stack that holds none of the guest's.
        BranchRegister::Br | BranchRegister::Ret => {
            out.commit(encode::branch_register(form::BR, SCRATCH), slot)
        }
        BranchRegister::Blr => {
            if !linked_first {
                out.build_address(Reg::X(LINK), return_address, slot);
            }
            out.put(encode::branch_register(form::BR, SCRATCH));
        }
    }
}

/// The branch table a `br` goes through when the instructions before it in its straight-line part show its
/// target to be a constant base plus a byte (section 3.2, "A jump table's known targets"): `br xN` where xN was
/// last written by `add xN, xB, xE` (shifted left or extended), xB holding a constant guest address (`adr`, or
/// `adrp` and `add`), and xE, no other register, last written by a one-byte load into xE itself, with nothing
/// after the add writing xE. Gives xN, xE and the table, whose start is where it is written.
fn known_targets(before: &[(u64, I)], i: &I) -> Option<(Register, Register, BranchTable)> {
    let I::BranchRegister {
        op: BranchRegister::Br,
        rn: Reg::X(n),
    } = *i
    else {
        return None;
    };
    let add = habits::last_def(before, Reg::X(n), before.len())?;
    let I::AddSub {
        width: Width::X64,
        subtract: false,
        set_flags: false,
        rn: base_register,
        operand,
        ..
    } = before[add].1
    else {
        return None;
    };
    let (index, shift, extend) = match operand {
        Operand::Shifted {
            rm,
            shift: Shift::Lsl,
            amount,
        } => (rm, amount, None),
        Operand::Extended { rm, extend, amount } => (rm, amount, Some(extend)),
        _ => return None,
    };
    let Reg::X(e) = index else {
        return None;
    };
    let written_after = |from: usize, r: Register| {
        before[from..]
            .iter()
            .any(|(_, later)| touched(later).1.has_bit(bit(r)))
    };
    if e == n || [n, e].iter().any(|r| WITHHELD.contains(r)) || written_after(add + 1, e) {
        return None;
    }
    let load = habits::last_def(before, index, add)?;
    let I::Load {
        bytes,
        signed: None,
        rt,
        address,
    } = before[load].1
    else {
        return None;
    };
    // The value added must be the byte itself, not a base the load writes back.
    let writes_back_index = matches!(
        address,
        Address::PreIndex { base, .. } | Address::PostIndex { base, .. } if base == index
    );
    if rt != index || writes_back_index || usize::from(bytes) != size_of::<u8>() {
        return None;
    }
    let base = habits::constant(before, base_register, add)?;
    Some((
        n,
        e,
        BranchTable {
            start: 0,
            base,
            entries: 1 << u8::BITS,
            shift,
            // Only a sign extension from the byte's own bits gives it a sign.
            signed: extend.is_some_and(|x| x.signed && u32::from(x.from_bits) <= u8::BITS),
            retarget: {
                let clear = encode::branch(false, 0).expect("a branch");
                clear ^ encode::branch(false, -(INSTRUCTION as i64)).expect("a branch")
            },
        },
    ))
}

/// A jump whose targets are known (see [`known_targets`]): `adr t, table; add t, t, xE, lsl #2; br t`, the `br`
/// its commit point, then the table of one direct branch for each value of the byte, which the loader points at
/// the target's piece and which until then goes to the jump through the target's slot written after it. Nothing
/// is read from memory and only branches the translator wrote are taken. False, with nothing written, for any
/// other jump.
pub(crate) fn known_jump(before: &[(u64, I)], i: &I, address: u64, code: &mut HostCode) -> bool {
    let Some((n, e, table)) = known_targets(before, i) else {
        return false;
    };
    // The table's branches are exits, which leave with the count the guest's.
    if code.lead_after_commit() != 0 {
        return false;
    }
    let word = INSTRUCTION as i64;
    let index = encode::add_extended(false, SCRATCH, SCRATCH, e, false, u64::BITS, ALIGNMENT_BITS)
        .expect("a shift by the alignment fits");
    let into_table = encode::branch_register(form::BR, SCRATCH);
    // The table starts after the adr, the add and the br.
    let lead_in = [index, into_table].len() as i64 + 1;
    code.push(
        &encode::adr(SCRATCH, lead_in * word)
            .expect("the table is near")
            .to_le_bytes(),
    );
    code.push(&index.to_le_bytes());
    // Past the commit point the guest is at the target, which xN still holds.
    code.commit(
        &into_table.to_le_bytes(),
        Resume::Through(Held::Register(n)),
    );
    code.branch_table(table);
    for k in 0..table.entries {
        let to_jump = i64::from(table.entries - k) * word;
        code.push(
            &encode::branch(false, to_jump)
                .expect("the jump after the table is near")
                .to_le_bytes(),
        );
    }
    let aligned = AlignedBases::default();
    let mut out = Out {
        code,
        address,
        aligned: &aligned,
    };
    jump(&mut out, BranchRegister::Br, Reg::X(n));
    true
}

/// The bytes from word `from` of a return's start to word `to`.
fn between(from: usize, to: usize) -> i64 {
    ((to - from) as u64 * INSTRUCTION) as i64
}

/// Move `k` of a jump's last target into t, from the lowest part up, with immediate `imm16`: a `movz`, then a
/// slot's `movk`s.
fn last_target_move(k: usize, imm16: u16) -> u32 {
    let half = k as u32;
    if k == 0 {
        encode::movz(SCRATCH, imm16, half)
    } else {
        encode::movk(SCRATCH, imm16, half)
    }
}

/// The moves of a jump's last target, with their immediates clear.
fn last_target_moves() -> [u32; SLOT_MOVES] {
    std::array::from_fn(|k| last_target_move(k, 0))
}

/// A return's or call's last target compared in line (docs/native.md section 3.2): on a match a direct branch the
/// inside links to that target's piece, otherwise on to the jump through the target's slot, by way of a call that has
/// the inside make this target the last one while there is none.
fn last_target(out: &mut Out, n: Register) {
    out.put_all(&last_target_moves());
    out.put(encode::logical_register(
        form::EOR_SHIFTED,
        SCRATCH,
        SCRATCH,
        n,
    ));
    out.put(
        encode::compare_branch(true, SCRATCH, between(LAST_TARGET_TEST, LAST_TARGET_CALL))
            .expect("in reach"),
    );
    out.put(
        encode::branch(false, between(LAST_TARGET_BRANCH, LAST_TARGET_MISSED)).expect("in reach"),
    );
    out.call(InsideCall::LastTarget);
}

/// A read or write of the float status or mode between two calls into the inside.
fn float_status(out: &mut Out, i: &I, word: u32) {
    let next = out.next();
    out.call(InsideCall::FloatBefore);
    match *i {
        I::WriteSystem {
            register: SystemRegister::Fpcr,
            rt,
        } => {
            out.put(
                encode::and_immediate(SCRATCH, number(rt), FPCR_BITS.into())
                    .expect("the presented bits are a logical immediate"),
            );
            out.commit(
                encode::replace(word, field::RT, SCRATCH.into()).expect("a register fits"),
                next,
            );
        }
        _ => out.commit(word, next),
    }
    out.code.fixup(Fixup::FloatStatus);
    out.call(InsideCall::FloatAfter);
    out.code.fixed();
}

/// The forms that load or swap `bytes` at a bare register.
fn sized(bytes: u32) -> (encode::Form, encode::Form, encode::Form) {
    match u8::try_from(bytes) {
        Ok(1) => (form::LDRB, form::LDADDB, form::CASB),
        Ok(H_BYTES) => (form::LDRH, form::LDADDH, form::CASH),
        Ok(W_BYTES) => (form::LDR_32, form::LDADD_32, form::CAS_32),
        _ => (form::LDR, form::LDADD_64, form::CAS),
    }
}

fn monitor(field: usize) -> u64 {
    page_word(field, 0)
}

/// The exclusive monitor kept in the register page as the meanings keep it (section 3.4, Exclusive).
fn exclusive(out: &mut Out, i: &I) {
    let next = out.next();
    let bytes_at = monitor(offset_of!(RegisterPage, exclusive_bytes));
    let address_at = monitor(offset_of!(RegisterPage, exclusive_address));
    let low_at = monitor(offset_of!(RegisterPage, exclusive_low));
    let high_at = monitor(offset_of!(RegisterPage, exclusive_high));
    match *i {
        I::ClearExclusive => out.commit(store_page(ZERO, bytes_at), next),
        I::LoadExclusive {
            bytes,
            pair,
            rt,
            rt2,
            rn,
        } => {
            let total = u32::from(bytes) * if pair { 2 } else { 1 };
            let m = i.memory_access().expect("an access");
            // The access goes through t, so a stack pointer base gets the check SA0 would make, and `ubfx`,
            // which reads register 31 as zero, takes it from t.
            let untag = if rn == Reg::Sp {
                let copy = encode::add_immediate(SCRATCH, number(rn), 0).expect("zero fits");
                out.put(copy);
                out.put(
                    encode::and_immediate(SCRATCH, SCRATCH, STACK_ALIGNMENT - 1)
                        .expect("a power of two less one is a logical immediate"),
                );
                out.tail(
                    encode::compare_branch(true, SCRATCH, 0).expect("fits"),
                    |t| t.call = Some(InsideCall::Misaligned),
                );
                vec![copy, encode::ubfx(SCRATCH, SCRATCH, 0, TAG_LOW)]
            } else {
                vec![encode::ubfx(SCRATCH, number(rn), 0, TAG_LOW)]
            };
            // A pair into the zero register twice loads its first element alone: the whole pair is in one page
            // once aligned.
            let load = if pair && rt == rt2 && rt == Reg::Zero {
                encode::load_store(sized(bytes.into()).0, ZERO, SCRATCH, 0, 1).expect("fits")
            } else if pair {
                let name = if bytes == X_BYTES {
                    form::LDP_64
                } else {
                    form::LDP_32
                };
                let first = if rt == rt2 { ZERO } else { number(rt) };
                encode::load_pair(name, first, number(rt2), SCRATCH)
            } else {
                encode::load_store(sized(total).0, number(rt), SCRATCH, 0, 1).expect("fits")
            };
            access(out, &m, false, Some(total), &untag, load);
            let remember = |address| Fixup::Remember {
                bytes,
                pair,
                address,
            };
            out.code.fixup(remember(Held::Register(SCRATCH)));
            out.put(store_page(SCRATCH, address_at));
            out.code.fixed();
            out.code.fixup(remember(held_page(address_at)));
            let whole = if pair && bytes == X_BYTES {
                u32::from(bytes)
            } else {
                total
            };
            let (load_name, ..) = sized(whole);
            out.put(encode::load_store(load_name, SCRATCH, SCRATCH, 0, 1).expect("fits"));
            out.put(store_page(SCRATCH, low_at));
            if pair && bytes == X_BYTES {
                out.put(load_page(SCRATCH, address_at));
                out.put(
                    encode::load_store(form::LDR, SCRATCH, SCRATCH, bytes.into(), bytes.into())
                        .expect("fits"),
                );
                out.put(store_page(SCRATCH, high_at));
            } else {
                out.put(store_page(ZERO, high_at));
            }
            out.put(encode::movz(SCRATCH, total as u16, 0));
            out.put(store_page(SCRATCH, bytes_at));
            out.code.fixed();
        }
        I::StoreExclusive {
            bytes,
            pair,
            rs,
            rt,
            rt2,
            rn,
        } => {
            let total = u32::from(bytes) * if pair { 2 } else { 1 };
            let base = number(rn);
            let status = number(rs);
            let (uses, defs) = touched(i);
            let mut avoid = uses.union(defs);
            if rn != Reg::Sp && total > 1 {
                out.put(
                    encode::and_immediate(SCRATCH, base, u64::from(total) - 1)
                        .expect("a power of two less one"),
                );
                out.tail(
                    encode::compare_branch(true, SCRATCH, 0).expect("fits"),
                    |t| t.call = Some(InsideCall::Misaligned),
                );
            }
            // Remembered for this size and address?
            out.put(load_page(SCRATCH, bytes_at));
            out.put(encode::eor_immediate(SCRATCH, SCRATCH, total.into()).expect("a power of two"));
            let wrong_size = out.branch(encode::compare_branch(true, SCRATCH, 0).expect("fits"));
            out.put(load_page(SCRATCH, address_at));
            out.put(
                encode::add_extended(true, SCRATCH, base, SCRATCH, false, u64::BITS, 0)
                    .expect("registers fit"),
            );
            out.put(
                encode::and_immediate(SCRATCH, SCRATCH, u64::MAX >> u8::BITS)
                    .expect("an address mask"),
            );
            let wrong_address = out.branch(encode::compare_branch(true, SCRATCH, 0).expect("fits"));
            let mismatch = out.code.snapshot();
            let first_borrowed = BORROWED;
            // The new value, in one register or an even pair; the expected one in t or an even pair.
            let mut borrowed = Vec::new();
            let mut borrow = |out: &mut Out, r: Register| {
                let slot = saved_slot(first_borrowed + borrowed.len());
                out.put(store_page(r, slot));
                out.code.relocate(r, held_page(slot));
                borrowed.push((r, slot));
            };
            let read = if pair && bytes == X_BYTES {
                let expected = free_pair(avoid);
                avoid = avoid.union(only(expected)).union(only(expected + 1));
                let new = free_pair(avoid);
                for r in [expected, expected + 1, new, new + 1] {
                    borrow(out, r);
                }
                out.put(load_page(expected, low_at));
                out.put(load_page(expected + 1, high_at));
                out.put(encode::move_register(new, number(rt)));
                out.put(encode::move_register(new + 1, number(rt2)));
                out.commit(
                    encode::compare_swap_of(form::CASP, expected, new, base),
                    next,
                );
                [
                    Some(Held::Register(expected)),
                    Some(Held::Register(expected + 1)),
                ]
            } else {
                let (.., cas) = sized(total);
                let new = if pair {
                    let s = free(avoid);
                    borrow(out, s);
                    out.put(encode::ubfx(s, number(rt), 0, u32::BITS));
                    out.put(encode::bfi(s, number(rt2), u32::BITS, u32::BITS));
                    s
                } else {
                    number(rt)
                };
                out.put(load_page(SCRATCH, low_at));
                out.commit(encode::compare_swap_of(cas, SCRATCH, new, base), next);
                [Some(Held::Register(SCRATCH)), None]
            };
            out.code.fixup(Fixup::Forget { status, read });
            out.put(store_page(ZERO, bytes_at));
            // The status: 0 if the swap read the remembered value.
            out.put(load_page(status, low_at));
            out.put(encode::logical_register(
                form::EOR_SHIFTED,
                status,
                status,
                match read[0] {
                    Some(Held::Register(r)) => r,
                    _ => SCRATCH,
                },
            ));
            let mut differ =
                vec![out.branch(encode::compare_branch(true, status, 0).expect("fits"))];
            if let [Some(_), Some(Held::Register(high))] = read {
                out.put(load_page(status, high_at));
                out.put(encode::logical_register(
                    form::EOR_SHIFTED,
                    status,
                    status,
                    high,
                ));
                differ.push(out.branch(encode::compare_branch(true, status, 0).expect("fits")));
            }
            let same = out.branch(encode::branch(false, 0).expect("fits"));
            for d in differ {
                out.land(d);
            }
            out.put(encode::movz(status, 1, 0));
            out.land(same);
            out.code.fixed();
            for &(r, slot) in borrowed.iter().rev() {
                out.put(load_page(r, slot));
                out.code.home(r);
            }
            let done = out.branch(encode::branch(false, 0).expect("fits"));
            // Not remembered: a write access that changes nothing, then status 1.
            out.code.restore(mismatch);
            out.land(wrong_size);
            out.land(wrong_address);
            let (_, add, _) = sized(total.min(X_BYTES.into()));
            out.commit(encode::load_add(add, ZERO, ZERO, base), next);
            out.code.fixup(Fixup::Forget {
                status,
                read: [None, None],
            });
            out.put(store_page(ZERO, bytes_at));
            out.put(encode::movz(status, 1, 0));
            out.code.fixed();
            out.land(done);
        }
        _ => unreachable!("only exclusive forms are routed as exclusive"),
    }
}
