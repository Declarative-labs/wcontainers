//! The meaning of AdvSIMD data processing instructions, as WebAssembly SIMD.

use crate::instruction::*;
use crate::meaning::Emitter;
use wasm_encoder::{InstructionSink, ValType};

/// A v128 with the low `bytes` bytes all ones.
fn low_bytes(bytes: u32) -> i128 {
    if bytes >= 16 {
        -1
    } else {
        ((1u128 << (8 * bytes)) - 1) as i128
    }
}

/// Clear the top half of the v128 on the stack unless the whole register is used.
fn narrow(s: &mut InstructionSink<'_>, whole: bool) {
    if !whole {
        s.v128_const(low_bytes(8)).v128_and();
    }
}

/// The shuffle that makes every element a copy of element `index` of `element` bytes.
fn broadcast(element: u8, index: u8) -> [u8; 16] {
    core::array::from_fn(|i| index * element + i as u8 % element)
}

/// Elementwise operation of two v128s on the stack, for the operations that need no locals.
fn lanewise(s: &mut InstructionSink<'_>, op: VectorBinary, element: u8) {
    use VectorBinary as B;
    match (op, element) {
        (B::And, _) => s.v128_and(),
        (B::AndNot, _) => s.v128_andnot(),
        (B::Or, _) => s.v128_or(),
        (B::OrNot, _) => s.v128_not().v128_or(),
        (B::Xor, _) => s.v128_xor(),
        (B::Add, 1) => s.i8x16_add(),
        (B::Add, 2) => s.i16x8_add(),
        (B::Add, 4) => s.i32x4_add(),
        (B::Add, _) => s.i64x2_add(),
        (B::Sub, 1) => s.i8x16_sub(),
        (B::Sub, 2) => s.i16x8_sub(),
        (B::Sub, 4) => s.i32x4_sub(),
        (B::Sub, _) => s.i64x2_sub(),
        (B::Mul, 2) => s.i16x8_mul(),
        (B::Mul, 4) => s.i32x4_mul(),
        (B::Equal, 1) => s.i8x16_eq(),
        (B::Equal, 2) => s.i16x8_eq(),
        (B::Equal, 4) => s.i32x4_eq(),
        (B::Equal, _) => s.i64x2_eq(),
        (B::SignedGreater, 1) => s.i8x16_gt_s(),
        (B::SignedGreater, 2) => s.i16x8_gt_s(),
        (B::SignedGreater, 4) => s.i32x4_gt_s(),
        (B::SignedGreater, _) => s.i64x2_gt_s(),
        (B::SignedGreaterOrEqual, 1) => s.i8x16_ge_s(),
        (B::SignedGreaterOrEqual, 2) => s.i16x8_ge_s(),
        (B::SignedGreaterOrEqual, 4) => s.i32x4_ge_s(),
        (B::SignedGreaterOrEqual, _) => s.i64x2_ge_s(),
        (B::UnsignedGreater, 1) => s.i8x16_gt_u(),
        (B::UnsignedGreater, 2) => s.i16x8_gt_u(),
        (B::UnsignedGreater, 4) => s.i32x4_gt_u(),
        (B::UnsignedGreaterOrEqual, 1) => s.i8x16_ge_u(),
        (B::UnsignedGreaterOrEqual, 2) => s.i16x8_ge_u(),
        (B::UnsignedGreaterOrEqual, 4) => s.i32x4_ge_u(),
        (B::Test, 1) => s.v128_and().v128_const(0).i8x16_eq().v128_not(),
        (B::Test, 2) => s.v128_and().v128_const(0).i16x8_eq().v128_not(),
        (B::Test, 4) => s.v128_and().v128_const(0).i32x4_eq().v128_not(),
        (B::Test, _) => s.v128_and().v128_const(0).i64x2_eq().v128_not(),
        _ => unreachable!("{op:?} on {element} byte elements"),
    };
}

fn across(s: &mut InstructionSink<'_>, op: VectorAcross, element: u8) {
    use VectorAcross as A;
    match (op, element) {
        (A::Add, 1) => s.i8x16_add(),
        (A::Add, 2) => s.i16x8_add(),
        (A::Add, _) => s.i32x4_add(),
        (A::SignedMax, 1) => s.i8x16_max_s(),
        (A::SignedMax, 2) => s.i16x8_max_s(),
        (A::SignedMax, _) => s.i32x4_max_s(),
        (A::SignedMin, 1) => s.i8x16_min_s(),
        (A::SignedMin, 2) => s.i16x8_min_s(),
        (A::SignedMin, _) => s.i32x4_min_s(),
        (A::UnsignedMax, 1) => s.i8x16_max_u(),
        (A::UnsignedMax, 2) => s.i16x8_max_u(),
        (A::UnsignedMax, _) => s.i32x4_max_u(),
        (A::UnsignedMin, 1) => s.i8x16_min_u(),
        (A::UnsignedMin, 2) => s.i16x8_min_u(),
        (A::UnsignedMin, _) => s.i32x4_min_u(),
    };
}

