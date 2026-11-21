//! The tests every implementation of wcontainers-negative-route-advice runs, written against its traits alone.

use wcontainers_negative_route_advice::NegativeRouteAdvice;

/// TODO: temporary placeholder: a suite given only a `NegativeRouteAdvice` has no handle to call it with.
pub fn call_negative_route_advice_methods<A, Handle>(_advice: A)
where
    A: NegativeRouteAdvice<Handle>,
{
    todo!("this is a placeholder: the real checks of NegativeRouteAdvice are not written yet");
}
