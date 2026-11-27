//! What [wasmtime's embedding API](https://docs.wasmtime.dev/api/wasmtime/) leaves unsafe: linear memories whose pages
//! the embedder maps itself, from [memory files](https://man7.org/linux/man-pages/man2/memfd_create.2.html), and the
//! trap handler of a call in progress, which the host functions it calls cannot hold since a store's data is `'static`.
#![cfg(target_os = "linux")]

use std::ops::Range;
use std::os::fd::BorrowedFd;

pub use rustix::io::Errno;
use wasmtime::{AsContextMut, Config, Memory, MemoryType};
use wcontainers_executor::TrapHandler;
use wcontainers_volatile_bytes::VolatileBytes;

/// One method for each use of wasmtime that needs `unsafe`, named as wasmtime names what it stands for.
#[allow(
    reviewable::explicit_shared_error::Errno,
    reason = "every mapping fails the same way, with the kernel's error number"
)]
pub trait Wasmtime {
    /// Host addresses reserved for one linear memory and a guard after it, every byte of the memory mapped from a file.
    type HostMemory: Clone + Send + Sync;
    /// Bytes of a host memory, which a guest may change while they are held; they keep the memory mapped.
    type Bytes: VolatileBytes + Send + Sync;

    /// Makes the engines of the configuration take every linear memory from [`Wasmtime::new_memory`], as
    /// [`Config::with_host_memory`] does.
    fn with_host_memory(&self, config: &mut Config);

    /// Reserves `length` bytes and an inaccessible guard of `guard` bytes after them, and maps the file's first `length`
    /// bytes over the memory, shared; `EINVAL` for a length or guard that is not whole host pages, or a file that may
    /// shrink or is shorter.
    fn new_host_memory(
        &self,
        file: BorrowedFd<'_>,
        length: usize,
        guard: usize,
    ) -> Result<Self::HostMemory, Errno>;

    /// Maps the file's bytes from the offset over the memory's addresses, shared, in place of what they showed; `EINVAL`
    /// for addresses outside the memory or not whole host pages, or a file that may shrink or is shorter.
    fn map_file(
        &self,
        memory: &Self::HostMemory,
        addresses: Range<u64>,
        file: BorrowedFd<'_>,
        offset: u64,
    ) -> Result<(), Errno>;

    /// Returns the memory's bytes at the offsets; none for offsets past its length.
    fn host_memory_bytes(
        &self,
        memory: &Self::HostMemory,
        offsets: Range<u64>,
    ) -> Option<Self::Bytes>;

    /// Returns a memory of the store, of the type, over the host memory; an error for a type larger than the host memory
    /// or an engine whose configuration did not take [`Wasmtime::with_host_memory`] of this value.
    fn new_memory(
        &self,
        store: impl AsContextMut,
        ty: MemoryType,
        memory: &Self::HostMemory,
    ) -> wasmtime::Result<Memory>;

    /// Calls `call` with the handler lent to [`Wasmtime::with_trap_handler`] on this thread until `call` returns.
    fn call_with_trap_handler<Returned>(
        &self,
        handler: &mut dyn TrapHandler,
        call: impl FnOnce() -> Returned,
    ) -> Returned;

    /// Lends the handler of the call in progress on this thread to `answer`; none outside such a call or inside
    /// `answer` itself.
    fn with_trap_handler<Returned>(
        &self,
        answer: impl FnOnce(&mut dyn TrapHandler) -> Returned,
    ) -> Option<Returned>;
}
