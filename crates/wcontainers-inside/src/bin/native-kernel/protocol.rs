//! What the native kernel program and the monitor's kernel test say to each other, after the inside's own
//! requests: one definition, read by the program and included by the test.

// Each side uses its own half.
#![allow(dead_code)]

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

wcontainers_inside::requests! {
    first: wcontainers_inside::container::Request::END;
    /// What the test crate's policy is made from. Answered with [`PolicySeeds`].
    Policy {} -> PolicySeeds;
    /// A slow path or refusal the policy kept: its reason in the link's bytes.
    Warned { count, detail } -> ();
    /// The container's first process ended with the wait status `status` after `turns` turns; its threads
    /// stopped `stops` times, at positions whose hash is `positions`.
    Ended { status, turns, stops, positions } -> ();
}

/// The test crate's policy's seed, the wall clock's reading at the start, and the counted events after which every
/// turn ends (none: the schedule's limits).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct PolicySeeds {
    pub seed: u64,
    pub wall_start: u64,
    pub turn_events: u64,
}
