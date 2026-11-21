//! The tests every implementation of wcontainers-route-resolver runs, written against its traits alone.

use core::net::Ipv4Addr;

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options};
use wcontainers_internet_protocol::{
    BroadcastPermission, Ipv4RouteSelectors, NeighbourConfirmation, PacketMark, RouteScope,
    RouteSelectors, SourceAddressCheck, UpperLayer,
};
use wcontainers_kernel_interface::UserId;
use wcontainers_route_resolver::RouteResolver;

/// Calls each method of `RouteResolver` as a placeholder that fails until the real checks are written.
pub fn call_route_resolver_methods<R: RouteResolver>(resolver: R) {
    let options = Ipv4Options::new();
    let route = Ipv4RouteSelectors {
        destination: Ipv4Addr::LOCALHOST,
        source: None,
        source_address_check: SourceAddressCheck::Required,
        interface: None,
        dscp: IpDscp::ZERO,
        ecn: IpEcn::NotEct,
        mark: PacketMark(u32::MIN),
        scope: RouteScope::Universe,
        broadcast: BroadcastPermission::Refused,
        options: &options,
        upper_layer: UpperLayer {
            protocol: IpNumber::ICMP,
            selector: None,
        },
    };
    let _ = resolver.resolve_route(
        RouteSelectors::V4(route),
        UserId(u32::MIN),
        NeighbourConfirmation::Unconfirmed,
    );
    todo!("this is a placeholder: the real checks of RouteResolver are not written yet");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn call_route_resolver_methods() {
            $crate::call_route_resolver_methods($value);
        }
    };
}
