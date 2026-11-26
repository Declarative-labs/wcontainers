//! F16C's conversions between half and single precision (Intel SDM Vol. 2, VCVTPH2PS, VCVTPS2PH). Making a half
//! single is exact, done in WebAssembly integer and float operations on the fast path; rounding to half
//! precision has no WebAssembly operation, so `vcvtps2ph` always goes to the float hook, a slow path counted
//! "half precision rounding in software" in MXCSR's default mode.

use crate::decode::Instruction;
use crate::exact::{Operation, Part, encode};
use crate::host::{QUIET32, splat32};
use crate::operand::*;
use crate::wide::{HALVES, halves, put};
use crate::{X86_64, XMM_BYTES};
use iced_x86::{Mnemonic, OpKind};
use rustc_apfloat::Float;
use rustc_apfloat::ieee::{Half, Single};
use wasm_encoder::{BlockType, ValType};
use wcontainers_guest_cpu::Emitter;

/// A half precision number's fraction bits, its sign and the bits of its magnitude.
const HALF_FRACTION: u32 = Half::PRECISION as u32 - 1;
const HALF_SIGN: u32 = 1 << (u16::BITS - 1);
const HALF_MAGNITUDE: u32 = HALF_SIGN - 1;
/// The magnitude of half precision's infinity: at or above it, infinity or not-a-number.
const HALF_INFINITY: u32 = HALF_MAGNITUDE & !((1 << HALF_FRACTION) - 1);
/// The magnitude's bits moved to single precision's places, then 2^(single's bias - half's) makes the value.
const FRACTION_SHIFT: u32 = Single::PRECISION as u32 - Half::PRECISION as u32;
const BIAS_SHIFT: i32 = Single::MAX_EXP - Half::MAX_EXP;

pub fn written(i: &Instruction) -> bool {
    crate::decode::claim(i.iced.code()) != crate::claims::CLAIM_NEVER
        && matches!(i.iced.mnemonic(), Mnemonic::Vcvtph2ps | Mnemonic::Vcvtps2ph)
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    let ins = &i.iced;
    let widening = m == Mnemonic::Vcvtph2ps;
    // A store is checked after the conversion, whose flags the CPU raises when the store faults (seen on the
    // reference).
    let check = |e: &mut E, at: u32, access| e.check(at, memory_bytes(i), check_kind(access), 0);
    let operand = memory_use(i).map(|access| {
        let at = address(e, i);
        if widening {
            check(e, at, access);
        }
        (at, access)
    });
    let at = operand.map(|(at, _)| at);
    let source = halves(e, i, 1, at);
    // A ymm register on either side makes it two lanes.
    let wide = (0..ins.op_count())
        .any(|n| ins.op_kind(n) == OpKind::Register && ymm(ins.op_register(n)).is_some());
    let count = if wide { HALVES } else { 1 };
    let parts: [u32; HALVES] = core::array::from_fn(|_| e.local(ValType::V128));
    for (h, &r) in parts.iter().enumerate() {
        let part = match (wide, h) {
            (false, _) => Part::Whole,
            (true, 0) => Part::LowLane,
            (true, _) => Part::HighLane,
        };
        if h >= count {
            e.sink().v128_const(0).local_set(r);
            continue;
        }
        // The lane's elements in the low 64 bits of `b`: halves 4h to 4h + 3 of the source, or its singles of
        // lane h.
        let b = if widening && h > 0 {
            let b = e.local(ValType::V128);
            e.sink()
                .local_get(source[0])
                .local_get(source[0])
                .i8x16_shuffle(core::array::from_fn(|k| {
                    (k as u32 % (XMM_BYTES / 2) + XMM_BYTES / 2) as u8
                }))
                .local_set(b);
            b
        } else if widening {
            source[0]
        } else {
            source[h]
        };
        let operation = if widening {
            Operation::FromHalf
        } else {
            Operation::ToHalf
        };
        let immediate = if widening { 0 } else { ins.immediate8() };
        let code = encode(operation, false, false, immediate, part);
        if widening && e.float_mode() {
            e.sink().if_(BlockType::Empty);
            crate::float::exact(e, code, [b, b], None, r);
            e.sink().else_();
            single(e, b, r);
            e.sink().end();
        } else if widening {
            single(e, b, r);
        } else {
            crate::float::exact(e, code, [b, b], None, r);
        }
    }
    if widening {
        put(e, i, None, parts);
    } else {
        // The two lanes' halves side by side, into the low 64 bits or the whole of operand 0.
        let r = e.local(ValType::V128);
        e.sink()
            .local_get(parts[0])
            .local_get(parts[1])
            .i8x16_shuffle(core::array::from_fn(|k| {
                let k = k as u32;
                (if k < XMM_BYTES / 2 {
                    k
                } else {
                    XMM_BYTES + k - XMM_BYTES / 2
                }) as u8
            }))
            .local_set(r);
        let zero = e.local(ValType::V128);
        e.sink().v128_const(0).local_set(zero);
        if ins.op_kind(0) == OpKind::Register {
            put(e, i, None, [r, zero]);
        } else {
            let (at, access) = operand.expect("a memory operand");
            check(e, at, access);
            let low = e.local(ValType::I64);
            e.sink().local_get(r).i64x2_extract_lane(0).local_set(low);
            if memory_bytes(i) == XMM_BYTES {
                store_vector_at(e, at, 0, r);
            } else {
                store(e, at, memory_bytes(i), low);
            }
        }
    }
}

/// The four halves of `b`'s low 64 bits as singles into `r`, exactly: a finite one's magnitude moved to
/// single precision's places and scaled by the difference of the biases, infinity and not-a-number (quieted)
/// by their bits.
fn single<E: Emitter<X86_64> + ?Sized>(e: &mut E, b: u32, r: u32) {
    let (wide, magnitude) = (e.local(ValType::V128), e.local(ValType::V128));
    let scale = 2f32.powi(BIAS_SHIFT);
    let mut s = e.sink();
    s.local_get(b)
        .i32x4_extend_low_i16x8_u()
        .local_tee(wide)
        .v128_const(splat32(HALF_MAGNITUDE))
        .v128_and()
        .local_set(magnitude);
    // Not-a-number or infinity: the exponent all ones, quiet when the fraction is not zero.
    s.local_get(magnitude)
        .i32_const(FRACTION_SHIFT as i32)
        .i32x4_shl()
        .v128_const(splat32(f32::INFINITY.to_bits()))
        .v128_or()
        .v128_const(splat32(QUIET32))
        .v128_const(0)
        .local_get(magnitude)
        .v128_const(splat32(HALF_INFINITY))
        .i32x4_gt_u()
        .v128_bitselect()
        .v128_or();
    s.local_get(magnitude)
        .i32_const(FRACTION_SHIFT as i32)
        .i32x4_shl()
        .v128_const(splat32(scale.to_bits()))
        .f32x4_mul();
    s.local_get(magnitude)
        .v128_const(splat32(HALF_INFINITY))
        .i32x4_ge_u()
        .v128_bitselect()
        .local_get(wide)
        .v128_const(splat32(HALF_SIGN))
        .v128_and()
        .i32_const((u32::BITS - u16::BITS) as i32)
        .i32x4_shl()
        .v128_or()
        .local_set(r);
}
