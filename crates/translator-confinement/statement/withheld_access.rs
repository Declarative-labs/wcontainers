//! Form I's pieces of a load or store that names a withheld register (x15, x18, x23, x24, x28; look-ahead none;
//! carry.rs `carry` with its stand-ins around `access_in_window`): withheld.rs's frame accesses and moves before and
//! after, and between them access.rs's byte or doubleword piece with two of its registers free:
//!
//! - the check register `c` (x15, or another free one when x15 is a stand-in), which the window check, the alignment
//!   test, the mark and the probes use;
//! - the base register `bb` the access is cut by: x24, or the register x24 was moved to (`mov xbb, x24`) when the
//!   instruction names x24, whose guest value is then loaded into x24.
//!
//! For example `str x24, [x1, #8]`: `str x29, [x28, #152]`, `mov x29, x24`, `ldr x24, [x28, #136]`, the check of x1,
//! `add x23, x29, w1, uxtw`, the alignment test, `str x24, [x23, #8]`, `str x24, [x28, #136]`, `mov x24, x29`, `ldr x29,
//! [x28, #152]`, the exit, the mark (`eor x15, x29, #2^32`), the slow tail.
//!
//! The proofs hold for any frame accesses and moves on either side meeting withheld.rs's hypotheses, and for the base
//! moved by any one of the words before. They do not state that x24 holds the base again at the exit: a piece is
//! entered by the entry rule, under which the run itself sets x24 (statement.rs `entered`). What each word does is a
//! hypothesis on the free `step` (see the crate's documentation). `tests/rest.rs` checks the pieces are the
//! translator's, by the pins at the end of this file, and that every such load and store is one of these pieces.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;
use super::withheld::*;
use super::branch::*;

