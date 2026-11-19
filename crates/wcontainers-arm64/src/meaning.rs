//! The meaning of every instruction, written once as the WebAssembly it becomes.
//!
//! A meaning talks to an [`Emitter`], which decides where registers and flags live and how control leaves.
//! Each look-ahead is one emitter; none of them carries instruction logic of its own.

use crate::instruction::*;
use wasm_encoder::{BlockType, InstructionSink, MemArg, MemArgTarget, ValType};

/// What a memory access does, for the permission check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

/// Where control goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// This many bytes from the current instruction.
    Relative(i64),
    /// The guest address in this i64 local.
    Address(u32),
}

/// Why an instruction stops the thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trap {
    Undefined,
    Unsupported,
    Breakpoint(u16),
    /// An alignment fault at the address in this i64 local.
    Alignment(u32),
}

/// A register of the thread that is never kept in a local: it lives in the thread's state, where the hooks
/// and the runner can read and write it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// `TPIDR_EL0`, i64.
    ThreadPointer,
    /// FPCR, i32, only the bits of [`FPCR_BITS`].
    Fpcr,
    /// FPSR, i32, only the bits of [`FPSR_BITS`].
    Fpsr,
    /// i32: not zero once the program has written FPSR, so the cumulative exception flags must be kept
    /// exact from then on.
    FloatTracking,
    /// i32: the size in bytes of the access a load exclusive remembered, zero when nothing is remembered.
    ExclusiveBytes,
    /// i64: the address it remembered.
    ExclusiveAddress,
    /// i64 each: the value it loaded, low then high (for pairs of 64 bit registers).
    ExclusiveLow,
    ExclusiveHigh,
}

impl Control {
    pub fn ty(self) -> ValType {
        match self {
            Control::Fpcr | Control::Fpsr | Control::FloatTracking | Control::ExclusiveBytes => {
                ValType::I32
            }
            _ => ValType::I64,
        }
    }
}

/// A host function a meaning calls directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// `(operation: i32, a: i64, b: i64, c: i64) -> i64`: one floating point operation done in software
    /// (see [`crate::exact`]), reading FPCR and adding to FPSR in the thread's state.
    Float,
    /// `() -> i64`: the monotonic clock in nanoseconds.
    Clock,
    /// `(address: i64)`: the code at the address may have changed.
    CodeChanged,
}

/// What the host's engine does where WebAssembly leaves the result open, found by [`crate::host::probe`].
/// It changes what is translated, so it is part of a translation's cache key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Host {
    /// The relaxed fused multiply-add really fuses.
    pub fused_multiply_add: bool,
    /// Not-a-number results already follow Arm's rules.
    pub arm_nans: bool,
}

/// One of the four condition flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    N,
    Z,
    C,
    V,
}

impl Flag {
    pub const ALL: [Flag; 4] = [Flag::N, Flag::Z, Flag::C, Flag::V];

    /// Bit position in the packed `NZCV` value (and in Linux's saved `pstate`).
    pub fn bit(self) -> u32 {
        match self {
            Flag::N => 31,
            Flag::Z => 30,
            Flag::C => 29,
            Flag::V => 28,
        }
    }
}

/// The carry into an addition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
    Zero,
    One,
    /// An i32 local holding 0 or 1.
    Local(u32),
}

/// How an instruction sets the flags. Locals hold values of the width's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flags {
    /// `result = a + b + carry`; subtraction arrives with `b` inverted and a carry of one.
    Add {
        width: Width,
        a: u32,
        b: u32,
        carry: Carry,
        result: u32,
        /// For a plain subtraction (`b` inverted, carry one): the local holding the value subtracted, so a
        /// condition can compare `a` with it directly.
        subtrahend: Option<u32>,
    },
    /// N and Z from the result, C and V clear.
    Logic { width: Width, result: u32 },
    /// The four bits of an immediate, N in bit 3.
    Constant(u8),
    /// An i64 local holding flags at their `NZCV` positions.
    Packed(u32),
}

/// Where a meaning's registers, flags and control flow go. Each look-ahead implements it.
///
/// Meanings keep two rules an emitter may rely on: `set` and `set_vector` are never called inside a part
/// of the meaning that runs only sometimes, and an instruction that calls `set_flags` once does so
/// unconditionally. Locals a meaning gets from `local` are only used within that instruction, except those
/// named in the `Flags` it passes to `set_flags`.
pub trait Emitter {
    /// The body being written.
    fn sink(&mut self) -> InstructionSink<'_>;
    /// A fresh local of this type.
    fn local(&mut self, ty: ValType) -> u32;
    /// Push a register as an i64. The zero register reads as zero.
    fn get(&mut self, r: Reg);
    /// Pop an i64 into a register. Writes to the zero register are dropped.
    fn set(&mut self, r: Reg);
    /// Push the guest address of this instruction plus `by`.
    fn this_address(&mut self, by: i64);
    fn set_flags(&mut self, flags: Flags);
    /// Push one flag as an i32 0 or 1.
    fn flag(&mut self, flag: Flag);
    /// Push SIMD and floating point register `n` as a v128.
    fn get_vector(&mut self, n: u8);
    /// Pop a v128 into SIMD and floating point register `n`.
    fn set_vector(&mut self, n: u8);
    /// Push a register kept only in the thread's state, as a value of [`Control::ty`].
    fn get_control(&mut self, control: Control);
    /// Pop a value of [`Control::ty`] into a register kept only in the thread's state.
    fn set_control(&mut self, control: Control);
    /// Call a hook with its arguments pushed; it leaves its results.
    fn hook(&mut self, hook: Hook);
    /// What the host's engine does where WebAssembly leaves it open.
    fn host(&self) -> Host;
    /// Floating point takes the slow path when FPCR is not zero or FPSR's flags are kept. Push whether it
    /// does, as an i32 not zero when it does, and return true: the meaning then writes both paths. Or, as
    /// nothing of the instruction is written yet, carry on from this instruction in other code when it does,
    /// and return false: the meaning then writes only the fast path.
    fn float_mode(&mut self) -> bool {
        self.get_control(Control::Fpcr);
        self.get_control(Control::FloatTracking);
        self.sink().i32_or();
        true
    }
    /// Whether the stack pointer is known here to be a multiple of 16, so its alignment needs no check.
    fn stack_aligned(&mut self) -> bool {
        false
    }
    /// Untag the address in this i64 local in place (see [`untag`]), then stop the thread with a memory
    /// fault unless `bytes` bytes at it may be accessed this way.
    fn check(&mut self, address: u32, bytes: u32, access: Access);
    /// Go to the target. Nothing after it in this instruction runs.
    fn jump(&mut self, target: Target);
    /// Pop an i32 and go to the target if it is not zero.
    fn jump_if(&mut self, target: Target);
    /// Call the target with the next instruction as the return address. x30 is already set.
    fn call(&mut self, target: Target);
    /// Return to the address in this i64 local.
    fn ret(&mut self, target: u32);
    /// The system call instruction: x8 is the number, x0 to x5 the arguments, x0 the result.
    fn syscall(&mut self);
    fn trap(&mut self, trap: Trap);
    /// Carry on with the next instruction. Ends every instruction that does not leave by itself.
    fn next(&mut self);

