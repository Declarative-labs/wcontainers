//! The carrying code of the native output: host instructions written around a guest instruction, from iced's
//! encoder (docs/x86-native-output.md sections 3 and 4), for the machine and a host process.

use iced_x86::{
    Code, ConditionCode, DecoderOptions, Encoder, Instruction as Iced, InstructionInfoFactory,
    MemoryOperand, Mnemonic, OpKind, Register,
};
use wcontainers_guest_cpu::position::{AddressPlace, DirectExit, Fixup, Held, Holds, Resume};
use wcontainers_guest_cpu::{HostCode, InsideCall, NativeReason, PATCH_BYTES, Patch, Route};

use crate::USER_END;
use crate::area::{
    ENTRIES, FIRST_CALL_VECTOR, FIRST_LEVEL, GUEST_GS_BASE, STAND_IN_VALUES, WITHHELD_VALUES,
    X87_CONTROL, X87_IP,
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
const T_LOW: Register = Register::R11D;
const U_LOW: Register = Register::R15D;

/// Bytes of a word of the thread area, as its fields are laid out.
const WORD: usize = wcontainers_abi::WORD as usize;
/// The registers a stand-in may be, in the order they are taken: every general register but rsp and the
/// withheld ones.
fn stand_ins() -> impl Iterator<Item = u8> {
    (crate::operand::RAX..=crate::operand::R15).filter(|n| *n != RSP && !WITHHELD.contains(n))
}

/// Where `mov r64, imm64`'s immediate starts: after its REX prefix and opcode.
const IMM64_AT: usize = 2;
/// The bytes of a rel32 displacement.
const REL32: usize = size_of::<i32>();

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
        .expect("the carrying code's instructions encode");
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
        displ_size: WORD as u32,
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
    WITHHELD_VALUES + k * WORD
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
        &Iced::with2(Code::Mov_r64_imm64, r, address).expect("a 64-bit constant"),
    );
}

