//! The link: the one way the inside asks the host for something. The inside writes a [`Link`] page and rings
//! the doorbell; the monitor answers in the same page and resumes the virtual CPU.

use wcontainers_abi::PAGE_SIZE;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// What the monitor tells a virtual CPU when it starts it, in memory the monitor wrote. Its inside address is
/// in x0 at entry.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Start {
    /// Bytes of guest-physical memory from [`crate::layout::MEMORY_BASE`].
    pub memory: u64,
    /// The first guest-physical address the inside may take frames from; everything below is the image, the
    /// boot tables, the stacks and the link pages.
    pub free: u64,
    /// The guest-physical address of this virtual CPU's link page.
    pub link: u64,
    /// This virtual CPU's index, which is also its redistributor's.
    pub cpu: u64,
    /// Virtual CPUs the machine can have, so redistributors the interrupt controller has.
    pub cpus: u64,
    /// The host's page size: memory is given back in whole host pages.
    pub host_page: u64,
    /// The guest-physical address of the top-level table of the boot map of the inside's half.
    pub boot_root: u64,
    /// The interrupt number of the virtual timer.
    pub timer_interrupt: u64,
    /// The first interrupt number the host may raise.
    pub first_raised_interrupt: u64,
}

/// Words a [`Link`] holds a request's message or its answer in.
pub const LINK_WORDS: usize = 8;

/// Bytes of text or data a [`Link`] carries.
pub const LINK_BYTES: usize = PAGE_SIZE as usize - (LINK_WORDS + 2) * size_of::<u64>();

/// One request and its answer.
#[repr(C)]
pub struct Link {
    /// A [`Request`].
    pub request: u64,
    /// The request's [`Message`] in, its answer out.
    message: [u64; LINK_WORDS],
    /// Bytes used of `bytes`.
    pub length: u64,
    /// Text or data.
    pub bytes: [u8; LINK_BYTES],
}

const _: () = assert!(size_of::<Link>() as u64 == PAGE_SIZE);

/// What one request carries, and what its answer is.
pub trait Message: IntoBytes + FromBytes + Immutable + KnownLayout + Copy {
    /// The request this message is.
    const REQUEST: Request;
    /// The answer the monitor writes back.
    type Answer: IntoBytes + FromBytes + Immutable + KnownLayout + Copy;
}

impl Link {
    /// Writes `value` as the message or answer.
    pub fn put<T: IntoBytes + Immutable>(&mut self, value: &T) {
        const { assert!(size_of::<T>() <= size_of::<[u64; LINK_WORDS]>()) };
        let words = self.message.as_mut_bytes();
        words.fill(0);
        words[..size_of::<T>()].copy_from_slice(value.as_bytes());
    }

    /// Reads the message or answer as `T`.
    pub fn get<T: FromBytes + KnownLayout + Immutable>(&self) -> T {
        T::read_from_prefix(self.message.as_bytes())
            .expect("a message fits its link")
            .0
    }

    /// Writes `message` and `bytes` as a request.
    pub fn ask<M: Message>(&mut self, message: &M, bytes: &[u8]) {
        self.request = M::REQUEST as u64;
        self.put(message);
        self.length = bytes.len() as u64;
        self.bytes[..bytes.len()].copy_from_slice(bytes);
    }

    /// The bytes the request carries.
    pub fn data(&self) -> &[u8] {
        &self.bytes[..self.length as usize]
    }
}

macro_rules! requests {
    ($(
        $(#[$doc:meta])*
        $name:ident { $($(#[$field_doc:meta])* $field:ident),* } -> $answer:ty;
    )*) => {
        /// What the inside asks.
        #[repr(u64)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Request { $($(#[$doc])* $name,)* }

        impl Request {
            /// The request a [`Link::request`] word holds.
            pub fn from_word(word: u64) -> Option<Request> {
                [$(Request::$name,)*].into_iter().find(|r| *r as u64 == word)
            }
        }

        $(
            $(#[$doc])*
            #[repr(C)]
            #[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
            pub struct $name { $($(#[$field_doc])* pub $field: u64,)* }

            impl Message for $name {
                const REQUEST: Request = Request::$name;
                type Answer = $answer;
            }
        )*
    };
}

requests! {
    /// Nothing; answered at once (the round trip's cost).
    Nothing {} -> ();
    /// The value comes back as it went.
    Echo { value } -> Echo;
    /// Text for the host's log, in `bytes`.
    Print {} -> ();
    /// The inside has finished.
    Done {
        /// Reports and measurements it sent.
        sent
    } -> ();
    /// The inside faulted itself.
    Fatal { esr, elr, far } -> ();
    /// Write `bytes` at a guest-physical address (code into a frame the inside gave).
    Write { at } -> ();
    /// Give back the host pages of a guest-physical range; they read as zeros after.
    GiveBack { at, length } -> ();
    /// A test case's result: its name and what it saw, in `bytes`.
    Report {
        /// 1 when it passed.
        passed
    } -> ();
    /// A measurement, not a check: what was measured, in `bytes`.
    Measurement {} -> ();
    /// The host process's memory footprint.
    Footprint {} -> Bytes;
    /// Raise an interrupt after a delay, from another host thread; answered at once.
    RaiseLater { interrupt, delay_us } -> ();
    /// Lower an interrupt; answered with the guest's counter at the moment it was raised.
    Lower { interrupt } -> CounterValue;
    /// Start another virtual CPU on a new host thread and answer when it is done.
    StartCpu { cpu } -> ();
    /// Stop this virtual CPU from another host thread, as many times as the host chooses, while it runs at
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
