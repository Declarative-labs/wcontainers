//! The VEX instructions of AVX and AVX2 written for the whole register: they cross the 128-bit lanes, or no
//! legacy form runs their lanes. Each takes its operands as two 128-bit halves and makes the result's halves
//! with WebAssembly shuffles, swizzles and lane operations; the conversions between widths run their legacy
//! forms on the halves (`crate::vex::run_legacy`).

use crate::decode::Instruction;
use crate::exact::Part;
use crate::flags::{Flag, Flags};
use crate::operand::*;
use crate::simd::{BYTE, DWORD, QWORD, WORD};
use crate::{X86_64, XMM_BYTES};
use iced_x86::{Code, Mnemonic, OpKind};
use wasm_encoder::{InstructionSink, ValType};
use wcontainers_guest_cpu::Emitter;

const HALF_BYTES: u8 = XMM_BYTES as u8;
/// The halves of a ymm register.
pub(crate) const HALVES: usize = 2;
/// The sign bit of each dword and of each qword, which `vtestps` and `vtestpd` test.
const DWORD_SIGNS: i128 = signs(DWORD as usize);
const QWORD_SIGNS: i128 = signs(QWORD as usize);
/// A dword's and a qword's byte numbers in each element, and the step from one element's to the next's, for
/// a swizzle.
const DWORD_BYTES: i128 = byte_numbers(DWORD as usize);
const QWORD_BYTES: i128 = byte_numbers(QWORD as usize);
const DWORD_STEP: i32 = i32::from_le_bytes([DWORD as u8; size_of::<i32>()]);
const QWORD_STEP: i64 = i64::from_le_bytes([QWORD as u8; size_of::<i64>()]);

const fn signs(element: usize) -> i128 {
    let mut bytes = [0; XMM_BYTES as usize];
    let mut b = 0;
    while b < bytes.len() {
        if b % element == element - 1 {
            bytes[b] = 1 << (u8::BITS - 1);
        }
        b += 1;
    }
    i128::from_le_bytes(bytes)
}

const fn byte_numbers(element: usize) -> i128 {
    let mut bytes = [0; XMM_BYTES as usize];
    let mut b = 0;
    while b < bytes.len() {
        bytes[b] = (b % element) as u8;
        b += 1;
    }
    i128::from_le_bytes(bytes)
}
/// The selector fields of `vperm2f128`'s immediate (two bits a half), the bit of each that gives zero, and
/// the bit of `vinsertf128`'s and `vextractf128`'s that names the half.
const PERM2_FIELD_BITS: u32 = u8::BITS / 2;
const PERM2_HALF: u8 = 3;
const PERM2_ZERO: u8 = 1 << 3;
const HALF_SELECT: u8 = 1;
/// `vpermq`'s and `vpermilps`'s selector: two bits an element.
const SELECTOR_BITS: u32 = 2;
const SELECTOR: u8 = (1 << SELECTOR_BITS) - 1;
/// `vpermd`'s index: three bits, the top one naming the half.
const PERMD_HALF: i32 = (XMM_BYTES / DWORD) as i32;
const PERMD_INDEX: i32 = 2 * PERMD_HALF - 1;

pub fn written(i: &Instruction) -> bool {
    use Mnemonic::*;
    // The EVEX forms of these mnemonics are AVX-512's, of features never told.
    crate::decode::claim(i.iced.code()) != crate::claims::CLAIM_NEVER
        && (matches!(
            i.iced.mnemonic(),
            Vmovmskps
                | Vmovmskpd
                | Vpmovmskb
                | Vptest
                | Vtestps
                | Vtestpd
                | Vmovddup
                | Vbroadcastss
                | Vbroadcastsd
                | Vbroadcastf128
                | Vbroadcasti128
                | Vpbroadcastb
                | Vpbroadcastw
                | Vpbroadcastd
                | Vpbroadcastq
                | Vpermq
                | Vpermpd
                | Vperm2f128
                | Vperm2i128
                | Vinsertf128
                | Vinserti128
                | Vextractf128
                | Vextracti128
                | Vpblendd
                | Vpermilps
                | Vpermilpd
                | Vpermd
                | Vpermps
                | Vpsllvd
                | Vpsllvq
                | Vpsrlvd
                | Vpsrlvq
                | Vpsravd
                | Vcvtps2pd
                | Vcvtdq2pd
                | Vcvtpd2ps
                | Vcvtpd2dq
                | Vcvttpd2dq
                | Vmaskmovdqu
        ) || extension(i.iced.mnemonic()).is_some())
}

