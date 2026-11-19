//! The host code of every route (docs/native.md sections 3.2 to 3.4 and 4): the withheld registers and their
//! stand-ins, [`access`] as the one place a guest access is emitted, and each route's sequence with its
//! position map.

use std::mem::offset_of;

use wcontainers_abi::PAGE_SIZE;
use wcontainers_guest_cpu::{
    Fixup, Guest, Held, HostCode, InsideCall, Native, Registers, Resume, Route, Span, effects,
};

use crate::encode::{self, Register, ZERO, field, form};
use crate::forms;
use crate::instruction::{
    Address, BranchRegister, Instruction as I, MemoryAccess, Reg, SystemRegister,
};
use crate::meaning::Host;
use crate::mirror::{MIRROR, REACH_BITS, SLOT_SHIFT};
use crate::{Arm64, FPCR_BITS, decode};

/// The progress count of definition E (section 4).
pub const COUNT: Register = 26;
/// The scratch register t.
pub const SCRATCH: Register = 27;
/// The thread's register page, which the guest's level reads and writes.
pub const PAGE: Register = 28;
/// The withheld registers, in the order the register page keeps their guest values.
pub const WITHHELD: [Register; 3] = [COUNT, SCRATCH, PAGE];

/// Guest registers one sequence moves or borrows at most at once: `casp`'s two even pairs.
const BORROWED: usize = 4;

/// The thread's register page (section 3.3).
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegisterPage {
    /// The guest's values of the withheld registers.
    pub withheld: [u64; WITHHELD.len()],
    /// A stand-in's own value while it holds another register's, then the registers a sequence borrows.
    pub saved: [u64; 2 * BORROWED],
    /// The exclusive monitor, as the meanings keep it.
    pub exclusive_bytes: u64,
    pub exclusive_address: u64,
    pub exclusive_low: u64,
    pub exclusive_high: u64,
}

/// The bytes of a 64-bit, a 32-bit and a 128-bit register.
const X_BYTES: u8 = size_of::<u64>() as u8;
const W_BYTES: u8 = size_of::<u32>() as u8;
const Q_BYTES: u8 = size_of::<u128>() as u8;
/// The bytes of a half-word access.
const H_BYTES: u8 = size_of::<u16>() as u8;
/// The stack pointer's alignment when it is a base (SA0): a quadword.
pub const STACK_ALIGNMENT: u64 = size_of::<u128>() as u64;

/// The bytes of one instruction.
const INSTRUCTION: u64 = Arm64::MAX_LENGTH;

