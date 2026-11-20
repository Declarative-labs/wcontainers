//! The meaning of floating point, scalar and AdvSIMD, as WebAssembly.
//!
//! Every operation has two paths. The fast one is WebAssembly's own arithmetic, which is exact for numbers
//! because FPCR is at its default (round to nearest, no flushing) whenever it runs; not-a-number results are
//! fixed to Arm's choice on hosts that need it ([`Host::arm_nans`]). The slow one hands each element to the
//! software floating point of [`crate::exact`] through [`Hook::Float`]; it runs when FPCR is not the
//! default or the program has asked for the exception flags ([`Control::FloatTracking`]), and for fused
//! multiply-add on hosts without a fused one.

use crate::Arm64;
use crate::exact::{DOUBLE, HALF, Operation, SINGLE, format_parts};
use crate::instruction::*;
use crate::meaning::{Emitter, Flag, Flags, Hook};
use crate::simd::{extract, replace};
use rustc_apfloat::ieee::{Double, Half, Single};
use wasm_encoder::{BlockType, InstructionSink, ValType};

/// The width of a 64 bit integer, as instructions give integer widths.
const LONG_BITS: u8 = u64::BITS as u8;
/// Bits in a byte, for sizes given in bytes.
const BYTE_BITS: u8 = u8::BITS as u8;

/// Push a value of `ty` from `slow` when the slow path must be taken, else from `fast`. Where the emitter
/// carries on elsewhere in that case, only `fast` is written here.
pub(crate) fn paths<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    ty: ValType,
    slow: impl FnOnce(&mut E),
    fast: impl FnOnce(&mut E),
) {
    if e.float_mode() {
        e.sink().if_(BlockType::Result(ty));
        slow(e);
        e.sink().else_();
        fast(e);
        e.sink().end();
    } else {
        fast(e);
    }
}

/// A v128 whose `element` byte elements all hold `bits`.
pub(crate) fn splat_bits(element: u8, bits: u64) -> i128 {
    let lanes = size_of::<u128>() as u8 / element;
    let mut v = 0u128;
    for i in 0..lanes {
        v |= u128::from(bits) << (u32::from(i) * u8::BITS * u32::from(element));
    }
    v as i128
}

/// The quiet bit, sign bit and default not-a-number of a size.
fn quiet_bit(size: u8) -> u64 {
    if size == DOUBLE { 1 << 51 } else { 1 << 22 }
}

fn default_nan(size: u8) -> u64 {
    if size == DOUBLE {
        0x7ff8_0000_0000_0000
    } else {
        0x7fc0_0000
    }
}

/// Lanewise float vector operations.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
    Abs,
    Neg,
    Sqrt,
    Ceil,
    Floor,
    Trunc,
    Nearest,
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
    /// Not a number, as a mask.
    IsNan,
}

pub(crate) fn op(s: &mut InstructionSink<'_>, op: Op, size: u8) {
    let double = size == DOUBLE;
    match (op, double) {
        (Op::Add, false) => s.f32x4_add(),
        (Op::Add, true) => s.f64x2_add(),
        (Op::Sub, false) => s.f32x4_sub(),
        (Op::Sub, true) => s.f64x2_sub(),
        (Op::Mul, false) => s.f32x4_mul(),
        (Op::Mul, true) => s.f64x2_mul(),
        (Op::Div, false) => s.f32x4_div(),
        (Op::Div, true) => s.f64x2_div(),
        (Op::Min, false) => s.f32x4_min(),
        (Op::Min, true) => s.f64x2_min(),
        (Op::Max, false) => s.f32x4_max(),
        (Op::Max, true) => s.f64x2_max(),
        (Op::Abs, false) => s.f32x4_abs(),
        (Op::Abs, true) => s.f64x2_abs(),
        (Op::Neg, false) => s.f32x4_neg(),
        (Op::Neg, true) => s.f64x2_neg(),
        (Op::Sqrt, false) => s.f32x4_sqrt(),
        (Op::Sqrt, true) => s.f64x2_sqrt(),
        (Op::Ceil, false) => s.f32x4_ceil(),
        (Op::Ceil, true) => s.f64x2_ceil(),
        (Op::Floor, false) => s.f32x4_floor(),
        (Op::Floor, true) => s.f64x2_floor(),
        (Op::Trunc, false) => s.f32x4_trunc(),
        (Op::Trunc, true) => s.f64x2_trunc(),
        (Op::Nearest, false) => s.f32x4_nearest(),
        (Op::Nearest, true) => s.f64x2_nearest(),
        (Op::Eq, false) => s.f32x4_eq(),
        (Op::Eq, true) => s.f64x2_eq(),
        (Op::Lt, false) => s.f32x4_lt(),
        (Op::Lt, true) => s.f64x2_lt(),
        (Op::Le, false) => s.f32x4_le(),
        (Op::Le, true) => s.f64x2_le(),
        (Op::Gt, false) => s.f32x4_gt(),
        (Op::Gt, true) => s.f64x2_gt(),
        (Op::Ge, false) => s.f32x4_ge(),
        (Op::Ge, true) => s.f64x2_ge(),
        (Op::IsNan, _) => {
            // x != x, with x already pushed twice by the caller.
            if double { s.f64x2_ne() } else { s.f32x4_ne() }
        }
    };
}

