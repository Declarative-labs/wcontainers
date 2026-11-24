//! How form P is driven (plan 4.6), the one place that implements an executor trait: the kernel's loop drives it
//! through the runtime's `Execution`, and the executor interface through [`HostProcess`] and its memories.

use core::convert::Infallible;
use std::cell::{RefCell, RefMut};
use std::ops::Range;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use aarch64_cpu::registers::ESR_EL1;
use rangemap::RangeSet;
use rustix::time::{ClockId, clock_gettime};
use wcontainers_abi::{
    Access, Cpu, CpuError, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, Resume, SharedId,
    ThreadId, Warning,
};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace,
};
use wcontainers_executor::{
    ExecutionInterrupter, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_processor_architecture::MemoryDelta;
use wcontainers_processor_architecture_arm64::encoding::IMM16;
use wcontainers_processor_architecture_arm64::memory::report_changed_bytes;
use wcontainers_processor_architecture_arm64::syndrome::{
    FaultStatus, WRITE_NOT_READ, encode_syndrome,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, Exception, INSTRUCTION_BYTES, InstructionFetch,
    PagePermission, Processor,
};
use wcontainers_runtime_single_kernel::{Answer, Carry, Execution, Turn};
use wcontainers_volatile_bytes::VolatileBytes;

use super::{GUEST, HostProcessExecutor, data_extents, page_cover, pieces};
use crate::host::HostProcessHost;
use crate::stub::{StopTarget, StopWhen};

/// The bytes a copy between memories moves at a time.
const COPY_CHUNK: usize = 1 << 20;

impl HostProcessExecutor {
    /// The count at which `turn`, starting on `thread` now, ends, and the time that bounds each entry into its code:
    /// with lone pieces none, since the thread comes back at every instruction's end; otherwise a unit of progress is
    /// taken as a nanosecond, since linked pieces cannot be stopped at an exact count.
    fn turn_end(&mut self, thread: ThreadId, turn: Turn) -> (Option<u64>, Option<Duration>) {
        match turn {
            Turn::Counted { limit: None } | Turn::Free => (None, None),
            Turn::Counted { limit: Some(limit) } if self.lone => {
                let count = self.guest_count(thread);
                self.slow(super::native::reason::TURN_RUN_IN_STEPS, 1, count);
                (Some(count + limit), None)
            }
            Turn::Counted { limit: Some(limit) } => (
                Some(self.guest_count(thread) + limit),
                Some(Duration::from_nanos(limit)),
            ),
        }
    }

    fn guest_count(&self, thread: ThreadId) -> u64 {
        self.guests[thread.0 as usize]
            .as_ref()
            .expect("a thread the kernel holds")
            .count()
    }

    /// Prints each slow path whose count reached a power of two since the last call: the executor interface has no
    /// channel for warnings, and a path taken at every entry must not flood the output.
    fn print_warnings(&mut self) {
        for (reason, count, detail) in self.take_warnings() {
            if count.is_power_of_two() {
                eprintln!("wcontainers warning: {reason}, {count} so far, the last at {detail:#x}");
            }
        }
    }

    /// Drops every block no memory maps: under the executor interface a block lives only while it is mapped.
    fn drop_unmapped_blocks(&mut self) {
        let mut mapped = vec![false; self.blocks.len()];
        for memory in self.memories.iter().flatten() {
            for (_, (block, _)) in memory.shared.iter() {
                mapped[block.0 as usize] = true;
            }
        }
        for (index, mapped) in mapped.into_iter().enumerate() {
            if !mapped && self.blocks[index].is_some() {
                self.shared_drop(SharedId(index as u32));
            }
        }
    }

    /// The parts of `range` of `id`'s private pages that may hold bytes other than zeros.
    fn private_data(&mut self, id: MemoryId, range: Range<u64>) -> Result<RangeSet<u64>, CpuError> {
        let mut data = RangeSet::new();
        match data_extents(self.memory(id).stub.memory()).map_err(super::host_error)? {
            Some(extents) => {
                for extent in extents {
                    let start = extent.start.max(range.start);
                    let end = extent.end.min(range.end);
                    if start < end {
                        data.insert(start..end);
                    }
                }
            }
            None => {
                self.counters.extents_unknown += 1;
                eprintln!(
                    "wcontainers warning: memory extents unknown, a copy reads every private page, {} so far",
                    self.counters.extents_unknown
                );
                data.insert(range.clone());
            }
        }
        for (piece, state) in pieces(&self.memory(id).pages, range) {
            if matches!(state, Some(super::PageState::Borrowed(_))) {
                data.insert(piece);
            }
        }
        Ok(data)
    }

    /// Maps the pages of `other` from `from` at `to` of `id`; the private ones first become a block both map.
    fn map_pages(
        &mut self,
        id: MemoryId,
        to: Range<u64>,
        other: MemoryId,
        from: u64,
    ) -> Result<(), CpuError> {
        let shift = to.start.wrapping_sub(from);
        let source = from..from + (to.end - to.start);
        let mut buffer = vec![0; COPY_CHUNK];
        for (piece, shared) in pieces(&self.memory(other).shared, source) {
            let length = piece.end - piece.start;
            let (block, offset) = match shared {
                Some((block, delta)) => (block, piece.start.wrapping_add(delta)),
                None => {
                    let block = self.shared_new(length)?;
                    for part in self.private_data(other, piece.clone())?.iter() {
                        for at in part.clone().step_by(COPY_CHUNK) {
                            let bytes =
                                &mut buffer[..(part.end - at).min(COPY_CHUNK as u64) as usize];
                            self.read_guest(other, at, bytes)?;
                            self.shared_write(block, at - piece.start, bytes)?;
                        }
                    }
                    self.shared_map(block, 0, other, piece.start, length)?;
                    (block, 0)
                }
            };
            self.shared_map(block, offset, id, piece.start.wrapping_add(shift), length)?;
        }
        Ok(())
    }

    /// Lends `borrower` every private page of `owner` in `range` that may hold bytes, at the same addresses; a page
    /// `owner` borrows is borrowed from its own owner.
    fn lend_private(
        &mut self,
        owner: MemoryId,
        borrower: MemoryId,
        range: Range<u64>,
    ) -> Result<(), CpuError> {
        let data: Vec<Range<u64>> = self.private_data(owner, range)?.into_iter().collect();
        for part in data {
            for (pages, state) in pieces(&self.memory(owner).pages, page_cover(&part)) {
                let from = match state {
                    Some(super::PageState::Borrowed(from)) => from,
                    _ => owner,
                };
                self.add_borrower(from, pages.clone(), borrower);
                self.memory_mut(borrower)
                    .pages
                    .insert(pages, super::PageState::Borrowed(from));
            }
        }
        Ok(())
    }

    /// Queues the loss of write in the view of every writable range of `id` inside `range`; true when one was queued.
    fn take_write_from_lent(&mut self, id: MemoryId, range: Range<u64>) -> Result<bool, CpuError> {
        let writable: Vec<(Range<u64>, Permissions)> = self
            .memory(id)
            .permissions
            .overlapping(range.clone())
            .filter(|(_, granted)| granted.contains(Permissions::WRITE))
            .map(|(part, granted)| {
                (
                    part.start.max(range.start)..part.end.min(range.end),
                    *granted,
                )
            })
            .collect();
        self.room(id, writable.len())?;
        for (part, granted) in &writable {
            let view = self.view_bound(id, part.clone(), *granted);
            self.memory_mut(id)
                .push(super::protect(part.clone(), view), None);
        }
        Ok(!writable.is_empty())
    }

    /// Zeroes the parts of `range` that show a block.
    fn unmap_pages(&mut self, id: MemoryId, range: Range<u64>) -> Result<(), CpuError> {
        for (piece, shared) in pieces(&self.memory(id).shared, range) {
            if shared.is_some() {
                self.zero(id, piece.start, piece.end - piece.start)?;
            }
        }
        Ok(())
    }

    /// Makes `to` of `id` what `other` holds from `from`: its private bytes lent at the same addresses (as a fork lends
    /// them) or else copied, its blocks mapped, its permissions.
    fn copy_pages(
        &mut self,
        id: MemoryId,
        to: Range<u64>,
        other: MemoryId,
        from: u64,
    ) -> Result<(), CpuError> {
        let length = to.end - to.start;
        let shift = to.start.wrapping_sub(from);
        let source = from..from + length;
        if id == other {
            if shift == 0 {
                return Ok(());
            }
            // The source's bytes are first lent to a memory of their own, since the destination may overlap them.
            let held = self.memory_new()?;
            let copied = self
                .copy_pages(held, source.clone(), other, from)
                .and_then(|()| self.copy_pages(id, to, held, from));
            self.memory_drop(held);
            return copied;
        }
        self.zero(id, to.start, length)?;
        self.set_permissions(id, to.start, length, Permissions::empty())?;
        let mut buffer = vec![0; COPY_CHUNK];
        for (piece, shared) in pieces(&self.memory(other).shared, source.clone()) {
            let length = piece.end - piece.start;
            match shared {
                Some((block, delta)) => self.shared_map(
                    block,
                    piece.start.wrapping_add(delta),
                    id,
                    piece.start.wrapping_add(shift),
                    length,
                )?,
                None if shift == 0 => self.lend_private(other, id, piece)?,
                None => {
                    for part in self.private_data(other, piece)?.iter() {
                        for at in part.clone().step_by(COPY_CHUNK) {
                            let bytes =
                                &mut buffer[..(part.end - at).min(COPY_CHUNK as u64) as usize];
                            self.read_guest(other, at, bytes)?;
                            if bytes.iter().any(|&byte| byte != 0) {
                                self.write(id, at.wrapping_add(shift), bytes)?;
                            }
                        }
                    }
                }
            }
        }
        if shift == 0 && self.take_write_from_lent(other, source.clone())? {
            self.apply_now(other)?;
        }
        let granted: Vec<(Range<u64>, Permissions)> = self
            .memory(other)
            .permissions
            .overlapping(source.clone())
            .map(|(range, allowed)| {
                (
                    range.start.max(source.start)..range.end.min(source.end),
                    *allowed,
                )
            })
            .collect();
        for (range, allowed) in granted {
            self.set_permissions(
                id,
                range.start.wrapping_add(shift),
                range.end - range.start,
                allowed,
            )?;
        }
        Ok(())
    }
}

impl Execution for HostProcessExecutor {
    type Failure = Infallible;

    /// Counter reads are told no progress; the progress made before one is handed with the exit that follows.
    fn run(
        &mut self,
        thread: ThreadId,
        mut resume: Resume,
        turn: Turn,
        answer: &mut dyn Answer,
    ) -> Result<(), Infallible> {
        let (mut until, mut slice) = self.turn_end(thread, turn);
        loop {
            let before = self.guest_count(thread);
            let when = StopWhen {
                slice,
                wanted: None,
            };
            let exit = self.run_translated(thread, resume, until, when, &mut || answer.clock(0));
            for (reason, count, detail) in self.take_warnings() {
                answer.warn(Warning {
                    reason,
                    count,
                    detail,
                });
            }
            let progress = self.guest_count(thread) - before;
            if exit != Exit::Stop {
                self.counters.kernel_entries += 1;
            }
            match answer.exit(self, exit, progress) {
                Carry::On {
                    resume: next,
                    turn: next_turn,
                } => {
                    (until, slice) = self.turn_end(thread, next_turn);
                    resume = next;
                }
                Carry::Leave(_) => return Ok(()),
            }
        }
    }

    fn restored(&self) -> Self {
        unimplemented!("a save of form P is not built")
    }
}

/// Form P under the executor interface: the executor's state, shared with every memory it made.
pub struct HostProcess {
    host: Arc<HostProcessHost>,
    executor: Rc<RefCell<HostProcessExecutor>>,
    /// Runs single instructions, each in a memory of its own; made at the first.
    single: Option<HostProcessExecutor>,
}

impl HostProcess {
    /// Stands in for the way the executor interface does not yet give to make an executor.
    pub fn new(host: Arc<HostProcessHost>) -> Self {
        Self {
            executor: Rc::new(RefCell::new(HostProcessExecutor::new(Arc::clone(&host)))),
            host,
            single: None,
        }
    }

    /// The executor's counts so far.
    pub fn counters(&self) -> super::Counters {
        lock(&self.executor).counters()
    }
}

/// A guest memory of form P: a stub of the executor that made it.
pub struct HostProcessMemory {
    executor: Rc<RefCell<HostProcessExecutor>>,
    id: MemoryId,
}

impl HostProcessMemory {
    fn lock(&self) -> RefMut<'_, HostProcessExecutor> {
        lock(&self.executor)
    }

    /// Panics unless `other` was made by the same executor: its pages are another executor's stubs.
    fn same_executor(&self, other: &Self) {
        assert!(
            Rc::ptr_eq(&self.executor, &other.executor),
            "a memory of another executor"
        );
    }
}

