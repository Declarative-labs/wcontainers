#![no_std]

use vstd::prelude::*;
use wcontainers_abi::HostError;

verus! {

pub trait NegativeRouteAdvisable<Socket> {
    /// Tells the socket its route is failing, so that it takes a new flow hash where it may rehash and drops its cached route.
    ///
    /// The route it drops is an IPv4 one that was redirected, expires or is obsolete, and an IPv6 one unless it is an exception route that has not expired, an expired exception route also leaving the routing table.
    fn give_negative_route_advice(&self, socket: &Socket) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