/// Push a mask of the lanes of `local` that are not-a-numbers.
fn is_nan<E: Emitter<Arm64> + ?Sized>(e: &mut E, size: u8, local: u32) {
    e.sink().local_get(local).local_get(local);
    op(&mut e.sink(), Op::IsNan, size);
}

/// Push the integer equality mask of two v128s at a size.
fn int_eq(s: &mut InstructionSink<'_>, size: u8) {
    if size == DOUBLE {
        s.i64x2_eq()
    } else {
        s.i32x4_eq()
    };
}

/// Push `result` with every not-a-number lane replaced by Arm's choice from `operands`, in Arm's order:
/// the first signaling one quieted, else the first quiet one, else the default.
fn arm_nans<E: Emitter<Arm64> + ?Sized>(e: &mut E, size: u8, result: u32, operands: &[u32]) {
    let quiet = splat_bits(size, quiet_bit(size));
    let chosen = e.local(ValType::V128);
    e.sink()
        .v128_const(splat_bits(size, default_nan(size)))
        .local_set(chosen);
    // Quiet ones, then signaling ones, each in reverse so the first wins.
    for signaling in [false, true] {
        for &operand in operands.iter().rev() {
            e.sink().local_get(operand).v128_const(quiet).v128_or();
            e.sink().local_get(chosen);
            is_nan(e, size, operand);
            if signaling {
                let mut s = e.sink();
                s.local_get(operand)
                    .v128_const(quiet)
                    .v128_and()
                    .v128_const(0);
                int_eq(&mut s, size);
                s.v128_and();
            }
            e.sink().v128_bitselect().local_set(chosen);
        }
    }
    e.sink().local_get(chosen).local_get(result);
    is_nan(e, size, result);
    e.sink().v128_bitselect();
}

/// An operand of a lane by lane operation: a v128 local, its element size and the lane of it the first
/// result lane takes.
#[derive(Clone, Copy)]
pub(crate) struct Source {
    pub local: u32,
    pub element: u8,
    pub first: u8,
}

impl Source {
    pub fn of(local: u32, element: u8) -> Self {
        Source {
            local,
            element,
            first: 0,
        }
    }
}

/// Push a v128 whose first `lanes` lanes of `element` bytes come from the software path, one hook call a
/// lane, and whose other lanes are zero. `finish` turns the hook's i64 into the lane's value.
pub(crate) fn by_hook<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    operation: Operation,
    size: u8,
    element: u8,
    lanes: u8,
    operands: &[Source],
    finish: &dyn Fn(&mut E),
) {
    e.sink().v128_const(0);
    for lane in 0..lanes {
        e.sink().i32_const(operation.encode(size));
        for operand in operands {
            e.sink().local_get(operand.local);
            extract(&mut e.sink(), operand.element, operand.first + lane, false);
        }
        for _ in operands.len()..crate::exact::OPERANDS {
            e.sink().i64_const(0);
        }
        e.hook(Hook::Float);
        finish(e);
        replace(&mut e.sink(), element, lane);
    }
}

/// Push the result of a float operation on `lanes` lanes: the slow path in exact mode (or always, for a
/// fused one on a host without), else `fast`, which pushes a v128 from the operands (in Arm's order), with
/// not-a-numbers fixed where the host needs it and `nans` says the result can be one.
pub(crate) fn lanes<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    operation: Operation,
    size: u8,
    lanes: u8,
    operands: &[u32],
    nans: bool,
    fast: &dyn Fn(&mut E),
) {
    let sources: Vec<Source> = operands.iter().map(|&l| Source::of(l, size)).collect();
    let host = e.host();
    if operation.fused() && !host.fused_multiply_add {
        by_hook(e, operation, size, size, lanes, &sources, &|_| {});
        return;
    }
    paths(
        e,
        ValType::V128,
        |e| by_hook(e, operation, size, size, lanes, &sources, &|_| {}),
        |e| {
            fast(e);
            if nans && !host.arm_nans {
                let result = e.local(ValType::V128);
                e.sink().local_set(result);
                arm_nans(e, size, result, operands);
            }
        },
    );
}

/// Push a v128 of `size` elements all `value`.
pub(crate) fn constant(s: &mut InstructionSink<'_>, size: u8, value: f64) {
    let bits = if size == DOUBLE {
        value.to_bits()
    } else {
        u64::from((value as f32).to_bits())
    };
    s.v128_const(splat_bits(size, bits));
}

/// Push the fused `addend + a * b` of three v128 locals.
fn fused<E: Emitter<Arm64> + ?Sized>(e: &mut E, size: u8, addend: u32, a: u32, b: u32) {
    let mut s = e.sink();
    s.local_get(a).local_get(b).local_get(addend);
    if size == DOUBLE {
        s.f64x2_relaxed_madd();
    } else {
        s.f32x4_relaxed_madd();
    }
}

