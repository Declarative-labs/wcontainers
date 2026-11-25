//! The one trusted function the provided methods call; it is not part of the specification.

use core::convert::Infallible;

use vstd::prelude::*;

verus! {

/// The value of a result whose error is `Infallible`, which no value has.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
pub fn infallible_ok<T>(result: Result<T, Infallible>) -> (value: T)
    ensures
        result == Ok::<T, Infallible>(value),
{
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

} // verus!

#[cfg(test)]
mod tests {
    extern crate std;

    use core::convert::Infallible;

    use super::infallible_ok;

    /// The one trusted line of this crate, checked on every kind of value it can be given: an `Ok`, since no `Err` of
    /// `Infallible` can be made.
    #[test]
    fn the_value_of_an_infallible_result_is_its_success() {
        for value in [u64::MIN, 1, u64::MAX] {
            assert_eq!(infallible_ok(Ok::<u64, Infallible>(value)), value);
        }
        let text = alloc::string::String::from("kept");
        assert_eq!(infallible_ok(Ok::<_, Infallible>(text.clone())), text);
    }
}
