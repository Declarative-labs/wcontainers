//! Facts about bits and integers that several proofs of this crate use.

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::div_mod::{lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse};
#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::{lemma_pow2_adds, lemma_pow2_pos, pow2};
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::pseudocode::{ones, sint, uint};

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

/// A value times 2^s, modulo 2^n, keeps only the value's low n - s bits.
pub proof fn lemma_mul_pow2_mod(x: int, s: nat, n: nat)
    requires
        0 <= x,
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

} // verus!
