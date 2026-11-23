//! Form P's executor: each guest memory a stub, its private pages kept by one owner each and lent after a fork until
//! a side touches them, its shared blocks memfds every stub that maps them shares (plan 3.5).

use std::ops::Range;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;
use std::time::Duration;

use rangemap::RangeMap;
use rustix::fs::{FallocateFlags, MemfdFlags, SeekFrom};
use rustix::io::Errno;
use rustix::mm::ProtFlags;
use rustix::process::Pid;
use wcontainers_abi::{
    Access, AddressSpace, Cpu, CpuError, LOWEST_ADDRESS, MemoryId, PAGE_SIZE, Permissions,
    RegisterBlock, SharedId, ThreadId,
};
use zerocopy::IntoBytes;

use crate::host::HostProcessHost;
use crate::layout::OUR_AREA;
use crate::pages::control::MAX_COMMANDS;
use crate::pages::{Command, Resume};
use crate::stub::{Stub, TurnEnd};
use crate::windows::FileKey;

/// The guest's addresses.
const GUEST: Range<u64> = LOWEST_ADDRESS..OUR_AREA as u64;

/// Where a private page's bytes are, when they are not simply this memory's own.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PageState {
    /// Still the owner's page at the same offset; never accessible in this memory's view.
    Borrowed(MemoryId),
    /// Owned and borrowed by these memories, sorted and never none; never writable in this memory's view.
    Lent(Vec<MemoryId>),
}

/// One guest memory.
struct Memory {
    stub: Stub,
    /// What the kernel last allowed, by range; a range not here allows nothing.
    permissions: RangeMap<u64, Permissions>,
    /// Private pages borrowed or lent; every other private page is this memory's own and lent to none.
    pages: RangeMap<u64, PageState>,
    /// Ranges that show a shared block: the block, and its offset less the address.
    shared: RangeMap<u64, (SharedId, u64)>,
    /// Changes to the view, applied by the next turn of one of its threads.
    queue: Vec<Command>,
    /// The memfd of each `MapShared` in the queue, in order.
    queued_blocks: Vec<Arc<OwnedFd>>,
    /// The stub's thread indices in use: every guest thread's and the spare's.
    indices: Vec<usize>,
    /// An index whose host thread no guest thread holds: the stub's first, or the last one dropped.
    spare: Option<usize>,
    /// Dropped by the kernel, its stub ended, kept only for the pages others still borrow from it.
    retired: bool,
}

/// One block of shared pages.
struct Block {
    file: Arc<OwnedFd>,
    key: FileKey,
    length: u64,
}

/// One guest thread: a host thread of its memory's stub.
#[derive(Clone, Copy)]
struct Thread {
    memory: MemoryId,
    index: usize,
}

/// The executor's counts of its slow and notable paths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counters {
    /// Pages copied from an owner to a borrower ("pages copied").
    pub pages_copied: u64,
    /// Faults settled by the executor, not given to the kernel ("faults resolved").
    pub faults_resolved: u64,
    /// Queues applied by a round trip of their own ("mapping applied at once").
    pub applied_at_once: u64,
    /// Turns that only applied a page of commands ("mapping commands past one page").
    pub commands_past_one_page: u64,
    /// Thread requests refused for want of an index ("guest threads per process exhausted").
    pub threads_exhausted: u64,
    /// Memories dropped while lending pages, kept as files until no page is borrowed ("owner ended while lending").
    pub retired_owners: u64,
    /// Stops sent when a slice ran out, and again while a thread did not come back.
    pub stops_sent: u64,
    /// Stubs killed because a thread did not come back 10 s after its stop ("thread ignored its stop").
    pub stops_ignored: u64,
}

/// How a guest fault the executor was told of is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultOwner {
    /// The kernel's permissions allow the access: it is put right for the thread's next resume.
    Executor,
    /// The kernel's permissions refuse it: the kernel's fault.
    Guest,
}

/// What a turn runs, standing in for guest code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    /// Loads the word at the address.
    Load(u64),
    /// Stores a word at the address.
    Store(u64, u64),
    /// Runs until a stop ends the turn.
    Spin,
}

/// How a touch ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touched {
    /// The load's word.
    Loaded(u64),
    /// The store was done.
    Stored,
    /// The kernel's permissions refused the access.
    Fault {
        /// The address the host reported.
        address: u64,
        /// What the access was.
        access: Access,
    },
    /// The stop signal ended the turn.
    Stopped,
    /// The stub's process ended.
    StubEnded,
    /// The stub reported an exit it could not have made.
    Forged,
}