/// Push a mask of lanes where one of two locals is infinite and the other zero.
fn infinity_times_zero<E: Emitter<Arm64> + ?Sized>(e: &mut E, size: u8, a: u32, b: u32) {
    for (x, y) in [(a, b), (b, a)] {
        let mut s = e.sink();
        s.local_get(x);
        op(&mut s, Op::Abs, size);
        constant(&mut s, size, f64::INFINITY);
        op(&mut s, Op::Eq, size);
        s.local_get(y);
        constant(&mut s, size, 0.0);
        op(&mut s, Op::Eq, size);
        s.v128_and();
    }
    e.sink().v128_or();
}

/// Push a v128 of the signs of `a * b` (the sign bits of `a` xor `b`).
fn product_sign<E: Emitter<Arm64> + ?Sized>(e: &mut E, size: u8, a: u32, b: u32) {
    let sign = splat_bits(size, 1 << (BYTE_BITS * size - 1));
    e.sink()
        .local_get(a)
        .local_get(b)
        .v128_xor()
        .v128_const(sign)
        .v128_and();
}

/// Round to an integer, ties away from zero, lane by lane: the integer part, plus one away from zero when
/// what is left is at least a half.
fn round_away<E: Emitter<Arm64> + ?Sized>(e: &mut E, size: u8, x: u32) {
    let t = e.local(ValType::V128);
    let sign = splat_bits(size, 1 << (BYTE_BITS * size - 1));
    let mut s = e.sink();
    s.local_get(x);
    op(&mut s, Op::Trunc, size);
    s.local_tee(t);
    // t + copysign(1, x)
    constant(&mut s, size, 1.0);
    s.local_get(x).v128_const(sign).v128_and().v128_or();
    op(&mut s, Op::Add, size);
    s.local_get(t);
    s.local_get(x).local_get(t);
    op(&mut s, Op::Sub, size);
    op(&mut s, Op::Abs, size);
    constant(&mut s, size, 0.5);
    op(&mut s, Op::Ge, size);
    s.v128_bitselect();
}

/// Push the fast rounding of a v128 local to integral values.
pub(crate) fn round<E: Emitter<Arm64> + ?Sized>(e: &mut E, size: u8, x: u32, rounding: Rounding) {
    let o = match rounding {
        Rounding::Away => return round_away(e, size, x),
        Rounding::Nearest | Rounding::Current | Rounding::CurrentExact => Op::Nearest,
        Rounding::Floor => Op::Floor,
        Rounding::Ceiling => Op::Ceil,
        Rounding::Zero => Op::Trunc,
    };
    let mut s = e.sink();
    s.local_get(x);
    op(&mut s, o, size);
}

/// The operation of a binary float instruction, by the software path's name.
fn binary_operation(op: FloatBinary) -> Operation {
    match op {
        FloatBinary::Add => Operation::Add,
        FloatBinary::Sub => Operation::Sub,
        FloatBinary::Mul | FloatBinary::NegatedMul => Operation::Mul,
        FloatBinary::Div => Operation::Div,
        FloatBinary::Max => Operation::Max,
        FloatBinary::Min => Operation::Min,
        FloatBinary::MaxNumber => Operation::MaxNumber,
        FloatBinary::MinNumber => Operation::MinNumber,
    }
}

/// Push the fast lanewise result of a binary operation on two v128 locals.
pub(crate) fn binary_fast<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    operation: Operation,
    size: u8,
    a: u32,
    b: u32,
) {
    match operation {
        Operation::Add
        | Operation::Sub
        | Operation::Mul
        | Operation::Div
        | Operation::Max
        | Operation::Min => {
            let o = match operation {
                Operation::Add => Op::Add,
                Operation::Sub => Op::Sub,
                Operation::Mul => Op::Mul,
                Operation::Div => Op::Div,
                Operation::Max => Op::Max,
                _ => Op::Min,
            };
            let mut s = e.sink();
            s.local_get(a).local_get(b);
            op(&mut s, o, size);
        }
        Operation::MaxNumber | Operation::MinNumber => {
            // A quiet not-a-number against a number gives the number; otherwise as max and min.
            let quiet = splat_bits(size, quiet_bit(size));
            let o = if operation == Operation::MaxNumber {
                Op::Max
            } else {
                Op::Min
            };
            let mut s = e.sink();
            s.local_get(a).local_get(b);
            op(&mut s, o, size);
            for (x, y) in [(a, b), (b, a)] {
                let picked = e.local(ValType::V128);
                e.sink().local_set(picked).local_get(y).local_get(picked);
                // x quiet not-a-number and y a number
                is_nan(e, size, x);
                let mut s = e.sink();
                s.local_get(x)
                    .v128_const(quiet)
                    .v128_and()
                    .v128_const(quiet);
                int_eq(&mut s, size);
                s.v128_and();
                is_nan(e, size, y);
                e.sink().v128_andnot().v128_bitselect();
            }
        }
        Operation::MulExtended => {
            // As a product, but infinity times zero is two with the product's sign.
            let mut s = e.sink();
            constant(&mut s, size, 2.0);
            product_sign(e, size, a, b);
            e.sink().v128_or();
            let mut s = e.sink();
            s.local_get(a).local_get(b);
            op(&mut s, Op::Mul, size);
            infinity_times_zero(e, size, a, b);
            e.sink().v128_bitselect();
        }
        _ => unreachable!("not binary: {operation:?}"),
    }
}

