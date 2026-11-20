//! Operands: iced's registers as ours, register reads and the x86 rules for narrow writes, memory addresses
//! and the accesses each instruction makes.

use crate::X86_64;
use crate::decode::Instruction;
use crate::flags::Condition;
use iced_x86::{MemorySize, Mnemonic, OpKind, Register};
use wasm_encoder::{MemArg, MemArgTarget, ValType};
use wcontainers_guest_cpu::{
    Access, AccessBytes, AccessKind, Base, Emitter, Extend, Index, MemoryAccess, WriteBack,
};

/// The general registers' numbers (hardware order), and the FS and GS bases after them.
pub const RAX: u8 = 0;
pub const RCX: u8 = 1;
pub const RDX: u8 = 2;
pub const RBX: u8 = 3;
pub const RSP: u8 = crate::RSP_NUMBER;
pub const RBP: u8 = 5;
pub const RSI: u8 = 6;
pub const RDI: u8 = 7;
pub const R8: u8 = 8;
pub const R9: u8 = 9;
pub const R10: u8 = 10;
pub const R11: u8 = 11;
pub const R12: u8 = 12;
pub const R13: u8 = 13;
pub const R14: u8 = 14;
pub const R15: u8 = 15;
pub const FS: u8 = 16;
pub const GS: u8 = 17;

/// Operand widths in bits.
pub const W8: u8 = u8::BITS as u8;
pub const W16: u8 = u16::BITS as u8;
pub const W32: u8 = u32::BITS as u8;
pub const W64: u8 = u64::BITS as u8;
/// The bytes of a stack slot: a push, a pop and a return address.
pub const SLOT: u32 = size_of::<u64>() as u32;

/// A general register as iced names it: its number, its width in bits, and whether it is the second byte
/// (ah, ch, dh, bh).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct General {
    pub number: u8,
    pub width: u8,
    pub high: bool,
}

/// iced's general registers are in four runs of sixteen (the bytes al, cl, dl, bl, ah, ch, dh, bh, spl, bpl,
/// sil, dil, r8l to r15l first).
pub fn general(r: Register) -> Option<General> {
    let v = r as u32;
    let (number, width, high) = match v {
        1..=4 => (v - 1, 8, false),
        5..=8 => (v - 5, 8, true),
        9..=20 => (v - 5, 8, false),
        21..=36 => (v - 21, 16, false),
        37..=52 => (v - 37, 32, false),
        53..=68 => (v - 53, 64, false),
        _ => return None,
    };
    Some(General {
        number: number as u8,
        width,
        high,
    })
}

/// An xmm register's number (xmm0 to xmm15, the ones legacy SSE names).
pub fn xmm(r: Register) -> Option<u8> {
    let v = r as u32;
    (Register::XMM0 as u32..=Register::XMM15 as u32)
        .contains(&v)
        .then(|| (v - Register::XMM0 as u32) as u8)
}

pub fn mask(width: u8) -> i64 {
    if width == 64 { -1 } else { (1i64 << width) - 1 }
}

/// Push a general register's value zero-extended from its width.
pub fn get_general<E: Emitter<X86_64> + ?Sized>(e: &mut E, g: General) {
    e.get(g.number);
    let mut s = e.sink();
    if g.high {
        s.i64_const(8).i64_shr_u();
    }
    if g.width < 64 {
        s.i64_const(mask(g.width)).i64_and();
    }
}

/// Write the i64 in `value` to a general register: a 32-bit write clears the upper half, an 8- or 16-bit one
/// keeps the rest.
pub fn set_general<E: Emitter<X86_64> + ?Sized>(e: &mut E, g: General, value: u32) {
    match g.width {
        64 => {
            e.sink().local_get(value);
        }
        32 => {
            e.sink().local_get(value).i64_const(0xffff_ffff).i64_and();
        }
        _ => {
            let shift = if g.high { 8 } else { 0 };
            let field = mask(g.width) << shift;
            e.get(g.number);
            let mut s = e.sink();
            s.i64_const(!field).i64_and().local_get(value);
            if shift > 0 {
                s.i64_const(shift).i64_shl();
            }
            s.i64_const(field).i64_and().i64_or();
        }
    }
    e.set(g.number);
}