/// Form P's executor for one container, made from the process-wide host.
pub struct HostProcessExecutor {
    host: Arc<HostProcessHost>,
    memories: Vec<Option<Memory>>,
    threads: Vec<Option<Thread>>,
    blocks: Vec<Option<Block>>,
    counters: Counters,
}

impl HostProcessExecutor {
    /// An executor with no memories, forking its stubs through `host`.
    pub fn new(host: Arc<HostProcessHost>) -> Self {
        Self {
            host,
            memories: Vec::new(),
            threads: Vec::new(),
            blocks: Vec::new(),
            counters: Counters::default(),
        }
    }

    /// The counts so far.
    pub fn counters(&self) -> Counters {
        self.counters
    }

    /// The process id of a memory's stub.
    pub fn stub_process(&self, memory: MemoryId) -> Pid {
        self.memory(memory).stub.process()
    }

    /// Where a thread's stops landed, as its stub counts them (untrusted).
    pub fn stops_landed(&self, thread: ThreadId) -> crate::stub::StopsLanded {
        let Thread { memory, index } = self.thread(thread);
        self.memory(memory).stub.stops_landed(index)
    }

    /// Runs a turn on a thread, standing in for guest code, for at most `slice` before it is stopped, settling every
    /// fault that is the executor's.
    pub fn touch(
        &mut self,
        thread: ThreadId,
        touch: Touch,
        slice: Option<Duration>,
    ) -> Result<Touched, CpuError> {
        let Thread { memory, index } = self.thread(thread);
        let (address, value, resume, access) = match touch {
            Touch::Load(address) => (address, 0, Resume::Load, Access::Read),
            Touch::Store(address, value) => (address, value, Resume::Store, Access::Write),
            Touch::Spin => (0, 0, Resume::Spin, Access::Read),
        };
        loop {
            let (commands, blocks) = self.take_queue(memory);
            let end = self.run_turn(
                memory,
                index,
                &commands,
                &blocks,
                Some((address as usize, value, resume, slice)),
            )?;
            return Ok(match end {
                TurnEnd::Loaded { value } if resume == Resume::Load => Touched::Loaded(value),
                TurnEnd::Done { .. } if resume == Resume::Store => Touched::Stored,
                TurnEnd::Fault { address, .. } => {
                    match self.resolve_fault(memory, address, access)? {
                        FaultOwner::Executor => continue,
                        FaultOwner::Guest => Touched::Fault { address, access },
                    }
                }
                TurnEnd::Stopped { .. } => Touched::Stopped,
                TurnEnd::StubEnded => Touched::StubEnded,
                TurnEnd::CommandRefused { .. } => return Err(CpuError::OutOfMemory),
                TurnEnd::Loaded { .. }
                | TurnEnd::Done { .. }
                | TurnEnd::BadTurn
                | TurnEnd::Forged => Touched::Forged,
            });
        }
    }

    /// Settles a fault a thread of `id` reported, from the executor's own records alone, so a forged report does only
    /// what a real access would: where the kernel allows the access, the page is copied from its owner or given to a
    /// borrower as the access needs, and the widest range around it whose view can now be what the kernel allows gets
    /// it at the thread's next resume.
    pub fn resolve_fault(
        &mut self,
        id: MemoryId,
        address: u64,
        access: Access,
    ) -> Result<FaultOwner, CpuError> {
        if !GUEST.contains(&address) {
            return Ok(FaultOwner::Guest);
        }
        let page = address & !(PAGE_SIZE - 1);
        let Some((granted_range, granted)) = self
            .memory(id)
            .permissions
            .get_key_value(&page)
            .map(|(range, granted)| (range.clone(), *granted))
        else {
            return Ok(FaultOwner::Guest);
        };
        let allowed = match access {
            // A host page that allows writes allows reads, on both architectures.
            Access::Read => granted.intersects(Permissions::READ | Permissions::WRITE),
            Access::Write => granted.contains(Permissions::WRITE),
            // Guest code runs only translated, never from its own pages.
            Access::Execute => false,
        };
        if !allowed {
            return Ok(FaultOwner::Guest);
        }
        let this_page = page..page + PAGE_SIZE;
        self.unborrow(id, this_page.clone(), true)?;
        if access == Access::Write {
            self.unlend(id, this_page)?;
        }
        let memory = self.memory(id);
        let (run, view) = match memory.pages.get_key_value(&page) {
            Some((lent, _)) => (
                lent.start.max(granted_range.start)..lent.end.min(granted_range.end),
                host_protection(granted) - ProtFlags::WRITE,
            ),
            None => {
                let start = if granted_range.start < page {
                    memory
                        .pages
                        .overlapping(granted_range.start..page)
                        .next_back()
                        .map_or(granted_range.start, |(range, _)| range.end)
                } else {
                    page
                };
                let end = if page + PAGE_SIZE < granted_range.end {
                    memory
                        .pages
                        .overlapping(page + PAGE_SIZE..granted_range.end)
                        .next()
                        .map_or(granted_range.end, |(range, _)| range.start)
                } else {
                    page + PAGE_SIZE
                };
                (start..end, host_protection(granted))
            }
        };
        self.memory_mut(id).queue.push(protect(run, view));
        self.counters.faults_resolved += 1;
        Ok(FaultOwner::Executor)
    }