impl Drop for HostProcessMemory {
    fn drop(&mut self) {
        let mut executor = lock(&self.executor);
        executor.memory_drop(self.id);
        executor.drop_unmapped_blocks();
    }
}

/// Bytes of a form P memory, read and written by position in the stub's memfd.
pub struct HostProcessBytes<'space> {
    memory: &'space HostProcessMemory,
    addresses: Range<u64>,
}

impl VolatileBytes for HostProcessBytes<'_> {
    fn len(&self) -> u64 {
        self.addresses.end - self.addresses.start
    }

    /// A host refusal to map the bytes reads none.
    fn read(&self, into: &mut [u8]) -> usize {
        let count = into.len().min(self.len() as usize);
        match self.memory.lock().read_guest(
            self.memory.id,
            self.addresses.start,
            &mut into[..count],
        ) {
            Ok(()) => count,
            Err(_) => 0,
        }
    }

    /// A host refusal to map the bytes writes none.
    fn write(&self, bytes: &[u8]) -> usize {
        let count = bytes.len().min(self.len() as usize);
        match self
            .memory
            .lock()
            .write(self.memory.id, self.addresses.start, &bytes[..count])
        {
            Ok(()) => count,
            Err(_) => 0,
        }
    }

    /// The bytes are copied out of the stub's memory first, so the sink is given them in one write.
    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let mut buffer = vec![u8::MIN; self.len() as usize];
        let count = self.read(&mut buffer);
        sink.write(&buffer[..count])
    }

    /// The source fills a buffer in one read, which is then copied into the stub's memory.
    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut buffer = vec![u8::MIN; self.len() as usize];
        let count = source.read(&mut buffer)?;
        Ok(self.write(&buffer[..count]))
    }
}

