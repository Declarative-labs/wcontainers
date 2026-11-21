//! The floating point instructions of SSE to SSE4.1 on xmm registers, and MXCSR's own (plan 5.8). Each has a
//! fast path of WebAssembly vector operations, for MXCSR's default mode with its flags not kept, and the exact
//! path through the float hook (`crate::exact`) for the rest. The approximate reciprocals are one closed
//! formula on every path.

use crate::XMM_BYTES;
use crate::decode::Instruction;
use crate::exact::{DOT_PRODUCT_SHIFT, Operation, ROUND_FROM_MXCSR, encode};
use crate::flags::{Flag, Flags};
use crate::guest::{Control, Hook, Reason, Trap};
use crate::host::{DEFAULT32, DEFAULT64, QUIET32, QUIET64, splat32, splat64};
use crate::operand::*;
use crate::{CF, MXCSR_WRITABLE, PF, X86_64, ZF};
use iced_x86::{Mnemonic, OpKind};
use wasm_encoder::{BlockType, InstructionSink, ValType};
use wcontainers_guest_cpu::Emitter;

/// The fraction bits of each format, and the exponent bias of single precision.
const FRACTION32: u32 = f32::MANTISSA_DIGITS - 1;
const FRACTION64: u32 = f64::MANTISSA_DIGITS - 1;
const BIAS32: i32 = f32::MAX_EXP - 1;
const BIAS64: i64 = f64::MAX_EXP as i64 - 1;
const SIGN32: u32 = 1 << (u32::BITS - 1);
const EXPONENT32: u32 = f32::INFINITY.to_bits();
const EXPONENT64: u64 = f64::INFINITY.to_bits();
/// The bits of Intel's approximations, and how many of the operand's top fraction bits choose the
/// interval they are worked out at (Intel SDM Vol. 2 RCPPS, RSQRTPS: |relative error| <= 1.5 * 2^-12).
const ESTIMATE_BITS: u32 = 12;
const RCP_INDEX_BITS: u32 = 11;
const RSQRT_INDEX_BITS: u32 = 10;
/// `cmpps`'s predicates, the low three bits of the immediate, and `roundps`'s roundings, the low two (with
/// bit 2 for MXCSR's).
const PREDICATE_MASK: u8 = 7;
const PREDICATE_NEGATED: u8 = 1 << 2;
const UNORDERED: u8 = 3;
const ROUNDING_MASK: u8 = 3;

/// The operation, the element size (whether double), and whether scalar, of each instruction the hook runs.
fn form(m: Mnemonic) -> Option<(Operation, bool, bool)> {
    use Mnemonic::*;
    use Operation as O;
    Some(match m {
        Addps => (O::Add, false, false),
        Addpd => (O::Add, true, false),
        Addss => (O::Add, false, true),
        Addsd => (O::Add, true, true),
        Subps => (O::Sub, false, false),
        Subpd => (O::Sub, true, false),
        Subss => (O::Sub, false, true),
        Subsd => (O::Sub, true, true),
        Mulps => (O::Mul, false, false),
        Mulpd => (O::Mul, true, false),
        Mulss => (O::Mul, false, true),
        Mulsd => (O::Mul, true, true),
        Divps => (O::Div, false, false),
        Divpd => (O::Div, true, false),
        Divss => (O::Div, false, true),
        Divsd => (O::Div, true, true),
        Minps => (O::Min, false, false),
        Minpd => (O::Min, true, false),
        Minss => (O::Min, false, true),
        Minsd => (O::Min, true, true),
        Maxps => (O::Max, false, false),
        Maxpd => (O::Max, true, false),
        Maxss => (O::Max, false, true),
        Maxsd => (O::Max, true, true),
        Sqrtps => (O::Sqrt, false, false),
        Sqrtpd => (O::Sqrt, true, false),
        Sqrtss => (O::Sqrt, false, true),
        Sqrtsd => (O::Sqrt, true, true),
        Cmpps => (O::Compare, false, false),
        Cmppd => (O::Compare, true, false),
        Cmpss => (O::Compare, false, true),
        Cmpsd => (O::Compare, true, true),
        Comiss => (O::OrderedCompare, false, true),
        Comisd => (O::OrderedCompare, true, true),
        Ucomiss => (O::UnorderedCompare, false, true),
        Ucomisd => (O::UnorderedCompare, true, true),
        Addsubps => (O::AddSub, false, false),
        Addsubpd => (O::AddSub, true, false),
        Haddps => (O::HorizontalAdd, false, false),
        Haddpd => (O::HorizontalAdd, true, false),
        Hsubps => (O::HorizontalSub, false, false),
        Hsubpd => (O::HorizontalSub, true, false),
        Roundps => (O::Round, false, false),
        Roundpd => (O::Round, true, false),
        Roundss => (O::Round, false, true),
        Roundsd => (O::Round, true, true),
        Dpps => (O::DotProduct, false, false),
        Dppd => (O::DotProduct, true, false),
        // Conversions between the float sizes name the source's.
        Cvtps2pd => (O::Convert, false, false),
        Cvtpd2ps => (O::Convert, true, false),
        Cvtss2sd => (O::Convert, false, true),
        Cvtsd2ss => (O::Convert, true, true),
        // From integers, the result's; to integers, the source's. The integer width is fixed later.
        Cvtdq2ps => (O::FromInt32, false, false),
        Cvtdq2pd => (O::FromInt32, true, false),
        Cvtsi2ss => (O::FromInt32, false, true),
        Cvtsi2sd => (O::FromInt32, true, true),
        Cvtps2dq => (O::ToInt32, false, false),
        Cvtpd2dq => (O::ToInt32, true, false),
        Cvttps2dq => (O::TruncateToInt32, false, false),
        Cvttpd2dq => (O::TruncateToInt32, true, false),
        Cvtss2si => (O::ToInt32, false, true),
        Cvtsd2si => (O::ToInt32, true, true),
        Cvttss2si => (O::TruncateToInt32, false, true),
        Cvttsd2si => (O::TruncateToInt32, true, true),
        // MMX's: two 32-bit integers of an MMX register, or into one.
        Cvtpi2ps => (O::FromInt32, false, false),
        Cvtpi2pd => (O::FromInt32, true, false),
        Cvtps2pi => (O::ToInt32, false, false),
        Cvtpd2pi => (O::ToInt32, true, false),
        Cvttps2pi => (O::TruncateToInt32, false, false),
        Cvttpd2pi => (O::TruncateToInt32, true, false),
        _ => return None,
    })
}

