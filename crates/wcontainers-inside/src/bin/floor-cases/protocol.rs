//! What the floor's cases and the monitor's floor test say to each other, after the floor's own requests:
//! one definition, read by the cases program and included by the test.

// Each side uses its own half.
#![allow(dead_code)]

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

wcontainers_inside::requests! {
    first: wcontainers_inside::link::Request::END;
    /// Nothing; answered at once (the round trip's cost).
    Nothing {} -> ();
    /// The value comes back as it went.
    Echo { value } -> Echo;
    /// The inside has finished.
    Done {} -> ();
    /// Write `bytes` at a guest-physical address (code into a frame the inside gave).
    Write { at } -> ();
    /// A case's result, a [`Case`]'s word: what it saw, in `bytes`.
    Report {
        case,
        /// 1 when it passed.
        passed
    } -> ();
    /// A measurement, not a check, a [`Case`]'s word: what was measured, in `bytes`.
    Measurement { case } -> ();
    /// The host process's memory footprint.
    Footprint {} -> Bytes;
    /// Raise an interrupt after a delay, from another host thread; answered at once.
    RaiseLater { interrupt, delay_us } -> ();
    /// Lower an interrupt; answered with the guest's counter at the moment it was raised.
    Lower { interrupt } -> CounterValue;
    /// Start another virtual CPU on a new host thread and answer when it is done.
    StartCpu { cpu } -> ();
    /// Stop this virtual CPU from another host thread, as many times as the test chooses, while it runs at
    /// the guest's level in the loop from `loop_start` to `loop_end` counting in general register `counter`,
    /// then raise `interrupt`.
    StopTest { interrupt, loop_start, loop_end, counter } -> ();
}

/// A number of bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Bytes {
    pub bytes: u64,
}

/// A reading of the guest's virtual counter.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct CounterValue {
    pub counter: u64,
}

macro_rules! cases {
    ($($(#[$doc:meta])* $name:ident,)*) => {
        /// Every item of the bar: each is reported exactly once, as a check or as a measurement.
        #[repr(u64)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Case { $($(#[$doc])* $name,)* }

        impl Case {
            /// Every case.
            pub const ALL: &[Case] = &[$(Case::$name,)*];

            /// The case a report's word names.
            pub fn from_word(word: u64) -> Option<Case> {
                Self::ALL.iter().copied().find(|c| *c as u64 == word)
            }
        }
    };
}

cases! {
    ExecuteOnly,
    SystemCallOut,
    GuestNoExecute,
    MisalignedBranch,
    Tagged,
    StopWord,
    RegisterPage,
    ProtectInsidePage,
    TwoContainers,
    ThreadViews,
    AdjacentPages,
    Block,
    LateCode,
    Step,
    Timer,
    Raised,
    /// The doorbell's round trip, a measurement.
    Doorbell,
    GiveBack,
    SecondCpu,
    /// The spin the host stops ends by a raised interrupt.
    Stops,
    /// The host's stops land in the spin and its count never goes back; reported by the host.
    HostStops,
}
