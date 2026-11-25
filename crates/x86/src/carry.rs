//! The carrying code of the native output: host instructions written around a guest instruction, from iced's
//! encoder (docs/x86-native-output.md sections 3 and 4), for each way of running.

use iced_x86::{
    Code, ConditionCode, DecoderOptions, Encoder, Instruction as Iced, InstructionInfoFactory,
    MemoryOperand, Mnemonic, OpKind, Register,
};
use wcontainers_guest_cpu::position::{AddressPlace, DirectExit, Fixup, Held, Holds, Resume};
use wcontainers_guest_cpu::{HostCode, InsideCall, NativeReason, PATCH_BYTES, Patch, Route};

mod window;

use crate::USER_END;
use crate::area::{
    ENTRIES, FIRST_CALL_VECTOR, FIRST_LEVEL, GUEST_FS_BASE, GUEST_GS_BASE, HOME_VALUES,
    STAND_IN_VALUES, STOP_WORD, WITHHELD_VALUES, X87_CONTROL, X87_IP,
};
use crate::decode::Instruction;
use crate::native::Way;
use crate::operand::{General, R8, R10, R11, R15, RAX, RBP, RSP, SLOT, W64, general};

/// The withheld registers: the count c, and the scratch registers t and u (section 3.4's first guess).
pub const COUNT: Register = Register::R10;
pub const T: Register = Register::R11;
pub const U: Register = Register::R15;
/// Their numbers, in the register area's order.
pub const WITHHELD: [u8; crate::area::WITHHELD] = [R10, R11, R15];
const T_NUMBER: u8 = WITHHELD[1];
const U_NUMBER: u8 = WITHHELD[2];
/// The guest register number of the FS base in a span's relocations: an operand on the thread area's segment holds
/// it in u.
pub const FS_BASE_NUMBER: u8 = crate::operand::FS;
const T_LOW: Register = Register::R11D;
const U_LOW: Register = Register::R15D;

/// Bytes of a word of the thread area, as its fields are laid out.
pub const AREA_WORD: usize = wcontainers_abi::WORD as usize;
/// The registers a stand-in may be, in the order they are taken: every general register but rsp and the
/// withheld ones.
fn stand_ins() -> impl Iterator<Item = u8> {
    (crate::operand::RAX..=crate::operand::R15).filter(|n| *n != RSP && !WITHHELD.contains(n))
}

/// Where `mov r64, imm64`'s immediate starts: after its REX prefix and opcode.
const IMM64_AT: usize = 2;
/// The bytes of a rel32 displacement.
pub const REL32: usize = size_of::<i32>();

/// Whether the instruction names a withheld register, which a stand-in must then replace.
pub fn names_withheld(i: &Iced) -> bool {
    let withheld = |r: Register| general(r).is_some_and(|g| WITHHELD.contains(&g.number));
    (0..i.op_count()).any(|n| i.op_kind(n) == OpKind::Register && withheld(i.op_register(n)))
        || withheld(i.memory_base())
        || withheld(i.memory_index())
}

/// iced's register of general register `number` at `width` bits (the low byte for 8).
fn sized(number: u8, width: u8) -> Register {
    Register::values()
        .find(|&r| {
            general(r)
                == Some(General {
                    number,
                    width,
                    high: false,
                })
        })
        .expect("every general register has every width")
}

fn full(number: u8) -> Register {
    sized(number, W64)
}

fn bytes_of(i: &Iced, at: u64) -> Vec<u8> {
    let mut encoder = Encoder::new(crate::MODE_BITS);
    encoder
        .encode(i, at)
        .unwrap_or_else(|e| panic!("the carrying code's instructions encode: {e} in {i:?} {i}"));
    encoder.take_buffer()
}

/// Writes one host instruction under the current state.
fn put(code: &mut HostCode, i: &Iced) {
    let at = code.offset() as u64;
    code.push(&bytes_of(i, at));
}

fn on_area(way: Way, offset: usize, displacement: i64) -> MemoryOperand {
    let offset = i64::try_from(offset).expect("an area offset is small");
    MemoryOperand {
        displ_size: AREA_WORD as u32,
        segment_prefix: way.area_segment(),
        ..MemoryOperand::with_base_displ(Register::None, offset + displacement)
    }
}

/// `mov r64, seg:[offset]` of the thread area.
fn load_area(way: Way, r: Register, offset: usize) -> Iced {
    Iced::with2(Code::Mov_r64_rm64, r, on_area(way, offset, 0)).expect("a load from the area")
}

/// `mov seg:[offset], r64` of the thread area.
fn store_area(way: Way, offset: usize, r: Register) -> Iced {
    Iced::with2(Code::Mov_rm64_r64, on_area(way, offset, 0), r).expect("a store to the area")
}

fn withheld_slot(number: u8) -> usize {
    let k = WITHHELD
        .iter()
        .position(|&w| w == number)
        .expect("a withheld register");
    WITHHELD_VALUES + k * AREA_WORD
}

fn page(offset: usize) -> Held {
    Held::Page(u16::try_from(offset).expect("the register area is one page"))
}

/// Writes `mov r, imm64` building guest `address`, an address place that moves with the code.
fn place_address(code: &mut HostCode, r: Register, address: u64) {
    let number = general(r).expect("a general register").number;
    code.place(AddressPlace {
        start: 0,
        register: number,
        part: 0,
        address,
        holds: Holds::Address,
    });
    put(
        code,
        &Iced::with2(Code::Mov_r64_imm64, r, address).expect("a constant of a whole register"),
    );
}

/// The `lea` that adds `by` to the count, its displacement always 32 bits wide so that a link can write a
/// `jmp rel32` over it.
fn count_add(by: i64) -> Iced {
    Iced::with2(
        Code::Lea_r64_m,
        COUNT,
        MemoryOperand::with_base_displ_size(COUNT, by, AREA_WORD as u32),
    )
    .expect("the count's add")
}

/// `jmp rel32` from host offset `at` to host offset `to`.
fn jump(at: usize, to: usize) -> Vec<u8> {
    bytes_of(
        &Iced::with_branch(Code::Jmp_rel32_64, to as u64).expect("a jump"),
        at as u64,
    )
}

/// The length of `jmp rel32`.
fn jump_length() -> usize {
    jump(0, 0).len()
}

/// The guest instruction with each withheld register it names replaced by a stand-in, and the stand-ins: each
/// withheld register's number with the register standing in for it.
fn stand_in(i: &Iced) -> (Iced, Vec<(u8, u8)>) {
    let mut info = InstructionInfoFactory::new();
    let used: Vec<u8> = info
        .info(i)
        .used_registers()
        .iter()
        .filter_map(|u| general(u.register()).map(|g| g.number))
        .collect();
    let mut free: Vec<u8> = stand_ins().filter(|n| !used.contains(n)).collect();
    let mut swaps: Vec<(u8, u8)> = Vec::new();
    // A stand-in for the base is never rbp, whose accesses take the stack segment's fault.
    let mut swapped = |r: Register, base: bool, swaps: &mut Vec<(u8, u8)>| -> Register {
        let Some(g) = general(r).filter(|g| WITHHELD.contains(&g.number)) else {
            return r;
        };
        let s = match swaps.iter().find(|(w, _)| *w == g.number) {
            Some(&(_, s)) => s,
            None => {
                let at = free
                    .iter()
                    .position(|&n| !(base && n == RBP))
                    .expect("an instruction leaves a register free");
                let s = free.remove(at);
                swaps.push((g.number, s));
                s
            }
        };
        sized(s, g.width)
    };
    let mut out = *i;
    out.set_memory_base(swapped(i.memory_base(), true, &mut swaps));
    for n in 0..i.op_count() {
        if i.op_kind(n) == OpKind::Register {
            out.set_op_register(n, swapped(i.op_register(n), false, &mut swaps));
        }
    }
    out.set_memory_index(swapped(i.memory_index(), false, &mut swaps));
    (out, swaps)
}