/// Whether the instruction is one of these, on xmm registers, general registers and memory, and MMX
/// registers for MMX's conversions.
pub fn written(i: &Instruction) -> bool {
    use Mnemonic::*;
    let ins = &i.iced;
    let m = ins.mnemonic();
    let known = form(m).is_some()
        || matches!(
            m,
            Rcpps
                | Rcpss
                | Rsqrtps
                | Rsqrtss
                | Ldmxcsr
                | Stmxcsr
                | Prefetchnta
                | Prefetcht0
                | Prefetcht1
                | Prefetcht2
        );
    known
        && !crate::meaning::is_string(i)
        && (0..ins.op_count()).all(|n| match ins.op_kind(n) {
            OpKind::Register => {
                let r = ins.op_register(n);
                xmm(r).is_some() || general(r).is_some() || (mmx(r).is_some() && converts_mmx(m))
            }
            _ => true,
        })
}

/// MMX's conversions.
fn converts_mmx(m: Mnemonic) -> bool {
    use Mnemonic::*;
    matches!(
        m,
        Cvtpi2ps | Cvtpi2pd | Cvtps2pi | Cvtpd2pi | Cvttps2pi | Cvttpd2pi
    )
}

/// Whether this instruction's result may differ on another x86 CPU: Intel's approximations are not
/// architectural.
pub fn estimates(m: Mnemonic) -> bool {
    matches!(
        m,
        Mnemonic::Rcpps | Mnemonic::Rcpss | Mnemonic::Rsqrtps | Mnemonic::Rsqrtss
    )
}

/// The float type's operations, chosen by element size.
struct Lanes {
    double: bool,
}

impl Lanes {
    fn ne(&self, s: &mut InstructionSink<'_>) {
        if self.double {
            s.f64x2_ne()
        } else {
            s.f32x4_ne()
        };
    }
    fn quiet(&self) -> i128 {
        if self.double {
            splat64(QUIET64)
        } else {
            splat32(QUIET32)
        }
    }
    fn default_nan(&self) -> i128 {
        if self.double {
            splat64(DEFAULT64)
        } else {
            splat32(DEFAULT32)
        }
    }
    fn arithmetic(&self, s: &mut InstructionSink<'_>, op: Operation) {
        use Operation as O;
        match (op, self.double) {
            (O::Add, false) => s.f32x4_add(),
            (O::Add, true) => s.f64x2_add(),
            (O::Sub, false) => s.f32x4_sub(),
            (O::Sub, true) => s.f64x2_sub(),
            (O::Mul, false) => s.f32x4_mul(),
            (O::Mul, true) => s.f64x2_mul(),
            (O::Div, false) => s.f32x4_div(),
            (O::Div, true) => s.f64x2_div(),
            _ => unreachable!("{op:?} is not arithmetic"),
        };
    }
    /// The low element of `r` with the rest of `a`, on the stack.
    fn merge_low(&self, s: &mut InstructionSink<'_>, a: u32, r: u32) {
        let element = if self.double {
            wcontainers_guest_cpu::DOUBLE
        } else {
            wcontainers_guest_cpu::SINGLE
        };
        s.local_get(a)
            .local_get(r)
            .i8x16_shuffle(core::array::from_fn(|b| {
                let b = b as u8;
                if b < element { XMM_BYTES as u8 + b } else { b }
            }));
    }
}

/// Make the not-a-number lanes of `r` what x86 gives: the first operand's not-a-number quieted, else the
/// next one's, else the default one. Nothing when the host's engine already does.
fn nans<E: Emitter<X86_64> + ?Sized>(e: &mut E, lanes: &Lanes, r: u32, operands: &[u32]) {
    if e.host().x86_nans {
        return;
    }
    let pick = e.local(ValType::V128);
    let mut s = e.sink();
    s.v128_const(lanes.default_nan()).local_set(pick);
    for &op in operands.iter().rev() {
        s.local_get(op)
            .v128_const(lanes.quiet())
            .v128_or()
            .local_get(pick)
            .local_get(op)
            .local_get(op);
        lanes.ne(&mut s);
        s.v128_bitselect().local_set(pick);
    }
    s.local_get(pick).local_get(r).local_get(r).local_get(r);
    lanes.ne(&mut s);
    s.v128_bitselect().local_set(r);
}