    /// Push whether a condition holds, as an i32.
    fn condition(&mut self, cond: Cond) {
        condition_from_flags(self, cond);
    }
}

/// Push whether a condition holds, worked out from the four flags.
pub fn condition_from_flags<E: Emitter + ?Sized>(e: &mut E, cond: Cond) {
    match cond.0 >> 1 {
        0 => e.flag(Flag::Z),
        1 => e.flag(Flag::C),
        2 => e.flag(Flag::N),
        3 => e.flag(Flag::V),
        4 => {
            e.flag(Flag::C);
            e.flag(Flag::Z);
            e.sink().i32_eqz().i32_and();
        }
        5 => {
            e.flag(Flag::N);
            e.flag(Flag::V);
            e.sink().i32_eq();
        }
        6 => {
            e.flag(Flag::N);
            e.flag(Flag::V);
            e.sink().i32_eq();
            e.flag(Flag::Z);
            e.sink().i32_eqz().i32_and();
        }
        _ => {
            e.sink().i32_const(1);
        }
    }
    if cond.0 & 1 == 1 && cond.0 != 15 {
        e.sink().i32_eqz();
    }
}

/// A WebAssembly comparison of two values of one width.
#[derive(Clone, Copy)]
enum Compare {
    Eq,
    Ne,
    GeU,
    LtU,
    GtU,
    LeU,
    GeS,
    LtS,
    GtS,
    LeS,
}

fn compare(sink: &mut InstructionSink<'_>, width: Width, op: Compare) {
    use Compare as C;
    match (width, op) {
        (Width::W32, C::Eq) => sink.i32_eq(),
        (Width::W32, C::Ne) => sink.i32_ne(),
        (Width::W32, C::GeU) => sink.i32_ge_u(),
        (Width::W32, C::LtU) => sink.i32_lt_u(),
        (Width::W32, C::GtU) => sink.i32_gt_u(),
        (Width::W32, C::LeU) => sink.i32_le_u(),
        (Width::W32, C::GeS) => sink.i32_ge_s(),
        (Width::W32, C::LtS) => sink.i32_lt_s(),
        (Width::W32, C::GtS) => sink.i32_gt_s(),
        (Width::W32, C::LeS) => sink.i32_le_s(),
        (Width::X64, C::Eq) => sink.i64_eq(),
        (Width::X64, C::Ne) => sink.i64_ne(),
        (Width::X64, C::GeU) => sink.i64_ge_u(),
        (Width::X64, C::LtU) => sink.i64_lt_u(),
        (Width::X64, C::GtU) => sink.i64_gt_u(),
        (Width::X64, C::LeU) => sink.i64_le_u(),
        (Width::X64, C::GeS) => sink.i64_ge_s(),
        (Width::X64, C::LtS) => sink.i64_lt_s(),
        (Width::X64, C::GtS) => sink.i64_gt_s(),
        (Width::X64, C::LeS) => sink.i64_le_s(),
    };
}

/// Push whether a condition holds as one comparison of the values the flags were set from, where they came
/// from a plain subtraction or a logical operation. Pushes nothing and returns false where it cannot.
pub fn condition_direct(sink: &mut InstructionSink<'_>, flags: Flags, cond: Cond) -> bool {
    use Compare as C;
    match flags {
        Flags::Add {
            width,
            a,
            subtrahend: Some(m),
            result,
            ..
        } => {
            let op = match cond.0 {
                0 => C::Eq,
                1 => C::Ne,
                2 => C::GeU,
                3 => C::LtU,
                8 => C::GtU,
                9 => C::LeU,
                10 => C::GeS,
                11 => C::LtS,
                12 => C::GtS,
                13 => C::LeS,
                4 | 5 => return sign_condition(sink, width, result, cond.0),
                _ => return false,
            };
            sink.local_get(a).local_get(m);
            compare(sink, width, op);
            true
        }
        Flags::Logic { width, result } => match cond.0 {
            0 | 1 => {
                sink.local_get(result);
                eqz(sink, width);
                if cond.0 == 1 {
                    sink.i32_eqz();
                }
                true
            }
            4 | 5 | 10..=13 => sign_condition(sink, width, result, cond.0),
            // C and V are clear.
            2 | 6 | 8 => {
                sink.i32_const(0);
                true
            }
            3 | 7 | 9 => {
                sink.i32_const(1);
                true
            }
            _ => false,
        },
        _ => false,
    }
}

/// Conditions on the sign of a result when V is clear or not asked: mi and lt, pl and ge, gt, le.
fn sign_condition(sink: &mut InstructionSink<'_>, width: Width, result: u32, cond: u8) -> bool {
    use Compare as C;
    let op = match cond {
        4 | 11 => C::LtS,
        5 | 10 => C::GeS,
        12 => C::GtS,
        13 => C::LeS,
        _ => return false,
    };
    sink.local_get(result);
    match width {
        Width::W32 => sink.i32_const(0),
        Width::X64 => sink.i64_const(0),
    };
    compare(sink, width, op);
    true
}