/// The module's memory that is the guest's.
pub const GUEST_MEMORY: u32 = 0;

pub fn memory() -> MemArg {
    MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(GUEST_MEMORY),
        ordering: None,
    }
}

/// Push the i64 of `bytes` bytes at the data address in `at`, zero-extended.
pub fn load<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, bytes: u32) {
    let mut s = e.sink();
    s.local_get(at);
    match bytes {
        1 => s.i64_load8_u(memory()),
        2 => s.i64_load16_u(memory()),
        4 => s.i64_load32_u(memory()),
        _ => s.i64_load(memory()),
    };
}

/// Store the low `bytes` bytes of the i64 in `value` at the data address in `at`.
pub fn store<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32, bytes: u32, value: u32) {
    let mut s = e.sink();
    s.local_get(at).local_get(value);
    match bytes {
        1 => s.i64_store8(memory()),
        2 => s.i64_store16(memory()),
        4 => s.i64_store32(memory()),
        _ => s.i64_store(memory()),
    };
}

/// The segment base an operand adds: FS or GS, or none (the others are zero in 64-bit mode).
pub fn segment(i: &Instruction) -> Option<u8> {
    match i.iced.memory_segment() {
        Register::FS => Some(FS),
        Register::GS => Some(GS),
        _ => None,
    }
}

/// Wrap the i64 address on the stack to 32 bits when the instruction has an address-size prefix.
fn wrap<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    if i.address32 {
        e.sink().i64_const(0xffff_ffff).i64_and();
    }
}

/// The address of the instruction's memory operand, into a fresh i64 local.
pub fn address<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) -> u32 {
    let ins = &i.iced;
    let at = e.local(ValType::I64);
    match ins.memory_base() {
        // iced decodes at address 0, so its displacement is from this instruction.
        Register::RIP | Register::EIP => e.this_address(ins.memory_displacement64() as i64),
        base => {
            e.sink().i64_const(ins.memory_displacement64() as i64);
            if let Some(g) = general(base) {
                e.get(g.number);
                e.sink().i64_add();
            }
            // `xlat`'s index is al, zero-extended.
            if let Some(g) = general(ins.memory_index()) {
                get_general(e, g);
                let scale = ins.memory_index_scale().trailing_zeros();
                let mut s = e.sink();
                if scale > 0 {
                    s.i64_const(scale.into()).i64_shl();
                }
                s.i64_add();
            }
        }
    }
    wrap(e, i);
    if let Some(base) = segment(i) {
        e.get(base);
        e.sink().i64_add();
    }
    e.sink().local_set(at);
    at
}

/// The address `register + by`, wrapped as the instruction's address size says, into a fresh i64 local.
pub fn register_address<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    register: u8,
    by: i64,
) -> u32 {
    let at = e.local(ValType::I64);
    e.get(register);
    if by != 0 {
        e.sink().i64_const(by).i64_add();
    }
    wrap(e, i);
    e.sink().local_set(at);
    at
}

/// The bytes of the instruction's memory operand, for the integer and far-pointer sizes (iced's own sizes are
/// in its encoder and `instr_info` tables, which the shipped decoder leaves out; a test checks these against
/// them); 0 for any other.
pub fn memory_bytes(i: &Instruction) -> u32 {
    size_of_memory(i.iced.memory_size())
}

