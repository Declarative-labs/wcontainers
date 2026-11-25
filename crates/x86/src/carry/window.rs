//! The in-process way's forms of a guest access (docs/x86-native-output.md section 4): each on GS, the window's
//! base, at a 32-bit address, so that no access, run or only speculated, leaves the window; the exact form first
//! probes the address the access reaches without writing a flag, so that one at or above 2^32 is the guest's fault.
//! rsp holds B plus the guest's rsp, so the stack's own accesses run as the guest wrote them.

use iced_x86::{
    Code, Instruction as Iced, InstructionInfoFactory, MemoryOperand, Mnemonic, OpAccess, OpKind,
    Register,
};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_guest_cpu::HostCode;
use wcontainers_guest_cpu::position::{Held, Resume};

use super::{AREA_WORD, T, T_LOW, U, U_LOW, bytes_of, full, load_area, put, sized, store_area};
use crate::area::{
    GUEST_FS_BASE, GUEST_GS_BASE, KEPT_T, PROBE_PAGE, STAND_IN_VALUES, WINDOW, WINDOW_NEGATED,
};
use crate::native::{Check, Way};
use crate::operand::{R8, RAX, RBP, RBX, RDI, RSI, RSP, W16, W32, W64, general};

/// t's low 16 bits, for a 16-bit `leave`.
const T_WORD: Register = Register::R11W;
/// The bytes of a displacement that make an operand without registers a 32-bit address (iced's convention).
const DISPLACEMENT32: u32 = size_of::<u32>() as u32;
/// How far below rsp an operand made as the guest wrote it may start: the guard page under the window.
const STACK_REACH_BELOW: i64 = PAGE_SIZE as i64;

/// Whether host instruction `i` reads or writes memory (forming an address alone, as `lea` does, is not).
pub(super) fn reaches_memory(i: &Iced) -> bool {
    let mut info = InstructionInfoFactory::new();
    info.info(i)
        .used_memory()
        .iter()
        .any(|m| m.access() != OpAccess::NoMemAccess)
}

/// Writes host instruction `i`, which reaches guest memory, in the window's form `check`, with `resume` its commit
/// point.
pub(super) fn access(check: Check, code: &mut HostCode, i: &Iced, resume: Option<Resume>) {
    use Mnemonic::*;
    let form = Form {
        exact: check == Check::Exact,
        way: Way::InProcess(check),
    };
    match i.mnemonic() {
        // The stack's side as written; an operand in memory in the window's form.
        Push | Pop if i.op0_kind() != OpKind::Memory => finish(code, i, resume),
        Push => explicit(form, code, i, resume),
        Pop => pop(form, code, i, resume),
        Leave => leave(form, code, i, resume),
        Xlatb | Maskmovq | Maskmovdqu | Vmaskmovdqu => implicit(form, code, i, resume),
        Bt | Bts | Btr | Btc if form.exact && i.op1_kind() == OpKind::Register => {
            bit_test(form, code, i, resume)
        }
        _ if is_string(i) => string(form, code, i, resume),
        _ => explicit(form, code, i, resume),
    }
}

/// Whether `i`'s operand is rsp and a displacement no lower than the guard page under the window: with rsp at B plus
/// a guest rsp within [0, 2^32 + 2^16 + 8], it reaches only the window and our reservation without access around it,
/// so it is made as the guest wrote it (the operand alone: never a bit test's register offset).
fn on_stack_as_written(i: &Iced) -> bool {
    i.memory_base() == Register::RSP
        && i.memory_index() == Register::None
        && i.memory_displacement64() as i64 >= -STACK_REACH_BELOW
        && guest_segment(i).is_none()
        && !(matches!(
            i.mnemonic(),
            Mnemonic::Bt | Mnemonic::Bts | Mnemonic::Btr | Mnemonic::Btc
        ) && i.op1_kind() == OpKind::Register)
}

/// The guest's rsp into `s`: rsp less the window's base.
pub(super) fn guest_stack_into(way: Way, code: &mut HostCode, s: Register) {
    put(code, &load_area(way, s, WINDOW_NEGATED));
    put(
        code,
        &Iced::with2(
            Code::Lea_r64_m,
            s,
            MemoryOperand::with_base_index(full(RSP), s),
        )
        .expect("the guest's rsp"),
    );
}