impl MemorySpace for HostProcessMemory {
    type Bytes<'space> = HostProcessBytes<'space>;

    fn borrow(&self, addresses: Range<u64>) -> Result<HostProcessBytes<'_>, InvalidAddressError> {
        inside(&addresses)?;
        Ok(HostProcessBytes {
            memory: self,
            addresses,
        })
    }

    fn zero(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        inside(&addresses)?;
        let mut executor = self.lock();
        let zeroed = executor.zero(self.id, addresses.start, addresses.end - addresses.start);
        executor.drop_unmapped_blocks();
        zeroed.map_err(refused)
    }

    /// Every page the addresses touch gets the permissions.
    fn set_permissions(
        &self,
        addresses: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), InvalidAddressError> {
        inside(&addresses)?;
        let pages = page_cover(&addresses);
        self.lock()
            .set_permissions(self.id, pages.start, pages.end - pages.start, allowed)
            .map_err(refused)
    }

    /// Only whole pages are mapped: addresses that do not start and end on a page are refused.
    fn map_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), MapMemoryError> {
        self.same_executor(other);
        let source =
            whole_pages(&addresses, other_start).map_err(MapMemoryError::InvalidAddress)?;
        let mut executor = self.lock();
        let mapped = executor.map_pages(self.id, addresses, other.id, source.start);
        executor.drop_unmapped_blocks();
        mapped.map_err(|error| match error {
            CpuError::BadAddress => MapMemoryError::InvalidAddress(InvalidAddressError),
            _ => MapMemoryError::OutOfMemory,
        })
    }

    fn unmap_from(&self, addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        inside(&addresses)?;
        let mut executor = self.lock();
        let unmapped = executor.unmap_pages(self.id, addresses);
        executor.drop_unmapped_blocks();
        unmapped.map_err(refused)
    }

    /// Only whole pages are copied: addresses that do not start and end on a page are refused.
    fn copy_from(
        &self,
        addresses: Range<u64>,
        other: &Self,
        other_start: u64,
    ) -> Result<(), CopyMemoryError> {
        self.same_executor(other);
        let source =
            whole_pages(&addresses, other_start).map_err(CopyMemoryError::InvalidAddress)?;
        let mut executor = self.lock();
        let copied = executor.copy_pages(self.id, addresses, other.id, source.start);
        executor.drop_unmapped_blocks();
        copied.map_err(|error| match error {
            CpuError::BadAddress => CopyMemoryError::InvalidAddress(InvalidAddressError),
            _ => CopyMemoryError::OutOfMemory,
        })
    }
}