/// Whether guest instruction `i` reads general register `number` (a write of fewer than 32 bits keeps the rest, so
/// it reads too), and whether it writes it.
fn reads_writes(i: &Iced, number: u8) -> (bool, bool) {
    use iced_x86::OpAccess::*;
    let mut info = InstructionInfoFactory::new();
    let mut found = (false, false);
    for u in info.info(i).used_registers() {
        let Some(g) = general(u.register()).filter(|g| g.number == number) else {
            continue;
        };
        let partial = g.width < crate::operand::W32;
        match u.access() {
            Read | CondRead => found.0 = true,
            Write => {
                found.0 |= partial;
                found.1 = true;
            }
            _ => found = (true, true),
        }
    }
    found
}

/// The moves that load each stand-in with its withheld register's guest value where `guest` reads it, keeping the
/// stand-in's own in the area.
fn stand_ins_in(way: Way, code: &mut HostCode, guest: &Iced, swaps: &[(u8, u8)]) {
    for (k, &(w, s)) in swaps.iter().enumerate() {
        let own = STAND_IN_VALUES + k * AREA_WORD;
        put(code, &store_area(way, own, full(s)));
        code.relocate(s, page(own));
        if reads_writes(guest, w).0 {
            put(code, &load_area(way, full(s), withheld_slot(w)));
        }
    }
}

/// After the instruction: each withheld register's value back to the area where `guest` writes it, and each
/// stand-in's own value back.
fn stand_ins_out(way: Way, code: &mut HostCode, guest: &Iced, swaps: &[(u8, u8)]) {
    let written: Vec<bool> = swaps
        .iter()
        .map(|&(w, _)| reads_writes(guest, w).1)
        .collect();
    for (&(w, s), &written) in swaps.iter().zip(&written) {
        if written {
            code.relocate(w, Held::Register(s));
        }
    }
    for (k, (&(w, s), &written)) in swaps.iter().zip(&written).enumerate() {
        if written {
            put(code, &store_area(way, withheld_slot(w), full(s)));
            code.home(w);
        }
        put(
            code,
            &load_area(way, full(s), STAND_IN_VALUES + k * AREA_WORD),
        );
        code.home(s);
    }
}

/// The registers that hold the withheld registers a loop names throughout its copy (section 3.4): each withheld
/// register's number with the one holding it, the k-th keeping its own guest value at `HOME_VALUES + 8 * k`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Homes {
    pairs: Vec<(u8, u8)>,
}

impl Homes {
    /// Each withheld register the part names with a register the part never uses (never rbp, whose accesses take
    /// the stack segment's fault), or none where too few are free: no stand-in is then needed in the loop.
    pub fn of(part: &[(u64, Instruction)]) -> Self {
        let mut info = InstructionInfoFactory::new();
        let mut used = [false; crate::operand::R15 as usize + 1];
        for (_, i) in part {
            for u in info.info(&i.iced).used_registers() {
                if let Some(g) = general(u.register()) {
                    used[usize::from(g.number)] = true;
                }
            }
        }
        let wanted: Vec<u8> = WITHHELD
            .iter()
            .copied()
            .filter(|&w| used[usize::from(w)])
            .collect();
        let free: Vec<u8> = stand_ins()
            .filter(|&n| n != RBP && !used[usize::from(n)])
            .collect();
        if free.len() < wanted.len() {
            return Self::default();
        }
        Self {
            pairs: wanted.into_iter().zip(free).collect(),
        }
    }

    /// The register holding `r`'s general register throughout, at `r`'s width, or `r`.
    fn of_register(&self, r: Register) -> Register {
        general(r)
            .and_then(|g| {
                self.pairs
                    .iter()
                    .find(|(w, _)| *w == g.number)
                    .map(|&(_, s)| sized(s, g.width))
            })
            .unwrap_or(r)
    }

    /// The guest instruction naming the holding registers in place of the withheld ones.
    fn renamed(&self, i: &Instruction) -> Instruction {
        let mut out = *i;
        out.iced
            .set_memory_base(self.of_register(i.iced.memory_base()));
        out.iced
            .set_memory_index(self.of_register(i.iced.memory_index()));
        for n in 0..i.iced.op_count() {
            if i.iced.op_kind(n) == OpKind::Register {
                out.iced
                    .set_op_register(n, self.of_register(i.iced.op_register(n)));
            }
        }
        out
    }

    /// The map throughout the current instruction: each withheld register in its holder, the holder's own value
    /// in the area.
    fn throughout(&self, code: &mut HostCode) {
        for (k, &(w, s)) in self.pairs.iter().enumerate() {
            code.relocate_throughout(s, page(HOME_VALUES + k * AREA_WORD));
            code.relocate_throughout(w, Held::Register(s));
        }
    }

    /// The moves into the holders: each holder's own value to the area, the withheld register's into it.
    fn enter(&self, way: Way, code: &mut HostCode) {
        for (k, &(w, s)) in self.pairs.iter().enumerate() {
            let own = HOME_VALUES + k * AREA_WORD;
            put(code, &store_area(way, own, full(s)));
            code.relocate(s, page(own));
            put(code, &load_area(way, full(s), withheld_slot(w)));
            code.relocate(w, Held::Register(s));
        }
    }

    /// The moves back: each withheld register's value to its slot, each holder's own back.
    fn leave(&self, way: Way, code: &mut HostCode) {
        for (k, &(w, s)) in self.pairs.iter().enumerate() {
            put(code, &store_area(way, withheld_slot(w), full(s)));
            code.home(w);
            put(code, &load_area(way, full(s), HOME_VALUES + k * AREA_WORD));
            code.home(s);
        }
    }
}

/// Before a loop's copy (`Native::test_loop_invariant`): the moves into the homes and a `jmp rel32` to the copy,
/// linked later; gives the offset of its rel32, or none where the loop names no withheld register.
pub fn enter_homes(way: Way, code: &mut HostCode, homes: &Homes) -> Option<usize> {
    if homes.pairs.is_empty() {
        return None;
    }
    homes.enter(way, code);
    let at = code.offset();
    code.push(&jump(at, at + jump_length()));
    for &(w, s) in &homes.pairs {
        code.home(w);
        code.home(s);
    }
    Some(at + jump_length() - REL32)
}