/// rsp made B plus the guest rsp written in `s`, which the map holds meanwhile: in the exact form a value at or above
/// 2^32 faults at the probe, the instruction done (section 5); in both it is cut to 32 bits, so that rsp stays in the
/// window even where that probe's fault is only speculated.
pub(super) fn guest_stack_from(check: Check, code: &mut HostCode, s: Register) {
    let number = general(s).expect("a general register").number;
    code.relocate(RSP, Held::Register(number));
    if check == Check::Exact {
        probe(code, s);
    }
    let low = sized(number, W32);
    put(
        code,
        &Iced::with2(Code::Mov_r32_rm32, low, low).expect("the rsp cut"),
    );
    put(code, &load_area(Way::InProcess(check), U, WINDOW));
    put(
        code,
        &Iced::with2(
            Code::Lea_r64_m,
            full(RSP),
            MemoryOperand::with_base_index(s, U),
        )
        .expect("rsp in the window"),
    );
    code.home(RSP);
}

#[derive(Clone, Copy)]
struct Form {
    exact: bool,
    way: Way,
}

fn is_string(i: &Iced) -> bool {
    (0..i.op_count()).any(|n| {
        matches!(
            i.op_kind(n),
            OpKind::MemorySegSI
                | OpKind::MemorySegESI
                | OpKind::MemorySegRSI
                | OpKind::MemoryESDI
                | OpKind::MemoryESEDI
                | OpKind::MemoryESRDI
        )
    })
}

/// Writes the access itself, the commit point where `resume` is given.
fn finish(code: &mut HostCode, i: &Iced, resume: Option<Resume>) {
    let bytes = bytes_of(i, code.offset() as u64);
    match resume {
        Some(resume) => code.commit(&bytes, resume),
        None => code.push(&bytes),
    }
}

/// A 64-bit general register's low half; any other register as it is.
fn narrow(r: Register) -> Register {
    general(r)
        .filter(|g| g.width == W64)
        .map_or(r, |g| sized(g.number, W32))
}

/// The size iced is asked for a 32-bit address's displacement: a byte where it fits.
fn displacement_size(displacement: u32) -> u32 {
    if displacement == 0 {
        0
    } else if i8::try_from(displacement as i32).is_ok() {
        1
    } else {
        4
    }
}

/// The guest's operand size prefix 0x67: its operand's address formed in 32 bits.
fn address32(i: &Iced) -> bool {
    let width = |r: Register| general(r).map(|g| g.width);
    match (width(i.memory_base()), width(i.memory_index())) {
        (Some(w), _) | (None, Some(w)) => w == W32,
        (None, None) => i.memory_displ_size() == DISPLACEMENT32,
    }
}

/// `i` with its operand on GS at the same address taken in 32 bits.
fn cut(i: &Iced) -> Iced {
    let mut out = *i;
    out.set_memory_base(narrow(i.memory_base()));
    out.set_memory_index(narrow(i.memory_index()));
    let displacement = i.memory_displacement64() as u32;
    out.set_memory_displacement64(u64::from(displacement));
    // Without a base the operand's displacement is 32 bits wide whatever its value.
    out.set_memory_displ_size(if out.memory_base() == Register::None {
        DISPLACEMENT32
    } else {
        displacement_size(displacement)
    });
    out.set_segment_prefix(Register::GS);
    out
}

/// `i` with its operand on GS at register `base`'s low 32 bits.
fn on(i: &Iced, base: Register) -> Iced {
    let mut out = *i;
    // An absolute address's move takes no register: its ModRM form, with the same operands in the same order.
    let modrm = match i.code() {
        Code::Mov_AL_moffs8 => Some(Code::Mov_r8_rm8),
        Code::Mov_AX_moffs16 => Some(Code::Mov_r16_rm16),
        Code::Mov_EAX_moffs32 => Some(Code::Mov_r32_rm32),
        Code::Mov_RAX_moffs64 => Some(Code::Mov_r64_rm64),
        Code::Mov_moffs8_AL => Some(Code::Mov_rm8_r8),
        Code::Mov_moffs16_AX => Some(Code::Mov_rm16_r16),
        Code::Mov_moffs32_EAX => Some(Code::Mov_rm32_r32),
        Code::Mov_moffs64_RAX => Some(Code::Mov_rm64_r64),
        _ => None,
    };
    if let Some(modrm) = modrm {
        out.set_code(modrm);
    }
    out.set_memory_base(narrow(base));
    out.set_memory_index(Register::None);
    out.set_memory_index_scale(1);
    out.set_memory_displacement64(0);
    out.set_memory_displ_size(0);
    out.set_segment_prefix(Register::GS);
    out
}