/// Push one flag, as an i32 0 or 1, for flags set this way.
pub fn flag_value(sink: &mut InstructionSink<'_>, flags: Flags, flag: Flag) {
    match flags {
        Flags::Constant(nzcv) => {
            sink.i32_const(i32::from((nzcv >> (flag.bit() - 28)) & 1));
        }
        Flags::Packed(local) => {
            sink.local_get(local)
                .i64_const(i64::from(flag.bit()))
                .i64_shr_u()
                .i32_wrap_i64()
                .i32_const(1)
                .i32_and();
        }
        Flags::Logic { width, result } => match flag {
            Flag::N => sign_bit(sink, width, result),
            Flag::Z => {
                sink.local_get(result);
                eqz(sink, width);
            }
            Flag::C | Flag::V => {
                sink.i32_const(0);
            }
        },
        Flags::Add {
            width,
            a,
            b,
            carry,
            result,
            ..
        } => match flag {
            Flag::N => sign_bit(sink, width, result),
            Flag::Z => {
                sink.local_get(result);
                eqz(sink, width);
            }
            // With no carry in, the sum carried out if it wrapped below `a`; with a carry in, also if it
            // came back to exactly `a`.
            Flag::C => {
                let below = |sink: &mut InstructionSink<'_>| {
                    sink.local_get(result).local_get(a);
                    match width {
                        Width::W32 => sink.i32_lt_u(),
                        Width::X64 => sink.i64_lt_u(),
                    };
                };
                let at_or_below = |sink: &mut InstructionSink<'_>| {
                    sink.local_get(result).local_get(a);
                    match width {
                        Width::W32 => sink.i32_le_u(),
                        Width::X64 => sink.i64_le_u(),
                    };
                };
                match carry {
                    Carry::Zero => below(sink),
                    Carry::One => at_or_below(sink),
                    Carry::Local(c) => {
                        at_or_below(sink);
                        below(sink);
                        sink.local_get(c).select();
                    }
                }
            }
            // Overflow when both inputs differ in sign from the result.
            Flag::V => {
                match width {
                    Width::W32 => sink
                        .local_get(a)
                        .local_get(result)
                        .i32_xor()
                        .local_get(b)
                        .local_get(result)
                        .i32_xor()
                        .i32_and()
                        .i32_const(31)
                        .i32_shr_u(),
                    Width::X64 => sink
                        .local_get(a)
                        .local_get(result)
                        .i64_xor()
                        .local_get(b)
                        .local_get(result)
                        .i64_xor()
                        .i64_and()
                        .i64_const(63)
                        .i64_shr_u()
                        .i32_wrap_i64(),
                };
            }
        },
    }
}

fn sign_bit(sink: &mut InstructionSink<'_>, width: Width, local: u32) {
    match width {
        Width::W32 => sink.local_get(local).i32_const(31).i32_shr_u(),
        Width::X64 => sink
            .local_get(local)
            .i64_const(63)
            .i64_shr_u()
            .i32_wrap_i64(),
    };
}

fn eqz(sink: &mut InstructionSink<'_>, width: Width) {
    match width {
        Width::W32 => sink.i32_eqz(),
        Width::X64 => sink.i64_eqz(),
    };
}

pub(crate) fn ty(width: Width) -> ValType {
    match width {
        Width::W32 => ValType::I32,
        Width::X64 => ValType::I64,
    }
}

/// The same operation at either width.
macro_rules! op {
    ($e:expr, $w:expr, $narrow:ident, $wide:ident) => {
        match $w {
            Width::W32 => {
                $e.sink().$narrow();
            }
            Width::X64 => {
                $e.sink().$wide();
            }
        }
    };
}

pub(crate) fn constant<E: Emitter + ?Sized>(e: &mut E, w: Width, value: u64) {
    match w {
        Width::W32 => e.sink().i32_const(value as u32 as i32),
        Width::X64 => e.sink().i64_const(value as i64),
    };
}

/// Push a register at a width.
pub(crate) fn get<E: Emitter + ?Sized>(e: &mut E, r: Reg, w: Width) {
    e.get(r);
    if w == Width::W32 {
        e.sink().i32_wrap_i64();
    }
}

/// Pop a value of a width into a register; 32 bit results clear the top half.
pub(crate) fn set<E: Emitter + ?Sized>(e: &mut E, r: Reg, w: Width) {
    if w == Width::W32 {
        e.sink().i64_extend_i32_u();
    }
    e.set(r);
}

/// Pop a value of a width into a fresh local.
pub(crate) fn save<E: Emitter + ?Sized>(e: &mut E, w: Width) -> u32 {
    let local = e.local(ty(w));
    e.sink().local_set(local);
    local
}

fn shift_by<E: Emitter + ?Sized>(e: &mut E, w: Width, shift: Shift, amount: u8) {
    if amount == 0 {
        return;
    }
    constant(e, w, amount.into());
    match shift {
        Shift::Lsl => op!(e, w, i32_shl, i64_shl),
        Shift::Lsr => op!(e, w, i32_shr_u, i64_shr_u),
        Shift::Asr => op!(e, w, i32_shr_s, i64_shr_s),
        Shift::Ror => op!(e, w, i32_rotr, i64_rotr),
    }
}

/// Push a register widened as `extend` says, as an i64.
pub(crate) fn extended<E: Emitter + ?Sized>(e: &mut E, r: Reg, extend: Extend) {
    e.get(r);
    match (extend.signed, extend.from_bits) {
        (_, 64) => {}
        (true, 8) => {
            e.sink().i64_extend8_s();
        }
        (true, 16) => {
            e.sink().i64_extend16_s();
        }
        (true, _) => {
            e.sink().i64_extend32_s();
        }
        (false, bits) => {
            e.sink().i64_const(((1u64 << bits) - 1) as i64).i64_and();
        }
    }
}

/// Push the second source at a width.
fn operand<E: Emitter + ?Sized>(e: &mut E, w: Width, operand: Operand) {
    match operand {
        Operand::Imm(v) => constant(e, w, v),
        Operand::Shifted { rm, shift, amount } => {
            get(e, rm, w);
            shift_by(e, w, shift, amount);
        }
        Operand::Extended { rm, extend, amount } => {
            extended(e, rm, extend);
            if w == Width::W32 {
                e.sink().i32_wrap_i64();
            }
            shift_by(e, w, Shift::Lsl, amount);
        }
    }
}

