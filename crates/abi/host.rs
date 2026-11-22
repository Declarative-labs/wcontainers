//! What the kernel asks of the host for everything that is not running instructions.
//!
//! Nothing here waits. A request whose answer takes time returns at once and the answer arrives later as an
//! [`Event`], which the driver hands to the kernel.

/// The host's clocks count nanoseconds: how many make a second.
pub const NANOSECONDS_PER_SECOND: u64 = core::time::Duration::from_secs(1).as_nanos() as u64;

/// One UDP flow to the world outside the container.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(pub u32);

/// One end of a byte stream between the kernel and the embedder: a handle local to one kernel,
/// whose other side the embedder holds. Each direction carries data, credit and closing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct End(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostError {
    /// This host cannot do this at all.
    NotSupported,
    /// Nothing to give or no room to take right now. An [`Event`] will say when to try again.
    WouldBlock,
    /// The other end is gone.
    Closed,
    /// Any other failure, as a Linux error number.
    Errno(i32),
}

/// One slow path that was taken, reported so that it is never silent.
#[derive(Clone, Copy, Debug)]
pub struct Warning<'a> {
    /// The fixed name of the reason, such as "system call not implemented".
    pub reason: &'a str,
    /// How many times it has happened so far.
    pub count: u64,
    /// The guest address or the number concerned, if there is one.
    pub detail: u64,
}

/// The state of one named counter.
#[derive(Clone, Copy, Debug)]
pub struct CounterReading {
    pub reason: &'static str,
    /// The guest address or the number concerned, as in [`Warning::detail`]; one counter per value.
    pub detail: u64,
    pub count: u64,
    pub nanoseconds: u64,
}

/// Something that happened outside, handed to the kernel by the driver. If a guest thread is running when
/// one arrives, the driver raises that thread's stop flag first and delivers the event once it comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event<'a> {
    /// The time given in [`crate::Next::Idle`] has come.
    Timer,
    /// The backend sent a FUSE notice that was not asked for, such as "this file changed".
    BackendNotice { notice: &'a [u8] },
    /// A UDP flow has a datagram to receive or room to send: try again. Sent once after a
    /// [`Host::receive`] or [`Host::send`] answered [`HostError::WouldBlock`], or after
    /// connecting, when that changes.
    ConnectionReady { connection: ConnectionId },
    /// Bytes for an end, never more than the credit the kernel gave on it.
    StreamData { end: End, bytes: &'a [u8] },
    /// The kernel may send this many more bytes on an end.
    StreamCredit { end: End, bytes: u64 },
    /// The embedder has let go of an end: it sends and takes nothing more on it.
    StreamClosed { end: End },
    /// A terminal end's terminal changed size.
    TerminalResized { end: End, columns: u16, rows: u16 },
}

/// What the kernel asks of the host. First bodies may be naive or answer [`HostError::NotSupported`]; the
/// interface is not optional.
pub trait Host: Sync {
    /// Nanoseconds on a clock that never goes backwards.
    fn monotonic_nanoseconds(&self) -> u64;
    /// Nanoseconds since 1970 by the wall clock.
    fn realtime_nanoseconds(&self) -> u64;
    /// Fill with random bytes fit for cryptography.
    fn random(&self, into: &mut [u8]);

    fn warn(&self, warning: Warning<'_>);

    /// Bytes out of an end, never more than the credit the embedder gave on it.
    fn send_stream(&self, end: End, bytes: &[u8]);
    /// The embedder may send this many more bytes on an end (added to what it may send already).
    fn credit_stream(&self, end: End, bytes: u64);
    /// The kernel sends nothing more on an end, data or credit; the embedder frees it by closing it.
    fn close_stream(&self, end: End);

    /// Starts a UDP flow to an outside address (4 or 16 bytes) and port. It may be sent on and
    /// received from at once; each send is one datagram and each receive gives one (what does not
    /// fit is lost).
    fn connect(&self, connection: ConnectionId, address: &[u8], port: u16)
    -> Result<(), HostError>;
    /// Send one datagram; returns how many bytes were taken.
    fn send(&self, connection: ConnectionId, bytes: &[u8]) -> Result<usize, HostError>;
    /// Receive one datagram; returns how many bytes arrived. [`HostError::Closed`] means the
    /// destination refused.
    fn receive(&self, connection: ConnectionId, into: &mut [u8]) -> Result<usize, HostError>;
    fn close(&self, connection: ConnectionId);
}