/// x86's not-a-number for the lanes of `promoted`, single precision lanes 0 and 1 of `source` made double:
/// quiet, with the payload kept.
fn promoted_nans<E: Emitter<X86_64> + ?Sized>(e: &mut E, source: u32, promoted: u32) {
    if e.host().x86_nans {
        return;
    }
    let wide = e.local(ValType::V128);
    let mut s = e.sink();
    s.local_get(source)
        .i64x2_extend_low_i32x4_u()
        .local_set(wide);
    s.local_get(wide)
        .v128_const(splat64(SIGN32.into()))
        .v128_and()
        .i32_const(u32::BITS as i32)
        .i64x2_shl()
        .v128_const(splat64(EXPONENT64))
        .v128_or()
        .local_get(wide)
        .v128_const(splat64(u64::from(QUIET32 | (QUIET32 - 1))))
        .v128_and()
        .v128_const(splat64(QUIET32.into()))
        .v128_or()
        .i32_const((FRACTION64 - FRACTION32) as i32)
        .i64x2_shl()
        .v128_or()
        .local_get(promoted)
        .local_get(source)
        .local_get(source)
        .f32x4_ne()
        .i64x2_extend_low_i32x4_s()
        .v128_bitselect()
        .local_set(promoted);
}

/// The bytes that put 32-bit lanes 0 and 2 of a vector in lanes 0 and 1, and zero in the rest (from a
/// second vector of zeros).
fn narrow_lanes() -> [u8; XMM_BYTES as usize] {
    core::array::from_fn(|b| {
        let (lane, byte) = (
            b as u32 / u32::from(wcontainers_guest_cpu::SINGLE),
            b as u32 % u32::from(wcontainers_guest_cpu::SINGLE),
        );
        (if lane < 2 {
            lane * u32::from(wcontainers_guest_cpu::DOUBLE) + byte
        } else {
            XMM_BYTES + byte
        }) as u8
    })
}

/// x86's not-a-number for the lanes of `demoted`, double precision lanes of `source` made single.
fn demoted_nans<E: Emitter<X86_64> + ?Sized>(e: &mut E, source: u32, demoted: u32) {
    if e.host().x86_nans {
        return;
    }
    let mut s = e.sink();
    s.local_get(source)
        .i32_const(u32::BITS as i32)
        .i64x2_shr_u()
        .v128_const(splat64(SIGN32.into()))
        .v128_and()
        .v128_const(splat64(EXPONENT32.into()))
        .v128_or()
        .local_get(source)
        .v128_const(splat64(QUIET64 | (QUIET64 - 1)))
        .v128_and()
        .v128_const(splat64(QUIET64))
        .v128_or()
        .i32_const((FRACTION64 - FRACTION32) as i32)
        .i64x2_shr_u()
        .v128_or()
        .v128_const(0)
        .i8x16_shuffle(narrow_lanes())
        .local_get(demoted)
        .local_get(source)
        .local_get(source)
        .f64x2_ne()
        .v128_const(0)
        .i8x16_shuffle(narrow_lanes())
        .v128_bitselect()
        .local_set(demoted);
}

