//! A record of everything the test host gave the kernel in a run, and a host that answers from it
//! alone: the clocks, randomness, the outside's answers, the events handed in and where each turn on
//! the CPU side ends. A run replayed from its record on a fresh kernel and CPU side must reach the
//! same guest memory, which shows that nothing the kernel does depends on anything else, such as a
//! host address.

use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hasher};

use wcontainers_abi::{End, Event, HostError, MemoryId};
use wcontainers_runtime_single_kernel::Turn;

/// One thing the host gave, in the order the kernel took it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Monotonic(u64),
    Realtime(u64),
    Random(Vec<u8>),
    BackendSent(Result<(), HostError>),
    Connected(Result<(), HostError>),
    Sent(Result<usize, HostError>),
    Received(Result<usize, HostError>, Vec<u8>),
    Listened(Result<(), HostError>),
    Accepted(Result<(), HostError>),
    /// Whether the backend owed an answer, where the driver asked.
    BackendWaiting(bool),
    Turn(Turn),
    /// An event the outside handed in.
    Event(OwnedEvent),
    /// The backend's reply to a request a call of the kernel's waits on, by its number.
    BackendAnswered(u64, Vec<u8>),
    NameAnswered(u64, Vec<u8>),
    /// What a socket a wait of the kernel's is on became ready for, by its number.
    SocketReady(u32, wcontainers_readiness::ReadyFor),
    /// A TCP connect, listen or accept, and the number it gave.
    SocketMade(Result<u32, HostError>),
    /// A signal the outside sent to the first process.
    Kill(wcontainers_kernel::signal::Signal),
    /// The ends of what the outside did at a point of the driver loop, with what it answered.
    Begun,
    Arrived(bool),
    Idled,
    Delivered,
    Finished(bool),
}

/// An [`Event`] holding its own bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedEvent {
    Timer,
    BackendNotice(Vec<u8>),
    StreamData { end: End, bytes: Vec<u8> },
    StreamCredit { end: End, bytes: u64 },
    StreamClosed(End),
    TerminalResized { end: End, columns: u16, rows: u16 },
}

impl OwnedEvent {
    pub fn of(event: &Event<'_>) -> Self {
        match *event {
            Event::Timer => Self::Timer,
            Event::BackendNotice { notice } => Self::BackendNotice(notice.to_vec()),
            Event::StreamData { end, bytes } => Self::StreamData {
                end,
                bytes: bytes.to_vec(),
            },
            Event::StreamCredit { end, bytes } => Self::StreamCredit { end, bytes },
            Event::StreamClosed { end } => Self::StreamClosed(end),
            Event::TerminalResized { end, columns, rows } => {
                Self::TerminalResized { end, columns, rows }
            }
        }
    }

    pub fn event(&self) -> Event<'_> {
        match self {
            Self::Timer => Event::Timer,
            Self::BackendNotice(notice) => Event::BackendNotice { notice },
            Self::StreamData { end, bytes } => Event::StreamData { end: *end, bytes },
            Self::StreamCredit { end, bytes } => Event::StreamCredit {
                end: *end,
                bytes: *bytes,
            },
            Self::StreamClosed(end) => Event::StreamClosed { end: *end },
            Self::TerminalResized { end, columns, rows } => Event::TerminalResized {
                end: *end,
                columns: *columns,
                rows: *rows,
            },
        }
    }
}

/// A run's record, being written or being answered from.
#[derive(Clone, Debug, Default)]
pub struct Tape {
    entries: VecDeque<Entry>,
    replaying: bool,
    /// How many entries a replay has taken.
    taken: usize,
    /// Whether the driver's own work is running, whose host calls are not the kernel's and are not
    /// kept.
    pub(crate) outside: bool,
}

impl Tape {
    /// An empty record to write.
    pub fn recording() -> Self {
        Self::default()
    }

    /// The same record, to be answered from.
    pub fn replaying(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            replaying: true,
            ..Self::default()
        }
    }

    pub fn is_replaying(&self) -> bool {
        self.replaying
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Keeps an entry, when recording what the kernel took.
    pub(crate) fn keep(&mut self, entry: Entry) {
        if !self.replaying {
            self.entries.push_back(entry);
        }
    }

    /// The next entry of a replay; a replay that asks past its record or for something else than
    /// was kept has gone out of step with the run it records.
    pub(crate) fn take(&mut self) -> Entry {
        let entry = self
            .entries
            .pop_front()
            .unwrap_or_else(|| panic!("the replay asked past its record's {} entries", self.taken));
        self.taken += 1;
        entry
    }

    /// Panics for an entry taken where another kind was due.
    pub(crate) fn out_of_step(&self, entry: &Entry) -> ! {
        panic!(
            "the replay went out of step at entry {}: {entry:?} was not the kind due",
            self.taken
        )
    }

    /// How many entries a replay has taken.
    pub(crate) fn taken(&self) -> usize {
        self.taken
    }

    /// Whether a replay used its whole record.
    pub fn used_up(&self) -> bool {
        !self.replaying || self.entries.is_empty()
    }
}

/// What every guest memory held: as each was dropped, then those left at the end, by id.
pub type MemoryDigests = Vec<(MemoryId, u64)>;

/// A digest of a memory's pages that hold anything but zeros, each with its address.
pub fn digest_pages<'a>(pages: impl Iterator<Item = (u64, &'a [u8])>) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (address, bytes) in pages {
        if bytes.iter().any(|byte| *byte != 0) {
            hasher.write_u64(address);
            hasher.write(bytes);
        }
    }
    hasher.finish()
}

/// A CPU side that can say what its memories hold.
pub trait DigestMemories {
    /// Starts keeping a digest of each memory as it is dropped.
    fn keep_digests(&mut self);
    /// The digests kept, then one of each memory left, by id.
    fn digests(&mut self) -> MemoryDigests;
}