/// The flag-free probe of the address in `x` (section 4): u reads the page of zeros at B - 2 pages when bits 32 to 63
/// of the address are clear, else a place 8 to 2^35 bytes below it where nothing is mapped, so the guest faults.
/// `x` may be u.
fn probe(code: &mut HostCode, x: Register) {
    put(
        code,
        &Iced::with3(Code::VEX_Rorx_r64_rm64_imm8, U, x, u64::BITS / 2).expect("the high half"),
    );
    put(
        code,
        &Iced::with2(Code::Mov_r32_rm32, U_LOW, U_LOW).expect("alone"),
    );
    let scaled = MemoryOperand::new(
        Register::None,
        U,
        AREA_WORD as u32,
        0,
        AREA_WORD as u32,
        false,
        Register::None,
    );
    put(
        code,
        &Iced::with2(Code::Lea_r64_m, U, scaled).expect("a word's distance"),
    );
    put(code, &Iced::with1(Code::Not_rm64, U).expect("below"));
    let below = 1 - i64::try_from(PROBE_PAGE).expect("a page");
    let read = MemoryOperand::new(
        U,
        Register::None,
        1,
        below,
        AREA_WORD as u32,
        false,
        Register::GS,
    );
    put(
        code,
        &Iced::with2(Code::Mov_r64_rm64, U, read).expect("the probe"),
    );
}

/// The probe of the address `bytes` past `x` as well: the last byte of an access the CPU makes in parts, each at
/// its own 32-bit address, which would otherwise wrap past 2^32 into the window.
fn probe_end(code: &mut HostCode, x: Register, bytes: usize) {
    let past = i64::try_from(bytes).expect("small") - 1;
    put(
        code,
        &Iced::with2(Code::Lea_r64_m, U, MemoryOperand::with_base_displ(x, past))
            .expect("the last byte"),
    );
    probe(code, U);
}

/// The x87 and FXSAVE images and environments, which the CPU stores and loads in parts.
fn in_parts(i: &Iced) -> bool {
    use Mnemonic::*;
    matches!(
        i.mnemonic(),
        Fxsave | Fxsave64 | Fxrstor | Fxrstor64 | Fnsave | Frstor | Fnstenv | Fldenv
    )
}

/// The guest's segment of the operand, by its base's word in the register area.
fn guest_segment(i: &Iced) -> Option<usize> {
    match i.memory_segment() {
        Register::FS => Some(GUEST_FS_BASE),
        Register::GS => Some(GUEST_GS_BASE),
        _ => None,
    }
}

/// Writes t as the operand's offset part (base + index * scale + displacement), wrapped to 32 bits under the
/// guest's 0x67, unless t is that already.
fn offset_into_t(way: Way, code: &mut HostCode, i: &Iced) {
    let (base, index) = (i.memory_base(), i.memory_index());
    let displacement = i.memory_displacement64();
    let wrapped = address32(i);
    if index == Register::None && displacement == 0 && matches!(base, T | T_LOW) {
        return;
    }
    if base == Register::None && index == Register::None {
        let at = if wrapped {
            Iced::with2(Code::Mov_r32_imm32, T_LOW, displacement as u32)
        } else {
            Iced::with2(Code::Mov_r64_imm64, T, displacement)
        };
        put(code, &at.expect("an absolute address"));
        return;
    }
    let displacement = if wrapped {
        i64::from(displacement as u32 as i32)
    } else {
        displacement as i64
    };
    let operand = MemoryOperand::new(
        base,
        index,
        i.memory_index_scale(),
        displacement,
        if displacement == 0 {
            0
        } else if wrapped {
            DISPLACEMENT32
        } else {
            AREA_WORD as u32
        },
        false,
        Register::None,
    );
    let lea = if wrapped {
        Iced::with2(Code::Lea_r32_m, T_LOW, operand)
    } else {
        Iced::with2(Code::Lea_r64_m, T, operand)
    };
    put(code, &lea.expect("the offset"));
    // rsp is B more than the guest's (in 32 bits, B's low half is zero).
    if base == Register::RSP {
        put(code, &load_area(way, U, WINDOW_NEGATED));
        put(
            code,
            &Iced::with2(Code::Lea_r64_m, T, MemoryOperand::with_base_index(T, U))
                .expect("the guest's offset"),
        );
    }
}

