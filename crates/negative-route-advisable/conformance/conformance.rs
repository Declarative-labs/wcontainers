//! The tests every implementation of wcontainers-negative-route-advisable runs, written against its traits alone.
//!
//! Each takes the implementor with a handle to advise, made by the suite of a trait that requires
//! `NegativeRouteAdvisable`, which checks the advice's effect on the route itself.

use wcontainers_negative_route_advisable::NegativeRouteAdvisable;

/// Checks that advice about a handle's route is taken.
pub fn advice_is_taken<A, Handle>((advice, handle): (A, Handle))
where
    A: NegativeRouteAdvisable<Handle>,
{
    assert_eq!(advice.give_negative_route_advice(&handle), Ok(()));
}

/// Checks that advice is taken again when the new route fails too.
pub fn advice_is_taken_again<A, Handle>((advice, handle): (A, Handle))
where
    A: NegativeRouteAdvisable<Handle>,
{
    assert_eq!(advice.give_negative_route_advice(&handle), Ok(()));
    assert_eq!(advice.give_negative_route_advice(&handle), Ok(()));
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

    crate::tests!((Counts(Cell::new(0)), ()));

    #[test]
    #[should_panic]
    fn catches_advice_taken_only_once() {
        crate::advice_is_taken_again((TakesOnce(Cell::new(false)), ()));
    }
}
