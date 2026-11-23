//! What a wait and a receive allocate, counted over the whole test process, of which this test is the only one.
#![cfg(target_os = "linux")]

mod support;

use std::alloc::System;
use std::sync::Arc;

use stats_alloc::{INSTRUMENTED_SYSTEM, StatsAlloc};
use wcontainers_abi::HostError;
use wcontainers_udp_sockets::UdpSockets;
use wcontainers_udp_sockets_host::HostUdpSockets;

use support::{Wakes, pair, wait_and_take};

#[global_allocator]
static COUNTING: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

/// The allocations and reallocations made so far.
fn allocations() -> usize {
    let stats = COUNTING.stats();
    stats.allocations + stats.reallocations
}

/// How many waits or receives each check counts.
const DATAGRAMS: usize = 1200;

/// How many waits warm a socket's waiter list before allocations are counted.
const WARM_UP: usize = 10;

/// A call that has to wait allocates nothing once its socket has been waited on, and a receive with nothing queued
/// allocates nothing.
#[test]
fn a_warm_wait_and_a_receive_that_would_block_allocate_nothing() {
    let sockets = HostUdpSockets::new().expect("make the host's UDP sockets");
    let (socket, sender) = pair(&sockets);
    let mut into = vec![u8::MIN; usize::from(u16::MAX)];
    let wakes = Arc::new(Wakes::default());
    for woken in 1..=WARM_UP {
        wait_and_take(&sockets, &socket, &sender, &wakes, woken, &mut into);
    }
    let before = allocations();
    for woken in WARM_UP + 1..=WARM_UP + DATAGRAMS {
        wait_and_take(&sockets, &socket, &sender, &wakes, woken, &mut into);
    }
    assert_eq!(
        allocations() - before,
        0,
        "allocations over {DATAGRAMS} waits"
    );

    assert_eq!(
        sockets.receive(&socket, &mut into),
        Err(HostError::WouldBlock)
    );
    let before = allocations();
    for _ in 0..DATAGRAMS {
        assert_eq!(
            sockets.receive(&socket, &mut into),
            Err(HostError::WouldBlock)
        );
    }
    assert_eq!(
        allocations() - before,
        0,
        "allocations over {DATAGRAMS} receives that would block"
    );
    let counted = Box::new(u8::MIN);
    assert_eq!(allocations() - before, 1, "the allocator counts");
    drop(counted);
}
