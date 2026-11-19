//! What the untranslated run and the monitor's speed test say to each other, after the floor's own requests:
//! one definition, read by the program and included by the test.

// Each side uses its own half.
#![allow(dead_code)]

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

wcontainers_inside::requests! {
    first: wcontainers_inside::link::Request::END;
    /// The program's loadable segments. Answered with [`Program`].
    Segments {} -> Program;
    /// Segment `index`, its file bytes as the answer's bytes. Answered with [`Segment`].
    Load { index } -> Segment;
    /// The program wrote the request's bytes to its standard output.
    Output {} -> ();
    /// The program exited with `status`.
    Exited { status } -> ();
}

/// How many loadable segments the program has, where it starts, and how many bytes its stack takes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Program {
    pub segments: u64,
    pub entry: u64,
    pub stack: u64,
}

/// One loadable segment: its address, its length in memory and in the file, and whether it may be written
/// and run.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Segment {
    pub address: u64,
    pub memory_length: u64,
    pub file_length: u64,
    pub writable: u64,
    pub executable: u64,
}