    fn memory(&self, id: MemoryId) -> &Memory {
        self.memories[id.0 as usize]
            .as_ref()
            .expect("a memory the kernel holds")
    }

    fn memory_mut(&mut self, id: MemoryId) -> &mut Memory {
        self.memories[id.0 as usize]
            .as_mut()
            .expect("a memory the kernel holds")
    }

    fn thread(&self, thread: ThreadId) -> Thread {
        self.threads[thread.0 as usize].expect("a thread the kernel holds")
    }

    fn block(&self, shared: SharedId) -> &Block {
        self.blocks[shared.0 as usize]
            .as_ref()
            .expect("a block the kernel holds")
    }

    /// Closes a memory that nothing uses: its views first, then its files.
    fn free(&mut self, id: MemoryId) {
        let memory = self.memories[id.0 as usize]
            .take()
            .expect("a memory the kernel holds");
        self.host.windows().forget(memory.stub.memory_key());
    }

    /// The most a range's view may allow: what is granted, less write where a page is lent, less everything where
    /// one is borrowed.
    fn view_bound(&self, id: MemoryId, range: Range<u64>, granted: Permissions) -> ProtFlags {
        let mut view = host_protection(granted);
        for (_, state) in self.memory(id).pages.overlapping(range) {
            match state {
                PageState::Borrowed(_) => return ProtFlags::empty(),
                PageState::Lent(_) => view.remove(ProtFlags::WRITE),
            }
        }
        view
    }

    /// Queues, after a range was mapped anew with no access, what each part of it may allow.
    fn queue_view(&mut self, id: MemoryId, range: Range<u64>) {
        let granted = pieces(&self.memory(id).permissions, range);
        for (piece, granted) in granted {
            if let Some(granted) = granted {
                let view = self.view_bound(id, piece.clone(), granted);
                self.memory_mut(id).queue.push(protect(piece, view));
            }
        }
    }

    /// Ends the sharing of every shared range inside `range`, queuing the memory's own memfd over each with no access.
    fn unshare(&mut self, id: MemoryId, range: Range<u64>) -> Vec<(Range<u64>, (SharedId, u64))> {
        let memory = self.memory_mut(id);
        let ended: Vec<_> = pieces(&memory.shared, range.clone())
            .into_iter()
            .filter_map(|(piece, shared)| shared.map(|shared| (piece, shared)))
            .collect();
        memory.shared.remove(range);
        for (piece, _) in &ended {
            memory.queue.push(Command::Map {
                address: piece.start as usize,
                length: (piece.end - piece.start) as usize,
                permissions: ProtFlags::empty(),
                offset: piece.start as usize,
            });
        }
        ended
    }

    fn take_queue(&mut self, id: MemoryId) -> (Vec<Command>, Vec<Arc<OwnedFd>>) {
        let memory = self.memory_mut(id);
        (
            core::mem::take(&mut memory.queue),
            core::mem::take(&mut memory.queued_blocks),
        )
    }