pub fn size_of_memory(size: MemorySize) -> u32 {
    use MemorySize::*;
    match size {
        UInt8 | Int8 => 1,
        UInt16 | Int16 | WordOffset | Packed16_UInt8 | Packed16_Int8 => 2,
        UInt32 | Int32 | DwordOffset | SegPtr16 | Float32 | Packed32_UInt8 | Packed32_Int8
        | Packed32_UInt16 | Packed32_Int16 => 4,
        UInt64 | Int64 | QwordOffset | Float64 | Packed64_UInt8 | Packed64_Int8
        | Packed64_UInt16 | Packed64_Int16 | Packed64_UInt32 | Packed64_Int32
        | Packed64_Float32 => 8,
        UInt128 | Int128 | Packed128_UInt8 | Packed128_Int8 | Packed128_UInt16
        | Packed128_Int16 | Packed128_UInt32 | Packed128_Int32 | Packed128_UInt64
        | Packed128_Int64 | Packed128_Float32 | Packed128_Float64 => 16,
        SegPtr32 | Fword6 => 6,
        SegPtr64 | Fword10 => 10,
        _ => 0,
    }
}

/// Whether operand `n` is the explicit memory operand.
pub fn is_memory(i: &Instruction, n: u32) -> bool {
    i.iced.op_kind(n) == OpKind::Memory
}

/// What the instruction does with its explicit memory operand, if it has one.
pub fn memory_use(i: &Instruction) -> Option<AccessKind> {
    let ins = &i.iced;
    let at = (0..ins.op_count()).find(|&n| is_memory(i, n))?;
    use Mnemonic::*;
    Some(match (at, ins.mnemonic()) {
        // A nop's memory operand is never reached: no access, no check, no fault.
        // Nor is a prefetch's: it is a hint that never faults.
        (_, Nop | Reservednop | Lea | Prefetchnta | Prefetcht0 | Prefetcht1 | Prefetcht2) => {
            return None;
        }
        (0, Ldmxcsr) => AccessKind::Read,
        (0, Stmxcsr) => AccessKind::Write,
        (
            0,
            Cmp | Test | Push | Call | Jmp | Mul | Imul | Div | Idiv | Bt | Clflush | Xlatb | Verr
            | Verw,
        ) => AccessKind::Read,
        (
            0,
            Mov | Pop | Movaps | Movups | Movapd | Movupd | Movdqa | Movdqu | Movq | Movd | Movnti
            | Movntdq | Movntps | Movntpd | Movss | Movsd | Movlps | Movlpd | Movhps | Movhpd
            | Pextrb | Pextrw | Pextrd | Pextrq | Extractps | Sldt | Str | Smsw | Sgdt | Sidt,
        ) => AccessKind::Write,
        (0, m) if setcc(m).is_some() => AccessKind::Write,
        (0, _) => AccessKind::ReadWrite,
        (_, Xchg) => AccessKind::ReadWrite,
        _ => AccessKind::Read,
    })
}

/// The check an access of this kind makes.
pub fn check_kind(kind: AccessKind) -> Access {
    match kind {
        AccessKind::Read => Access::Read,
        AccessKind::Write | AccessKind::ReadWrite => Access::Write,
    }
}

/// The condition of a `jcc`, `cmovcc` or `setcc` mnemonic.
pub fn condition(m: Mnemonic) -> Option<Condition> {
    jcc(m).or_else(|| cmovcc(m)).or_else(|| setcc(m))
}

pub fn jcc(m: Mnemonic) -> Option<Condition> {
    use Mnemonic::*;
    Some(match m {
        Jo => Condition::Overflow,
        Jno => Condition::NotOverflow,
        Jb => Condition::Below,
        Jae => Condition::AboveOrEqual,
        Je => Condition::Equal,
        Jne => Condition::NotEqual,
        Jbe => Condition::BelowOrEqual,
        Ja => Condition::Above,
        Js => Condition::Sign,
        Jns => Condition::NotSign,
        Jp => Condition::Parity,
        Jnp => Condition::NotParity,
        Jl => Condition::Less,
        Jge => Condition::GreaterOrEqual,
        Jle => Condition::LessOrEqual,
        Jg => Condition::Greater,
        _ => return None,
    })
}

