//! ARM64 as the translator sees it: the [`Guest`] implementation.

use crate::exact::Operation;
use crate::instruction::*;
use crate::meaning::{
    Carry, Control, Emitter, Flag, Flags, Hook, Host, Trap, condition_direct, condition_from_flags,
    emit, flag_value, untag,
};
use crate::{Arm64, EXCLUSIVE_ADDRESS, EXCLUSIVE_BYTES, EXCLUSIVE_VALUE, FLOAT_TRACKING, habits};
use core::mem::offset_of;
use core::ops::Range;
use wasm_encoder::{InstructionSink, ValType};
use wcontainers_abi::RegisterBlock as B;
use wcontainers_guest_cpu::{
    AccessBytes, AccessKind, Base, Bit, Class, CodeChanges, Comparison, Effects, Extend, Flow,
    FlowKind, Guest, GuestId, HookBody, HookDone, HookType, Index, Induction, Locals, MemoryAccess,
    MemoryFault, Reason, Registers, Relation, ReturnAddress, StackMove, StateBytes,
    SyscallRegisters, Through, TrapDetail, TrapKind, TrapPc, Ty, Value, WriteBack,
};

/// The float hook's bytes of the thread state: FPSR and FPCR in the block, through FPSR tracking after it.
const FLOAT_STATE: StateBytes = {
    let at = offset_of!(B, fpsr);
    assert!(offset_of!(B, fpcr) > at && FLOAT_TRACKING > at);
    StateBytes {
        at: at as u16,
        len: (FLOAT_TRACKING + 4 - at) as u16,
    }
};

/// The slow paths of ARM64's own: none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoReason {}

/// How a flag setting that is kept lazily looks, so setters of one shape can share locals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Add {
        width: Width,
        carry: u8,
        subtract: bool,
    },
    Logic(Width),
}

/// Every shape a kept flag setting may have, in the order of their flags-word functions: a subtraction only
/// ever has a carry of one.
const SHAPES: [Shape; 10] = {
    let mut shapes = [Shape::Logic(Width::W32); 10];
    let widths = [Width::W32, Width::X64];
    let mut w = 0;
    while w < 2 {
        let width = widths[w];
        let mut carry = 0;
        while carry < 3 {
            shapes[5 * w + carry as usize] = Shape::Add {
                width,
                carry,
                subtract: false,
            };
            carry += 1;
        }
        shapes[5 * w + 3] = Shape::Add {
            width,
            carry: 1,
            subtract: true,
        };
        shapes[5 * w + 4] = Shape::Logic(width);
        w += 1;
    }
    shapes
};

/// x0 to x30 at bits 0 to 30, sp at 31, N, Z, C and V at 32 to 35, v0 to v31 at 64 to 95.
const fn general(bit: u32, offset: usize) -> Bit {
    Bit {
        bit,
        class: Class::General,
        ty: Ty::I64,
        offset: offset as u16,
    }
}

const BITS: [Bit; 68] = {
    let mut bits = [general(0, 0); 68];
    let mut n = 0;
    while n < 31 {
        bits[n] = general(n as u32, offset_of!(B, x) + 8 * n);
        n += 1;
    }
    bits[31] = general(31, offset_of!(B, sp));
    let mut f = 0;
    while f < 4 {
        bits[32 + f] = Bit {
            bit: 32 + f as u32,
            class: Class::Flag,
            ty: Ty::I32,
            offset: offset_of!(B, pstate) as u16,
        };
        f += 1;
    }
    let mut v = 0;
    while v < 32 {
        bits[36 + v] = Bit {
            bit: 64 + v as u32,
            class: Class::Vector,
            ty: Ty::V128,
            offset: (offset_of!(B, v) + 16 * v) as u16,
        };
        v += 1;
    }
    bits
};

/// The register bits of N, Z, C and V.
pub(crate) const FLAGS: Registers = Registers(0xf << 32);

/// What the effects of an instruction are for the analyses of this crate (switch tables): which registers
/// it reads and writes does not depend on the host.
pub(crate) fn effects(i: &Instruction) -> Effects<Arm64> {
    let host = Host {
        fused_multiply_add: true,
        arm_nans: true,
    };
    wcontainers_guest_cpu::effects::<Arm64>(i, host, ())
}

fn width_type(width: Width) -> ValType {
    match width {
        Width::W32 => ValType::I32,
        Width::X64 => ValType::I64,
    }
}

fn width_bits(width: Width) -> u8 {
    width.bits() as u8
}

