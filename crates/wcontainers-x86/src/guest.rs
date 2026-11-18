//! [`X86_64`]'s `Guest`: the register file, flags, traps and the facts about each instruction the analyses
//! use.

use crate::decode::Instruction;
use crate::flags::{Flag, Flags};
use crate::meaning::is_string;
use crate::operand::*;
use crate::{FLAGS_WORD, FS_BASE, GS_BASE, RFLAGS, X86_64, X86Block, XMM};
use core::ops::Range;
use iced_x86::{Mnemonic, OpKind, Register};
use wasm_encoder::{InstructionSink, ValType};
use wcontainers_guest_cpu::*;

/// Registers kept only in the thread's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The rflags bits user code keeps outside the six flags (TF, DF, NT and ID), at their positions.
    Rflags,
}

/// No hooks yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {}

/// No flags-word functions yet: every flag setting is worked out when it is written back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trap {
    /// Not an instruction: illegal instruction.
    Undefined,
    /// An instruction of a feature not claimed: illegal instruction, counted.
    Unsupported,
    GeneralProtection,
    DivideError,
    /// `int3` and `int 3`: SIGTRAP with `SI_KERNEL`, after the instruction.
    Int3,
    /// `int1`: SIGTRAP with `TRAP_BRKPT`, after the instruction.
    Int1,
    /// `int 4`: a trap Linux sends as SIGSEGV, after the instruction.
    Overflow,
}

/// x86's own slow paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    AvxNotOffered,
    ThirtyTwoBitSystemCall,
}

/// What the container offers beyond the default claims: the AVX setting (x86-64-v3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Offer {
    pub avx: bool,
}

const fn general_bit(n: u32) -> Bit {
    Bit {
        bit: n,
        class: Class::General,
        ty: Ty::I64,
        offset: 8 * n as u16,
    }
}

const fn flag_bit(n: u32) -> Bit {
    Bit {
        bit: n,
        class: Class::Flag,
        ty: Ty::I32,
        offset: FLAGS_WORD as u16,
    }
}

/// The register bit of the first flag and of the first xmm register.
const FIRST_FLAG: u32 = 18;
const FIRST_XMM: u32 = 32;

const BITS: [Bit; 40] = {
    let mut bits = [general_bit(0); 40];
    let mut n = 0;
    while n < 16 {
        bits[n] = general_bit(n as u32);
        n += 1;
    }
    bits[16] = Bit {
        bit: FS as u32,
        class: Class::Segment,
        ty: Ty::I64,
        offset: FS_BASE as u16,
    };
    bits[17] = Bit {
        bit: GS as u32,
        class: Class::Segment,
        ty: Ty::I64,
        offset: GS_BASE as u16,
    };
    let mut f = 0;
    while f < 6 {
        bits[18 + f] = flag_bit(FIRST_FLAG + f as u32);
        f += 1;
    }
    let mut x = 0;
    while x < 16 {
        bits[24 + x] = Bit {
            bit: FIRST_XMM + x as u32,
            class: Class::Vector,
            ty: Ty::V128,
            offset: (XMM + 16 * x) as u16,
        };
        x += 1;
    }
    bits
};

/// Every flag's register bit.
const ALL_FLAGS: Registers = Registers(0x3f << FIRST_FLAG);

/// An access through the stack segment: base rsp or rbp, no `fs:` or `gs:`.
fn rsp_based(access: &MemoryAccess<u8>) -> bool {
    access.segment.is_none()
        && matches!(
            access.base,
            Some(Base::Register(RSP)) | Some(Base::Register(5))
        )
}

impl Guest for X86_64 {
    const ID: GuestId = GuestId::X86_64;
    const ALIGN: u64 = 1;
    const MAX_LENGTH: u64 = 15;
    type Instruction = Instruction;

    fn decode(code: &[u8]) -> Option<(Instruction, u8)> {
        crate::decode::decode(code)
    }

