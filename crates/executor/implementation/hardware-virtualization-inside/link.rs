//! The link: the one way the inside asks the host for something. The inside writes a [`Link`] page and rings
//! the doorbell; the monitor answers in the same page and resumes the virtual CPU.

use alloc::vec::Vec;

use wcontainers_abi::PAGE_SIZE;
use zerocopy::{FromBytes, FromZeros, Immutable, IntoBytes, KnownLayout};

/// The bit of an SMCCC function number where its owner starts.
const SMCCC_OWNER_SHIFT: u32 = 24;

/// SMCCC's owner of OEM service calls, which neither hypervisor defines a call of.
const SMCCC_OWNER_OEM: u32 = 3;

/// The doorbell: an `hvc #0` with this SMCCC function number in w0, a yielding 32-bit call. KVM forwards to the
/// monitor only the functions its filter names, and answers every other `hvc` itself; Hypervisor.framework
/// answers a fast call itself.
pub const DOORBELL_FUNCTION: u32 = SMCCC_OWNER_OEM << SMCCC_OWNER_SHIFT;

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

/// The words of a [`Link`] before its bytes.
#[repr(C)]
#[derive(FromZeros)]
pub struct LinkHead {
    /// The request's word ([`Message::REQUEST`]).
    pub request: u64,
    /// The request's [`Message`] in, its answer out.
    message: [u64; LINK_WORDS],
    /// Bytes used of the link's bytes.
    pub length: u64,
}

/// Bytes of text or data a [`Link`] carries: the rest of its page.
pub const LINK_BYTES: usize = PAGE_SIZE as usize - size_of::<LinkHead>();

/// One request and its answer.
#[repr(C)]
#[derive(FromZeros)]
pub struct Link {
    /// The request and its message.
    pub head: LinkHead,
    /// Text or data.
    pub bytes: [u8; LINK_BYTES],
}

const _: () = assert!(size_of::<Link>() as u64 == PAGE_SIZE);

/// What one request carries, and what its answer is.
pub trait Message: IntoBytes + FromBytes + Immutable + KnownLayout + Copy {
    /// The word a [`Link`] carries for this request.
    const REQUEST: u64;
    /// The answer the monitor writes back.
    type Answer: IntoBytes + FromBytes + Immutable + KnownLayout + Copy;
}

impl Link {
    /// Writes `value` as the message or answer.
    pub fn put<T: IntoBytes + Immutable>(&mut self, value: &T) {
        const { assert!(size_of::<T>() <= size_of::<[u64; LINK_WORDS]>()) };
        let words = self.head.message.as_mut_bytes();
        words.fill(0);
        words[..size_of::<T>()].copy_from_slice(value.as_bytes());
    }

    /// Reads the message or answer as `T`.
    pub fn get<T: FromBytes + KnownLayout + Immutable>(&self) -> T {
        T::read_from_prefix(self.head.message.as_bytes())
            .expect("a message fits its link")
            .0
    }

    /// Writes `message` and `bytes` as a request.
    pub fn ask<M: Message>(&mut self, message: &M, bytes: &[u8]) {
        self.head.request = M::REQUEST;
        self.put(message);
        self.head.length = bytes.len() as u64;
        self.bytes[..bytes.len()].copy_from_slice(bytes);
    }

    /// The bytes the request carries.
    pub fn data(&self) -> &[u8] {
        &self.bytes[..self.head.length as usize]
    }
}

