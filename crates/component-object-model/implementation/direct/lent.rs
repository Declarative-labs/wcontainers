//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
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
//! come in size classes, powers of two, so that a pool reused by lengths a program chooses stays bounded. A block goes
//! back to the pool only through an Unmap (`give_back`); one whose object died while mapped is never lent again, so
//! that a program writing it after the release cannot reach a later mapping.

use std::sync::{Arc, Mutex, PoisonError, RwLock};

use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};

use crate::key::GlueKey;

/// The smallest block of the pool.
#[cfg(not(feature = "exact-shadow"))]
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
    pub unsafe fn new(
        address: usize,
        length: usize,
        keep: Vec<Box<dyn Send + Sync>>,
    ) -> ForeignLent {
        #[deny(unsafe_code)]
        let body = move || {
            let pointer = core::ptr::with_exposed_provenance_mut::<u8>(address);
            #[expect(unsafe_code, reason = "memory the system lends us")]
            // SAFETY: the caller's promise; `keep` goes into the block, so the objects live as long as any clone.
            let config = unsafe { PointerAndLength::new(pointer, length) };
            ForeignLent(Arc::new(ForeignBlock {
                view: RwLock::new(Some(PointerBytes::new(config))),
                length,
                keep,
            }))
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
        self.0
            .view
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map_or(empty, body)
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

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        self.with_view(0, |view| view.read_at(offset, into))
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        self.with_view(0, |view| view.write_at(offset, bytes))
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
        let Some(mut bytes) = buffer(self.0.length) else {
            return Ok(0);
        };
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

/// Blocks whose object died while they were lent, oldest first: lent again only once this many bytes of later ones
/// wait behind them, so that a program writing one after the release reaches no recent mapping, and the memory held
/// stays bounded.
static ABANDONED: Mutex<std::collections::VecDeque<PoolBlock>> =
    Mutex::new(std::collections::VecDeque::new());

/// The bytes of abandoned blocks kept back from the pool.
const ABANDONED_BYTES: usize = 64 << 20;

/// How many blocks a thread keeps for itself before giving them to the pool, so that a Map and its Unmap on one thread
/// take no lock for their block.
const SPARE_BLOCKS: usize = 4;

/// A thread's own spare blocks, given to the pool when the thread ends.
struct Spare(Vec<PoolBlock>);

impl Drop for Spare {
    fn drop(&mut self) {
        POOL.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .append(&mut self.0);
    }
}

thread_local! {
    static SPARE: core::cell::RefCell<Spare> = const { core::cell::RefCell::new(Spare(Vec::new())) };
}

impl Shadow {
    /// Returns memory of this many bytes, from the pool or new; what it holds at first is unspecified. None when the
    /// allocator cannot give it.
    pub fn allocate(length: usize) -> Option<Shadow> {
        #[cfg(not(feature = "exact-shadow"))]
        let class = length.max(SMALLEST).checked_next_power_of_two()?;
        // Exactly the length lent (at least one byte), so that a touch past it lands outside the block.
        #[cfg(feature = "exact-shadow")]
        let class = length.max(1);
        let spare = SPARE
            .try_with(|spare| {
                let mut spare = spare.try_borrow_mut().ok()?;
                let at = spare.0.iter().position(|block| block.class == class)?;
                Some(spare.0.swap_remove(at))
            })
            .ok()
            .flatten();
        let reused = spare.or_else(|| {
            let mut pool = POOL.lock().unwrap_or_else(PoisonError::into_inner);
            pool.iter()
                .position(|block| block.class == class)
                .map(|at| pool.swap_remove(at))
        });
        let block = match reused {
            Some(block) => block,
            None => {
                let bytes = buffer(class)?;
                let block = Box::into_raw(bytes.into_boxed_slice()).cast::<u8>();
                #[expect(
                    unsafe_code,
                    reason = "a block of ours whose Box was given up, never to be freed"
                )]
                // SAFETY: `Box::into_raw` gave up the block, so no reference covers it; the pool keeps it for life.
                let config = unsafe { PointerAndLength::new(block, class) };
                PoolBlock {
                    view: PointerBytes::new(config),
                    address: block.expose_provenance(),
                    class,
                }
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

    /// Copies the bytes in from `source`'s `read_to`, with no copy of ours between, as many as both hold.
    pub fn fill_from(&self, source: &impl VolatileBytes) {
        let Some(block) = self.0.as_ref() else { return };
        let mut writer = Filling {
            view: &block.view,
            at: 0,
            end: self.1 as u64,
        };
        let _ = source.read_to(&mut writer);
    }

    /// Copies the bytes the C side left out to `target`'s `write_from`, with no copy of ours between.
    pub fn empty_into(&self, target: &impl VolatileBytes) {
        let Some(block) = self.0.as_ref() else { return };
        let mut reader = Emptying {
            view: &block.view,
            at: 0,
            end: self.1 as u64,
        };
        let _ = target.write_from(&mut reader);
    }

    /// Writes zeros over the bytes lent, so that memory whose contents the API leaves undefined (a discarding Map)
    /// shows nothing of an earlier lending, with no read of the implementation's bytes.
    pub fn clear(&self) {
        const ZEROS: [u8; 4096] = [0; 4096];
        let Some(block) = self.0.as_ref() else { return };
        let mut at = 0_u64;
        let end = self.1 as u64;
        while at < end {
            let written = block
                .view
                .write_at(at, &ZEROS[..within(ZEROS.len(), at, end)]);
            if written == 0 {
                return;
            }
            at += written as u64;
        }
    }

    /// Copies the bytes in from `bytes`, as many as both hold, and returns how many.
    pub fn fill(&self, bytes: &[u8]) -> usize {
        self.0.as_ref().map_or(0, |block| {
            block
                .view
                .write(&bytes[..within(bytes.len(), 0, self.1 as u64)])
        })
    }

    /// Returns a copy of the bytes the C side left, or none when the allocator cannot give it.
    pub fn contents(&self) -> Option<Vec<u8>> {
        let mut bytes = buffer(self.1)?;
        let read = self
            .0
            .as_ref()
            .map_or(0, |block| block.view.read(&mut bytes));
        bytes.truncate(read);
        Some(bytes)
    }
}

impl Shadow {
    /// Gives the block back to the pool, at the Unmap that ends the lending.
    pub fn give_back(mut self) {
        let mut held = self.0.take();
        let _ = SPARE.try_with(|spare| {
            if let Ok(mut spare) = spare.try_borrow_mut()
                && spare.0.len() < SPARE_BLOCKS
                && let Some(block) = held.take()
            {
                spare.0.push(block);
            }
        });
        if let Some(block) = held {
            POOL.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(block);
        }
    }
}

impl Drop for Shadow {
    fn drop(&mut self) {
        let Some(block) = self.0.take() else { return };
        let released: Vec<PoolBlock> = {
            let mut abandoned = ABANDONED.lock().unwrap_or_else(PoisonError::into_inner);
            abandoned.push_back(block);
            let mut held: usize = abandoned.iter().map(|block| block.class).sum();
            let mut released = Vec::new();
            while held > ABANDONED_BYTES && abandoned.len() > 1 {
                let Some(oldest) = abandoned.pop_front() else {
                    break;
                };
                held -= oldest.class;
                released.push(oldest);
            }
            released
        };
        POOL.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend(released);
    }
}

#[cfg(test)]
#[path = "lent_tests.rs"]
mod tests;

/// How many of a buffer's `count` bytes fit from `at` to `end`.
fn within(count: usize, at: u64, end: u64) -> usize {
    usize::try_from(end.saturating_sub(at)).map_or(count, |room| count.min(room))
}

/// A sink writing a shadow's bytes in order, as many as its first `end` bytes hold; past them it takes nothing.
struct Filling<'block> {
    view: &'block PointerBytes,
    at: u64,
    end: u64,
}

impl ByteSink for Filling<'_> {
    type Error = core::convert::Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        let written = self
            .view
            .write_at(self.at, &bytes[..within(bytes.len(), self.at, self.end)]);
        self.at += written as u64;
        Ok(written)
    }
}

/// A source reading a shadow's first `end` bytes in order; past them it gives nothing.
struct Emptying<'block> {
    view: &'block PointerBytes,
    at: u64,
    end: u64,
}

impl ByteSource for Emptying<'_> {
    type Error = core::convert::Infallible;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, Self::Error> {
        let count = within(into.len(), self.at, self.end);
        let read = self.view.read_at(self.at, &mut into[..count]);
        self.at += read as u64;
        Ok(read)
    }
}