/// `a + b + carry` with flags if asked; `b` is already on the stack, `a` in a local.
fn add_with<E: Emitter + ?Sized>(
    e: &mut E,
    w: Width,
    a: u32,
    invert: bool,
    carry: Carry,
    set_flags: bool,
) -> u32 {
    let mut subtrahend = None;
    if invert {
        if carry == Carry::One {
            let m = save(e, w);
            e.sink().local_get(m);
            subtrahend = Some(m);
        }
        constant(e, w, u64::MAX);
        op!(e, w, i32_xor, i64_xor);
    }
    let b = save(e, w);
    e.sink().local_get(a).local_get(b);
    op!(e, w, i32_add, i64_add);
    match carry {
        Carry::Zero => {}
        Carry::One => {
            constant(e, w, 1);
            op!(e, w, i32_add, i64_add);
        }
        Carry::Local(c) => {
            e.sink().local_get(c);
            if w == Width::X64 {
                e.sink().i64_extend_i32_u();
            }
            op!(e, w, i32_add, i64_add);
        }
    }
    let result = save(e, w);
    if set_flags {
        e.set_flags(Flags::Add {
            width: w,
            a,
            b,
            carry,
            result,
            subtrahend,
        });
    }
    result
}

/// Push the address in an i64 local as the CPU uses it. Linux turns on Top Byte Ignore for user space: when
/// bit 55 is clear the top byte is ignored, which here means cleared; when it is set the address is used
/// whole.
pub fn untag(sink: &mut InstructionSink<'_>, local: u32) {
    sink.local_get(local)
        .local_get(local)
        .i64_const(8)
        .i64_shl()
        .i64_const(63)
        .i64_shr_s()
        .i64_const(56)
        .i64_shl()
        .i64_const(0x00ff_ffff_ffff_ffff)
        .i64_or()
        .i64_and();
}

/// The address of a load or store as the program computed it, in a fresh i64 local for the check to untag,
/// and the write back to make after the access: the base register and what to add to the tagged address.
pub(crate) fn address<E: Emitter + ?Sized>(
    e: &mut E,
    address: Address,
) -> (u32, Option<(Reg, u32, i64)>) {
    let local = e.local(ValType::I64);
    let base = match address {
        Address::Offset { base, .. }
        | Address::PreIndex { base, .. }
        | Address::PostIndex { base, .. }
        | Address::Indexed { base, .. } => Some(base),
        Address::Literal { .. } => None,
    };
    // Linux has the CPU check that the stack pointer is a multiple of 16 when it is the base.
    if base == Some(Reg::Sp) && !e.stack_aligned() {
        e.get(Reg::Sp);
        e.sink()
            .local_tee(local)
            .i64_const(15)
            .i64_and()
            .i64_const(0)
            .i64_ne()
            .if_(BlockType::Empty);
        e.trap(Trap::Alignment(local));
        e.sink().end();
    }
    let mut post = None;
    match address {
        Address::Offset { base, offset } | Address::PreIndex { base, offset } => {
            e.get(base);
            if offset != 0 {
                e.sink().i64_const(offset).i64_add();
            }
        }
        Address::PostIndex { base, offset } => {
            e.get(base);
            post = Some((base, offset));
        }
        Address::Indexed {
            base,
            index,
            extend,
            shift,
        } => {
            e.get(base);
            extended(e, index, extend);
            if shift != 0 {
                e.sink().i64_const(shift.into()).i64_shl();
            }
            e.sink().i64_add();
        }
        Address::Literal { offset } => e.this_address(offset),
    }
    e.sink().local_set(local);
    if let Address::PreIndex { base, .. } = address {
        post = Some((base, 0));
    }
    // A write back keeps the tagged address; otherwise the check may untag it where it is.
    if post.is_none() {
        return (local, None);
    }
    let at = e.local(ValType::I64);
    e.sink().local_get(local).local_set(at);
    (at, post.map(|(base, offset)| (base, local, offset)))
}

/// The write back of a pre or post indexed access, after the access itself.
pub(crate) fn write_back<E: Emitter + ?Sized>(e: &mut E, update: Option<(Reg, u32, i64)>) {
    if let Some((base, local, offset)) = update {
        e.sink().local_get(local);
        if offset != 0 {
            e.sink().i64_const(offset).i64_add();
        }
        e.set(base);
    }
}

pub(crate) fn memory(offset: u64) -> MemArg {
    MemArg {
        offset,
        align: 0,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }
}

/// Load from the address in `local` plus `offset`, leaving an i64.
pub(crate) fn load<E: Emitter + ?Sized>(
    e: &mut E,
    local: u32,
    offset: u64,
    bytes: u8,
    signed: Option<Width>,
) {
    e.sink().local_get(local);
    if offset != 0 {
        e.sink().i64_const(offset as i64).i64_add();
    }
    let m = memory(0);
    let mut s = e.sink();
    match (bytes, signed) {
        (1, None) => s.i64_load8_u(m),
        (2, None) => s.i64_load16_u(m),
        (4, None) => s.i64_load32_u(m),
        (8, _) => s.i64_load(m),
        (1, Some(Width::X64)) => s.i64_load8_s(m),
        (2, Some(Width::X64)) => s.i64_load16_s(m),
        (4, Some(_)) => s.i64_load32_s(m),
        (1, Some(Width::W32)) => s.i32_load8_s(m).i64_extend_i32_u(),
        (_, Some(Width::W32)) => s.i32_load16_s(m).i64_extend_i32_u(),
        _ => unreachable!("load of {bytes} bytes"),
    };
}

/// Store the low `bytes` of a register at the address in `local` plus `offset`.
pub(crate) fn store<E: Emitter + ?Sized>(e: &mut E, local: u32, offset: u64, bytes: u8, rt: Reg) {
    e.sink().local_get(local);
    if offset != 0 {
        e.sink().i64_const(offset as i64).i64_add();
    }
    e.get(rt);
    let m = memory(0);
    let mut s = e.sink();
    match bytes {
        1 => s.i64_store8(m),
        2 => s.i64_store16(m),
        4 => s.i64_store32(m),
        _ => s.i64_store(m),
    };
}

/// Load `bytes` from the address in `local` plus `offset` into the low bytes of a v128, the rest zero.
fn load_vector<E: Emitter + ?Sized>(e: &mut E, local: u32, offset: u64, bytes: u8) {
    if bytes < 4 {
        e.sink().v128_const(0);
    }
    e.sink().local_get(local);
    if offset != 0 {
        e.sink().i64_const(offset as i64).i64_add();
    }
    let m = memory(0);
    let mut s = e.sink();
    match bytes {
        1 => s.i32_load8_u(m).i32x4_replace_lane(0),
        2 => s.i32_load16_u(m).i32x4_replace_lane(0),
        4 => s.v128_load32_zero(m),
        8 => s.v128_load64_zero(m),
        _ => s.v128_load(m),
    };
}