/// Intel's `rcpps` (or `rsqrtps`) of the four single precision lanes on the stack, by plan 5.8's formula:
/// the estimate at the middle of the operand's interval, rounded to 12 bits, in f64 arithmetic.
pub fn reciprocal(s: &mut InstructionSink<'_>, x: u32, exponent: u32, wide: u32, sqrt: bool) {
    let index_bits = if sqrt {
        RSQRT_INDEX_BITS
    } else {
        RCP_INDEX_BITS
    };
    let index = ((1u64 << index_bits) - 1) << (FRACTION32 - index_bits);
    let half_step = 1u64 << (FRACTION64 - index_bits - 1);
    let scale = f64::from(1u32 << (ESTIMATE_BITS + 1));
    let fraction_scale = f64::from(1u32 << (FRACTION32 - ESTIMATE_BITS));
    s.local_set(x);
    s.local_get(x)
        .i32_const(FRACTION32 as i32)
        .i32x4_shr_u()
        .v128_const(splat32(u32::from(u8::MAX)))
        .v128_and()
        .local_set(exponent);
    // Each half: the interval's middle as an f64 in [1, 2) (with rsqrt's odd exponents in [2, 4)), the
    // estimate rounded to whole units of 2^-13, then as the fraction field plus one unit of 2^23.
    for high in [false, true] {
        s.local_get(x);
        if high {
            s.i64x2_extend_high_i32x4_u();
        } else {
            s.i64x2_extend_low_i32x4_u();
        }
        s.local_set(wide);
        s.local_get(wide)
            .v128_const(splat64(index))
            .v128_and()
            .i32_const((FRACTION64 - FRACTION32) as i32)
            .i64x2_shl()
            .v128_const(splat64(half_step | (BIAS64 as u64) << FRACTION64))
            .v128_or();
        if sqrt {
            // 2^b, b the parity of the unbiased exponent: set when the exponent field is even.
            s.local_get(wide)
                .i32_const(FRACTION32 as i32)
                .i64x2_shr_u()
                .v128_const(splat64(1))
                .v128_and()
                .v128_const(splat64(1))
                .v128_xor()
                .i32_const(FRACTION64 as i32)
                .i64x2_shl()
                .i64x2_add()
                .f64x2_sqrt();
        }
        s.local_set(wide);
        s.v128_const(splat64(scale.to_bits()))
            .local_get(wide)
            .f64x2_div()
            .f64x2_nearest()
            .v128_const(splat64(fraction_scale.to_bits()))
            .f64x2_mul()
            .i32x4_trunc_sat_f64x2_s_zero();
    }
    // The two halves' units side by side.
    s.i8x16_shuffle(core::array::from_fn(|b| {
        let b = b as u32;
        (if b < u32::from(wcontainers_guest_cpu::DOUBLE) {
            b
        } else {
            XMM_BYTES + b - u32::from(wcontainers_guest_cpu::DOUBLE)
        }) as u8
    }));
    // The exponent field less one, shifted, plus the units (which carry the one back).
    if sqrt {
        // BIAS - 2 - (e - BIAS - b) / 2.
        s.v128_const(splat32((BIAS32 - 2) as u32))
            .local_get(exponent)
            .v128_const(splat32(BIAS32 as u32))
            .i32x4_sub()
            .local_get(exponent)
            .v128_const(splat32(1))
            .v128_and()
            .v128_const(splat32(1))
            .v128_xor()
            .i32x4_sub()
            .i32_const(1)
            .i32x4_shr_s()
            .i32x4_sub();
    } else {
        // 2 * BIAS - 2 - e.
        s.v128_const(splat32((2 * BIAS32 - 2) as u32))
            .local_get(exponent)
            .i32x4_sub();
    }
    s.i32_const(FRACTION32 as i32)
        .i32x4_shl()
        .i32x4_add()
        .local_get(x)
        .v128_const(splat32(SIGN32))
        .v128_and()
        .v128_or()
        .local_set(wide);
    // The special operands: each result where its lanes are set, the later ones over the earlier.
    let special = |s: &mut InstructionSink<'_>,
                   value: &dyn Fn(&mut InstructionSink<'_>),
                   lanes: &dyn Fn(&mut InstructionSink<'_>)| {
        value(s);
        s.local_get(wide);
        lanes(s);
        s.v128_bitselect().local_set(wide);
    };
    let sign = |s: &mut InstructionSink<'_>| {
        s.local_get(x).v128_const(splat32(SIGN32)).v128_and();
    };
    let signed_infinity = |s: &mut InstructionSink<'_>| {
        sign(s);
        s.v128_const(splat32(EXPONENT32)).v128_or();
    };
    let zero_or_denormal = |s: &mut InstructionSink<'_>| {
        s.local_get(exponent).v128_const(0).i32x4_eq();
    };
    if sqrt {
        // Negative numbers give the default not-a-number; zeros and denormals a signed infinity; +inf +0.
        special(
            s,
            &|s| {
                s.v128_const(splat32(DEFAULT32));
            },
            &|s| {
                s.local_get(x).v128_const(0).i32x4_lt_s();
            },
        );
        special(s, &signed_infinity, &zero_or_denormal);
        special(
            s,
            &|s| {
                s.v128_const(0);
            },
            &|s| {
                s.local_get(x).v128_const(splat32(EXPONENT32)).i32x4_eq();
            },
        );
    } else {
        // Results below the normal range are a signed zero; zeros and denormals give a signed infinity.
        special(s, &sign, &|s| {
            s.local_get(exponent)
                .v128_const(splat32((2 * BIAS32 - 1) as u32))
                .i32x4_ge_u();
        });
        special(s, &signed_infinity, &zero_or_denormal);
    }
    // Not-a-number: the operand quieted.
    special(
        s,
        &|s| {
            s.local_get(x).v128_const(splat32(QUIET32)).v128_or();
        },
        &|s| {
            s.local_get(x).local_get(x).f32x4_ne();
        },
    );
    s.local_get(wide);
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let ins = &i.iced;
    if matches!(m, Prefetchnta | Prefetcht0 | Prefetcht1 | Prefetcht2) {
        // A hint: it never faults, and there is no cache to fill.
        return;
    }
    // A conversion naming an MMX register meets a pending x87 exception first; one from memory does not look
    // (seen on the reference).
    if crate::simd::is_mmx(i) {
        crate::fpu::pending(e);
    }
    let mem = memory_use(i).map(|kind| {
        let at = address(e, i);
        let bytes = memory_bytes(i);
        if bytes == XMM_BYTES {
            crate::simd::aligned16(e, at);
        }
        e.check(at, bytes, check_kind(kind), 0);
        at
    });
    if matches!(m, Ldmxcsr | Stmxcsr) {
        mxcsr(e, m, mem.expect("a memory operand"));
        return;
    }
    let value = |e: &mut E, n: u32| -> u32 {
        let v = e.local(ValType::V128);
        match ins.op_kind(n) {
            OpKind::Register => match xmm(ins.op_register(n)) {
                Some(x) => e.get_vector(x),
                None if mmx(ins.op_register(n)).is_some() => {
                    e.sink().v128_const(0);
                    e.get(mmx_register(mmx(ins.op_register(n)).unwrap()));
                    e.sink().i64x2_replace_lane(0);
                }
                None => {
                    e.sink().v128_const(0);
                    get_general(e, general(ins.op_register(n)).unwrap());
                    e.sink().i64x2_replace_lane(0);
                }
            },
            _ => {
                let at = mem.unwrap();
                let bytes = memory_bytes(i);
                if bytes == XMM_BYTES {
                    e.sink().local_get(at).v128_load(memory());
                } else {
                    e.sink().v128_const(0);
                    load(e, at, bytes);
                    e.sink().i64x2_replace_lane(0);
                }
            }
        }
        e.sink().local_set(v);
        v
    };
    let destination = xmm(ins.op_register(0));
    let a = match destination {
        Some(x) => {
            let a = e.local(ValType::V128);
            e.get_vector(x);
            e.sink().local_set(a);
            a
        }
        None => {
            let a = e.local(ValType::V128);
            e.sink().v128_const(0).local_set(a);
            a
        }
    };
    let b = value(e, 1);
    // Of a single source, the two singles converted alone, so that the others raise no exception.
    if matches!(m, Cvtps2pi | Cvttps2pi) {
        e.sink()
            .local_get(b)
            .v128_const(i128::from(u64::MAX))
            .v128_and()
            .local_set(b);
    }
    let r = e.local(ValType::V128);
    if matches!(m, Rcpps | Rcpss | Rsqrtps | Rsqrtss) {
        let (x, exponent, wide) = (
            e.local(ValType::V128),
            e.local(ValType::V128),
            e.local(ValType::V128),
        );
        let mut s = e.sink();
        s.local_get(b);
        reciprocal(&mut s, x, exponent, wide, matches!(m, Rsqrtps | Rsqrtss));
        s.local_set(r);
        if matches!(m, Rcpss | Rsqrtss) {
            Lanes { double: false }.merge_low(&mut s, a, r);
            s.local_set(r);
        }
        e.sink().local_get(r);
        e.set_vector(destination.unwrap());
        return;
    }
    let (mut op, double, scalar) = form(m).expect("a float instruction");
    // The integer side's width: a general register's, or 32 bits.
    let integer = (0..ins.op_count())
        .find_map(|n| {
            (ins.op_kind(n) == OpKind::Register)
                .then(|| general(ins.op_register(n)))
                .flatten()
        })
        .map(|g| g.width)
        .or_else(|| (m == Cvtsi2ss || m == Cvtsi2sd).then(|| (memory_bytes(i) * u8::BITS) as u8));
    if integer == Some(W64) {
        op = match op {
            Operation::FromInt32 => Operation::FromInt64,
            Operation::ToInt32 => Operation::ToInt64,
            Operation::TruncateToInt32 => Operation::TruncateToInt64,
            op => op,
        };
    }
    let immediate = if ins.op_count() > 2 {
        ins.immediate8()
    } else {
        0
    };
    // A conversion naming an MMX register makes the change to the x87 side (one from memory does not)
    // before its own exceptions, after the slow path's test, which comes before anything is written.
    let mmx_form = crate::simd::is_mmx(i);
    let transition = |e: &mut E| {
        if mmx_form {
            crate::simd::mmx_transition(e, false);
        }
    };
    if e.float_mode() {
        e.sink().if_(BlockType::Empty);
        transition(e);
        exact(
            e,
            encode(op, double, scalar, immediate, destination.unwrap_or(0)),
            b,
            r,
        );
        e.sink().else_();
        transition(e);
        fast(e, op, double, scalar, immediate, a, b, r);
        e.sink().end();
    } else {
        transition(e);
        fast(e, op, double, scalar, immediate, a, b, r);
    }
    match op {
        Operation::OrderedCompare | Operation::UnorderedCompare => {
            let each = Flag::ALL.map(|_| e.local(ValType::I32));
            let mut s = e.sink();
            for (flag, &local) in Flag::ALL.iter().zip(&each) {
                let bit = match flag {
                    Flag::Z => ZF,
                    Flag::P => PF,
                    Flag::C => CF,
                    _ => 0,
                };
                s.local_get(r)
                    .i64x2_extract_lane(0)
                    .i64_const(bit as i64)
                    .i64_and()
                    .i64_const(0)
                    .i64_ne()
                    .local_set(local);
            }
            e.set_flags(Flags::Each(each));
        }
        _ => match destination {
            // `cvtpi2ps` keeps the destination's high half.
            Some(x) if m == Cvtpi2ps => {
                e.sink()
                    .local_get(r)
                    .local_get(a)
                    .i8x16_shuffle(core::array::from_fn(|b| {
                        let b = b as u8;
                        if b < XMM_BYTES as u8 / 2 {
                            b
                        } else {
                            XMM_BYTES as u8 + b
                        }
                    }));
                e.set_vector(x);
            }
            Some(x) => {
                e.sink().local_get(r);
                e.set_vector(x);
            }
            None if mmx(ins.op_register(0)).is_some() => {
                let n = mmx(ins.op_register(0)).unwrap();
                e.sink().local_get(r).i64x2_extract_lane(0);
                e.set(mmx_register(n));
                e.sink().i32_const(i32::from(u16::MAX));
                e.set_control(Control::X87High(n));
            }
            None => {
                let g = general(ins.op_register(0)).unwrap();
                let v = e.local(ValType::I64);
                e.sink().local_get(r).i64x2_extract_lane(0).local_set(v);
                set_general(e, g, v);
            }
        },
    }
}

