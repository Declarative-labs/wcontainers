//! The wasmtime embedding calls that need `unsafe`, on this host: each use of `unsafe` is one marked line.
#![cfg(target_os = "linux")]

use core::ffi::c_void;
use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::os::fd::BorrowedFd;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use rustix::fs::SealFlags;
use rustix::mm::{MapFlags, MprotectFlags, ProtFlags};
use wasm_encoder::{ExportKind, ExportSection, MemorySection};
use wasmtime::{
    AsContextMut, Config, Engine, Instance, LinearMemory, Memory, MemoryCreator, MemoryType, Module,
};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_executor::TrapHandler;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};
use wcontainers_wasmtime::{Errno, Wasmtime};

/// This process's wasmtime: host memories from its own mappings, and the trap handler kept for this thread.
#[derive(Clone, Default)]
pub struct DirectWasmtime {
    creator: Arc<HostMemories>,
}

/// The memory creator of the engines configured with it: the host memory `new_memory` names, or one of its own.
///
/// It hands wasmtime the host memory `new_memory` names on the same thread, and makes every other memory the engine
/// asks for, such as a heap of its garbage collector, itself.
#[derive(Default)]
struct HostMemories {
    /// The modules that define and export one memory, by engine and type, compiled once each.
    modules: Mutex<Vec<(Engine, MemoryType, Module)>>,
}

/// The name the module of one memory exports it under.
const MEMORY_EXPORT: &str = "memory";
/// The log2 of WebAssembly's default page size.
///
/// <https://webassembly.github.io/spec/core/exec/runtime.html#memory-instances>
const DEFAULT_PAGE_SIZE_LOG2: u8 = 16;
/// The module of one memory's one memory, the first.
const MEMORY_INDEX: u32 = u32::MIN;
/// Why a store's memory was not made: the engine takes its memories from another value.
const NOT_CONFIGURED: &str = "the engine was not configured with this value's host memories";
/// Why a host memory did not grow: past its length.
const GROWN_PAST_LENGTH: &str = "a host memory grows only inside its length";
/// Why a memory of its own did not grow: past its reservation.
const GROWN_PAST_RESERVATION: &str = "a memory grows only inside its reservation";

/// A host memory: its reservation, shared by every store's memory over it and every view of its bytes.
#[derive(Clone)]
pub struct DirectHostMemory(Arc<Reservation>);

/// Addresses this process reserved: `length` bytes, then `guard` bytes that allow nothing.
///
/// A host memory's length is all mapped from files; another memory's is readable and writable as far as it has grown.
struct Reservation {
    /// An `AtomicPtr` only so that the type is `Send` and `Sync` without an `unsafe impl`.
    base: AtomicPtr<u8>,
    length: usize,
    guard: usize,
}

/// Bytes of a host memory, which keep its reservation mapped.
pub struct DirectBytes {
    bytes: PointerBytes,
    /// Held, never read: while it lives the reservation stays mapped.
    _keeps: Arc<Reservation>,
}

/// A store's memory over a host memory.
struct StoreMemory {
    reservation: Arc<Reservation>,
    size: usize,
}

/// A memory the engine asked for that is no host memory: anonymous pages opened as it grows.
struct OwnMemory {
    reservation: Reservation,
    size: usize,
}

thread_local! {
    /// The handler of the call in progress on this thread, out of the slot while it is lent.
    static TRAP_HANDLER: Cell<Option<NonNull<dyn TrapHandler + 'static>>> = const { Cell::new(None) };
    /// The host memory `new_memory` is making a store's memory over on this thread, and the type it asked for.
    static NEXT_HOST_MEMORY: RefCell<Option<(Arc<Reservation>, MemoryType)>> = const { RefCell::new(None) };
}

