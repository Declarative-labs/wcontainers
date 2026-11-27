#![no_std]

use vstd::prelude::*;
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{NeighbourConfirmation, Route, RouteSelectors};
use wcontainers_kernel::UserId;

verus! {

pub trait RouteResolver {
    /// Returns the route a packet with these selectors would take, sending nothing.
    fn resolve_route(
        &self,
        route: RouteSelectors<'_>,
        owner: UserId,
        neighbour: NeighbourConfirmation,
    ) -> Result<Route, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