/// Whether a whole instruction sits at this offset of the code.
fn inside(code: &Range<u64>, offset: u64) -> bool {
    offset.is_multiple_of(4) && code.contains(&offset) && code.end - offset >= 4
}

fn word(blob: &[u8], offset: u64) -> u32 {
    let at = offset as usize;
    u32::from_le_bytes(blob[at..at + 4].try_into().unwrap())
}

impl Guest for Arm64 {
    const ID: GuestId = GuestId::Arm64;
    const ALIGN: u64 = 4;
    const MAX_LENGTH: u64 = 4;
    type Instruction = Instruction;

    #[inline]
    fn decode(code: &[u8]) -> Option<(Instruction, u8)> {
        let word = u32::from_le_bytes(code.get(..4)?.try_into().unwrap());
        Some((crate::decode(word), 4))
    }

    #[inline]
    fn trap_detail(code: &[u8]) -> i64 {
        i64::from(u32::from_le_bytes(code[..4].try_into().unwrap()))
    }

    type Reg = Reg;
    type Vector = u8;
    const BITS: &'static [Bit] = &BITS;

    #[inline]
    fn reg_bit(r: Reg) -> Option<u32> {
        match r {
            Reg::X(n) => Some(n.into()),
            Reg::Sp => Some(31),
            Reg::Zero => None,
        }
    }

    #[inline]
    fn vector_bit(v: u8) -> u32 {
        64 + u32::from(v)
    }

    #[inline]
    fn bit_reg(bit: u32) -> Reg {
        match bit {
            0..=30 => Reg::X(bit as u8),
            31 => Reg::Sp,
            _ => panic!("bit {bit} is no general register"),
        }
    }

    const SP_BIT: u32 = 31;
    const PASSED_IN: &'static [u32] = &[0, 1, 31];
    const PASSED_OUT: &'static [u32] = &[0, 1, 31];