    /// Gives a thread turns that apply `commands`, a page at a time, each `MapShared`'s memfd sent just before its
    /// turn, the last with the touch if there is one; how the last ended, or the first that did not end `Done`.
    fn run_turn(
        &mut self,
        id: MemoryId,
        index: usize,
        commands: &[Command],
        blocks: &[Arc<OwnedFd>],
        touch: Option<(usize, u64, Resume, Option<Duration>)>,
    ) -> Result<TurnEnd, CpuError> {
        let rounds = commands.len().div_ceil(MAX_COMMANDS).max(1);
        let mut blocks = blocks.iter();
        for round in 0..rounds {
            let chunk =
                &commands[round * MAX_COMMANDS..commands.len().min((round + 1) * MAX_COMMANDS)];
            let stub = &self.memory(id).stub;
            for command in chunk {
                if let Command::MapShared { .. } = command {
                    let block = blocks.next().expect("a memfd for every queued block");
                    stub.send_block(block.as_fd()).map_err(host_error)?;
                }
            }
            let last = round + 1 == rounds;
            let (address, value, resume, slice) = match touch {
                Some(touch) if last => touch,
                _ => (0, 0, Resume::Ping, None),
            };
            let turn = stub
                .begin_touch(index, chunk, address, value, resume)
                .map_err(host_error)?;
            let mut stops = 0;
            let waited = stub
                .wait_sliced(index, turn, slice, &mut stops)
                .map_err(host_error);
            if matches!(waited, Ok(None)) {
                let _ = self.host.kill(stub);
            }
            self.counters.stops_sent += stops;
            let end = match waited? {
                Some(end) => end,
                None => {
                    self.counters.stops_ignored += 1;
                    eprintln!(
                        "wcontainers warning: thread ignored its stop, its process killed, {} so far",
                        self.counters.stops_ignored
                    );
                    TurnEnd::StubEnded
                }
            };
            if last {
                return Ok(end);
            }
            self.counters.commands_past_one_page += 1;
            if !matches!(end, TurnEnd::Done { .. }) {
                return Ok(end);
            }
        }
        unreachable!("at least one round")
    }

    /// Applies a memory's queue now, by a round trip of one of its waiting threads.
    fn apply_now(&mut self, id: MemoryId) -> Result<(), CpuError> {
        self.counters.applied_at_once += 1;
        let index = self.memory(id).indices[0];
        let (commands, blocks) = self.take_queue(id);
        match self.run_turn(id, index, &commands, &blocks, None)? {
            TurnEnd::Done { .. } => Ok(()),
            _ => Err(CpuError::OutOfMemory),
        }
    }

    fn add_borrower(&mut self, owner: MemoryId, range: Range<u64>, borrower: MemoryId) {
        let pages = &mut self.memory_mut(owner).pages;
        for (piece, state) in pieces(pages, range) {
            let borrowers = match state {
                None => vec![borrower],
                Some(PageState::Lent(mut borrowers)) => {
                    if let Err(at) = borrowers.binary_search(&borrower) {
                        borrowers.insert(at, borrower);
                    }
                    borrowers
                }
                Some(PageState::Borrowed(_)) => continue,
            };
            pages.insert(piece, PageState::Lent(borrowers));
        }
    }

    /// Takes a borrower out of an owner's lent pages; a retired owner frees what no one borrows any more.
    fn remove_borrower(&mut self, owner: MemoryId, range: Range<u64>, borrower: MemoryId) {
        let memory = self.memory_mut(owner);
        let mut unlent = Vec::new();
        for (piece, state) in pieces(&memory.pages, range) {
            if let Some(PageState::Lent(mut borrowers)) = state {
                borrowers.retain(|held| *held != borrower);
                if borrowers.is_empty() {
                    memory.pages.remove(piece.clone());
                    unlent.push(piece);
                } else {
                    memory.pages.insert(piece, PageState::Lent(borrowers));
                }
            }
        }
        if memory.retired {
            for piece in unlent {
                // A failure leaves bytes no one reads until the file closes.
                let _ = punch(memory.stub.memory(), piece);
            }
            if memory.pages.is_empty() {
                self.free(owner);
            }
        }
    }

    /// Makes every borrowed page of a range this memory's own: copied from its owner, or left a hole where its bytes
    /// are about to be zeroed or dropped.
    fn unborrow(&mut self, id: MemoryId, range: Range<u64>, copy: bool) -> Result<(), CpuError> {
        for (piece, state) in pieces(&self.memory(id).pages, range) {
            if let Some(PageState::Borrowed(owner)) = state {
                if copy {
                    self.copy(owner, id, piece.clone())?;
                }
                self.memory_mut(id).pages.remove(piece.clone());
                self.remove_borrower(owner, piece, id);
            }
        }
        Ok(())
    }

    /// Gives every lent page of a range to its first borrower, which becomes its owner and lends it to the rest, so
    /// this memory's page may change: one copy whatever the number of borrowers.
    fn unlend(&mut self, id: MemoryId, range: Range<u64>) -> Result<(), CpuError> {
        for (piece, state) in pieces(&self.memory(id).pages, range) {
            if let Some(PageState::Lent(borrowers)) = state {
                let (&heir, rest) = borrowers.split_first().expect("a lent page has a borrower");
                self.copy(id, heir, piece.clone())?;
                let heir_pages = &mut self.memory_mut(heir).pages;
                if rest.is_empty() {
                    heir_pages.remove(piece.clone());
                } else {
                    heir_pages.insert(piece.clone(), PageState::Lent(rest.to_vec()));
                }
                for &other in rest {
                    self.memory_mut(other)
                        .pages
                        .insert(piece.clone(), PageState::Borrowed(heir));
                }
                self.memory_mut(id).pages.remove(piece);
            }
        }
        Ok(())
    }

