//! Facts about bits and integers that several proofs of this crate use.

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::{lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse};
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::{lemma_pow2_adds, lemma_pow2_pos, pow2};
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::pseudocode::{highest_set_bit, ones, replicate, sint, uint};

verus! {

/// A mask of `k` ones keeps the low `k` bits of a value: the value modulo 2^k.
pub proof fn lemma_mask(x: u64, k: nat)
    requires
        k <= 64,
    ensures
        0 <= ones(k) <= u64::MAX,
        (x & (ones(k) as u64)) as int == uint(x as int, k),
{
    vstd::arithmetic::power2::lemma2_to64();
    vstd::arithmetic::power2::lemma2_to64_rest();
    if k < 64 {
        vstd::bits::lemma_u64_pow2_no_overflow(k);
        vstd::bits::lemma_u64_low_bits_mask_is_mod(x, k);
    } else {
        assert(x & u64::MAX == x) by (bit_vector);
        vstd::arithmetic::div_mod::lemma_small_mod(x as nat, pow2(64));
    }
}

/// Ones(n) shifted right by `s` is Ones(n - s).
pub proof fn lemma_ones_div(n: nat, s: nat)
    requires
        s <= n,
    ensures
        ones(n) / (pow2(s) as int) == ones((n - s) as nat),
{
    let (high, low) = (pow2((n - s) as nat) as int, pow2(s) as int);
    lemma_pow2_adds((n - s) as nat, s);
    lemma_pow2_pos(s);
    lemma_pow2_pos((n - s) as nat);
    assert((high - 1) * low == high * low - low) by (nonlinear_arith);
    lemma_fundamental_div_mod_converse(ones(n), low, high - 1, low - 1);
}

/// A value times 2^s, modulo 2^n, keeps only the value's low n - s bits, whatever its sign.
pub proof fn lemma_mul_pow2_mod(x: int, s: nat, n: nat)
    requires
        s <= n,
    ensures
        (x * pow2(s)) % (pow2(n) as int) == (x % (pow2((n - s) as nat) as int)) * pow2(s),
        0 <= (x % (pow2((n - s) as nat) as int)) * pow2(s) < pow2(n),
{
    let (high, low) = (pow2((n - s) as nat) as int, pow2(s) as int);
    lemma_pow2_adds((n - s) as nat, s);
    lemma_pow2_pos(s);
    lemma_pow2_pos((n - s) as nat);
    lemma_fundamental_div_mod(x, high);
    let (q, r) = (x / high, x % high);
    assert(x * low == q * (high * low) + r * low && 0 <= r * low < high * low) by (nonlinear_arith)
        requires
            x == high * q + r,
            0 <= r < high,
            0 < low;
    lemma_fundamental_div_mod_converse(x * low, high * low, q, r * low);
}

/// LSL by `s` within `n` bits, as a 64-bit shift and a mask computes it.
pub proof fn lemma_shift_left(x: u64, s: u32, n: nat)
    requires
        s < n <= 64,
    ensures
        ((x << s) & (ones(n) as u64)) as int == uint(x as int * pow2(s as nat), n),
{
    lemma_mask(x, n);
    lemma_mask(x, (n - s) as nat);
    let mask = ones(n) as u64;
    let kept = ones((n - s) as nat) as u64;
    lemma_ones_div(n, s as nat);
    vstd::bits::lemma_u64_shr_is_div(mask, s as u64);
    assert(mask >> s == mask >> (s as u64)) by (bit_vector);
    assert((x << s) & mask == (x & (mask >> s)) << s) by (bit_vector);
    lemma_mul_pow2_mod(x as int, s as nat, n);
    vstd::arithmetic::power2::lemma2_to64_rest();
    if n < 64 {
        vstd::arithmetic::power2::lemma_pow2_strictly_increases(n, 64);
    }
    vstd::bits::lemma_u64_shl_is_mul(x & kept, s as u64);
    assert((x & kept) << s == (x & kept) << (s as u64)) by (bit_vector);
}

/// An arithmetic shift right of a signed value is its division by 2^s, rounded down.
pub proof fn lemma_arithmetic_shift_right(x: i64, s: u32)
    requires
        s < 64,
    ensures
        (x >> s) as int == x as int / (pow2(s as nat) as int),
{
    let shifted = x >> s;
    assert(
        (x >= 0 ==> shifted >= 0 && shifted as u64 == (x as u64) >> s)
        && (x < 0 ==> shifted < 0 && (!shifted) as u64 == ((!x) as u64) >> s)
    ) by (bit_vector)
        requires
            shifted == x >> s,
    ;
    vstd::bits::lemma_u64_shr_is_div(if x >= 0 { x as u64 } else { (!x) as u64 }, s as u64);
    assert(((if x >= 0 { x as u64 } else { (!x) as u64 }) >> s) == ((if x >= 0 { x as u64 } else { (!x) as u64 }) >> (s as u64))) by (bit_vector);
    lemma_pow2_pos(s as nat);
    let d = pow2(s as nat) as int;
    if x < 0 {
        // -x - 1 = q d + r, so x = (-q - 1) d + (d - r - 1).
        assert((!x) as int == -x - 1);
        assert((!shifted) as int == -shifted - 1);
        lemma_fundamental_div_mod(-x - 1, d);
        let (q, r) = ((-x - 1) / d, (-x - 1) % d);
        assert(x == d * (-q - 1) + (d - r - 1)) by (nonlinear_arith)
            requires
                -x - 1 == d * q + r;
        lemma_fundamental_div_mod_converse(x as int, d, -q - 1, d - r - 1);
    }
}

/// The low `n` bits of a signed value's 64-bit pattern are its value modulo 2^n.
pub proof fn lemma_signed_mask(x: i64, n: nat)
    requires
        0 < n <= 64,
    ensures
        ((x as u64) & (ones(n) as u64)) as int == uint(x as int, n),
{
    lemma_mask(x as u64, n);
    let mask = ones(n) as u64;
    if x < 0 {
        lemma_mask((!x) as u64, n);
        assert((x as u64) & mask == (mask - (((!x) as u64) & mask)) as u64) by (bit_vector);
        assert((!x) as int == -x - 1);
        let p = pow2(n) as int;
        lemma_pow2_pos(n);
        lemma_fundamental_div_mod(-x - 1, p);
        let (q, r) = ((-x - 1) / p, (-x - 1) % p);
        assert(x == p * (-q - 1) + (p - 1 - r)) by (nonlinear_arith)
            requires
                -x - 1 == p * q + r;
        lemma_fundamental_div_mod_converse(x as int, p, -q - 1, p - 1 - r);
    }
}

/// A sign extension from the low `64 - s` bits, as a shift left and an arithmetic shift right compute it, is SInt.
pub proof fn lemma_sign_extension(value: u64, s: u32)
    requires
        s < 64,
    ensures
        ((((value << s) as i64) >> s) as int) == sint(value as int, (64 - s) as nat),
{
    let extended = ((value << s) as i64) >> s;
    let mask = u64::MAX >> s;
    let top = (value & mask) >> ((63 - s) as u32);
    assert(
        (extended >= 0) == (top == 0)
        && (extended >= 0 ==> extended as u64 == value & mask)
        && (extended < 0 ==> (!extended) as u64 == (mask - (value & mask)) as u64)
    ) by (bit_vector)
        requires
            extended == ((value << s) as i64) >> s,
            mask == u64::MAX >> s,
            top == (value & mask) >> ((63 - s) as u32),
            s < 64;
    let length = (64 - s) as nat;
    vstd::arithmetic::power2::lemma2_to64_rest();
    vstd::bits::lemma_unsigned_int_max_is_pow2_minus_1();
    lemma_ones_div(64, s as nat);
    vstd::bits::lemma_u64_shr_is_div(u64::MAX, s as u64);
    assert(u64::MAX >> s == u64::MAX >> (s as u64)) by (bit_vector);
    lemma_mask(value, length);
    let unsigned = (value & mask) as int;
    vstd::bits::lemma_u64_shr_is_div(value & mask, (63 - s) as u64);
    assert((value & mask) >> ((63 - s) as u32) == (value & mask) >> ((63 - s) as u64)) by (bit_vector)
        requires
            s < 64;
    let half = pow2((length - 1) as nat) as int;
    lemma_pow2_pos((length - 1) as nat);
    lemma_pow2_adds((length - 1) as nat, 1);
    vstd::arithmetic::power2::lemma2_to64();
    if unsigned < half {
        vstd::arithmetic::div_mod::lemma_basic_div(unsigned, half);
    } else {
        vstd::arithmetic::div_mod::lemma_div_non_zero(unsigned, half);
    }
    if extended >= 0 {
        assert(extended as int == (extended as u64) as int);
    } else {
        assert((!extended) as int == -extended - 1);
        assert(((!extended) as u64) as int == (!extended) as int);
        assert(((mask - (value & mask)) as u64) as int == mask - unsigned);
    }
}

/// Appending `a` zero bits to the low `l` bits of a value multiplies its SInt by 2^a.
pub proof fn lemma_sint_shift(v: int, l: nat, a: nat)
    requires
        l >= 1,
    ensures
        sint(uint(v, l) * pow2(a), l + a) == sint(v, l) * pow2(a),
{
    let (u, step) = (uint(v, l), pow2(a) as int);
    lemma_pow2_pos(a);
    lemma_pow2_pos(l);
    lemma_pow2_adds(l, a);
    lemma_pow2_adds((l - 1) as nat, a);
    lemma_pow2_adds((l - 1) as nat, 1);
    lemma_pow2_adds((l + a - 1) as nat, 1);
    assert(0 <= u * step < pow2(l + a)) by (nonlinear_arith)
        requires
            0 <= u < pow2(l),
            0 < step,
            pow2(l + a) == pow2(l) * step;
    vstd::arithmetic::div_mod::lemma_small_mod((u * step) as nat, pow2(l + a));
    assert((u * step >= pow2((l + a - 1) as nat)) == (u >= pow2((l - 1) as nat))) by (nonlinear_arith)
        requires
            0 < step,
            pow2((l + a - 1) as nat) == pow2((l - 1) as nat) * step;
    assert((u - pow2(l)) * step == u * step - pow2(l + a)) by (nonlinear_arith)
        requires
            pow2(l + a) == pow2(l) * step;
}

/// SInt of `l` bits agrees with the value modulo 2^k for k up to l.
pub proof fn lemma_sint_mod(v: int, l: nat, k: nat)
    requires
        1 <= k <= l,
    ensures
        sint(v, l) % (pow2(k) as int) == v % (pow2(k) as int),
{
    let (low, high) = (pow2(k) as int, pow2((l - k) as nat) as int);
    lemma_pow2_pos(k);
    lemma_pow2_pos((l - k) as nat);
    lemma_pow2_adds(k, (l - k) as nat);
    vstd::arithmetic::div_mod::lemma_mod_mod(v, low, high);
    if sint(v, l) != uint(v, l) {
        assert(uint(v, l) - pow2(l) == uint(v, l) - low * high) by (nonlinear_arith)
            requires
                pow2(l) == low * high;
        vstd::arithmetic::div_mod::lemma_mod_multiples_vanish(-high, uint(v, l), low);
        assert(low * -high + uint(v, l) == uint(v, l) - low * high) by (nonlinear_arith);
    }
}

/// A mask of `k` ones keeps the low `k` bits of a 32-bit value: the value modulo 2^k.
pub proof fn lemma_mask_32(x: u32, k: nat)
    requires
        k <= 32,
    ensures
        0 <= ones(k) <= u32::MAX,
        (x & (ones(k) as u32)) as int == uint(x as int, k),
{
    vstd::arithmetic::power2::lemma2_to64();
    if k < 32 {
        vstd::arithmetic::power2::lemma_pow2_strictly_increases(k, 32);
    }
    lemma_mask(x as u64, k);
    let mask = ones(k) as u32;
    assert((x & mask) as u64 == (x as u64) & (mask as u64)) by (bit_vector);
}

/// HighestSetBit of a value below 2^k is below k, and of one at least 2^k is k or more.
pub proof fn lemma_highest_set_bit_bounds(x: nat, k: nat)
    ensures
        highest_set_bit(x) >= -1,
        x < pow2(k) ==> highest_set_bit(x) < k,
        x >= pow2(k) ==> highest_set_bit(x) >= k,
    decreases x,
{
    lemma_pow2_pos(k);
    vstd::arithmetic::power2::lemma2_to64();
    if x > 0 {
        if k > 0 {
            vstd::arithmetic::power2::lemma_pow2_unfold(k);
            lemma_highest_set_bit_bounds(x / 2, (k - 1) as nat);
            let half = pow2((k - 1) as nat) as int;
            assert(x < 2 * half ==> x / 2 < half);
            assert(x >= 2 * half ==> x / 2 >= half);
        } else {
            lemma_highest_set_bit_bounds(x / 2, 0);
        }
    }
}

/// Replicate of an element below 2^e, `count` times, is below 2^(count * e).
pub proof fn lemma_replicate_bound(x: int, e: nat, count: nat)
    ensures
        0 <= replicate(x, e, count) < pow2(count * e),
    decreases count,
{
    if count == 0 {
        vstd::arithmetic::power2::lemma2_to64();
    } else {
        lemma_replicate_bound(x, e, (count - 1) as nat);
        lemma_pow2_pos(e);
        lemma_pow2_pos(((count - 1) * e) as nat);
        let (step, below) = (pow2(e) as int, pow2(((count - 1) * e) as nat) as int);
        assert(((count - 1) * e) as nat + e == count * e) by (nonlinear_arith)
            requires
                count > 0;
        lemma_pow2_adds(((count - 1) * e) as nat, e);
        let element = uint(x, e);
        assert(0 <= element < step);
        assert(0 <= element * below + replicate(x, e, (count - 1) as nat) < step * below) by (nonlinear_arith)
            requires
                0 <= element < step,
                0 <= replicate(x, e, (count - 1) as nat) < below;
    }
}

/// The low `k * e` bits of `count` copies of an element are `k` copies of it, for `k` up to `count`.
pub proof fn lemma_replicate_low(x: int, e: nat, count: nat, k: nat)
    requires
        k <= count,
    ensures
        uint(replicate(x, e, count), k * e) == replicate(x, e, k),
    decreases count,
{
    lemma_pow2_pos(k * e);
    if count == k {
        lemma_replicate_bound(x, e, k);
        vstd::arithmetic::div_mod::lemma_small_mod(replicate(x, e, k) as nat, pow2(k * e));
    } else {
        lemma_replicate_low(x, e, (count - 1) as nat, k);
        let rest = (((count - 1) - k) * e) as nat;
        assert(k * e + rest == ((count - 1) * e) as nat) by (nonlinear_arith)
            requires
                k < count,
                rest == ((count - 1) - k) * e;
        lemma_pow2_adds(k * e, rest);
        let (low, high) = (pow2(k * e) as int, pow2(rest) as int);
        let element = uint(x, e);
        assert(element * pow2(((count - 1) * e) as nat) == low * (element * high)) by (nonlinear_arith)
            requires
                pow2(((count - 1) * e) as nat) == low * high;
        vstd::arithmetic::div_mod::lemma_mod_multiples_vanish(element * high, replicate(x, e, (count - 1) as nat), low);
    }
}

/// `u64::leading_zeros` as vstd states it is 63 less HighestSetBit.
pub proof fn lemma_leading_zeros(x: u64)
    ensures
        vstd::std_specs::bits::u64_leading_zeros(x) == 63 - highest_set_bit(x as nat),
    decreases x,
{
    reveal(vstd::std_specs::bits::u64_leading_zeros);
    if x != 0 {
        lemma_leading_zeros(x / 2);
    }
}

/// An arithmetic shift right of a signed 128-bit value is its division by 2^s, rounded down.
pub proof fn lemma_arithmetic_shift_right_128(x: i128, s: u32)
    requires
        s < 128,
    ensures
        (x >> s) as int == x as int / (pow2(s as nat) as int),
{
    let shifted = x >> s;
    assert(
        (x >= 0 ==> shifted >= 0 && shifted as u128 == (x as u128) >> s)
        && (x < 0 ==> shifted < 0 && (!shifted) as u128 == ((!x) as u128) >> s)
    ) by (bit_vector)
        requires
            shifted == x >> s,
    ;
    let unsigned = if x >= 0 { x as u128 } else { (!x) as u128 };
    vstd::bits::lemma_u128_shr_is_div(unsigned, s as u128);
    assert(unsigned >> s == unsigned >> (s as u128)) by (bit_vector);
    lemma_pow2_pos(s as nat);
    let d = pow2(s as nat) as int;
    if x < 0 {
        // -x - 1 = q d + r, so x = (-q - 1) d + (d - r - 1).
        assert((!x) as int == -x - 1);
        assert((!shifted) as int == -shifted - 1);
        lemma_fundamental_div_mod(-x - 1, d);
        let (q, r) = ((-x - 1) / d, (-x - 1) % d);
        assert(x == d * (-q - 1) + (d - r - 1)) by (nonlinear_arith)
            requires
                -x - 1 == d * q + r;
        lemma_fundamental_div_mod_converse(x as int, d, -q - 1, d - r - 1);
    }
}

/// A value modulo 2^64, then modulo 2^n for n up to 64, is the value modulo 2^n.
pub proof fn lemma_wrapped_mod(x: int, n: nat)
    requires
        n <= 64,
    ensures
        (x % (pow2(64) as int)) % (pow2(n) as int) == x % (pow2(n) as int),
{
    lemma_pow2_pos(n);
    lemma_pow2_pos((64 - n) as nat);
    lemma_pow2_adds(n, (64 - n) as nat);
    vstd::arithmetic::div_mod::lemma_mod_mod(x, pow2(n) as int, pow2((64 - n) as nat) as int);
}

/// A width's count of bits divides 2^width, so UInt of a register's low bits modulo the width is the register modulo
/// the width.
pub proof fn lemma_modulo_width(x: int, n: nat)
    requires
        n == 32 || n == 64,
    ensures
        uint(x, n) % (n as int) == x % (n as int),
{
    vstd::arithmetic::power2::lemma2_to64();
    vstd::arithmetic::power2::lemma2_to64_rest();
    if n == 32 {
        assert(pow2(32) == 32 * 0x800_0000);
        vstd::arithmetic::div_mod::lemma_mod_mod(x, 32, 0x800_0000);
    } else {
        assert(pow2(64) == 64 * 0x400_0000_0000_0000);
        vstd::arithmetic::div_mod::lemma_mod_mod(x, 64, 0x400_0000_0000_0000);
    }
}

/// A signed value's 64-bit pattern, as `as u64` gives it, is its value modulo 2^64.
pub proof fn lemma_signed_pattern(x: i64)
    ensures
        (x as u64) as int == x as int % (pow2(64) as int),
{
    lemma_signed_mask(x, 64);
    assert((x as u64) & (0xffff_ffff_ffff_ffffu64) == x as u64) by (bit_vector);
    vstd::arithmetic::power2::lemma2_to64();
}

} // verus!
