//! A sink or source that calls Unmap (the calling glue's revoke) from inside `read_to` or `write_from` must not hang:
//! the spike's form held its lock while the sink ran. Each case runs on its own thread under a 10 s limit.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::VolatileBytes;

use super::{ForeignLent, Shadow};
use crate::key::GlueKey;

const LIMIT: Duration = Duration::from_secs(10);
const LENGTH: usize = 256;
const FILL: u8 = 0x5a;

/// Lent memory over a block of ours given up for the test's life.
fn lent() -> ForeignLent {
    let block = Box::into_raw(vec![FILL; LENGTH].into_boxed_slice()).cast::<u8>();
    #[expect(unsafe_code, reason = "the test's own block, standing for memory the system lends")]
    // SAFETY: `Box::into_raw` gave up the block, which is never freed, so it is readable and writable for life.
    unsafe {
        ForeignLent::new(block.expose_provenance(), LENGTH, Vec::new())
    }
}

/// A sink that revokes the memory it is being given, as a sink calling the calling glue's Unmap does.
struct Revoking(ForeignLent, usize);

impl ByteSink for Revoking {
    type Error = ();

    fn write(&mut self, bytes: &[u8]) -> Result<usize, ()> {
        self.0.revoke(GlueKey::OWN);
        self.1 = bytes.len();
        Ok(bytes.len())
    }
}

impl ByteSource for Revoking {
    type Error = ();

    fn read(&mut self, into: &mut [u8]) -> Result<usize, ()> {
        self.0.revoke(GlueKey::OWN);
        into.fill(FILL);
        self.1 = into.len();
        Ok(into.len())
    }
}

/// Runs `body` on its own thread and returns its answer, or none when it has not ended within the limit.
fn within_limit<R: Send + 'static>(body: impl FnOnce() -> R + Send + 'static) -> Option<R> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(body());
    });
    receiver.recv_timeout(LIMIT).ok()
}

#[test]
fn a_sink_that_unmaps_inside_read_to_ends() {
    let answer = within_limit(|| {
        let memory = lent();
        let mut sink = Revoking(memory.clone(), 0);
        let read = memory.read_to(&mut sink);
        (read, sink.1, memory.len())
    });
    assert_eq!(answer, Some((Ok(LENGTH), LENGTH, 0)), "read_to with a sink that unmaps did not end within 10 s");
}

#[test]
fn a_source_that_unmaps_inside_write_from_ends() {
    let answer = within_limit(|| {
        let memory = lent();
        let mut source = Revoking(memory.clone(), 0);
        let written = memory.write_from(&mut source);
        (written, source.1, memory.len())
    });
    // Revoked before the copy in: nothing reaches the memory, and the call ends.
    assert_eq!(answer, Some((Ok(0), LENGTH, 0)), "write_from with a source that unmaps did not end within 10 s");
}

#[test]
fn revoked_memory_reads_and_writes_nothing() {
    let memory = lent();
    let mut into = [0_u8; LENGTH];
    assert_eq!(memory.read(&mut into), LENGTH);
    assert!(into.iter().all(|&byte| byte == FILL));
    memory.revoke(GlueKey::OWN);
    assert_eq!(memory.read(&mut into), 0);
    assert_eq!(memory.write(&into), 0);
    assert_eq!(memory.length(), LENGTH);
}

#[test]
fn a_shadow_keeps_what_was_filled_and_goes_back_to_the_pool_only_at_unmap() {
    let shadow = Shadow::allocate(LENGTH).expect("a small block");
    assert_eq!(shadow.fill(&[FILL; LENGTH]), LENGTH);
    assert_eq!(shadow.contents(), Some(vec![FILL; LENGTH]));
    let address = shadow.address();
    shadow.give_back();
    let again = Shadow::allocate(LENGTH).expect("a small block");
    assert_eq!(again.address(), address, "a block given back at Unmap comes back from the pool");
    drop(again);
    let other = Shadow::allocate(LENGTH).expect("a small block");
    assert_ne!(other.address(), address, "a block dropped while lent is never lent again");
    assert!(Shadow::allocate(usize::MAX).is_none());
}