    /// Copies whole pages from one memory's memfd to another's at the same offsets.
    fn copy(&mut self, from: MemoryId, to: MemoryId, range: Range<u64>) -> Result<(), CpuError> {
        let (from, to) = (&self.memory(from).stub, &self.memory(to).stub);
        let mut windows = self.host.windows();
        let mut page = [0; PAGE_SIZE as usize];
        for at in range.clone().step_by(PAGE_SIZE as usize) {
            windows
                .read(from.memory(), from.memory_key(), at, &mut page)
                .map_err(host_error)?;
            windows
                .write(to.memory(), to.memory_key(), at, &page)
                .map_err(host_error)?;
        }
        self.counters.pages_copied += (range.end - range.start) / PAGE_SIZE;
        Ok(())
    }

    /// Reads private bytes, whoever owns their pages.
    fn read_private(
        &self,
        id: MemoryId,
        range: Range<u64>,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        let memory = self.memory(id);
        let mut windows = self.host.windows();
        for (piece, state) in pieces(&memory.pages, range.clone()) {
            let from = match state {
                Some(PageState::Borrowed(owner)) => &self.memory(owner).stub,
                _ => &memory.stub,
            };
            let at = (piece.start - range.start) as usize..(piece.end - range.start) as usize;
            windows
                .read(from.memory(), from.memory_key(), piece.start, &mut into[at])
                .map_err(host_error)?;
        }
        Ok(())
    }
}

impl Cpu for HostProcessExecutor {
    fn address_space(&self) -> AddressSpace {
        AddressSpace { end: GUEST.end }
    }