/// The exact path: the hook does the instruction on the destination register and `b` into `r`, or stops the thread with the
/// unmasked exception it met.
fn exact<E: Emitter<X86_64> + ?Sized>(e: &mut E, code: i32, b: u32, r: u32) {
    let (low, high, exception) = (
        e.local(ValType::I64),
        e.local(ValType::I64),
        e.local(ValType::I64),
    );
    e.sink()
        .i32_const(code)
        .local_get(b)
        .i64x2_extract_lane(0)
        .local_get(b)
        .i64x2_extract_lane(1);
    e.hook(Hook::Float);
    e.sink().local_set(high).local_set(low);
    e.get_control(Control::FloatException);
    e.sink()
        .i64_extend_i32_u()
        .local_tee(exception)
        .i64_const(0)
        .i64_ne()
        .if_(BlockType::Empty);
    e.trap(Trap::FloatException(exception));
    e.sink()
        .end()
        .v128_const(0)
        .local_get(low)
        .i64x2_replace_lane(0)
        .local_get(high)
        .i64x2_replace_lane(1)
        .local_set(r);
}

/// The fast path, exact in MXCSR's default mode but for the sticky flags.
#[allow(clippy::too_many_arguments)]
fn fast<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    op: Operation,
    double: bool,
    scalar: bool,
    immediate: u8,
    a: u32,
    b: u32,
    r: u32,
) {
    use Operation as O;
    let lanes = Lanes { double };
    let element = if double {
        u32::from(wcontainers_guest_cpu::DOUBLE)
    } else {
        u32::from(wcontainers_guest_cpu::SINGLE)
    };
    match op {
        O::Add | O::Sub | O::Mul | O::Div => {
            let mut s = e.sink();
            s.local_get(a).local_get(b);
            lanes.arithmetic(&mut s, op);
            s.local_set(r);
            nans(e, &lanes, r, &[a, b]);
        }
        O::Sqrt => {
            let mut s = e.sink();
            s.local_get(b);
            if double {
                s.f64x2_sqrt()
            } else {
                s.f32x4_sqrt()
            };
            s.local_set(r);
            nans(e, &lanes, r, &[b]);
        }
        // x86's min and max give the second operand unless the first is strictly less (greater): WebAssembly's
        // pseudo-minimum and -maximum with the operands swapped.
        O::Min | O::Max => {
            let mut s = e.sink();
            s.local_get(b).local_get(a);
            match (op, double) {
                (O::Min, false) => s.f32x4_pmin(),
                (O::Min, true) => s.f64x2_pmin(),
                (_, false) => s.f32x4_pmax(),
                (_, true) => s.f64x2_pmax(),
            };
            s.local_set(r);
        }
        O::Compare => {
            let mut s = e.sink();
            let compare = |s: &mut InstructionSink<'_>, x: u32, y: u32, p: u8| {
                s.local_get(x).local_get(y);
                match (p, double) {
                    (0, false) => s.f32x4_eq(),
                    (0, true) => s.f64x2_eq(),
                    (1, false) => s.f32x4_lt(),
                    (1, true) => s.f64x2_lt(),
                    (2, false) => s.f32x4_le(),
                    (2, true) => s.f64x2_le(),
                    (_, false) => s.f32x4_ne(),
                    (_, true) => s.f64x2_ne(),
                };
            };
            // Equal, less, less or equal, unordered, and their negations.
            let p = immediate & PREDICATE_MASK;
            match p & !PREDICATE_NEGATED {
                UNORDERED => {
                    compare(&mut s, a, a, UNORDERED);
                    compare(&mut s, b, b, UNORDERED);
                    s.v128_or();
                }
                p => compare(&mut s, a, b, p),
            }
            if p & PREDICATE_NEGATED != 0 {
                s.v128_not();
            }
            s.local_set(r);
        }
        O::OrderedCompare | O::UnorderedCompare => {
            // ZF for equal or unordered, PF for unordered, CF for less or unordered.
            let (x, y) = (e.local(ValType::V128), e.local(ValType::V128));
            let mut s = e.sink();
            s.local_get(a).local_get(a);
            lanes.ne(&mut s);
            s.local_get(b).local_get(b);
            lanes.ne(&mut s);
            s.v128_or().local_set(x);
            s.local_get(a).local_get(b);
            if double {
                s.f64x2_eq()
            } else {
                s.f32x4_eq()
            };
            s.local_get(x).v128_or().local_set(y);
            s.v128_const(0)
                .local_get(y)
                .i64x2_extract_lane(0)
                .i64_const(ZF as i64)
                .i64_and()
                .local_get(x)
                .i64x2_extract_lane(0)
                .i64_const(PF as i64)
                .i64_and()
                .i64_or();
            s.local_get(a).local_get(b);
            if double {
                s.f64x2_lt()
            } else {
                s.f32x4_lt()
            };
            s.local_get(x)
                .v128_or()
                .i64x2_extract_lane(0)
                .i64_const(CF as i64)
                .i64_and()
                .i64_or()
                .i64x2_replace_lane(0)
                .local_set(r);
        }
        O::AddSub => {
            let (difference, sum) = (e.local(ValType::V128), e.local(ValType::V128));
            for (local, o) in [(difference, O::Sub), (sum, O::Add)] {
                let mut s = e.sink();
                s.local_get(a).local_get(b);
                lanes.arithmetic(&mut s, o);
                s.local_set(local);
                nans(e, &lanes, local, &[a, b]);
            }
            // Even elements from the difference, odd from the sum.
            e.sink()
                .local_get(difference)
                .local_get(sum)
                .i8x16_shuffle(core::array::from_fn(|byte| {
                    let byte = byte as u32;
                    (if (byte / element).is_multiple_of(2) {
                        byte
                    } else {
                        XMM_BYTES + byte
                    }) as u8
                }))
                .local_set(r);
        }
        O::HorizontalAdd | O::HorizontalSub => {
            // The first and second of each pair, from a then b.
            let (x, y) = (e.local(ValType::V128), e.local(ValType::V128));
            let per = XMM_BYTES / element;
            for (local, second) in [(x, 0), (y, 1)] {
                e.sink()
                    .local_get(a)
                    .local_get(b)
                    .i8x16_shuffle(core::array::from_fn(|byte| {
                        let byte = byte as u32;
                        let n = byte / element;
                        let pair = n % (per / 2);
                        let from = if n < per / 2 { 0 } else { XMM_BYTES };
                        (from + (2 * pair + second) * element + byte % element) as u8
                    }))
                    .local_set(local);
            }
            let mut s = e.sink();
            s.local_get(x).local_get(y);
            lanes.arithmetic(
                &mut s,
                if op == O::HorizontalAdd {
                    O::Add
                } else {
                    O::Sub
                },
            );
            s.local_set(r);
            nans(e, &lanes, r, &[x, y]);
        }
        O::Round => {
            let mut s = e.sink();
            s.local_get(b);
            let rounding = if immediate & ROUND_FROM_MXCSR != 0 {
                0
            } else {
                immediate & ROUNDING_MASK
            };
            match (rounding, double) {
                (0, false) => s.f32x4_nearest(),
                (0, true) => s.f64x2_nearest(),
                (1, false) => s.f32x4_floor(),
                (1, true) => s.f64x2_floor(),
                (2, false) => s.f32x4_ceil(),
                (2, true) => s.f64x2_ceil(),
                (_, false) => s.f32x4_trunc(),
                (_, true) => s.f64x2_trunc(),
            };
            s.local_set(r);
            nans(e, &lanes, r, &[b]);
        }
        O::DotProduct => {
            let per = XMM_BYTES / element;
            let products = u32::from(immediate) >> DOT_PRODUCT_SHIFT;
            let mask = |bits: u32| -> i128 {
                let mut v = 0u128;
                for n in 0..per {
                    if bits >> n & 1 != 0 {
                        v |= (u128::MAX >> (u128::BITS - element * u8::BITS))
                            << (n * element * u8::BITS);
                    }
                }
                v as i128
            };
            let (p, swapped) = (e.local(ValType::V128), e.local(ValType::V128));
            let mut s = e.sink();
            s.local_get(a).local_get(b);
            lanes.arithmetic(&mut s, O::Mul);
            s.local_set(p);
            nans(e, &lanes, p, &[a, b]);
            e.sink()
                .local_get(p)
                .v128_const(mask(products))
                .v128_and()
                .local_set(p);
            // Each lane sums with its neighbour, then (single precision) with the other pair's sum, its own
            // value the first operand, as the CPU does.
            let mut distance = element;
            while distance < XMM_BYTES {
                e.sink()
                    .local_get(p)
                    .local_get(p)
                    .i8x16_shuffle(core::array::from_fn(|byte| {
                        ((byte as u32) ^ distance) as u8
                    }))
                    .local_set(swapped);
                // The neighbour first in single precision's first step (seen on the reference).
                let order = if distance == u32::from(wcontainers_guest_cpu::SINGLE) {
                    [swapped, p]
                } else {
                    [p, swapped]
                };
                let mut s = e.sink();
                s.local_get(order[0]).local_get(order[1]);
                lanes.arithmetic(&mut s, O::Add);
                s.local_set(r);
                nans(e, &lanes, r, &order);
                e.sink().local_get(r).local_set(p);
                distance *= 2;
            }
            // Each lane's sum where the low half of the immediate names it.
            e.sink()
                .local_get(p)
                .v128_const(mask(u32::from(immediate)))
                .v128_and()
                .local_set(r);
        }
        O::Convert => {
            let mut s = e.sink();
            s.local_get(b);
            if double {
                s.f32x4_demote_f64x2_zero().local_set(r);
                demoted_nans(e, b, r);
            } else {
                s.f64x2_promote_low_f32x4().local_set(r);
                promoted_nans(e, b, r);
            }
            if scalar {
                let mut s = e.sink();
                Lanes { double: !double }.merge_low(&mut s, a, r);
                s.local_set(r);
            }
            return;
        }
        O::FromInt32 | O::FromInt64 if scalar => {
            let mut s = e.sink();
            s.local_get(b).i64x2_extract_lane(0);
            match (op == O::FromInt64, double) {
                (false, false) => s.i32_wrap_i64().f32_convert_i32_s(),
                (false, true) => s.i32_wrap_i64().f64_convert_i32_s(),
                (true, false) => s.f32_convert_i64_s(),
                (true, true) => s.f64_convert_i64_s(),
            };
            let v = e.local(if double { ValType::F64 } else { ValType::F32 });
            let mut s = e.sink();
            s.local_set(v).local_get(a).local_get(v);
            if double {
                s.f64x2_replace_lane(0);
            } else {
                s.f32x4_replace_lane(0);
            }
            s.local_set(r);
            return;
        }
        O::FromInt32 | O::FromInt64 => {
            let mut s = e.sink();
            s.local_get(b);
            if double {
                s.f64x2_convert_low_i32x4_s();
            } else {
                s.f32x4_convert_i32x4_s();
            }
            s.local_set(r);
        }
        O::ToInt32 | O::ToInt64 | O::TruncateToInt32 | O::TruncateToInt64 => {
            to_integer(e, op, double, scalar, b, r);
            return;
        }
    }
    if scalar && !matches!(op, O::OrderedCompare | O::UnorderedCompare) {
        let mut s = e.sink();
        lanes.merge_low(&mut s, a, r);
        s.local_set(r);
    }
}