/// Push the fast result of frecps (`2 - a * b`) or frsqrts (`(3 - a * b) / 2`) on two v128 locals, fused.
pub(crate) fn step_fast<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    size: u8,
    a: u32,
    b: u32,
    sqrt: bool,
) {
    let (negated, k, result) = (
        e.local(ValType::V128),
        e.local(ValType::V128),
        e.local(ValType::V128),
    );
    let mut s = e.sink();
    s.local_get(a);
    op(&mut s, Op::Neg, size);
    s.local_set(negated);
    constant(&mut s, size, if sqrt { 3.0 } else { 2.0 });
    s.local_set(k);
    fused(e, size, k, negated, b);
    if sqrt {
        // (3 - a * b) / 2: halving the fused sum is exact, unless the sum overflowed with b finite; then a is
        // large, and -a/2 * b + 3/2 is the same single rounding. With b infinite the infinite sum is exact.
        let (halved, alternative) = (e.local(ValType::V128), e.local(ValType::V128));
        let mut s = e.sink();
        constant(&mut s, size, 0.5);
        op(&mut s, Op::Mul, size);
        s.local_set(halved);
        s.local_get(negated);
        constant(&mut s, size, 0.5);
        op(&mut s, Op::Mul, size);
        s.local_set(alternative);
        constant(&mut s, size, 1.5);
        s.local_set(k);
        fused(e, size, k, alternative, b);
        let mut s = e.sink();
        s.local_get(halved);
        s.local_get(halved);
        op(&mut s, Op::Abs, size);
        constant(&mut s, size, f64::INFINITY);
        op(&mut s, Op::Eq, size);
        s.local_get(b);
        op(&mut s, Op::Abs, size);
        constant(&mut s, size, f64::INFINITY);
        op(&mut s, Op::Eq, size);
        s.v128_andnot();
        s.v128_bitselect();
    }
    let mut s = e.sink();
    s.local_set(result);
    // Infinity times zero gives the constant itself.
    constant(&mut s, size, if sqrt { 1.5 } else { 2.0 });
    s.local_get(result);
    infinity_times_zero(e, size, a, b);
    e.sink().v128_bitselect();
}