/// Write the meaning of an AdvSIMD data processing instruction.
pub(crate) fn emit<E: Emitter + ?Sized>(e: &mut E, instruction: &Instruction) {
    use Instruction as I;
    use VectorBinary as B;
    match *instruction {
        I::VectorImmediate { op, rd, imm, whole } => {
            let high = if whole { imm } else { 0 };
            let value = (u128::from(high) << 64 | u128::from(imm)) as i128;
            if op == VectorImmediate::Move {
                e.sink().v128_const(value);
            } else {
                e.get_vector(rd);
                let mut s = e.sink();
                s.v128_const(value);
                if op == VectorImmediate::Or {
                    s.v128_or();
                } else {
                    s.v128_andnot();
                }
                narrow(&mut s, whole);
            }
            e.set_vector(rd);
        }
        I::VectorBinary {
            op,
            arrangement: Arrangement { element, whole },
            rd,
            rn,
            rm,
        } => {
            match op {
                // bitselect(a, b, c) takes a where c is set and b where it is clear.
                B::Select => {
                    e.get_vector(rn);
                    e.get_vector(rm);
                    e.get_vector(rd);
                    e.sink().v128_bitselect();
                }
                B::InsertIfTrue => {
                    e.get_vector(rn);
                    e.get_vector(rd);
                    e.get_vector(rm);
                    e.sink().v128_bitselect();
                }
                B::InsertIfFalse => {
                    e.get_vector(rd);
                    e.get_vector(rn);
                    e.get_vector(rm);
                    e.sink().v128_bitselect();
                }
                // No 8 bit multiply: multiply into 16 bits and keep the low bytes.
                B::Mul if element == 1 => {
                    let (a, b) = (e.local(ValType::V128), e.local(ValType::V128));
                    e.get_vector(rn);
                    e.sink().local_set(a);
                    e.get_vector(rm);
                    e.sink()
                        .local_set(b)
                        .local_get(a)
                        .local_get(b)
                        .i16x8_extmul_low_i8x16_u()
                        .local_get(a)
                        .local_get(b)
                        .i16x8_extmul_high_i8x16_u()
                        .i8x16_shuffle(core::array::from_fn(|i| 2 * i as u8));
                }
                // No unsigned 64 bit compare: flip the sign bits and compare signed.
                B::UnsignedGreater | B::UnsignedGreaterOrEqual if element == 8 => {
                    let sign = i64::MIN as u64 as u128;
                    let sign = (sign << 64 | sign) as i128;
                    e.get_vector(rn);
                    e.sink().v128_const(sign).v128_xor();
                    e.get_vector(rm);
                    let mut s = e.sink();
                    s.v128_const(sign).v128_xor();
                    if op == B::UnsignedGreater {
                        s.i64x2_gt_s();
                    } else {
                        s.i64x2_ge_s();
                    }
                }
                _ => {
                    e.get_vector(rn);
                    e.get_vector(rm);
                    lanewise(&mut e.sink(), op, element);
                }
            }
            narrow(&mut e.sink(), whole);
            e.set_vector(rd);
        }
        I::VectorUnary {
            op,
            arrangement: Arrangement { whole, .. },
            rd,
            rn,
        } => {
            e.get_vector(rn);
            let mut s = e.sink();
            match op {
                VectorUnary::Not => s.v128_not(),
                VectorUnary::CountBits => s.i8x16_popcnt(),
            };
            narrow(&mut s, whole);
            e.set_vector(rd);
        }
        I::VectorAcross {
            op,
            arrangement: Arrangement { element, whole },
            rd,
            rn,
        } => {
            // Fold the upper half of the elements still counted onto the lower half until one is left.
            let value = e.local(ValType::V128);
            e.get_vector(rn);
            let mut s = e.sink();
            s.local_set(value);
            let mut bytes = if whole { 16 } else { 8 };
            while bytes > element {
                bytes /= 2;
                let lanes = core::array::from_fn(|i| {
                    let from = i as u8 + bytes;
                    if from < 16 { from } else { 0 }
                });
                s.local_get(value)
                    .local_get(value)
                    .local_get(value)
                    .i8x16_shuffle(lanes);
                across(&mut s, op, element);
                s.local_set(value);
            }
            s.local_get(value)
                .v128_const(low_bytes(element.into()))
                .v128_and();
            e.set_vector(rd);
        }
        I::DuplicateGeneral {
            arrangement: Arrangement { element, whole },
            rd,
            rn,
        } => {
            e.get(rn);
            let mut s = e.sink();
            match element {
                1 => s.i32_wrap_i64().i8x16_splat(),
                2 => s.i32_wrap_i64().i16x8_splat(),
                4 => s.i32_wrap_i64().i32x4_splat(),
                _ => s.i64x2_splat(),
            };
            narrow(&mut s, whole);
            e.set_vector(rd);
        }
        I::DuplicateElement {
            arrangement: Arrangement { element, whole },
            index,
            rd,
            rn,
        } => {
            e.get_vector(rn);
            e.get_vector(rn);
            let mut s = e.sink();
            s.i8x16_shuffle(broadcast(element, index));
            narrow(&mut s, whole);
            e.set_vector(rd);
        }
        I::MoveToGeneral {
            bytes,
            index,
            signed,
            rd,
            rn,
        } => {
            e.get_vector(rn);
            let mut s = e.sink();
            match (bytes, signed) {
                (1, None) => s.i8x16_extract_lane_u(index).i64_extend_i32_u(),
                (1, Some(Width::W32)) => s.i8x16_extract_lane_s(index).i64_extend_i32_u(),
                (1, Some(Width::X64)) => s.i8x16_extract_lane_s(index).i64_extend_i32_s(),
                (2, None) => s.i16x8_extract_lane_u(index).i64_extend_i32_u(),
                (2, Some(Width::W32)) => s.i16x8_extract_lane_s(index).i64_extend_i32_u(),
                (2, Some(Width::X64)) => s.i16x8_extract_lane_s(index).i64_extend_i32_s(),
                (4, None) => s.i32x4_extract_lane(index).i64_extend_i32_u(),
                (4, Some(_)) => s.i32x4_extract_lane(index).i64_extend_i32_s(),
                _ => s.i64x2_extract_lane(index),
            };
            e.set(rd);
        }
        I::MoveFromGeneral { bytes, rd, rn } => {
            e.sink().v128_const(0);
            e.get(rn);
            let mut s = e.sink();
            if bytes == 4 {
                s.i32_wrap_i64().i32x4_replace_lane(0);
            } else {
                s.i64x2_replace_lane(0);
            }
            e.set_vector(rd);
        }
        I::InsertGeneral {
            bytes,
            index,
            rd,
            rn,
        } => {
            e.get_vector(rd);
            e.get(rn);
            let mut s = e.sink();
            match bytes {
                1 => s.i32_wrap_i64().i8x16_replace_lane(index),
                2 => s.i32_wrap_i64().i16x8_replace_lane(index),
                4 => s.i32_wrap_i64().i32x4_replace_lane(index),
                _ => s.i64x2_replace_lane(index),
            };
            e.set_vector(rd);
        }
        I::InsertElement {
            bytes,
            to,
            from,
            rd,
            rn,
        } => {
            e.get_vector(rd);
            e.get_vector(rn);
            let lanes = core::array::from_fn(|i| {
                let i = i as u8;
                if i / bytes == to {
                    16 + from * bytes + i % bytes
                } else {
                    i
                }
            });
            e.sink().i8x16_shuffle(lanes);
            e.set_vector(rd);
        }
        I::ExtractVector {
            whole,
            index,
            rd,
            rn,
            rm,
        } => {
            e.get_vector(rn);
            e.get_vector(rm);
            // Shuffle lanes 0 to 15 are rn's bytes and 16 to 31 rm's.
            let lanes = core::array::from_fn(|i| {
                let at = index + i as u8;
                match whole {
                    true => at,
                    false if i >= 8 => 0,
                    false if at < 8 => at,
                    false => at + 8,
                }
            });
            let mut s = e.sink();
            s.i8x16_shuffle(lanes);
            narrow(&mut s, whole);
            e.set_vector(rd);
        }
        _ => unreachable!("not AdvSIMD data processing: {instruction:?}"),
    }
    e.next();
}
