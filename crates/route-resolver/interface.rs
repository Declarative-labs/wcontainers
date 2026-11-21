#![no_std]

use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{NeighbourConfirmation, Route, RouteSelectors};
use wcontainers_kernel_interface::UserId;

pub trait RouteResolver {
    /// Returns the route a packet with these selectors would take, sending nothing.
    fn resolve_route(
        &self,
        route: RouteSelectors<'_>,
        owner: UserId,
        neighbour: NeighbourConfirmation,
    ) -> Result<Route, HostError>;
}