/// Write the meaning of a scalar floating point instruction.
pub(crate) fn emit<E: Emitter<Arm64> + ?Sized>(e: &mut E, instruction: &Instruction) {
    use Instruction as I;
    match *instruction {
        I::Float {
            op: o,
            size,
            rd,
            rn,
        } => {
            let x = vector(e, rn);
            match o {
                FloatUnary::Move => {
                    e.sink().local_get(x);
                }
                FloatUnary::Abs | FloatUnary::Negate => {
                    let mut s = e.sink();
                    s.local_get(x);
                    op(
                        &mut s,
                        if o == FloatUnary::Abs {
                            Op::Abs
                        } else {
                            Op::Neg
                        },
                        size,
                    );
                }
                FloatUnary::Sqrt => lanes(e, Operation::Sqrt, size, 1, &[x], true, &|e| {
                    let mut s = e.sink();
                    s.local_get(x);
                    op(&mut s, Op::Sqrt, size);
                }),
                FloatUnary::Round(r) => lanes(e, Operation::Round(r), size, 1, &[x], true, &|e| {
                    round(e, size, x, r)
                }),
            }
            low(e, size);
            e.set_vector(rd);
        }
        I::FloatConvert { from, to, rd, rn } => {
            let x = vector(e, rn);
            convert(e, x, from, to, 1, false, false);
            low(e, to);
            e.set_vector(rd);
        }
        I::FloatBinary {
            op: o,
            size,
            rd,
            rn,
            rm,
        } => {
            let (a, b) = (vector(e, rn), vector(e, rm));
            let operation = binary_operation(o);
            lanes(e, operation, size, 1, &[a, b], true, &|e| {
                binary_fast(e, operation, size, a, b)
            });
            if o == FloatBinary::NegatedMul {
                op(&mut e.sink(), Op::Neg, size);
            }
            low(e, size);
            e.set_vector(rd);
        }
        I::FloatMulAdd {
            negate_addend,
            negate_product,
            size,
            rd,
            rn,
            rm,
            ra,
        } => {
            let (n, m, a) = (vector(e, rn), vector(e, rm), vector(e, ra));
            for (local, negate) in [(a, negate_addend), (n, negate_product)] {
                if negate {
                    let mut s = e.sink();
                    s.local_get(local);
                    op(&mut s, Op::Neg, size);
                    s.local_set(local);
                }
            }
            lanes(e, Operation::MulAdd, size, 1, &[a, n, m], true, &|e| {
                fused(e, size, a, n, m)
            });
            low(e, size);
            e.set_vector(rd);
        }
        I::FloatCompare {
            signaling,
            size,
            rn,
            rm,
        } => {
            let a = vector(e, rn);
            let b = match rm {
                Some(rm) => vector(e, rm),
                None => {
                    let b = e.local(ValType::V128);
                    e.sink().v128_const(0).local_set(b);
                    b
                }
            };
            compare(e, signaling, size, a, b);
            let packed = e.local(ValType::I64);
            e.sink().local_set(packed);
            e.set_flags(Flags::Packed(packed));
        }
        I::FloatCondCompare {
            signaling,
            size,
            rn,
            rm,
            nzcv,
            cond,
        } => {
            let (a, b) = (vector(e, rn), vector(e, rm));
            e.condition(cond);
            e.sink().if_(BlockType::Result(ValType::I64));
            compare(e, signaling, size, a, b);
            e.sink()
                .else_()
                .i64_const(i64::from(nzcv) << Flag::V.bit())
                .end();
            let packed = e.local(ValType::I64);
            e.sink().local_set(packed);
            e.set_flags(Flags::Packed(packed));
        }
        I::FloatSelect {
            size,
            rd,
            rn,
            rm,
            cond,
        } => {
            e.get_vector(rn);
            e.get_vector(rm);
            e.condition(cond);
            e.sink().select();
            low(e, size);
            e.set_vector(rd);
        }
        I::FloatToInteger {
            size,
            width,
            signed,
            rounding,
            fraction_bits,
            rd,
            rn,
        } => {
            let x = vector(e, rn);
            let bits = width.bits() as u8;
            let operation = Operation::ToInteger {
                signed,
                bits,
                rounding,
                fraction_bits,
            };
            paths(
                e,
                ValType::I64,
                |e| {
                    e.sink().i32_const(operation.encode(size)).local_get(x);
                    extract(&mut e.sink(), size, 0, false);
                    e.sink().i64_const(0).i64_const(0);
                    e.hook(Hook::Float);
                },
                |e| to_integer_fast(e, x, 0, size, bits, signed, rounding, fraction_bits),
            );
            e.set(rd);
        }
        I::IntegerToFloat {
            size,
            width,
            signed,
            fraction_bits,
            rd,
            rn,
        } => {
            let bits = width.bits() as u8;
            let x = e.local(ValType::V128);
            e.sink().v128_const(0);
            e.get(rn);
            e.sink().i64x2_replace_lane(0).local_set(x);
            from_integer(e, x, size, bits, signed, fraction_bits, 1);
            low(e, size);
            e.set_vector(rd);
        }
        _ => unreachable!("not scalar floating point: {instruction:?}"),
    }
}

/// Read a vector register into a fresh local.
pub(crate) fn vector<E: Emitter<Arm64> + ?Sized>(e: &mut E, n: u8) -> u32 {
    let local = e.local(ValType::V128);
    e.get_vector(n);
    e.sink().local_set(local);
    local
}

/// Keep only the low `bytes` of the v128 on the stack.
pub(crate) fn low<E: Emitter<Arm64> + ?Sized>(e: &mut E, bytes: u8) {
    if usize::from(bytes) < size_of::<u128>() {
        let mask = if bytes == DOUBLE {
            u64::MAX as i128
        } else {
            ((1u128 << (BYTE_BITS * bytes)) - 1) as i128
        };
        e.sink().v128_const(mask).v128_and();
    }
}

/// Push the packed NZCV (an i64) of comparing lane 0 of two v128 locals.
fn compare<E: Emitter<Arm64> + ?Sized>(e: &mut E, signaling: bool, size: u8, a: u32, b: u32) {
    let slow = |e: &mut E| {
        e.sink()
            .i32_const(Operation::Compare { signaling }.encode(size))
            .local_get(a);
        extract(&mut e.sink(), size, 0, false);
        e.sink().local_get(b);
        extract(&mut e.sink(), size, 0, false);
        e.sink().i64_const(0);
        e.hook(Hook::Float);
    };
    paths(e, ValType::I64, slow, |e| compare_fast(e, size, a, b));
}

fn compare_fast<E: Emitter<Arm64> + ?Sized>(e: &mut E, size: u8, a: u32, b: u32) {
    let mut s = e.sink();
    // N: less; Z: equal; C: greater, equal or unordered; V: unordered.
    let scalar = |s: &mut InstructionSink<'_>, x: u32| {
        s.local_get(x);
        if size == DOUBLE {
            s.f64x2_extract_lane(0);
        } else {
            s.f32x4_extract_lane(0);
        }
    };
    let cmp = |s: &mut InstructionSink<'_>, o: Op| {
        scalar(s, a);
        scalar(s, b);
        match (o, size) {
            (Op::Lt, DOUBLE) => s.f64_lt(),
            (Op::Lt, _) => s.f32_lt(),
            (Op::Eq, DOUBLE) => s.f64_eq(),
            (Op::Eq, _) => s.f32_eq(),
            (_, DOUBLE) => s.f64_ge(),
            (_, _) => s.f32_ge(),
        };
    };
    cmp(&mut s, Op::Lt);
    s.i32_const(Flag::N.bit() as i32).i32_shl();
    cmp(&mut s, Op::Eq);
    s.i32_const(Flag::Z.bit() as i32).i32_shl().i32_or();
    // C is clear only when less.
    cmp(&mut s, Op::Lt);
    s.i32_eqz()
        .i32_const(Flag::C.bit() as i32)
        .i32_shl()
        .i32_or();
    // V: neither less nor greater or equal.
    cmp(&mut s, Op::Lt);
    cmp(&mut s, Op::Ge);
    s.i32_or()
        .i32_eqz()
        .i32_const(Flag::V.bit() as i32)
        .i32_shl()
        .i32_or();
    s.i64_extend_i32_u();
}