    type Flag = Flag;
    const FLAGS: &'static [(Flag, u32, u32)] = &[
        (Flag::N, 31, 32),
        (Flag::Z, 30, 33),
        (Flag::C, 29, 34),
        (Flag::V, 28, 35),
    ];
    const FLAGS_WORD_OFFSET: usize = offset_of!(B, pstate);
    type Flags = Flags;
    type Shape = Shape;
    const SHAPES: &'static [Shape] = &SHAPES;

    #[inline]
    fn shape(flags: Flags) -> Option<Shape> {
        match flags {
            Flags::Add {
                width,
                carry,
                subtrahend,
                ..
            } => Some(Shape::Add {
                width,
                carry: match carry {
                    Carry::Zero => 0,
                    Carry::One => 1,
                    Carry::Local(_) => 2,
                },
                subtract: subtrahend.is_some(),
            }),
            Flags::Logic { width, .. } => Some(Shape::Logic(width)),
            _ => None,
        }
    }

    /// An addition keeps `a` and `b` (or the value subtracted), and the carry when it is a local; logic
    /// keeps the result.
    fn shape_locals(shape: Shape) -> &'static [ValType] {
        use ValType::{I32, I64};
        match shape {
            Shape::Add {
                width: Width::W32,
                carry: 2,
                ..
            } => &[I32, I32, I32],
            Shape::Add {
                width: Width::X64,
                carry: 2,
                ..
            } => &[I64, I64, I32],
            Shape::Add {
                width: Width::W32, ..
            } => &[I32, I32],
            Shape::Add {
                width: Width::X64, ..
            } => &[I64, I64],
            Shape::Logic(Width::W32) => &[I32],
            Shape::Logic(Width::X64) => &[I64],
        }
    }

    fn with_locals(shape: Shape, locals: &[u32]) -> Flags {
        match shape {
            Shape::Add {
                width,
                carry,
                subtract,
            } => Flags::Add {
                width,
                a: locals[0],
                b: if subtract { u32::MAX } else { locals[1] },
                carry: match carry {
                    0 => Carry::Zero,
                    1 => Carry::One,
                    _ => Carry::Local(locals[2]),
                },
                result: u32::MAX,
                subtrahend: subtract.then_some(locals[1]),
            },
            Shape::Logic(width) => Flags::Logic {
                width,
                result: locals[0],
            },
        }
    }

    fn locals_of(flags: Flags) -> Locals {
        match flags {
            Flags::Add {
                a,
                b,
                carry: Carry::Local(c),
                subtrahend,
                ..
            } => Locals::new(&[a, subtrahend.unwrap_or(b), c]),
            Flags::Add {
                a, b, subtrahend, ..
            } => Locals::new(&[a, subtrahend.unwrap_or(b)]),
            Flags::Logic { result, .. } => Locals::new(&[result]),
            Flags::Constant(_) | Flags::Packed(_) => Locals::new(&[]),
        }
    }

    fn temporaries(flags: Flags) -> Option<ValType> {
        match flags {
            Flags::Add { width, .. } => Some(width_type(width)),
            _ => None,
        }
    }

    /// For an addition, `b` (when only the value subtracted was kept) and the result are worked out into
    /// the temporaries.
    fn complete(s: &mut InstructionSink<'_>, flags: Flags, temporaries: [u32; 2]) -> Flags {
        let Flags::Add {
            width,
            a,
            b,
            carry,
            subtrahend,
            ..
        } = flags
        else {
            return flags;
        };
        let [inverted, result] = temporaries;
        let b = match subtrahend {
            Some(m) => {
                s.local_get(m);
                match width {
                    Width::W32 => s.i32_const(-1).i32_xor(),
                    Width::X64 => s.i64_const(-1).i64_xor(),
                };
                s.local_set(inverted);
                inverted
            }
            None => b,
        };
        s.local_get(a).local_get(b);
        let add = |s: &mut InstructionSink<'_>| match width {
            Width::W32 => {
                s.i32_add();
            }
            Width::X64 => {
                s.i64_add();
            }
        };
        add(s);
        match carry {
            Carry::Zero => {}
            Carry::One => {
                match width {
                    Width::W32 => s.i32_const(1),
                    Width::X64 => s.i64_const(1),
                };
                add(s);
            }
            Carry::Local(c) => {
                s.local_get(c);
                if width == Width::X64 {
                    s.i64_extend_i32_u();
                }
                add(s);
            }
        }
        s.local_set(result);
        Flags::Add {
            width,
            a,
            b,
            carry,
            result,
            subtrahend,
        }
    }

    /// Every setter sets all four.
    #[inline]
    fn sets(_flags: Flags) -> Registers {
        FLAGS
    }

    fn flag_value(sink: &mut InstructionSink<'_>, flags: Flags, flag: Flag) {
        flag_value(sink, flags, flag);
    }

    type Cond = Cond;

    fn condition_direct(sink: &mut InstructionSink<'_>, flags: Flags, cond: Cond) -> bool {
        condition_direct(sink, flags, cond)
    }

    fn condition_from_flags<E: Emitter<Self> + ?Sized>(e: &mut E, cond: Cond) {
        condition_from_flags(e, cond);
    }

    /// Returns are 4-byte aligned, so bit 0 is free.
    const FLAGS_WANTED: i64 = 1;

    type Control = Control;

    #[inline]
    fn control_ty(c: Control) -> ValType {
        c.ty()
    }

    #[inline]
    fn control_offset(c: Control) -> usize {
        match c {
            Control::ThreadPointer => offset_of!(B, tpidr_el0),
            Control::Fpcr => offset_of!(B, fpcr),
            Control::Fpsr => offset_of!(B, fpsr),
            Control::FloatTracking => FLOAT_TRACKING,
            Control::ExclusiveBytes => EXCLUSIVE_BYTES,
            Control::ExclusiveAddress => EXCLUSIVE_ADDRESS,
            Control::ExclusiveLow => EXCLUSIVE_VALUE,
            Control::ExclusiveHigh => EXCLUSIVE_VALUE + 8,
        }
    }

    /// When FPCR is not zero or FPSR's flags are kept.
    fn float_slow<E: Emitter<Self> + ?Sized>(e: &mut E) {
        e.get_control(Control::Fpcr);
        e.get_control(Control::FloatTracking);
        e.sink().i32_or();
    }

    const FLOAT_CONTROLS: &'static [Control] = &[Control::Fpcr, Control::FloatTracking];

    type Hook = Hook;
    const HOOKS: &'static [HookType] = &[
        HookType {
            name: "float",
            params: &[ValType::I32, ValType::I64, ValType::I64, ValType::I64],
            results: &[ValType::I64],
            registers: Registers::NONE,
            state: FLOAT_STATE,
            body: HookBody::Cpu,
        },
        HookType {
            name: "clock",
            params: &[],
            results: &[ValType::I64],
            registers: Registers::NONE,
            state: StateBytes::NONE,
            body: HookBody::Clock,
        },
        HookType {
            name: "code_changed",
            params: &[ValType::I64],
            results: &[],
            registers: Registers::NONE,
            state: StateBytes::NONE,
            // `ic ivau` works on the 64-byte line.
            body: HookBody::CodeChanged { line: 64 },
        },
    ];

    #[inline]
    fn hook_index(h: Hook) -> usize {
        match h {
            Hook::Float => 0,
            Hook::Clock => 1,
            Hook::CodeChanged => 2,
        }
    }

    /// The float hook: one operation by [`crate::exact::run`] with the thread's FPCR, adding to its FPSR.
    /// Counted as "software fused multiply-add" when FPCR is zero and the flags are not kept (the host's
    /// engine does not fuse), else as "exact floating point requested".
    fn run_hook(hook: usize, args: &[i64], state: &mut [u8]) -> HookDone {
        assert_eq!(hook, 0, "the float hook is the only one of ARM64's own");
        let at = |offset: usize| offset - FLOAT_STATE.at as usize;
        let word = |state: &[u8], offset: usize| {
            u32::from_le_bytes(state[at(offset)..at(offset) + 4].try_into().unwrap())
        };
        let operation = args[0] as i32;
        let fpcr = word(state, offset_of!(B, fpcr));
        let mut fpsr = word(state, offset_of!(B, fpsr));
        let tracking = word(state, FLOAT_TRACKING);
        let (a, b, c) = (args[1] as u64, args[2] as u64, args[3] as u64);
        let result = crate::exact::run(operation, a, b, c, fpcr, &mut fpsr);
        state[at(offset_of!(B, fpsr))..][..4].copy_from_slice(&fpsr.to_le_bytes());
        let (kind, _) = Operation::decode(operation);
        let reason = if kind.fused() && fpcr == 0 && tracking == 0 {
            Reason::SoftwareFusedMultiplyAdd
        } else {
            Reason::ExactFloatingPointRequested
        };
        HookDone {
            results: [result as i64, 0],
            reason: Some(reason as u32),
        }
    }

    type Trap = Trap;

    fn trap_kind(t: Trap) -> (TrapKind, TrapDetail, TrapPc) {
        match t {
            Trap::Undefined => (TrapKind::Undefined, TrapDetail::Instruction, TrapPc::At),
            Trap::Unsupported => (TrapKind::Unsupported, TrapDetail::Instruction, TrapPc::At),
            Trap::Breakpoint(comment) => (
                TrapKind::Breakpoint,
                TrapDetail::Const(comment.into()),
                TrapPc::At,
            ),
            Trap::Alignment(address) => {
                (TrapKind::Alignment, TrapDetail::Local(address), TrapPc::At)
            }
        }
    }

    /// ARM64 has no slow path of its own beyond the translator's.
    type Reason = NoReason;
    const REASONS: &'static [(NoReason, &'static str)] = &[];

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

    type Offer = ();

    fn offer_key(_offer: ()) -> u32 {
        0
    }

    fn memory_fault(_address: u64, _code: &[u8], _block: &B) -> MemoryFault {
        MemoryFault::Page
    }

    fn emit<E: Emitter<Self> + ?Sized>(e: &mut E, i: &Instruction) {
        emit(e, i);
    }

    #[inline]
    fn flow(i: &Instruction) -> Flow<Reg> {
        let flow = i.flow();
        let kind = match *i {
            Instruction::Branch { link: true, .. } => FlowKind::Call,
            Instruction::BranchRegister { op, rn } => match op {
                BranchRegister::Br => FlowKind::JumpThrough(Through::Register(rn)),
                BranchRegister::Blr => FlowKind::CallThrough(Through::Register(rn)),
                BranchRegister::Ret => FlowKind::Return,
            },
            _ => FlowKind::Plain,
        };
        Flow {
            falls_through: flow.falls_through,
            target: flow.target,
            kind,
        }
    }

    #[inline]
    fn accesses(i: &Instruction) -> [Option<MemoryAccess<Reg>>; 2] {
        let access = i.memory_access().map(|a| {
            let (base, index, displacement, write_back) = match a.address {
                Address::Offset { base, offset } => {
                    (Base::Register(base), None, offset, WriteBack::None)
                }
                Address::PreIndex { base, offset } => {
                    (Base::Register(base), None, 0, WriteBack::Before(offset))
                }
                Address::PostIndex { base, offset } => {
                    (Base::Register(base), None, 0, WriteBack::After(offset))
                }
                Address::Indexed {
                    base,
                    index,
                    extend,
                    shift,
                } => {
                    let index = Index::Register {
                        reg: index,
                        shift,
                        extend: Extend {
                            from_bits: extend.from_bits,
                            signed: extend.signed,
                        },
                    };
                    (Base::Register(base), Some(index), 0, WriteBack::None)
                }
                Address::Literal { offset } => (Base::Pc, None, offset, WriteBack::None),
            };
            MemoryAccess {
                segment: None,
                base: Some(base),
                index,
                displacement,
                address_bits: 64,
                write_back,
                bytes: AccessBytes::Fixed(a.bytes as u16),
                kind: if a.write {
                    AccessKind::Write
                } else {
                    AccessKind::Read
                },
            }
        });
        [access, None]
    }

    /// `add` or `sub` of an immediate to sp, and accesses through sp that write it back.
    #[inline]
    fn stack_move(i: &Instruction) -> StackMove {
        match *i {
            Instruction::AddSub {
                width: Width::X64,
                subtract,
                set_flags: false,
                rd: Reg::Sp,
                rn: Reg::Sp,
                operand: Operand::Imm(k),
            } if k < 1 << 24 => StackMove::By {
                delta: if subtract { -(k as i64) } else { k as i64 },
                before_access: true,
            },
            _ => match i.memory_access().map(|a| a.address) {
                Some(Address::PreIndex {
                    base: Reg::Sp,
                    offset,
                }) => StackMove::By {
                    delta: offset,
                    before_access: true,
                },
                Some(Address::PostIndex {
                    base: Reg::Sp,
                    offset,
                }) => StackMove::By {
                    delta: offset,
                    before_access: false,
                },
                _ => StackMove::None,
            },
        }
    }

    /// `adr`, or `adrp` and an `add` in the word before it (read from the blob, whatever reached the add).
    fn built_address(
        at: u64,
        i: &Instruction,
        blob: &[u8],
        code: Range<u64>,
        _before: impl Fn(u64) -> Option<(u64, Instruction)>,
    ) -> Option<u64> {
        match *i {
            Instruction::PcRelative {
                page: false,
                offset: by,
                ..
            } => Some(at.wrapping_add_signed(by)),
            Instruction::AddSub {
                width: Width::X64,
                subtract: false,
                set_flags: false,
                rn,
                operand: Operand::Imm(value),
                ..
            } if at >= 4 && inside(&code, at - 4) => match crate::decode(word(blob, at - 4)) {
                Instruction::PcRelative {
                    rd,
                    page: true,
                    offset: by,
                } if rd == rn => Some(
                    ((at - 4) & !0xfff)
                        .wrapping_add_signed(by)
                        .wrapping_add(value),
                ),
                _ => None,
            },
            _ => None,
        }
    }

    /// The 64 words before the jump that lie in the code, read from the blob, and [`habits::jump_table`].
    fn jump_table(
        at: u64,
        reg: Reg,
        blob: &[u8],
        code: Range<u64>,
        _before: impl Fn(u64) -> Option<(u64, Instruction)>,
    ) -> Option<Vec<u64>> {
        let first = at.saturating_sub(4 * 64).max(code.start);
        let before: Vec<(u64, Instruction)> = (first..at)
            .step_by(4)
            .filter(|&o| inside(&code, o))
            .map(|o| (o, crate::decode(word(blob, o))))
            .collect();
        habits::jump_table(&before, reg, |offset, bytes| {
            let at = usize::try_from(offset).ok()?;
            let slice = blob.get(at..at.checked_add(bytes.into())?)?;
            Some(
                slice
                    .iter()
                    .rev()
                    .fold(0, |value, &b| value << 8 | u64::from(b)),
            )
        })
    }

    #[inline]
    fn plain(i: &Instruction) -> bool {
        use Instruction as I;
        matches!(
            i,
            I::AddSub { .. }
                | I::AddSubCarry { .. }
                | I::Logical { .. }
                | I::MoveWide { .. }
                | I::Bitfield { .. }
                | I::Extract { .. }
                | I::PcRelative { .. }
                | I::CondCompare { .. }
                | I::CondSelect { .. }
                | I::Unary { .. }
                | I::Binary { .. }
                | I::MulAdd { .. }
                | I::Nothing
        )
    }

    /// `bl` and `blr` leave the return address in x30.
    const RETURN_ADDRESS: ReturnAddress = ReturnAddress::Register;
    const RED_ZONE: u64 = 0;

    /// Linux checks sp's alignment when it is the base of an access.
    const SP_ALIGNMENT_CHECKED: bool = true;

    fn data_address(sink: &mut InstructionSink<'_>, local: u32) {
        untag(sink, local);
    }

    fn branch_target<E: Emitter<Self> + ?Sized>(e: &mut E, local: u32) {
        untag(&mut e.sink(), local);
    }

    /// Return targets are untagged, and an untagged address has a top byte of 0x00 or 0xff.
    const UNREACHABLE_RETURN: i64 = i64::MIN;

    /// Code changes are announced by `ic ivau`.
    const CODE_CHANGES: CodeChanges = CodeChanges::Announced;

    const SYSCALL: SyscallRegisters<Reg> = SyscallRegisters {
        number: Reg::X(8),
        args: [
            Reg::X(0),
            Reg::X(1),
            Reg::X(2),
            Reg::X(3),
            Reg::X(4),
            Reg::X(5),
        ],
        result: Reg::X(0),
    };

    /// `add` or `sub` of an immediate or of a register to a register in place, and the write back of a pre-
    /// or post-indexed access.
    #[inline]
    fn induction(i: &Instruction) -> Option<Induction<Reg>> {
        match *i {
            Instruction::AddSub {
                width,
                subtract,
                rd,
                rn,
                operand,
                ..
            } if rd == rn => {
                let step = match operand {
                    Operand::Imm(k) => Value::Imm(k as i64),
                    Operand::Shifted {
                        rm: rm @ Reg::X(_),
                        amount: 0,
                        ..
                    } => Value::Reg(rm),
                    _ => return None,
                };
                Some(Induction {
                    reg: rd,
                    bits: width_bits(width),
                    step,
                    negate: subtract,
                })
            }
            Instruction::AddSub { .. } => None,
            _ => match i.memory_access()?.address {
                Address::PreIndex { base, offset } | Address::PostIndex { base, offset } => {
                    Some(Induction {
                        reg: base,
                        bits: 64,
                        step: Value::Imm(offset),
                        negate: false,
                    })
                }
                _ => None,
            },
        }
    }

    /// `b.cond` after a `cmp` (or `subs`) of a register with an immediate or a register, and `cbz` and
    /// `cbnz`.
    fn branch_comparison(
        path: &[(u64, Instruction, Effects<Arm64>)],
        (at, branch): (u64, &Instruction),
    ) -> Option<Comparison<Reg>> {
        match *branch {
            Instruction::BranchCond { cond, .. } if cond.0 < 14 => {
                let (relation, signed) = match cond.0 {
                    0 => (Relation::Eq, false),
                    1 => (Relation::Ne, false),
                    2 => (Relation::Above { inclusive: true }, false),
                    3 => (Relation::Below { inclusive: false }, false),
                    8 => (Relation::Above { inclusive: false }, false),
                    9 => (Relation::Below { inclusive: true }, false),
                    10 => (Relation::Above { inclusive: true }, true),
                    11 => (Relation::Below { inclusive: false }, true),
                    12 => (Relation::Above { inclusive: false }, true),
                    13 => (Relation::Below { inclusive: true }, true),
                    _ => return None,
                };
                // The last instruction before the branch that sets flags must be a comparison.
                let &(setter, instruction, _) = path
                    .iter()
                    .rev()
                    .find(|(_, _, effects)| !effects.defs.intersect(FLAGS).is_empty())?;
                let Instruction::AddSub {
                    width,
                    subtract: true,
                    set_flags: true,
                    rn,
                    operand,
                    ..
                } = instruction
                else {
                    return None;
                };
                let right = match operand {
                    Operand::Imm(k) => Value::Imm(k as i64),
                    Operand::Shifted {
                        rm: rm @ Reg::X(_),
                        amount: 0,
                        ..
                    } => Value::Reg(rm),
                    _ => return None,
                };
                Some(Comparison {
                    left: rn,
                    right,
                    relation,
                    signed,
                    bits: width_bits(width),
                    at: setter,
                })
            }
            Instruction::CompareBranch {
                width, nonzero, rt, ..
            } => Some(Comparison {
                left: rt,
                right: Value::Imm(0),
                relation: if nonzero { Relation::Ne } else { Relation::Eq },
                signed: false,
                bits: width_bits(width),
                at,
            }),
            _ => None,
        }
    }

    #[inline]
    fn loaded_max(i: &Instruction) -> Option<(Reg, i64)> {
        match *i {
            Instruction::Load {
                bytes,
                signed: None,
                rt,
                ..
            } if bytes <= 4 => Some((rt, (1i64 << (8 * bytes)) - 1)),
            _ => None,
        }
    }
}
