//! Arm's shared pseudocode that the integer instructions use, as specification functions: each is the function of
//! the [shared pseudocode](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer)
//! it names, read over integers, with a bit string of `n` bits read as its unsigned value.

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

verus! {

/// [UInt](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.UInt.1)
/// of the low `n` bits of `x`.
pub open spec fn uint(x: int, n: nat) -> int {
    x % (pow2(n) as int)
}

/// [SInt](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.SInt.1)
/// of the low `n` bits of `x`: their unsigned value, less 2^n when the top one is set.
pub open spec fn sint(x: int, n: nat) -> int {
    if n > 0 && uint(x, n) >= pow2((n - 1) as nat) { uint(x, n) - pow2(n) } else { uint(x, n) }
}

/// [Ones](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-common#impl-shared.Ones.1)
/// of `n` bits, as an unsigned value.
pub open spec fn ones(n: nat) -> int {
    pow2(n) - 1
}

/// [IsZero](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-common#impl-shared.IsZero.1)
/// of the low `n` bits of `x`.
pub open spec fn is_zero(x: int, n: nat) -> bool {
    uint(x, n) == 0
}

/// [AddWithCarry](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-integer#impl-shared.AddWithCarry.3)
/// of the low `n` bits of `x` and `y`: the result's unsigned value, then N, Z, C and V.
pub open spec fn add_with_carry(x: int, y: int, carry_in: bool, n: nat) -> (int, bool, bool, bool, bool) {
    let carry = if carry_in { 1int } else { 0int };
    let unsigned_sum = uint(x, n) + uint(y, n) + carry;
    let signed_sum = sint(x, n) + sint(y, n) + carry;
    let result = uint(unsigned_sum, n);
    // N is the result's top bit, which is set exactly when SInt of the result is negative.
    (result, sint(result, n) < 0, is_zero(result, n), uint(result, n) != unsigned_sum, sint(result, n) != signed_sum)
}

} // verus!
