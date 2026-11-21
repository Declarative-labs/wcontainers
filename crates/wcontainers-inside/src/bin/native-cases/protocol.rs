//! What the native cases program and the monitor's native test say to each other, after the floor's own
//! requests: one definition, read by the program and included by the test.

// Each side uses its own half.
#![allow(dead_code)]

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

wcontainers_inside::requests! {
    first: wcontainers_inside::link::Request::END;
    /// The next region to map; `frame` is the first guest-physical frame of the region mapped before (0 for
    /// none). Answered with a region of no bytes when every region is mapped.
    Map { frame } -> Region;
    /// The next run: answered with the code to make visible before it, or with no bytes when there is none.
    Next {} -> Code;
    /// The program has finished.
    Done {} -> ();
}

/// A region the program maps at `va`: `bytes` of the given [`Kind`]'s word.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Region {
    pub va: u64,
    pub bytes: u64,
    pub kind: u64,
}

/// What a region holds and who may use it.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Guest memory the guest's level reads and writes, fresh frames in a row.
    ReadWrite,
    /// Guest memory the guest's level reads only, fresh frames in a row.
    ReadOnly,
    /// Guest memory the guest's level reads only, every page but the first the first one's frame.
    Repeated,
    /// Translated code in the code area, fresh frames in a row.
    Code,
    /// The thread's register page.
    RegisterPage,
    /// The exception vectors and the gate, which the host writes and the inside runs.
    Gate,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::ReadWrite,
        Kind::ReadOnly,
        Kind::Repeated,
        Kind::Code,
        Kind::RegisterPage,
        Kind::Gate,
    ];

    pub fn from_word(word: u64) -> Option<Kind> {
        Self::ALL.into_iter().find(|k| *k as u64 == word)
    }
}

/// Code written for a run, to be made visible.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Code {
    pub va: u64,
    pub bytes: u64,
}

/// Where the gate starts in its region: after the vector table. The program calls it for each run; the host
/// runs the guest's level from it and comes back after it.
pub const GATE_OFFSET: u64 = wcontainers_inside::layout::VECTOR_TABLE_BYTES;
