//! [`X86_64`]'s `Guest`: the register file, flags, traps and the facts about each instruction the analyses
//! use.

use crate::decode::Instruction;
use crate::flags::{Condition, Flag, Flags, Shape};
use crate::host::Host;
use crate::meaning::is_string;
use crate::operand::*;
use crate::{
    FLAGS_WORD, FLOAT_EXCEPTION, FS_BASE, GS_BASE, MXCSR, MXCSR_AT_START, MXCSR_MODE,
    MXCSR_TRACKING, RFLAGS, X86_64, X86Block, XMM,
};
use core::ops::Range;
use iced_x86::{Mnemonic, OpKind, Register};
use wasm_encoder::{InstructionSink, ValType};
use wcontainers_guest_cpu::*;

/// Registers kept only in the thread's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The rflags bits user code keeps outside the six flags (TF, DF, NT and ID), at their positions.
    Rflags,
    /// The selectors of ds, es, fs and gs, 16 bits each in `crate::SegmentRegister`'s order, as an i64.
    Selectors,
    /// i32s: MXCSR, whether its flags are kept, and the float hook's unmasked exception.
    Mxcsr,
    MxcsrTracking,
    FloatException,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// `() -> i64`: the clock in nanoseconds, which `rdtsc` and `rdtscp` give as a 1 GHz counter.
    Clock,
    /// `(operation, a low, a high, b low, b high) -> (low, high)`: one SIMD floating point instruction on
    /// the exact path (`crate::exact`).
    Float,
}

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
    /// An unmasked SIMD floating point exception, with MXCSR's raised unmasked flags in this i64 local.
    FloatException(u32),
}

/// x86's own slow paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    AvxNotOffered,
    AvxAskedAndToldNo,
    ThirtyTwoBitSystemCall,
    StringCompared,
    StringLoaded,
    StringAddress32,
    StringDirectionDown,
    StringTrapFlag,
    StringOverlap,
    StringRangeNotAllowed,
    TrapFlagNotStepped,
    AlignmentCheckNotProvided,
    MxcsrFlagsReadNotKept,
    FarTransferTo32Bit,
}

/// The thread state the float hook works on: MXCSR in the block to the end of the private words.
const FLOAT_STATE: StateBytes = StateBytes {
    at: MXCSR as u16,
    len: (FLOAT_EXCEPTION + size_of::<u32>() - MXCSR) as u16,
};

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

/// Every xmm register's bit: the float hook reads its destination from the block.
const XMM_COUNT: u32 = crate::Fxsave::ZERO.xmm.len() as u32;
const XMM_REGISTERS: Registers = Registers(((1 << XMM_COUNT) - 1) << FIRST_XMM);

/// Every flag's register bit.
pub(crate) const ALL_FLAGS: Registers = Registers(0x3f << FIRST_FLAG);