/// Defines a set of requests: an enum `Request` of them, whose words start at `first`, and for each one a
/// `#[repr(C)]` message type of `u64` fields with its answer's type. The floor's set is below; a program
/// on the floor may define its own after it, from [`Request::END`].
#[macro_export]
macro_rules! requests {
    (first: $first:expr; $(
        $(#[$doc:meta])*
        $name:ident { $($(#[$field_doc:meta])* $field:ident),* } -> $answer:ty;
    )*) => {
        /// What the inside asks.
        #[repr(u64)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Request { $($(#[$doc])* $name,)* }

        impl Request {
            /// Every request of the set.
            pub const ALL: &[Request] = &[$(Request::$name,)*];
            /// The first request's word.
            pub const FIRST: u64 = $first;
            /// The word after the last request's: another set may start here.
            pub const END: u64 = Self::FIRST + Self::ALL.len() as u64;

            /// The word a link carries for this request.
            pub const fn word(self) -> u64 {
                Self::FIRST + self as u64
            }

            /// The request a link's request word holds, if it is one of this set.
            pub fn from_word(word: u64) -> Option<Request> {
                Self::ALL.iter().copied().find(|r| r.word() == word)
            }
        }

        $(
            $(#[$doc])*
            #[repr(C)]
            #[derive(
                Clone,
                Copy,
                Debug,
                Default,
                zerocopy::FromBytes,
                zerocopy::IntoBytes,
                zerocopy::Immutable,
                zerocopy::KnownLayout,
            )]
            pub struct $name { $($(#[$field_doc])* pub $field: u64,)* }

            impl $crate::link::Message for $name {
                const REQUEST: u64 = Request::$name.word();
                type Answer = $answer;
            }
        )*
    };
}

requests! {
    first: 0;
    /// Text for the host's log, in `bytes`.
    Print {} -> ();
    /// The inside faulted itself.
    Fatal { esr, elr, far } -> ();
    /// Give back the host pages of a guest-physical range; they read as zeros after.
    GiveBack { at, length } -> ();
    /// Bytes of the next request's bytes, ahead of those it carries itself.
    Carry {} -> ();
    /// The next part of the last answer's bytes.
    Fetch {} -> ();
}

/// One link page and the way to have its request answered.
pub trait Ring {
    fn link(&mut self) -> &mut Link;
    /// Hands the link's request to the monitor and returns once it is answered.
    fn ring(&mut self);

    /// Asks `message` with `bytes`, carrying ahead what does not fit one link.
    fn ask<M: Message>(&mut self, message: &M, bytes: &[u8]) -> M::Answer {
        let ahead = bytes.len().saturating_sub(1) / LINK_BYTES * LINK_BYTES;
        for part in bytes[..ahead].chunks(LINK_BYTES) {
            self.link().ask(&Carry {}, part);
            self.ring();
        }
        self.link().ask(message, &bytes[ahead..]);
        self.ring();
        self.link().get()
    }

    /// Fills `into` from the last answer's bytes, fetching the parts after the first.
    fn take(&mut self, into: &mut [u8]) {
        let mut at = 0;
        loop {
            let data = self.link().data();
            let part = data.len().min(into.len() - at);
            into[at..at + part].copy_from_slice(&data[..part]);
            at += part;
            if at == into.len() || part == 0 {
                return;
            }
            self.link().ask(&Fetch {}, &[]);
            self.ring();
        }
    }
}

/// The monitor's half of bytes longer than one link: what was carried ahead of a request, and what an
/// answer still owes.
#[derive(Default)]
pub struct Parts {
    carried: Vec<u8>,
    owed: Vec<u8>,
}

impl Parts {
    /// The whole bytes of the request in `link`; none for a [`Carry`] or [`Fetch`], which this answers.
    pub fn request(&mut self, link: &mut Link) -> Option<Vec<u8>> {
        let mut bytes = core::mem::take(&mut self.carried);
        bytes.extend_from_slice(link.data());
        match Request::from_word(link.head.request) {
            Some(Request::Carry) => {
                self.carried = bytes;
                None
            }
            Some(Request::Fetch) => {
                let owed = core::mem::take(&mut self.owed);
                self.answer(link, owed);
                None
            }
            _ => Some(bytes),
        }
    }

    /// Writes an answer's bytes, owing what does not fit the link.
    pub fn answer(&mut self, link: &mut Link, mut bytes: Vec<u8>) {
        let part = bytes.len().min(LINK_BYTES);
        link.bytes[..part].copy_from_slice(&bytes[..part]);
        link.head.length = part as u64;
        self.owed = bytes.split_off(part);
    }
}