    /// The first 8 bytes, zero-filled past the end.
    fn trap_detail(code: &[u8]) -> i64 {
        let mut bytes = [0u8; 8];
        let n = code.len().min(8);
        bytes[..n].copy_from_slice(&code[..n]);
        i64::from_le_bytes(bytes)
    }

    type Reg = u8;
    type Vector = u8;
    const BITS: &'static [Bit] = &BITS;

    #[inline]
    fn reg_bit(r: u8) -> Option<u32> {
        Some(r.into())
    }

    #[inline]
    fn vector_bit(v: u8) -> u32 {
        FIRST_XMM + u32::from(v)
    }

    #[inline]
    fn bit_reg(bit: u32) -> u8 {
        bit as u8
    }

    const SP_BIT: u32 = RSP as u32;
    const PASSED_IN: &'static [u32] = &[4, 7, 6, 2, 1, 8, 9, 0];
    const PASSED_OUT: &'static [u32] = &[4, 0, 2];

    type Flag = Flag;
    const FLAGS: &'static [(Flag, u32, u32)] = &[
        (Flag::C, 0, FIRST_FLAG),
        (Flag::P, 2, FIRST_FLAG + 1),
        (Flag::A, 4, FIRST_FLAG + 2),
        (Flag::Z, 6, FIRST_FLAG + 3),
        (Flag::S, 7, FIRST_FLAG + 4),
        (Flag::O, 11, FIRST_FLAG + 5),
    ];
    const FLAGS_WORD_OFFSET: usize = FLAGS_WORD;
    type Flags = Flags;
    type Shape = Shape;
    const SHAPES: &'static [Shape] = &[];

    fn shape(_flags: Flags) -> Option<Shape> {
        None
    }

    fn shape_locals(shape: Shape) -> &'static [ValType] {
        match shape {}
    }

    fn with_locals(shape: Shape, _locals: &[u32]) -> Flags {
        match shape {}
    }

    fn locals_of(flags: Flags) -> Locals {
        match flags {
            Flags::Add {
                a,
                b,
                carry,
                result,
                kept_carry,
                ..
            }
            | Flags::Sub {
                a,
                b,
                borrow: carry,
                result,
                kept_carry,
                ..
            } => {
                let mut locals = vec![a, b, result];
                locals.extend(carry);
                locals.extend(kept_carry);
                Locals::new(&locals)
            }
            Flags::Logic { result, .. } => Locals::new(&[result]),
            Flags::Multiply {
                result, overflow, ..
            } => Locals::new(&[result, overflow]),
            Flags::Packed(word) => Locals::new(&[word]),
        }
    }

    fn temporaries(_flags: Flags) -> Option<ValType> {
        None
    }

    fn complete(_sink: &mut InstructionSink<'_>, flags: Flags, _temporaries: [u32; 2]) -> Flags {
        flags
    }

    fn sets(flags: Flags) -> Registers {
        match flags {
            Flags::Add {
                kept_carry: Some(_),
                ..
            }
            | Flags::Sub {
                kept_carry: Some(_),
                ..
            } => ALL_FLAGS.minus(Registers(1 << FIRST_FLAG)),
            _ => ALL_FLAGS,
        }
    }

    fn flag_value(sink: &mut InstructionSink<'_>, flags: Flags, flag: Flag) {
        crate::flags::flag_value(sink, flags, flag);
    }

    /// The low four bits of the opcode of `jcc`.
    type Cond = u8;

    fn condition_direct(_sink: &mut InstructionSink<'_>, _flags: Flags, _cond: u8) -> bool {
        false
    }

    fn condition_from_flags<E: Emitter<Self> + ?Sized>(e: &mut E, cond: u8) {
        crate::flags::condition_from_flags(e, cond);
    }

    const FLAGS_WANTED: i64 = 1 << 62;

    type Control = Control;

    fn control_ty(_c: Control) -> ValType {
        ValType::I64
    }

    fn control_offset(_c: Control) -> usize {
        RFLAGS
    }

    /// No floating point yet.
    fn float_slow<E: Emitter<Self> + ?Sized>(e: &mut E) {
        e.sink().i32_const(0);
    }

    const FLOAT_CONTROLS: &'static [Control] = &[];

    type Hook = Hook;
    const HOOKS: &'static [HookType] = &[];

    fn hook_index(h: Hook) -> usize {
        match h {}
    }

    fn run_hook(hook: usize, _args: &[i64], _state: &mut [u8]) -> HookDone {
        unreachable!("x86-64 has no hook {hook}")
    }

    type Trap = Trap;

    fn trap_kind(t: Trap) -> (TrapKind, TrapDetail, TrapPc) {
        match t {
            Trap::Undefined => (TrapKind::Undefined, TrapDetail::Instruction, TrapPc::At),
            Trap::Unsupported => (TrapKind::Unsupported, TrapDetail::Instruction, TrapPc::At),
            Trap::GeneralProtection => (
                TrapKind::GeneralProtection,
                TrapDetail::Const(0),
                TrapPc::At,
            ),
            Trap::DivideError => (TrapKind::DivideError, TrapDetail::Const(0), TrapPc::At),
            Trap::Int3 => (TrapKind::Breakpoint, TrapDetail::Const(0), TrapPc::After),
            Trap::Int1 => (TrapKind::Breakpoint, TrapDetail::Const(1), TrapPc::After),
            Trap::Overflow => (TrapKind::Overflow, TrapDetail::Const(0), TrapPc::After),
        }
    }

    type Reason = Reason;
    const REASONS: &'static [(Reason, &'static str)] = &[
        (
            Reason::AvxNotOffered,
            "an AVX instruction was reached while AVX is not offered",
        ),
        (
            Reason::ThirtyTwoBitSystemCall,
            "32-bit system call from 64-bit code, not provided",
        ),
    ];

    type Host = ();

    /// Nothing x86 does yet depends on the host's engine.
    fn probe_module() -> Vec<u8> {
        use wasm_encoder::*;
        let mut module = Module::new();
        let mut types = TypeSection::new();
        types.ty().function([], [ValType::I32]);
        let mut functions = FunctionSection::new();
        functions.function(0);
        let mut exports = ExportSection::new();
        exports.export("probe", ExportKind::Func, 0);
        let mut code = CodeSection::new();
        let mut f = Function::new([]);
        f.instructions().i32_const(0).end();
        code.function(&f);
        module
            .section(&types)
            .section(&functions)
            .section(&exports)
            .section(&code);
        module.finish()
    }

    fn host_from_probe(_answer: i32) {}

    fn host_key(_host: ()) -> u32 {
        0
    }

    type Offer = Offer;

    fn offer_key(offer: Offer) -> u32 {
        offer.avx.into()
    }

    /// An access whose first or last byte is not canonical (bits 63 to 47 not all equal) is a stack fault
    /// through the stack segment (base rsp or rbp, no `fs:` or `gs:`), else general protection; any other,
    /// the kernel's half included, is a page fault.
    fn memory_fault(address: u64, code: &[u8], block: &X86Block) -> MemoryFault {
        let Some((i, _)) = crate::decode::decode(code) else {
            return MemoryFault::Page;
        };
        let value = |r: u8| match r {
            FS => block.fs_base,
            GS => block.gs_base,
            r => block.gpr[usize::from(r)],
        };
        for access in Self::accesses(&i).into_iter().flatten() {
            let mut start = access.displacement as u64;
            match access.base {
                Some(Base::Register(r)) => start = start.wrapping_add(value(r)),
                Some(Base::Pc) => start = start.wrapping_add(block.rip),
                None => {}
            }
            if let Some(Index::Register { reg, shift, .. }) = access.index {
                start = start.wrapping_add(value(reg) << shift);
            }
            if access.address_bits == 32 {
                start &= 0xffff_ffff;
            }
            if let Some(segment) = access.segment {
                start = start.wrapping_add(value(segment));
            }
            let bytes = access.fixed_bytes().unwrap_or(1).into();
            let last = start.wrapping_add(bytes - 1);
            if address.wrapping_sub(start) >= bytes {
                continue;
            }
            let canonical = |x: u64| ((x << 16) as i64 >> 16) as u64 == x;
            // An access that wraps past the top of the address space is canonical at both ends.
            let canonical = canonical(start) && canonical(last);
            return if canonical {
                MemoryFault::Page
            } else if rsp_based(&access) {
                MemoryFault::Stack
            } else {
                MemoryFault::GeneralProtection
            };
        }
        MemoryFault::Page
    }

    fn emit<E: Emitter<Self> + ?Sized>(e: &mut E, i: &Instruction) {
        crate::meaning::emit(e, i);
    }

    fn flow(i: &Instruction) -> Flow<u8> {
        use Mnemonic::*;
        let ins = &i.iced;
        let plain = |falls_through| Flow {
            falls_through,
            target: None,
            kind: FlowKind::Plain,
        };
        if i.too_long || ins.is_invalid() || !crate::meaning::written(i) {
            return plain(!(i.too_long || ins.is_invalid()) && may_be_offered(i));
        }
        let through = || match ins.op_kind(0) {
            OpKind::Register => Through::Register(general(ins.op_register(0)).unwrap().number),
            _ => Through::Memory,
        };
        match ins.mnemonic() {
            Jmp if ins.op_kind(0) == OpKind::NearBranch64 => Flow {
                falls_through: false,
                target: Some(ins.near_branch64() as i64),
                kind: FlowKind::Plain,
            },
            Jmp => Flow {
                falls_through: false,
                target: None,
                kind: FlowKind::JumpThrough(through()),
            },
            Call if ins.op_kind(0) == OpKind::NearBranch64 => Flow {
                falls_through: true,
                target: Some(ins.near_branch64() as i64),
                kind: FlowKind::Call,
            },
            Call => Flow {
                falls_through: true,
                target: None,
                kind: FlowKind::CallThrough(through()),
            },
            Ret => Flow {
                falls_through: false,
                target: None,
                kind: FlowKind::Return,
            },
            Int3 | Int1 | Int | Ud0 | Ud1 | Ud2 => plain(false),
            m if jcc(m).is_some() => Flow {
                falls_through: true,
                target: Some(ins.near_branch64() as i64),
                kind: FlowKind::Plain,
            },
            _ => plain(true),
        }
    }

    fn accesses(i: &Instruction) -> [Option<MemoryAccess<u8>>; 2] {
        use Mnemonic::*;
        let ins = &i.iced;
        if i.too_long || ins.is_invalid() || !crate::meaning::written(i) {
            return [None, None];
        }
        let explicit = memory_use(i).map(|kind| operand_access(i, kind));
        if is_string(i) {
            let bytes = element_bytes(i);
            let m = ins.mnemonic();
            let string = |register, kind| MemoryAccess {
                segment: if register == RSI { segment(i) } else { None },
                address_bits: if i.address32 { 32 } else { 64 },
                ..through(register, 0, bytes, kind)
            };
            let source = matches!(
                m,
                Movsb
                    | Movsw
                    | Movsd
                    | Movsq
                    | Lodsb
                    | Lodsw
                    | Lodsd
                    | Lodsq
                    | Cmpsb
                    | Cmpsw
                    | Cmpsd
                    | Cmpsq
            )
            .then(|| string(RSI, AccessKind::Read));
            let destination = (!matches!(m, Lodsb | Lodsw | Lodsd | Lodsq)).then(|| {
                let writes = matches!(
                    m,
                    Movsb | Movsw | Movsd | Movsq | Stosb | Stosw | Stosd | Stosq
                );
                string(
                    RDI,
                    if writes {
                        AccessKind::Write
                    } else {
                        AccessKind::Read
                    },
                )
            });
            // cmps reads [rdi] before [rsi], as the reference CPU faults.
            let compares = matches!(m, Cmpsb | Cmpsw | Cmpsd | Cmpsq);
            return match source {
                Some(s) if compares => [destination, Some(s)],
                Some(s) => [Some(s), destination],
                None => [destination, None],
            };
        }
        match ins.mnemonic() {
            Push => {
                let bytes = stack_bytes(i);
                let push = through(RSP, -i64::from(bytes), bytes, AccessKind::Write);
                match explicit {
                    Some(m) => [Some(m), Some(push)],
                    None => [Some(push), None],
                }
            }
            Pop => {
                let bytes = stack_bytes(i);
                let pop = through(RSP, 0, bytes, AccessKind::Read);
                // The destination's address is worked out with the stack pointer moved.
                let destination = explicit.map(|mut m| {
                    if m.base == Some(Base::Register(RSP)) {
                        m.displacement += i64::from(bytes);
                    }
                    m
                });
                [Some(pop), destination]
            }
            Call => {
                let push = through(RSP, -8, 8, AccessKind::Write);
                match explicit {
                    Some(m) => [Some(m), Some(push)],
                    None => [Some(push), None],
                }
            }
            Ret => [Some(through(RSP, 0, 8, AccessKind::Read)), None],
            _ => [explicit, None],
        }
    }

    fn stack_move(i: &Instruction) -> StackMove {
        use Mnemonic::*;
        let ins = &i.iced;
        if i.too_long || ins.is_invalid() || !crate::meaning::written(i) {
            return StackMove::None;
        }
        let writes_rsp = ins.op_count() > 0
            && ins.op_kind(0) == OpKind::Register
            && general(ins.op_register(0)).is_some_and(|g| g.number == RSP);
        match ins.mnemonic() {
            Push => StackMove::By {
                delta: -i64::from(stack_bytes(i)),
                before_access: true,
            },
            Pop if writes_rsp => StackMove::Unknown,
            Pop => StackMove::By {
                delta: stack_bytes(i).into(),
                before_access: false,
            },
            Call => StackMove::By {
                delta: -8,
                before_access: true,
            },
            Add | Sub
                if writes_rsp
                    && general(ins.op_register(0)).is_some_and(|g| g.width == 64)
                    && ins.op_kind(1) != OpKind::Register =>
            {
                let k = ins.immediate(1) as i64;
                StackMove::By {
                    delta: if ins.mnemonic() == Add { k } else { -k },
                    before_access: false,
                }
            }
            Ret | Cmp | Test => StackMove::None,
            _ if writes_rsp => StackMove::Unknown,
            Xchg if ins.op_kind(1) == OpKind::Register
                && general(ins.op_register(1)).is_some_and(|g| g.number == RSP) =>
            {
                StackMove::Unknown
            }
            _ => StackMove::None,
        }
    }

    /// `lea r, [rip + d]`.
    fn built_address(
        at: u64,
        i: &Instruction,
        _blob: &[u8],
        _code: Range<u64>,
        _before: impl Fn(u64) -> Option<(u64, Instruction)>,
    ) -> Option<u64> {
        let ins = &i.iced;
        (ins.mnemonic() == Mnemonic::Lea && ins.memory_base() == Register::RIP)
            .then(|| at.wrapping_add(ins.memory_displacement64()))
    }

    /// Switch recognition comes with full look-ahead.
    fn jump_table(
        _at: u64,
        _reg: u8,
        _blob: &[u8],
        _code: Range<u64>,
        _before: impl Fn(u64) -> Option<(u64, Instruction)>,
    ) -> Option<Vec<u64>> {
        None
    }

    fn plain(i: &Instruction) -> bool {
        use Mnemonic::*;
        let ins = &i.iced;
        crate::meaning::written(i)
            && !i.too_long
            && memory_use(i).is_none()
            && !is_string(i)
            && (matches!(
                ins.mnemonic(),
                Add | Adc
                    | Sub
                    | Sbb
                    | Cmp
                    | And
                    | Or
                    | Xor
                    | Test
                    | Inc
                    | Dec
                    | Neg
                    | Not
                    | Mov
                    | Movzx
                    | Movsx
                    | Movsxd
                    | Lea
                    | Xchg
                    | Cbw
                    | Cwde
                    | Cdqe
                    | Cwd
                    | Cdq
                    | Cqo
                    | Shl
                    | Sal
                    | Shr
                    | Sar
                    | Rol
                    | Ror
                    | Rcl
                    | Rcr
                    | Nop
                    | Clc
                    | Stc
                    | Cmc
            ) || setcc(ins.mnemonic()).is_some()
                || cmovcc(ins.mnemonic()).is_some()
                || (ins.mnemonic() == Imul && ins.op_count() > 1))
    }

    /// Where the meaning fixes flags Intel leaves undefined to the reference CPU's: the multiplications, the
    /// divisions, the shifts and rotates (OF beyond a count of 1, AF, CF beyond the width), and the logic
    /// instructions' AF.
    fn may_vary(i: &Instruction) -> bool {
        use Mnemonic::*;
        matches!(
            i.iced.mnemonic(),
            Mul | Imul
                | Div
                | Idiv
                | Shl
                | Sal
                | Shr
                | Sar
                | Rol
                | Ror
                | Rcl
                | Rcr
                | And
                | Or
                | Xor
                | Test
        )
    }

    const RETURN_ADDRESS: ReturnAddress = ReturnAddress::Stack { bytes: 8 };
    const RED_ZONE: u64 = 128;
    const SP_ALIGNMENT_CHECKED: bool = false;

    fn data_address(sink: &mut InstructionSink<'_>, local: u32) {
        sink.local_get(local);
    }

    /// The meanings stop with general protection before handing a non-canonical target on, so the target is
    /// the address itself.
    fn branch_target<E: Emitter<Self> + ?Sized>(e: &mut E, local: u32) {
        e.sink().local_get(local);
    }

    const UNREACHABLE_RETURN: i64 = i64::MIN;
    const SYSCALL: SyscallRegisters<u8> = SyscallRegisters {
        number: RAX,
        args: [RDI, RSI, RDX, 10, 8, 9],
        result: RAX,
    };
    const CODE_CHANGES: CodeChanges = CodeChanges::Detected;

    /// `add`, `sub`, `inc` and `dec` of a 32- or 64-bit register by a constant.
    fn induction(i: &Instruction) -> Option<Induction<u8>> {
        use Mnemonic::*;
        let ins = &i.iced;
        if i.too_long || ins.is_invalid() || ins.op_kind(0) != OpKind::Register {
            return None;
        }
        let g = general(ins.op_register(0))?;
        if g.width < 32 {
            return None;
        }
        let (step, negate) = match ins.mnemonic() {
            Inc => (1, false),
            Dec => (1, true),
            Add | Sub
                if ins.op_count() == 2
                    && ins.op_kind(1) != OpKind::Register
                    && ins.op_kind(1) != OpKind::Memory =>
            {
                (ins.immediate(1) as i64, ins.mnemonic() == Sub)
            }
            _ => return None,
        };
        Some(Induction {
            reg: g.number,
            bits: g.width,
            step: Value::Imm(step),
            negate,
        })
    }

    /// The loop proofs' comparisons come with full look-ahead.
    fn branch_comparison(
        _path: &[(u64, Instruction, Effects<Self>)],
        _branch: (u64, &Instruction),
    ) -> Option<Comparison<u8>> {
        None
    }

    fn loaded_max(_i: &Instruction) -> Option<(u8, i64)> {
        None
    }
}

/// Whether an instruction with no meaning written yet may run on some offer, so control may fall through it.
fn may_be_offered(i: &Instruction) -> bool {
    use crate::claims::{GROUP_KERNEL, GROUP_NEVER};
    !matches!(
        crate::decode::claim_group(i.iced.code()),
        GROUP_KERNEL | GROUP_NEVER
    )
}
