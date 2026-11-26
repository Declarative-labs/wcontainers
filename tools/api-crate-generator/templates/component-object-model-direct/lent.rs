//! Memory lent across a call pair such as Map and Unmap, in both directions.
//!
//! `ForeignLent` is the system's memory, which the calling glue receives from Map: it holds the objects the memory
//! depends on, so it cannot die while any clone lives, and the calling glue revokes it before the system's Unmap. A
//! revoke waits only for a copy under way, never for code of either side: `read_to` copies the bytes out before the
//! sink runs, and `write_from` lets the source fill a buffer of ours before the bytes are copied in. (The spike's
//! form held its lock while the sink or source ran, so a sink that called Unmap hung.)
//!
//! `Shadow` is memory of ours the serving glue lends the C side from Map to Unmap, from a pool Rust's allocator never
//! gets back: the C side writes it at any moment, also after its Unmap, and only ever through its address. Blocks
//! come in size classes, powers of two, so that a pool reused by lengths a program chooses stays bounded.

use std::sync::{Arc, Mutex, PoisonError, RwLock};

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};

use crate::key::GlueKey;

/// The smallest block of the pool.
const SMALLEST: usize = 64;

/// The system's lent memory, shared by its clones; reached only through `VolatileBytes`, and empty once revoked.
#[derive(Clone)]
pub struct ForeignLent(Arc<ForeignBlock>);

struct ForeignBlock {
    view: RwLock<Option<PointerBytes>>,
    length: usize,
    /// The objects the memory belongs to, released when the last clone drops, under no lock.
    #[expect(dead_code, reason = "held only to keep the objects alive")]
    keep: Vec<Box<dyn Send + Sync>>,
}

impl ForeignLent {
    /// Returns a view of memory the system lends until its Unmap, kept alive by `keep`; the calling glue runs
    /// `revoke` before that Unmap.
    ///
    /// # Safety
    /// As `PointerAndLength::new`, for `length` bytes at `address`, for as long as the objects in `keep` live and
    /// `revoke` has not been called.
    #[expect(unsafe_code, reason = "memory the system lends us")]
    pub unsafe fn new(address: usize, length: usize, keep: Vec<Box<dyn Send + Sync>>) -> ForeignLent {
        #[deny(unsafe_code)]
        let body = move || {
            let pointer = core::ptr::with_exposed_provenance_mut::<u8>(address);
            #[expect(unsafe_code, reason = "memory the system lends us")]
            // SAFETY: the caller's promise; `keep` goes into the block, so the objects live as long as any clone.
            let config = unsafe { PointerAndLength::new(pointer, length) };
            ForeignLent(Arc::new(ForeignBlock { view: RwLock::new(Some(PointerBytes::new(config))), length, keep }))
        };
        body()
    }

    /// Returns the length lent, also after a revoke.
    pub fn length(&self) -> usize {
        self.0.length
    }

    /// Empties the view of every clone, waiting only for a copy under way.
    pub fn revoke(&self, _key: GlueKey) {
        *self.0.view.write().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// Runs `body` on the view under the lock: only copies by memcpy run there.
    fn with_view<R>(&self, empty: R, body: impl FnOnce(&PointerBytes) -> R) -> R {
        self.0.view.read().unwrap_or_else(PoisonError::into_inner).as_ref().map_or(empty, body)
    }
}

/// Returns a buffer of `length` zero bytes, or none when the allocator cannot give it.
fn buffer(length: usize) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).ok()?;
    bytes.resize(length, 0_u8);
    Some(bytes)
}

impl VolatileBytes for ForeignLent {
    fn len(&self) -> u64 {
        self.with_view(0, VolatileBytes::len)
    }

    fn read(&self, into: &mut [u8]) -> usize {
        self.with_view(0, |view| view.read(into))
    }

    fn write(&self, bytes: &[u8]) -> usize {
        self.with_view(0, |view| view.write(bytes))
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        // The bytes are copied out under the lock and the sink runs after it: a sink may revoke.
        let copied = self.with_view(None, |view| {
            let mut bytes = buffer(usize::try_from(view.len()).ok()?)?;
            let read = view.read(&mut bytes);
            bytes.truncate(read);
            Some(bytes)
        });
        match copied {
            Some(bytes) => sink.write(&bytes),
            None => Ok(0),
        }
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        // The source fills a buffer of ours under no lock; the bytes are copied in after it, unless revoked meanwhile.
        let Some(mut bytes) = buffer(self.0.length) else { return Ok(0) };
        let filled = source.read(&mut bytes)?.min(bytes.len());
        Ok(self.with_view(0, |view| view.write(&bytes[..filled])))
    }
}

/// Memory of ours lent to the C side from Map to Unmap.
pub struct Shadow(Option<PoolBlock>, usize);

/// A block of ours: its whole view, address and size class.
struct PoolBlock {
    view: PointerBytes,
    address: usize,
    class: usize,
}

/// Blocks of ours that no shadow holds now.
static POOL: Mutex<Vec<PoolBlock>> = Mutex::new(Vec::new());

impl Shadow {
    /// Returns memory of this many bytes, from the pool or new; what it holds at first is unspecified. None when the
    /// allocator cannot give it.
    pub fn allocate(length: usize) -> Option<Shadow> {
        let class = length.max(SMALLEST).checked_next_power_of_two()?;
        let reused = {
            let mut pool = POOL.lock().unwrap_or_else(PoisonError::into_inner);
            pool.iter().position(|block| block.class == class).map(|at| pool.swap_remove(at))
        };
        let block = match reused {
            Some(block) => block,
            None => {
                let bytes = buffer(class)?;
                let block = Box::into_raw(bytes.into_boxed_slice()).cast::<u8>();
                #[expect(unsafe_code, reason = "a block of ours whose Box was given up, never to be freed")]
                // SAFETY: `Box::into_raw` gave up the block, so no reference covers it; the pool keeps it for life.
                let config = unsafe { PointerAndLength::new(block, class) };
                PoolBlock { view: PointerBytes::new(config), address: block.expose_provenance(), class }
            }
        };
        Some(Shadow(Some(block), length))
    }

    /// Returns the address of the first byte, its provenance exposed for C to use.
    pub fn address(&self) -> usize {
        self.0.as_ref().map_or(0, |block| block.address)
    }

    /// Returns the length lent.
    pub fn length(&self) -> usize {
        self.1
    }

    /// Copies the bytes in from `bytes`, as many as both hold, and returns how many.
    pub fn fill(&self, bytes: &[u8]) -> usize {
        self.0.as_ref().map_or(0, |block| block.view.part(0..self.1 as u64).map_or(0, |view| view.write(bytes)))
    }

    /// Returns a copy of the bytes the C side left, or none when the allocator cannot give it.
    pub fn contents(&self) -> Option<Vec<u8>> {
        let mut bytes = buffer(self.1)?;
        let read = self.0.as_ref().map_or(0, |block| block.view.read(&mut bytes));
        bytes.truncate(read);
        Some(bytes)
    }
}

impl Drop for Shadow {
    fn drop(&mut self) {
        if let Some(block) = self.0.take() {
            POOL.lock().unwrap_or_else(PoisonError::into_inner).push(block);
        }
    }
}

#[cfg(test)]
#[path = "lent_tests.rs"]
mod tests;
