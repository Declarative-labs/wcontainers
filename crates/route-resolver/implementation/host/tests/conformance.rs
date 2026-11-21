#![cfg(target_os = "linux")]

use wcontainers_route_resolver_linux_netlink::LinuxNetlinkRouteResolver;

wcontainers_route_resolver_conformance::tests!(
    LinuxNetlinkRouteResolver::open().expect("a routing socket opens")
);
