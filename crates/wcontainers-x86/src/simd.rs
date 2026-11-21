//! The integer, bitwise and move instructions of SSE to SSE4.2 on xmm registers, each a few WebAssembly
//! vector operations or shuffles.

use crate::decode::Instruction;
use crate::flags::{Flag, Flags};
use crate::guest::{Control, Trap};
use crate::operand::*;
use crate::{X86_64, XMM_BYTES};
use iced_x86::{Mnemonic, OpKind, Register};
use wasm_encoder::{BlockType, InstructionSink, ValType};
use wcontainers_guest_cpu::{AccessKind, Emitter};

/// Whether the instruction is one of these, on xmm or MMX registers.
pub fn written(i: &Instruction) -> bool {
    use Mnemonic::*;
    let ins = &i.iced;
    let m = ins.mnemonic();
    let known = matches!(
        m,
        Pshufw
            | Movq2dq
            | Movdq2q
            | Movntq
            | Maskmovq
            | Movaps
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
            | Pmulhw
            | Pmulhuw
            | Psadbw
            | Pshufb
            | Pabsb
            | Pabsw
            | Pabsd
            | Psignb
            | Psignw
            | Psignd
            | Palignr
            | Phaddw
            | Phaddd
            | Phaddsw
            | Phsubw
            | Phsubd
            | Phsubsw
            | Pmaddubsw
            | Pmulhrsw
            | Pmuldq
            | Pmovsxbw
            | Pmovsxbd
            | Pmovsxbq
            | Pmovsxwd
            | Pmovsxwq
            | Pmovsxdq
            | Pmovzxbw
            | Pmovzxbd
            | Pmovzxbq
            | Pmovzxwd
            | Pmovzxwq
            | Pmovzxdq
            | Ptest
            | Pblendw
            | Blendps
            | Blendpd
            | Pblendvb
            | Blendvps
            | Blendvpd
            | Pextrb
            | Pextrd
            | Pextrq
            | Extractps
            | Pinsrb
            | Pinsrd
            | Pinsrq
            | Insertps
            | Phminposuw
            | Mpsadbw
            | Lddqu
            | Movntdqa
            | Movntdq
            | Movntps
            | Movntpd
            | Movddup
            | Movshdup
            | Movsldup
            | Movhlps
            | Movlhps
            | Movhps
            | Movhpd
            | Movlps
            | Movlpd
            | Movss
            | Movsd
    ) || binary(m).is_some()
        || shift(m).is_some();
    let vector = |r: Register| xmm(r).is_some() || mmx(r).is_some();
    known
        && (0..ins.op_count())
            .any(|n| ins.op_kind(n) == OpKind::Register && vector(ins.op_register(n)))
        && (0..ins.op_count()).all(|n| match ins.op_kind(n) {
            OpKind::Register => vector(ins.op_register(n)) || general(ins.op_register(n)).is_some(),
            _ => true,
        })
        && !(m == Maskmovq && !is_mmx(i))
}

/// Whether the instruction names an MMX register.
pub fn is_mmx(i: &Instruction) -> bool {
    let ins = &i.iced;
    (0..ins.op_count())
        .any(|n| ins.op_kind(n) == OpKind::Register && mmx(ins.op_register(n)).is_some())
}

/// How an MMX instruction's 64-bit operands go into the 128-bit vectors the xmm form's meaning works on:
/// each in the low half; both, destination first, in each operand, where the xmm form's lanes cross (its
/// result's low half then is the MMX result); each in both halves (`pshufb`); or each moved right by 32 bits,
/// for the forms that take the high halves, which then work as the low-half form.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MmxLanes {
    Low,
    Paired,
    Doubled,
    High,
}

fn mmx_lanes(m: Mnemonic) -> MmxLanes {
    use Mnemonic::*;
    match m {
        Packsswb | Packssdw | Packuswb | Phaddw | Phaddd | Phaddsw | Phsubw | Phsubd | Phsubsw => {
            MmxLanes::Paired
        }
        Pshufb => MmxLanes::Doubled,
        Punpckhbw | Punpckhwd | Punpckhdq => MmxLanes::High,
        _ => MmxLanes::Low,
    }
}

/// The form of the low halves that an MMX form of the high halves works as, and `pshufw` as `pshuflw`.
fn mmx_as(m: Mnemonic) -> Mnemonic {
    use Mnemonic::*;
    match m {
        Punpckhbw => Punpcklbw,
        Punpckhwd => Punpcklwd,
        Punpckhdq => Punpckldq,
        Pshufw => Pshuflw,
        _ => m,
    }
}

/// An MMX instruction's x87 side (plan 5.8): TOP 0 and every register valid, and `emms` every one empty.
pub(crate) fn mmx_transition<E: Emitter<X86_64> + ?Sized>(e: &mut E, empty: bool) {
    e.get_control(Control::X87StatusTag);
    let tags = if empty {
        0
    } else {
        i32::from(u8::MAX) << u16::BITS
    };
    e.sink()
        .i32_const(i32::from(!crate::x87::TOP))
        .i32_and()
        .i32_const(tags)
        .i32_or();
    e.set_control(Control::X87StatusTag);
}