/// How a run under `execute` ended.
enum Ended {
    /// A system call answered terminate: the state is after the call.
    SystemCall,
    MemoryFault {
        address: u64,
        access: Access,
    },
    /// An interrupt answered terminate.
    Interrupt,
    /// An exit no handler is given.
    Exit(Exit),
}

impl Executor for HostProcess {
    type Architecture = Arm64;
    type TranslatorConfig = ();
    type GuestMemory = HostProcessMemory;
    type CreateGuestMemoryError = CpuError;
    type Interrupter = HostProcessInterrupter;

    fn create_guest_memory(&self) -> Result<HostProcessMemory, CpuError> {
        let id = lock(&self.executor).memory_new()?;
        Ok(HostProcessMemory {
            executor: Rc::clone(&self.executor),
            id,
        })
    }

    /// The execution runs to its end at its first poll, on a host thread of the memory's stub: the guest runs in
    /// another process, and nothing in the interface wakes a waiting future when that process answers.
    fn execute<'execution>(
        &'execution mut self,
        processor: Processor,
        memory: &'execution HostProcessMemory,
        handler: &'execution mut impl TrapHandler,
    ) -> (
        impl Future<Output = Processor> + 'execution,
        HostProcessInterrupter,
    ) {
        assert!(
            Rc::ptr_eq(&self.executor, &memory.executor),
            "a memory this executor made"
        );
        let interruption = Arc::new(Interruption::default());
        let interrupter = HostProcessInterrupter {
            interruption: Arc::clone(&interruption),
        };
        let executor = &self.executor;
        (
            async move { run_to_end(executor, processor, memory, handler, &interruption) },
            interrupter,
        )
    }

    /// Runs the instruction alone in a memory made of the deltas, its piece a stretch of that one instruction, never
    /// linked, so its first landing ends the run; the pages it may write are compared before and after.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: &[MemoryDelta<'_, Arm64>],
        mut on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    ) {
        let host = &self.host;
        let executor = self.single.get_or_insert_with(|| {
            let mut executor = HostProcessExecutor::new(Arc::clone(host));
            executor.lone = true;
            executor
        });
        let space = executor.memory_new().expect("the host makes one more stub");
        let mut permissions = std::collections::BTreeMap::new();
        for delta in memory {
            match delta {
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } => {
                    executor
                        .set_permissions(
                            space,
                            addresses.start,
                            addresses.end - addresses.start,
                            to_permissions(*permission),
                        )
                        .expect("the state's pages lie in form P's guest area");
                    for page in addresses.clone().step_by(PAGE_SIZE as usize) {
                        permissions.insert(page, *permission);
                    }
                }
                MemoryDelta::ValueAssignment { start, bytes } => executor
                    .write(space, *start, bytes)
                    .expect("the state's bytes lie in form P's guest area"),
            }
        }
        let writable: Vec<u64> = permissions
            .iter()
            .filter(|(_, permission)| {
                matches!(
                    permission,
                    PagePermission::Mapped {
                        data_access: DataAccess::ReadWrite,
                        ..
                    }
                )
            })
            .map(|(page, _)| *page)
            .collect();
        let read_page = |executor: &HostProcessExecutor, page: u64| {
            let mut bytes = vec![0; PAGE_SIZE as usize];
            executor
                .read_guest(space, page, &mut bytes)
                .expect("a page of the state");
            bytes
        };
        let before: Vec<Vec<u8>> = writable
            .iter()
            .map(|page| read_page(executor, *page))
            .collect();
        let thread = executor
            .thread_new(space)
            .expect("the stub has its first host thread");
        executor.set_registers(thread, &to_register_block(processor));
        // Reached at the first instruction's end.
        let until = executor.guest_count(thread);
        let exit = executor.run_translated(
            thread,
            Resume::FromRegisters,
            Some(until),
            StopWhen::default(),
            &mut monotonic,
        );
        executor.print_warnings();
        let mut registers = RegisterBlock::ZERO;
        executor.registers(thread, &mut registers);
        let exception = match exit {
            Exit::Stop => None,
            Exit::Syscall(_) => {
                registers.pc = processor.program_counter;
                let mut word = [0; INSTRUCTION_BYTES as usize];
                let _ = executor.read_guest(space, registers.pc, &mut word);
                Some(system_call(u32::from_le_bytes(word)))
            }
            Exit::MemoryFault { address, access } => {
                let page = address & !(PAGE_SIZE - 1);
                let mapped = permissions
                    .get(&page)
                    .is_some_and(|permission| *permission != PagePermission::Unmapped);
                Some(abort(address, access, mapped))
            }
            other => Some(to_exception(other, registers.pc)),
        };
        *processor = to_processor(&registers, exception);
        for (page, before) in writable.iter().zip(before) {
            let after = read_page(executor, *page);
            report_changed_bytes(*page, &before, &after, &mut on_memory_mutated);
        }
        executor.thread_drop(thread);
        executor.memory_drop(space);
    }
}