    /// A new stub whose whole guest area is its memfd, mapped with no access.
    fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        let stub = self.host.fork_stub().map_err(|_| CpuError::OutOfMemory)?;
        let memory = Memory {
            stub,
            permissions: RangeMap::new(),
            pages: RangeMap::new(),
            shared: RangeMap::new(),
            queue: vec![Command::Map {
                address: GUEST.start as usize,
                length: (GUEST.end - GUEST.start) as usize,
                permissions: ProtFlags::empty(),
                offset: GUEST.start as usize,
            }],
            queued_blocks: Vec::new(),
            indices: vec![0],
            spare: Some(0),
            retired: false,
        };
        Ok(MemoryId(place(&mut self.memories, memory) as u32))
    }

    /// The child borrows every private page the source owns or borrows and maps its shared ranges, and the source's
    /// writable ranges lose write in its view before this returns, which is the fork's instant.
    fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
        let child = self.memory_new()?;
        let borrowed: Vec<_> = self
            .memory(source)
            .pages
            .iter()
            .filter_map(|(range, state)| match state {
                PageState::Borrowed(owner) => Some((range.clone(), *owner)),
                PageState::Lent(_) => None,
            })
            .collect();
        for (range, owner) in borrowed {
            self.add_borrower(owner, range.clone(), child);
            self.memory_mut(child)
                .pages
                .insert(range, PageState::Borrowed(owner));
        }
        let owned = data_extents(self.memory(source).stub.memory()).map_err(host_error)?;
        for extent in owned {
            for (piece, state) in pieces(&self.memory(source).pages, extent) {
                if !matches!(state, Some(PageState::Borrowed(_))) {
                    self.add_borrower(source, piece.clone(), child);
                    self.memory_mut(child)
                        .pages
                        .insert(piece, PageState::Borrowed(source));
                }
            }
        }
        let permissions = self.memory(source).permissions.clone();
        let shared = self.memory(source).shared.clone();
        let mut protect_source = Vec::new();
        for (range, granted) in permissions.iter() {
            if granted.contains(Permissions::WRITE) {
                let view = self.view_bound(source, range.clone(), *granted) - ProtFlags::WRITE;
                protect_source.push(protect(range.clone(), view));
            }
        }
        let mut blocks = Vec::new();
        for (range, (block, delta)) in shared.iter() {
            blocks.push((range.clone(), self.block(*block).file.clone(), *delta));
        }
        let memory = self.memory_mut(child);
        memory.permissions = permissions;
        memory.shared = shared;
        for (range, file, delta) in blocks {
            let memory = self.memory_mut(child);
            memory.queue.push(Command::MapShared {
                address: range.start as usize,
                length: (range.end - range.start) as usize,
                permissions: ProtFlags::empty(),
                offset: range.start.wrapping_add(delta) as usize,
            });
            memory.queued_blocks.push(file);
            self.queue_view(child, range);
        }
        self.memory_mut(source).queue.extend(protect_source);
        if let Err(error) = self.apply_now(source) {
            self.memory_drop(child);
            return Err(error);
        }
        Ok(child)
    }

    /// Ends a memory's stub; one that lends pages keeps its memfd until no page is borrowed from it.
    fn memory_drop(&mut self, id: MemoryId) {
        let _ = self.unborrow(id, GUEST, false);
        let memory = self.memory_mut(id);
        memory.retired = true;
        memory.shared = RangeMap::new();
        let (last, others) = memory
            .indices
            .split_last()
            .expect("a stub always has a thread");
        for &index in others {
            let _ = memory.stub.end_thread(index);
        }
        let last = *last;
        memory.indices.clear();
        memory.spare = None;
        let stub = &self.memory(id).stub;
        if stub.end(last).is_err() {
            let _ = self.host.kill(stub);
        }
        let _ = self.host.reap(stub);
        if self.memory(id).pages.is_empty() {
            self.free(id);
        } else {
            self.counters.retired_owners += 1;
        }
    }

    fn read(&mut self, id: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        let range = checked(address, into.len() as u64)?;
        for (piece, shared) in pieces(&self.memory(id).shared, range.clone()) {
            let at = (piece.start - address) as usize..(piece.end - address) as usize;
            match shared {
                Some((block, delta)) => {
                    let block = self.block(block);
                    self.host
                        .windows()
                        .read(
                            block.file.as_fd(),
                            block.key,
                            piece.start.wrapping_add(delta),
                            &mut into[at],
                        )
                        .map_err(host_error)?;
                }
                None => self.read_private(id, piece, &mut into[at])?,
            }
        }
        Ok(())
    }

    /// Private pages are first made the memory's own and lent to none; shared ones are written in their block.
    fn write(&mut self, id: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        let range = checked(address, from.len() as u64)?;
        for (piece, shared) in pieces(&self.memory(id).shared, range) {
            let bytes = &from[(piece.start - address) as usize..(piece.end - address) as usize];
            let (file, key, offset) = match shared {
                Some((block, delta)) => {
                    let block = self.block(block);
                    (
                        block.file.clone(),
                        block.key,
                        piece.start.wrapping_add(delta),
                    )
                }
                None => {
                    let pages = page_cover(&piece);
                    self.unborrow(id, pages.clone(), true)?;
                    self.unlend(id, pages)?;
                    let stub = &self.memory(id).stub;
                    self.host
                        .windows()
                        .write(stub.memory(), stub.memory_key(), piece.start, bytes)
                        .map_err(host_error)?;
                    continue;
                }
            };
            self.host
                .windows()
                .write(file.as_fd(), key, offset, bytes)
                .map_err(host_error)?;
        }
        Ok(())
    }

    /// Ends any sharing in the range, then punches it out of the memfd, its lent pages first given to a borrower.
    fn zero(&mut self, id: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        let range = checked(address, length)?;
        if !self.unshare(id, range.clone()).is_empty() {
            self.queue_view(id, range.clone());
        }
        self.unborrow(id, range.clone(), false)?;
        self.unlend(id, range.clone())?;
        punch(self.memory(id).stub.memory(), range).map_err(host_error)
    }

    /// Records what a range allows and queues its view's change, at most what the kernel allows.
    fn set_permissions(
        &mut self,
        id: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError> {
        let range = checked(address, length)?;
        let view = self.view_bound(id, range.clone(), permissions);
        let memory = self.memory_mut(id);
        if permissions.is_empty() {
            memory.permissions.remove(range.clone());
        } else {
            memory.permissions.insert(range.clone(), permissions);
        }
        memory.queue.push(protect(range, view));
        Ok(())
    }

    /// Nothing is translated here yet, so there is nothing to drop.
    fn code_changed(&mut self, _memory: MemoryId, _address: u64, _length: u64, _starts: &[u64]) {}

    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        let file = rustix::fs::memfd_create("wcontainers shared block", MemfdFlags::CLOEXEC)
            .map_err(host_error)?;
        rustix::fs::ftruncate(&file, length).map_err(host_error)?;
        let block = Block {
            file: Arc::new(file),
            key: FileKey::new(),
            length,
        };
        Ok(SharedId(place(&mut self.blocks, block) as u32))
    }

    /// The range's private pages are given up (a later unmapping copies the block's bytes back), and the block's
    /// memfd is mapped over it with what each part allows.
    fn shared_map(
        &mut self,
        shared: SharedId,
        offset: u64,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        let range = checked(address, length)?;
        let block = self.block(shared);
        if offset
            .checked_add(length)
            .is_none_or(|end| end > block.length)
        {
            return Err(CpuError::BadAddress);
        }
        let file = block.file.clone();
        self.unshare(memory, range.clone());
        self.unborrow(memory, range.clone(), false)?;
        self.unlend(memory, range.clone())?;
        punch(self.memory(memory).stub.memory(), range.clone()).map_err(host_error)?;
        let target = self.memory_mut(memory);
        target
            .shared
            .insert(range.clone(), (shared, offset.wrapping_sub(address)));
        target.queue.push(Command::MapShared {
            address: address as usize,
            length: length as usize,
            permissions: ProtFlags::empty(),
            offset: offset as usize,
        });
        target.queued_blocks.push(file);
        self.queue_view(memory, range);
        Ok(())
    }

    /// The block's bytes become the memory's own private copy.
    fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        let range = checked(address, length)?;
        for (piece, (block, delta)) in self.unshare(memory, range.clone()) {
            let mut bytes = vec![0; (piece.end - piece.start) as usize];
            let block = self.block(block);
            let stub = &self.memory(memory).stub;
            let mut windows = self.host.windows();
            windows
                .read(
                    block.file.as_fd(),
                    block.key,
                    piece.start.wrapping_add(delta),
                    &mut bytes,
                )
                .map_err(host_error)?;
            windows
                .write(stub.memory(), stub.memory_key(), piece.start, &bytes)
                .map_err(host_error)?;
        }
        self.queue_view(memory, range);
        Ok(())
    }

    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        let block = self.block(shared);
        if offset
            .checked_add(into.len() as u64)
            .is_none_or(|end| end > block.length)
        {
            return Err(CpuError::BadAddress);
        }
        self.host
            .windows()
            .read(block.file.as_fd(), block.key, offset, into)
            .map_err(host_error)
    }

    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError> {
        let block = self.block(shared);
        if offset
            .checked_add(from.len() as u64)
            .is_none_or(|end| end > block.length)
        {
            return Err(CpuError::BadAddress);
        }
        self.host
            .windows()
            .write(block.file.as_fd(), block.key, offset, from)
            .map_err(host_error)
    }

    /// Its views go; its memfd closes once no queued map holds it.
    fn shared_drop(&mut self, shared: SharedId) {
        let block = self.blocks[shared.0 as usize]
            .take()
            .expect("a block the kernel holds");
        self.host.windows().forget(block.key);
    }

    /// The memory's spare host thread, or a new one made by a turn of one of its threads.
    fn thread_new(&mut self, id: MemoryId) -> Result<ThreadId, CpuError> {
        let index = match self.memory_mut(id).spare.take() {
            Some(index) => index,
            None => {
                let memory = self.memory_mut(id);
                let Some(command) = memory.stub.new_thread() else {
                    self.counters.threads_exhausted += 1;
                    eprintln!(
                        "wcontainers warning: guest threads per process exhausted, {} so far",
                        self.counters.threads_exhausted
                    );
                    return Err(CpuError::OutOfMemory);
                };
                let Command::NewThread { thread: index, .. } = command else {
                    unreachable!("new_thread gives a thread's command")
                };
                let maker = memory.indices[0];
                let (mut commands, blocks) = self.take_queue(id);
                commands.push(command);
                if !matches!(
                    self.run_turn(id, maker, &commands, &blocks, None),
                    Ok(TurnEnd::Done { .. })
                ) {
                    self.memory_mut(id).stub.release_thread(index);
                    return Err(CpuError::OutOfMemory);
                }
                self.memory_mut(id).indices.push(index);
                index
            }
        };
        self.memory(id)
            .stub
            .write_registers(index, RegisterBlock::ZERO.as_bytes());
        let thread = Thread { memory: id, index };
        Ok(ThreadId(place(&mut self.threads, thread) as u32))
    }

    /// Ends a guest thread's host thread, or keeps it as its memory's spare if it has none.
    fn thread_drop(&mut self, thread: ThreadId) {
        let Thread { memory, index } = self.threads[thread.0 as usize]
            .take()
            .expect("a thread the kernel holds");
        let memory = self.memory_mut(memory);
        if memory.spare.is_none() {
            memory.spare = Some(index);
        } else {
            let _ = memory.stub.end_thread(index);
            memory.indices.retain(|held| *held != index);
        }
    }

    /// As its register page holds them.
    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        let Thread { memory, index } = self.thread(thread);
        self.memory(memory)
            .stub
            .read_registers(index, into.as_mut_bytes());
    }

    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        let Thread { memory, index } = self.thread(thread);
        self.memory(memory)
            .stub
            .write_registers(index, from.as_bytes());
    }
}