/// TOP 0, the tags as they are.
fn mmx_top<E: Emitter<X86_64> + ?Sized>(e: &mut E) {
    e.get_control(Control::X87StatusTag);
    e.sink().i32_const(!i32::from(crate::x87::TOP)).i32_and();
    e.set_control(Control::X87StatusTag);
}

/// Write the low 64 bits of the vector in `value` to MMX register `n`, whose sign and exponent field becomes
/// all ones.
fn set_mmx<E: Emitter<X86_64> + ?Sized>(e: &mut E, n: u8, value: u32) {
    e.sink().local_get(value).i64x2_extract_lane(0);
    e.set(mmx_register(n));
    e.sink().i32_const(i32::from(u16::MAX));
    e.set_control(Control::X87High(n));
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
        Pminsb => |s| {
            s.i8x16_min_s();
        },
        Pmaxsb => |s| {
            s.i8x16_max_s();
        },
        Pminuw => |s| {
            s.i16x8_min_u();
        },
        Pmaxuw => |s| {
            s.i16x8_max_u();
        },
        Pminsd => |s| {
            s.i32x4_min_s();
        },
        Pmaxsd => |s| {
            s.i32x4_max_s();
        },
        Pminud => |s| {
            s.i32x4_min_u();
        },
        Pmaxud => |s| {
            s.i32x4_max_u();
        },
        Paddsb => |s| {
            s.i8x16_add_sat_s();
        },
        Paddsw => |s| {
            s.i16x8_add_sat_s();
        },
        Paddusb => |s| {
            s.i8x16_add_sat_u();
        },
        Paddusw => |s| {
            s.i16x8_add_sat_u();
        },
        Psubsb => |s| {
            s.i8x16_sub_sat_s();
        },
        Psubsw => |s| {
            s.i16x8_sub_sat_s();
        },
        Psubusb => |s| {
            s.i8x16_sub_sat_u();
        },
        Psubusw => |s| {
            s.i16x8_sub_sat_u();
        },
        Pavgb => |s| {
            s.i8x16_avgr_u();
        },
        Pavgw => |s| {
            s.i16x8_avgr_u();
        },
        Pmullw => |s| {
            s.i16x8_mul();
        },
        Pmulld => |s| {
            s.i32x4_mul();
        },
        // The sums of adjacent signed products, wrapping as the CPU does when both pairs are -2^15.
        Pmaddwd => |s| {
            s.i32x4_dot_i16x8_s();
        },
        // dst's elements narrowed into the low half, the source's into the high half.
        Packsswb => |s| {
            s.i8x16_narrow_i16x8_s();
        },
        Packuswb => |s| {
            s.i8x16_narrow_i16x8_u();
        },
        Packssdw => |s| {
            s.i16x8_narrow_i32x4_s();
        },
        Packusdw => |s| {
            s.i16x8_narrow_i32x4_u();
        },
        Pcmpeqq => |s| {
            s.i64x2_eq();
        },
        Pcmpgtq => |s| {
            s.i64x2_gt_s();
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

/// The bytes of a vector, of its halves, and of its elements.
const HALF: u8 = (XMM_BYTES / 2) as u8;
pub(crate) const BYTE: u32 = size_of::<u8>() as u32;
pub(crate) const WORD: u32 = size_of::<u16>() as u32;
pub(crate) const DWORD: u32 = size_of::<u32>() as u32;
const QWORD: u32 = size_of::<u64>() as u32;
/// `insertps`'s immediate: the source element (register form), the target element, the zero mask below.
const INSERTPS_SOURCE_SHIFT: u8 = 6;
const INSERTPS_TARGET_SHIFT: u8 = 4;
/// `mpsadbw`'s immediate: bit 2 picks dst's starting four bytes, bits 0 and 1 the source's four.
pub(crate) const MPSADBW_DESTINATION_SHIFT: u8 = 2;
/// The low 32 bits of each 64-bit lane.
const LOW_HALVES: i128 = 0x0000_0000_ffff_ffff_0000_0000_ffff_ffff;
/// The bits of a `pshufb` index that count: the zeroing top bit and the four of the byte's number.
const SWIZZLE_INDEX: u8 = 0x8f;

fn splat8(b: u8) -> i128 {
    i128::from_le_bytes([b; XMM_BYTES as usize])
}

fn splat16(h: i16) -> i128 {
    let [lo, hi] = h.to_le_bytes();
    i128::from_le_bytes(core::array::from_fn(|n| {
        if n.is_multiple_of(2) { lo } else { hi }
    }))
}

/// The bytes of one element of the instructions that name an element size.
fn element_bytes(i: &Instruction, m: Mnemonic) -> u32 {
    use Mnemonic::*;
    match m {
        Pextrb | Pinsrb => BYTE,
        Pblendw => WORD,
        Movss | Pextrd | Pinsrd | Extractps | Blendps => DWORD,
        Movsd | Movlps | Movlpd | Movhps | Movhpd | Movhlps | Movlhps | Pextrq | Pinsrq
        | Blendpd => HALF.into(),
        _ => memory_bytes(i),
    }
}

/// The element bytes a `pmovsx` or `pmovzx` reads and writes, and whether it extends the sign.
fn extension(m: Mnemonic) -> (u32, u32, bool) {
    use Mnemonic::*;
    match m {
        Pmovsxbw => (BYTE, WORD, true),
        Pmovsxbd => (BYTE, DWORD, true),
        Pmovsxbq => (BYTE, QWORD, true),
        Pmovsxwd => (WORD, DWORD, true),
        Pmovsxwq => (WORD, QWORD, true),
        Pmovsxdq => (DWORD, QWORD, true),
        Pmovzxbw => (BYTE, WORD, false),
        Pmovzxbd => (BYTE, DWORD, false),
        Pmovzxbq => (BYTE, QWORD, false),
        Pmovzxwd => (WORD, DWORD, false),
        Pmovzxwq => (WORD, QWORD, false),
        _ => (DWORD, QWORD, false),
    }
}

/// `ptest`: ZF when dst and the source share no bit, CF when the source has none outside dst; AF, OF, PF and
/// SF clear.
fn test<E: Emitter<X86_64> + ?Sized>(e: &mut E, dst: u8, source: impl Fn(&mut E)) {
    let (a, b) = (e.local(ValType::V128), e.local(ValType::V128));
    e.get_vector(dst);
    e.sink().local_set(a);
    source(e);
    e.sink().local_set(b);
    let each = Flag::ALL.map(|_| e.local(ValType::I32));
    let mut s = e.sink();
    for local in each {
        s.i32_const(0).local_set(local);
    }
    s.local_get(a)
        .local_get(b)
        .v128_and()
        .v128_any_true()
        .i32_eqz()
        .local_set(each[Flag::Z.index()]);
    s.local_get(b)
        .local_get(a)
        .v128_andnot()
        .v128_any_true()
        .i32_eqz()
        .local_set(each[Flag::C.index()]);
    e.set_flags(Flags::Each(each));
}

/// The shift of `shift`'s lane width and kind on the vector and count on the stack.
fn shift_lanes(s: &mut InstructionSink<'_>, lane: u8, kind: u8) {
    match (lane, kind) {
        (W16, 0) => s.i16x8_shr_u(),
        (W32, 0) => s.i32x4_shr_u(),
        (W64, 0) => s.i64x2_shr_u(),
        (W16, 1) => s.i16x8_shl(),
        (W32, 1) => s.i32x4_shl(),
        (W64, 1) => s.i64x2_shl(),
        (W16, _) => s.i16x8_shr_s(),
        _ => s.i32x4_shr_s(),
    };
}

/// Stop with general protection unless the address in `at` is a multiple of 16, as legacy SSE's aligned
/// moves and its other 16-byte memory operands require, before the access's own check.
pub(crate) fn aligned16<E: Emitter<X86_64> + ?Sized>(e: &mut E, at: u32) {
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
    let mmx_form = is_mmx(i);
    // An MMX instruction makes its change to the x87 side after the memory it reads; one that writes
    // memory sets TOP before the write may fault, the tags after it, but `maskmovq` makes all of it first
    // (seen on the reference).
    let reads_memory = memory_use(i) == Some(AccessKind::Read);
    let writes_memory = memory_use(i).is_some() && !reads_memory;
    if mmx_form {
        crate::fpu::pending(e);
        if writes_memory {
            mmx_top(e);
        } else if !reads_memory {
            mmx_transition(e, false);
        }
        if m == Maskmovq {
            masked_store_mmx(e, i);
            return;
        }
    }
    let lanes = mmx_lanes(m);
    let m = if mmx_form { mmx_as(m) } else { m };
    let mem = memory_use(i).map(|kind| {
        let at = address(e, i);
        let bytes = memory_bytes(i);
        if bytes == XMM_BYTES && !i.vex_lane && !matches!(m, Movups | Movupd | Movdqu | Lddqu) {
            aligned16(e, at);
        }
        e.check(at, bytes, check_kind(kind), 0);
        at
    });
    if mmx_form && reads_memory {
        mmx_transition(e, false);
    }
    let register = |n: u32| xmm(ins.op_register(n));
    let mm = |n: u32| {
        (ins.op_kind(n) == OpKind::Register)
            .then(|| mmx(ins.op_register(n)))
            .flatten()
    };
    // An MMX operand's 64 bits, from its register or memory, `bytes` of them zero-extended.
    let mmx_value = |e: &mut E, n: u32, bytes: u32| {
        match mm(n) {
            Some(r) => e.get(mmx_register(r)),
            None => load(e, mem.unwrap(), memory_bytes(i).min(bytes)),
        }
        if bytes < QWORD && mm(n).is_some() {
            e.sink().i64_const(mask((bytes * u8::BITS) as u8)).i64_and();
        }
    };
    // An MMX instruction's operand `n` as the vector the xmm form's meaning takes.
    let mmx_vector = |e: &mut E, n: u32, bytes: u32| match lanes {
        MmxLanes::Low => {
            e.sink().v128_const(0);
            mmx_value(e, n, bytes);
            e.sink().i64x2_replace_lane(0);
        }
        MmxLanes::Paired => {
            e.sink().v128_const(0);
            mmx_value(e, 0, QWORD);
            e.sink().i64x2_replace_lane(0);
            mmx_value(e, 1, QWORD);
            e.sink().i64x2_replace_lane(1);
        }
        MmxLanes::Doubled => {
            mmx_value(e, n, QWORD);
            e.sink().i64x2_splat();
        }
        MmxLanes::High => {
            e.sink().v128_const(0);
            mmx_value(e, n, QWORD);
            e.sink()
                .i64_const(u32::BITS.into())
                .i64_shr_u()
                .i64x2_replace_lane(0);
        }
    };
    let dest = |e: &mut E| match register(0) {
        Some(x) => e.get_vector(x),
        None => mmx_vector(e, 0, QWORD),
    };
    // Push operand `n` as a v128: a whole xmm register or 16 bytes of memory, or the low `bytes` of either
    // (or of a general register) zero-extended; an MMX instruction's as `mmx_vector`.
    let push = |e: &mut E, n: u32, bytes: u32| match ins.op_kind(n) {
        _ if mm(n).is_some() || (mmx_form && ins.op_kind(n) == OpKind::Memory) => {
            mmx_vector(e, n, bytes)
        }
        OpKind::Register => match register(n) {
            Some(x) if bytes == XMM_BYTES => e.get_vector(x),
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
            if bytes == XMM_BYTES {
                e.sink().local_get(at).v128_load(memory());
            } else {
                e.sink().v128_const(0);
                load(e, at, bytes);
                e.sink().i64x2_replace_lane(0);
            }
        }
    };
    let immediate = (ins.op_count() > 1 && ins.op_kind(ins.op_count() - 1) == OpKind::Immediate8)
        .then(|| ins.immediate8())
        // An MMX register's four words.
        .map(|k| match m {
            Pextrw | Pinsrw if mmx_form => k & (QWORD / WORD - 1) as u8,
            _ => k,
        });
    let value = e.local(ValType::V128);
    if m == Ptest {
        test(e, register(0).unwrap(), |e| push(e, 1, XMM_BYTES));
        return;
    }
    let bytes = element_bytes(i, m);
    match m {
        Movq | Movd => push(e, 1, if m == Movq { 8 } else { 4 }),
        // Moves of the low or high half or element, keeping the rest of a register destination.
        Movss | Movsd | Movlps | Movlpd | Movhps | Movhpd | Movhlps | Movlhps
            if ins.op_kind(0) == OpKind::Register =>
        {
            let (from, to) = match m {
                Movhps | Movhpd | Movlhps => (0, HALF),
                Movhlps => (HALF, 0),
                _ => (0, 0),
            };
            if matches!(m, Movss | Movsd) && ins.op_kind(1) != OpKind::Register {
                // From memory the rest is cleared.
                push(e, 1, bytes);
            } else {
                dest(e);
                let whole = ins.op_kind(1) == OpKind::Register;
                push(e, 1, if whole { XMM_BYTES } else { bytes });
                let lanes: [u8; 16] = core::array::from_fn(|b| {
                    let b = b as u8;
                    if (to..to + bytes as u8).contains(&b) {
                        XMM_BYTES as u8 + from + b - to
                    } else {
                        b
                    }
                });
                e.sink().i8x16_shuffle(lanes);
            }
        }
        // A store of the high half.
        Movhps | Movhpd => {
            push(e, 1, XMM_BYTES);
            e.sink()
                .local_tee(value)
                .local_get(value)
                .i8x16_shuffle(core::array::from_fn(|b| (b as u8 + HALF) % XMM_BYTES as u8));
        }
        Movddup | Movshdup | Movsldup => {
            push(e, 1, if m == Movddup { HALF.into() } else { XMM_BYTES });
            let lanes: [u8; 16] = core::array::from_fn(|b| {
                let b = b as u8;
                match m {
                    Movddup => b % HALF,
                    Movshdup => (b / QWORD as u8) * QWORD as u8 + DWORD as u8 + b % DWORD as u8,
                    _ => (b / QWORD as u8) * QWORD as u8 + b % DWORD as u8,
                }
            });
            e.sink()
                .local_tee(value)
                .local_get(value)
                .i8x16_shuffle(lanes);
        }
        // The high halves of the 32-bit products.
        Pmulhw | Pmulhuw => {
            let (a, b) = (e.local(ValType::V128), e.local(ValType::V128));
            dest(e);
            e.sink().local_set(a);
            push(e, 1, XMM_BYTES);
            e.sink().local_set(b);
            let mut s = e.sink();
            s.local_get(a).local_get(b);
            if m == Pmulhw {
                s.i32x4_extmul_low_i16x8_s();
            } else {
                s.i32x4_extmul_low_i16x8_u();
            }
            s.local_get(a).local_get(b);
            if m == Pmulhw {
                s.i32x4_extmul_high_i16x8_s();
            } else {
                s.i32x4_extmul_high_i16x8_u();
            }
            s.i8x16_shuffle(core::array::from_fn(|b| {
                let b = b as u32;
                ((b / WORD) * DWORD + WORD + b % WORD) as u8
            }));
        }
        // |dst - src| of each byte, summed in each 64-bit half.
        Psadbw => {
            let (a, b) = (e.local(ValType::V128), e.local(ValType::V128));
            dest(e);
            e.sink().local_set(a);
            push(e, 1, XMM_BYTES);
            e.sink()
                .local_set(b)
                .local_get(a)
                .local_get(b)
                .i8x16_max_u()
                .local_get(a)
                .local_get(b)
                .i8x16_min_u()
                .i8x16_sub()
                .i16x8_extadd_pairwise_i8x16_u()
                .i32x4_extadd_pairwise_i16x8_u()
                .local_tee(a)
                .v128_const(LOW_HALVES)
                .v128_and()
                .local_get(a)
                .i32_const(u32::BITS as i32)
                .i64x2_shr_u()
                .i64x2_add();
        }
        // Each byte from dst's byte the source's byte names, or zero where its top bit is set.
        Pshufb => {
            dest(e);
            push(e, 1, XMM_BYTES);
            e.sink()
                .v128_const(splat8(SWIZZLE_INDEX))
                .v128_and()
                .i8x16_swizzle();
        }
        Pabsb | Pabsw | Pabsd => {
            push(e, 1, XMM_BYTES);
            let mut s = e.sink();
            match m {
                Pabsb => s.i8x16_abs(),
                Pabsw => s.i16x8_abs(),
                _ => s.i32x4_abs(),
            };
        }
        // dst negated where the source is negative, zero where it is zero.
        Psignb | Psignw | Psignd => {
            let (a, b) = (e.local(ValType::V128), e.local(ValType::V128));
            dest(e);
            e.sink().local_set(a);
            push(e, 1, XMM_BYTES);
            e.sink().local_set(b);
            let mut s = e.sink();
            s.v128_const(0).local_get(a);
            match m {
                Psignb => s.i8x16_sub(),
                Psignw => s.i16x8_sub(),
                _ => s.i32x4_sub(),
            };
            s.local_get(a).local_get(b).v128_const(0);
            match m {
                Psignb => s.i8x16_lt_s(),
                Psignw => s.i16x8_lt_s(),
                _ => s.i32x4_lt_s(),
            };
            s.v128_bitselect().local_get(b).v128_const(0);
            match m {
                Psignb => s.i8x16_eq(),
                Psignw => s.i16x8_eq(),
                _ => s.i32x4_eq(),
            };
            s.v128_andnot();
        }
        // MMX's: the 16 bytes dst:src shifted right by the immediate's bytes, the low 8 kept.
        Palignr if mmx_form => {
            let k = immediate.unwrap();
            let span = XMM_BYTES as u8;
            e.sink().v128_const(0);
            mmx_value(e, 1, QWORD);
            e.sink().i64x2_replace_lane(0);
            mmx_value(e, 0, QWORD);
            e.sink()
                .i64x2_replace_lane(1)
                .v128_const(0)
                .i8x16_shuffle(core::array::from_fn(|b| {
                    (b + usize::from(k)).min(usize::from(span)) as u8
                }));
        }
        // The 32 bytes dst:src shifted right by the immediate's bytes, the low 16 kept.
        Palignr => {
            let k = immediate.unwrap();
            let span = XMM_BYTES as u8;
            if k >= 2 * span {
                e.sink().v128_const(0);
            } else if k >= span {
                dest(e);
                e.sink()
                    .v128_const(0)
                    .i8x16_shuffle(core::array::from_fn(|b| (b as u8 + k - span).min(span)));
            } else {
                push(e, 1, XMM_BYTES);
                dest(e);
                e.sink()
                    .i8x16_shuffle(core::array::from_fn(|b| b as u8 + k));
            }
        }
        // The even and the odd elements of dst then the source, added or subtracted pairwise.
        Phaddw | Phaddd | Phaddsw | Phsubw | Phsubd | Phsubsw | Pmaddubsw => {
            let (a, b) = (e.local(ValType::V128), e.local(ValType::V128));
            dest(e);
            e.sink().local_set(a);
            push(e, 1, XMM_BYTES);
            e.sink().local_set(b);
            if m == Pmaddubsw {
                // Unsigned bytes of dst times signed bytes of the source, as 16-bit products.
                e.sink()
                    .local_get(a)
                    .i16x8_extend_low_i8x16_u()
                    .local_get(b)
                    .i16x8_extend_low_i8x16_s()
                    .i16x8_mul()
                    .local_get(a)
                    .i16x8_extend_high_i8x16_u()
                    .local_get(b)
                    .i16x8_extend_high_i8x16_s()
                    .i16x8_mul()
                    .local_set(b)
                    .local_set(a);
            }
            let unit = if matches!(m, Phaddd | Phsubd) {
                DWORD
            } else {
                WORD
            } as u8;
            let pick = |odd: u8| -> [u8; 16] {
                core::array::from_fn(|b| {
                    let b = b as u8;
                    (b / unit) * 2 * unit + odd * unit + b % unit
                })
            };
            let mut s = e.sink();
            s.local_get(a).local_get(b).i8x16_shuffle(pick(0));
            s.local_get(a).local_get(b).i8x16_shuffle(pick(1));
            match m {
                Phaddw => s.i16x8_add(),
                Phaddd => s.i32x4_add(),
                Phaddsw | Pmaddubsw => s.i16x8_add_sat_s(),
                Phsubw => s.i16x8_sub(),
                Phsubd => s.i32x4_sub(),
                _ => s.i16x8_sub_sat_s(),
            };
        }
        // Rounded high halves of the products; -2^15 times itself gives -2^15, where WebAssembly saturates.
        Pmulhrsw => {
            let (a, b) = (e.local(ValType::V128), e.local(ValType::V128));
            dest(e);
            e.sink().local_set(a);
            push(e, 1, XMM_BYTES);
            let lowest = splat16(i16::MIN);
            e.sink()
                .local_set(b)
                .local_get(a)
                .local_get(b)
                .i16x8_q15mulr_sat_s()
                .local_get(a)
                .v128_const(lowest)
                .i16x8_eq()
                .local_get(b)
                .v128_const(lowest)
                .i16x8_eq()
                .v128_and()
                .v128_xor();
        }
        // The signed products of elements 0 and 2.
        Pmuldq => {
            let evens: [u8; 16] = core::array::from_fn(|b| {
                ((b as u32 / DWORD) * QWORD % XMM_BYTES + b as u32 % DWORD) as u8
            });
            dest(e);
            e.sink()
                .local_tee(value)
                .local_get(value)
                .i8x16_shuffle(evens);
            push(e, 1, XMM_BYTES);
            e.sink()
                .local_tee(value)
                .local_get(value)
                .i8x16_shuffle(evens)
                .i64x2_extmul_low_i32x4_s();
        }
        Pmovsxbw | Pmovsxbd | Pmovsxbq | Pmovsxwd | Pmovsxwq | Pmovsxdq | Pmovzxbw | Pmovzxbd
        | Pmovzxbq | Pmovzxwd | Pmovzxwq | Pmovzxdq => {
            let (from, to, signed) = extension(m);
            push(e, 1, XMM_BYTES / (to / from));
            let mut s = e.sink();
            let mut width = from;
            while width < to {
                match (width, signed) {
                    (BYTE, true) => s.i16x8_extend_low_i8x16_s(),
                    (BYTE, false) => s.i16x8_extend_low_i8x16_u(),
                    (WORD, true) => s.i32x4_extend_low_i16x8_s(),
                    (WORD, false) => s.i32x4_extend_low_i16x8_u(),
                    (_, true) => s.i64x2_extend_low_i32x4_s(),
                    (_, false) => s.i64x2_extend_low_i32x4_u(),
                };
                width *= 2;
            }
        }
        // Each element from the source where the immediate's bit is set.
        Pblendw | Blendps | Blendpd => {
            let k = immediate.unwrap();
            dest(e);
            push(e, 1, XMM_BYTES);
            e.sink().i8x16_shuffle(core::array::from_fn(|b| {
                let b = b as u8;
                if k >> (b / bytes as u8) & 1 == 1 {
                    XMM_BYTES as u8 + b
                } else {
                    b
                }
            }));
        }
        // Each element from the source where xmm0's element has its top bit set.
        Pblendvb | Blendvps | Blendvpd => {
            push(e, 1, XMM_BYTES);
            dest(e);
            e.get_vector(0);
            let mut s = e.sink();
            match m {
                Pblendvb => s.i32_const(u8::BITS as i32 - 1).i8x16_shr_s(),
                Blendvps => s.i32_const(u32::BITS as i32 - 1).i32x4_shr_s(),
                _ => s.i32_const(u64::BITS as i32 - 1).i64x2_shr_s(),
            };
            s.v128_bitselect();
        }
        Pextrb | Pextrd | Pextrq | Extractps => {
            let k = immediate.unwrap() & (XMM_BYTES / bytes - 1) as u8;
            e.sink().v128_const(0);
            e.get_vector(register(1).unwrap());
            let mut s = e.sink();
            match bytes {
                BYTE => s.i8x16_extract_lane_u(k).i64_extend_i32_u(),
                DWORD => s.i32x4_extract_lane(k).i64_extend_i32_u(),
                _ => s.i64x2_extract_lane(k),
            };
            s.i64x2_replace_lane(0);
        }
        Pinsrb | Pinsrd | Pinsrq => {
            let k = immediate.unwrap() & (XMM_BYTES / bytes - 1) as u8;
            dest(e);
            match ins.op_kind(1) {
                OpKind::Register => get_general(e, general(ins.op_register(1)).unwrap()),
                _ => load(e, mem.unwrap(), bytes),
            }
            let mut s = e.sink();
            match bytes {
                BYTE => s.i32_wrap_i64().i8x16_replace_lane(k),
                DWORD => s.i32_wrap_i64().i32x4_replace_lane(k),
                _ => s.i64x2_replace_lane(k),
            };
        }
        // An element of the source (from memory, its only one) into dst's element, then the zero mask.
        Insertps => {
            let k = immediate.unwrap();
            let whole = ins.op_kind(1) == OpKind::Register;
            let lanes = (XMM_BYTES / DWORD) as u8;
            let source = if whole { k >> INSERTPS_SOURCE_SHIFT } else { 0 };
            let target = (k >> INSERTPS_TARGET_SHIFT) & (lanes - 1);
            let unit = DWORD as u8;
            dest(e);
            push(e, 1, if whole { XMM_BYTES } else { DWORD });
            e.sink()
                .i8x16_shuffle(core::array::from_fn(|b| {
                    let b = b as u8;
                    if b / unit == target {
                        XMM_BYTES as u8 + source * unit + b % unit
                    } else {
                        b
                    }
                }))
                .v128_const(i128::from_le_bytes(core::array::from_fn(|b| {
                    if k >> (b as u8 / unit) & 1 == 1 {
                        0
                    } else {
                        u8::MAX
                    }
                })))
                .v128_and();
        }
        // The smallest unsigned word of the source in word 0 and its first index in word 1.
        Phminposuw => {
            push(e, 1, XMM_BYTES);
            e.sink().local_set(value);
            let (least, at) = (e.local(ValType::I32), e.local(ValType::I32));
            let mut s = e.sink();
            s.local_get(value).i16x8_extract_lane_u(0).local_set(least);
            s.i32_const(0).local_set(at);
            for lane in 1..(XMM_BYTES / 2) as u8 {
                // A later word takes the place only when it is smaller.
                s.local_get(value)
                    .i16x8_extract_lane_u(lane)
                    .local_get(least)
                    .i32_lt_u()
                    .if_(BlockType::Empty)
                    .local_get(value)
                    .i16x8_extract_lane_u(lane)
                    .local_set(least)
                    .i32_const(lane.into())
                    .local_set(at)
                    .end();
            }
            s.v128_const(0)
                .local_get(least)
                .i16x8_replace_lane(0)
                .local_get(at)
                .i16x8_replace_lane(1);
        }
        // Eight sums of four absolute byte differences: dst's bytes from the chosen offset on, against a
        // chosen four of the source's.
        Mpsadbw => {
            let k = immediate.unwrap();
            let four = DWORD as u8;
            let (dst_at, src_at) = (
                ((k >> MPSADBW_DESTINATION_SHIFT) & 1) * four,
                (k & ((1 << MPSADBW_DESTINATION_SHIFT) - 1)) * four,
            );
            let (a, b, sum) = (
                e.local(ValType::V128),
                e.local(ValType::V128),
                e.local(ValType::V128),
            );
            dest(e);
            e.sink().local_set(a);
            push(e, 1, XMM_BYTES);
            e.sink().local_set(b).v128_const(0).local_set(sum);
            let zero = XMM_BYTES as u8;
            let mut s = e.sink();
            for j in 0..four {
                s.local_get(sum)
                    .local_get(a)
                    .v128_const(0)
                    .i8x16_shuffle(core::array::from_fn(|n| {
                        let n = n as u8;
                        if n.is_multiple_of(2) {
                            dst_at + n / 2 + j
                        } else {
                            zero
                        }
                    }))
                    .local_get(b)
                    .v128_const(0)
                    .i8x16_shuffle(core::array::from_fn(|n| {
                        if n.is_multiple_of(2) {
                            src_at + j
                        } else {
                            zero
                        }
                    }))
                    .i16x8_sub()
                    .i16x8_abs()
                    .i16x8_add()
                    .local_set(sum);
            }
            s.local_get(sum);
        }
        // ~op0 & op1.
        Pandn | Andnps | Andnpd => {
            push(e, 1, XMM_BYTES);
            dest(e);
            e.sink().v128_andnot();
        }
        // The low halves of each 64-bit lane, multiplied to 64 bits.
        Pmuludq => {
            dest(e);
            e.sink().v128_const(LOW_HALVES).v128_and();
            push(e, 1, XMM_BYTES);
            e.sink().v128_const(LOW_HALVES).v128_and().i64x2_mul();
        }
        _ if binary(m).is_some() => {
            dest(e);
            push(e, 1, XMM_BYTES);
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
            push(e, 1, XMM_BYTES);
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
            dest(e);
            push(e, 1, XMM_BYTES);
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
            dest(e);
            e.sink().v128_const(0).i8x16_shuffle(lanes);
        }
        // By the count in the source's low 64 bits: past the lane's last bit, zero, or the sign for psra.
        _ if shift(m).is_some() && immediate.is_none() => {
            let (lane, kind) = shift(m).unwrap();
            let count = e.local(ValType::I64);
            push(e, 1, HALF.into());
            e.sink().i64x2_extract_lane(0).local_set(count);
            let last = i64::from(lane) - 1;
            dest(e);
            let mut s = e.sink();
            s.local_get(count);
            if kind == 2 {
                s.i64_const(last)
                    .local_get(count)
                    .i64_const(last)
                    .i64_lt_u()
                    .select();
            }
            s.i32_wrap_i64();
            shift_lanes(&mut s, lane, kind);
            if kind != 2 {
                s.v128_const(0)
                    .local_get(count)
                    .i64_const(last)
                    .i64_le_u()
                    .select();
            }
        }
        _ if shift(m).is_some() => {
            let (lane, kind) = shift(m).unwrap();
            let k = u32::from(immediate.unwrap());
            dest(e);
            let mut s = e.sink();
            if k >= u32::from(lane) && kind != 2 {
                // Every bit shifted out.
                s.drop().v128_const(0);
            } else {
                s.i32_const(k.min(u32::from(lane) - 1) as i32);
                shift_lanes(&mut s, lane, kind);
            }
        }
        Pinsrw => {
            dest(e);
            match ins.op_kind(1) {
                OpKind::Register => get_general(e, general(ins.op_register(1)).unwrap()),
                _ => load(e, mem.unwrap(), 2),
            }
            e.sink()
                .i32_wrap_i64()
                .i16x8_replace_lane(immediate.unwrap() & 7);
        }
        // The moves, and the instructions writing a general register, which push their source here.
        _ => push(e, 1, XMM_BYTES),
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
            match ins.op_kind(0) {
                OpKind::Register => set_general(e, general(ins.op_register(0)).unwrap(), result),
                // SSE4.1's `pextrw` to memory.
                _ => store(e, mem.unwrap(), memory_bytes(i), result),
            }
        }
        _ => match ins.op_kind(0) {
            OpKind::Register => match register(0) {
                Some(x) => {
                    e.sink().local_get(value);
                    e.set_vector(x);
                }
                None if mm(0).is_some() => set_mmx(e, mm(0).unwrap(), value),
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
                if bytes == XMM_BYTES {
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
    if mmx_form && writes_memory {
        mmx_transition(e, false);
    }
}

/// `maskmovq`: the bytes of its first MMX register whose byte in the second has its top bit set, to the eight
/// bytes at rdi (checked whole first, as `maskmovdqu`'s halves are).
fn masked_store_mmx<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction) {
    let ins = &i.iced;
    let bytes = QWORD;
    let at = register_address(e, i, RDI, 0);
    if let Some(base) = segment(i) {
        e.sink().local_get(at);
        e.get(base);
        e.sink().i64_add().local_set(at);
    }
    e.check(at, bytes, wcontainers_guest_cpu::Access::Write, 0);
    let mask = e.local(ValType::I64);
    e.get(mmx_register(
        mmx(ins.op_register(2)).expect("an MMX register"),
    ));
    // Each byte's top bit spread over the byte.
    e.sink()
        .local_set(mask)
        .v128_const(0)
        .local_get(mask)
        .i64x2_replace_lane(0)
        .i32_const((u8::BITS - 1) as i32)
        .i8x16_shr_s()
        .i64x2_extract_lane(0)
        .local_set(mask);
    let value = e.local(ValType::I64);
    e.get(mmx_register(
        mmx(ins.op_register(1)).expect("an MMX register"),
    ));
    e.sink().local_get(mask).i64_and();
    load(e, at, bytes);
    e.sink()
        .local_get(mask)
        .i64_const(-1)
        .i64_xor()
        .i64_and()
        .i64_or()
        .local_set(value);
    store(e, at, bytes, value);
}