/// Store the low `bytes` of vector register `rt` at the address in `local` plus `offset`.
fn store_vector<E: Emitter + ?Sized>(e: &mut E, local: u32, offset: u64, bytes: u8, rt: u8) {
    e.sink().local_get(local);
    if offset != 0 {
        e.sink().i64_const(offset as i64).i64_add();
    }
    e.get_vector(rt);
    let m = memory(0);
    let mut s = e.sink();
    match bytes {
        1 => s.v128_store8_lane(m, 0),
        2 => s.v128_store16_lane(m, 0),
        4 => s.v128_store32_lane(m, 0),
        8 => s.v128_store64_lane(m, 0),
        _ => s.v128_store(m),
    };
}

/// Swap neighbouring groups of `size` bits, in every group of `2 * size`.
fn swap<E: Emitter + ?Sized>(e: &mut E, w: Width, value: u32, size: u32) {
    let pattern = (0..64)
        .filter(|bit| (bit / size).is_multiple_of(2))
        .fold(0u64, |m, bit| m | 1 << bit);
    e.sink().local_get(value);
    constant(e, w, size.into());
    op!(e, w, i32_shr_u, i64_shr_u);
    constant(e, w, pattern);
    op!(e, w, i32_and, i64_and);
    e.sink().local_get(value);
    constant(e, w, pattern);
    op!(e, w, i32_and, i64_and);
    constant(e, w, size.into());
    op!(e, w, i32_shl, i64_shl);
    op!(e, w, i32_or, i64_or);
    e.sink().local_set(value);
}

/// Bit masks of a bitfield move: (wmask, tmask) from Arm's `DecodeBitMasks` with `immediate` false.
fn bitfield_masks(w: Width, immr: u8, imms: u8) -> (u64, u64) {
    let size = w.bits();
    let ones = |n: u32| if n >= 64 { u64::MAX } else { (1u64 << n) - 1 };
    let all = ones(size);
    let welem = ones(u32::from(imms) + 1);
    let r = u32::from(immr);
    let wmask = if r == 0 {
        welem
    } else {
        ((welem >> r) | (welem << (size - r))) & all
    };
    let d = (u32::from(imms).wrapping_sub(r)) & (size - 1);
    (wmask, ones(d + 1))
}

/// The high 64 bits of an unsigned 128 bit product of two i64 locals, pushed.
fn multiply_high<E: Emitter + ?Sized>(e: &mut E, a: u32, b: u32) {
    let half = |e: &mut E, local: u32, high: bool| {
        e.sink().local_get(local);
        if high {
            e.sink().i64_const(32).i64_shr_u();
        } else {
            e.sink().i64_const(0xffff_ffff).i64_and();
        }
    };
    // a_hi * b_lo, kept for its two halves.
    half(e, a, true);
    half(e, b, false);
    e.sink().i64_mul();
    let high_low = e.local(ValType::I64);
    e.sink().local_set(high_low);
    // cross = (a_lo * b_lo >> 32) + (high_low & mask) + a_lo * b_hi, which cannot overflow.
    half(e, a, false);
    half(e, b, false);
    e.sink()
        .i64_mul()
        .i64_const(32)
        .i64_shr_u()
        .local_get(high_low)
        .i64_const(0xffff_ffff)
        .i64_and()
        .i64_add();
    half(e, a, false);
    half(e, b, true);
    e.sink().i64_mul().i64_add();
    let cross = e.local(ValType::I64);
    e.sink().local_set(cross);
    half(e, a, true);
    half(e, b, true);
    e.sink()
        .i64_mul()
        .local_get(high_low)
        .i64_const(32)
        .i64_shr_u()
        .i64_add()
        .local_get(cross)
        .i64_const(32)
        .i64_shr_u()
        .i64_add();
}