/// `vpmovsx` and `vpmovzx`: the source's and the result's element bits, and whether the sign is extended.
fn extension(m: Mnemonic) -> Option<(u32, u32, bool)> {
    use Mnemonic::*;
    let (b, w, d, q) = (u8::BITS, u16::BITS, u32::BITS, u64::BITS);
    Some(match m {
        Vpmovsxbw => (b, w, true),
        Vpmovsxbd => (b, d, true),
        Vpmovsxbq => (b, q, true),
        Vpmovsxwd => (w, d, true),
        Vpmovsxwq => (w, q, true),
        Vpmovsxdq => (d, q, true),
        Vpmovzxbw => (b, w, false),
        Vpmovzxbd => (b, d, false),
        Vpmovzxbq => (b, q, false),
        Vpmovzxwd => (w, d, false),
        Vpmovzxwq => (w, q, false),
        Vpmovzxdq => (d, q, false),
        _ => return None,
    })
}

/// Operand `n`'s halves in fresh locals: an xmm register's upper one zero, and the bytes past a memory
/// operand's size zero; `at` is the checked memory operand's data address.
pub(crate) fn halves<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    n: u32,
    at: Option<u32>,
) -> [u32; HALVES] {
    let ins = &i.iced;
    let [low, high] = [e.local(ValType::V128), e.local(ValType::V128)];
    e.sink().v128_const(0).local_set(high);
    match ins.op_kind(n) {
        OpKind::Register => {
            let r = ins.op_register(n);
            match (ymm(r), xmm(r)) {
                (Some(x), _) => {
                    e.get_vector(ymm_high(x));
                    e.sink().local_set(high);
                    e.get_vector(x);
                }
                (_, Some(x)) => e.get_vector(x),
                _ => {
                    e.sink().v128_const(0);
                    get_general(e, general(r).expect("a general register"));
                    e.sink().i64x2_replace_lane(0);
                }
            }
        }
        _ => {
            let at = at.expect("a checked memory operand");
            match memory_bytes(i) {
                bytes if bytes == 2 * XMM_BYTES => {
                    load_vector_at(e, at, XMM_BYTES.into());
                    e.sink().local_set(high);
                    load_vector_at(e, at, 0);
                }
                XMM_BYTES => load_vector_at(e, at, 0),
                bytes => {
                    e.sink().v128_const(0);
                    load(e, at, bytes);
                    e.sink().i64x2_replace_lane(0);
                }
            }
        }
    }
    e.sink().local_set(low);
    [low, high]
}

/// Write the result's halves to operand 0: both to a ymm register, the lower to an xmm register with its
/// upper half cleared, or the operand's bytes to memory.
pub(crate) fn put<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    at: Option<u32>,
    [low, high]: [u32; HALVES],
) {
    let ins = &i.iced;
    if ins.op_kind(0) == OpKind::Register {
        let r = ins.op_register(0);
        let (x, wide) = match ymm(r) {
            Some(x) => (x, true),
            None => (xmm(r).expect("a vector register"), false),
        };
        e.sink().local_get(low);
        e.set_vector(x);
        if wide {
            e.sink().local_get(high);
        } else {
            e.sink().v128_const(0);
        }
        e.set_vector(ymm_high(x));
    } else {
        let at = at.expect("a checked memory operand");
        store_vector_at(e, at, 0, low);
        if memory_bytes(i) == 2 * XMM_BYTES {
            store_vector_at(e, at, XMM_BYTES.into(), high);
        }
    }
}

/// Whether operand 0 is a ymm register.
fn wide_destination(i: &Instruction) -> bool {
    i.iced.op_kind(0) == OpKind::Register && ymm(i.iced.op_register(0)).is_some()
}