/// Conversion to integers: rounded to nearest (MXCSR's default) or truncated, with x86's "integer
/// indefinite" (the lowest integer) for not-a-number and out of range where WebAssembly saturates.
fn to_integer<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    op: Operation,
    double: bool,
    scalar: bool,
    b: u32,
    r: u32,
) {
    use Operation as O;
    let truncate = matches!(op, O::TruncateToInt32 | O::TruncateToInt64);
    let wide = matches!(op, O::ToInt64 | O::TruncateToInt64);
    let bits = if wide { u64::BITS } else { u32::BITS };
    let limit = 2f64.powi(bits as i32 - 1);
    if scalar {
        let v = e.local(if double { ValType::F64 } else { ValType::F32 });
        let n = e.local(ValType::I64);
        let mut s = e.sink();
        s.local_get(b);
        if double {
            s.f64x2_extract_lane(0);
            if !truncate {
                s.f64_nearest();
            }
        } else {
            s.f32x4_extract_lane(0);
            if !truncate {
                s.f32_nearest();
            }
        }
        s.local_tee(v);
        if double {
            s.i64_trunc_sat_f64_s();
        } else {
            s.i64_trunc_sat_f32_s();
        }
        s.local_set(n);
        // In range: -limit <= v < limit (false for not-a-number).
        s.local_get(n).i64_const(1i64 << (bits - 1));
        if double {
            s.local_get(v)
                .f64_const((-limit).into())
                .f64_ge()
                .local_get(v)
                .f64_const(limit.into())
                .f64_lt()
                .i32_and();
        } else {
            s.local_get(v)
                .f32_const((-limit as f32).into())
                .f32_ge()
                .local_get(v)
                .f32_const((limit as f32).into())
                .f32_lt()
                .i32_and();
        }
        s.select();
        if !wide {
            s.i64_const(u32::MAX.into()).i64_and();
        }
        s.local_set(n);
        s.v128_const(0)
            .local_get(n)
            .i64x2_replace_lane(0)
            .local_set(r);
        return;
    }
    let v = e.local(ValType::V128);
    let mut s2 = e.sink();
    s2.local_get(b);
    if !truncate {
        if double {
            s2.f64x2_nearest()
        } else {
            s2.f32x4_nearest()
        };
    }
    s2.local_tee(v);
    if double {
        s2.i32x4_trunc_sat_f64x2_s_zero()
            .v128_const((u64::from(SIGN32) | u64::from(SIGN32) << u32::BITS).into())
            .local_get(v)
            .v128_const(splat64((-limit).to_bits()))
            .f64x2_ge()
            .local_get(v)
            .v128_const(splat64(limit.to_bits()))
            .f64x2_lt()
            .v128_and()
            .v128_const(0)
            .i8x16_shuffle(narrow_lanes())
            .v128_bitselect();
    } else {
        s2.i32x4_trunc_sat_f32x4_s()
            .v128_const(splat32(SIGN32))
            .local_get(v)
            .v128_const(splat32((-limit as f32).to_bits()))
            .f32x4_ge()
            .local_get(v)
            .v128_const(splat32((limit as f32).to_bits()))
            .f32x4_lt()
            .v128_and()
            .v128_bitselect();
    }
    s2.local_set(r);
}