/// Writes what forms the address the access reaches where it is not one register already, and gives that register
/// with the access made on it: the guest's segment base added to the offset part, as the meaning adds it.
fn operand_address(form: Form, code: &mut HostCode, i: &Iced) -> (Register, Iced) {
    if let Some(word) = guest_segment(i) {
        offset_into_t(form.way, code, i);
        put(code, &load_area(form.way, U, word));
        put(
            code,
            &Iced::with2(Code::Lea_r64_m, T, MemoryOperand::with_base_index(T, U))
                .expect("the segment's base added"),
        );
        return (T, on(i, T));
    }
    let base = i.memory_base();
    let alone = i.memory_index() == Register::None && i.memory_displacement64() == 0;
    if alone && general(base).is_some_and(|g| g.width == W64 && g.number != RSP) {
        return (base, cut(i));
    }
    offset_into_t(form.way, code, i);
    (T, on(i, T))
}

/// An instruction whose operand alone is the guest memory it reaches.
fn explicit(form: Form, code: &mut HostCode, i: &Iced, resume: Option<Resume>) {
    assert!(
        (0..i.op_count()).any(|n| i.op_kind(n) == OpKind::Memory),
        "an access of a shape the window has no form for: {:?}",
        i.code()
    );
    if on_stack_as_written(i) {
        return finish(code, i, resume);
    }
    let segment = guest_segment(i).is_some();
    // Under the guest's 0x67 the address is below 2^32 and the CPU wraps each part as the meaning does.
    if !segment && (!form.exact || address32(i)) {
        return finish(code, &cut(i), resume);
    }
    let (x, at) = operand_address(form, code, i);
    if form.exact {
        probe(code, x);
        if in_parts(i) && !address32(i) {
            probe_end(code, x, i.memory_size().size());
        }
    }
    let rex = general(x).is_some_and(|g| g.number >= R8);
    if !(rex && super::names_high_byte(i)) {
        return finish(code, &at, resume);
    }
    // ah, ch, dh and bh exclude a REX prefix, so the address goes to a register of the first eight but rsp, kept
    // in the area meanwhile; such an instruction names no withheld register, and the first stand-in slot may hold
    // the register a RIP-relative operand's address took (`carry::on_legacy`), so the second is free.
    let mut info = InstructionInfoFactory::new();
    let used: Vec<u8> = info
        .info(i)
        .used_registers()
        .iter()
        .filter_map(|u| general(u.register()).map(|g| g.number))
        .collect();
    let legacy = (RAX..R8)
        .find(|n| *n != RSP && !used.contains(n))
        .expect("a byte instruction leaves a legacy register free");
    let own = STAND_IN_VALUES + AREA_WORD;
    put(code, &store_area(form.way, own, full(legacy)));
    code.relocate(legacy, Held::Page(own as u16));
    put(
        code,
        &Iced::with2(Code::Mov_r64_rm64, full(legacy), x).expect("the address"),
    );
    finish(code, &on(&at, full(legacy)), resume);
    put(code, &load_area(form.way, full(legacy), own));
    code.home(legacy);
}

/// A pop to memory: the stack's side as written, the destination in the window's form; the CPU forms an rsp-based
/// destination after rsp moves, which the exact form's probe follows.
fn pop(form: Form, code: &mut HostCode, i: &Iced, resume: Option<Resume>) {
    if on_stack_as_written(i) {
        return finish(code, i, resume);
    }
    if guest_segment(i).is_none() && (!form.exact || address32(i)) {
        return finish(code, &cut(i), resume);
    }
    let mut formed = *i;
    if general(i.memory_base()).is_some_and(|g| g.number == RSP && g.width == W64) {
        let bytes = u64::from(i.stack_pointer_increment().unsigned_abs());
        formed.set_memory_displacement64(i.memory_displacement64().wrapping_add(bytes));
        formed.set_memory_displ_size(AREA_WORD as u32);
    }
    let (x, at) = operand_address(form, code, &formed);
    if form.exact {
        probe(code, x);
    }
    finish(code, &at, resume);
}