/// The executor's state, never held while a trap handler runs, which may use the memories.
fn lock(executor: &RefCell<HostProcessExecutor>) -> RefMut<'_, HostProcessExecutor> {
    executor.borrow_mut()
}

/// Runs `processor` on a host thread of `memory`'s stub until a trap ends it; the executor is not held while a handler
/// runs, and an interrupt asked for is taken before each entry or stops the thread's turn.
fn run_to_end(
    shared: &RefCell<HostProcessExecutor>,
    processor: Processor,
    memory: &HostProcessMemory,
    handler: &mut impl TrapHandler,
    interruption: &Interruption,
) -> Processor {
    let mut executor = lock(shared);
    let thread = executor
        .thread_new(memory.id)
        .expect("the stub makes one more host thread");
    executor.set_registers(thread, &to_register_block(&processor));
    let index = executor.thread(thread).index;
    *interruption.target() = Some(executor.memory(memory.id).stub.stop_target(index));
    let mut resume = Resume::FromRegisters;
    let ended = loop {
        let before_entry = interruption.requested.swap(false, Ordering::AcqRel);
        let exit = if before_entry {
            Exit::Stop
        } else {
            let when = StopWhen {
                slice: None,
                wanted: Some(&interruption.requested),
            };
            let exit = executor.run_translated(thread, resume, None, when, &mut monotonic);
            executor.print_warnings();
            if exit == Exit::Stop {
                interruption.requested.store(false, Ordering::Release);
            }
            exit
        };
        let next = match exit {
            Exit::Syscall(_) | Exit::MemoryFault { .. } | Exit::Stop => {
                if exit != Exit::Stop {
                    executor.counters.kernel_entries += 1;
                }
                drop(executor);
                let next = match exit {
                    Exit::Syscall(request) => match handler.handle_system_call(request) {
                        SystemCallResponse::Return(value) => Ok(Resume::Return(value)),
                        SystemCallResponse::Terminate => Err(Ended::SystemCall),
                    },
                    Exit::MemoryFault { address, access } => {
                        match handler.handle_memory_fault(address, access) {
                            MemoryFaultResponse::Retry => Ok(Resume::FromRegisters),
                            MemoryFaultResponse::Terminate => {
                                Err(Ended::MemoryFault { address, access })
                            }
                        }
                    }
                    _ => match handler.handle_interrupt() {
                        InterruptResponse::Continue => Ok(Resume::Continue),
                        InterruptResponse::Terminate => Err(Ended::Interrupt),
                    },
                };
                executor = lock(shared);
                // An interrupt answered continue before the thread entered goes on as it would have.
                match next {
                    Ok(Resume::Continue) if before_entry => Ok(resume),
                    next => next,
                }
            }
            other => Err(Ended::Exit(other)),
        };
        match next {
            Ok(next) => resume = next,
            Err(ended) => break ended,
        }
    };
    *interruption.target() = None;
    let mut registers = RegisterBlock::ZERO;
    executor.registers(thread, &mut registers);
    let exception = match ended {
        Ended::SystemCall => {
            registers.pc -= INSTRUCTION_BYTES;
            let mut word = [0; INSTRUCTION_BYTES as usize];
            let _ = executor.read_guest(memory.id, registers.pc, &mut word);
            Some(system_call(u32::from_le_bytes(word)))
        }
        Ended::MemoryFault { address, access } => {
            let page = address & !(PAGE_SIZE - 1);
            let mapped = executor.memory(memory.id).permissions.get(&page).is_some();
            Some(abort(address, access, mapped))
        }
        Ended::Interrupt => None,
        Ended::Exit(exit) => Some(to_exception(exit, registers.pc)),
    };
    executor.thread_drop(thread);
    to_processor(&registers, exception)
}