verus! {

/// `ubfx xc, xN, #32, #24`, `cbnz xc, .+by`, `cbz xc, .+by`, `b .-by`.
pub open spec fn check_c(n: u32, c: int) -> u32 {
    (0xd360_dc00u32 | (n << 5u32) | (c as u32)) as u32
}

pub open spec fn cbnz_c(c: int, by: int) -> u32 {
    (0xb500_0000u32 | (((by / 4) as u32) << 5u32) | (c as u32)) as u32
}

pub open spec fn cbz_c(c: int, by: int) -> u32 {
    (0xb400_0000u32 | (((by / 4) as u32) << 5u32) | (c as u32)) as u32
}

pub open spec fn b_back(by: int) -> u32 {
    (0x1800_0000int - by / 4) as u32
}

/// `ldrb/strb wT, [xbb, wN, uxtw]`, `add x23, xbb, wN, uxtw`, `and xc, xN, #7`, `ldr/str xT, [x23, #off]`.
pub open spec fn cut_byte_by(n: u32, t: u32, store: bool, bb: int) -> u32 {
    ((if store { 0x3820_4800u32 } else { 0x3860_4800u32 }) | (n << 16u32) | ((bb as u32) << 5u32) | t) as u32
}

pub open spec fn add_cut_by(n: u32, bb: int) -> u32 {
    (0x8b20_4017u32 | (n << 16u32) | ((bb as u32) << 5u32)) as u32
}

pub open spec fn align_c(n: u32, c: int) -> u32 {
    (0x9240_0800u32 | (n << 5u32) | (c as u32)) as u32
}

pub open spec fn wide_at(t: u32, store: bool, off: int) -> u32 {
    ((if store { 0xf900_02e0u32 } else { 0xf940_02e0u32 }) | (((off / 8) as u32) << 10u32) | t) as u32
}

/// The mark through `c`: `eor xc, xbb, #2^32`, `ldrb wzr, [xc, #4095]`.
pub open spec fn eor_c(c: int, bb: int) -> u32 {
    (0xd260_0000u32 | ((bb as u32) << 5u32) | (c as u32)) as u32
}

pub open spec fn mark_load_c(c: int) -> u32 {
    (0x397f_fc1fu32 | ((c as u32) << 5u32)) as u32
}

/// The slow tail through `c`: `add xc, x23, #k`, `and xc, xc, #0xff8`, `movz xc, #k`, `add xc, x23, xc`, `ldrb wzr,
/// [xc, #k]`, `staddb wzr, [xc]`, `add xc, xc, #k`.
pub open spec fn add_c_x23(c: int, k: int) -> u32 {
    (0x9100_02e0u32 | ((k as u32) << 10u32) | (c as u32)) as u32
}

pub open spec fn and_c_page(c: int) -> u32 {
    (0x927d_2000u32 | ((c as u32) << 5u32) | (c as u32)) as u32
}

pub open spec fn movz_c(c: int, k: int) -> u32 {
    (0xd280_0000u32 | ((k as u32) << 5u32) | (c as u32)) as u32
}

pub open spec fn add_x23_c(c: int) -> u32 {
    (0x8b20_62e0u32 | ((c as u32) << 16u32) | (c as u32)) as u32
}

pub open spec fn probe_load_c(c: int, k: int) -> u32 {
    (0x3940_001fu32 | ((k as u32) << 10u32) | ((c as u32) << 5u32)) as u32
}

pub open spec fn probe_store_c(c: int) -> u32 {
    (0x383f_001fu32 | ((c as u32) << 5u32)) as u32
}

pub open spec fn add_c_c(c: int, k: int) -> u32 {
    (0x9100_0000u32 | ((k as u32) << 10u32) | ((c as u32) << 5u32) | (c as u32)) as u32
}

/// The base reaches `bb` before the access: x24 kept by every word before, or moved to `bb` by word `j` (`mov xbb,
/// x24`) with x24 and `bb` kept before it and `bb` kept after it.
pub open spec fn base_moved(before: Seq<FrameAccess>, bb: int, j: int) -> bool {
    if bb == 24 {
        forall|k: int| 0 <= k < before.len() ==> (#[trigger] before[k]).rt != 24
    } else {
        &&& 0 <= j < before.len() && before[j].moves && before[j].rt == bb && before[j].off == 24
        &&& forall|k: int| 0 <= k < j ==> (#[trigger] before[k]).rt != 24 && before[k].rt != bb
        &&& forall|k: int| j < k < before.len() ==> (#[trigger] before[k]).rt != bb
    }
}

/// The register holding the base before word `i` of the words before.
pub open spec fn holder(bb: int, j: int, i: int) -> int {
    if bb == 24 || i <= j { 24 } else { bb }
}

/// The registers: `c` and `bb` apart and neither x23 nor x28 (`c` not x24 either), the address register neither x23
/// nor `c`, a load into any register but x28.
pub open spec fn stand_in_registers(n: u32, t: u32, store: bool, bb: int, c: int) -> bool {
    &&& n < 31 && n != 23 && n as int != c
    &&& 0 <= bb < 31 && bb != 23 && bb != 28 && bb != c
    &&& 0 <= c < 31 && c != 23 && c != 24 && c != 28
    &&& t < 32 && (store || t != 28)
}

/// Each frame access and move before meets withheld.rs's hypotheses; each word after does, or is a register-only
/// instruction writing `rt` (copy.rs `copied_means`): with look-ahead, the instructions after the access in its part.
pub open spec fn around_means(step: spec_fn(Machine, u32) -> Stepped, before: Seq<FrameAccess>, after: Seq<FrameAccess>) -> bool {
    &&& forall|k: int| 0 <= k < before.len() ==> frame_access_means(step, #[trigger] before[k])
    &&& forall|k: int| 0 <= k < after.len() ==> frame_access_means(step, #[trigger] after[k]) || copied_means(step, after[k].word, after[k].rt)
}

/// A register-only instruction writing `rd`, as a word after.
pub open spec fn integer_word(word: u32, rd: int) -> FrameAccess {
    FrameAccess { word, rt: rd, off: 0, moves: false }
}

/// The byte stand-in piece: the words before, the check, the access cut by `bb`, the words after, the exit, the mark.
pub open spec fn byte_stand_in(before: Seq<FrameAccess>, n: u32, t: u32, store: bool, bb: int, c: int, after: Seq<FrameAccess>, slot: u64) -> Seq<u32> {
    words_of(before) + seq![check_c(n, c), cbnz_c(c, (after.len() as int + 6) * 4), cut_byte_by(n, t, store, bb)] + words_of(after)
        + exit_words(slot) + seq![eor_c(c, bb), mark_load_c(c), UDF]
}

/// The mark's meaning through `c` (Arm ARM).
pub open spec fn mark_c_means(step: spec_fn(Machine, u32) -> Stepped, bb: int, c: int) -> bool {
    &&& writes(step, eor_c(c, bb), c, |m: Machine| m.x[bb] ^ 0x1_0000_0000u64)
    &&& accesses(step, mark_load_c(c), 31, |m: Machine| (m.x[c] + 4095) % two_to_64(), 1)
    &&& stops(step, UDF)
}

/// What the byte stand-in piece's own words do (Arm ARM).
pub open spec fn byte_stand_in_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, t: u32, store: bool, bb: int, c: int, after_len: int) -> bool {
    &&& writes(step, check_c(n, c), c, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, cbnz_c(c, (after_len + 6) * 4), c, (after_len + 6) * 4, true)
    &&& accesses(step, cut_byte_by(n, t, store, bb), loaded(t, store), |m: Machine| (m.x[bb] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64(), 1)
    &&& mark_c_means(step, bb, c)
    &&& exit_means(step)
}

/// One word of the words around steps confined and keeps the frame, writing only its register.
proof fn lemma_around_step(step: spec_fn(Machine, u32) -> Stepped, f: FrameAccess, a: Allowed, m: Machine)
    requires
        frame_access_means(step, f) || copied_means(step, f.word, f.rt),
        frame_in_reach(a),
        kept(a, m),
        !m.ended,
    ensures
        ({
            let s = step(m, f.word);
            &&& forall|k: int| 0 <= k < s.accesses.len() ==> touches_allowed(a, #[trigger] s.accesses[k].0, s.accesses[k].1)
            &&& !s.system_call && s.after.unlisted == m.unlisted
            &&& kept(a, s.after)
            &&& s.after.ended || s.after.pc == (m.pc + 4) as u64
            &&& forall|r: int| 0 <= r < 31 && r != f.rt ==> s.after.x[r] == m.x[r]
            &&& (frame_access_means(step, f) && f.moves) ==> s.after.x[f.rt] == m.x[f.off] && !s.after.ended
        }),
{
    let s = step(m, f.word);
    if frame_access_means(step, f) {
        if !f.moves {
            assert((m.x[28] + f.off) % two_to_64() == m.x[28] + f.off);
        }
    } else {
        assert(copied_means(step, f.word, f.rt));
        assert(s.after.x[frame_register()] == m.x[frame_register()]);
    }
}

/// Where the byte stand-in's run stands: the frame kept, the base in its holder up to the access, the exit's moves, the
/// mark's address.
pub open spec fn byte_stand_in_inv(a: Allowed, slot: u64, b: int, al: int, bb: int, c: int, j: int, m: Machine) -> bool {
    let i = (m.pc - a.code) / 4;
    &&& kept(a, m)
    &&& i <= b ==> m.x[holder(bb, j, i)] as int == a.base
    &&& (b <= i <= b + 2 || i == b + al + 7) ==> m.x[bb] as int == a.base
    &&& (b + 3 + al) * 4 <= m.pc - a.code < (b + 7 + al) * 4 ==> at_exit(m, slot, (m.pc - a.code - (b + 3 + al) * 4) / 4)
    &&& i == b + al + 8 ==> m.x[c] as int == a.base + 0x1_0000_0000
}

/// THE BYTE STAND-IN PIECE IS CONFINED.
pub proof fn byte_stand_in_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    before: Seq<FrameAccess>,
    n: u32,
    t: u32,
    store: bool,
    bb: int,
    c: int,
    j: int,
    after: Seq<FrameAccess>,
    a: Allowed,
    guest: int,
)
    requires
        stand_in_registers(n, t, store, bb, c),
        base_moved(before, bb, j),
        around_means(step, before, after),
        byte_stand_in_means(step, n, t, store, bb, c, after.len() as int),
        in_reach(a, guest),
        frame_in_reach(a),
        a.length == (before.len() + after.len() + 10) * 4,
    ensures
        is_confined(step, byte_stand_in(before, n, t, store, bb, c, after, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = byte_stand_in(before, n, t, store, bb, c, after, slot);
    let b = before.len() as int;
    let al = after.len() as int;
    let inv = |m: Machine| byte_stand_in_inv(a, slot, b, al, bb, c, j, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == b + al + 10);
    let e = (b + 3 + al) * 4;
    assert(e / 4 == b + 3 + al);
    assert(instructions.subrange(e / 4, e / 4 + 4) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let i = o / 4;
        let w = word_at(instructions, a, m.pc);
        assert(w == instructions[i]);
        if i < b {
            assert(w == before[i].word);
            lemma_around_step(step, before[i], a, m);
            if bb != 24 && i < j {
                assert(before[i].rt != 24 && before[i].rt != bb);
            } else if bb != 24 && i > j {
                assert(before[i].rt != bb);
            } else if bb == 24 {
                assert(before[i].rt != 24);
            }
        } else if i == b {
            assert(w == check_c(n, c));
        } else if i == b + 1 {
            assert(w == cbnz_c(c, (al + 6) * 4));
        } else if i == b + 2 {
            assert(w == cut_byte_by(n, t, store, bb));
            lemma_low_word(m.x[n as int]);
        } else if i < b + 3 + al {
            assert(w == after[i - b - 3].word);
            lemma_around_step(step, after[i - b - 3], a, m);
        } else if i < b + 7 + al {
            lemma_exit_step(step, instructions, a, guest, e, m);
        } else if i == b + al + 7 {
            assert(w == eor_c(c, bb));
            lemma_mark(a, m.x[bb]);
        } else if i == b + al + 8 {
            assert(w == mark_load_c(c));
        } else {
            assert(i == b + al + 9);
            assert(w == UDF);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The doubleword stand-in piece: the words before, the check, the cut, the alignment test, the access `[x23, #off]`,
/// the words after, the exit, the mark, the slow tail with its probes.
pub open spec fn wide_stand_in(before: Seq<FrameAccess>, n: u32, t: u32, store: bool, off: int, bb: int, c: int, after: Seq<FrameAccess>, slot: u64) -> Seq<u32> {
    let al = after.len() as int;
    words_of(before)
        + seq![check_c(n, c), cbnz_c(c, (al + 9) * 4), add_cut_by(n, bb), align_c(n, c), cbnz_c(c, (al + 9) * 4), wide_at(t, store, off)]
        + words_of(after)
        + exit_words(slot)
        + seq![eor_c(c, bb), mark_load_c(c), UDF, add_c_x23(c, off + 7), and_c_page(c), cbz_c(c, 8), b_back((al + 11) * 4), movz_c(c, off), add_x23_c(c)]
        + if store { seq![probe_store_c(c), add_c_c(c, 7), probe_store_c(c), b_back((al + 17) * 4)] } else { seq![probe_load_c(c, 0), probe_load_c(c, 7), b_back((al + 16) * 4)] }
}

/// What the doubleword stand-in piece's own words do (Arm ARM).
pub open spec fn wide_stand_in_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, t: u32, store: bool, off: int, bb: int, c: int, al: int) -> bool {
    &&& writes(step, check_c(n, c), c, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, cbnz_c(c, (al + 9) * 4), c, (al + 9) * 4, true)
    &&& writes(step, add_cut_by(n, bb), 23, |m: Machine| ((m.x[bb] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& writes(step, align_c(n, c), c, |m: Machine| m.x[n as int] & 7u64)
    &&& accesses(step, wide_at(t, store, off), loaded(t, store), |m: Machine| (m.x[23] + off) % two_to_64(), 8)
    &&& mark_c_means(step, bb, c)
    &&& exit_means(step)
    &&& writes(step, add_c_x23(c, off + 7), c, |m: Machine| ((m.x[23] + off + 7) % two_to_64()) as u64)
    &&& writes(step, and_c_page(c), c, |m: Machine| m.x[c] & 0xff8u64)
    &&& branches_if(step, cbz_c(c, 8), c, 8, false)
    &&& branches(step, b_back((al + 11) * 4), -(al + 11) * 4)
    &&& writes(step, movz_c(c, off), c, |m: Machine| off as u64)
    &&& writes(step, add_x23_c(c), c, |m: Machine| ((m.x[23] + m.x[c]) % two_to_64()) as u64)
    &&& store ==> accesses(step, probe_store_c(c), 31, |m: Machine| m.x[c] as int, 1)
    &&& store ==> writes(step, add_c_c(c, 7), c, |m: Machine| ((m.x[c] + 7) % two_to_64()) as u64)
    &&& store ==> branches(step, b_back((al + 17) * 4), -(al + 17) * 4)
    &&& !store ==> accesses(step, probe_load_c(c, 0), 31, |m: Machine| m.x[c] as int, 1)
    &&& !store ==> accesses(step, probe_load_c(c, 7), 31, |m: Machine| (m.x[c] + 7) % two_to_64(), 1)
    &&& !store ==> branches(step, b_back((al + 16) * 4), -(al + 16) * 4)
}

/// Where the doubleword stand-in's run stands: the frame kept; the base in its holder up to the core and in `bb`
/// through the core, the mark and the slow tail; x23 the cut address from the cut to the access and all through the
/// slow tail; the probed address in `c` at the probes; the exit's moves; the mark's address.
pub open spec fn wide_stand_in_inv(a: Allowed, slot: u64, store: bool, off: int, b: int, al: int, bb: int, c: int, j: int, m: Machine) -> bool {
    let i = (m.pc - a.code) / 4;
    let s = b + al + 13;
    &&& kept(a, m)
    &&& i <= b ==> m.x[holder(bb, j, i)] as int == a.base
    &&& (b <= i <= b + 5 || i == b + al + 10 || s <= i) ==> m.x[bb] as int == a.base
    &&& (b + 3 <= i <= b + 5 || s <= i) ==> in_window(a, m.x[23])
    &&& i == s + 5 ==> m.x[c] as int == off
    &&& (i == s + 6 || i == s + 7) ==> m.x[c] as int == m.x[23] + off
    &&& (store && i == s + 8) ==> m.x[c] as int == m.x[23] + off + 7
    &&& (b + 6 + al) * 4 <= m.pc - a.code < (b + 10 + al) * 4 ==> at_exit(m, slot, (m.pc - a.code - (b + 6 + al) * 4) / 4)
    &&& i == b + al + 11 ==> m.x[c] as int == a.base + 0x1_0000_0000
}

/// The doubleword stand-in proof's hypotheses, bundled for its step lemmas.
pub open spec fn wide_stand_in_requires(
    step: spec_fn(Machine, u32) -> Stepped,
    before: Seq<FrameAccess>,
    n: u32,
    t: u32,
    store: bool,
    off: int,
    bb: int,
    c: int,
    j: int,
    after: Seq<FrameAccess>,
    a: Allowed,
    guest: int,
) -> bool {
    &&& stand_in_registers(n, t, store, bb, c)
    &&& 0 <= off && off + 7 < 4096 && off % 8 == 0
    &&& base_moved(before, bb, j)
    &&& around_means(step, before, after)
    &&& wide_stand_in_means(step, n, t, store, off, bb, c, after.len() as int)
    &&& in_reach(a, guest)
    &&& frame_in_reach(a)
    &&& a.length == (before.len() + after.len() + if store { 23int } else { 22int }) * 4
}

/// The step from `m` of the doubleword stand-in piece is confined and keeps its invariant.
pub open spec fn wide_step_kept(
    step: spec_fn(Machine, u32) -> Stepped,
    before: Seq<FrameAccess>,
    n: u32,
    t: u32,
    store: bool,
    off: int,
    bb: int,
    c: int,
    j: int,
    after: Seq<FrameAccess>,
    a: Allowed,
    guest: int,
    m: Machine,
) -> bool {
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = wide_stand_in(before, n, t, store, off, bb, c, after, slot);
    let st = step(m, word_at(instructions, a, m.pc));
    &&& confined_step(a, m, st)
    &&& runs(a, st.after) ==> wide_stand_in_inv(a, slot, store, off, before.len() as int, after.len() as int, bb, c, j, st.after)
}

/// The step of a word around (before or after the core) or of the exit.
proof fn lemma_wide_around(
    step: spec_fn(Machine, u32) -> Stepped,
    before: Seq<FrameAccess>,
    n: u32,
    t: u32,
    store: bool,
    off: int,
    bb: int,
    c: int,
    j: int,
    after: Seq<FrameAccess>,
    a: Allowed,
    guest: int,
    m: Machine,
)
    requires
        wide_stand_in_requires(step, before, n, t, store, off, bb, c, j, after, a, guest),
        wide_stand_in_inv(a, exit_slot(a, guest + 4) as u64, store, off, before.len() as int, after.len() as int, bb, c, j, m),
        runs(a, m),
        ({
            let i = (m.pc - a.code) / 4;
            let b = before.len() as int;
            i < b || (b + 6 <= i < b + 10 + after.len())
        }),
    ensures
        wide_step_kept(step, before, n, t, store, off, bb, c, j, after, a, guest, m),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = wide_stand_in(before, n, t, store, off, bb, c, after, slot);
    let b = before.len() as int;
    let al = after.len() as int;
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == b + al + if store { 23int } else { 22int });
    let e = (b + 6 + al) * 4;
    assert(e / 4 == b + 6 + al);
    assert(instructions.subrange(e / 4, e / 4 + 4) =~= exit_words(slot));
    let i = (m.pc - a.code) / 4;
    let w = word_at(instructions, a, m.pc);
    assert(w == instructions[i]);
    if i < b {
        assert(w == before[i].word);
        lemma_around_step(step, before[i], a, m);
        if bb != 24 && i < j {
            assert(before[i].rt != 24 && before[i].rt != bb);
        } else if bb != 24 && i > j {
            assert(before[i].rt != bb);
        } else if bb == 24 {
            assert(before[i].rt != 24);
        }
    } else if i < b + 6 + al {
        assert(w == after[i - b - 6].word);
        lemma_around_step(step, after[i - b - 6], a, m);
    } else {
        lemma_exit_step(step, instructions, a, guest, e, m);
    }
}

/// The step of a word of the core: the check, the cut, the alignment test, the access.
proof fn lemma_wide_core(
    step: spec_fn(Machine, u32) -> Stepped,
    before: Seq<FrameAccess>,
    n: u32,
    t: u32,
    store: bool,
    off: int,
    bb: int,
    c: int,
    j: int,
    after: Seq<FrameAccess>,
    a: Allowed,
    guest: int,
    m: Machine,
)
    requires
        wide_stand_in_requires(step, before, n, t, store, off, bb, c, j, after, a, guest),
        wide_stand_in_inv(a, exit_slot(a, guest + 4) as u64, store, off, before.len() as int, after.len() as int, bb, c, j, m),
        runs(a, m),
        ({
            let i = (m.pc - a.code) / 4;
            let b = before.len() as int;
            b <= i < b + 6
        }),
    ensures
        wide_step_kept(step, before, n, t, store, off, bb, c, j, after, a, guest, m),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = wide_stand_in(before, n, t, store, off, bb, c, after, slot);
    let b = before.len() as int;
    let al = after.len() as int;
    assert(instructions.len() == b + al + if store { 23int } else { 22int });
    let i = (m.pc - a.code) / 4;
    let w = word_at(instructions, a, m.pc);
    assert(w == instructions[i]);
    if i == b {
        assert(w == check_c(n, c));
    } else if i == b + 1 {
        assert(w == cbnz_c(c, (al + 9) * 4));
    } else if i == b + 2 {
        assert(w == add_cut_by(n, bb));
        lemma_low_word(m.x[n as int]);
    } else if i == b + 3 {
        assert(w == align_c(n, c));
    } else if i == b + 4 {
        assert(w == cbnz_c(c, (al + 9) * 4));
    } else {
        assert(w == wide_at(t, store, off));
    }
}

/// The step of a word of the tails: the mark and the slow tail with its probes.
proof fn lemma_wide_tail(
    step: spec_fn(Machine, u32) -> Stepped,
    before: Seq<FrameAccess>,
    n: u32,
    t: u32,
    store: bool,
    off: int,
    bb: int,
    c: int,
    j: int,
    after: Seq<FrameAccess>,
    a: Allowed,
    guest: int,
    m: Machine,
)
    requires
        wide_stand_in_requires(step, before, n, t, store, off, bb, c, j, after, a, guest),
        wide_stand_in_inv(a, exit_slot(a, guest + 4) as u64, store, off, before.len() as int, after.len() as int, bb, c, j, m),
        runs(a, m),
        (m.pc - a.code) / 4 >= before.len() + after.len() + 10,
    ensures
        wide_step_kept(step, before, n, t, store, off, bb, c, j, after, a, guest, m),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = wide_stand_in(before, n, t, store, off, bb, c, after, slot);
    let b = before.len() as int;
    let al = after.len() as int;
    let s = b + al + 13;
    assert(instructions.len() == b + al + if store { 23int } else { 22int });
    let i = (m.pc - a.code) / 4;
    let w = word_at(instructions, a, m.pc);
    assert(w == instructions[i]);
    if i == b + al + 10 {
        assert(w == eor_c(c, bb));
        lemma_mark(a, m.x[bb]);
    } else if i == b + al + 11 {
        assert(w == mark_load_c(c));
    } else if i == b + al + 12 {
        assert(w == UDF);
    } else if i == s {
        assert(w == add_c_x23(c, off + 7));
    } else if i == s + 1 {
        assert(w == and_c_page(c));
    } else if i == s + 2 {
        assert(w == cbz_c(c, 8));
    } else if i == s + 3 {
        assert(w == b_back((al + 11) * 4));
    } else if i == s + 4 {
        assert(w == movz_c(c, off));
    } else if i == s + 5 {
        assert(w == add_x23_c(c));
    } else if store {
        if i == s + 6 {
            assert(w == probe_store_c(c));
        } else if i == s + 7 {
            assert(w == add_c_c(c, 7));
        } else if i == s + 8 {
            assert(w == probe_store_c(c));
        } else {
            assert(i == s + 9);
            assert(w == b_back((al + 17) * 4));
        }
    } else {
        if i == s + 6 {
            assert(w == probe_load_c(c, 0));
        } else if i == s + 7 {
            assert(w == probe_load_c(c, 7));
        } else {
            assert(i == s + 8);
            assert(w == b_back((al + 16) * 4));
        }
    }
}

/// THE DOUBLEWORD STAND-IN PIECE IS CONFINED.
pub proof fn wide_stand_in_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    before: Seq<FrameAccess>,
    n: u32,
    t: u32,
    store: bool,
    off: int,
    bb: int,
    c: int,
    j: int,
    after: Seq<FrameAccess>,
    a: Allowed,
    guest: int,
)
    requires
        stand_in_registers(n, t, store, bb, c),
        0 <= off && off + 7 < 4096 && off % 8 == 0,
        base_moved(before, bb, j),
        around_means(step, before, after),
        wide_stand_in_means(step, n, t, store, off, bb, c, after.len() as int),
        in_reach(a, guest),
        frame_in_reach(a),
        a.length == (before.len() + after.len() + if store { 23int } else { 22int }) * 4,
    ensures
        is_confined(step, wide_stand_in(before, n, t, store, off, bb, c, after, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = wide_stand_in(before, n, t, store, off, bb, c, after, slot);
    let b = before.len() as int;
    let al = after.len() as int;
    let inv = |m: Machine| wide_stand_in_inv(a, slot, store, off, b, al, bb, c, j, m);
    assert(wide_stand_in_requires(step, before, n, t, store, off, bb, c, j, after, a, guest));
    assert(instructions.len() == b + al + if store { 23int } else { 22int });
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let st = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, st)
        &&& runs(a, st.after) ==> inv(st.after)
    } by {
        let i = (m.pc - a.code) / 4;
        if i < b || (b + 6 <= i < b + 10 + al) {
            lemma_wide_around(step, before, n, t, store, off, bb, c, j, after, a, guest, m);
        } else if i < b + 6 {
            lemma_wide_core(step, before, n, t, store, off, bb, c, j, after, a, guest, m);
        } else {
            lemma_wide_tail(step, before, n, t, store, off, bb, c, j, after, a, guest, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The pins (see copy.rs and rest.rs): one line each, the piece's function and its arguments, the words.
proof fn pin_wide_stand_in_load_x15() { assert(wide_stand_in(seq![frame_word(true, 29u32, 144int), frame_word(false, 15u32, 128int)], 1u32, 15u32, false, 0int, 24int, 29int, seq![frame_word(true, 15u32, 128int), frame_word(false, 29u32, 144int)], super::rest::at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0xf900_4b9du32, 0xf940_438fu32, 0xd360_dc3du32, 0xb500_017du32, 0x8b21_4317u32, 0x9240_083du32, 0xb500_017du32, 0xf940_02efu32, 0xf900_438fu32, 0xf940_4b9du32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_031du32, 0x397f_ffbfu32, 0x0000_0000u32, 0x9100_1efdu32, 0x927d_23bdu32, 0xb400_005du32, 0x17ff_fff3u32, 0xd280_001du32, 0x8b3d_62fdu32, 0x3940_03bfu32, 0x3940_1fbfu32, 0x17ff_ffeeu32]) by (compute); }
proof fn pin_wide_stand_in_load_x15_displaced() { assert(wide_stand_in(seq![frame_word(true, 0u32, 24int), frame_word(false, 0u32, 128int)], 1u32, 0u32, false, 0int, 24int, 15int, seq![frame_word(true, 0u32, 128int), frame_word(false, 0u32, 24int)], super::rest::at(0x6_0000_0000int, 0x40_1000int, true)) =~= seq![0xf900_0f80u32, 0xf940_4380u32, 0xd360_dc2fu32, 0xb500_016fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_016fu32, 0xf940_02e0u32, 0xf900_4380u32, 0xf940_0f80u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff3u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_ffeeu32]) by (compute); }
proof fn pin_wide_stand_in_load_through_x15() { assert(wide_stand_in(seq![frame_word(true, 29u32, 144int), frame_word(false, 15u32, 128int)], 15u32, 0u32, false, 0int, 24int, 29int, seq![frame_word(true, 15u32, 128int), frame_word(false, 29u32, 144int)], super::rest::at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0xf900_4b9du32, 0xf940_438fu32, 0xd360_ddfdu32, 0xb500_017du32, 0x8b2f_4317u32, 0x9240_09fdu32, 0xb500_017du32, 0xf940_02e0u32, 0xf900_438fu32, 0xf940_4b9du32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_031du32, 0x397f_ffbfu32, 0x0000_0000u32, 0x9100_1efdu32, 0x927d_23bdu32, 0xb400_005du32, 0x17ff_fff3u32, 0xd280_001du32, 0x8b3d_62fdu32, 0x3940_03bfu32, 0x3940_1fbfu32, 0x17ff_ffeeu32]) by (compute); }
proof fn pin_wide_stand_in_store_x24() { assert(wide_stand_in(seq![frame_word(true, 29u32, 152int), move_word(29u32, 24u32), frame_word(false, 24u32, 136int)], 1u32, 24u32, true, 8int, 29int, 15int, seq![frame_word(true, 24u32, 136int), move_word(24u32, 29u32), frame_word(false, 29u32, 152int)], super::rest::at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0xf900_4f9du32, 0xaa18_03fdu32, 0xf940_4798u32, 0xd360_dc2fu32, 0xb500_018fu32, 0x8b21_43b7u32, 0x9240_082fu32, 0xb500_018fu32, 0xf900_06f8u32, 0xf900_4798u32, 0xaa1d_03f8u32, 0xf940_4f9du32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_03afu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_3eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff2u32, 0xd280_010fu32, 0x8b2f_62efu32, 0x383f_01ffu32, 0x9100_1defu32, 0x383f_01ffu32, 0x17ff_ffecu32]) by (compute); }
proof fn pin_wide_stand_in_store_x24_displaced() { assert(wide_stand_in(seq![frame_word(true, 0u32, 24int), frame_word(false, 0u32, 136int)], 1u32, 0u32, true, 8int, 24int, 15int, seq![frame_word(false, 0u32, 24int)], super::rest::at(0x6_0000_0000int, 0x40_1000int, true)) =~= seq![0xf900_0f80u32, 0xf940_4780u32, 0xd360_dc2fu32, 0xb500_014fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_014fu32, 0xf900_06e0u32, 0xf940_0f80u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_3eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff4u32, 0xd280_010fu32, 0x8b2f_62efu32, 0x383f_01ffu32, 0x9100_1defu32, 0x383f_01ffu32, 0x17ff_ffeeu32]) by (compute); }
proof fn pin_wide_stand_in_store_through_x28() { assert(wide_stand_in(seq![frame_word(true, 1u32, 24int), frame_word(false, 1u32, 16int)], 1u32, 0u32, true, 0int, 24int, 15int, seq![frame_word(false, 1u32, 24int)], super::rest::at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0xf900_0f81u32, 0xf940_0b81u32, 0xd360_dc2fu32, 0xb500_014fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_014fu32, 0xf900_02e0u32, 0xf940_0f81u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff4u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x383f_01ffu32, 0x9100_1defu32, 0x383f_01ffu32, 0x17ff_ffeeu32]) by (compute); }
proof fn pin_byte_stand_in_load_w23() { assert(byte_stand_in(seq![frame_word(true, 1u32, 24int), frame_word(false, 1u32, 8int)], 0u32, 1u32, false, 24int, 15int, seq![frame_word(true, 1u32, 8int), frame_word(false, 1u32, 24int)], super::rest::at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0xf900_0f81u32, 0xf940_0781u32, 0xd360_dc0fu32, 0xb500_010fu32, 0x3860_4b01u32, 0xf900_0781u32, 0xf940_0f81u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32]) by (compute); }

} // verus!