/// The fast conversion of lane 0 of a v128 local to an integer, pushed as an i64 (the low `bits`
/// meaningful, zero extended).
#[allow(clippy::too_many_arguments)]
pub(crate) fn to_integer_fast<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    x: u32,
    lane: u8,
    size: u8,
    bits: u8,
    signed: bool,
    rounding: Rounding,
    fraction_bits: u8,
) {
    let scaled = e.local(ValType::V128);
    let mut s = e.sink();
    s.local_get(x);
    if fraction_bits > 0 {
        constant(&mut s, size, (fraction_bits as f64).exp2());
        op(&mut s, Op::Mul, size);
    }
    s.local_set(scaled);
    round(e, size, scaled, rounding);
    let mut s = e.sink();
    if size == DOUBLE {
        s.f64x2_extract_lane(lane);
    } else {
        s.f32x4_extract_lane(lane);
    }
    match (size == DOUBLE, bits == LONG_BITS, signed) {
        (true, true, true) => s.i64_trunc_sat_f64_s(),
        (true, true, false) => s.i64_trunc_sat_f64_u(),
        (true, false, true) => s.i32_trunc_sat_f64_s().i64_extend_i32_u(),
        (true, false, false) => s.i32_trunc_sat_f64_u().i64_extend_i32_u(),
        (false, true, true) => s.i64_trunc_sat_f32_s(),
        (false, true, false) => s.i64_trunc_sat_f32_u(),
        (false, false, true) => s.i32_trunc_sat_f32_s().i64_extend_i32_u(),
        (false, false, false) => s.i32_trunc_sat_f32_u().i64_extend_i32_u(),
    };
}

/// Push a v128 holding, in its first `lanes` lanes of `size` bytes, the integers (`bits` wide) of the same
/// lanes of `x` converted to floats and divided by 2 to the `fraction_bits`.
pub(crate) fn from_integer<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    x: u32,
    size: u8,
    bits: u8,
    signed: bool,
    fraction_bits: u8,
    lanes_count: u8,
) {
    let operation = Operation::FromInteger {
        signed,
        bits,
        fraction_bits,
    };
    let slow = |e: &mut E| {
        by_hook(
            e,
            operation,
            size,
            size,
            lanes_count,
            &[Source::of(x, bits / BYTE_BITS)],
            &|_| {},
        )
    };
    paths(e, ValType::V128, slow, |e| {
        from_integer_fast(e, x, size, bits, signed, fraction_bits, lanes_count)
    });
}

fn from_integer_fast<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    x: u32,
    size: u8,
    bits: u8,
    signed: bool,
    fraction_bits: u8,
    lanes_count: u8,
) {
    e.sink().v128_const(0);
    for lane in 0..lanes_count {
        let mut s = e.sink();
        s.local_get(x);
        if bits == LONG_BITS {
            s.i64x2_extract_lane(lane);
        } else {
            s.i32x4_extract_lane(lane);
        }
        match (size == DOUBLE, bits == LONG_BITS, signed) {
            (true, true, true) => s.f64_convert_i64_s(),
            (true, true, false) => s.f64_convert_i64_u(),
            (true, false, true) => s.f64_convert_i32_s(),
            (true, false, false) => s.f64_convert_i32_u(),
            (false, true, true) => s.f32_convert_i64_s(),
            (false, true, false) => s.f32_convert_i64_u(),
            (false, false, true) => s.f32_convert_i32_s(),
            (false, false, false) => s.f32_convert_i32_u(),
        };
        if fraction_bits > 0 {
            let scale = (-f64::from(fraction_bits)).exp2();
            if size == DOUBLE {
                s.f64_const(scale.into()).f64_mul();
            } else {
                s.f32_const((scale as f32).into()).f32_mul();
            }
        }
        if size == DOUBLE {
            s.f64x2_replace_lane(lane);
        } else {
            s.f32x4_replace_lane(lane);
        }
    }
}