/// Write the meaning of one instruction.
pub fn emit<E: Emitter + ?Sized>(e: &mut E, instruction: &Instruction) {
    use Instruction as I;
    match *instruction {
        // Without flags, a plain addition or subtraction; adding zero is a copy.
        I::AddSub {
            width: w,
            subtract,
            set_flags: false,
            rd,
            rn,
            operand: source,
        } => {
            get(e, rn, w);
            if source != Operand::Imm(0) {
                operand(e, w, source);
                match subtract {
                    false => op!(e, w, i32_add, i64_add),
                    true => op!(e, w, i32_sub, i64_sub),
                }
            }
            set(e, rd, w);
        }
        I::AddSub {
            width: w,
            subtract,
            set_flags,
            rd,
            rn,
            operand: source,
        } => {
            get(e, rn, w);
            let a = save(e, w);
            operand(e, w, source);
            let carry = if subtract { Carry::One } else { Carry::Zero };
            let result = add_with(e, w, a, subtract, carry, set_flags);
            e.sink().local_get(result);
            set(e, rd, w);
        }
        I::AddSubCarry {
            width: w,
            subtract,
            set_flags,
            rd,
            rn,
            rm,
        } => {
            e.flag(Flag::C);
            let c = e.local(ValType::I32);
            e.sink().local_set(c);
            get(e, rn, w);
            let a = save(e, w);
            get(e, rm, w);
            let result = add_with(e, w, a, subtract, Carry::Local(c), set_flags);
            e.sink().local_get(result);
            set(e, rd, w);
        }
        I::Logical {
            width: w,
            op,
            invert,
            set_flags,
            rd,
            rn,
            operand: source,
        } => {
            // A copy (`mov`, `mvn`) is an or or exclusive or with the zero register.
            let copy = rn == Reg::Zero && op != Logic::And;
            if !copy {
                get(e, rn, w);
            }
            operand(e, w, source);
            if invert {
                constant(e, w, u64::MAX);
                op!(e, w, i32_xor, i64_xor);
            }
            match op {
                _ if copy => {}
                Logic::And => op!(e, w, i32_and, i64_and),
                Logic::Or => op!(e, w, i32_or, i64_or),
                Logic::Xor => op!(e, w, i32_xor, i64_xor),
            }
            if set_flags {
                let result = save(e, w);
                e.set_flags(Flags::Logic { width: w, result });
                e.sink().local_get(result);
            }
            set(e, rd, w);
        }
        I::MoveWide {
            width: w,
            op,
            rd,
            imm,
            shift,
        } => {
            let placed = u64::from(imm) << shift;
            match op {
                MoveWide::Zero => constant(e, w, placed),
                MoveWide::Not => constant(e, w, !placed),
                MoveWide::Keep => {
                    get(e, rd, w);
                    constant(e, w, !(0xffffu64 << shift));
                    op!(e, w, i32_and, i64_and);
                    constant(e, w, placed);
                    op!(e, w, i32_or, i64_or);
                }
            }
            set(e, rd, w);
        }
        I::Bitfield {
            width: w,
            op,
            rd,
            rn,
            immr,
            imms,
        } => {
            let (wmask, tmask) = bitfield_masks(w, immr, imms);
            get(e, rn, w);
            let src = save(e, w);
            // bot = (dst & !wmask) | (ror(src, immr) & wmask), where dst is Rd only for bfm.
            e.sink().local_get(src);
            constant(e, w, immr.into());
            op!(e, w, i32_rotr, i64_rotr);
            constant(e, w, wmask);
            op!(e, w, i32_and, i64_and);
            if op == Bitfield::Insert {
                get(e, rd, w);
                constant(e, w, !wmask);
                op!(e, w, i32_and, i64_and);
                op!(e, w, i32_or, i64_or);
            }
            constant(e, w, tmask);
            op!(e, w, i32_and, i64_and);
            // top = copies of bit imms of the source for sbfm, Rd for bfm, zero for ubfm.
            match op {
                Bitfield::Signed => {
                    e.sink().local_get(src);
                    constant(e, w, u64::from(w.bits() - 1 - u32::from(imms)));
                    op!(e, w, i32_shl, i64_shl);
                    constant(e, w, u64::from(w.bits() - 1));
                    op!(e, w, i32_shr_s, i64_shr_s);
                }
                Bitfield::Insert => get(e, rd, w),
                Bitfield::Unsigned => constant(e, w, 0),
            }
            constant(e, w, !tmask);
            op!(e, w, i32_and, i64_and);
            op!(e, w, i32_or, i64_or);
            set(e, rd, w);
        }
        I::Extract {
            width: w,
            rd,
            rn,
            rm,
            lsb,
        } => {
            get(e, rm, w);
            if lsb != 0 {
                constant(e, w, lsb.into());
                op!(e, w, i32_shr_u, i64_shr_u);
                get(e, rn, w);
                constant(e, w, u64::from(w.bits() - u32::from(lsb)));
                op!(e, w, i32_shl, i64_shl);
                op!(e, w, i32_or, i64_or);
            }
            set(e, rd, w);
        }
        I::PcRelative { rd, page, offset } => {
            if page {
                e.this_address(0);
                e.sink().i64_const(!0xfff).i64_and();
                e.sink().i64_const(offset).i64_add();
            } else {
                e.this_address(offset);
            }
            e.set(rd);
        }
        I::CondCompare {
            width: w,
            negate,
            rn,
            operand: source,
            nzcv,
            cond,
        } => {
            e.condition(cond);
            e.sink().if_(BlockType::Empty);
            get(e, rn, w);
            let a = save(e, w);
            operand(e, w, source);
            let carry = if negate { Carry::Zero } else { Carry::One };
            add_with(e, w, a, !negate, carry, true);
            e.sink().else_();
            e.set_flags(Flags::Constant(nzcv));
            e.sink().end();
        }
        I::CondSelect {
            width: w,
            op,
            rd,
            rn,
            rm,
            cond,
        } => {
            get(e, rn, w);
            get(e, rm, w);
            match op {
                Select::Plain => {}
                Select::Increment => {
                    constant(e, w, 1);
                    op!(e, w, i32_add, i64_add);
                }
                Select::Invert => {
                    constant(e, w, u64::MAX);
                    op!(e, w, i32_xor, i64_xor);
                }
                Select::Negate => {
                    let value = save(e, w);
                    constant(e, w, 0);
                    e.sink().local_get(value);
                    op!(e, w, i32_sub, i64_sub);
                }
            }
            e.condition(cond);
            e.sink().select();
            set(e, rd, w);
        }
        I::Unary {
            width: w,
            op,
            rd,
            rn,
        } => {
            get(e, rn, w);
            match op {
                Unary::Clz => op!(e, w, i32_clz, i64_clz),
                Unary::Cls => {
                    let value = save(e, w);
                    e.sink().local_get(value).local_get(value);
                    constant(e, w, 1);
                    op!(e, w, i32_shr_s, i64_shr_s);
                    op!(e, w, i32_xor, i64_xor);
                    op!(e, w, i32_clz, i64_clz);
                    constant(e, w, 1);
                    op!(e, w, i32_sub, i64_sub);
                }
                Unary::Rbit | Unary::Rev16 | Unary::Rev32 | Unary::Rev => {
                    let value = save(e, w);
                    let (bit_steps, container) = match op {
                        Unary::Rbit => (true, w.bits()),
                        Unary::Rev16 => (false, 16),
                        Unary::Rev32 => (false, 32),
                        _ => (false, w.bits()),
                    };
                    if bit_steps {
                        for size in [1, 2, 4] {
                            swap(e, w, value, size);
                        }
                    }
                    for size in [8, 16, 32] {
                        if size < container {
                            swap(e, w, value, size);
                        }
                    }
                    e.sink().local_get(value);
                }
            }
            set(e, rd, w);
        }
        I::Binary {
            width: w,
            op,
            rd,
            rn,
            rm,
        } => {
            match op {
                Binary::Udiv | Binary::Sdiv => {
                    get(e, rn, w);
                    let n = save(e, w);
                    get(e, rm, w);
                    let m = save(e, w);
                    // Division by zero gives zero; dividing by -1 is a negation, which wraps as Arm does.
                    let t = ty(w);
                    e.sink().local_get(m);
                    eqz(&mut e.sink(), w);
                    e.sink().if_(BlockType::Result(t));
                    constant(e, w, 0);
                    e.sink().else_();
                    if op == Binary::Sdiv {
                        e.sink().local_get(m);
                        constant(e, w, u64::MAX);
                        op!(e, w, i32_eq, i64_eq);
                        e.sink().if_(BlockType::Result(t));
                        constant(e, w, 0);
                        e.sink().local_get(n);
                        op!(e, w, i32_sub, i64_sub);
                        e.sink().else_().local_get(n).local_get(m);
                        op!(e, w, i32_div_s, i64_div_s);
                        e.sink().end();
                    } else {
                        e.sink().local_get(n).local_get(m);
                        op!(e, w, i32_div_u, i64_div_u);
                    }
                    e.sink().end();
                }
                // WebAssembly takes shift counts modulo the width, as Arm does.
                _ => {
                    get(e, rn, w);
                    get(e, rm, w);
                    match op {
                        Binary::Lslv => op!(e, w, i32_shl, i64_shl),
                        Binary::Lsrv => op!(e, w, i32_shr_u, i64_shr_u),
                        Binary::Asrv => op!(e, w, i32_shr_s, i64_shr_s),
                        _ => op!(e, w, i32_rotr, i64_rotr),
                    }
                }
            }
            set(e, rd, w);
        }
        I::MulAdd {
            width: w,
            op,
            rd,
            rn,
            rm,
            ra,
        } => match op {
            MulAdd::Madd | MulAdd::Msub => {
                get(e, ra, w);
                get(e, rn, w);
                get(e, rm, w);
                op!(e, w, i32_mul, i64_mul);
                if op == MulAdd::Madd {
                    op!(e, w, i32_add, i64_add);
                } else {
                    op!(e, w, i32_sub, i64_sub);
                }
                set(e, rd, w);
            }
            MulAdd::Smaddl | MulAdd::Smsubl | MulAdd::Umaddl | MulAdd::Umsubl => {
                let signed = matches!(op, MulAdd::Smaddl | MulAdd::Smsubl);
                let extend = Extend {
                    signed,
                    from_bits: 32,
                };
                e.get(ra);
                extended(e, rn, extend);
                extended(e, rm, extend);
                e.sink().i64_mul();
                if matches!(op, MulAdd::Smaddl | MulAdd::Umaddl) {
                    e.sink().i64_add();
                } else {
                    e.sink().i64_sub();
                }
                e.set(rd);
            }
            MulAdd::Smulh | MulAdd::Umulh => {
                e.get(rn);
                let a = save(e, Width::X64);
                e.get(rm);
                let b = save(e, Width::X64);
                multiply_high(e, a, b);
                if op == MulAdd::Smulh {
                    // The signed high half is the unsigned one less b when a is negative and less a
                    // when b is negative.
                    for (x, y) in [(a, b), (b, a)] {
                        e.sink()
                            .local_get(y)
                            .i64_const(0)
                            .local_get(x)
                            .i64_const(0)
                            .i64_lt_s()
                            .select()
                            .i64_sub();
                    }
                }
                e.set(rd);
            }
        },
        I::Branch { link, offset } => {
            if link {
                link_register(e);
                e.call(Target::Relative(offset));
                e.next();
            } else {
                e.jump(Target::Relative(offset));
            }
        }
        I::BranchCond { cond, offset } => {
            e.condition(cond);
            e.jump_if(Target::Relative(offset));
            e.next();
        }
        I::CompareBranch {
            width: w,
            nonzero,
            rt,
            offset,
        } => {
            get(e, rt, w);
            eqz(&mut e.sink(), w);
            if nonzero {
                e.sink().i32_eqz();
            }
            e.jump_if(Target::Relative(offset));
            e.next();
        }
        I::TestBranch {
            nonzero,
            rt,
            bit,
            offset,
        } => {
            e.get(rt);
            e.sink()
                .i64_const(bit.into())
                .i64_shr_u()
                .i32_wrap_i64()
                .i32_const(1)
                .i32_and();
            if !nonzero {
                e.sink().i32_eqz();
            }
            e.jump_if(Target::Relative(offset));
            e.next();
        }
        I::BranchRegister { op, rn } => {
            e.get(rn);
            let tagged = save(e, Width::X64);
            untag(&mut e.sink(), tagged);
            let target = save(e, Width::X64);
            match op {
                BranchRegister::Br => e.jump(Target::Address(target)),
                BranchRegister::Blr => {
                    link_register(e);
                    e.call(Target::Address(target));
                    e.next();
                }
                BranchRegister::Ret => e.ret(target),
            }
        }
        I::Load {
            bytes,
            signed,
            rt,
            address: form,
        } => {
            let (at, update) = address(e, form);
            e.check(at, bytes.into(), Access::Read);
            load(e, at, 0, bytes, signed);
            e.set(rt);
            write_back(e, update);
            e.next();
        }
        I::Store {
            bytes,
            rt,
            address: form,
        } => {
            let (at, update) = address(e, form);
            e.check(at, bytes.into(), Access::Write);
            store(e, at, 0, bytes, rt);
            write_back(e, update);
            e.next();
        }
        I::LoadPair {
            bytes,
            signed,
            rt,
            rt2,
            address: form,
        } => {
            let (at, update) = address(e, form);
            e.check(at, 2 * u32::from(bytes), Access::Read);
            let signed = signed.then_some(Width::X64);
            load(e, at, 0, bytes, signed);
            let first = save(e, Width::X64);
            load(e, at, bytes.into(), bytes, signed);
            e.set(rt2);
            e.sink().local_get(first);
            e.set(rt);
            write_back(e, update);
            e.next();
        }
        I::StorePair {
            bytes,
            rt,
            rt2,
            address: form,
        } => {
            let (at, update) = address(e, form);
            e.check(at, 2 * u32::from(bytes), Access::Write);
            store(e, at, 0, bytes, rt);
            store(e, at, bytes.into(), bytes, rt2);
            write_back(e, update);
            e.next();
        }
        I::LoadVector {
            bytes,
            rt,
            address: form,
        } => {
            let (at, update) = address(e, form);
            e.check(at, bytes.into(), Access::Read);
            load_vector(e, at, 0, bytes);
            e.set_vector(rt);
            write_back(e, update);
            e.next();
        }
        I::StoreVector {
            bytes,
            rt,
            address: form,
        } => {
            let (at, update) = address(e, form);
            e.check(at, bytes.into(), Access::Write);
            store_vector(e, at, 0, bytes, rt);
            write_back(e, update);
            e.next();
        }
        I::LoadVectorPair {
            bytes,
            rt,
            rt2,
            address: form,
        } => {
            let (at, update) = address(e, form);
            e.check(at, 2 * u32::from(bytes), Access::Read);
            load_vector(e, at, 0, bytes);
            let first = e.local(ValType::V128);
            e.sink().local_set(first);
            load_vector(e, at, bytes.into(), bytes);
            e.set_vector(rt2);
            e.sink().local_get(first);
            e.set_vector(rt);
            write_back(e, update);
            e.next();
        }
        I::StoreVectorPair {
            bytes,
            rt,
            rt2,
            address: form,
        } => {
            let (at, update) = address(e, form);
            e.check(at, 2 * u32::from(bytes), Access::Write);
            store_vector(e, at, 0, bytes, rt);
            store_vector(e, at, bytes.into(), bytes, rt2);
            write_back(e, update);
            e.next();
        }
        I::VectorImmediate { .. }
        | I::DuplicateGeneral { .. }
        | I::DuplicateElement { .. }
        | I::MoveToGeneral { .. }
        | I::MoveFromGeneral { .. }
        | I::InsertGeneral { .. }
        | I::InsertElement { .. }
        | I::ExtractVector { .. } => crate::vector::emit(e, instruction),
        I::Simd { .. } | I::Permute { .. } | I::Table { .. } | I::ScalarElement { .. } => {
            crate::simd::emit(e, instruction);
            e.next();
        }
        I::Float { .. }
        | I::FloatConvert { .. }
        | I::FloatBinary { .. }
        | I::FloatMulAdd { .. }
        | I::FloatCompare { .. }
        | I::FloatCondCompare { .. }
        | I::FloatSelect { .. }
        | I::FloatToInteger { .. }
        | I::IntegerToFloat { .. } => {
            crate::float::emit(e, instruction);
            e.next();
        }
        I::Atomic { .. }
        | I::CompareSwap { .. }
        | I::LoadExclusive { .. }
        | I::StoreExclusive { .. }
        | I::LoadAcquire { .. }
        | I::StoreRelease { .. }
        | I::ClearExclusive
        | I::ZeroBlock { .. }
        | I::CleanCache { .. }
        | I::CodeChanged { .. }
        | I::SimdMemory { .. } => {
            crate::memory::emit(e, instruction);
            e.next();
        }
        I::Crc32 {
            castagnoli,
            bytes,
            rd,
            rn,
            rm,
        } => crc32(e, castagnoli, bytes, rd, rn, rm),
        I::Syscall => {
            // A return from the kernel forgets any exclusive access.
            e.sink().i32_const(0);
            e.set_control(Control::ExclusiveBytes);
            e.syscall();
            e.next();
        }
        I::Breakpoint { comment } => e.trap(Trap::Breakpoint(comment)),
        I::Nothing => e.next(),
        I::ReadSystem { register, rt } => {
            match register {
                SystemRegister::Nzcv => {
                    e.sink().i64_const(0);
                    for flag in Flag::ALL {
                        e.flag(flag);
                        e.sink()
                            .i64_extend_i32_u()
                            .i64_const(flag.bit().into())
                            .i64_shl()
                            .i64_or();
                    }
                }
                SystemRegister::TpidrEl0 => e.get_control(Control::ThreadPointer),
                SystemRegister::Fpcr => {
                    e.get_control(Control::Fpcr);
                    e.sink().i64_extend_i32_u();
                }
                SystemRegister::Fpsr => {
                    // The flags are only kept once asked for: reading them before is the slow path.
                    e.get_control(Control::FloatTracking);
                    e.sink().i32_eqz().if_(BlockType::Empty);
                    e.sink()
                        .i32_const(crate::exact::Operation::ReadStatus.encode(0))
                        .i64_const(0)
                        .i64_const(0)
                        .i64_const(0);
                    e.hook(Hook::Float);
                    e.sink().drop().end();
                    e.get_control(Control::Fpsr);
                    e.sink().i64_extend_i32_u();
                }
                SystemRegister::Counter => e.hook(Hook::Clock),
                SystemRegister::Fixed(value) => {
                    e.sink().i64_const(value as i64);
                }
            }
            e.set(rt);
            e.next();
        }
        I::WriteSystem { register, rt } => {
            e.get(rt);
            match register {
                SystemRegister::Nzcv => {
                    let packed = save(e, Width::X64);
                    e.set_flags(Flags::Packed(packed));
                }
                SystemRegister::TpidrEl0 => e.set_control(Control::ThreadPointer),
                SystemRegister::Fpcr => {
                    e.sink()
                        .i32_wrap_i64()
                        .i32_const(FPCR_BITS as i32)
                        .i32_and();
                    e.set_control(Control::Fpcr);
                }
                SystemRegister::Fpsr => {
                    e.sink()
                        .i32_wrap_i64()
                        .i32_const(FPSR_BITS as i32)
                        .i32_and();
                    e.set_control(Control::Fpsr);
                    e.sink().i32_const(1);
                    e.set_control(Control::FloatTracking);
                }
                SystemRegister::Counter | SystemRegister::Fixed(_) => {
                    unreachable!("a register that cannot be written")
                }
            }
            e.next();
        }
        I::Undefined => e.trap(Trap::Undefined),
        I::Unsupported => e.trap(Trap::Unsupported),
    }
    if instruction.is_data_processing() {
        e.next();
    }
}