/// Puts the handler that was in the slot back when dropped, on a panic too.
struct RestoreHandler(Option<NonNull<dyn TrapHandler + 'static>>);

impl Drop for RestoreHandler {
    fn drop(&mut self) {
        TRAP_HANDLER.with(|slot| slot.set(self.0));
    }
}

/// Empties the slot of the next host memory when dropped, on an error too.
struct ClearNext;

impl Drop for ClearNext {
    fn drop(&mut self) {
        NEXT_HOST_MEMORY.with(|next| next.borrow_mut().take());
    }
}

impl Wasmtime for DirectWasmtime {
    type HostMemory = DirectHostMemory;
    type Bytes = DirectBytes;

    fn with_host_memory(&self, config: &mut Config) {
        config.with_host_memory(self.creator.clone());
    }

    fn new_host_memory(
        &self,
        file: BorrowedFd<'_>,
        length: usize,
        guard: usize,
    ) -> Result<DirectHostMemory, Errno> {
        let page = rustix::param::page_size();
        if length == 0 || !length.is_multiple_of(page) || !guard.is_multiple_of(page) {
            return Err(Errno::INVAL);
        }
        check_file(file, u64::MIN, length)?;
        let reservation = Reservation::new(length, guard)?;
        reservation.map_over(usize::MIN, length, file, u64::MIN)?;
        Ok(DirectHostMemory(Arc::new(reservation)))
    }

    fn map_file(
        &self,
        memory: &DirectHostMemory,
        addresses: Range<u64>,
        file: BorrowedFd<'_>,
        offset: u64,
    ) -> Result<(), Errno> {
        let page = rustix::param::page_size() as u64;
        let fits = addresses.start < addresses.end
            && addresses.end <= memory.0.length as u64
            && addresses.start.is_multiple_of(page)
            && addresses.end.is_multiple_of(page)
            && offset.is_multiple_of(page);
        if !fits {
            return Err(Errno::INVAL);
        }
        let start = usize::try_from(addresses.start).map_err(|_| Errno::INVAL)?;
        let length = usize::try_from(addresses.end - addresses.start).map_err(|_| Errno::INVAL)?;
        check_file(file, offset, length)?;
        memory.0.map_over(start, length, file, offset)
    }

    fn host_memory_bytes(
        &self,
        memory: &DirectHostMemory,
        offsets: Range<u64>,
    ) -> Option<DirectBytes> {
        let bytes = memory.0.bytes(offsets)?;
        Some(DirectBytes {
            bytes,
            _keeps: memory.0.clone(),
        })
    }

    fn new_memory(
        &self,
        mut store: impl AsContextMut,
        ty: MemoryType,
        memory: &DirectHostMemory,
    ) -> wasmtime::Result<Memory> {
        // wasmtime makes a memory of `Memory::new` with an allocator of its own that has no creator, so the memory is
        // the one export of an instance of a module that defines it, which the engine's allocator makes on this thread.
        let module = self.creator.module(store.as_context_mut().engine(), &ty)?;
        NEXT_HOST_MEMORY.with(|next| *next.borrow_mut() = Some((memory.0.clone(), ty)));
        let _clear = ClearNext;
        let instance = Instance::new(&mut store, &module, &[])?;
        let made = NEXT_HOST_MEMORY.with(|next| next.borrow().is_none());
        wasmtime::ensure!(made, NOT_CONFIGURED);
        instance
            .get_memory(&mut store, MEMORY_EXPORT)
            .ok_or_else(|| wasmtime::Error::msg("the module of one memory exports it"))
    }

    fn call_with_trap_handler<Returned>(
        &self,
        handler: &mut dyn TrapHandler,
        call: impl FnOnce() -> Returned,
    ) -> Returned {
        let lent = NonNull::from(handler);
        #[expect(
            unsafe_code,
            reason = "a store's data must be 'static, so the handler's lifetime is erased while the call that holds its borrow runs"
        )]
        // SAFETY: only the lifetime changes. The pointer stays in the slot only until `RestoreHandler` puts the one before
        // back, which happens before this function returns and so before the borrow of `handler` ends; while it is lent
        // it is out of the slot, so no two references to the handler live at once.
        let erased = unsafe {
            std::mem::transmute::<NonNull<dyn TrapHandler + '_>, NonNull<dyn TrapHandler + 'static>>(
                lent,
            )
        };
        let before = TRAP_HANDLER.with(|slot| slot.replace(Some(erased)));
        let _restore = RestoreHandler(before);
        call()
    }

    fn with_trap_handler<Returned>(
        &self,
        answer: impl FnOnce(&mut dyn TrapHandler) -> Returned,
    ) -> Option<Returned> {
        let mut taken = TRAP_HANDLER.with(Cell::take)?;
        let _restore = RestoreHandler(Some(taken));
        #[expect(
            unsafe_code,
            reason = "lends the handler a call in progress on this thread put in the slot"
        )]
        // SAFETY: it was in the slot, so `call_with_trap_handler` is running on this thread and its borrow of the handler
        // lives; it is out of the slot while lent, so this is the only reference to it.
        let handler = unsafe { taken.as_mut() };
        Some(answer(handler))
    }
}

