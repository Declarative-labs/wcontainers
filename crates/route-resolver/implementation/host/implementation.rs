//! A route resolver for the calling thread's network namespace, through Linux's routing socket.
#![cfg(unix)]

#[cfg(target_os = "linux")]
mod linux;

use wcontainers_abi::{CounterReading, HostError};
use wcontainers_internet_protocol::{NeighbourConfirmation, Route, RouteSelectors};
use wcontainers_internet_protocol_linux_socket_options::Shortfalls;
use wcontainers_kernel::UserId;
use wcontainers_route_resolver::RouteResolver;

pub struct HostRouteResolver {
    #[cfg(target_os = "linux")]
    buffers: std::sync::Mutex<linux::Buffers>,
    shortfalls: Shortfalls,
}

impl HostRouteResolver {
    /// Opens a resolver; each lookup asks the routing of the namespace its calling thread is in.
    pub fn open() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            buffers: std::sync::Mutex::default(),
            shortfalls: Shortfalls::default(),
        }
    }

    /// Returns how many lookups were refused, for each reason that refused one.
    pub fn counters(&self) -> Vec<CounterReading> {
        self.shortfalls.readings()
    }
}

impl RouteResolver for HostRouteResolver {
    #[cfg(target_os = "linux")]
    fn resolve_route(
        &self,
        route: RouteSelectors<'_>,
        owner: UserId,
        neighbour: NeighbourConfirmation,
    ) -> Result<Route, HostError> {
        // A lookup that finds the kept buffers in use takes buffers of its own rather than wait for them.
        match self.buffers.try_lock() {
            Ok(mut buffers) => {
                linux::resolve(&mut buffers, &self.shortfalls, route, owner, neighbour)
            }
            Err(std::sync::TryLockError::Poisoned(poisoned)) => linux::resolve(
                &mut poisoned.into_inner(),
                &self.shortfalls,
                route,
                owner,
                neighbour,
            ),
            Err(std::sync::TryLockError::WouldBlock) => linux::resolve(
                &mut linux::Buffers::default(),
                &self.shortfalls,
                route,
                owner,
                neighbour,
            ),
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn resolve_route(
        &self,
        _route: RouteSelectors<'_>,
        _owner: UserId,
        _neighbour: NeighbourConfirmation,
    ) -> Result<Route, HostError> {
        /// Why a lookup is refused on a system without Linux's routing socket.
        const NO_ROUTING_SOCKET: &str =
            "route lookup: this system has no Linux routing socket to ask";
        Err(self.shortfalls.refuse(NO_ROUTING_SOCKET))
    }
}