/// A call into the inside: `brk` with the call's code.
pub fn call_word(call: InsideCall) -> u32 {
    encode::exception(form::BRK, call.code())
}
/// The low bits of a branch target that an aligned one has clear.
const ALIGNMENT_BITS: u32 = INSTRUCTION.trailing_zeros();
/// The lowest bit of the tag the top byte holds.
pub const TAG_LOW: u32 = u64::BITS - u8::BITS;
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
        for w in self.words {
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
    let fields: Vec<(&str, Register)> = form
        .fields
        .iter()
        .filter(|(name, _, width)| {
            name.starts_with(field::REGISTER) && *width == encode::REGISTER_BITS
        })
        .filter_map(|(name, ..)| {
            let named = form.field(word, name)?;
            let k = from.iter().position(|&f| u32::from(f) == named)?;
            Some((name.as_str(), to[k]))
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
pub(crate) fn carry(i: &I, word: u32, address: u64, code: &mut HostCode) {
    let route = Arm64::route(i);
    let (uses, defs) = touched(i);
    let named = |r: Register| uses.union(defs).has_bit(bit(r));
    let mut out = Out { code, address };
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
            Some(m) => access(out, &m, None, &[], word),
            None => out.commit(word, next),
        },
        Route::Split => access(
            out,
            &i.memory_access().expect("a load or store"),
            None,
            &[],
            word,
        ),
        Route::Aligned => {
            let m = i.memory_access().expect("an access");
            access(out, &m, Some(m.bytes), &[], word);
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

/// The one place a guest access is emitted (section 3.1): the alignment test when the form must be aligned
/// (`align` bytes), the page test and probes when it may cross a page, then `prefix` and the copied instruction
/// `word`, the commit point.
fn access(out: &mut Out, m: &MemoryAccess, align: Option<u32>, prefix: &[u32], word: u32) {
    let base = match m.address {
        Address::Offset { base, .. }
        | Address::PreIndex { base, .. }
        | Address::PostIndex { base, .. }
        | Address::Indexed { base, .. } => base,
        Address::Literal { .. } => unreachable!("a literal access is rebased"),
    };
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
    if align.is_none() && m.bytes > 1 {
        let page_bits = PAGE_SIZE.trailing_zeros();
        out.put(page_offset(&m.address));
        out.put(encode::and_immediate(SCRATCH, SCRATCH, PAGE_SIZE - 1).expect("a page mask"));
        out.put(
            encode::add_immediate(SCRATCH, SCRATCH, i64::from(m.bytes) - 1)
                .expect("an access is short"),
        );
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
        let resume_at = out.code.offset() + INSTRUCTION as usize;
        out.tail(
            encode::test_branch_nonzero(SCRATCH, page_bits, 0).expect("fits"),
            |t| {
                t.words = probes;
                t.back = Some(resume_at);
            },
        );
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
    let replace =
        |w: u32, f: &str, r: Register| encode::replace(w, f, r.into()).expect("a register fits");
    match *i {
        I::Load { .. } => access(out, &m, None, &[], replace(word, field::RT, SCRATCH)),
        I::Store { rt, .. } => {
            let save = encode::move_register(SCRATCH, number(rt));
            access(out, &m, None, &[save], replace(word, field::RT, SCRATCH));
        }
        I::LoadPair { rt, rt2, .. } => {
            let first = if rt == rt2 || base == Some(rt) {
                SCRATCH
            } else {
                number(rt)
            };
            let second = match base == Some(rt2) {
                true if first == SCRATCH => ZERO,
                true => SCRATCH,
                false => number(rt2),
            };
            let changed = replace(replace(word, field::RT, first), field::RT2, second);
            access(out, &m, None, &[], changed);
        }
        I::StorePair { rt, rt2, .. } => {
            let base = base.expect("a store pair collides only with its written back base");
            let save = encode::move_register(SCRATCH, number(base));
            let pick = |r: Reg| if r == base { SCRATCH } else { number(r) };
            let changed = replace(replace(word, field::RT, pick(rt)), field::RT2, pick(rt2));
            access(out, &m, None, &[save], changed);
        }
        _ => unreachable!("only loads and stores collide"),
    }
}

/// A pc-relative instruction rebuilt on its guest address.
fn rebase(out: &mut Out, i: &I) {
    let at = out.address;
    let next = out.next();
    let (rt, bytes, offset, name) = match *i {
        I::PcRelative { rd, page, offset } => {
            let from = if page { at & !(ADRP_PAGE - 1) } else { at };
            return out.build(rd, from.wrapping_add_signed(offset), next);
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
    out.put_all(&encode::move_constant(SCRATCH, target));
    // Whether the access crosses a page is known here, from the guest address.
    let last = u64::from(bytes) - 1;
    if target % PAGE_SIZE + last >= PAGE_SIZE {
        out.put(encode::load_store(form::LDRB, ZERO, SCRATCH, 0, 1).expect("fits"));
        out.put(encode::load_store(form::LDRB, ZERO, SCRATCH, last, 1).expect("fits"));
    }
    let load = encode::load_store(name, rt, SCRATCH, 0, 1).expect("fits");
    out.commit(load, next);
}

/// A branch, call or return (section 3.2), at look-ahead none: every exit goes to the inside.
fn transfer(out: &mut Out, i: &I, word: u32) {
    let at = out.address;
    let next = out.next();
    let return_address = at.wrapping_add(INSTRUCTION);
    match *i {
        I::Branch { link, offset } => {
            let target = Resume::At(at.wrapping_add_signed(offset));
            if link {
                out.build(Reg::X(LINK), return_address, target);
            } else {
                out.code.after(target);
            }
            out.call(InsideCall::Exit);
        }
        I::BranchCond { offset, .. }
        | I::CompareBranch { offset, .. }
        | I::TestBranch { offset, .. } => {
            let mut word = word;
            if let I::CompareBranch { rt, .. } | I::TestBranch { rt, .. } = *i
                && WITHHELD.contains(&number(rt))
            {
                out.put(load_page(SCRATCH, withheld_slot(number(rt))));
                word = encode::replace(word, field::RT, SCRATCH.into()).expect("a register fits");
            }
            let taken = out.code.offset();
            out.put(encode::retarget(word, 0).expect("a branch"));
            out.code.after(next);
            out.call(InsideCall::Exit);
            out.land(taken);
            out.code.after(Resume::At(at.wrapping_add_signed(offset)));
            out.call(InsideCall::Exit);
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
    if withheld {
        out.put(load_page(SCRATCH, withheld_slot(n)));
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
            if op == BranchRegister::Blr {
                t.fixups.push(Fixup::Constant {
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
        BranchRegister::Br => out.commit(encode::branch_register(form::BR, SCRATCH), slot),
        BranchRegister::Ret => out.commit(encode::branch_register(form::RET, SCRATCH), slot),
        BranchRegister::Blr => {
            out.build(Reg::X(LINK), return_address, slot);
            out.put(encode::branch_register(form::BR, SCRATCH));
        }
    }
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
fn sized(bytes: u32) -> (&'static str, &'static str, &'static str) {
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
            let load = if pair {
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
            access(out, &m, Some(total), &untag, load);
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