impl Drop for HostProcessExecutor {
    fn drop(&mut self) {
        for slot in 0..self.threads.len() {
            if self.threads[slot].is_some() {
                self.thread_drop(ThreadId(slot as u32));
            }
        }
        for slot in 0..self.memories.len() {
            if self.memories[slot]
                .as_ref()
                .is_some_and(|memory| !memory.retired)
            {
                self.memory_drop(MemoryId(slot as u32));
            }
        }
        for slot in 0..self.memories.len() {
            if self.memories[slot].is_some() {
                self.free(MemoryId(slot as u32));
            }
        }
        for slot in 0..self.blocks.len() {
            if self.blocks[slot].is_some() {
                self.shared_drop(SharedId(slot as u32));
            }
        }
    }
}

/// Puts a value in the first free slot; its index.
fn place<T>(slots: &mut Vec<Option<T>>, value: T) -> usize {
    let index = slots.iter().position(Option::is_none).unwrap_or_else(|| {
        slots.push(None);
        slots.len() - 1
    });
    slots[index] = Some(value);
    index
}

/// The pieces of `range` in order, each with the value the map gives it, or none.
fn pieces<V: Clone + Eq>(
    map: &RangeMap<u64, V>,
    range: Range<u64>,
) -> Vec<(Range<u64>, Option<V>)> {
    let mut out = Vec::new();
    let mut at = range.start;
    for (held, value) in map.overlapping(range.clone()) {
        let start = held.start.max(range.start);
        if start > at {
            out.push((at..start, None));
        }
        let end = held.end.min(range.end);
        out.push((start..end, Some(value.clone())));
        at = end;
    }
    if at < range.end {
        out.push((at..range.end, None));
    }
    out
}