impl HostMemories {
    /// Returns the engine's module that defines one memory of the type and exports it, compiled the first time.
    fn module(&self, engine: &Engine, ty: &MemoryType) -> wasmtime::Result<Module> {
        let mut modules = self.modules.lock().unwrap_or_else(PoisonError::into_inner);
        let kept = modules
            .iter()
            .find(|(kept_engine, kept_ty, _)| Engine::same(kept_engine, engine) && kept_ty == ty);
        if let Some((_, _, module)) = kept {
            return Ok(module.clone());
        }
        let page_size_log2 = ty.page_size_log2();
        let mut memories = MemorySection::new();
        memories.memory(wasm_encoder::MemoryType {
            minimum: ty.minimum(),
            maximum: ty.maximum(),
            memory64: ty.is_64(),
            shared: false,
            page_size_log2: (page_size_log2 != DEFAULT_PAGE_SIZE_LOG2)
                .then_some(u32::from(page_size_log2)),
        });
        let mut exports = ExportSection::new();
        exports.export(MEMORY_EXPORT, ExportKind::Memory, MEMORY_INDEX);
        let mut bytes = wasm_encoder::Module::new();
        bytes.section(&memories).section(&exports);
        let module = Module::new(engine, bytes.finish())?;
        modules.push((engine.clone(), ty.clone(), module.clone()));
        Ok(module)
    }
}

/// Returns `EINVAL` unless the file cannot shrink and holds `length` bytes from the offset.
///
/// Then no page mapped from it ever faults.
fn check_file(file: BorrowedFd<'_>, offset: u64, length: usize) -> Result<(), Errno> {
    if !rustix::fs::fcntl_get_seals(file)?.contains(SealFlags::SHRINK) {
        return Err(Errno::INVAL);
    }
    let size = u64::try_from(rustix::fs::fstat(file)?.st_size).map_err(|_| Errno::INVAL)?;
    let end = offset.checked_add(length as u64).ok_or(Errno::INVAL)?;
    if end > size {
        return Err(Errno::INVAL);
    }
    Ok(())
}

