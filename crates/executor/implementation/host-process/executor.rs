//! Form P's executor: each guest memory a stub, its private pages kept by one owner each and lent after a fork until
//! a side touches them, its shared blocks memfds every stub that maps them shares (plan 3.5).

use std::ops::Range;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rangemap::RangeMap;
use rustix::fs::{FallocateFlags, MemfdFlags, SeekFrom};
use rustix::io::Errno;
use rustix::mm::ProtFlags;
use rustix::process::Pid;
use wcontainers_abi::{
    Access, AddressSpace, Cpu, CpuError, KIB, LOWEST_ADDRESS, MemoryId, PAGE_SIZE, Permissions,
    RegisterBlock, SharedId, ThreadId,
};
use zerocopy::IntoBytes;

use crate::host::HostProcessHost;
use crate::layout::{OUR_AREA, OWN_MAPPINGS};
use crate::pages::control::MAX_COMMANDS;
use crate::pages::{Command, Resume};
use crate::stub::{Stub, TurnEnd};
use crate::view::View;
use crate::windows::FileKey;

#[cfg(target_arch = "aarch64")]
#[path = "execution.rs"]
mod execution;
#[cfg(target_arch = "aarch64")]
pub use execution::{HostProcess, HostProcessBytes, HostProcessInterrupter, HostProcessMemory};
#[cfg(target_arch = "aarch64")]
#[path = "native.rs"]
pub mod native;

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
    /// The stub's guest-area mappings once the queue is applied.
    view: View,
    /// The stub's thread indices in use: every guest thread's and the spare's.
    indices: Vec<usize>,
    /// An index whose host thread no guest thread holds: the stub's first, or the last one dropped.
    spare: Option<usize>,
    /// Dropped by the kernel, its stub ended, kept only for the pages others still borrow from it.
    retired: bool,
    /// Pages translated code was made from: never writable in the view, so the first write drops it (plan 3.4).
    #[cfg(target_arch = "aarch64")]
    code_pages: rangemap::RangeSet<u64>,
}

impl Memory {
    /// Queues a change to the view; a `MapShared` comes with its block and memfd.
    fn push(&mut self, command: Command, block: Option<(SharedId, Arc<OwnedFd>)>) {
        self.view
            .apply(&command, block.as_ref().map(|(shared, _)| *shared));
        self.queue.push(command);
        self.queued_blocks.extend(block.map(|(_, file)| file));
    }

    /// Queues a mapping of the code area's memfd, which comes with it.
    #[cfg(target_arch = "aarch64")]
    fn push_code(&mut self, command: Command, file: Arc<OwnedFd>) {
        self.view.apply(&command, None);
        self.queue.push(command);
        self.queued_blocks.push(file);
    }
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
    /// The CPU its host thread was last moved to.
    cpu: Option<usize>,
}

/// The pages a fault the executor settles copies or gives away at once: the aligned unit around the page, within the
/// range of one permission (plan 3.5's 2^u).
const COPY_UNIT: u64 = PAGE_SIZE;

/// The bytes of zeros written at a time where the host refuses to punch a hole.
const ZEROS: usize = 64 * KIB as usize;

/// How much of a runner's busy time it samples its CPU's share over.
const SAMPLE_EVERY: Duration = Duration::from_millis(10);

/// A CPU whose share falls below this fraction of the busy time (as numerator over denominator) is contended.
const CONTENDED_BELOW: (u128, u128) = (3, 4);