/// What an execution's interrupter and its run share.
#[derive(Default)]
struct Interruption {
    /// An interrupt asked for and not yet given to the handler.
    requested: AtomicBool,
    /// Where the stop goes while the run has a thread.
    target: Mutex<Option<StopTarget>>,
}

impl Interruption {
    fn target(&self) -> MutexGuard<'_, Option<StopTarget>> {
        self.target.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Interrupts one execution of form P: the stop signal to its thread, and a request its run takes before its next entry
/// or sends the stop again for, once a stop sent while the thread was between turns has landed in its wait.
pub struct HostProcessInterrupter {
    interruption: Arc<Interruption>,
}

impl ExecutionInterrupter for HostProcessInterrupter {
    fn interrupt(&self) {
        self.interruption.requested.store(true, Ordering::Release);
        if let Some(target) = *self.interruption.target() {
            let _ = target.stop();
        }
    }
}

/// The guest's counter, which the translator presents at a nanosecond a tick: the host's monotonic clock, since the
/// executor interface gives no clock of the system's.
fn monotonic() -> u64 {
    const NANOSECONDS: u64 = 1_000_000_000;
    let now = clock_gettime(ClockId::Monotonic);
    now.tv_sec as u64 * NANOSECONDS + now.tv_nsec as u64
}

fn inside(addresses: &Range<u64>) -> Result<(), InvalidAddressError> {
    if addresses.start <= addresses.end
        && addresses.start >= GUEST.start
        && addresses.end <= GUEST.end
    {
        Ok(())
    } else {
        Err(InvalidAddressError)
    }
}

/// The other space's addresses of a mapping or copy to `addresses` from `other_start`, both whole pages inside.
fn whole_pages(
    addresses: &Range<u64>,
    other_start: u64,
) -> Result<Range<u64>, InvalidAddressError> {
    inside(addresses)?;
    let length = addresses.end - addresses.start;
    let source = other_start..other_start.checked_add(length).ok_or(InvalidAddressError)?;
    inside(&source)?;
    let whole = |at: u64| at.is_multiple_of(PAGE_SIZE);
    if whole(addresses.start) && whole(length) && whole(other_start) {
        Ok(source)
    } else {
        Err(InvalidAddressError)
    }
}

/// An address refused, or a panic: the memory space's methods have no error for a host that refuses its resources.
fn refused(error: CpuError) -> InvalidAddressError {
    match error {
        CpuError::BadAddress => InvalidAddressError,
        other => panic!("the host refused a change to a guest memory ({other:?})"),
    }
}

fn to_permissions(permission: PagePermission) -> Permissions {
    match permission {
        PagePermission::Unmapped => Permissions::empty(),
        PagePermission::Mapped {
            data_access,
            instruction_fetch,
        } => {
            let data = match data_access {
                DataAccess::Forbidden => Permissions::empty(),
                DataAccess::ReadOnly => Permissions::READ,
                DataAccess::ReadWrite => Permissions::READ | Permissions::WRITE,
            };
            let fetch = match instruction_fetch {
                InstructionFetch::Forbidden => Permissions::empty(),
                InstructionFetch::Allowed => Permissions::EXECUTE,
            };
            data | fetch
        }
    }
}

fn to_register_block(processor: &Processor) -> RegisterBlock {
    RegisterBlock {
        x: processor.general_purpose_registers,
        sp: processor.stack_pointer,
        pc: processor.program_counter,
        pstate: u64::from(processor.condition_flags.bits()),
        v: processor
            .vector_registers
            .map(|value| [value as u64, (value >> u64::BITS) as u64]),
        fpsr: processor.floating_point_status as u32,
        fpcr: processor.floating_point_control as u32,
        tpidr_el0: processor.software_thread_id,
    }
}

fn to_processor(registers: &RegisterBlock, exception: Option<Exception>) -> Processor {
    Processor {
        general_purpose_registers: registers.x,
        stack_pointer: registers.sp,
        program_counter: registers.pc,
        condition_flags: ConditionFlags::from_bits_truncate(registers.pstate as u32),
        vector_registers: registers
            .v
            .map(|[low, high]| u128::from(low) | u128::from(high) << u64::BITS),
        floating_point_control: u64::from(registers.fpcr),
        floating_point_status: u64::from(registers.fpsr),
        software_thread_id: registers.tpidr_el0,
        exception,
    }
}

/// The exception of the system call instruction `word`.
fn system_call(word: u32) -> Exception {
    Exception {
        syndrome: encode_syndrome(ESR_EL1::EC::Value::SVC64, u64::from(IMM16.read(word))),
        fault_address: None,
    }
}

/// The abort of an access to `address`, whose page held no permission unless `mapped`.
fn abort(address: u64, access: Access, mapped: bool) -> Exception {
    let status = if mapped {
        FaultStatus::PermissionFaultLevel3
    } else {
        FaultStatus::TranslationFaultLevel3
    } as u64;
    let (class, specific) = match access {
        Access::Execute => (ESR_EL1::EC::Value::InstrAbortLowerEL, status),
        Access::Read => (ESR_EL1::EC::Value::DataAbortLowerEL, status),
        Access::Write => (
            ESR_EL1::EC::Value::DataAbortLowerEL,
            WRITE_NOT_READ | status,
        ),
    };
    Exception {
        syndrome: encode_syndrome(class, specific),
        fault_address: Some(address),
    }
}

/// The exception of an exit no handler is given, at `pc`: a stub that ended is an SError, the interface having no
/// other way to say the guest's process is gone.
fn to_exception(exit: Exit, pc: u64) -> Exception {
    let (class, specific, fault_address) = match exit {
        Exit::Breakpoint { comment } => (ESR_EL1::EC::Value::Brk64, u64::from(comment), None),
        Exit::IllegalInstruction => (ESR_EL1::EC::Value::Unknown, 0, None),
        Exit::AlignmentFault { address } if address == pc => {
            (ESR_EL1::EC::Value::PCAlignmentFault, 0, Some(address))
        }
        Exit::AlignmentFault { .. } => (ESR_EL1::EC::Value::SPAlignmentFault, 0, None),
        Exit::MemoryFaultWithoutRegisters
        | Exit::Syscall(_)
        | Exit::MemoryFault { .. }
        | Exit::Stop => (ESR_EL1::EC::Value::SError, 0, None),
    };
    Exception {
        syndrome: encode_syndrome(class, specific),
        fault_address,
    }
}