impl Reservation {
    /// Reserves `length` and `guard` bytes anywhere, all inaccessible.
    fn new(length: usize, guard: usize) -> Result<Reservation, Errno> {
        let reserved = length.checked_add(guard).ok_or(Errno::INVAL)?;
        #[expect(
            unsafe_code,
            reason = "reserves addresses anywhere the kernel chooses, which rustix declares unsafe for the hint it takes"
        )]
        // SAFETY: a null hint lets the kernel choose addresses nothing else uses; the new mapping allows nothing.
        let base = unsafe {
            rustix::mm::mmap_anonymous(
                std::ptr::null_mut(),
                reserved,
                ProtFlags::empty(),
                MapFlags::PRIVATE | MapFlags::NORESERVE,
            )
        }?;
        Ok(Reservation {
            base: AtomicPtr::new(base.cast()),
            length,
            guard,
        })
    }

    fn base(&self) -> *mut u8 {
        self.base.load(Ordering::Relaxed)
    }

    /// Maps `length` bytes of the file from the offset at `start` bytes into the reservation, shared and writable.
    ///
    /// The mapping is also readable. The caller checked both ranges.
    fn map_over(
        &self,
        start: usize,
        length: usize,
        file: BorrowedFd<'_>,
        offset: u64,
    ) -> Result<(), Errno> {
        let at = self.base().wrapping_add(start).cast::<c_void>();
        #[expect(
            unsafe_code,
            reason = "maps a file over addresses of the reservation, which rustix leaves to the caller to keep sound"
        )]
        // SAFETY: the addresses lie inside this reservation, which this value alone made and owns. The kernel replaces
        // what they showed in one step, so each byte stays readable and writable throughout, as the promise of the
        // reservation's bytes asks; the file cannot shrink and holds the range, so no page of it faults.
        let mapped = unsafe {
            rustix::mm::mmap(
                at,
                length,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED | MapFlags::FIXED,
                file,
                offset,
            )
        }?;
        debug_assert_eq!(mapped, at, "a fixed mapping lands where it was asked");
        Ok(())
    }

    /// Lets the first `length` bytes be read and written.
    fn open(&self, length: usize) -> Result<(), Errno> {
        if length == 0 {
            return Ok(());
        }
        #[expect(
            unsafe_code,
            reason = "opens the start of the reservation, which rustix takes as a raw pointer"
        )]
        // SAFETY: the start of this value's own private anonymous reservation, which holds at least `length` bytes and
        // which no host memory's bytes are lent from.
        unsafe {
            rustix::mm::mprotect(
                self.base().cast::<c_void>(),
                length,
                MprotectFlags::READ | MprotectFlags::WRITE,
            )
        }
    }

    /// Returns the bytes of a host memory at the offsets, which another party may change; none past its length.
    fn bytes(&self, offsets: Range<u64>) -> Option<PointerBytes> {
        let start = usize::try_from(offsets.start).ok()?;
        let end = usize::try_from(offsets.end).ok()?;
        if start > end || end > self.length {
            return None;
        }
        #[expect(
            unsafe_code,
            reason = "the one promise about a host memory's bytes, on which every copy of them relies"
        )]
        // SAFETY: only host memories lend bytes, and only the offsets checked above, inside the length. A file that cannot shrink is mapped over every page of a host memory's
        // length before it is handed out, and every later mapping over it is another such file, put in place in one step,
        // so the bytes are readable and writable from any thread and never trap while the reservation lives; every
        // `DirectBytes` keeps it alive; this crate forms no reference or slice over them, and wasmtime reaches them only
        // from compiled code.
        let pointer =
            unsafe { PointerAndLength::new(self.base().wrapping_add(start), end - start) };
        Some(PointerBytes::new(pointer))
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "unmaps the reservation this value alone holds")]
        // SAFETY: the reservation is this value's alone; every store memory and view of it held it, so none is left.
        let unmapped =
            unsafe { rustix::mm::munmap(self.base().cast::<c_void>(), self.length + self.guard) };
        debug_assert!(unmapped.is_ok(), "a reservation is unmapped");
    }
}

#[expect(
    unsafe_code,
    reason = "wasmtime's trait for a memory the embedder made is unsafe"
)]
// SAFETY: the base never moves; `byte_capacity` bytes from it are readable and writable while the reservation lives,
// which this value keeps alive, and `guard` bytes after them allow nothing; `byte_size` never passes the capacity.
unsafe impl LinearMemory for StoreMemory {
    fn byte_size(&self) -> usize {
        self.size
    }