/// Push the conversion of the first `lanes` lanes of `from` bytes in local `x` (starting at lane `first`
/// when `upper`) to `to` bytes each, packed from lane 0. Not-a-numbers keep their sign and the top of their
/// payload, quieted, on every host.
pub(crate) fn convert<E: Emitter<Arm64> + ?Sized>(
    e: &mut E,
    x: u32,
    from: u8,
    to: u8,
    lanes_count: u8,
    upper: bool,
    odd: bool,
) {
    let first = if upper { lanes_count } else { 0 };
    let operation = Operation::Convert { to, odd };
    let source = Source {
        local: x,
        element: from,
        first,
    };
    let slow = |e: &mut E| by_hook(e, operation, from, to, lanes_count, &[source], &|_| {});
    paths(e, ValType::V128, slow, |e| {
        let bits = e.local(ValType::I64);
        e.sink().v128_const(0);
        for lane in 0..lanes_count {
            e.sink().local_get(x);
            extract(&mut e.sink(), from, first + lane, false);
            e.sink().local_set(bits);
            convert_bits(e, bits, from, to, odd);
            replace(&mut e.sink(), to, lane);
        }
    });
}

/// Push the bits (an i64) of the float whose bits are in `bits`, converted from `from` to `to` bytes,
/// rounding to nearest.
fn convert_bits<E: Emitter<Arm64> + ?Sized>(e: &mut E, bits: u32, from: u8, to: u8, odd: bool) {
    let (from_fraction, from_exponent) = parts(from);
    let (to_fraction, to_exponent) = parts(to);
    let exponent_mask = (1i64 << from_exponent) - 1;
    // Not-a-number: sign, all ones exponent, quiet bit, and the payload's top bits.
    let nan = e.local(ValType::I64);
    let mut s = e.sink();
    s.local_get(bits)
        .i64_const(from_fraction as i64 + from_exponent as i64)
        .i64_shr_u()
        .i64_const(to_fraction as i64 + to_exponent as i64)
        .i64_shl();
    s.i64_const(((1i64 << to_exponent) - 1) << to_fraction | 1 << (to_fraction - 1))
        .i64_or();
    s.local_get(bits)
        .i64_const((1i64 << from_fraction) - 1)
        .i64_and();
    if to_fraction > from_fraction {
        s.i64_const((to_fraction - from_fraction) as i64).i64_shl();
    } else {
        s.i64_const((from_fraction - to_fraction) as i64)
            .i64_shr_u();
    }
    s.i64_or().local_set(nan);
    // Numbers.
    match (from, to) {
        (DOUBLE, SINGLE) if odd => to_single_odd(e, bits),
        (SINGLE, DOUBLE) => {
            let mut s = e.sink();
            s.local_get(bits)
                .i32_wrap_i64()
                .f32_reinterpret_i32()
                .f64_promote_f32()
                .i64_reinterpret_f64();
        }
        (DOUBLE, SINGLE) => {
            let mut s = e.sink();
            s.local_get(bits)
                .f64_reinterpret_i64()
                .f32_demote_f64()
                .i32_reinterpret_f32()
                .i64_extend_i32_u();
        }
        (HALF, _) => half_to(e, bits, to),
        (_, _) => to_half(e, bits, from),
    }
    // Is it a not-a-number?
    let mut s = e.sink();
    s.local_get(nan)
        .local_get(bits)
        .i64_const(from_fraction as i64)
        .i64_shr_u()
        .i64_const(exponent_mask)
        .i64_and()
        .i64_const(exponent_mask)
        .i64_eq()
        .local_get(bits)
        .i64_const((1i64 << from_fraction) - 1)
        .i64_and()
        .i64_const(0)
        .i64_ne()
        .i32_and()
        .i32_eqz()
        .select();
}

/// Fraction and exponent bits of a size.
fn parts(size: u8) -> (u32, u32) {
    match size {
        HALF => format_parts::<Half>(),
        SINGLE => format_parts::<Single>(),
        _ => format_parts::<Double>(),
    }
}

/// Push the bits of a half precision number in `bits` as `to` (4 or 8) bytes; exact. Infinities convert;
/// not-a-numbers are replaced by the caller.
fn half_to<E: Emitter<Arm64> + ?Sized>(e: &mut E, bits: u32, to: u8) {
    let magnitude = e.local(ValType::F32);
    let mut s = e.sink();
    let exponent = |s: &mut InstructionSink<'_>| {
        s.local_get(bits)
            .i64_const(10)
            .i64_shr_u()
            .i64_const(0x1f)
            .i64_and();
    };
    // (fraction | implicit one) * 2^(max(exponent, 1) - 25), in single precision, which holds every half
    // precision number exactly.
    s.local_get(bits).i64_const(0x3ff).i64_and();
    exponent(&mut s);
    s.i64_const(0)
        .i64_ne()
        .i64_extend_i32_u()
        .i64_const(10)
        .i64_shl()
        .i64_or();
    s.f32_convert_i64_u();
    exponent(&mut s);
    s.i64_const(1);
    exponent(&mut s);
    s.i64_const(1).i64_gt_u().select();
    s.i64_const(102)
        .i64_add()
        .i64_const(23)
        .i64_shl()
        .i32_wrap_i64()
        .f32_reinterpret_i32();
    s.f32_mul();
    // The top exponent is infinity.
    s.f32_const(f32::INFINITY.into());
    exponent(&mut s);
    s.i64_const(0x1f).i64_ne().select();
    s.local_tee(magnitude).f32_neg().local_get(magnitude);
    s.local_get(bits)
        .i64_const(0x8000)
        .i64_and()
        .i64_const(0)
        .i64_ne()
        .select();
    if to == SINGLE {
        s.i32_reinterpret_f32().i64_extend_i32_u();
    } else {
        s.f64_promote_f32().i64_reinterpret_f64();
    }
}