/// A vector of bytes of the vectors in `from`, into a fresh local: byte k is byte b of `from[h]` where
/// `pick(k)` is `Some((h, b))`, zero where it is `None`.
fn pick<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    from: &[u32],
    pick: impl Fn(u8) -> Option<(usize, u8)>,
) -> u32 {
    let r = e.local(ValType::V128);
    let mut s = e.sink();
    // The bytes not yet placed come from the zero vector, the second shuffle operand at first.
    s.v128_const(0).local_set(r);
    for (h, &v) in from.iter().enumerate() {
        if !(0..HALF_BYTES).any(|k| pick(k).is_some_and(|(at, _)| at == h)) {
            continue;
        }
        s.local_get(r)
            .local_get(v)
            .i8x16_shuffle(core::array::from_fn(|k| match pick(k as u8) {
                Some((at, b)) if at == h => HALF_BYTES + b,
                _ => k as u8,
            }))
            .local_set(r);
    }
    r
}

/// The result's halves, each by `half`: both for a ymm destination, else the lower and zero.
fn each_half<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    mut half: impl FnMut(&mut E, usize) -> u32,
) -> [u32; HALVES] {
    let low = half(e, 0);
    let high = if wide_destination(i) {
        half(e, 1)
    } else {
        let zero = e.local(ValType::V128);
        e.sink().v128_const(0).local_set(zero);
        zero
    };
    [low, high]
}

/// Of the vectors in `from`, each result half by `pick_byte(half, byte)` as `pick` takes it.
fn pick_halves<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    i: &Instruction,
    from: &[u32],
    pick_byte: impl Fn(usize, u8) -> Option<(usize, u8)>,
) -> [u32; HALVES] {
    each_half(e, i, |e, h| pick(e, from, |k| pick_byte(h, k)))
}