/// `leave`: rbp's load into t from the window, then rbp written, the commit point, after which u holds the guest's
/// rsp, rbp's low half and the bytes popped, until rsp is B more.
fn leave(form: Form, code: &mut HostCode, i: &Iced, resume: Option<Resume>) {
    let word = i.code() == Code::Leaveq;
    let bytes = i64::from(if word { W64 } else { W16 } / 8);
    if form.exact {
        probe(code, full(RBP));
    }
    let frame = MemoryOperand::new(Register::EBP, Register::None, 1, 0, 0, false, Register::GS);
    let load = if word {
        Iced::with2(Code::Mov_r64_rm64, T, frame)
    } else {
        put(
            code,
            &Iced::with2(Code::Mov_r64_rm64, T, full(RBP)).expect("the rest of rbp"),
        );
        Iced::with2(Code::Mov_r16_rm16, T_WORD, frame)
    };
    finish(code, &load.expect("the frame's load"), None);
    put(
        code,
        &Iced::with2(Code::Mov_r32_rm32, U_LOW, Register::EBP).expect("rbp cut"),
    );
    put(
        code,
        &Iced::with2(Code::Lea_r64_m, U, MemoryOperand::with_base_displ(U, bytes))
            .expect("the guest's rsp"),
    );
    finish(
        code,
        &Iced::with2(Code::Mov_r64_rm64, full(RBP), T).expect("rbp"),
        resume,
    );
    code.relocate(RSP, Held::Register(super::U_NUMBER));
    put(code, &load_area(form.way, T, WINDOW));
    put(
        code,
        &Iced::with2(
            Code::Lea_r64_m,
            full(RSP),
            MemoryOperand::with_base_index(U, T),
        )
        .expect("rsp in the window"),
    );
    code.home(RSP);
}

/// `xlat` at rbx + al, and the masked stores at rdi, which no override of ours can move off their implicit
/// registers: made on GS at 32 bits, the exact form probing the address first (section 4).
fn implicit(form: Form, code: &mut HostCode, i: &Iced, resume: Option<Resume>) {
    let mut out = *i;
    out.set_segment_prefix(Register::GS);
    if i.mnemonic() == Mnemonic::Xlatb {
        let wrapped = address32(i);
        out.set_memory_base(narrow(i.memory_base()));
        if form.exact && !wrapped {
            put(
                code,
                &Iced::with2(Code::Movzx_r32_rm8, U_LOW, Register::AL).expect("al"),
            );
            put(
                code,
                &Iced::with2(
                    Code::Lea_r64_m,
                    U,
                    MemoryOperand::with_base_index(full(RBX), U),
                )
                .expect("the table's byte"),
            );
            probe(code, U);
        }
    } else {
        let n = (0..i.op_count())
            .find(|&n| matches!(i.op_kind(n), OpKind::MemorySegRDI | OpKind::MemorySegEDI))
            .expect("a masked store's operand");
        if form.exact && i.op_kind(n) == OpKind::MemorySegRDI {
            probe(code, full(RDI));
        }
        out.set_op_kind(n, OpKind::MemorySegEDI);
    }
    finish(code, &out, resume);
}