/// `ldmxcsr`, the program's own write of MXCSR, which keeps its flags exact from then on, and `stmxcsr`,
/// whose read of the flags while they are not kept is counted.
fn mxcsr<E: Emitter<X86_64> + ?Sized>(e: &mut E, m: Mnemonic, at: u32) {
    let value = e.local(ValType::I64);
    if m == Mnemonic::Ldmxcsr {
        load(e, at, size_of::<u32>() as u32);
        e.sink()
            .local_tee(value)
            .i64_const(!i64::from(MXCSR_WRITABLE))
            .i64_and()
            .i64_const(0)
            .i64_ne()
            .if_(BlockType::Empty);
        e.trap(Trap::GeneralProtection);
        e.sink().end().local_get(value).i32_wrap_i64();
        e.set_control(Control::Mxcsr);
        e.sink().i32_const(1);
        e.set_control(Control::MxcsrTracking);
    } else {
        read_mxcsr(e, value);
        store(e, at, size_of::<u32>() as u32, value);
    }
}

/// MXCSR into the i64 local `value`, its exception flags as last written: a counted slow path while they
/// are not kept (plan 5.8).
pub(crate) fn read_mxcsr<E: Emitter<X86_64> + ?Sized>(e: &mut E, value: u32) {
    e.get_control(Control::MxcsrTracking);
    e.sink().i32_eqz().if_(BlockType::Empty);
    e.warn(Reason::MxcsrFlagsReadNotKept);
    e.sink().end();
    e.get_control(Control::Mxcsr);
    e.sink().i64_extend_i32_u().local_set(value);
}