pub fn cmovcc(m: Mnemonic) -> Option<Condition> {
    use Mnemonic::*;
    Some(match m {
        Cmovo => Condition::Overflow,
        Cmovno => Condition::NotOverflow,
        Cmovb => Condition::Below,
        Cmovae => Condition::AboveOrEqual,
        Cmove => Condition::Equal,
        Cmovne => Condition::NotEqual,
        Cmovbe => Condition::BelowOrEqual,
        Cmova => Condition::Above,
        Cmovs => Condition::Sign,
        Cmovns => Condition::NotSign,
        Cmovp => Condition::Parity,
        Cmovnp => Condition::NotParity,
        Cmovl => Condition::Less,
        Cmovge => Condition::GreaterOrEqual,
        Cmovle => Condition::LessOrEqual,
        Cmovg => Condition::Greater,
        _ => return None,
    })
}

pub fn setcc(m: Mnemonic) -> Option<Condition> {
    use Mnemonic::*;
    Some(match m {
        Seto => Condition::Overflow,
        Setno => Condition::NotOverflow,
        Setb => Condition::Below,
        Setae => Condition::AboveOrEqual,
        Sete => Condition::Equal,
        Setne => Condition::NotEqual,
        Setbe => Condition::BelowOrEqual,
        Seta => Condition::Above,
        Sets => Condition::Sign,
        Setns => Condition::NotSign,
        Setp => Condition::Parity,
        Setnp => Condition::NotParity,
        Setl => Condition::Less,
        Setge => Condition::GreaterOrEqual,
        Setle => Condition::LessOrEqual,
        Setg => Condition::Greater,
        _ => return None,
    })
}

/// The bytes a push or pop moves: 2 with an operand-size prefix, else 8.
pub fn stack_bytes(i: &Instruction) -> u32 {
    let ins = &i.iced;
    let width = match ins.op_kind(0) {
        // `pushf`, `popf`, `leave` and `enter` with 66 move words; they have no register or memory operand.
        _ if matches!(
            ins.code(),
            iced_x86::Code::Pushfw
                | iced_x86::Code::Popfw
                | iced_x86::Code::Leavew
                | iced_x86::Code::Enterw_imm16_imm8
        ) =>
        {
            u16::BITS as u8
        }
        OpKind::Register => general(ins.op_register(0)).map_or(64, |g| g.width),
        OpKind::Memory => memory_bytes(i) as u8 * 8,
        // An immediate's width is the operand size: `push imm16` has 66.
        _ if matches!(
            ins.code(),
            iced_x86::Code::Pushw_imm8 | iced_x86::Code::Push_imm16
        ) =>
        {
            16
        }
        _ => 64,
    };
    u32::from(width / 8)
}

/// The size of a string instruction's element.
pub fn element_bytes(i: &Instruction) -> u32 {
    memory_bytes(i)
}

/// An access through `register` (with `by` added first) of `bytes` bytes.
pub fn through(register: u8, by: i64, bytes: u32, kind: AccessKind) -> MemoryAccess<u8> {
    MemoryAccess {
        segment: None,
        base: Some(Base::Register(register)),
        index: None,
        displacement: by,
        address_bits: 64,
        write_back: WriteBack::None,
        bytes: AccessBytes::Fixed(bytes as u16),
        kind,
    }
}

/// The explicit memory operand as an access description.
pub fn operand_access(i: &Instruction, kind: AccessKind) -> MemoryAccess<u8> {
    let ins = &i.iced;
    let (base, displacement) = match ins.memory_base() {
        Register::RIP | Register::EIP => (Some(Base::Pc), ins.memory_displacement64() as i64),
        b => (
            general(b).map(|g| Base::Register(g.number)),
            ins.memory_displacement64() as i64,
        ),
    };
    MemoryAccess {
        segment: segment(i),
        base,
        index: general(ins.memory_index()).map(|g| Index::Register {
            reg: g.number,
            shift: ins.memory_index_scale().trailing_zeros() as u8,
            extend: Extend {
                from_bits: g.width,
                signed: false,
            },
        }),
        displacement,
        address_bits: if i.address32 { 32 } else { 64 },
        write_back: WriteBack::None,
        bytes: AccessBytes::Fixed(memory_bytes(i) as u16),
        kind,
    }
}