/// The CPU the runner thread holds, and its sampling window (plan 4.3).
struct Runner {
    cpu: usize,
    /// Time inside turns since the window began.
    busy: Duration,
    /// The runner thread's CPU clock when the window began.
    own: Duration,
    /// Each stub given a turn in the window, with its process's CPU clock before its first.
    stubs: Vec<(Pid, Duration)>,
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
    /// Moves of the runner to another CPU after its own was found contended.
    pub placement_moves: u64,
    /// Runners that place without samples, the CPU clocks being refused ("placement without samples").
    pub placement_without_samples: u64,
    /// Changes refused because the stub's host mappings could pass the host's limit ("host mappings exhausted").
    pub mappings_exhausted: u64,
    /// Forks that lent the whole guest area, the host not saying which pages hold data ("memory extents unknown").
    pub extents_unknown: u64,
    /// Ranges zeroed by writing zeros, the host refusing to punch them out ("hole punch refused").
    pub zeros_written: u64,
    /// Exits handed to the kernel other than a turn's end: system calls, faults and the rest (plan 7's k).
    pub kernel_entries: u64,
    /// Pieces loaded after one a thread stopped for, as an earlier memory starting from the same piece loaded them.
    pub pieces_loaded_ahead: u64,
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

/// What the last round of a turn does after its commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Then {
    /// A touch or a ping, standing in for guest code.
    Touch {
        address: usize,
        value: u64,
        resume: Resume,
    },
    /// Translated code from this address in the code area.
    #[cfg(target_arch = "aarch64")]
    Enter(usize),
}

