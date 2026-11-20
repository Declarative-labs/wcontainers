//! What the kernel asks of the host for everything that is not running instructions.
//!
//! Nothing here waits. A request whose answer takes time returns at once and the answer arrives later as an
//! [`Event`], which the driver hands to the kernel.

/// One connection to the world outside the container.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(pub u32);

/// One request sent to the file backend, chosen by the kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BackendRequestId(pub u64);

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
    /// The backend answered a request. The bytes are one FUSE reply.
    BackendReply {
        request: BackendRequestId,
        reply: &'a [u8],
    },
    /// The backend sent a FUSE notice that was not asked for, such as "this file changed".
    BackendNotice { notice: &'a [u8] },
    /// A connection finished connecting, or failed with a Linux error number (such as ECONNREFUSED,
    /// ETIMEDOUT or ENETUNREACH).
    Connected {
        connection: ConnectionId,
        error: i32,
    },
    /// A connection has bytes to receive, room to send, or has closed: try again. Sent once after
    /// a [`Host::receive`] or [`Host::send`] answered [`HostError::WouldBlock`], or after
    /// connecting, when that changes.
    ConnectionReady { connection: ConnectionId },
    /// A name lookup finished. `addresses` holds 16 byte addresses one after another, IPv4 ones
    /// as IPv4-mapped IPv6 addresses (`::ffff:a.b.c.d`). `error` is 0 when the name was found (with any addresses, perhaps none), ENOENT when no
    /// such name exists, and another Linux error number when the lookup failed.
    LookedUp {
        request: u64,
        addresses: &'a [u8],
        error: i32,
    },
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
pub trait Host {
    /// Nanoseconds on a clock that never goes backwards.
    fn monotonic_nanoseconds(&self) -> u64;
    /// Nanoseconds since 1970 by the wall clock.
    fn realtime_nanoseconds(&self) -> u64;
    /// Fill with random bytes fit for cryptography.
    fn random(&self, into: &mut [u8]);

    fn warn(&self, warning: Warning<'_>);

    /// Send one FUSE request to the file backend. The reply arrives as [`Event::BackendReply`], unless the
    /// request is one FUSE defines as having no reply.
    fn backend_send(&self, request: BackendRequestId, message: &[u8]) -> Result<(), HostError>;

    /// Bytes out of an end, never more than the credit the embedder gave on it.
    fn send_stream(&self, end: End, bytes: &[u8]);
    /// The embedder may send this many more bytes on an end (added to what it may send already).
    fn credit_stream(&self, end: End, bytes: u64);
    /// The kernel sends nothing more on an end, data or credit; the embedder frees it by closing it.
    fn close_stream(&self, end: End);

    /// Start connecting to an outside address (4 or 16 bytes) and port. The result arrives as
    /// [`Event::Connected`]. `stream` is true for TCP and false for UDP. A UDP connection may be
    /// sent on and received from at once; each send is one datagram and each receive gives one
    /// (what does not fit is lost).
    fn connect(
        &self,
        connection: ConnectionId,
        address: &[u8],
        port: u16,
        stream: bool,
    ) -> Result<(), HostError>;
    /// Send bytes; returns how many were taken.
    fn send(&self, connection: ConnectionId, bytes: &[u8]) -> Result<usize, HostError>;
    /// Receive bytes; returns how many arrived. Zero means the other end finished sending;
    /// [`HostError::Closed`] means the connection was reset.
    fn receive(&self, connection: ConnectionId, into: &mut [u8]) -> Result<usize, HostError>;
    /// No more bytes will be sent on a TCP connection; the other end reads its end. Receiving
    /// goes on.
    fn finish(&self, connection: ConnectionId);
    fn close(&self, connection: ConnectionId);
    /// Start looking up a name. The result arrives as [`Event::LookedUp`] with the same request number.
    fn look_up(&self, request: u64, name: &str) -> Result<(), HostError>;
    /// Start accepting TCP connections from outside to the container's port `port`. While some
    /// wait, [`Event::ConnectionReady`] comes on `listener` (once after this call, and again after
    /// [`Host::accept`] answered [`HostError::WouldBlock`]); each is taken with [`Host::accept`].
    /// [`Host::close`] on `listener` stops it.
    fn listen(&self, listener: ConnectionId, port: u16) -> Result<(), HostError>;
    /// Take one waiting connection from a listener and give it the number `connection`, used then
    /// as one made by [`Host::connect`]. [`HostError::WouldBlock`] when none waits.
    fn accept(&self, listener: ConnectionId, connection: ConnectionId) -> Result<(), HostError>;
}
