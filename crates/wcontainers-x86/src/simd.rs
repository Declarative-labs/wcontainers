//! The SSE2 integer and bitwise instructions on xmm registers, each one WebAssembly vector operation or a
//! shuffle: the moves, logic, adds, compares, interleaves, shuffles, shifts by an immediate and the masks.

use crate::X86_64;
use crate::decode::Instruction;
use crate::guest::Trap;
use crate::operand::*;
use iced_x86::{Mnemonic, OpKind};
use wasm_encoder::{BlockType, InstructionSink, ValType};
use wcontainers_guest_cpu::Emitter;

/// Whether the instruction is one of these, on xmm registers (not MMX).
pub fn written(i: &Instruction) -> bool {
    use Mnemonic::*;
    let ins = &i.iced;
    let m = ins.mnemonic();
    let known = matches!(
        m,
        Movaps
            | Movups
            | Movapd
            | Movupd
            | Movdqa
            | Movdqu
            | Movq
            | Movd
            | Pextrw
            | Pinsrw
            | Pmovmskb
            | Movmskps
            | Movmskpd
            | Pshufd
            | Pshuflw
            | Pshufhw
            | Shufps
            | Shufpd
            | Psrldq
            | Pslldq
            | Pandn
            | Andnps
            | Andnpd
            | Pmuludq
    ) || binary(m).is_some()
        || (shift(m).is_some() && ins.op_count() == 2 && ins.op_kind(1) == OpKind::Immediate8);
    known
        && (0..ins.op_count())
            .any(|n| ins.op_kind(n) == OpKind::Register && xmm(ins.op_register(n)).is_some())
        && (0..ins.op_count()).all(|n| match ins.op_kind(n) {
            OpKind::Register => {
                xmm(ins.op_register(n)).is_some() || general(ins.op_register(n)).is_some()
            }
            _ => true,
        })
}

/// The two-operand instructions, `op0 = op0 . op1`, as the WebAssembly operation on `[op0, op1]`.
fn binary(m: Mnemonic) -> Option<fn(&mut InstructionSink<'_>)> {
    use Mnemonic::*;
    fn interleave(s: &mut InstructionSink<'_>, unit: u8, high: bool) {
        let mut lanes = [0u8; 16];
        let start = if high { 8 } else { 0 };
        for (k, lane) in lanes.iter_mut().enumerate() {
            let k = k as u8;
            let element = k / (2 * unit);
            let byte = k % unit;
            let from_second = (k / unit) % 2 == 1;
            *lane = start + element * unit + byte + if from_second { 16 } else { 0 };
        }
        s.i8x16_shuffle(lanes);
    }
    Some(match m {
        Pand | Andps | Andpd => |s| {
            s.v128_and();
        },
        Por | Orps | Orpd => |s| {
            s.v128_or();
        },
        Pxor | Xorps | Xorpd => |s| {
            s.v128_xor();
        },
        Paddb => |s| {
            s.i8x16_add();
        },
        Paddw => |s| {
            s.i16x8_add();
        },
        Paddd => |s| {
            s.i32x4_add();
        },
        Paddq => |s| {
            s.i64x2_add();
        },
        Psubb => |s| {
            s.i8x16_sub();
        },
        Psubw => |s| {
            s.i16x8_sub();
        },
        Psubd => |s| {
            s.i32x4_sub();
        },
        Psubq => |s| {
            s.i64x2_sub();
        },
        Pcmpeqb => |s| {
            s.i8x16_eq();
        },
        Pcmpeqw => |s| {
            s.i16x8_eq();
        },
        Pcmpeqd => |s| {
            s.i32x4_eq();
        },
        Pcmpgtb => |s| {
            s.i8x16_gt_s();
        },
        Pcmpgtw => |s| {
            s.i16x8_gt_s();
        },
        Pcmpgtd => |s| {
            s.i32x4_gt_s();
        },
        Pminub => |s| {
            s.i8x16_min_u();
        },
        Pmaxub => |s| {
            s.i8x16_max_u();
        },
        Pminsw => |s| {
            s.i16x8_min_s();
        },
        Pmaxsw => |s| {
            s.i16x8_max_s();
        },
        Punpcklbw => |s| interleave(s, 1, false),
        Punpcklwd => |s| interleave(s, 2, false),
        Punpckldq | Unpcklps => |s| interleave(s, 4, false),
        Punpcklqdq | Unpcklpd => |s| interleave(s, 8, false),
        Punpckhbw => |s| interleave(s, 1, true),
        Punpckhwd => |s| interleave(s, 2, true),
        Punpckhdq | Unpckhps => |s| interleave(s, 4, true),
        Punpckhqdq | Unpckhpd => |s| interleave(s, 8, true),
        _ => return None,
    })
}