/// Form P's executor for one container, made from the process-wide host.
pub struct HostProcessExecutor {
    host: Arc<HostProcessHost>,
    memories: Vec<Option<Memory>>,
    threads: Vec<Option<Thread>>,
    blocks: Vec<Option<Block>>,
    counters: Counters,
    /// Each memory's translated code, by memory.
    #[cfg(target_arch = "aarch64")]
    codes: Vec<Option<native::Code>>,
    /// Each guest thread's own state, by thread.
    #[cfg(target_arch = "aarch64")]
    guests: Vec<Option<native::Guest>>,
    #[cfg(target_arch = "aarch64")]
    translations: native::Translations,
    #[cfg(target_arch = "aarch64")]
    /// Each slow path's count and last detail.
    slow: std::collections::BTreeMap<&'static str, (u64, u64)>,
    #[cfg(target_arch = "aarch64")]
    /// The slow paths whose counts changed since they were last warned.
    unsent: std::collections::BTreeMap<&'static str, (u64, u64)>,
    /// The calling thread's place, from its first turn.
    runner: Option<Runner>,
    /// Whether the CPU clocks can be read.
    sampling: bool,
    /// Runs one instruction per entry: pieces of one instruction and nothing more, never linked or given a slot.
    #[cfg(target_arch = "aarch64")]
    single: bool,
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
            #[cfg(target_arch = "aarch64")]
            codes: Vec::new(),
            #[cfg(target_arch = "aarch64")]
            guests: Vec::new(),
            #[cfg(target_arch = "aarch64")]
            translations: native::Translations::default(),
            #[cfg(target_arch = "aarch64")]
            slow: std::collections::BTreeMap::new(),
            #[cfg(target_arch = "aarch64")]
            unsent: std::collections::BTreeMap::new(),
            runner: None,
            sampling: true,
            #[cfg(target_arch = "aarch64")]
            single: false,
        }
    }

    /// The CPU the runner holds, once it has run a turn.
    pub fn runner_cpu(&self) -> Option<usize> {
        self.runner.as_ref().map(|runner| runner.cpu)
    }

    /// The counts so far.
    pub fn counters(&self) -> Counters {
        self.counters
    }

    /// Each slow path of running translated code taken so far, with its count.
    #[cfg(target_arch = "aarch64")]
    pub fn slow_paths(&self) -> Vec<(&'static str, u64)> {
        self.slow
            .iter()
            .map(|(reason, (count, _))| (*reason, *count))
            .collect()
    }

    /// The process id of a memory's stub.
    pub fn stub_process(&self, memory: MemoryId) -> Pid {
        self.memory(memory).stub.process()
    }

    /// The host mappings a memory's stub holds once its queue is applied.
    pub fn host_mappings(&self, memory: MemoryId) -> usize {
        self.memory(memory).view.count() + OWN_MAPPINGS
    }

    /// Where a thread's stops landed, as its stub counts them (untrusted).
    pub fn stops_landed(&self, thread: ThreadId) -> crate::stub::StopsLanded {
        let Thread { memory, index, .. } = self.thread(thread);
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
        let Thread { memory, .. } = self.thread(thread);
        let (address, value, resume, access) = match touch {
            Touch::Load(address) => (address, 0, Resume::Load, Access::Read),
            Touch::Store(address, value) => (address, value, Resume::Store, Access::Write),
            Touch::Spin => (0, 0, Resume::Spin, Access::Read),
        };
        loop {
            let then = Then::Touch {
                address: address as usize,
                value,
                resume,
            };
            let end = self.turn_with(thread, then, slice)?;
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
                | TurnEnd::Called { .. }
                | TurnEnd::Interrupted { .. }
                | TurnEnd::Forged => Touched::Forged,
            });
        }
    }

    /// Gives a thread a turn that applies its memory's queue and then does `then`, for at most `slice` before it is
    /// stopped, its host thread placed on the runner's CPU first; how the turn ended.
    fn turn_with(
        &mut self,
        thread: ThreadId,
        then: Then,
        slice: Option<Duration>,
    ) -> Result<TurnEnd, CpuError> {
        let Thread { memory, index, .. } = self.thread(thread);
        let cpu = self.place_runner();
        let place = (self.thread(thread).cpu != Some(cpu)).then_some(Command::Place { cpu });
        let (mut commands, blocks) = self.take_queue(memory);
        commands.extend(place);
        let stub = self.memory(memory).stub.process();
        self.note_stub(stub);
        let started = Instant::now();
        let end = self.run_turn(memory, index, &commands, &blocks, Some((then, slice)))?;
        self.account(started.elapsed());
        if !matches!(end, TurnEnd::CommandRefused { .. } | TurnEnd::StubEnded) {
            self.threads[thread.0 as usize]
                .as_mut()
                .expect("a thread the kernel holds")
                .cpu = Some(cpu);
        }
        Ok(end)
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
        self.room(id, 1)?;
        #[cfg(target_arch = "aarch64")]
        if access == Access::Write && self.memory(id).code_pages.contains(&page) {
            self.code_page_written(id, page);
        }
        let unit = page & !(COPY_UNIT - 1);
        let unit = unit.max(granted_range.start)..(unit + COPY_UNIT).min(granted_range.end);
        self.unborrow(id, unit.clone(), true)?;
        if access == Access::Write {
            self.unlend(id, unit)?;
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
        // Pages translated code was made from keep their write protection.
        #[cfg(target_arch = "aarch64")]
        let run = {
            let code = &self.memory(id).code_pages;
            let start = code
                .overlapping(run.start..page)
                .next_back()
                .map_or(run.start, |held| held.end.max(run.start));
            let end = code
                .overlapping(page + PAGE_SIZE..run.end)
                .next()
                .map_or(run.end, |held| held.start.min(run.end));
            start..end
        };
        self.memory_mut(id).push(protect(run, view), None);
        self.counters.faults_resolved += 1;
        Ok(FaultOwner::Executor)
    }

    /// The runner's CPU: on its first turn the host's choice, to which it pins itself.
    fn place_runner(&mut self) -> usize {
        if let Some(runner) = &self.runner {
            return runner.cpu;
        }
        let cpu = self.pin(self.host.take_cpu());
        self.runner = Some(Runner {
            cpu,
            busy: Duration::ZERO,
            own: Duration::ZERO,
            stubs: Vec::new(),
        });
        self.start_window();
        cpu
    }

    /// Pins the calling thread to a CPU the host gave; if the process may no longer use it, reads its CPUs again and
    /// takes another. The CPU it holds.
    fn pin(&mut self, cpu: usize) -> usize {
        let pinned = |cpu: usize| {
            let mut set = rustix::thread::CpuSet::new();
            set.set(cpu);
            rustix::thread::sched_setaffinity(None, &set)
        };
        if pinned(cpu).is_ok() || self.host.reread_cpus().is_err() {
            return cpu;
        }
        self.host.release_cpu(cpu);
        let again = self.host.take_cpu();
        // Refused again, the thread runs where the host puts it, its CPU still counted as held.
        let _ = pinned(again);
        again
    }

    fn start_window(&mut self) {
        let own = thread_clock();
        if let Some(runner) = &mut self.runner {
            runner.busy = Duration::ZERO;
            runner.own = own.unwrap_or_default();
            runner.stubs.clear();
        }
        if own.is_err() {
            self.no_samples();
        }
    }

    fn no_samples(&mut self) {
        if self.sampling {
            self.sampling = false;
            self.counters.placement_without_samples += 1;
            eprintln!("wcontainers warning: placement without samples, the CPU clocks are refused");
        }
    }

    /// Reads a stub's CPU clock before its first turn in the window.
    fn note_stub(&mut self, stub: Pid) {
        let Some(runner) = &mut self.runner else {
            return;
        };
        if !self.sampling || runner.stubs.iter().any(|(held, _)| *held == stub) {
            return;
        }
        match process_clock(stub) {
            Ok(at) => runner.stubs.push((stub, at)),
            Err(_) => self.no_samples(),
        }
    }

    /// Adds a turn's time to the window; past 10 ms of it, the CPU's share (the runner's and its stubs' CPU time over
    /// the busy time) decides whether the runner moves to the CPU the host would give now.
    fn account(&mut self, turn: Duration) {
        let Some(runner) = &mut self.runner else {
            return;
        };
        runner.busy += turn;
        if !self.sampling || runner.busy < SAMPLE_EVERY {
            return;
        }
        let (Ok(own), Ok(stubs)) = (
            thread_clock(),
            runner
                .stubs
                .iter()
                .map(|(stub, at)| process_clock(*stub).map(|now| now.saturating_sub(*at)))
                .sum::<Result<Duration, Errno>>(),
        ) else {
            // A stub that ended has no clock: this window decides nothing.
            self.start_window();
            return;
        };
        let used = own.saturating_sub(runner.own) + stubs;
        let (part, whole) = CONTENDED_BELOW;
        let cpu = runner.cpu;
        if used.as_nanos() * whole < runner.busy.as_nanos() * part {
            self.host.mark_contended(cpu);
            let new = self.host.take_cpu();
            self.host.release_cpu(if new == cpu { new } else { cpu });
            if new != cpu {
                let new = self.pin(new);
                self.runner.as_mut().expect("placed").cpu = new;
                self.counters.placement_moves += 1;
            }
        }
        self.start_window();
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
        #[cfg(target_arch = "aarch64")]
        if self.memory(id).code_pages.overlaps(&range) {
            view.remove(ProtFlags::WRITE);
        }
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
                self.memory_mut(id).push(protect(piece, view), None);
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
            memory.push(
                Command::Map {
                    address: piece.start as usize,
                    length: (piece.end - piece.start) as usize,
                    permissions: ProtFlags::empty(),
                    offset: piece.start as usize,
                },
                None,
            );
        }
        ended
    }

    /// Refuses a change of at most `commands` commands that could take the stub past the host's limit on mappings,
    /// each command splitting at most two (plan 4.2).
    fn room(&mut self, id: MemoryId, commands: usize) -> Result<(), CpuError> {
        let limit = self.host.probe().mapping_limit();
        if self.host_mappings(id) + 2 * commands <= limit {
            return Ok(());
        }
        Err(self.mappings_exhausted())
    }

    /// Counts and warns of a change refused for want of host mappings.
    fn mappings_exhausted(&mut self) -> CpuError {
        self.counters.mappings_exhausted += 1;
        eprintln!(
            "wcontainers warning: host mappings exhausted, a change refused, {} so far",
            self.counters.mappings_exhausted
        );
        CpuError::OutOfMemory
    }

    /// The commands a change of a range's view takes at most: one for each shared piece it ends, one for each
    /// piece of what the kernel allows.
    fn view_commands(&self, id: MemoryId, range: Range<u64>) -> usize {
        let memory = self.memory(id);
        memory.shared.overlapping(range.clone()).count()
            + memory.permissions.overlapping(range).count()
    }

    /// Makes a range of a memory's own memfd read as zeros: punched out, or written with zeros where the host
    /// refuses that (gVisor).
    fn clear(&mut self, id: MemoryId, range: Range<u64>) -> Result<(), CpuError> {
        match punch(self.memory(id).stub.memory(), range.clone()) {
            Ok(()) => return Ok(()),
            Err(Errno::OPNOTSUPP) => {}
            Err(other) => return Err(host_error(other)),
        }
        self.counters.zeros_written += 1;
        eprintln!(
            "wcontainers warning: hole punch refused, zeros written, {} so far",
            self.counters.zeros_written
        );
        let zeros = [0; ZEROS];
        let stub = &self.memory(id).stub;
        let mut windows = self.host.windows();
        for at in range.clone().step_by(ZEROS) {
            let length = (range.end - at).min(ZEROS as u64) as usize;
            windows
                .write(stub.memory(), stub.memory_key(), at, &zeros[..length])
                .map_err(host_error)?;
        }
        Ok(())
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
        then: Option<(Then, Option<Duration>)>,
    ) -> Result<TurnEnd, CpuError> {
        let rounds = commands.len().div_ceil(MAX_COMMANDS).max(1);
        let mut blocks = blocks.iter();
        for round in 0..rounds {
            let chunk =
                &commands[round * MAX_COMMANDS..commands.len().min((round + 1) * MAX_COMMANDS)];
            let stub = &self.memory(id).stub;
            for command in chunk {
                if matches!(
                    command,
                    Command::MapShared { .. } | Command::MapCode | Command::MapSlots { .. }
                ) {
                    let block = blocks.next().expect("a memfd for every queued block");
                    stub.send_block(block.as_fd()).map_err(host_error)?;
                }
            }
            let last = round + 1 == rounds;
            let ping = Then::Touch {
                address: 0,
                value: 0,
                resume: Resume::Ping,
            };
            let (then, slice) = match then {
                Some(then) if last => then,
                _ => (ping, None),
            };
            let turn = match then {
                Then::Touch {
                    address,
                    value,
                    resume,
                } => stub.begin_touch(index, chunk, address, value, resume),
                #[cfg(target_arch = "aarch64")]
                Then::Enter(at) => stub.begin_enter(index, chunk, at),
            }
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

    /// Reads guest memory as the kernel's `read` does, whoever owns its pages.
    fn read_guest(&self, id: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
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
            queue: Vec::new(),
            queued_blocks: Vec::new(),
            view: View::default(),
            indices: vec![0],
            spare: Some(0),
            retired: false,
            #[cfg(target_arch = "aarch64")]
            code_pages: rangemap::RangeSet::new(),
        };
        let id = MemoryId(place(&mut self.memories, memory) as u32);
        self.memory_mut(id).push(
            Command::Map {
                address: GUEST.start as usize,
                length: (GUEST.end - GUEST.start) as usize,
                permissions: ProtFlags::empty(),
                offset: GUEST.start as usize,
            },
            None,
        );
        Ok(id)
    }

    /// The child borrows every private page the source owns or borrows and maps its shared ranges, and the source's
    /// writable ranges lose write in its view before this returns, which is the fork's instant.
    fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
        let writable = self
            .memory(source)
            .permissions
            .iter()
            .filter(|(_, granted)| granted.contains(Permissions::WRITE))
            .count();
        self.room(source, writable)?;
        let child_commands = 1 + self
            .memory(source)
            .shared
            .iter()
            .map(|(range, _)| {
                1 + self
                    .memory(source)
                    .permissions
                    .overlapping(range.clone())
                    .count()
            })
            .sum::<usize>();
        if OWN_MAPPINGS + 2 * child_commands > self.host.probe().mapping_limit() {
            return Err(self.mappings_exhausted());
        }
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
        let owned = match data_extents(self.memory(source).stub.memory()).map_err(host_error)? {
            Some(extents) => extents,
            None => {
                self.counters.extents_unknown += 1;
                eprintln!(
                    "wcontainers warning: memory extents unknown, a fork lends every private page, {} so far",
                    self.counters.extents_unknown
                );
                pieces(&self.memory(source).shared, GUEST)
                    .into_iter()
                    .filter(|(_, shared)| shared.is_none())
                    .map(|(piece, _)| piece)
                    .collect()
            }
        };
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
            blocks.push((
                range.clone(),
                *block,
                self.block(*block).file.clone(),
                *delta,
            ));
        }
        let memory = self.memory_mut(child);
        memory.permissions = permissions;
        memory.shared = shared;
        for (range, block, file, delta) in blocks {
            self.memory_mut(child).push(
                Command::MapShared {
                    address: range.start as usize,
                    length: (range.end - range.start) as usize,
                    permissions: ProtFlags::empty(),
                    offset: range.start.wrapping_add(delta) as usize,
                },
                Some((block, file)),
            );
            self.queue_view(child, range);
        }
        let memory = self.memory_mut(source);
        for command in protect_source {
            memory.push(command, None);
        }
        if let Err(error) = self.apply_now(source) {
            self.memory_drop(child);
            return Err(error);
        }
        Ok(child)
    }

    /// Ends a memory's stub; one that lends pages keeps its memfd until no page is borrowed from it.
    fn memory_drop(&mut self, id: MemoryId) {
        #[cfg(target_arch = "aarch64")]
        self.drop_code(id);
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
        self.read_guest(id, address, into)
    }

    /// Private pages are first made the memory's own and lent to none; shared ones are written in their block.
    fn write(&mut self, id: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        let range = checked(address, from.len() as u64)?;
        #[cfg(target_arch = "aarch64")]
        self.drop_code_of(id, range.clone());
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
        #[cfg(target_arch = "aarch64")]
        self.drop_code_of(id, range.clone());
        self.room(id, self.view_commands(id, range.clone()))?;
        if !self.unshare(id, range.clone()).is_empty() {
            self.queue_view(id, range.clone());
        }
        self.unborrow(id, range.clone(), false)?;
        self.unlend(id, range.clone())?;
        self.clear(id, range)
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
        self.room(id, 1)?;
        #[cfg(target_arch = "aarch64")]
        if !permissions.contains(Permissions::EXECUTE) {
            self.drop_code_of(id, range.clone());
        }
        let view = self.view_bound(id, range.clone(), permissions);
        let memory = self.memory_mut(id);
        if permissions.is_empty() {
            memory.permissions.remove(range.clone());
        } else {
            memory.permissions.insert(range.clone(), permissions);
        }
        memory.push(protect(range, view), None);
        Ok(())
    }

    /// Drops the pieces made from the range (ARM64; elsewhere nothing is translated yet).
    fn code_changed(&mut self, memory: MemoryId, address: u64, length: u64, _starts: &[u64]) {
        #[cfg(target_arch = "aarch64")]
        self.forget_changed_code(memory, address, length);
        #[cfg(not(target_arch = "aarch64"))]
        let _ = (memory, address, length);
    }

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
        #[cfg(target_arch = "aarch64")]
        self.drop_code_of(memory, range.clone());
        let block = self.block(shared);
        if offset
            .checked_add(length)
            .is_none_or(|end| end > block.length)
        {
            return Err(CpuError::BadAddress);
        }
        let file = block.file.clone();
        self.room(memory, 1 + self.view_commands(memory, range.clone()))?;
        self.unshare(memory, range.clone());
        self.unborrow(memory, range.clone(), false)?;
        self.unlend(memory, range.clone())?;
        self.clear(memory, range.clone())?;
        let target = self.memory_mut(memory);
        target
            .shared
            .insert(range.clone(), (shared, offset.wrapping_sub(address)));
        target.push(
            Command::MapShared {
                address: address as usize,
                length: length as usize,
                permissions: ProtFlags::empty(),
                offset: offset as usize,
            },
            Some((shared, file)),
        );
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
        #[cfg(target_arch = "aarch64")]
        self.drop_code_of(memory, range.clone());
        self.room(memory, self.view_commands(memory, range.clone()))?;
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
        let thread = Thread {
            memory: id,
            index,
            cpu: None,
        };
        let thread = ThreadId(place(&mut self.threads, thread) as u32);
        #[cfg(target_arch = "aarch64")]
        {
            let at = thread.0 as usize;
            if self.guests.len() <= at {
                self.guests.resize_with(at + 1, || None);
            }
            self.guests[at] = Some(native::Guest::new());
        }
        Ok(thread)
    }

    /// Ends a guest thread's host thread, or keeps it as its memory's spare if it has none.
    fn thread_drop(&mut self, thread: ThreadId) {
        let Thread { memory, index, .. } = self.threads[thread.0 as usize]
            .take()
            .expect("a thread the kernel holds");
        #[cfg(target_arch = "aarch64")]
        {
            self.guests[thread.0 as usize] = None;
        }
        let memory = self.memory_mut(memory);
        if memory.spare.is_none() {
            memory.spare = Some(index);
        } else {
            let _ = memory.stub.end_thread(index);
            memory.indices.retain(|held| *held != index);
        }
    }

    /// The guest's own, as its last exit resolved them (ARM64), or as its register page holds them.
    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        let Thread { memory, index, .. } = self.thread(thread);
        #[cfg(target_arch = "aarch64")]
        if let Some(guest) = &mut self.guests[thread.0 as usize] {
            if guest.vectors_held() {
                guest.take_vectors(
                    self.memories[memory.0 as usize]
                        .as_ref()
                        .expect("a memory the kernel holds")
                        .stub
                        .vectors(index),
                );
            }
            guest.registers(into);
            return;
        }
        self.memory(memory)
            .stub
            .read_registers(index, into.as_mut_bytes());
    }

    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        #[cfg(target_arch = "aarch64")]
        if let Some(guest) = &mut self.guests[thread.0 as usize] {
            guest.set_registers(from);
        }
        let Thread { memory, index, .. } = self.thread(thread);
        self.memory(memory)
            .stub
            .write_registers(index, from.as_bytes());
    }
}