/// The `lea` that adds `by` to the count, its displacement always 32 bits wide so that a link can write a
/// `jmp rel32` over it.
fn count_add(by: i64) -> Iced {
    Iced::with2(
        Code::Lea_r64_m,
        COUNT,
        MemoryOperand::with_base_displ_size(COUNT, by, WORD as u32),
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

/// The moves that load each stand-in with its withheld register's guest value, keeping its own in the area.
fn stand_ins_in(way: Way, code: &mut HostCode, swaps: &[(u8, u8)]) {
    for (k, &(w, s)) in swaps.iter().enumerate() {
        let own = STAND_IN_VALUES + k * WORD;
        put(code, &store_area(way, own, full(s)));
        code.relocate(s, page(own));
        put(code, &load_area(way, full(s), withheld_slot(w)));
    }
}

/// After the instruction: each withheld register's value back to the area and each stand-in's own value back.
fn stand_ins_out(way: Way, code: &mut HostCode, swaps: &[(u8, u8)]) {
    for &(w, s) in swaps {
        code.relocate(w, Held::Register(s));
    }
    for (k, &(w, s)) in swaps.iter().enumerate() {
        put(code, &store_area(way, withheld_slot(w), full(s)));
        code.home(w);
        put(code, &load_area(way, full(s), STAND_IN_VALUES + k * WORD));
        code.home(s);
    }
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
    /// On the thread area's segment, rebuilt on the guest's base in t.
    Area,
}

/// Writes the code that puts the operand's address in t (and u), and gives the instruction made to use it.
/// The ModRM form of a `mov` to or from an absolute offset (`moffs`), which can then take a base register.
fn from_offset(code: Code) -> Option<Code> {
    Some(match code {
        Code::Mov_AL_moffs8 => Code::Mov_r8_rm8,
        Code::Mov_AX_moffs16 => Code::Mov_r16_rm16,
        Code::Mov_EAX_moffs32 => Code::Mov_r32_rm32,
        Code::Mov_RAX_moffs64 => Code::Mov_r64_rm64,
        Code::Mov_moffs8_AL => Code::Mov_rm8_r8,
        Code::Mov_moffs16_AX => Code::Mov_rm16_r16,
        Code::Mov_moffs32_EAX => Code::Mov_rm32_r32,
        Code::Mov_moffs64_RAX => Code::Mov_rm64_r64,
        _ => return None,
    })
}

fn reach_operand(
    way: Way,
    code: &mut HostCode,
    i: &Iced,
    address32: bool,
    reach: Reach,
    guest: u64,
) -> Iced {
    let mut access = *i;
    if let Some(form) = from_offset(i.code()) {
        access.set_code(form);
    }
    match reach {
        Reach::Plain => {}
        Reach::Rebase => {
            place_address(code, T, guest);
            if address32 {
                put(
                    code,
                    &Iced::with2(Code::Mov_r32_rm32, T_LOW, T_LOW).expect("a move"),
                );
            }
            access.set_memory_base(T);
            access.set_memory_index(Register::None);
            access.set_memory_displacement64(0);
            access.set_memory_displ_size(0);
        }
        Reach::Area => {
            put(code, &load_area(way, T, GUEST_GS_BASE));
            access.set_segment_prefix(Register::None);
            let (base, index) = (i.memory_base(), i.memory_index());
            // A `pop` whose operand is based on rsp forms its address after the pop (Intel SDM Vol. 2B, POP).
            let popped =
                if i.mnemonic() == Mnemonic::Pop && matches!(base, Register::RSP | Register::ESP) {
                    i64::from(i.stack_pointer_increment())
                } else {
                    0
                };
            if i.is_ip_rel_memory_operand() {
                place_address(code, U, guest);
                if address32 {
                    put(
                        code,
                        &Iced::with2(Code::Mov_r32_rm32, U_LOW, U_LOW).expect("a move"),
                    );
                }
                set_t_plus_u(&mut access);
            } else if address32 {
                let displacement = i64::from(i.memory_displacement32() as i32) + popped;
                let part = MemoryOperand::new(
                    base,
                    index,
                    i.memory_index_scale(),
                    displacement,
                    i.memory_displ_size().max(displacement_size(displacement)),
                    false,
                    Register::None,
                );
                put(
                    code,
                    &Iced::with2(Code::Lea_r32_m, U_LOW, part).expect("the operand's 32-bit part"),
                );
                set_t_plus_u(&mut access);
            } else if from_offset(i.code()).is_some() && !address32 {
                // A 64-bit absolute offset: no displacement field holds it.
                put(
                    code,
                    &Iced::with2(Code::Mov_r64_imm64, U, i.memory_displacement64())
                        .expect("a 64-bit constant"),
                );
                set_t_plus_u(&mut access);
            } else if base == Register::None {
                access.set_memory_base(T);
            } else if index == Register::None && !stack_based(base) {
                access.set_memory_index(T);
                access.set_memory_index_scale(1);
            } else {
                // t is the base, so the access stays on the default data segment: one based on rsp or rbp
                // would take the stack segment's fault (#SS, not #GP) at a non-canonical address.
                let add = MemoryOperand::with_base_index_scale_displ_size(
                    base,
                    T,
                    1,
                    popped,
                    displacement_size(popped),
                );
                put(
                    code,
                    &Iced::with2(Code::Lea_r64_m, T, add).expect("t plus the operand's base"),
                );
                access.set_memory_base(T);
            }
        }
    }
    access
}

/// Based on rsp or rbp, whose accesses take the stack segment.
fn stack_based(base: Register) -> bool {
    matches!(base, Register::RSP | Register::RBP)
}

/// The bytes of a displacement field holding `displacement`: none for zero, else as wide as it needs.
fn displacement_size(displacement: i64) -> u32 {
    if displacement == 0 {
        0
    } else if i8::try_from(displacement).is_ok() {
        size_of::<i8>() as u32
    } else {
        size_of::<i32>() as u32
    }
}

fn set_t_plus_u(access: &mut Iced) {
    access.set_memory_base(T);
    access.set_memory_index(U);
    access.set_memory_index_scale(1);
    access.set_memory_displacement64(0);
    access.set_memory_displ_size(0);
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
    if reach == Reach::Plain
        && let Some(on) = on_slot(way, guest)
    {
        code.slow(NativeReason::WithheldRegisterNamed);
        let at = code.offset() as u64;
        code.commit(&bytes_of(&on, at), next);
        return;
    }
    let (ins, swaps) = stand_in(guest);
    if !swaps.is_empty() {
        code.slow(NativeReason::WithheldRegisterNamed);
    }
    stand_ins_in(way, code, &swaps);
    let target = rip_target(i, address);
    let ins = if reach == Reach::Rebase && ins.mnemonic() == Mnemonic::Lea {
        // `lea r, [rip+d]` is the guest address itself.
        place_address(code, T, target);
        let mut lea = ins;
        lea.set_memory_base(T);
        lea.set_memory_displacement64(0);
        lea.set_memory_displ_size(0);
        if i.address32 {
            put(
                code,
                &Iced::with2(Code::Mov_r32_rm32, T_LOW, T_LOW).expect("a move"),
            );
        }
        lea
    } else {
        reach_operand(way, code, &ins, i.address32, reach, target)
    };
    let high = reach != Reach::Plain && names_high_byte(&ins);
    let (ins, legacy) = if high {
        on_legacy(way, code, &ins)
    } else {
        (ins, 0)
    };
    let at = code.offset() as u64;
    code.commit(&bytes_of(&ins, at), next);
    if high {
        put(code, &load_area(way, full(legacy), STAND_IN_VALUES));
        code.home(legacy);
    }
    stand_ins_out(way, code, &swaps);
}

fn names_high_byte(i: &Iced) -> bool {
    (0..i.op_count()).any(|n| {
        i.op_kind(n) == OpKind::Register && general(i.op_register(n)).is_some_and(|g| g.high)
    })
}

/// An instruction naming ah, ch, dh or bh cannot take a REX prefix, so its rebuilt operand cannot name t or
/// u: the address goes to a register of the first eight but rbp (whose accesses take the stack segment), kept in
/// the area meanwhile. It names no withheld
/// register, which would need REX itself, so the first stand-in slot is free.
fn on_legacy(way: Way, code: &mut HostCode, access: &Iced) -> (Iced, u8) {
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
    on.set_memory_base(full(legacy));
    on.set_memory_index(Register::None);
    on.set_memory_index_scale(1);
    on.set_memory_displacement64(0);
    on.set_memory_displ_size(0);
    (on, legacy)
}

/// Writes the host code of one guest instruction, `bytes` at guest `address`, as `route` says.
pub fn carry(
    way: Way,
    route: Route,
    i: &Instruction,
    bytes: &[u8],
    address: u64,
    code: &mut HostCode,
) {
    assert!(
        matches!(way, Way::Machine | Way::Process),
        "the in-process forms are written at XN2"
    );
    let next = address.wrapping_add(bytes.len() as u64);
    let repeated =
        crate::meaning::is_string(i) && (i.iced.has_rep_prefix() || i.iced.has_repne_prefix());
    if repeated && code.lead_after_commit() < code.lead() {
        // The count takes the meaning's passes, which the hardware's run of it does not show.
        code.slow(NativeReason::RepeatedStringCounted);
        call(way, code, InsideCall::Answer);
        code.after(Resume::At(next));
        return;
    }
    match route {
        Route::Copy if !names_withheld(&i.iced) => code.commit(bytes, Resume::At(next)),
        Route::Copy => rewritten(way, code, i, &i.iced, address, Reach::Plain),
        Route::Rebase => rewritten(way, code, i, &i.iced, address, Reach::Rebase),
        Route::Area => area(way, code, i, address),
        Route::Nothing => code.after(Resume::At(next)),
        Route::Kernel => {
            call(way, code, InsideCall::Answer);
            code.after(Resume::At(next));
        }
        Route::Trap => call(way, code, InsideCall::Trap),
        Route::FloatStatus => {
            call(way, code, InsideCall::FloatBefore);
            let reach =
                if has_memory_operand(&i.iced) && i.iced.memory_segment() == way.area_segment() {
                    Reach::Area
                } else if i.iced.is_ip_rel_memory_operand() {
                    Reach::Rebase
                } else {
                    Reach::Plain
                };
            rewritten(way, code, i, &i.iced, address, reach);
            code.fixup(Fixup::FloatStatus);
            call(way, code, InsideCall::FloatAfter);
            code.fixed();
        }
        Route::Transfer => transfer(way, code, i, address, next),
        Route::Split | Route::Collision | Route::Aligned | Route::Constant | Route::Exclusive => {
            unreachable!("x86-64 gives no {route:?} route")
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
        displ_size: WORD as u32,
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

/// After a copied x87 instruction that is not a control one: its guest address stored as the guest's FIP, the
/// hardware's holding a host address (section 3.1).
fn x87_pointer(way: Way, code: &mut HostCode, address: u64) {
    code.fixup(Fixup::X87Pointer(address));
    place_address(code, T, address);
    put(code, &store_area(way, X87_IP, T));
    code.fixed();
}

/// An operand or base instruction of the thread area's segment.
fn area(way: Way, code: &mut HostCode, i: &Instruction, address: u64) {
    let ins = &i.iced;
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
    } else {
        rewritten(way, code, i, ins, address, Reach::Area);
    }
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
                on_area(way, ENTRIES + usize::from(kind_code) * WORD, 0),
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
/// after an add that brings the count back to the guest's where it is ahead.
pub fn exit(way: Way, code: &mut HostCode, target: u64, commit: Option<Resume>) {
    if target >= USER_END {
        if let Some(resume) = commit {
            code.after(resume);
        }
        call(way, code, InsideCall::Exit);
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
    code[at..at + WORD].copy_from_slice(&place.address.to_le_bytes());
}

/// A branch, call, return or jump.
fn transfer(way: Way, code: &mut HostCode, i: &Instruction, address: u64, next: u64) {
    use Mnemonic::*;
    let ins = &i.iced;
    let direct = |offset: u64| address.wrapping_add(offset);
    match ins.mnemonic() {
        Jmp | Call if ins.op_kind(0) == OpKind::NearBranch64 => {
            let target = direct(ins.near_branch64());
            if target >= USER_END {
                call(way, code, InsideCall::Answer);
                return;
            }
            if ins.mnemonic() == Call {
                place_address(code, T, next);
                put(code, &Iced::with1(Code::Push_r64, T).expect("a push"));
                code.after(Resume::At(target));
                exit(way, code, target, None);
            } else {
                exit(way, code, target, Some(Resume::At(target)));
            }
        }
        Jmp | Call => {
            into_t(way, code, i, address);
            first_level(way, code);
            if ins.mnemonic() == Call {
                let own = STAND_IN_VALUES;
                put(code, &store_area(way, own, full(RAX)));
                code.relocate(RAX, page(own));
                place_address(code, full(RAX), next);
                put(
                    code,
                    &Iced::with1(Code::Push_r64, full(RAX)).expect("a push"),
                );
                code.after(Resume::Through(Held::Register(T_NUMBER)));
                put(code, &load_area(way, full(RAX), own));
                code.home(RAX);
            } else {
                code.after(Resume::Through(Held::Register(T_NUMBER)));
            }
            second_level(code);
        }
        Ret => {
            let top = MemoryOperand::with_base(full(RSP));
            put(
                code,
                &Iced::with2(Code::Mov_r64_rm64, T, top).expect("the return address"),
            );
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
            second_level(code);
        }
        _ if ins.is_jcc_short_or_near() => {
            let target = direct(ins.near_branch64());
            if target >= USER_END {
                call(way, code, InsideCall::Answer);
                code.after(Resume::At(next));
                return;
            }
            let mut jcc = *ins;
            jcc.set_code(ins.code().as_near_branch());
            let at = code.offset();
            jcc.set_near_branch64(at as u64);
            let bytes = bytes_of(&jcc, at as u64);
            let length = bytes.len();
            let linkable = code.lead_after_commit() == 0;
            code.commit(&bytes, Resume::At(next));
            if linkable {
                // iced numbers the conditions from 1 in the order of a `jcc`'s low four bits.
                let condition = ins.condition_code() as u8 - ConditionCode::o as u8;
                let field = u32::try_from(length - REL32).expect("small");
                code.branch_exit(at, target, field, Some(condition));
            }
            let after = code.snapshot();
            code.defer(move |code: &mut HostCode| {
                let to = code.offset();
                let field = at + length - REL32;
                let rel = i32::try_from(to - (at + length)).expect("a piece is small");
                code.patch(field, &rel.to_le_bytes());
                code.restore(after);
                code.after(Resume::At(target));
                if linkable {
                    call(way, code, InsideCall::Exit);
                } else {
                    exit(way, code, target, None);
                }
            });
        }
        Loop | Loope | Loopne | Jrcxz | Jecxz => {
            let target = direct(ins.near_branch64());
            if target >= USER_END {
                call(way, code, InsideCall::Answer);
                code.after(Resume::At(next));
                return;
            }
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
            exit(way, code, target, None);
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
    let reach = if operand.op0_kind() == OpKind::Register {
        Reach::Plain
    } else if i.iced.memory_segment() == way.area_segment() {
        Reach::Area
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
    stand_ins_in(way, code, &swaps);
    let read = reach_operand(way, code, &read, i.address32, reach, rip_target(i, address));
    put(code, &read);
    stand_ins_out(way, code, &swaps);
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
        WORD as u32,
        i64::try_from(FIRST_LEVEL).expect("a small offset"),
        WORD as u32,
        false,
        way.area_segment(),
    );
    put(
        code,
        &Iced::with2(Code::Mov_r64_rm64, U, level).expect("the first level"),
    );
}

/// The lookup's second level: the jump to the host entry of the piece at t.
fn second_level(code: &mut HostCode) {
    let entry = MemoryOperand::new(U, T, WORD as u32, 0, 0, false, Register::None);
    put(
        code,
        &Iced::with1(Code::Jmp_rm64, entry).expect("the second level"),
    );
}