/// The shifts by an immediate: the lane width and the kind (0 right, 1 left, 2 right arithmetic).
fn shift(m: Mnemonic) -> Option<(u8, u8)> {
    use Mnemonic::*;
    Some(match m {
        Psrlw => (16, 0),
        Psrld => (32, 0),
        Psrlq => (64, 0),
        Psllw => (16, 1),
        Pslld => (32, 1),
        Psllq => (64, 1),
        Psraw => (16, 2),
        Psrad => (32, 2),
        _ => return None,
    })
}

/// Stop with general protection unless the address in `at` is a multiple of 16, as legacy SSE's aligned
/// moves and its other 16-byte memory operands require, before the access's own check.
fn aligned16<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32) {
    e.sink()
        .local_get(at)
        .i64_const(15)
        .i64_and()
        .i64_const(0)
        .i64_ne()
        .if_(BlockType::Empty);
    e.trap(Trap::GeneralProtection);
    e.sink().end();
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let ins = &i.iced;
    let mem = memory_use(i).map(|kind| {
        let at = address(e, i);
        let bytes = memory_bytes(i);
        if bytes == 16 && !matches!(m, Movups | Movupd | Movdqu) {
            aligned16(e, at);
        }
        e.check(at, bytes, check_kind(kind), 0);
        at
    });
    let register = |n: u32| xmm(ins.op_register(n));
    // Push operand `n` as a v128: a whole xmm register or 16 bytes of memory, or the low `bytes` of either
    // (or of a general register) zero-extended.
    let push = |e: &mut E, n: u32, bytes: u32| match ins.op_kind(n) {
        OpKind::Register => match register(n) {
            Some(x) if bytes == 16 => e.get_vector(x),
            Some(x) => {
                e.sink().v128_const(0);
                e.get_vector(x);
                e.sink().i64x2_extract_lane(0);
                if bytes == 4 {
                    e.sink().i64_const(0xffff_ffff).i64_and();
                }
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
            if bytes == 16 {
                e.sink().local_get(at).v128_load(memory());
            } else {
                e.sink().v128_const(0);
                load(e, at, bytes);
                e.sink().i64x2_replace_lane(0);
            }
        }
    };
    let immediate = (ins.op_count() > 1 && ins.op_kind(ins.op_count() - 1) == OpKind::Immediate8)
        .then(|| ins.immediate8());
    let value = e.local(ValType::V128);
    match m {
        Movq | Movd => push(e, 1, if m == Movq { 8 } else { 4 }),
        // ~op0 & op1.
        Pandn | Andnps | Andnpd => {
            push(e, 1, 16);
            e.get_vector(register(0).unwrap());
            e.sink().v128_andnot();
        }
        // The low halves of each 64-bit lane, multiplied to 64 bits.
        Pmuludq => {
            const LOW_HALVES: i128 = 0x0000_0000_ffff_ffff_0000_0000_ffff_ffff;
            e.get_vector(register(0).unwrap());
            e.sink().v128_const(LOW_HALVES).v128_and();
            push(e, 1, 16);
            e.sink().v128_const(LOW_HALVES).v128_and().i64x2_mul();
        }
        _ if binary(m).is_some() => {
            e.get_vector(register(0).unwrap());
            push(e, 1, 16);
            binary(m).unwrap()(&mut e.sink());
        }
        Pshufd | Pshuflw | Pshufhw => {
            let k = immediate.unwrap();
            let mut lanes: [u8; 16] = core::array::from_fn(|b| b as u8);
            for (n, lane) in lanes.iter_mut().enumerate() {
                let n = n as u8;
                *lane = match m {
                    Pshufd => 4 * ((k >> (2 * (n / 4))) & 3) + n % 4,
                    Pshuflw if n < 8 => 2 * ((k >> (2 * (n / 2))) & 3) + n % 2,
                    Pshufhw if n >= 8 => 8 + 2 * ((k >> (2 * ((n - 8) / 2))) & 3) + n % 2,
                    _ => n,
                };
            }
            push(e, 1, 16);
            e.sink()
                .local_tee(value)
                .local_get(value)
                .i8x16_shuffle(lanes);
        }
        Shufps | Shufpd => {
            let k = immediate.unwrap();
            let lanes: [u8; 16] = core::array::from_fn(|b| {
                let b = b as u8;
                if m == Shufps {
                    let n = b / 4;
                    4 * ((k >> (2 * n)) & 3) + b % 4 + if n >= 2 { 16 } else { 0 }
                } else {
                    let n = b / 8;
                    8 * ((k >> n) & 1) + b % 8 + if n == 1 { 16 } else { 0 }
                }
            });
            e.get_vector(register(0).unwrap());
            push(e, 1, 16);
            e.sink().i8x16_shuffle(lanes);
        }
        Psrldq | Pslldq => {
            let k = immediate.unwrap();
            // Byte b of the result is byte b + k (right) or b - k (left) of the register, else zero (lane 16
            // of the zero vector).
            let lanes: [u8; 16] = core::array::from_fn(|b| {
                let from = if m == Psrldq {
                    b as i32 + i32::from(k)
                } else {
                    b as i32 - i32::from(k)
                };
                if (0..16).contains(&from) {
                    from as u8
                } else {
                    16
                }
            });
            e.get_vector(register(0).unwrap());
            e.sink().v128_const(0).i8x16_shuffle(lanes);
        }
        _ if shift(m).is_some() => {
            let (lane, kind) = shift(m).unwrap();
            let k = u32::from(immediate.unwrap());
            e.get_vector(register(0).unwrap());
            let mut s = e.sink();
            if k >= u32::from(lane) && kind != 2 {
                // Every bit shifted out.
                s.drop().v128_const(0);
            } else {
                s.i32_const(k.min(u32::from(lane) - 1) as i32);
                match (lane, kind) {
                    (16, 0) => s.i16x8_shr_u(),
                    (32, 0) => s.i32x4_shr_u(),
                    (64, 0) => s.i64x2_shr_u(),
                    (16, 1) => s.i16x8_shl(),
                    (32, 1) => s.i32x4_shl(),
                    (64, 1) => s.i64x2_shl(),
                    (16, _) => s.i16x8_shr_s(),
                    _ => s.i32x4_shr_s(),
                };
            }
        }
        Pinsrw => {
            e.get_vector(register(0).unwrap());
            match ins.op_kind(1) {
                OpKind::Register => get_general(e, general(ins.op_register(1)).unwrap()),
                _ => load(e, mem.unwrap(), 2),
            }
            e.sink()
                .i32_wrap_i64()
                .i16x8_replace_lane(immediate.unwrap() & 7);
        }
        // The moves, and the instructions writing a general register, which push their source here.
        _ => push(e, 1, 16),
    }
    e.sink().local_set(value);
    match m {
        Pmovmskb | Movmskps | Movmskpd | Pextrw => {
            let result = e.local(ValType::I64);
            let mut s = e.sink();
            s.local_get(value);
            match m {
                Pmovmskb => s.i8x16_bitmask(),
                Movmskps => s.i32x4_bitmask(),
                Movmskpd => s.i64x2_bitmask(),
                _ => s.i16x8_extract_lane_u(immediate.unwrap() & 7),
            };
            s.i64_extend_i32_u().local_set(result);
            set_general(e, general(ins.op_register(0)).unwrap(), result);
        }
        _ => match ins.op_kind(0) {
            OpKind::Register => match register(0) {
                Some(x) => {
                    e.sink().local_get(value);
                    e.set_vector(x);
                }
                None => {
                    let low = e.local(ValType::I64);
                    e.sink()
                        .local_get(value)
                        .i64x2_extract_lane(0)
                        .local_set(low);
                    set_general(e, general(ins.op_register(0)).unwrap(), low);
                }
            },
            _ => {
                let at = mem.unwrap();
                let bytes = memory_bytes(i);
                if bytes == 16 {
                    e.sink().local_get(at).local_get(value).v128_store(memory());
                } else {
                    let low = e.local(ValType::I64);
                    e.sink()
                        .local_get(value)
                        .i64x2_extract_lane(0)
                        .local_set(low);
                    store(e, at, bytes, low);
                }
            }
        },
    }
}