impl Drop for HostProcessExecutor {
    fn drop(&mut self) {
        if let Some(runner) = self.runner.take() {
            self.host.release_cpu(runner.cpu);
        }
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

/// The calling thread's CPU time.
fn thread_clock() -> Result<Duration, Errno> {
    let at = rustix::time::clock_gettime(rustix::time::ClockId::ThreadCPUTime);
    Ok(Duration::new(at.tv_sec as u64, at.tv_nsec as u32))
}

/// A process's CPU time, through its CPU clock, which any process may read.
fn process_clock(process: Pid) -> Result<Duration, Errno> {
    let process = nix::unistd::Pid::from_raw(process.as_raw_nonzero().get());
    let at = nix::time::clock_getcpuclockid(process)
        .and_then(nix::time::clock_gettime)
        .map_err(|errno| Errno::from_raw_os_error(errno as i32))?;
    Ok(Duration::new(at.tv_sec() as u64, at.tv_nsec() as u32))
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

/// The extents of a memfd's guest area that hold pages, rounded out to whole pages; none where the host's files do
/// not say (gVisor refuses `SEEK_DATA` with EINVAL).
fn data_extents(file: BorrowedFd<'_>) -> Result<Option<Vec<Range<u64>>>, Errno> {
    let mut extents = Vec::new();
    let mut at = GUEST.start;
    while at < GUEST.end {
        let start = match rustix::fs::seek(file, SeekFrom::Data(at)) {
            Ok(start) => start,
            Err(Errno::NXIO) => break,
            Err(Errno::INVAL) => return Ok(None),
            Err(other) => return Err(other),
        };
        if start >= GUEST.end {
            break;
        }
        let end = rustix::fs::seek(file, SeekFrom::Hole(start))?.min(GUEST.end);
        extents.push(page_cover(&(start..end)));
        at = end;
    }
    Ok(Some(extents))
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