/// The instruction with the one withheld register it names, as a whole operand and nothing else, read or written
/// in its slot of the register area instead (section 3.4): one host instruction, no stand-in. None where its form
/// takes no memory there, the instruction has a memory operand already, or the memory form would act otherwise: a
/// 32-bit write (which zero-extends only in a register), memory of another size than the register, a bit test
/// with a register offset (which reaches past its operand in memory), `xchg` (which locks memory), or a result
/// Intel leaves undefined, which the probe found the host to match for the register form only.
fn on_slot(way: Way, i: &Iced) -> Option<Iced> {
    use iced_x86::{OpAccess, OpCodeOperandKind as Kind};
    if has_memory_operand(i)
        || crate::native::undefined_result(i.mnemonic())
        || matches!(
            i.mnemonic(),
            Mnemonic::Xchg | Mnemonic::Bt | Mnemonic::Bts | Mnemonic::Btr | Mnemonic::Btc
        )
    {
        return None;
    }
    let withheld = |r: Register| general(r).filter(|g| WITHHELD.contains(&g.number));
    let named: Vec<u32> = (0..i.op_count())
        .filter(|&n| i.op_kind(n) == OpKind::Register && withheld(i.op_register(n)).is_some())
        .collect();
    let &[n] = named.as_slice() else {
        return None;
    };
    let g = withheld(i.op_register(n))?;
    let mut info = InstructionInfoFactory::new();
    let uses: Vec<OpAccess> = info
        .info(i)
        .used_registers()
        .iter()
        .filter(|u| general(u.register()).is_some_and(|u| u.number == g.number))
        .map(|u| u.access())
        .collect();
    let &[access] = uses.as_slice() else {
        return None;
    };
    let writes = !matches!(access, OpAccess::Read | OpAccess::CondRead);
    if writes && g.width == crate::operand::W32 {
        return None;
    }
    // Register-only operands and the operands that may be memory, by width.
    let pairs = [
        (Kind::r8_reg, Kind::r8_or_mem),
        (Kind::r16_reg, Kind::r16_or_mem),
        (Kind::r32_reg, Kind::r32_or_mem),
        (Kind::r64_reg, Kind::r64_or_mem),
        (Kind::r8_opcode, Kind::r8_or_mem),
        (Kind::r16_opcode, Kind::r16_or_mem),
        (Kind::r32_opcode, Kind::r32_or_mem),
        (Kind::r64_opcode, Kind::r64_or_mem),
    ];
    let may_be_memory = |k: Kind| pairs.iter().any(|&(_, m)| m == k);
    let info = i.code().op_code();
    let kinds: Vec<Kind> = (0..info.op_count()).map(|k| info.op_kind(k)).collect();
    let code = if may_be_memory(kinds[n as usize]) {
        i.code()
    } else {
        // The same operation with the operand register-only here taking memory, and the other register operand,
        // if it took memory, register-only: `mov r/m32, r32` for `mov r32, r/m32`.
        let flipped: Vec<Kind> = kinds
            .iter()
            .enumerate()
            .map(|(k, &kind)| {
                pairs
                    .iter()
                    .find_map(|&(register, memory)| {
                        if k == n as usize && kind == register {
                            Some(memory)
                        } else if k != n as usize && kind == memory {
                            Some(register)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(kind)
            })
            .collect();
        Code::values().find(|c| {
            let o = c.op_code();
            c.mnemonic() == i.mnemonic()
                && o.encoding() == info.encoding()
                && o.operand_size() == info.operand_size()
                && o.mode64()
                && (0..o.op_count())
                    .map(|k| o.op_kind(k))
                    .eq(flipped.iter().copied())
        })?
    };
    let mut out = *i;
    out.set_code(code);
    out.set_op_kind(n, OpKind::Memory);
    let slot = on_area(way, withheld_slot(g.number), 0);
    out.set_memory_base(Register::None);
    out.set_memory_index(Register::None);
    out.set_memory_index_scale(1);
    out.set_memory_displacement64(slot.displacement as u64);
    out.set_memory_displ_size(slot.displ_size);
    out.set_segment_prefix(slot.segment_prefix);
    let mut encoder = Encoder::new(crate::MODE_BITS);
    encoder.encode(&out, 0).ok()?;
    let register_bytes = usize::from(g.width) / u8::BITS as usize;
    (out.memory_size().size() == register_bytes).then_some(out)
}

/// How the access of a rewritten instruction reaches its operand.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// As the guest names it.
    Plain,
    /// RIP-relative, rebuilt on the guest address in t.
    Rebase,
    /// On the thread area's segment, made on FS while FS holds the guest's GS base ([`enter_guest_segment`]).
    GuestSegment,
}

/// What a host instruction that may reach guest memory is written from.
enum Written<'a> {
    /// The guest's own bytes.
    Bytes(&'a [u8]),
    Instruction(&'a Iced),
}

/// Writes a host instruction that may reach guest memory, with `resume` its commit point: the one place each way's
/// form of a guest access is made (section 4). With no memory check it is the instruction itself.
fn access(way: Way, code: &mut HostCode, written: Written, resume: Option<Resume>) {
    if let Way::InProcess(check) = way {
        let decoded;
        let i = match written {
            Written::Bytes(bytes) => {
                // Decoded as the translator decodes the guest.
                decoded = crate::decode::decode(bytes).expect("a guest instruction").0;
                &decoded.iced
            }
            Written::Instruction(i) => i,
        };
        if window::reaches_memory(i) {
            return window::access(check, code, i, resume);
        }
    }
    let at = code.offset() as u64;
    let bytes = match written {
        Written::Bytes(bytes) => bytes.to_vec(),
        Written::Instruction(i) => bytes_of(i, at),
    };
    match resume {
        Some(resume) => code.commit(&bytes, resume),
        None => code.push(&bytes),
    }
}

/// Puts the guest's GS base on FS, so that an operand on the thread area's segment, made on FS, is the CPU's own
/// access with the guest's base (each part of an access wrapped under 0x67, an implicit rsi, rdi or rbx, a bit
/// offset); the guest's FS base is in u until [`leave_guest_segment`] (section 3.1).
fn enter_guest_segment(way: Way, code: &mut HostCode) {
    put(
        code,
        &Iced::with1(Code::Rdfsbase_r64, U).expect("the guest's FS base kept"),
    );
    code.relocate(FS_BASE_NUMBER, Held::Register(U_NUMBER));
    put(code, &load_area(way, T, GUEST_GS_BASE));
    put(
        code,
        &Iced::with1(Code::Wrfsbase_r64, T).expect("the guest's GS base on FS"),
    );
}

/// Puts the guest's FS base back on FS.
fn leave_guest_segment(code: &mut HostCode) {
    put(
        code,
        &Iced::with1(Code::Wrfsbase_r64, U).expect("the guest's FS base back"),
    );
    code.home(FS_BASE_NUMBER);
}

/// The guest's bytes with each GS override made FS, its other prefixes and their order kept: the CPU then picks
/// the segment as it would for the guest.
fn on_fs(bytes: &[u8]) -> Vec<u8> {
    use crate::encoding::{FS, GS, LEGACY_PREFIXES, is_rex};
    let mut out = bytes.to_vec();
    for b in out
        .iter_mut()
        .take_while(|b| LEGACY_PREFIXES.contains(b) || is_rex(**b))
    {
        if *b == GS {
            *b = FS;
        }
    }
    out
}

/// Writes the code that puts a rebuilt operand's address in t, and gives the instruction made to use it.
fn reach_operand(code: &mut HostCode, i: &Iced, address32: bool, reach: Reach, guest: u64) -> Iced {
    let mut access = *i;
    if reach != Reach::Plain && i.is_ip_rel_memory_operand() {
        place_address(code, T, guest);
        // Under 0x67 the access keeps its 32-bit address, so the CPU wraps what it adds to t (a bit offset, each
        // part of the access) as it does for the guest.
        access.set_memory_base(if address32 { T_LOW } else { T });
        access.set_memory_index(Register::None);
        access.set_memory_displacement64(0);
        access.set_memory_displ_size(0);
    }
    if reach == Reach::GuestSegment {
        access.set_segment_prefix(Register::FS);
    }
    access
}

fn has_memory_operand(i: &Iced) -> bool {
    (0..i.op_count()).any(|n| i.op_kind(n) == OpKind::Memory)
}

/// The guest address a RIP-relative operand names, as the meaning forms it.
fn rip_target(i: &Instruction, address: u64) -> u64 {
    // Decoded at address 0, iced's displacement is the operand's distance from the instruction.
    let target = address.wrapping_add(i.iced.memory_displacement64());
    if i.address32 {
        u64::from(target as u32)
    } else {
        target
    }
}

/// Writes the instruction as `reach` rebuilds it, with stand-ins for the withheld registers it names, the
/// instruction itself the commit point.
fn rewritten(
    way: Way,
    code: &mut HostCode,
    i: &Instruction,
    guest: &Iced,
    address: u64,
    reach: Reach,
) {
    let next = Resume::At(address.wrapping_add(i.iced.len() as u64));
    let in_process = matches!(way, Way::InProcess(_));
    let stack = matches!(guest.mnemonic(), Mnemonic::Push | Mnemonic::Pop)
        && guest.op0_kind() == OpKind::Register
        && names_withheld(guest);
    if stack && in_process {
        // A withheld register's push and pop are t's, the stack's side as written: t loaded from its slot before,
        // and the slot written from it after.
        code.slow(NativeReason::WithheldRegisterNamed);
        let named = general(guest.op0_register()).expect("a general register");
        let mut made = *guest;
        made.set_op0_register(sized(T_NUMBER, named.width));
        let pop = guest.mnemonic() == Mnemonic::Pop;
        if !pop || named.width < W64 {
            put(code, &load_area(way, T, withheld_slot(named.number)));
        }
        access(way, code, Written::Instruction(&made), Some(next));
        if pop {
            code.relocate(named.number, Held::Register(T_NUMBER));
            put(code, &store_area(way, withheld_slot(named.number), T));
            code.home(named.number);
        }
        return;
    }
    let stack_value = in_process && names_stack_value(guest);
    if reach == Reach::Plain
        && !stack_value
        && let Some(on) = on_slot(way, guest)
    {
        // Its operand is the register area, not guest memory.
        code.slow(NativeReason::WithheldRegisterNamed);
        let at = code.offset() as u64;
        code.commit(&bytes_of(&on, at), next);
        return;
    }
    let (ins, swaps) = stand_in(guest);
    if !swaps.is_empty() {
        code.slow(NativeReason::WithheldRegisterNamed);
    }
    stand_ins_in(way, code, guest, &swaps);
    let (ins, stack) = if stack_value {
        let stack = StackValue::of(way, &ins, reach, swaps.len());
        stack.enter(way, code);
        (stack.on(&ins), Some(stack))
    } else {
        (ins, None)
    };
    let window = reach == Reach::GuestSegment;
    if window {
        enter_guest_segment(way, code);
    }
    let target = rip_target(i, address);
    let ins = if reach == Reach::Rebase && ins.mnemonic() == Mnemonic::Lea {
        // `lea r, [rip+d]` is the guest address itself.
        place_address(code, T, target);
        let mut lea = ins;
        lea.set_memory_base(T);
        lea.set_memory_displacement64(0);
        lea.set_memory_displ_size(0);
        lea
    } else {
        reach_operand(code, &ins, i.address32, reach, target)
    };
    // An operand rebuilt on t of an instruction naming a high byte takes its address in one of the first eight.
    let high = reach != Reach::Plain && guest.is_ip_rel_memory_operand() && names_high_byte(&ins);
    let (ins, legacy) = if high {
        on_legacy(way, code, &ins, i.address32)
    } else {
        (ins, 0)
    };
    access(way, code, Written::Instruction(&ins), Some(next));
    if high {
        put(code, &load_area(way, full(legacy), STAND_IN_VALUES));
        code.home(legacy);
    }
    if let Some(stack) = &stack {
        stack.done(code);
    }
    stand_ins_out(way, code, guest, &swaps);
    if let Some(stack) = stack {
        stack.leave(way, code);
    }
    if window {
        leave_guest_segment(code);
    }
}

/// Whether guest instruction `i` names rsp as a value, which in process is B less than the host's rsp: as a register
/// operand, or as the base of a 64-bit `lea` (whose low 32 bits B leaves as they are).
fn names_stack_value(i: &Iced) -> bool {
    let rsp = |r: Register| general(r).is_some_and(|g| g.number == RSP);
    (0..i.op_count()).any(|n| i.op_kind(n) == OpKind::Register && rsp(i.op_register(n)))
        || i.code() == Code::Lea_r64_m && i.memory_base() == Register::RSP
}

/// In process, the register that stands in for the guest's rsp in an instruction naming it as a value: t where the
/// instruction leaves t free, else a guest register kept in a stand-in slot meanwhile (section 4).
struct StackValue {
    s: u8,
    /// The area word keeping s's own value, where s is not t.
    own: Option<usize>,
    reads: bool,
    writes: bool,
}

impl StackValue {
    fn of(way: Way, i: &Iced, reach: Reach, taken: usize) -> Self {
        let Way::InProcess(_) = way else {
            unreachable!("rsp is the guest's own outside the in-process way")
        };
        let mut info = InstructionInfoFactory::new();
        let info = info.info(i);
        let (mut reads, mut writes) = (false, false);
        for n in 0..i.op_count() {
            let named = i.op_kind(n) == OpKind::Register
                && general(i.op_register(n)).is_some_and(|g| g.number == RSP);
            if !named {
                continue;
            }
            let partial = general(i.op_register(n)).is_some_and(|g| g.width < crate::operand::W32);
            use iced_x86::OpAccess::*;
            match info.op_access(n) {
                Read | CondRead => reads = true,
                Write => {
                    writes = true;
                    reads |= partial;
                }
                // A write that may not happen (a `cmov`) keeps the value, or in 32 bits its low half, so it reads.
                _ => (reads, writes) = (true, true),
            }
        }
        reads |= i.code() == Code::Lea_r64_m && i.memory_base() == Register::RSP;
        let lea = i.mnemonic() == Mnemonic::Lea && !i.is_ip_rel_memory_operand();
        let t_free =
            reach == Reach::Plain && (!has_memory_operand(i) || lea) && !names_high_byte(i);
        if t_free {
            return Self {
                s: T_NUMBER,
                own: None,
                reads,
                writes,
            };
        }
        let used: Vec<u8> = info
            .used_registers()
            .iter()
            .filter_map(|u| general(u.register()).map(|g| g.number))
            .collect();
        // A register of the first four where the instruction names a high byte, which excludes a REX prefix.
        let s = stand_ins()
            .filter(|n| !names_high_byte(i) || *n <= crate::operand::RBX)
            .find(|n| !used.contains(n))
            .expect("an instruction leaves a register free");
        let own = if taken < crate::area::WITHHELD {
            STAND_IN_VALUES + taken * AREA_WORD
        } else {
            crate::area::KEPT_T
        };
        Self {
            s,
            own: Some(own),
            reads,
            writes,
        }
    }

    /// The instruction with rsp, where it names it as a value, made s.
    fn on(&self, i: &Iced) -> Iced {
        let mut out = *i;
        for n in 0..i.op_count() {
            if i.op_kind(n) != OpKind::Register {
                continue;
            }
            if let Some(g) = general(i.op_register(n)).filter(|g| g.number == RSP) {
                out.set_op_register(n, sized(self.s, g.width));
            }
        }
        if i.code() == Code::Lea_r64_m && i.memory_base() == Register::RSP {
            out.set_memory_base(full(self.s));
        }
        out
    }

    fn enter(&self, way: Way, code: &mut HostCode) {
        if let Some(own) = self.own {
            put(code, &store_area(way, own, full(self.s)));
            code.relocate(self.s, page(own));
        }
        if self.reads {
            window::guest_stack_into(way, code, full(self.s));
        }
    }

    /// After the instruction: the guest's rsp is s's where the instruction writes it.
    fn done(&self, code: &mut HostCode) {
        if self.writes {
            code.relocate(RSP, Held::Register(self.s));
        }
    }

    fn leave(&self, way: Way, code: &mut HostCode) {
        let Way::InProcess(check) = way else {
            unreachable!("rsp is the guest's own outside the in-process way")
        };
        if self.writes {
            window::guest_stack_from(check, code, full(self.s));
        }
        if let Some(own) = self.own {
            put(code, &load_area(way, full(self.s), own));
            code.home(self.s);
        }
    }
}

fn names_high_byte(i: &Iced) -> bool {
    (0..i.op_count()).any(|n| {
        i.op_kind(n) == OpKind::Register && general(i.op_register(n)).is_some_and(|g| g.high)
    })
}

/// An instruction naming ah, ch, dh or bh cannot take a REX prefix, so its rebuilt operand cannot name t or
/// u: the address goes to a register of the first eight but rbp (whose accesses take the stack segment), kept in
/// the area meanwhile. It names no withheld register, which would need REX itself, so the first stand-in slot is
/// free. Under 0x67 the access stays 32-bit, as on t.
fn on_legacy(way: Way, code: &mut HostCode, access: &Iced, address32: bool) -> (Iced, u8) {
    let mut info = InstructionInfoFactory::new();
    let used: Vec<u8> = info
        .info(access)
        .used_registers()
        .iter()
        .filter_map(|u| general(u.register()).map(|g| g.number))
        .collect();
    let legacy = stand_ins()
        .find(|n| *n < R8 && *n != RBP && !used.contains(n))
        .expect("a byte instruction leaves a legacy register free");
    put(code, &store_area(way, STAND_IN_VALUES, full(legacy)));
    code.relocate(legacy, page(STAND_IN_VALUES));
    let operand = MemoryOperand::new(
        access.memory_base(),
        access.memory_index(),
        access.memory_index_scale(),
        access.memory_displacement64() as i64,
        access.memory_displ_size(),
        false,
        Register::None,
    );
    put(
        code,
        &Iced::with2(Code::Lea_r64_m, full(legacy), operand).expect("the address"),
    );
    let mut on = *access;
    on.set_memory_base(if address32 {
        sized(legacy, crate::operand::W32)
    } else {
        full(legacy)
    });
    on.set_memory_index(Register::None);
    on.set_memory_index_scale(1);
    on.set_memory_displacement64(0);
    on.set_memory_displ_size(0);
    (on, legacy)
}

/// Writes the host code of one guest instruction, `bytes` at guest `address`, as `route` says, in a loop's copy
/// with the withheld registers it names in their `homes`.
pub fn carry(
    way: Way,
    route: Route,
    i: &Instruction,
    bytes: &[u8],
    address: u64,
    homes: &Homes,
    code: &mut HostCode,
) {
    if homes.pairs.is_empty() {
        return carry_plain(way, route, i, bytes, address, homes, code);
    }
    homes.throughout(code);
    let m = i.iced.mnemonic();
    let repeated =
        crate::meaning::is_string(i) && (i.iced.has_rep_prefix() || i.iced.has_repne_prefix());
    // A rebuilt operand of an instruction naming a high byte takes its address in one of the first eight.
    let stays = matches!(
        route,
        Route::Copy | Route::Rebase | Route::Nothing | Route::Transfer
    ) && !repeated
        && m != Mnemonic::Fldcw
        && !(route == Route::Rebase && names_high_byte(&i.iced))
        && !(route == Route::Transfer && far_direct(i, address));
    if !stays {
        // The inside reads and writes the guest's registers where they are between loops.
        homes.leave(way, code);
        carry_plain(way, route, i, bytes, address, &Homes::default(), code);
        homes.enter(way, code);
        return;
    }
    let renamed = homes.renamed(i);
    let encoded;
    let bytes = if renamed.iced == i.iced {
        bytes
    } else {
        encoded = bytes_of(&renamed.iced, address);
        &encoded[..]
    };
    carry_plain(way, route, &renamed, bytes, address, homes, code);
    if route == Route::Transfer && i.iced.flow_control() == iced_x86::FlowControl::ConditionalBranch
    {
        // The copy's way out where its branch back is not taken.
        homes.leave(way, code);
    }
}

fn carry_plain(
    way: Way,
    route: Route,
    i: &Instruction,
    bytes: &[u8],
    address: u64,
    homes: &Homes,
    code: &mut HostCode,
) {
    let next = address.wrapping_add(bytes.len() as u64);
    let repeated =
        crate::meaning::is_string(i) && (i.iced.has_rep_prefix() || i.iced.has_repne_prefix());
    let counted_string = repeated && code.lead_after_commit() < code.lead();
    // A stepping thread's string traps after each pass, which the hardware's run of it does not.
    if counted_string || (repeated && code.stepping) {
        if counted_string {
            // The count takes the meaning's passes, which the hardware's run of it does not show.
            code.slow(NativeReason::RepeatedStringCounted);
        }
        call(way, code, InsideCall::Answer);
        code.after(Resume::At(next));
        return;
    }
    match route {
        Route::Copy
            if !names_withheld(&i.iced)
                && !(matches!(way, Way::InProcess(_)) && names_stack_value(&i.iced)) =>
        {
            access(way, code, Written::Bytes(bytes), Some(Resume::At(next)));
        }
        Route::Copy => rewritten(way, code, i, &i.iced, address, Reach::Plain),
        Route::Rebase => rewritten(way, code, i, &i.iced, address, Reach::Rebase),
        Route::Area => area(way, code, i, bytes, address),
        Route::Nothing => code.after(Resume::At(next)),
        Route::Kernel => {
            call(way, code, InsideCall::Answer);
            code.after(Resume::At(next));
        }
        Route::Trap => call(way, code, InsideCall::Trap),
        Route::FloatStatus => {
            call(way, code, InsideCall::FloatBefore);
            if has_memory_operand(&i.iced) && i.iced.memory_segment() == way.area_segment() {
                area(way, code, i, bytes, address);
            } else {
                let reach = if i.iced.is_ip_rel_memory_operand() {
                    Reach::Rebase
                } else {
                    Reach::Plain
                };
                rewritten(way, code, i, &i.iced, address, reach);
            }
            code.fixup(Fixup::FloatStatus);
            call(way, code, InsideCall::FloatAfter);
            code.fixed();
        }
        Route::Transfer => transfer(way, code, i, address, next, homes),
        Route::Split | Route::Collision | Route::Aligned | Route::Constant | Route::Exclusive => {
            unreachable!("this guest gives no {route:?} route")
        }
    }
    let m = i.iced.mnemonic();
    let copied = matches!(route, Route::Copy | Route::Rebase | Route::Area);
    if copied && crate::fpu::written(m) && !crate::fpu::control(m) {
        x87_pointer(way, code, address);
    }
    if copied && m == Mnemonic::Fldcw {
        x87_control_check(way, code);
    }
}

/// After a copied `fldcw`: the control word the hardware loaded is read back from the area, never the guest's
/// operand, and a call into the inside made when it unmasks an exception (section 3.1). No flag is written: rcx
/// is borrowed into u for `jrcxz`.
fn x87_control_check(way: Way, code: &mut HostCode) {
    let control = MemoryOperand {
        displ_size: AREA_WORD as u32,
        segment_prefix: way.area_segment(),
        ..MemoryOperand::with_base_displ(Register::None, X87_CONTROL as i64)
    };
    let rcx = full(crate::operand::RCX);
    put(
        code,
        &Iced::with1(Code::Fnstcw_m2byte, control).expect("a store of the control word"),
    );
    put(
        code,
        &Iced::with2(Code::Movzx_r32_rm16, T_LOW, control).expect("its load"),
    );
    put(code, &Iced::with1(Code::Not_rm32, T_LOW).expect("a not"));
    // The control word's six masks sit where the status word's six exception flags do (Intel SDM Vol. 1 8.1.5).
    let masks = u32::from(crate::x87::EXCEPTIONS);
    put(
        code,
        &Iced::with2(Code::Mov_r32_imm32, U_LOW, masks).expect("the masks"),
    );
    put(
        code,
        &Iced::with3(Code::VEX_Pext_r32_r32_rm32, T_LOW, T_LOW, U_LOW).expect("the unmasked ones"),
    );
    put(
        code,
        &Iced::with2(Code::Mov_r64_rm64, U, rcx).expect("rcx kept"),
    );
    code.relocate(crate::operand::RCX, Held::Register(U_NUMBER));
    put(
        code,
        &Iced::with2(Code::Mov_r32_rm32, Register::ECX, T_LOW).expect("the test's value"),
    );
    let at = code.offset();
    let skip = |to: u64| Iced::with_branch(Code::Jrcxz_rel8_64, to).expect("a jrcxz");
    let length = bytes_of(&skip(0), 0).len();
    let relocated = code.snapshot();
    let back = Iced::with2(Code::Mov_r64_rm64, rcx, U).expect("rcx back");
    let back_length = bytes_of(&back, 0).len();
    // The jrcxz is patched to the masked side once the unmasked side is written.
    code.push(&bytes_of(&skip((at + length) as u64), at as u64));
    put(code, &back);
    code.home(crate::operand::RCX);
    call(way, code, InsideCall::X87Control);
    let over = code.offset();
    let short = |to: u64| Iced::with_branch(Code::Jmp_rel8_64, to).expect("a jump");
    let short_length = bytes_of(&short(0), 0).len();
    let after = code.snapshot();
    let masked = over + short_length;
    code.push(&bytes_of(
        &short((masked + back_length) as u64),
        over as u64,
    ));
    code.restore(relocated);
    code.patch(at, &bytes_of(&skip(masked as u64), at as u64));
    put(code, &back);
    code.restore(after);
}

/// Writes the replay check before an instruction (section 3.7): the call of kind Stop where the thread's stop word
/// equals the count, no flag written, rcx borrowed into t.
pub fn stop_check(way: Way, code: &mut HostCode) {
    let rcx = full(crate::operand::RCX);
    put(
        code,
        &Iced::with2(Code::Mov_r64_rm64, T, rcx).expect("rcx kept"),
    );
    code.relocate(crate::operand::RCX, Held::Register(T_NUMBER));
    put(code, &load_area(way, rcx, STOP_WORD));
    put(code, &Iced::with1(Code::Not_rm64, rcx).expect("a not"));
    // `not` gives minus the stop word less one, so this is the count less the stop word.
    let difference = MemoryOperand::new(rcx, COUNT, 1, 1, 1, false, Register::None);
    put(
        code,
        &Iced::with2(Code::Lea_r64_m, rcx, difference).expect("the difference"),
    );
    let back = Iced::with2(Code::Mov_r64_rm64, rcx, T).expect("rcx back");
    let back_length = bytes_of(&back, 0).len();
    let short = |to: u64| Iced::with_branch(Code::Jmp_rel8_64, to).expect("a jump");
    let short_length = bytes_of(&short(0), 0).len();
    let equal = |to: u64| Iced::with_branch(Code::Jrcxz_rel8_64, to).expect("a jrcxz");
    let at = code.offset();
    let stop = at + bytes_of(&equal(0), 0).len() + back_length + short_length;
    code.push(&bytes_of(&equal(stop as u64), at as u64));
    let relocated = code.snapshot();
    put(code, &back);
    code.home(crate::operand::RCX);
    // Over the stop's call, patched once its length is known.
    let over = code.offset();
    code.push(&bytes_of(&short((over + short_length) as u64), over as u64));
    let going_on = code.snapshot();
    code.restore(relocated);
    put(code, &back);
    code.home(crate::operand::RCX);
    call(way, code, InsideCall::Stop);
    let end = code.offset();
    code.patch(over, &bytes_of(&short(end as u64), over as u64));
    code.restore(going_on);
}

/// After a copied x87 instruction that is not a control one: its guest address stored as the guest's FIP, the
/// hardware's holding a host address (section 3.1).
fn x87_pointer(way: Way, code: &mut HostCode, address: u64) {
    code.fixup(Fixup::X87Pointer(address));
    place_address(code, T, address);
    put(code, &store_area(way, X87_IP, T));
    code.fixed();
}

/// An operand or base instruction of the thread area's segment, `bytes` at guest `address`.
fn area(way: Way, code: &mut HostCode, i: &Instruction, bytes: &[u8], address: u64) {
    let ins = &i.iced;
    if matches!(way, Way::InProcess(_)) {
        return area_in_process(way, code, i, address);
    }
    if ins.mnemonic() == Mnemonic::Rdgsbase {
        // `rdgsbase r32` zero-extends, as a 32-bit load does.
        let destination = ins.op_register(0);
        let wide = general(destination).is_some_and(|g| g.width == W64);
        let read = if wide {
            load_area(way, destination, GUEST_GS_BASE)
        } else {
            Iced::with2(
                Code::Mov_r32_rm32,
                destination,
                on_area(way, GUEST_GS_BASE, 0),
            )
            .expect("a load from the area")
        };
        rewritten(way, code, i, &read, address, Reach::Plain);
    } else if names_withheld(ins) || ins.is_ip_rel_memory_operand() {
        rewritten(way, code, i, ins, address, Reach::GuestSegment);
    } else {
        let next = Resume::At(address.wrapping_add(bytes.len() as u64));
        enter_guest_segment(way, code);
        access(way, code, Written::Bytes(&on_fs(bytes)), Some(next));
        leave_guest_segment(code);
    }
}

/// The in-process way's `rdfsbase`, `rdgsbase` and `fs:` and `gs:` operands: the bases are words of the register
/// area, and an operand's access adds its segment's word in the window's form (section 4).
fn area_in_process(way: Way, code: &mut HostCode, i: &Instruction, address: u64) {
    let ins = &i.iced;
    let next = Resume::At(address.wrapping_add(ins.len() as u64));
    let word = match ins.mnemonic() {
        Mnemonic::Rdgsbase => GUEST_GS_BASE,
        Mnemonic::Rdfsbase => GUEST_FS_BASE,
        _ => {
            let reach = if ins.is_ip_rel_memory_operand() {
                Reach::Rebase
            } else {
                Reach::Plain
            };
            return rewritten(way, code, i, ins, address, reach);
        }
    };
    let destination = general(ins.op_register(0)).expect("a general register");
    // The 32-bit form zero-extends, as a 32-bit load does.
    let read = |into: u8| {
        if destination.width == W64 {
            load_area(way, full(into), word)
        } else {
            Iced::with2(
                Code::Mov_r32_rm32,
                sized(into, crate::operand::W32),
                on_area(way, word, 0),
            )
            .expect("a load from the area")
        }
    };
    if let (Way::InProcess(check), RSP) = (way, destination.number) {
        // rsp is B more than the guest's: the base read into t, then rsp made from it.
        let at = code.offset() as u64;
        code.commit(&bytes_of(&read(T_NUMBER), at), next);
        return window::guest_stack_from(check, code, T);
    }
    let written = if WITHHELD.contains(&destination.number) {
        put(code, &read(U_NUMBER));
        store_area(way, withheld_slot(destination.number), U)
    } else {
        read(destination.number)
    };
    let at = code.offset() as u64;
    code.commit(&bytes_of(&written, at), next);
}

/// Writes a call into the inside: an interrupt gate of the floor's in the machine, a jump through the area's
/// entry with the return point in t elsewhere (section 3.6).
pub fn call(way: Way, code: &mut HostCode, kind: InsideCall) {
    let kind_code = u8::try_from(kind.code()).expect("a call's code fits a byte");
    match way {
        Way::Machine => put(
            code,
            &Iced::with1(Code::Int_imm8, u32::from(FIRST_CALL_VECTOR + kind_code))
                .expect("an interrupt"),
        ),
        Way::Process | Way::InProcess(_) => {
            let jump = Iced::with1(
                Code::Jmp_rm64,
                on_area(way, ENTRIES + usize::from(kind_code) * AREA_WORD, 0),
            )
            .expect("a jump through the area");
            let at = code.offset() as u64;
            // `lea t, [rip]`: rip-relative to the jump after it, the call's return point.
            let back = |length: u64| {
                Iced::with2(
                    Code::Lea_r64_m,
                    T,
                    MemoryOperand::with_base_displ(Register::RIP, (at + length) as i64),
                )
                .expect("the return point")
            };
            let length = bytes_of(&back(0), at).len() as u64;
            let jump_at = at + length;
            let return_point = jump_at + bytes_of(&jump, jump_at).len() as u64;
            put(code, &back(return_point - at));
            put(code, &jump);
        }
    }
}

/// Writes the exit to direct guest `target`: a `jmp rel32`, first pointed at the call into the inside after it,
/// after an add that brings the count back to the guest's where it is ahead; the call leaves a loop's `homes`. In
/// a stepping piece it is the call of kind Step alone.
pub fn exit(way: Way, code: &mut HostCode, target: u64, commit: Option<Resume>, homes: &Homes) {
    if code.stepping || target >= USER_END {
        if let Some(resume) = commit {
            code.after(resume);
        }
        homes.leave(way, code);
        let kind = if code.stepping {
            assert_eq!(
                code.lead(),
                0,
                "a stepping piece's count is the guest's at its end"
            );
            InsideCall::Step
        } else {
            InsideCall::Exit
        };
        call(way, code, kind);
        return;
    }
    let lead = code.lead_after_commit();
    if lead > 0 {
        let settle = bytes_of(&count_add(-i64::from(lead)), 0);
        assert!(
            settle.len() >= jump_length(),
            "a link fits over the settling add"
        );
        code.settling_exit(&settle, target, commit);
        let at = code.offset();
        code.push(&jump(at, at + jump_length()));
    } else {
        let at = code.offset();
        code.exit(&jump(at, at + jump_length()), target, commit);
    }
    homes.leave(way, code);
    call(way, code, InsideCall::Exit);
}

/// Points direct exit `exit` of `code` at host offset `to`.
pub fn link(code: &mut HostCode, exit: &DirectExit, to: usize) -> bool {
    let distance = to as i64 - i64::from(exit.start);
    let Some(patch) = retarget(&code.bytes, exit, distance) else {
        return false;
    };
    code.patch(
        usize::try_from(patch.at).expect("a host offset fits usize"),
        patch.written(),
    );
    true
}

/// The change that points direct exit `exit` of `code` at the host code `distance` bytes from the exit's first
/// byte: its branch's new rel32, or where it has none a `jmp rel32` over its first instruction (a settling add
/// is longer: the rest becomes `nop`s); none where a rel32 cannot reach.
pub fn retarget(code: &[u8], exit: &DirectExit, distance: i64) -> Option<Patch> {
    let at = usize::try_from(exit.start).expect("a host offset fits usize");
    let mut bytes = [0; PATCH_BYTES];
    let (at, length) = match exit.retarget {
        None => {
            let over = iced_x86::Decoder::new(crate::MODE_BITS, &code[at..], DecoderOptions::NONE)
                .decode()
                .len();
            let rel = i32::try_from(distance - jump_length() as i64).ok()?;
            let mut written = jump(0, 0);
            written[jump_length() - REL32..].copy_from_slice(&rel.to_le_bytes());
            let nop = bytes_of(&Iced::with(Code::Nopd), 0);
            while written.len() < over {
                written.extend_from_slice(&nop);
            }
            bytes[..written.len()].copy_from_slice(&written);
            (at, written.len())
        }
        Some(field) => {
            let field = usize::try_from(field).expect("a byte offset");
            let end = i64::try_from(field + REL32).expect("small");
            let rel = i32::try_from(distance - end).ok()?;
            bytes[..REL32].copy_from_slice(&rel.to_le_bytes());
            (at + field, REL32)
        }
    };
    Some(Patch {
        at: u32::try_from(at).expect("host code fits in u32 offsets"),
        bytes,
        length: u8::try_from(length).expect("a patch is short"),
    })
}

/// Writes the count's add of `events` events.
pub fn count(code: &mut HostCode, events: u32) {
    let at = code.offset() as u64;
    code.count(&bytes_of(&count_add(events.into()), at), events);
}

/// Writes the word of `code` at `place` for the place's address.
pub fn write_place(code: &mut [u8], place: &AddressPlace) {
    let at = usize::try_from(place.start).expect("a host offset fits usize") + IMM64_AT;
    code[at..at + AREA_WORD].copy_from_slice(&place.address.to_le_bytes());
}

/// Whether `i` at `address` is a direct transfer to a target at or above 2^47, which the inside answers
/// (`Question::NearBranch`).
fn far_direct(i: &Instruction, address: u64) -> bool {
    i.iced.op0_kind() == OpKind::NearBranch64
        && address.wrapping_add(i.iced.near_branch64()) >= USER_END
}

/// A branch, call, return or jump.
fn transfer(
    way: Way,
    code: &mut HostCode,
    i: &Instruction,
    address: u64,
    next: u64,
    homes: &Homes,
) {
    use Mnemonic::*;
    let ins = &i.iced;
    let direct = |offset: u64| address.wrapping_add(offset);
    match ins.mnemonic() {
        _ if far_direct(i, address) => {
            // The inside's answer: the transfer, a fault, or for a branch not taken the next instruction.
            call(way, code, InsideCall::Answer);
            code.after(Resume::At(next));
        }
        Jmp | Call if ins.op_kind(0) == OpKind::NearBranch64 => {
            let target = direct(ins.near_branch64());
            if ins.mnemonic() == Call {
                place_address(code, T, next);
                let push = Iced::with1(Code::Push_r64, T).expect("a push");
                access(
                    way,
                    code,
                    Written::Instruction(&push),
                    Some(Resume::At(target)),
                );
                exit(way, code, target, None, homes);
            } else {
                exit(way, code, target, Some(Resume::At(target)), homes);
            }
        }
        Jmp | Call => {
            into_t(way, code, i, address);
            let call = ins.mnemonic() == Call;
            first_level(way, code);
            let through = Resume::Through(Held::Register(T_NUMBER));
            if call {
                let own = STAND_IN_VALUES;
                put(code, &store_area(way, own, full(RAX)));
                code.relocate(RAX, page(own));
                place_address(code, full(RAX), next);
                let push = Iced::with1(Code::Push_r64, full(RAX)).expect("a push");
                access(way, code, Written::Instruction(&push), Some(through));
                put(code, &load_area(way, full(RAX), own));
                code.home(RAX);
            } else {
                code.after(through);
            }
            onward(way, code);
        }
        Ret => {
            let top = MemoryOperand::with_base(full(RSP));
            let read = Iced::with2(Code::Mov_r64_rm64, T, top).expect("the return address");
            access(way, code, Written::Instruction(&read), None);
            first_level(way, code);
            let popped = i64::from(SLOT) + i64::from(ins.immediate16());
            let lea = Iced::with2(
                Code::Lea_r64_m,
                full(RSP),
                MemoryOperand::with_base_displ(full(RSP), popped),
            )
            .expect("the pop");
            let at = code.offset() as u64;
            code.commit(
                &bytes_of(&lea, at),
                Resume::Through(Held::Register(T_NUMBER)),
            );
            onward(way, code);
        }
        _ if ins.is_jcc_short_or_near() => {
            let target = direct(ins.near_branch64());
            let mut jcc = *ins;
            jcc.set_code(ins.code().as_near_branch());
            let at = code.offset();
            jcc.set_near_branch64(at as u64);
            let bytes = bytes_of(&jcc, at as u64);
            let length = bytes.len();
            let linkable = code.lead_after_commit() == 0 && !code.stepping;
            code.commit(&bytes, Resume::At(next));
            if linkable {
                // iced numbers the conditions from 1 in the order of a `jcc`'s low four bits.
                let condition = ins.condition_code() as u8 - ConditionCode::o as u8;
                let field = u32::try_from(length - REL32).expect("small");
                code.branch_exit(at, target, field, Some(condition));
            }
            let after = code.snapshot();
            let homes = homes.clone();
            code.defer(move |code: &mut HostCode| {
                let to = code.offset();
                let field = at + length - REL32;
                let rel = i32::try_from(to - (at + length)).expect("a piece is small");
                code.patch(field, &rel.to_le_bytes());
                code.restore(after);
                code.after(Resume::At(target));
                if linkable {
                    homes.leave(way, code);
                    call(way, code, InsideCall::Exit);
                } else {
                    exit(way, code, target, None, &homes);
                }
            });
        }
        Loop | Loope | Loopne | Jrcxz | Jecxz => {
            let target = direct(ins.near_branch64());
            let at = code.offset();
            // The rel8 branch goes over the short jump after it to the taken side's exit.
            let short = Iced::with_branch(Code::Jmp_rel8_64, 0).expect("a jump");
            let short_length = bytes_of(&short, 0).len();
            let mut branch = *ins;
            branch.set_near_branch64(0);
            let length = bytes_of(&branch, 0).len();
            branch.set_near_branch64((at + length + short_length) as u64);
            code.commit(&bytes_of(&branch, at as u64), Resume::At(next));
            let after = code.snapshot();
            let over = code.offset();
            let short =
                Iced::with_branch(Code::Jmp_rel8_64, (over + short_length) as u64).expect("a jump");
            code.push(&bytes_of(&short, over as u64));
            code.after(Resume::At(target));
            exit(way, code, target, None, homes);
            let to = code.offset();
            let rel = i8::try_from(to - (over + short_length)).expect("the exit is short");
            code.patch(over + short_length - 1, &rel.to_le_bytes());
            code.restore(after);
        }
        other => unreachable!("{other:?} is no transfer"),
    }
}

/// The memory operand of a guest instruction as iced builds one.
fn guest_operand(i: &Iced, address32: bool) -> MemoryOperand {
    let displacement = if address32 && !i.is_ip_rel_memory_operand() {
        i64::from(i.memory_displacement32() as i32)
    } else {
        i.memory_displacement64() as i64
    };
    MemoryOperand::new(
        i.memory_base(),
        i.memory_index(),
        i.memory_index_scale(),
        displacement,
        i.memory_displ_size(),
        false,
        i.segment_prefix(),
    )
}

/// Writes the load into t of the operand of `i`, a jump or call through a register or memory.
fn into_t(way: Way, code: &mut HostCode, i: &Instruction, address: u64) {
    let (operand, swaps) = stand_in(&i.iced);
    let in_process = matches!(way, Way::InProcess(_));
    let reach = if operand.op0_kind() == OpKind::Register {
        Reach::Plain
    } else if !in_process && i.iced.memory_segment() == way.area_segment() {
        Reach::GuestSegment
    } else if i.iced.is_ip_rel_memory_operand() {
        Reach::Rebase
    } else {
        Reach::Plain
    };
    let read = if operand.op0_kind() == OpKind::Register {
        Iced::with2(Code::Mov_r64_rm64, T, operand.op0_register())
    } else {
        Iced::with2(Code::Mov_r64_rm64, T, guest_operand(&operand, i.address32))
    }
    .expect("a load into t");
    if !swaps.is_empty() {
        code.slow(NativeReason::WithheldRegisterNamed);
    }
    stand_ins_in(way, code, &i.iced, &swaps);
    let target = rip_target(i, address);
    let window = reach == Reach::GuestSegment;
    if window {
        enter_guest_segment(way, code);
    }
    let read = reach_operand(code, &read, i.address32, reach, target);
    let stack = operand.op0_kind() == OpKind::Register && operand.op0_register() == Register::RSP;
    if in_process && stack {
        window::guest_stack_into(way, code, T);
    } else {
        access(way, code, Written::Instruction(&read), None);
    }
    if window {
        leave_guest_segment(code);
    }
    stand_ins_out(way, code, &i.iced, &swaps);
}

/// The lookup's first level (section 3.3): u gets the second-level table of t's region.
fn first_level(way: Way, code: &mut HostCode) {
    let rotate = Iced::with3(Code::VEX_Rorx_r64_rm64_imm8, U, T, u64::BITS / 2).expect("a rotate");
    put(code, &rotate);
    put(
        code,
        &Iced::with2(Code::Mov_r32_rm32, U_LOW, U_LOW).expect("a move"),
    );
    let level = MemoryOperand::new(
        Register::None,
        U,
        AREA_WORD as u32,
        i64::try_from(FIRST_LEVEL).expect("a small offset"),
        AREA_WORD as u32,
        false,
        way.area_segment(),
    );
    put(
        code,
        &Iced::with2(Code::Mov_r64_rm64, U, level).expect("the first level"),
    );
}

/// Where a jump, call or return through t goes on: the lookup's second level, or in a stepping piece the call of
/// kind Step with the target moved to u, since the process way's call writes t.
fn onward(way: Way, code: &mut HostCode) {
    if code.stepping {
        put(
            code,
            &Iced::with2(Code::Mov_r64_rm64, U, T).expect("the target kept"),
        );
        code.after(Resume::Through(Held::Register(U_NUMBER)));
        call(way, code, InsideCall::Step);
    } else {
        second_level(code);
    }
}

/// The lookup's second level: the jump to the host entry of the piece at t.
fn second_level(code: &mut HostCode) {
    let entry = MemoryOperand::new(U, T, AREA_WORD as u32, 0, 0, false, Register::None);
    put(
        code,
        &Iced::with1(Code::Jmp_rm64, entry).expect("the second level"),
    );
}