/// Push the bits of the single or double precision number in `bits` as half precision, rounded to nearest
/// with ties to even. Not-a-numbers are replaced by the caller.
fn to_half<E: Emitter<Arm64> + ?Sized>(e: &mut E, bits: u32, from: u8) {
    let [double, biased, shift, q] = [(); 4].map(|_| e.local(ValType::I64));
    let mut s = e.sink();
    s.local_get(bits);
    if from == SINGLE {
        // Exact in double precision.
        s.i32_wrap_i64()
            .f32_reinterpret_i32()
            .f64_promote_f32()
            .i64_reinterpret_f64();
    }
    s.local_set(double);
    // The half precision biased exponent.
    s.local_get(double)
        .i64_const(52)
        .i64_shr_u()
        .i64_const(0x7ff)
        .i64_and()
        .i64_const(1023 - 15)
        .i64_sub()
        .local_set(biased);
    // Bits to drop: 42 for a normal result, more for a denormal one, at most 63.
    s.i64_const(42)
        .i64_const(43)
        .local_get(biased)
        .i64_sub()
        .i64_const(63)
        .i64_const(43)
        .local_get(biased)
        .i64_sub()
        .i64_const(63)
        .i64_lt_s()
        .select()
        .local_get(biased)
        .i64_const(1)
        .i64_ge_s()
        .select()
        .local_set(shift);
    // The significand with its implicit one (none for a zero or denormal double).
    let significand = |s: &mut InstructionSink<'_>| {
        s.local_get(double)
            .i64_const((1 << 52) - 1)
            .i64_and()
            .local_get(double)
            .i64_const(52)
            .i64_shr_u()
            .i64_const(0x7ff)
            .i64_and()
            .i64_const(0)
            .i64_ne()
            .i64_extend_i32_u()
            .i64_const(52)
            .i64_shl()
            .i64_or();
    };
    significand(&mut s);
    s.local_get(shift).i64_shr_u().local_set(q);
    // Round up when what is dropped is more than a half, or exactly a half and q is odd.
    significand(&mut s);
    s.i64_const(1)
        .local_get(shift)
        .i64_shl()
        .i64_const(1)
        .i64_sub()
        .i64_and();
    let remainder = |s: &mut InstructionSink<'_>| {
        s.i64_const(1)
            .local_get(shift)
            .i64_const(1)
            .i64_sub()
            .i64_shl();
    };
    let r = e.local(ValType::I64);
    let mut s = e.sink();
    s.local_set(r);
    s.local_get(r);
    remainder(&mut s);
    s.i64_gt_u();
    s.local_get(r);
    remainder(&mut s);
    s.i64_eq()
        .local_get(q)
        .i32_wrap_i64()
        .i32_const(1)
        .i32_and()
        .i32_and()
        .i32_or()
        .i64_extend_i32_u()
        .local_get(q)
        .i64_add();
    // A normal result's exponent field; the significand's carry moves it up by itself.
    s.local_get(biased)
        .i64_const(10)
        .i64_shl()
        .i64_const(0x400)
        .i64_sub()
        .i64_const(0)
        .local_get(biased)
        .i64_const(1)
        .i64_ge_s()
        .select()
        .i64_add();
    // Too large: infinity.
    let rounded = e.local(ValType::I64);
    let mut s = e.sink();
    s.local_set(rounded)
        .i64_const(0x7c00)
        .local_get(rounded)
        .local_get(biased)
        .i64_const(31)
        .i64_ge_s()
        .select();
    s.local_get(double)
        .i64_const(63)
        .i64_shr_u()
        .i64_const(15)
        .i64_shl()
        .i64_or();
}

/// Push the bits of the double in `bits` rounded to odd as single precision (fcvtxn): rounded towards
/// zero, with the lowest bit set when anything was lost. Not-a-numbers are replaced by the caller.
fn to_single_odd<E: Emitter<Arm64> + ?Sized>(e: &mut E, bits: u32) {
    let (x, r) = (e.local(ValType::F64), e.local(ValType::F32));
    let mut s = e.sink();
    s.local_get(bits)
        .f64_reinterpret_i64()
        .local_tee(x)
        .f32_demote_f64()
        .local_set(r);
    // Rounded away from zero: step back towards zero.
    s.local_get(r)
        .i32_reinterpret_f32()
        .local_get(r)
        .f64_promote_f32()
        .f64_abs()
        .local_get(x)
        .f64_abs()
        .f64_gt()
        .i32_sub();
    // Inexact: set the lowest bit.
    s.local_get(r)
        .f64_promote_f32()
        .local_get(x)
        .f64_ne()
        .i32_or()
        .i64_extend_i32_u();
}