    fn byte_capacity(&self) -> usize {
        self.reservation.length
    }

    fn grow_to(&mut self, new_size: usize) -> wasmtime::Result<()> {
        wasmtime::ensure!(new_size <= self.reservation.length, GROWN_PAST_LENGTH);
        self.size = new_size;
        Ok(())
    }

    fn as_ptr(&self) -> *mut u8 {
        self.reservation.base()
    }
}

#[expect(
    unsafe_code,
    reason = "wasmtime's trait for a memory the embedder made is unsafe"
)]
// SAFETY: the base never moves; `byte_size` bytes from it are readable and writable, opened as the memory grows, and the
// rest of the capacity and the guard after it allow nothing until then; the size never passes the capacity.
unsafe impl LinearMemory for OwnMemory {
    fn byte_size(&self) -> usize {
        self.size
    }

    fn byte_capacity(&self) -> usize {
        self.reservation.length
    }

    fn grow_to(&mut self, new_size: usize) -> wasmtime::Result<()> {
        wasmtime::ensure!(new_size <= self.reservation.length, GROWN_PAST_RESERVATION);
        self.reservation
            .open(new_size)
            .map_err(|error| wasmtime::Error::msg(format!("opening a memory: {error}")))?;
        self.size = new_size;
        Ok(())
    }

    fn as_ptr(&self) -> *mut u8 {
        self.reservation.base()
    }
}

#[expect(
    unsafe_code,
    reason = "wasmtime's trait for making the memories of an engine is unsafe"
)]
// SAFETY: a memory of the type `new_memory` asked for on this thread is its host memory, whose length holds the size,
// the most it may grow to and the reservation wasmtime asked for, followed by at least the guard it asked for. Any other
// memory is a fresh reservation of that size and guard. Either way every bounds check wasmtime leaves out lands on a
// byte the memory has or a page that allows nothing.
unsafe impl MemoryCreator for HostMemories {
    fn new_memory(
        &self,
        ty: MemoryType,
        minimum: usize,
        maximum: Option<usize>,
        reserved_size_in_bytes: Option<usize>,
        guard_size_in_bytes: usize,
    ) -> Result<Box<dyn LinearMemory>, String> {
        let next = NEXT_HOST_MEMORY.with(|next| {
            let mut next = next.borrow_mut();
            let asked = next.as_ref().is_some_and(|(_, asked)| *asked == ty);
            if asked { next.take() } else { None }
        });
        let Some((reservation, _)) = next else {
            let capacity = reserved_size_in_bytes
                .unwrap_or(maximum.unwrap_or(minimum))
                .max(minimum);
            let reservation = Reservation::new(capacity, guard_size_in_bytes)
                .map_err(|error| format!("reserving a memory: {error}"))?;
            reservation
                .open(minimum)
                .map_err(|error| format!("opening a memory: {error}"))?;
            return Ok(Box::new(OwnMemory {
                reservation,
                size: minimum,
            }));
        };
        let fits = minimum <= reservation.length
            && maximum.is_none_or(|maximum| maximum <= reservation.length)
            && reserved_size_in_bytes.is_none_or(|reserved| reserved <= reservation.length)
            && guard_size_in_bytes <= reservation.guard;
        if !fits {
            return Err(format!(
                "a memory of {minimum} to {maximum:?} bytes, reserving {reserved_size_in_bytes:?} with a guard of \
                 {guard_size_in_bytes}, does not fit a host memory of {} bytes with a guard of {}",
                reservation.length, reservation.guard
            ));
        }
        Ok(Box::new(StoreMemory {
            reservation,
            size: minimum,
        }))
    }
}

impl VolatileBytes for DirectBytes {
    fn len(&self) -> u64 {
        self.bytes.len()
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        self.bytes.read_at(offset, into)
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        self.bytes.write_at(offset, bytes)
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        self.bytes.read_to(sink)
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        self.bytes.write_from(source)
    }
}