pub fn emit<E: Emitter<X86_64> + ?Sized>(e: &mut E, i: &Instruction, m: Mnemonic) {
    use Mnemonic::*;
    let ins = &i.iced;
    if m == Vmaskmovdqu {
        let mut legacy = *i;
        legacy.iced.set_code(Code::Maskmovdqu_rDI_xmm_xmm);
        assert!(crate::meaning::legacy_meaning(e, &legacy), "maskmovdqu");
        return;
    }
    let at = memory_use(i).map(|kind| {
        let at = address(e, i);
        e.check(at, memory_bytes(i), check_kind(kind), 0);
        at
    });
    let last = ins.op_count() - 1;
    let immediate = (ins.op_kind(last) == OpKind::Immediate8).then(|| ins.immediate8());
    match m {
        Vmovmskps | Vmovmskpd | Vpmovmskb => {
            let [low, high] = halves(e, i, 1, at);
            let (elements, bitmask): (u32, fn(&mut InstructionSink<'_>)) = match m {
                Vmovmskps => (XMM_BYTES / DWORD, |s| {
                    s.i32x4_bitmask();
                }),
                Vmovmskpd => (XMM_BYTES / QWORD, |s| {
                    s.i64x2_bitmask();
                }),
                _ => (XMM_BYTES / BYTE, |s| {
                    s.i8x16_bitmask();
                }),
            };
            let r = e.local(ValType::I64);
            let mut s = e.sink();
            s.local_get(high);
            bitmask(&mut s);
            s.i32_const(elements as i32).i32_shl().local_get(low);
            bitmask(&mut s);
            s.i32_or().i64_extend_i32_u().local_set(r);
            set_general(
                e,
                general(ins.op_register(0)).expect("a general register"),
                r,
            );
        }
        Vptest | Vtestps | Vtestpd => {
            let a = halves(e, i, 0, at);
            let b = halves(e, i, 1, at);
            let signs = match m {
                Vtestps => DWORD_SIGNS,
                Vtestpd => QWORD_SIGNS,
                _ => -1,
            };
            let each = Flag::ALL.map(|_| e.local(ValType::I32));
            let mut s = e.sink();
            for local in each {
                s.i32_const(0).local_set(local);
            }
            for (flag, first, second) in [(Flag::Z, a, b), (Flag::C, b, a)] {
                for h in 0..HALVES {
                    s.local_get(first[h]).local_get(second[h]);
                    if flag == Flag::Z {
                        s.v128_and();
                    } else {
                        s.v128_andnot();
                    }
                }
                s.v128_or()
                    .v128_const(signs)
                    .v128_and()
                    .v128_any_true()
                    .i32_eqz()
                    .local_set(each[flag.index()]);
            }
            e.set_flags(Flags::Each(each));
        }
        Vmovddup => {
            let from = halves(e, i, 1, at);
            let r = pick_halves(e, i, &from, |h, k| Some((h, k % QWORD as u8)));
            put(e, i, at, r);
        }
        Vbroadcastss | Vbroadcastsd | Vbroadcastf128 | Vbroadcasti128 | Vpbroadcastb
        | Vpbroadcastw | Vpbroadcastd | Vpbroadcastq => {
            let element = match m {
                Vpbroadcastb => BYTE,
                Vpbroadcastw => WORD,
                Vbroadcastss | Vpbroadcastd => DWORD,
                Vbroadcastsd | Vpbroadcastq => QWORD,
                _ => XMM_BYTES,
            } as u8;
            let from = halves(e, i, 1, at);
            let r = pick_halves(e, i, &from, |_, k| Some((0, k % element)));
            put(e, i, at, r);
        }
        Vpermq | Vpermpd => {
            let from = halves(e, i, 1, at);
            let imm = immediate.expect("an immediate");
            let r = pick_halves(e, i, &from, |h, k| {
                let q = h as u32 * XMM_BYTES / QWORD + u32::from(k) / QWORD;
                let selected = imm >> (SELECTOR_BITS * q) & SELECTOR;
                let per_half = (XMM_BYTES / QWORD) as u8;
                Some((
                    usize::from(selected / per_half),
                    selected % per_half * QWORD as u8 + k % QWORD as u8,
                ))
            });
            put(e, i, at, r);
        }
        Vperm2f128 | Vperm2i128 => {
            let [a, b] = [halves(e, i, 1, at), halves(e, i, 2, at)];
            let from = [a[0], a[1], b[0], b[1]];
            let imm = immediate.expect("an immediate");
            let r = pick_halves(e, i, &from, |h, k| {
                let field = imm >> (PERM2_FIELD_BITS * h as u32);
                (field & PERM2_ZERO == 0).then(|| (usize::from(field & PERM2_HALF), k))
            });
            put(e, i, at, r);
        }
        Vinsertf128 | Vinserti128 => {
            let [a, b] = [halves(e, i, 1, at), halves(e, i, 2, at)];
            let half = usize::from(immediate.expect("an immediate") & HALF_SELECT);
            let mut r = a;
            r[half] = b[0];
            put(e, i, at, r);
        }
        Vextractf128 | Vextracti128 => {
            let from = halves(e, i, 1, at);
            let half = usize::from(immediate.expect("an immediate") & HALF_SELECT);
            let zero = e.local(ValType::V128);
            e.sink().v128_const(0).local_set(zero);
            put(e, i, at, [from[half], zero]);
        }
        Vpblendd => {
            let [a, b] = [halves(e, i, 1, at), halves(e, i, 2, at)];
            let from = [a[0], a[1], b[0], b[1]];
            let imm = immediate.expect("an immediate");
            let r = pick_halves(e, i, &from, |h, k| {
                let d = h as u32 * XMM_BYTES / DWORD + u32::from(k) / DWORD;
                Some((if imm >> d & 1 != 0 { HALVES + h } else { h }, k))
            });
            put(e, i, at, r);
        }
        Vpermilps | Vpermilpd if immediate.is_some() => {
            let from = halves(e, i, 1, at);
            let imm = immediate.unwrap();
            let r = pick_halves(e, i, &from, |h, k| {
                Some(if m == Vpermilps {
                    let d = u32::from(k) / DWORD;
                    let selected = imm >> (SELECTOR_BITS * d) & SELECTOR;
                    (h, selected * DWORD as u8 + k % DWORD as u8)
                } else {
                    let q = h as u32 * XMM_BYTES / QWORD + u32::from(k) / QWORD;
                    let selected = imm >> q & 1;
                    (h, selected * QWORD as u8 + k % QWORD as u8)
                })
            });
            put(e, i, at, r);
        }
        Vpermilps | Vpermilpd => {
            let [table, control] = [halves(e, i, 1, at), halves(e, i, 2, at)];
            let r = each_half(e, i, |e, h| {
                let r = e.local(ValType::V128);
                let mut s = e.sink();
                s.local_get(table[h]).local_get(control[h]);
                if m == Vpermilps {
                    s.i32_const(SELECTOR.into())
                        .i32x4_splat()
                        .v128_and()
                        .i32_const(DWORD_STEP)
                        .i32x4_splat()
                        .i32x4_mul()
                        .v128_const(DWORD_BYTES)
                        .i32x4_add();
                } else {
                    s.i32_const(1)
                        .i64x2_shr_u()
                        .i64_const(1)
                        .i64x2_splat()
                        .v128_and()
                        .i64_const(QWORD_STEP)
                        .i64x2_splat()
                        .i64x2_mul()
                        .v128_const(QWORD_BYTES)
                        .i64x2_add();
                }
                s.i8x16_swizzle().local_set(r);
                r
            });
            put(e, i, at, r);
        }
        Vpermd | Vpermps => {
            let [index, table] = [halves(e, i, 1, at), halves(e, i, 2, at)];
            let r = each_half(e, i, |e, h| {
                let (bytes, r) = (e.local(ValType::V128), e.local(ValType::V128));
                let mut s = e.sink();
                s.local_get(index[h])
                    .i32_const(PERMD_INDEX & !PERMD_HALF)
                    .i32x4_splat()
                    .v128_and()
                    .i32_const(DWORD_STEP)
                    .i32x4_splat()
                    .i32x4_mul()
                    .v128_const(DWORD_BYTES)
                    .i32x4_add()
                    .local_set(bytes);
                s.local_get(table[1])
                    .local_get(bytes)
                    .i8x16_swizzle()
                    .local_get(table[0])
                    .local_get(bytes)
                    .i8x16_swizzle()
                    .local_get(index[h])
                    .i32_const(PERMD_HALF)
                    .i32x4_splat()
                    .v128_and()
                    .v128_const(0)
                    .i32x4_ne()
                    .v128_bitselect()
                    .local_set(r);
                r
            });
            put(e, i, at, r);
        }
        Vpsllvd | Vpsllvq | Vpsrlvd | Vpsrlvq | Vpsravd => {
            let [value, count] = [halves(e, i, 1, at), halves(e, i, 2, at)];
            let quad = matches!(m, Vpsllvq | Vpsrlvq);
            let r = each_half(e, i, |e, h| shift_each(e, m, quad, value[h], count[h]));
            put(e, i, at, r);
        }
        Vcvtps2pd | Vcvtdq2pd => {
            // The low two elements of the source in the low lane, the next two in the high one.
            let [source, _] = halves(e, i, 1, at);
            let legacy = if m == Vcvtps2pd {
                Code::Cvtps2pd_xmm_xmmm64
            } else {
                Code::Cvtdq2pd_xmm_xmmm64
            };
            let upper = pick(e, &[source], |k| {
                (k < HALF_BYTES / 2).then_some((0, k + HALF_BYTES / 2))
            });
            let r = [(source, Part::LowLane), (upper, Part::HighLane)].map(|(b, part)| {
                let a = e.local(ValType::V128);
                e.sink().v128_const(0).local_set(a);
                crate::vex::run_legacy(e, i, legacy, part, a, b);
                a
            });
            put(e, i, at, r);
        }
        Vcvtpd2ps | Vcvtpd2dq | Vcvttpd2dq => {
            // Each lane's two results in the low half of the result.
            let source = halves(e, i, 1, at);
            let legacy = match m {
                Vcvtpd2ps => Code::Cvtpd2ps_xmm_xmmm128,
                Vcvtpd2dq => Code::Cvtpd2dq_xmm_xmmm128,
                _ => Code::Cvttpd2dq_xmm_xmmm128,
            };
            let parts = [Part::LowLane, Part::HighLane];
            let lanes: [u32; HALVES] = core::array::from_fn(|h| {
                let a = e.local(ValType::V128);
                e.sink().v128_const(0).local_set(a);
                crate::vex::run_legacy(e, i, legacy, parts[h], a, source[h]);
                a
            });
            let low = pick(e, &lanes, |k| {
                let h = usize::from(k / (HALF_BYTES / 2));
                Some((h, k % (HALF_BYTES / 2)))
            });
            let zero = e.local(ValType::V128);
            e.sink().v128_const(0).local_set(zero);
            put(e, i, at, [low, zero]);
        }
        _ => {
            let (from, to, signed) = extension(m).expect("a vpmovsx or vpmovzx");
            let [source, _] = halves(e, i, 1, at);
            let steps = (to / from).trailing_zeros();
            let r = each_half(e, i, |e, h| {
                let r = e.local(ValType::V128);
                let mut s = e.sink();
                s.local_get(source);
                for step in 0..steps {
                    let high = h > 0 && step == steps - 1;
                    extend(&mut s, from << step, high, signed);
                }
                s.local_set(r);
                r
            });
            put(e, i, at, r);
        }
    }
}

/// Widen the low or high half of the vector on the stack's elements of `bits` bits to twice their width.
fn extend(s: &mut InstructionSink<'_>, bits: u32, high: bool, signed: bool) {
    const B: u32 = u8::BITS;
    const W: u32 = u16::BITS;
    match (bits, high, signed) {
        (B, false, true) => s.i16x8_extend_low_i8x16_s(),
        (B, true, true) => s.i16x8_extend_high_i8x16_s(),
        (B, false, false) => s.i16x8_extend_low_i8x16_u(),
        (B, true, false) => s.i16x8_extend_high_i8x16_u(),
        (W, false, true) => s.i32x4_extend_low_i16x8_s(),
        (W, true, true) => s.i32x4_extend_high_i16x8_s(),
        (W, false, false) => s.i32x4_extend_low_i16x8_u(),
        (W, true, false) => s.i32x4_extend_high_i16x8_u(),
        (_, false, true) => s.i64x2_extend_low_i32x4_s(),
        (_, true, true) => s.i64x2_extend_high_i32x4_s(),
        (_, false, false) => s.i64x2_extend_low_i32x4_u(),
        (_, true, false) => s.i64x2_extend_high_i32x4_u(),
    };
}

/// `vpsllvd` and its kin on one half: each element shifted by the count in the same element of `count`, a
/// count of the element's width or more giving zero (or the sign in every bit, for `vpsravd`).
fn shift_each<E: Emitter<X86_64> + ?Sized>(
    e: &mut E,
    m: Mnemonic,
    quad: bool,
    value: u32,
    count: u32,
) -> u32 {
    use Mnemonic::*;
    let r = e.local(ValType::V128);
    let c = e.local(if quad { ValType::I64 } else { ValType::I32 });
    let mut s = e.sink();
    s.local_get(value).local_set(r);
    let elements = XMM_BYTES / if quad { QWORD } else { DWORD };
    for n in 0..elements as u8 {
        s.local_get(r);
        if quad {
            s.local_get(count).i64x2_extract_lane(n).local_set(c);
            s.local_get(value).i64x2_extract_lane(n).local_get(c);
            if m == Vpsllvq {
                s.i64_shl();
            } else {
                s.i64_shr_u();
            }
            s.i64_const(0)
                .local_get(c)
                .i64_const(u64::BITS.into())
                .i64_lt_u()
                .select()
                .i64x2_replace_lane(n);
        } else {
            s.local_get(count).i32x4_extract_lane(n).local_set(c);
            s.local_get(value).i32x4_extract_lane(n);
            if m == Vpsravd {
                s.local_get(c)
                    .i32_const(u32::BITS as i32 - 1)
                    .local_get(c)
                    .i32_const(u32::BITS as i32)
                    .i32_lt_u()
                    .select()
                    .i32_shr_s();
            } else {
                s.local_get(c);
                if m == Vpsllvd {
                    s.i32_shl();
                } else {
                    s.i32_shr_u();
                }
                s.i32_const(0)
                    .local_get(c)
                    .i32_const(u32::BITS as i32)
                    .i32_lt_u()
                    .select();
            }
            s.i32x4_replace_lane(n);
        }
        s.local_set(r);
    }
    r
}