/// An access through the stack segment: base rsp or rbp, no `fs:` or `gs:`.
fn rsp_based(access: &MemoryAccess<u8>) -> bool {
    access.segment.is_none()
        && matches!(
            access.base,
            Some(Base::Register(RSP)) | Some(Base::Register(RBP))
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
    const PASSED_IN: &'static [u32] = &[RSP as u32, RDI as u32, RSI as u32];
    const PASSED_OUT: &'static [u32] = &[RSP as u32, RAX as u32];

    type Flag = Flag;
    const FLAGS: &'static [(Flag, u32, u32)] = &[
        (Flag::C, Flag::C.bit(), FIRST_FLAG + Flag::C.index() as u32),
        (Flag::P, Flag::P.bit(), FIRST_FLAG + Flag::P.index() as u32),
        (Flag::A, Flag::A.bit(), FIRST_FLAG + Flag::A.index() as u32),
        (Flag::Z, Flag::Z.bit(), FIRST_FLAG + Flag::Z.index() as u32),
        (Flag::S, Flag::S.bit(), FIRST_FLAG + Flag::S.index() as u32),
        (Flag::O, Flag::O.bit(), FIRST_FLAG + Flag::O.index() as u32),
    ];
    const FLAGS_WORD_OFFSET: usize = FLAGS_WORD;
    type Flags = Flags;
    type Shape = Shape;
    const SHAPES: &'static [Shape] = &crate::flags::SHAPES;

    fn shape(flags: Flags) -> Option<Shape> {
        crate::flags::shape(flags)
    }

    fn shape_locals(shape: Shape) -> &'static [ValType] {
        crate::flags::shape_locals(shape)
    }

    fn with_locals(shape: Shape, locals: &[u32]) -> Flags {
        crate::flags::with_locals(shape, locals)
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
            Flags::Shift {
                result,
                carry,
                overflow,
                unmoved,
                ..
            } => {
                let mut locals = vec![result, carry, overflow];
                if let Some(u) = unmoved {
                    locals.push(u.count);
                    locals.extend(u.kept);
                }
                Locals::new(&locals)
            }
            Flags::Each(each) => Locals::new(&each),
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

    type Cond = Condition;

    fn condition_direct(sink: &mut InstructionSink<'_>, flags: Flags, cond: Condition) -> bool {
        crate::flags::condition_direct(sink, flags, cond)
    }

    fn condition_from_flags<E: Emitter<Self> + ?Sized>(e: &mut E, cond: Condition) {
        crate::flags::condition_from_flags(e, cond);
    }

    const FLAGS_WANTED: i64 = 1 << 62;

    type Control = Control;

    fn control_ty(c: Control) -> ValType {
        match c {
            Control::Rflags | Control::Selectors => ValType::I64,
            _ => ValType::I32,
        }
    }

    fn control_offset(c: Control) -> usize {
        match c {
            Control::Rflags => RFLAGS,
            Control::Selectors => crate::SELECTORS,
            Control::Mxcsr => MXCSR,
            Control::MxcsrTracking => MXCSR_TRACKING,
            Control::FloatException => FLOAT_EXCEPTION,
        }
    }

    /// The exact path when MXCSR's mode is not the default or its flags are kept.
    fn float_slow<E: Emitter<Self> + ?Sized>(e: &mut E) {
        e.get_control(Control::Mxcsr);
        e.sink()
            .i32_const(MXCSR_MODE as i32)
            .i32_and()
            .i32_const((MXCSR_AT_START & MXCSR_MODE) as i32)
            .i32_xor();
        e.get_control(Control::MxcsrTracking);
        e.sink().i32_or();
    }

    const FLOAT_CONTROLS: &'static [Control] = &[Control::Mxcsr, Control::MxcsrTracking];

    type Hook = Hook;
    const HOOKS: &'static [HookType] = &[
        HookType {
            name: "clock",
            params: &[],
            results: &[ValType::I64],
            registers: Registers::NONE,
            state: StateBytes::NONE,
            body: HookBody::Clock,
        },
        HookType {
            name: "float",
            params: &[ValType::I32, ValType::I64, ValType::I64],
            results: &[ValType::I64, ValType::I64],
            registers: XMM_REGISTERS,
            state: FLOAT_STATE,
            body: HookBody::Cpu,
        },
    ];

    fn hook_index(h: Hook) -> usize {
        match h {
            Hook::Clock => 0,
            Hook::Float => 1,
        }
    }

    fn run_hook(hook: usize, args: &[i64], state: &mut [u8]) -> HookDone {
        assert_eq!(
            hook,
            Self::hook_index(Hook::Float),
            "the float hook is x86's own"
        );
        let word = |offset: usize| {
            offset - usize::from(FLOAT_STATE.at)
                ..offset - usize::from(FLOAT_STATE.at) + size_of::<u32>()
        };
        let mut mxcsr = u32::from_le_bytes(state[word(MXCSR)].try_into().unwrap());
        let wide =
            |low: i64, high: i64| u128::from(low as u64) | u128::from(high as u64) << u64::BITS;
        let code = args[0] as i32;
        let register = word(XMM + usize::from(crate::exact::destination(code)) * size_of::<u128>());
        let a = &state[register.start..register.start + size_of::<u128>()];
        let a = u128::from_le_bytes(a.try_into().unwrap());
        let (result, exception) = crate::exact::run(code, a, wide(args[1], args[2]), &mut mxcsr);
        state[word(MXCSR)].copy_from_slice(&mxcsr.to_le_bytes());
        state[word(FLOAT_EXCEPTION)].copy_from_slice(&exception.to_le_bytes());
        HookDone {
            results: [result as u64 as i64, (result >> u64::BITS) as u64 as i64],
            reason: Some(wcontainers_guest_cpu::Reason::ExactFloatingPointRequested as u32),
        }
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
            Trap::FloatException(code) => (
                TrapKind::FloatException,
                TrapDetail::Local(code),
                TrapPc::At,
            ),
        }
    }

    type Reason = Reason;
    const REASONS: &'static [(Reason, &'static str)] = &[
        (
            Reason::AvxNotOffered,
            "an AVX instruction was reached while AVX is not offered",
        ),
        (
            Reason::AvxAskedAndToldNo,
            "a program asked whether AVX is available and was told no",
        ),
        (
            Reason::ThirtyTwoBitSystemCall,
            "32-bit system call from 64-bit code, not provided",
        ),
        (
            Reason::StringCompared,
            "rep cmps or scas, one element at a time: they stop on data",
        ),
        (Reason::StringLoaded, "rep lods, one element at a time"),
        (
            Reason::StringAddress32,
            "rep string instruction with 32-bit addresses, one element at a time",
        ),
        (
            Reason::StringDirectionDown,
            "rep movs or stos with the direction flag set, one element at a time",
        ),
        (
            Reason::StringTrapFlag,
            "rep movs or stos with the trap flag set, one element at a time",
        ),
        (
            Reason::StringOverlap,
            "rep movs onto a destination inside its source, one element at a time",
        ),
        (
            Reason::StringRangeNotAllowed,
            "rep movs or stos over memory not all accessible, one element at a time",
        ),
        (
            Reason::TrapFlagNotStepped,
            "popf set the trap flag: single steps are not taken",
        ),
        (
            Reason::AlignmentCheckNotProvided,
            "alignment checking asked for, not provided",
        ),
        (
            Reason::MxcsrFlagsReadNotKept,
            "MXCSR's exception flags read while not kept",
        ),
        (
            Reason::FarTransferTo32Bit,
            "far transfer to 32-bit code, not provided",
        ),
    ];

    type Host = Host;

    fn probe_module() -> Vec<u8> {
        crate::host::probe_module()
    }

    fn host_from_probe(answer: i32) -> Host {
        Host::from_probe(answer)
    }

    fn host_key(host: Host) -> u32 {
        host.answer() as u32
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
        // The slots of a far return's frame fault one by one, each through the stack segment.
        if let Some(slots) = crate::segments::frame_slots(&i) {
            for (by, bytes) in slots {
                let start = value(RSP).wrapping_add_signed(by);
                if address.wrapping_sub(start) < bytes.into() {
                    let last = start.wrapping_add(u64::from(bytes) - 1);
                    return if crate::is_canonical(start) && crate::is_canonical(last) {
                        MemoryFault::Page
                    } else {
                        MemoryFault::Stack
                    };
                }
            }
        }
        for access in Self::accesses(&i).into_iter().flatten() {
            let mut start = access.displacement as u64;
            match access.base {
                Some(Base::Register(r)) => start = start.wrapping_add(value(r)),
                Some(Base::Pc) => start = start.wrapping_add(block.rip),
                None => {}
            }
            match access.index {
                Some(Index::Register { reg, shift, extend }) => {
                    // Only the index's own bits (`xlat`'s al), extended as the description says.
                    let unused = u64::BITS - u32::from(extend.from_bits);
                    let moved = value(reg) << unused;
                    let index = if extend.signed {
                        ((moved as i64) >> unused) as u64
                    } else {
                        moved >> unused
                    };
                    start = start.wrapping_add(index << shift);
                }
                Some(Index::BitOffset {
                    reg,
                    width,
                    operand,
                }) => {
                    if let Some((index, shift)) = operand {
                        start = start.wrapping_add(value(index) << shift);
                    }
                    let unused = u64::BITS - u32::from(width);
                    let offset =
                        ((value(reg) << unused) as i64 >> unused) >> width.trailing_zeros();
                    start = start.wrapping_add((offset << (width / W8).trailing_zeros()) as u64);
                }
                _ => {}
            }
            if access.address_bits == W32 {
                start &= u64::from(u32::MAX);
            }
            if let Some(segment) = access.segment {
                start = start.wrapping_add(value(segment));
            }
            let mut bytes: u64 = access.fixed_bytes().unwrap_or(1).into();
            if address.wrapping_sub(start) >= bytes {
                continue;
            }
            // A far pointer's offset and selector are two reads, each faulting by itself.
            if crate::segments::reads_pointer(&i) && access.kind == AccessKind::Read {
                for (by, piece) in crate::segments::pointer_pieces(&i) {
                    let from = start.wrapping_add(by.into());
                    if address.wrapping_sub(from) < piece.into() {
                        (start, bytes) = (from, piece.into());
                    }
                }
            }
            let last = start.wrapping_add(bytes - 1);
            // An access that wraps past the top of the address space is canonical at both ends.
            let canonical = crate::is_canonical(start) && crate::is_canonical(last);
            return if canonical {
                MemoryFault::Page
            } else if rsp_based(&access) {
                MemoryFault::Stack
            } else {
                MemoryFault::GeneralProtection
            };
        }
        // Every access of `enter` is through the stack segment, the frame pointers it copies included.
        if i.iced.mnemonic() == Mnemonic::Enter && !crate::is_canonical(address) {
            return MemoryFault::Stack;
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
        if is_string(i) && (ins.has_rep_prefix() || ins.has_repne_prefix()) {
            // Each pass goes back to the instruction while elements remain.
            return Flow {
                falls_through: true,
                target: Some(0),
                kind: FlowKind::Plain,
            };
        }
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
            Ret | Retf | Iret | Iretd | Iretq => Flow {
                falls_through: false,
                target: None,
                kind: FlowKind::Return,
            },
            Int3 | Int1 | Int | Sysenter | Ud0 | Ud1 | Ud2 => plain(false),
            m if jcc(m).is_some() || matches!(m, Jrcxz | Jecxz | Loop | Loope | Loopne) => Flow {
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
                address_bits: if i.address32 { W32 } else { W64 },
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
            Push | Pushf | Pushfq => {
                let bytes = stack_bytes(i);
                let push = through(RSP, -i64::from(bytes), bytes, AccessKind::Write);
                match explicit {
                    Some(m) => [Some(m), Some(push)],
                    None => [Some(push), None],
                }
            }
            Leave => [
                Some(through(RBP, 0, stack_bytes(i), AccessKind::Read)),
                None,
            ],
            Popf | Popfq => [
                Some(through(RSP, 0, stack_bytes(i), AccessKind::Read)),
                None,
            ],
            Pop => {
                // A segment register's pop reads the selector's 16 bits alone.
                let bytes = if crate::segments::written(i) {
                    size_of::<u16>() as u32
                } else {
                    stack_bytes(i)
                };
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
            Call if crate::segments::is_far(i) => {
                let bytes = crate::segments::far_call_push(i);
                [
                    explicit,
                    Some(through(RSP, -i64::from(bytes), bytes, AccessKind::Write)),
                ]
            }
            // Each slot is checked by itself, in the reference's order (`frame_slots`); this covers them all.
            Retf | Iret | Iretd | Iretq => {
                let slots = crate::segments::frame_slots(i).expect("a frame");
                let end = slots.iter().map(|&(by, bytes)| by + i64::from(bytes)).max();
                [
                    Some(through(RSP, 0, end.unwrap_or(0) as u32, AccessKind::Read)),
                    None,
                ]
            }
            Call => {
                let push = through(RSP, -i64::from(SLOT), SLOT, AccessKind::Write);
                match explicit {
                    Some(m) => [Some(m), Some(push)],
                    None => [Some(push), None],
                }
            }
            Ret => [Some(through(RSP, 0, SLOT, AccessKind::Read)), None],
            // The push of rbp; the frame pointers it copies are checked as they are made.
            Enter => {
                let bytes = stack_bytes(i);
                [
                    Some(through(RSP, -i64::from(bytes), bytes, AccessKind::Write)),
                    None,
                ]
            }
            // The upper half, then the lower, each wrapped by itself under an address-size prefix.
            Maskmovdqu => {
                let half = crate::XMM_BYTES / 2;
                [half, 0].map(|by| {
                    Some(MemoryAccess {
                        segment: segment(i),
                        address_bits: if i.address32 { W32 } else { W64 },
                        ..through(RDI, by.into(), half, AccessKind::Write)
                    })
                })
            }
            Bt | Bts | Btr | Btc if ins.op_kind(1) == OpKind::Register => [
                explicit.map(|m| MemoryAccess {
                    index: Some(Index::BitOffset {
                        reg: general(ins.op_register(1)).unwrap().number,
                        width: memory_bytes(i) as u8 * W8,
                        operand: match m.index {
                            Some(Index::Register { reg, shift, .. }) => Some((reg, shift)),
                            _ => None,
                        },
                    }),
                    ..m
                }),
                None,
            ],
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
            Push | Pushf | Pushfq => StackMove::By {
                delta: -i64::from(stack_bytes(i)),
                before_access: true,
            },
            Pop if writes_rsp => StackMove::Unknown,
            Leave | Enter | Retf | Iret | Iretd | Iretq => StackMove::Unknown,
            Call if crate::segments::is_far(i) => StackMove::Unknown,
            Pop | Popf | Popfq => StackMove::By {
                delta: stack_bytes(i).into(),
                before_access: false,
            },
            Call => StackMove::By {
                delta: -i64::from(SLOT),
                before_access: true,
            },
            Add | Sub
                if writes_rsp
                    && general(ins.op_register(0)).is_some_and(|g| g.width == W64)
                    && !matches!(ins.op_kind(1), OpKind::Register | OpKind::Memory) =>
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

    /// gcc's and clang's position-independent switch: `cmp i, n; ja default` guarding `lea t, [rip + T]`,
    /// `movsxd x, dword [t + i * 4]` and `add` of the two, then the jump through the sum; the targets are `T`
    /// plus each entry.
    fn jump_table(
        at: u64,
        reg: u8,
        blob: &[u8],
        _code: Range<u64>,
        before: impl Fn(u64) -> Option<(u64, Instruction)>,
    ) -> Option<Vec<u64>> {
        crate::habits::jump_table(at, reg, blob, before)
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
                    | Bsf
                    | Bsr
                    | Tzcnt
                    | Lzcnt
                    | Popcnt
                    | Bswap
                    | Shld
                    | Shrd
                    | Adcx
                    | Adox
                    | Lahf
                    | Sahf
                    | Andn
                    | Bextr
                    | Blsi
                    | Blsmsk
                    | Blsr
            ) || setcc(ins.mnemonic()).is_some()
                || cmovcc(ins.mnemonic()).is_some()
                || (ins.mnemonic() == Imul && ins.op_count() > 1))
    }

    /// Where the meaning fixes flags Intel leaves undefined to the reference CPU's (the multiplications, the
    /// divisions, the shifts and rotates, the logic instructions' AF, the bit tests' OF, SF, AF and PF, BMI1's AF and PF), answers
    /// by design (`int 0x80` and `sysenter`, which Linux runs as 32-bit system calls, give general protection),
    /// or stops where a CPU with more features runs on: an encoding iced calls invalid, every instruction
    /// outside the default claims, and one of them whose meaning is not written yet. Kernel-only instructions
    /// of the claimed features fault on every CPU under every kernel, but those a kernel can grant to user
    /// code; an instruction over 15 bytes faults on every CPU.
    fn may_vary(i: &Instruction) -> bool {
        use crate::claims::{GROUP_DEFAULT, GROUP_KERNEL};
        use Mnemonic::*;
        let ins = &i.iced;
        if i.too_long {
            return false;
        }
        if ins.is_invalid() {
            return true;
        }
        match crate::decode::claim_group(ins.code()) {
            // A kernel may let user code run these: rdpmc under CR4.PCE, port I/O and cli and sti after
            // iopl or ioperm.
            GROUP_KERNEL => {
                return matches!(
                    ins.mnemonic(),
                    Rdpmc | In | Out | Insb | Insw | Insd | Outsb | Outsw | Outsd | Cli | Sti
                );
            }
            GROUP_DEFAULT if crate::meaning::written(i) => {}
            // Illegal instruction, where a CPU with the feature (or our own later meaning) runs it.
            _ => return true,
        }
        match ins.mnemonic() {
            Int => ins.immediate8() == crate::LINUX_32_BIT_SYSTEM_CALL,
            // Answered by design: the clock, not the CPU's own counter; the claims and designed leaves, not the
            // CPU's; AC held clear and TF not stepped.
            Rdtsc | Rdtscp | Cpuid | Popf | Popfq => true,
            // Intel's approximations are its own (plan 5.8).
            m if crate::float::estimates(m) => true,
            // Where an unmasked exception stops a dot product part way, the flags raised follow the
            // reference's micro-operations; maskmovdqu with an all-zero mask may fault or not (Intel's
            // manual: implementation dependent); ldmxcsr refuses MXCSR's bit 17, which AMD's misaligned
            // SSE mode takes.
            Dpps | Dppd | Maskmovdqu | Ldmxcsr => true,
            // Intel ignores an operand-size prefix on a near branch; AMD takes it as 16 bits, with another
            // length, a truncated target and a 2-byte return address.
            Jmp | Call | Ret | Jrcxz | Jecxz | Loop | Loope | Loopne if i.operand_size_prefix => {
                true
            }
            m if jcc(m).is_some() => i.operand_size_prefix,
            // AMD reads no ModRM after `0F FF`, so the length, and a fault past a page or past 15 bytes,
            // differ.
            Ud0 => true,
            m if crate::descriptors::may_vary(m) => true,
            _ if crate::segments::may_vary(i) => true,
            // Intel leaves the result of 16 bits undefined.
            Bswap => crate::meaning::width(i, 0) == crate::operand::W16,
            m => matches!(
                m,
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
                    | Bt
                    | Bts
                    | Btr
                    | Btc
                    | Bsf
                    | Bsr
                    | Tzcnt
                    | Lzcnt
                    | Shld
                    | Shrd
                    | Andn
                    | Bextr
                    | Blsi
                    | Blsmsk
                    | Blsr
                    | Sysenter
            ),
        }
    }

    const RETURN_ADDRESS: ReturnAddress = ReturnAddress::Stack { bytes: SLOT as u8 };
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
        args: [RDI, RSI, RDX, R10, R8, R9],
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
        if g.width < W32 {
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

    /// A `jcc` after `cmp` or `sub` of a register with a register or an immediate, after `test r, r` (a
    /// comparison with 0), or `je` and `jne` after `dec`.
    fn branch_comparison(
        path: &[(u64, Instruction, Effects<Self>)],
        (_, branch): (u64, &Instruction),
    ) -> Option<Comparison<u8>> {
        crate::habits::branch_comparison(path, branch)
    }

    /// `movzx` of a byte or a word, and `mov` of a double word into a 32-bit register.
    fn loaded_max(i: &Instruction) -> Option<(u8, i64)> {
        crate::habits::loaded_max(i)
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
