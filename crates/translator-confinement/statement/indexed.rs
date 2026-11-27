//! Form I's piece of a pre- or post-indexed load or store of a doubleword through a guest register, `ldr/str xT, [xN,
//! #imm]!` and `ldr/str xT, [xN], #imm` (look-ahead none), in the shape the translator gives a post-index and a
//! pre-index whose immediate keeps the base's alignment (carry.rs `access_in_window`): the window check, the cut `add
//! x23, x24, wN, uxtw`, the alignment test of the base, the new base `add x15, xN, #imm`, the copied access through
//! x23 (written back, x23 being scratch), `mov xN, x15`, the exit, the mark's tail, and the slow tail with its page
//! probes at x23 plus the access's offset.
//!
//! The access reaches up to 256 bytes below the window: this proof needs that much reserved below it (`a.below`).
//! What each word does is a hypothesis on the free `step` (see the crate's documentation). `tests/pieces.rs` checks the
//! pieces are the translator's, by the pins at the end of this file.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;

verus! {

/// The 9-bit field of an immediate from -256 to 255.
pub open spec fn imm9(imm: int) -> u32 {
    (if imm < 0 { imm + 512 } else { imm }) as u32
}

/// `add x15, xN, #imm` (`sub` for a negative one).
pub open spec fn new_base(n: u32, imm: int) -> u32 {
    if imm >= 0 {
        (0x9100_000fu32 | ((imm as u32) << 10u32) | (n << 5u32)) as u32
    } else {
        (0xd100_000fu32 | (((-imm) as int) as u32) << 10u32 | (n << 5u32)) as u32
    }
}

/// The copied access through x23: `ldr/str xT, [x23, #imm]!` or `ldr/str xT, [x23], #imm`.
pub open spec fn indexed_word(t: u32, store: bool, pre: bool, imm: int) -> u32 {
    ((if store { 0xf800_0000u32 } else { 0xf840_0000u32 }) | (imm9(imm) << 12u32) | (if pre { 0xc00u32 } else { 0x400u32 })
        | 0x2e0u32 | t) as u32
}

/// `mov xN, x15`.
pub open spec fn move_base(n: u32) -> u32 {
    (0xaa0f_03e0u32 | n) as u32
}

/// `cbnz x15, .+44`, `b .-52`, and the probes' way back: a load's `b .-72`, a store's `b .-76`.
pub const CBNZ_X15_44: u32 = 0xb500_016f;
pub const B_BACK_52: u32 = 0x17ff_fff3;
pub const B_BACK_72: u32 = 0x17ff_ffee;
pub const B_BACK_76: u32 = 0x17ff_ffed;

/// The slow tail's first word, `add x15, x23, #((off + 7) mod 4096)`: the page offset of the access's last byte.
pub open spec fn slow_start(off: int) -> u32 {
    (0x9100_02efu32 | ((((off + 7) % 4096) as u32) << 10u32)) as u32
}

/// `movz x15, #off`, or `movn x15, #(-off - 1)` for a negative one.
pub open spec fn move_offset(off: int) -> u32 {
    if off >= 0 {
        (0xd280_000fu32 | ((off as u32) << 5u32)) as u32
    } else {
        (0x9280_000fu32 | ((((-off - 1) as int) as u32) << 5u32)) as u32
    }
}

/// The access's offset from x23: the immediate before (pre-index), none after (post-index).
pub open spec fn offset_of(pre: bool, imm: int) -> int {
    if pre { imm } else { 0 }
}

/// The piece: 24 words for a load, 25 for a store.
pub open spec fn indexed_access(n: u32, t: u32, store: bool, pre: bool, imm: int, slot: u64) -> Seq<u32> {
    let off = offset_of(pre, imm);
    seq![check_word(n), CBNZ_X15_44, add_cut(n), align_word(n), CBNZ_X15_44, new_base(n, imm), indexed_word(t, store, pre, imm), move_base(n)]
        + exit_words(slot)
        + seq![EOR_X15_TOP, LDRB_MARK, UDF, slow_start(off), AND_X15_PAGE, CBZ_X15_8, B_BACK_52, move_offset(off), ADD_X15_X23_X15]
        + if store { seq![PROBE_STORE, ADD_X15_X15_7, PROBE_STORE, B_BACK_76] } else { seq![PROBE_LOAD_FIRST, PROBE_LOAD_LAST, B_BACK_72] }
}

/// The piece's words' meaning (Arm ARM).
pub open spec fn indexed_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, t: u32, store: bool, pre: bool, imm: int) -> bool {
    let off = offset_of(pre, imm);
    &&& writes(step, check_word(n), 15, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, CBNZ_X15_44, 15, 44, true)
    &&& writes(step, add_cut(n), 23, |m: Machine| ((m.x[24] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& writes(step, align_word(n), 15, |m: Machine| m.x[n as int] & 7u64)
    &&& writes(step, new_base(n, imm), 15, |m: Machine| ((m.x[n as int] + imm) % two_to_64()) as u64)
    &&& accesses_writing_back(step, indexed_word(t, store, pre, imm), loaded(t, store), 23, |m: Machine| (m.x[23] + off) % two_to_64(), 8)
    &&& writes(step, move_base(n), n as int, |m: Machine| m.x[15])
    &&& mark_means(step)
    &&& exit_means(step)
    &&& writes(step, slow_start(off), 15, |m: Machine| ((m.x[23] + (off + 7) % 4096) % two_to_64()) as u64)
    &&& writes(step, AND_X15_PAGE, 15, |m: Machine| m.x[15] & 0xff8u64)
    &&& branches_if(step, CBZ_X15_8, 15, 8, false)
    &&& branches(step, B_BACK_52, -52)
    &&& writes(step, move_offset(off), 15, |m: Machine| (off % two_to_64()) as u64)
    &&& writes(step, ADD_X15_X23_X15, 15, |m: Machine| ((m.x[23] + m.x[15]) % two_to_64()) as u64)
    &&& store ==> accesses(step, PROBE_STORE, 31, |m: Machine| m.x[15] as int, 1)
    &&& store ==> writes(step, ADD_X15_X15_7, 15, |m: Machine| ((m.x[15] + 7) % two_to_64()) as u64)
    &&& store ==> branches(step, B_BACK_76, -76)
    &&& !store ==> accesses(step, PROBE_LOAD_FIRST, 31, |m: Machine| m.x[15] as int, 1)
    &&& !store ==> accesses(step, PROBE_LOAD_LAST, 31, |m: Machine| (m.x[15] + 7) % two_to_64(), 1)
    &&& !store ==> branches(step, B_BACK_72, -72)
}

/// x plus a small offset, taken modulo 2^64, is x plus the offset where that stays an address.
proof fn lemma_offset(x: int, off: int)
    requires
        0 <= x < two_to_64(),
        -256 <= off < 256,
        0 <= x + off < two_to_64(),
    ensures
        (x + off % two_to_64()) % two_to_64() == x + off,
        (x + off) % two_to_64() == x + off,
{
    if off < 0 {
        assert(off % two_to_64() == off + two_to_64());
        assert((x + off + two_to_64()) % two_to_64() == x + off);
    }
}

/// Where the run stands: the frame and base kept; x23 the cut address from the cut to the access and through the
/// slow tail; x15 the offset, then the address probed, at the probes; the exit's moves; the mark.
pub open spec fn indexed_inv(a: Allowed, slot: u64, store: bool, off: int, m: Machine) -> bool {
    let o = m.pc - a.code;
    let back = if store { 96int } else { 92int };
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& (12 <= o <= 24 || (60 <= o && o <= back)) ==> in_window(a, m.x[23])
    &&& o == 80 ==> m.x[15] as int == off % two_to_64()
    &&& (o == 84 || o == 88) ==> m.x[15] as int == m.x[23] + off
    &&& (store && o == 92) ==> m.x[15] as int == m.x[23] + off + 7
    &&& 32 <= o < 48 ==> at_exit(m, slot, (o - 32) / 4)
    &&& o == 52 ==> m.x[15] as int == a.base + 0x1_0000_0000
}

/// THE PRE- AND POST-INDEXED PIECE IS CONFINED, with 256 bytes reserved below the window.
pub proof fn indexed_access_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    n: u32,
    t: u32,
    store: bool,
    pre: bool,
    imm: int,
    a: Allowed,
    guest: int,
)
    requires
        registers(n, t, store) && n != 24 && n != 28,
        -256 <= imm < 256,
        indexed_means(step, n, t, store, pre, imm),
        in_reach(a, guest),
        a.below >= 256,
        a.length == if store { 100int } else { 96int },
    ensures
        is_confined(step, indexed_access(n, t, store, pre, imm, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let off = offset_of(pre, imm);
    let instructions = indexed_access(n, t, store, pre, imm, slot);
    let inv = |m: Machine| indexed_inv(a, slot, store, off, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == if store { 25int } else { 24int });
    assert(instructions.subrange(8, 12) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == check_word(n));
        } else if o == 4 {
            assert(w == CBNZ_X15_44);
        } else if o == 8 {
            assert(w == add_cut(n));
            lemma_low_word(m.x[n as int]);
        } else if o == 12 {
            assert(w == align_word(n));
        } else if o == 16 {
            assert(w == CBNZ_X15_44);
        } else if o == 20 {
            assert(w == new_base(n, imm));
        } else if o == 24 {
            assert(w == indexed_word(t, store, pre, imm));
            lemma_offset(m.x[23] as int, off);
        } else if o == 28 {
            assert(w == move_base(n));
        } else if 32 <= o < 48 {
            lemma_exit_step(step, instructions, a, guest, 32, m);
        } else if o == 48 {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if o == 52 {
            assert(w == LDRB_MARK);
        } else if o == 56 {
            assert(w == UDF);
        } else if o == 60 {
            assert(w == slow_start(off));
        } else if o == 64 {
            assert(w == AND_X15_PAGE);
        } else if o == 68 {
            assert(w == CBZ_X15_8);
        } else if o == 72 {
            assert(w == B_BACK_52);
        } else if o == 76 {
            assert(w == move_offset(off));
        } else if o == 80 {
            assert(w == ADD_X15_X23_X15);
            lemma_offset(m.x[23] as int, off);
        } else if store {
            if o == 84 {
                assert(w == PROBE_STORE);
            } else if o == 88 {
                assert(w == ADD_X15_X15_7);
            } else if o == 92 {
                assert(w == PROBE_STORE);
            } else {
                assert(o == 96);
                assert(w == B_BACK_76);
            }
        } else {
            if o == 84 {
                assert(w == PROBE_LOAD_FIRST);
            } else if o == 88 {
                assert(w == PROBE_LOAD_LAST);
            } else {
                assert(o == 92);
                assert(w == B_BACK_72);
            }
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `cbnz x15, .+48`, `cbz x15, .+44`, and the probes' way back to the new base: a load's `b .-56`, a store's `b .-60`.
pub const CBNZ_X15_48: u32 = 0xb500_018f;
pub const CBZ_X15_44: u32 = 0xb400_016f;
pub const B_BACK_56: u32 = 0x17ff_fff2;
pub const B_BACK_60: u32 = 0x17ff_fff1;

/// The other pre-indexed piece, for an immediate that does not keep the base's alignment (carry.rs `page_crossing`,
/// its second shape): the window check, the cut, the page test of the access's own last byte inline (`add x15, x23,
/// #((imm + 7) mod 4096)`, `and x15, x15, #0xff8`, `cbz x15` to the probes), the new base, the access, `mov xN, x15`,
/// the exit, the mark's tail, and the probes at x23 plus the immediate, back to the new base. 21 words for a load, 22
/// for a store.
pub open spec fn unaligned_access(n: u32, t: u32, store: bool, imm: int, slot: u64) -> Seq<u32> {
    seq![check_word(n), CBNZ_X15_48, add_cut(n), slow_start(imm), AND_X15_PAGE, CBZ_X15_44, new_base(n, imm), indexed_word(t, store, true, imm), move_base(n)]
        + exit_words(slot)
        + seq![EOR_X15_TOP, LDRB_MARK, UDF, move_offset(imm), ADD_X15_X23_X15]
        + if store { seq![PROBE_STORE, ADD_X15_X15_7, PROBE_STORE, B_BACK_60] } else { seq![PROBE_LOAD_FIRST, PROBE_LOAD_LAST, B_BACK_56] }
}

/// That piece's words' meaning (Arm ARM).
pub open spec fn unaligned_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, t: u32, store: bool, imm: int) -> bool {
    &&& writes(step, check_word(n), 15, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, CBNZ_X15_48, 15, 48, true)
    &&& writes(step, add_cut(n), 23, |m: Machine| ((m.x[24] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& writes(step, slow_start(imm), 15, |m: Machine| ((m.x[23] + (imm + 7) % 4096) % two_to_64()) as u64)
    &&& writes(step, AND_X15_PAGE, 15, |m: Machine| m.x[15] & 0xff8u64)
    &&& branches_if(step, CBZ_X15_44, 15, 44, false)
    &&& writes(step, new_base(n, imm), 15, |m: Machine| ((m.x[n as int] + imm) % two_to_64()) as u64)
    &&& accesses_writing_back(step, indexed_word(t, store, true, imm), loaded(t, store), 23, |m: Machine| (m.x[23] + imm) % two_to_64(), 8)
    &&& writes(step, move_base(n), n as int, |m: Machine| m.x[15])
    &&& mark_means(step)
    &&& exit_means(step)
    &&& writes(step, move_offset(imm), 15, |m: Machine| (imm % two_to_64()) as u64)
    &&& writes(step, ADD_X15_X23_X15, 15, |m: Machine| ((m.x[23] + m.x[15]) % two_to_64()) as u64)
    &&& store ==> accesses(step, PROBE_STORE, 31, |m: Machine| m.x[15] as int, 1)
    &&& store ==> writes(step, ADD_X15_X15_7, 15, |m: Machine| ((m.x[15] + 7) % two_to_64()) as u64)
    &&& store ==> branches(step, B_BACK_60, -60)
    &&& !store ==> accesses(step, PROBE_LOAD_FIRST, 31, |m: Machine| m.x[15] as int, 1)
    &&& !store ==> accesses(step, PROBE_LOAD_LAST, 31, |m: Machine| (m.x[15] + 7) % two_to_64(), 1)
    &&& !store ==> branches(step, B_BACK_56, -56)
}

/// Where that piece's run stands: x23 the cut address from the cut to the access and through the probes; x15 the
/// immediate, then the address probed; the exit's moves; the mark.
pub open spec fn unaligned_inv(a: Allowed, slot: u64, store: bool, imm: int, m: Machine) -> bool {
    let o = m.pc - a.code;
    let back = if store { 84int } else { 80int };
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& (12 <= o <= 28 || (64 <= o && o <= back)) ==> in_window(a, m.x[23])
    &&& o == 68 ==> m.x[15] as int == imm % two_to_64()
    &&& (o == 72 || o == 76) ==> m.x[15] as int == m.x[23] + imm
    &&& (store && o == 80) ==> m.x[15] as int == m.x[23] + imm + 7
    &&& 36 <= o < 52 ==> at_exit(m, slot, (o - 36) / 4)
    &&& o == 56 ==> m.x[15] as int == a.base + 0x1_0000_0000
}

/// THE OTHER PRE-INDEXED PIECE IS CONFINED, with 256 bytes reserved below the window.
pub proof fn unaligned_access_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    n: u32,
    t: u32,
    store: bool,
    imm: int,
    a: Allowed,
    guest: int,
)
    requires
        registers(n, t, store) && n != 24 && n != 28,
        -256 <= imm < 256,
        unaligned_means(step, n, t, store, imm),
        in_reach(a, guest),
        a.below >= 256,
        a.length == if store { 88int } else { 84int },
    ensures
        is_confined(step, unaligned_access(n, t, store, imm, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = unaligned_access(n, t, store, imm, slot);
    let inv = |m: Machine| unaligned_inv(a, slot, store, imm, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == if store { 22int } else { 21int });
    assert(instructions.subrange(9, 13) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == check_word(n));
        } else if o == 4 {
            assert(w == CBNZ_X15_48);
        } else if o == 8 {
            assert(w == add_cut(n));
            lemma_low_word(m.x[n as int]);
        } else if o == 12 {
            assert(w == slow_start(imm));
        } else if o == 16 {
            assert(w == AND_X15_PAGE);
        } else if o == 20 {
            assert(w == CBZ_X15_44);
        } else if o == 24 {
            assert(w == new_base(n, imm));
        } else if o == 28 {
            assert(w == indexed_word(t, store, true, imm));
            lemma_offset(m.x[23] as int, imm);
        } else if o == 32 {
            assert(w == move_base(n));
        } else if 36 <= o < 52 {
            lemma_exit_step(step, instructions, a, guest, 36, m);
        } else if o == 52 {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if o == 56 {
            assert(w == LDRB_MARK);
        } else if o == 60 {
            assert(w == UDF);
        } else if o == 64 {
            assert(w == move_offset(imm));
        } else if o == 68 {
            assert(w == ADD_X15_X23_X15);
            lemma_offset(m.x[23] as int, imm);
        } else if store {
            if o == 72 {
                assert(w == PROBE_STORE);
            } else if o == 76 {
                assert(w == ADD_X15_X15_7);
            } else if o == 80 {
                assert(w == PROBE_STORE);
            } else {
                assert(o == 84);
                assert(w == B_BACK_60);
            }
        } else {
            if o == 72 {
                assert(w == PROBE_LOAD_FIRST);
            } else if o == 76 {
                assert(w == PROBE_LOAD_LAST);
            } else {
                assert(o == 80);
                assert(w == B_BACK_56);
            }
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The pins (see copy.rs): one line each, the piece's function and its arguments, the words.
pub open spec fn unaligned_access_at(n: u32, t: u32, store: bool, imm: int, base: int, guest: int, displaced: bool) -> Seq<u32> {
    unaligned_access(n, t, store, imm, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

pub open spec fn indexed_access_at(n: u32, t: u32, store: bool, pre: bool, imm: int, base: int, guest: int, displaced: bool) -> Seq<u32> {
    indexed_access(n, t, store, pre, imm, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

proof fn pin_indexed_load_pre_minus_16() { assert(indexed_access_at(1u32, 0u32, false, true, -16int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_016fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_016fu32, 0xd100_402fu32, 0xf85f_0ee0u32, 0xaa0f_03e1u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x913f_deefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff3u32, 0x9280_01efu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_ffeeu32]) by (compute); }
proof fn pin_indexed_load_post_minus_16() { assert(indexed_access_at(1u32, 0u32, false, false, -16int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_016fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_016fu32, 0xd100_402fu32, 0xf85f_06e0u32, 0xaa0f_03e1u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff3u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_ffeeu32]) by (compute); }
proof fn pin_indexed_store_pre_minus_256() { assert(indexed_access_at(1u32, 0u32, true, true, -256int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_016fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_016fu32, 0xd104_002fu32, 0xf810_0ee0u32, 0xaa0f_03e1u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x913c_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff3u32, 0x9280_1fefu32, 0x8b2f_62efu32, 0x383f_01ffu32, 0x9100_1defu32, 0x383f_01ffu32, 0x17ff_ffedu32]) by (compute); }
proof fn pin_indexed_store_post_minus_3() { assert(indexed_access_at(1u32, 0u32, true, false, -3int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_016fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_016fu32, 0xd100_0c2fu32, 0xf81f_d6e0u32, 0xaa0f_03e1u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff3u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x383f_01ffu32, 0x9100_1defu32, 0x383f_01ffu32, 0x17ff_ffedu32]) by (compute); }
proof fn pin_indexed_load_pre_8_displaced() { assert(indexed_access_at(1u32, 0u32, false, true, 8int, 0x6_0000_0000int, 0x40_1000int, true) =~= seq![0xd360_dc2fu32, 0xb500_016fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_016fu32, 0x9100_202fu32, 0xf840_8ee0u32, 0xaa0f_03e1u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_3eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff3u32, 0xd280_010fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_ffeeu32]) by (compute); }
proof fn pin_indexed_load_post_255() { assert(indexed_access_at(7u32, 30u32, false, false, 255int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dcefu32, 0xb500_016fu32, 0x8b27_4317u32, 0x9240_08efu32, 0xb500_016fu32, 0x9103_fcefu32, 0xf84f_f6feu32, 0xaa0f_03e7u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff3u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_ffeeu32]) by (compute); }
proof fn pin_unaligned_load_minus_3() { assert(unaligned_access_at(1u32, 0u32, false, -3int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_018fu32, 0x8b21_4317u32, 0x9100_12efu32, 0x927d_21efu32, 0xb400_016fu32, 0xd100_0c2fu32, 0xf85f_dee0u32, 0xaa0f_03e1u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9280_004fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_fff2u32]) by (compute); }
proof fn pin_unaligned_store_1() { assert(unaligned_access_at(1u32, 0u32, true, 1int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_018fu32, 0x8b21_4317u32, 0x9100_22efu32, 0x927d_21efu32, 0xb400_016fu32, 0x9100_042fu32, 0xf800_1ee0u32, 0xaa0f_03e1u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0xd280_002fu32, 0x8b2f_62efu32, 0x383f_01ffu32, 0x9100_1defu32, 0x383f_01ffu32, 0x17ff_fff1u32]) by (compute); }
proof fn pin_unaligned_load_255_displaced() { assert(unaligned_access_at(1u32, 0u32, false, 255int, 0x6_0000_0000int, 0x40_1000int, true) =~= seq![0xd360_dc2fu32, 0xb500_018fu32, 0x8b21_4317u32, 0x9104_1aefu32, 0x927d_21efu32, 0xb400_016fu32, 0x9103_fc2fu32, 0xf84f_fee0u32, 0xaa0f_03e1u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0xd280_1fefu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_fff2u32]) by (compute); }

} // verus!
