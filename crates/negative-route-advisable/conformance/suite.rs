//! The tests every implementation of wcontainers-negative-route-advisable runs, written against its traits alone.
//!
//! Each takes the implementor with a handle to advise, made by the suite of a trait that requires
//! `NegativeRouteAdvisable`, which checks the advice's effect on the route itself.

use wcontainers_abi::HostError;
use wcontainers_negative_route_advisable::NegativeRouteAdvisable;

/// Returns the answer to advice about the handle's route, checking that it is taken or honestly refused.
fn taken_or_refused<A, Handle>(advice: &A, handle: &Handle) -> Result<(), HostError>
where
    A: NegativeRouteAdvisable<Handle>,
{
    let answer = advice.give_negative_route_advice(handle);
    assert!(
        matches!(answer, Ok(()) | Err(HostError::NotSupported)),
        "advice is taken or refused as not supported, not {answer:?}"
    );
    answer
}

/// Checks that advice about a handle's route is taken, or refused as not supported by a host that cannot take it.
pub fn advice_is_taken<A, Handle>((advice, handle): (A, Handle))
where
    A: NegativeRouteAdvisable<Handle>,
{
    taken_or_refused(&advice, &handle).ok();
}

/// Checks that advice given again, when the new route fails too, gets the answer the first advice got.
pub fn advice_is_taken_again<A, Handle>((advice, handle): (A, Handle))
where
    A: NegativeRouteAdvisable<Handle>,
{
    let first = taken_or_refused(&advice, &handle);
    assert_eq!(advice.give_negative_route_advice(&handle), first);
}

/// Runs each suite function above as its own test on the value a requiring suite gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn advice_is_taken() {
            $crate::advice_is_taken($value);
        }

        #[test]
        fn advice_is_taken_again() {
            $crate::advice_is_taken_again($value);
        }
    };
}

#[cfg(test)]
mod suite {
    use core::cell::Cell;

    use wcontainers_abi::HostError;
    use wcontainers_negative_route_advisable::NegativeRouteAdvisable;

    /// Tallies the advice it takes, as a correct implementation may.
    struct Counts(Cell<u32>);

    impl NegativeRouteAdvisable<()> for Counts {
        fn give_negative_route_advice(&self, _handle: &()) -> Result<(), HostError> {
            self.0.set(self.0.get() + 1);
            Ok(())
        }
    }

    /// Refuses every advice as not supported, as a correct implementation on a host that cannot take it does.
    struct Refuses;

    impl NegativeRouteAdvisable<()> for Refuses {
        fn give_negative_route_advice(&self, _handle: &()) -> Result<(), HostError> {
            Err(HostError::NotSupported)
        }
    }

    /// Takes advice once and refuses it after, which the suite must catch.
    struct TakesOnce(Cell<bool>);

    impl NegativeRouteAdvisable<()> for TakesOnce {
        fn give_negative_route_advice(&self, _handle: &()) -> Result<(), HostError> {
            if self.0.replace(true) {
                Err(HostError::NotSupported)
            } else {
                Ok(())
            }
        }
    }

    /// Answers advice about a live handle as if it were closed, which is neither taking nor an honest refusal.
    struct Fails;

    impl NegativeRouteAdvisable<()> for Fails {
        fn give_negative_route_advice(&self, _handle: &()) -> Result<(), HostError> {
            Err(HostError::Closed)
        }
    }

    crate::tests!((Counts(Cell::new(0)), ()));

    mod refusing {
        crate::tests!((super::Refuses, ()));
    }

    #[test]
    #[should_panic]
    fn catches_advice_taken_only_once() {
        crate::advice_is_taken_again((TakesOnce(Cell::new(false)), ()));
    }

    #[test]
    #[should_panic]
    fn catches_advice_answered_with_an_error() {
        crate::advice_is_taken((Fails, ()));
    }

    #[test]
    #[should_panic]
    fn catches_advice_answered_with_an_error_again() {
        crate::advice_is_taken_again((Fails, ()));
    }
}
