//! A host call that takes a slice, made once on all of some volatile bytes, which decide how the call reaches them.

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::VolatileBytes;

/// A host call that takes the bytes to send, and whether the bytes have reached it.
struct Sending<Call> {
    call: Call,
    made: bool,
}

impl<Call, Error> ByteSink for Sending<Call>
where
    Call: FnMut(&[u8]) -> Result<usize, Error>,
{
    type Error = Error;

    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        self.made = true;
        (self.call)(bytes)
    }
}

/// A host call that fills the bytes it receives into, and whether the bytes have reached it.
struct Receiving<Call> {
    call: Call,
    made: bool,
}

impl<Call, Error> ByteSource for Receiving<Call>
where
    Call: FnMut(&mut [u8]) -> Result<usize, Error>,
{
    type Error = Error;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, Error> {
        self.made = true;
        (self.call)(into)
    }
}

/// Makes the call on all of the payload, or on no bytes where the payload never reaches it, and returns the
/// number of bytes the call took.
pub(crate) fn send_from<Error>(
    payload: &impl VolatileBytes,
    call: impl FnMut(&[u8]) -> Result<usize, Error>,
) -> Result<usize, Error> {
    let mut sending = Sending { call, made: false };
    let count = payload.read_to(&mut sending)?;
    match sending.made {
        true => Ok(count),
        false => (sending.call)(&[]),
    }
}

/// Makes the call into all of the bytes, or into no bytes where the bytes never reach it, and returns the
/// number of bytes the call gave.
pub(crate) fn receive_into<Error>(
    into: &impl VolatileBytes,
    call: impl FnMut(&mut [u8]) -> Result<usize, Error>,
) -> Result<usize, Error> {
    let mut receiving = Receiving { call, made: false };
    let count = into.write_from(&mut receiving)?;
    match receiving.made {
        true => Ok(count),
        false => (receiving.call)(&mut []),
    }
}