/// The extents of a memfd's guest area that hold pages, rounded out to whole pages.
fn data_extents(file: BorrowedFd<'_>) -> Result<Vec<Range<u64>>, Errno> {
    let mut extents = Vec::new();
    let mut at = GUEST.start;
    while at < GUEST.end {
        let start = match rustix::fs::seek(file, SeekFrom::Data(at)) {
            Ok(start) => start,
            Err(Errno::NXIO) => break,
            Err(other) => return Err(other),
        };
        if start >= GUEST.end {
            break;
        }
        let end = rustix::fs::seek(file, SeekFrom::Hole(start))?.min(GUEST.end);
        extents.push(page_cover(&(start..end)));
        at = end;
    }
    Ok(extents)
}

/// Frees a range of a memfd, which then reads as zeros.
fn punch(file: BorrowedFd<'_>, range: Range<u64>) -> Result<(), Errno> {
    rustix::fs::fallocate(
        file,
        FallocateFlags::PUNCH_HOLE | FallocateFlags::KEEP_SIZE,
        range.start,
        range.end - range.start,
    )
}

/// A byte range of the guest area, or `BadAddress`.
fn checked(address: u64, length: u64) -> Result<Range<u64>, CpuError> {
    match address.checked_add(length) {
        Some(end) if address >= GUEST.start && end <= GUEST.end => Ok(address..end),
        _ => Err(CpuError::BadAddress),
    }
}

/// The whole pages a byte range touches.
fn page_cover(range: &Range<u64>) -> Range<u64> {
    range.start & !(PAGE_SIZE - 1)..range.end.next_multiple_of(PAGE_SIZE)
}

fn protect(range: Range<u64>, permissions: ProtFlags) -> Command {
    Command::Protect {
        address: range.start as usize,
        length: (range.end - range.start) as usize,
        permissions,
    }
}

/// What a view gives for what the kernel allows: guest code never runs from its own pages.
fn host_protection(granted: Permissions) -> ProtFlags {
    let mut view = ProtFlags::empty();
    view.set(ProtFlags::READ, granted.contains(Permissions::READ));
    view.set(ProtFlags::WRITE, granted.contains(Permissions::WRITE));
    view
}

/// A host failure as the kernel sees it.
fn host_error(_: Errno) -> CpuError {
    CpuError::OutOfMemory
}