/// crc32 and crc32c, bit by bit on the reflected polynomial.
fn crc32<E: Emitter + ?Sized>(e: &mut E, castagnoli: bool, bytes: u8, rd: Reg, rn: Reg, rm: Reg) {
    let polynomial: i64 = if castagnoli { 0x82f6_3b78 } else { 0xedb8_8320 };
    let crc = e.local(ValType::I64);
    e.get(rn);
    e.sink().i64_const(0xffff_ffff).i64_and();
    e.get(rm);
    if bytes < 8 {
        e.sink().i64_const((1i64 << (8 * bytes)) - 1).i64_and();
    }
    let mut s = e.sink();
    s.i64_xor().local_set(crc);
    for _ in 0..8 * u32::from(bytes) {
        // crc = (crc >> 1) ^ (polynomial if the low bit is set)
        s.local_get(crc)
            .i64_const(1)
            .i64_shr_u()
            .i64_const(polynomial)
            .i64_const(0)
            .local_get(crc)
            .i32_wrap_i64()
            .i32_const(1)
            .i32_and()
            .select()
            .i64_xor()
            .local_set(crc);
    }
    s.local_get(crc).i64_const(0xffff_ffff).i64_and();
    e.set(rd);
    e.next();
}

/// x30 = the address of the next instruction.
pub(crate) fn link_register<E: Emitter + ?Sized>(e: &mut E) {
    e.this_address(4);
    e.set(Reg::X(30));
}

impl Instruction {
    /// Instructions that only compute; their meaning above leaves `next` to the end of [`emit`].
    fn is_data_processing(&self) -> bool {
        use Instruction as I;
        matches!(
            self,
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
        )
    }
}
