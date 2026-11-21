#![cfg(target_os = "linux")]

use wcontainers_route_resolver_host::HostRouteResolver;

wcontainers_route_resolver_conformance::tests!(
    HostRouteResolver::open().expect("a routing socket opens")
);