/// A bit test with a register offset in the exact form, which reaches A = t + (w/8) * (offset SAR log2 w), not its
/// operand t: the probe is made on A, computed in u without a flag, and the CPU, under the address-size prefix,
/// reaches A modulo 2^32 from t's low half (section 4).
fn bit_test(form: Form, code: &mut HostCode, i: &Iced, resume: Option<Resume>) {
    let segment = guest_segment(i);
    let wrapped = address32(i);
    if wrapped && segment.is_none() {
        // A is wrapped to 32 bits, as the meaning wraps it.
        return finish(code, &cut(i), resume);
    }
    // Under the guest's 0x67 on a segment, A is the segment's base plus the wrapped sum of the operand's offset and
    // the bit's words (x86/meaning.rs `bit_test`): the base is added after.
    let (x, at) = if wrapped {
        offset_into_t(form.way, code, i);
        (T, on(i, T))
    } else {
        operand_address(form, code, i)
    };
    let offset = general(i.op1_register()).expect("a general register");
    let scale = u32::from(offset.width / 8);
    let shift = scale.trailing_zeros() + 3;
    let sarx = |width: u8, into: Register, from: Register, count: Register| {
        let code_of = if width == W64 {
            Code::VEX_Sarx_r64_rm64_r64
        } else {
            Code::VEX_Sarx_r32_rm32_r32
        };
        Iced::with3(code_of, into, from, count).expect("the offset's words")
    };
    if offset.width == W16 {
        // sarx has no 16-bit form: the offset is sign-extended first and the count borrows t.
        put(
            code,
            &Iced::with2(Code::Movsx_r64_rm16, U, i.op1_register()).expect("the offset"),
        );
        let keep = x == T;
        if keep {
            put(code, &store_area(form.way, KEPT_T, T));
        }
        put(
            code,
            &Iced::with2(Code::Mov_r32_imm32, T_LOW, shift).expect("the count"),
        );
        put(code, &sarx(W64, U, U, T));
        if keep {
            put(code, &load_area(form.way, T, KEPT_T));
        }
    } else {
        put(
            code,
            &Iced::with2(Code::Mov_r32_imm32, U_LOW, shift).expect("the count"),
        );
        if offset.width == W64 {
            put(code, &sarx(W64, U, i.op1_register(), U));
        } else {
            put(code, &sarx(W32, U_LOW, i.op1_register(), U_LOW));
            put(
                code,
                &Iced::with2(Code::Movsxd_r64_rm32, U, U_LOW).expect("sign-extended"),
            );
        }
    }
    let Some(word) = segment.filter(|_| wrapped) else {
        let reached = MemoryOperand::new(x, U, scale, 0, 0, false, Register::None);
        put(
            code,
            &Iced::with2(Code::Lea_r64_m, U, reached).expect("the address reached"),
        );
        probe(code, U);
        return finish(code, &at, resume);
    };
    let reached = MemoryOperand::new(T_LOW, U_LOW, scale, 0, 0, false, Register::None);
    put(
        code,
        &Iced::with2(Code::Lea_r32_m, U_LOW, reached).expect("the wrapped offset reached"),
    );
    // The offset waits in the area while t holds the segment's base.
    put(code, &store_area(form.way, KEPT_T, T));
    put(code, &load_area(form.way, T, word));
    let sum = |into: Register| {
        Iced::with2(Code::Lea_r64_m, into, MemoryOperand::with_base_index(T, U))
            .expect("the segment's base added")
    };
    put(code, &sum(U));
    probe(code, U);
    put(code, &load_area(form.way, U, KEPT_T));
    put(code, &sum(T));
    finish(code, &at, resume);
}

/// A string instruction without the guest's 0x67 or a segment prefix (the inside answers those): each start
/// probed in both forms, so that one at or above 2^32 goes to the inside, then cut to 32 bits, the rsi side on GS and
/// rdi rebased by B around it, since its ES side takes no override (section 4). A run from a start in the window
/// meets a guard page before it leaves.
fn string(form: Form, code: &mut HostCode, i: &Iced, resume: Option<Resume>) {
    let kinds: Vec<OpKind> = (0..i.op_count()).map(|n| i.op_kind(n)).collect();
    assert!(
        !kinds.iter().any(|k| matches!(
            k,
            OpKind::MemorySegSI | OpKind::MemorySegESI | OpKind::MemoryESDI | OpKind::MemoryESEDI
        )) && i.segment_prefix() == Register::None,
        "a string the inside answers: {i}"
    );
    let source = kinds.contains(&OpKind::MemorySegRSI);
    let destination = kinds.contains(&OpKind::MemoryESRDI);
    let starts: Vec<u8> = [(source, RSI), (destination, RDI)]
        .into_iter()
        .filter_map(|(used, r)| used.then_some(r))
        .collect();
    for &r in &starts {
        probe(code, full(r));
    }
    for &r in &starts {
        let low = sized(r, W32);
        put(
            code,
            &Iced::with2(Code::Mov_r32_rm32, low, low).expect("the start cut"),
        );
    }
    let rebase = |code: &mut HostCode, by: usize| {
        put(code, &load_area(form.way, T, by));
        put(
            code,
            &Iced::with2(
                Code::Lea_r64_m,
                full(RDI),
                MemoryOperand::with_base_index(full(RDI), T),
            )
            .expect("rdi moved by the window's base"),
        );
    };
    if destination {
        rebase(code, WINDOW);
        code.relocate(RDI, Held::Rebased(RDI));
    }
    let mut out = *i;
    if source {
        out.set_segment_prefix(Register::GS);
    }
    finish(code, &out, resume);
    if destination {
        rebase(code, WINDOW_NEGATED);
        code.home(RDI);
    }
}
