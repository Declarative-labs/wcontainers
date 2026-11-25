//! The meaning of AdvSIMD data processing instructions, as WebAssembly SIMD.

use crate::Arm64;
use crate::instruction::*;
use crate::meaning::Emitter;
use wasm_encoder::InstructionSink;

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

/// Write the meaning of an AdvSIMD data processing instruction.
pub(crate) fn emit<E: Emitter<Arm64> + ?Sized>(e: &mut E, instruction: &Instruction) {
    use Instruction as I;
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
