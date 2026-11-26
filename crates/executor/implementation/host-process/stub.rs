//! A stub: one guest memory as a host process forked by the zygote, each of its threads driven through its pages.

use core::mem::offset_of;
use std::io::IoSlice;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use linux_raw_sys::general as linux;
use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
use rustix::process::Pid;
use rustix::thread::futex;
use wcontainers_abi::RegisterBlock;
#[cfg(target_arch = "aarch64")]
use wcontainers_executor_arm64_stop::Interrupted;
use zerocopy::IntoBytes;

use crate::filter::stub_filter;
use crate::layout::THREADS_PER_PROCESS;
use crate::pages::{Command, ExitKind, Resume, ThreadPages, control, register};

/// The stub's socket's fixed descriptor number, above the few the zygote holds when it forks.
pub const SOCKET_FD: u32 = 8;
/// The memory memfd's fixed descriptor number.
pub const MEMORY_FD: u32 = 9;
/// The number a shared block's memfd gets in the stub while it is mapped: the lowest, as the stub holds only the two
/// above.
pub const BLOCK_FD: u32 = 0;

/// How long a runner waits for a thread before it looks whether the stub has ended, and how long a stopped thread
/// may take to come back before it is sent its stop again.
const LOOK_FOR_END: Duration = Duration::from_millis(100);

/// How often a runner looks again whether a thread whose slice has passed has taken its turn yet, so the stop is not
/// spent before the turn begins.
const TAKEN_POLL: Duration = Duration::from_millis(1);

/// When a turn's thread is stopped: once `slice` has passed, or as soon as `wanted` is set.
#[derive(Clone, Copy, Default)]
pub struct StopWhen<'wanted> {
    /// How long the turn may run.
    pub slice: Option<Duration>,
    /// Set from another thread to have the turn stopped now.
    pub wanted: Option<&'wanted AtomicBool>,
}

/// How long after its first stop a thread that has not come back has its stub killed: only a compromised stub can
/// hold the stop off.
pub const STOP_IGNORED: Duration = Duration::from_secs(10);

/// The largest error number a host call answers, Linux's `MAX_ERRNO` (<https://github.com/torvalds/linux/blob/master/include/linux/err.h>).
const MAX_ERRNO: u32 = 4095;

/// The bytes of a register block that hold the vectors.
const VECTORS: core::ops::Range<usize> =
    offset_of!(RegisterBlock, v)..offset_of!(RegisterBlock, fpsr);

// FPSR and FPCR share one word of the register page, FPSR in its low half.
const _: () =
    assert!(offset_of!(RegisterBlock, fpcr) == offset_of!(RegisterBlock, fpsr) + size_of::<u32>());
const _: () = assert!(offset_of!(RegisterBlock, fpsr).is_multiple_of(size_of::<u64>()));

/// One stub, owned by the executor; the zygote that forked it reaps it, and its owner makes the window cache forget
/// its memory before dropping it.
pub struct Stub {
    process: Pid,
    /// The number of the zygote that forked it.
    zygote: u64,
    socket: OwnedFd,
    pages: ThreadPages,
    memory: OwnedFd,
    /// One bit per thread index the executor holds, from its `new_thread` to its `end_thread`.
    held: Vec<u64>,
    /// Exits from translated code whose FP/SIMD registers came by a return through the frame ("signal frame without
    /// FP/SIMD registers").
    vectors_by_return: AtomicU64,
}

/// How a stub's threads are made: sharing everything a thread of one process shares, the new thread's exit-clear word
/// written before the call returns and cleared, with a wake, at its end.
pub(crate) const THREAD_FLAGS: u32 = linux::CLONE_VM
    | linux::CLONE_FS
    | linux::CLONE_FILES
    | linux::CLONE_SIGHAND
    | linux::CLONE_THREAD
    | linux::CLONE_SYSVSEM
    | linux::CLONE_PARENT_SETTID
    | linux::CLONE_CHILD_CLEARTID;

/// The signal that stops a stub's thread: a standard one, so stops sent while one is pending are that one, and a
/// stub holds no C library that claims it.
pub const STOP_SIGNAL: u32 = linux::SIGUSR1;

/// [`STOP_SIGNAL`] alone, as a kernel signal set.
pub(crate) const STOP_SET: u64 = 1 << (STOP_SIGNAL - 1);

/// How a turn ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnEnd {
    /// Every command was applied and the turn's resume done.
    Done {
        /// The commands applied.
        commands_applied: u64,
    },
    /// A command's host call failed; those before it were applied.
    CommandRefused {
        /// The command's index.
        index: u32,
        /// The host's error.
        errno: Errno,
    },
    /// The stub found the turn malformed.
    BadTurn,
    /// The stop signal ended the turn, after its commands.
    Stopped {
        /// The commands applied.
        commands_applied: u64,
    },
    /// The turn's touch faulted.
    Fault {
        /// The address the host reported.
        address: u64,
        /// The host's signal.
        signal: u32,
        /// The signal's code (`SEGV_MAPERR`, `SEGV_ACCERR`, ...).
        code: u32,
    },
    /// The turn's load was done.
    Loaded {
        /// The word loaded.
        value: u64,
    },
    /// Translated code called a floor entry; the register block holds what the code held, x30 and pc the call's link.
    Called {
        /// The entry's number.
        entry: u64,
    },
    /// The stop signal ended translated code; the register block holds what the code held there.
    Interrupted {
        /// The host pc it stopped at.
        at: u64,
    },
    /// The stub reported an exit it has no name for, or one with a value no such exit holds.
    Forged,
    /// The stub's process ended: its socket hung up.
    StubEnded,
}

impl Stub {
    pub(crate) fn new(
        process: Pid,
        zygote: u64,
        socket: OwnedFd,
        pages: ThreadPages,
        memory: OwnedFd,
    ) -> Self {
        let mut held = vec![0; THREADS_PER_PROCESS / u64::BITS as usize];
        held[0] = 1;
        Self {
            process,
            zygote,
            socket,
            pages,
            memory,
            held,
            vectors_by_return: AtomicU64::new(0),
        }
    }

    /// Holds the first free thread index and gives the command that makes its thread, for a turn of a thread in a
    /// kernel entry; `None` when every index is held or still ending. An index is free once the executor has let it
    /// go and the host kernel has cleared its exit-clear word.
    pub fn new_thread(&mut self) -> Option<Command> {
        let thread = (0..THREADS_PER_PROCESS).find(|&thread| {
            !self.holds(thread)
                && self
                    .pages
                    .register32(thread, register::EXIT_CLEAR)
                    .load(Ordering::Acquire)
                    == 0
        })?;
        self.held[thread / u64::BITS as usize] |= 1 << (thread % u64::BITS as usize);
        self.pages.write_records(thread);
        let last_turn = self
            .pages
            .control32(thread, control::ANSWER)
            .load(Ordering::Relaxed);
        Some(Command::NewThread { thread, last_turn })
    }

    /// Lets an index go: its thread was refused, or has been given its end.
    pub fn release_thread(&mut self, thread: usize) {
        self.held[thread / u64::BITS as usize] &= !(1 << (thread % u64::BITS as usize));
    }

    fn holds(&self, thread: usize) -> bool {
        self.held[thread / u64::BITS as usize] & (1 << (thread % u64::BITS as usize)) != 0
    }

    /// Ends one thread of several and lets its index go; the index is reused only after the host kernel clears its
    /// exit-clear word.
    pub fn end_thread(&mut self, thread: usize) -> Result<(), Errno> {
        self.give(thread, &[], Resume::ExitThread)?;
        self.release_thread(thread);
        Ok(())
    }

    /// Waits up to `limit` for the host kernel to clear a thread's exit-clear word, which it does after the thread has
    /// left every page of its index; whether it did.
    pub fn thread_gone(&self, thread: usize, limit: Duration) -> Result<bool, Errno> {
        let word = self.pages.register32(thread, register::EXIT_CLEAR);
        let until = Instant::now() + limit;
        loop {
            let seen = word.load(Ordering::Acquire);
            if seen == 0 {
                return Ok(true);
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(false);
            }
            match futex::wait(word, futex::Flags::empty(), seen, Some(&timespec(left))) {
                Ok(()) | Err(Errno::AGAIN | Errno::INTR | Errno::TIMEDOUT) => {}
                Err(other) => return Err(other),
            }
        }
    }

    /// Writes the stub's start: thread 0's records, then into its control page the filter, which the stub reads, and the
    /// start's turn; a thread made later has its records written when its index is taken.
    pub(crate) fn write_start(pages: &ThreadPages, frames_without_vectors: bool) {
        pages.write_records(0);
        let filter = stub_filter(frames_without_vectors);
        assert!(
            filter.len() * size_of::<u64>() <= control::COMMANDS_END - control::COMMANDS,
            "the stub's filter outgrew its page"
        );
        for (index, instruction) in filter.iter().enumerate() {
            let word = u64::from(instruction.code)
                | (u64::from(instruction.jt) << 16)
                | (u64::from(instruction.jf) << 24)
                | (u64::from(instruction.k) << 32);
            pages
                .control64(0, control::COMMANDS + index * size_of::<u64>())
                .store(word, Ordering::Relaxed);
        }
        pages
            .control64(0, control::FILTER_LENGTH)
            .store(filter.len() as u64, Ordering::Relaxed);
        pages
            .control32(0, control::ANSWER)
            .store(control::START, Ordering::Release);
    }

    /// Waits for the start's turn; how the stub ended, if it ended instead.
    pub(crate) fn started(&self) -> Result<TurnEnd, Errno> {
        self.wait(0, control::START)
    }

    /// The stub's process id, as the zygote reported it.
    pub fn process(&self) -> Pid {
        self.process
    }

    /// The memory's memfd: guest address `a` is its offset `a`.
    pub fn memory(&self) -> BorrowedFd<'_> {
        self.memory.as_fd()
    }

    pub(crate) fn zygote(&self) -> u64 {
        self.zygote
    }

    /// A thread's host id, as the stub reports it (untrusted).
    pub fn reported_thread_id(&self, thread: usize) -> u32 {
        self.pages
            .register32(thread, register::THREAD_ID)
            .load(Ordering::Acquire)
    }

    /// Where a thread's stops landed, as the stub counts them (untrusted).
    pub fn stops_landed(&self, thread: usize) -> StopsLanded {
        let count = |at| self.pages.register64(thread, at).load(Ordering::Relaxed);
        StopsLanded {
            in_turn: count(register::STOPS_IN_TURN),
            in_publish: count(register::STOPS_IN_PUBLISH),
            in_wait: count(register::STOPS_IN_WAIT),
            in_end: count(register::STOPS_IN_END),
            in_call: count(register::STOPS_IN_CALL),
        }
    }

    /// How many exits from translated code saved their FP/SIMD registers after a return through a signal frame that
    /// held none (as the stub counts them, untrusted).
    pub fn vectors_by_return(&self) -> u64 {
        self.vectors_by_return.load(Ordering::Relaxed)
    }

    /// Gives a thread a turn that applies `commands` in order and reports back, and waits for its end.
    pub fn turn(&self, thread: usize, commands: &[Command]) -> Result<TurnEnd, Errno> {
        let turn = self.give(thread, commands, Resume::Ping)?;
        self.wait(thread, turn)
    }

    /// Gives a thread a turn that loads the word at `address`, as guest code would, and waits for its end.
    pub fn load(&self, thread: usize, address: usize) -> Result<TurnEnd, Errno> {
        self.touch(thread, &[], address, 0, Resume::Load)
    }

    /// Gives a thread a turn that stores `value` at `address`, as guest code would, and waits for its end.
    pub fn store(&self, thread: usize, address: usize, value: u64) -> Result<TurnEnd, Errno> {
        self.touch(thread, &[], address, value, Resume::Store)
    }

    /// Gives a thread a turn that applies `commands`, then does a [`Resume::Load`] or [`Resume::Store`] of `value` at
    /// `address`, and waits for its end.
    pub fn touch(
        &self,
        thread: usize,
        commands: &[Command],
        address: usize,
        value: u64,
        resume: Resume,
    ) -> Result<TurnEnd, Errno> {
        let turn = self.begin_touch(thread, commands, address, value, resume)?;
        self.wait(thread, turn)
    }

    /// Gives a thread a turn that applies `commands`, then loads its register block and branches to `at` in the code
    /// area, and waits for its end.
    pub fn enter(&self, thread: usize, commands: &[Command], at: usize) -> Result<TurnEnd, Errno> {
        let turn = self.begin_enter(thread, commands, at)?;
        self.wait(thread, turn)
    }

    /// Gives a thread a turn as [`Self::enter`] does, without waiting for it; the turn's number.
    pub fn begin_enter(
        &self,
        thread: usize,
        commands: &[Command],
        at: usize,
    ) -> Result<u32, Errno> {
        self.pages
            .control64(thread, control::ENTER_AT)
            .store(at as u64, Ordering::Relaxed);
        self.give(thread, commands, Resume::Enter)
    }

    /// A thread's register block as the stub left it (untrusted).
    pub fn registers(&self, thread: usize) -> RegisterBlock {
        let mut block = RegisterBlock::ZERO;
        self.read_registers(thread, block.as_mut_bytes());
        block
    }

    /// Replaces a thread's register block.
    pub fn set_registers(&self, thread: usize, block: &RegisterBlock) {
        self.write_registers(thread, block.as_bytes());
    }

    /// A thread's registers but for its vectors, and its thread pointer, as the stub left them (untrusted); read word
    /// by word, with no register block built.
    #[cfg(target_arch = "aarch64")]
    pub fn scalars(&self, thread: usize) -> (Interrupted, u64) {
        let word = |at: usize| {
            self.pages
                .register64(thread, register::REGISTERS + at)
                .load(Ordering::Relaxed)
        };
        let float = word(offset_of!(RegisterBlock, fpsr));
        let registers = Interrupted {
            x: std::array::from_fn(|index| {
                word(offset_of!(RegisterBlock, x) + index * size_of::<u64>())
            }),
            sp: word(offset_of!(RegisterBlock, sp)),
            pc: word(offset_of!(RegisterBlock, pc)),
            state: word(offset_of!(RegisterBlock, pstate)),
            fpsr: float as u32,
            fpcr: (float >> u32::BITS) as u32,
        };
        (registers, word(offset_of!(RegisterBlock, tpidr_el0)))
    }

    /// Replaces a thread's registers but for its vectors, and its thread pointer, word by word.
    #[cfg(target_arch = "aarch64")]
    pub fn set_scalars(&self, thread: usize, registers: &Interrupted, tpidr: u64) {
        let set = |at: usize, value: u64| {
            self.pages
                .register64(thread, register::REGISTERS + at)
                .store(value, Ordering::Relaxed);
        };
        for (index, &value) in registers.x.iter().enumerate() {
            set(
                offset_of!(RegisterBlock, x) + index * size_of::<u64>(),
                value,
            );
        }
        set(offset_of!(RegisterBlock, sp), registers.sp);
        set(offset_of!(RegisterBlock, pc), registers.pc);
        set(offset_of!(RegisterBlock, pstate), registers.state);
        set(
            offset_of!(RegisterBlock, fpsr),
            u64::from(registers.fpsr) | u64::from(registers.fpcr) << u32::BITS,
        );
        set(offset_of!(RegisterBlock, tpidr_el0), tpidr);
    }

    /// A thread's vectors as the stub left them (untrusted).
    pub fn vectors(&self, thread: usize) -> [[u64; 2]; 32] {
        let mut block = RegisterBlock::ZERO;
        self.read_registers_from(
            thread,
            VECTORS.start,
            &mut block.as_mut_bytes()[VECTORS.clone()],
        );
        block.v
    }

    /// Gives a thread a turn as [`Self::touch`] does, without waiting for it; the turn's number.
    pub fn begin_touch(
        &self,
        thread: usize,
        commands: &[Command],
        address: usize,
        value: u64,
        resume: Resume,
    ) -> Result<u32, Errno> {
        self.pages
            .control64(thread, control::TOUCH_ADDRESS)
            .store(address as u64, Ordering::Relaxed);
        self.pages
            .control64(thread, control::TOUCH_VALUE)
            .store(value, Ordering::Relaxed);
        self.give(thread, commands, resume)
    }

    /// Waits for the end of `turn`, sending the stop once `when` says so and the thread has taken the turn (or 100 ms
    /// after that at most), and again every 100 ms after; `None` if the thread has not come back [`STOP_IGNORED`]
    /// after its first stop. Counts the stops sent in `stops`.
    pub fn wait_sliced(
        &self,
        thread: usize,
        turn: u32,
        when: StopWhen<'_>,
        stops: &mut u64,
    ) -> Result<Option<TurnEnd>, Errno> {
        let mut slice_end = when.slice.map(|slice| Instant::now() + slice);
        let mut stop_at = slice_end;
        let mut first_stop = None;
        let mut wanted_seen = false;
        loop {
            if !wanted_seen
                && when
                    .wanted
                    .is_some_and(|wanted| wanted.load(Ordering::Acquire))
            {
                wanted_seen = true;
                let now = Instant::now();
                if slice_end.is_none_or(|end| end > now) {
                    slice_end = Some(now);
                    stop_at = Some(now);
                }
            }
            let limit = match stop_at {
                Some(at) => at
                    .saturating_duration_since(Instant::now())
                    .min(LOOK_FOR_END),
                // A stop wanted before the turn is taken lands in the wait: it is sent again once the turn is taken.
                None if when.wanted.is_some() && self.taken(thread) != turn => TAKEN_POLL,
                None => LOOK_FOR_END,
            };
            if let Some(end) = self.wait_within(thread, turn, limit)? {
                return Ok(Some(end));
            }
            if self.hung_up()? {
                return Ok(Some(TurnEnd::StubEnded));
            }
            let now = Instant::now();
            if let (Some(slice_end), None) = (slice_end, first_stop)
                && stop_at.is_some_and(|at| now >= at)
                && now.duration_since(slice_end) < LOOK_FOR_END
                && self.taken(thread) != turn
            {
                // A stop before the turn is taken would land in the wait, where it changes nothing.
                stop_at = Some(now + TAKEN_POLL);
                continue;
            }
            if stop_at.is_some_and(|at| now >= at) {
                if first_stop.is_some_and(|first| now.duration_since(first) >= STOP_IGNORED) {
                    return Ok(None);
                }
                // A thread id the stub forged reaches no thread; the kill above ends that.
                let _ = self.stop(thread);
                *stops += 1;
                first_stop.get_or_insert(now);
                stop_at = Some(now + LOOK_FOR_END);
            }
        }
    }

    /// Sends a shared block's memfd for the next [`Command::MapShared`] a thread of the stub applies.
    pub fn send_block(&self, block: BorrowedFd<'_>) -> Result<(), Errno> {
        let blocks = [block];
        let mut space = [core::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !ancillary.push(SendAncillaryMessage::ScmRights(&blocks)) {
            return Err(Errno::NOBUFS);
        }
        let sent = rustix::net::sendmsg(
            &self.socket,
            &[IoSlice::new(&[0])],
            &mut ancillary,
            SendFlags::DONTWAIT,
        )?;
        if sent == 1 { Ok(()) } else { Err(Errno::PROTO) }
    }

    /// The turn a thread last took, as it reports it (untrusted: a forged value only delays its own stop, by at most
    /// 100 ms).
    fn taken(&self, thread: usize) -> u32 {
        self.pages
            .register32(thread, register::TAKEN)
            .load(Ordering::Acquire)
    }

    /// Reads a thread's register block, word by word, as the stub left it (untrusted).
    pub fn read_registers(&self, thread: usize, into: &mut [u8]) {
        self.read_registers_from(thread, 0, into);
    }

    /// Reads a thread's register block from byte `at` on, word by word.
    fn read_registers_from(&self, thread: usize, at: usize, into: &mut [u8]) {
        let (words, _) = into.as_chunks_mut::<{ size_of::<u64>() }>();
        for (index, word) in words.iter_mut().enumerate() {
            let value = self
                .pages
                .register64(thread, register::REGISTERS + at + index * size_of::<u64>())
                .load(Ordering::Relaxed);
            *word = value.to_le_bytes();
        }
    }

    /// Replaces a thread's register block, word by word.
    pub fn write_registers(&self, thread: usize, from: &[u8]) {
        self.write_registers_from(thread, 0, from);
    }

    /// Replaces a thread's register block from byte `at` on, word by word.
    fn write_registers_from(&self, thread: usize, at: usize, from: &[u8]) {
        let (words, _) = from.as_chunks::<{ size_of::<u64>() }>();
        for (index, word) in words.iter().enumerate() {
            let value = u64::from_le_bytes(*word);
            self.pages
                .register64(thread, register::REGISTERS + at + index * size_of::<u64>())
                .store(value, Ordering::Relaxed);
        }
    }

    /// A thread's words of the native output's register page, in their order there, as translated code left them
    /// (untrusted).
    #[cfg(target_arch = "aarch64")]
    pub fn translated_page(&self, thread: usize) -> wcontainers_arm64::thread_state::RegisterPage {
        let mut page = wcontainers_arm64::thread_state::RegisterPage::default();
        let mut words = (0..).map(|index| {
            self.pages
                .register64(thread, register::TRANSLATED + index * size_of::<u64>())
                .load(Ordering::Relaxed)
        });
        let mut next = || words.next().expect("words without end");
        for slot in page
            .withheld
            .iter_mut()
            .chain(&mut page.saved)
            .chain([&mut page.link])
        {
            *slot = next();
        }
        page.set_exclusive(std::array::from_fn(|_| next()));
        page
    }

    /// Replaces a thread's words of the native output's register page.
    #[cfg(target_arch = "aarch64")]
    pub fn set_translated_page(
        &self,
        thread: usize,
        page: &wcontainers_arm64::thread_state::RegisterPage,
    ) {
        let exclusive = page.exclusive();
        let words = page
            .withheld
            .iter()
            .chain(&page.saved)
            .chain([&page.link])
            .chain(&exclusive);
        for (index, &value) in words.enumerate() {
            self.pages
                .register64(thread, register::TRANSLATED + index * size_of::<u64>())
                .store(value, Ordering::Relaxed);
        }
    }

    /// Gives a thread a turn without waiting for it; the turn's number, for [`Self::wait`].
    pub fn begin(&self, thread: usize, commands: &[Command], resume: Resume) -> Result<u32, Errno> {
        self.give(thread, commands, resume)
    }

    /// Where a thread's stop goes, by the host id the stub reported: a forged id reaches only another thread of the
    /// same stub, which takes a stale stop.
    pub fn stop_target(&self, thread: usize) -> StopTarget {
        StopTarget {
            process: self.process.as_raw_nonzero().get(),
            thread: self.reported_thread_id(thread) as libc::pid_t,
        }
    }

    /// Sends the stop signal to a thread; a thread not in a turn, or already leaving one, ignores it.
    pub fn stop(&self, thread: usize) -> Result<(), Errno> {
        self.stop_target(thread).stop()
    }

    /// Waits for a thread to publish the end of `turn`, looking every 100 ms whether the stub has ended.
    pub fn wait(&self, thread: usize, turn: u32) -> Result<TurnEnd, Errno> {
        loop {
            if let Some(end) = self.wait_within(thread, turn, LOOK_FOR_END)? {
                return Ok(end);
            }
            if self.hung_up()? {
                return Ok(TurnEnd::StubEnded);
            }
        }
    }

    /// Waits up to `limit` for a thread to publish the end of `turn`; `None` if it has not. The clock is read only after
    /// a wake that did not publish it, so a wait from the first wake on may last a little longer than `limit`.
    pub fn wait_within(
        &self,
        thread: usize,
        turn: u32,
        limit: Duration,
    ) -> Result<Option<TurnEnd>, Errno> {
        let request = self.pages.register32(thread, register::REQUEST);
        let mut left = limit;
        let mut until = None;
        loop {
            let seen = request.load(Ordering::Acquire);
            if seen == turn {
                return Ok(Some(self.exit(thread)));
            }
            if left.is_zero() {
                return Ok(None);
            }
            match futex::wait(request, futex::Flags::empty(), seen, Some(&timespec(left))) {
                Err(Errno::TIMEDOUT) => left = Duration::ZERO,
                Ok(()) | Err(Errno::AGAIN | Errno::INTR) => {
                    let now = Instant::now();
                    left = until
                        .get_or_insert(now + left)
                        .saturating_duration_since(now);
                }
                Err(other) => return Err(other),
            }
        }
    }

    /// Ends the stub's last thread, and so its process, and waits for its socket to hang up; `TIMEDOUT` if it has
    /// not after 100 ms, when the zygote must kill it.
    pub fn end(&self, thread: usize) -> Result<(), Errno> {
        self.give(thread, &[], Resume::ExitThread)?;
        let mut socket = [PollFd::new(&self.socket, PollFlags::IN)];
        let ready = rustix::event::poll(&mut socket, Some(&timeout()))?;
        if ready > 0 && socket[0].revents().contains(PollFlags::HUP) {
            Ok(())
        } else {
            Err(Errno::TIMEDOUT)
        }
    }

    fn give(&self, thread: usize, commands: &[Command], resume: Resume) -> Result<u32, Errno> {
        if commands.len() > control::MAX_COMMANDS {
            return Err(Errno::TOOBIG);
        }
        let mut places = commands.iter().filter_map(|command| match command {
            Command::Place { cpu } => Some(*cpu),
            _ => None,
        });
        if let Some(cpu) = places.next() {
            if places.next().is_some() {
                return Err(Errno::INVAL);
            }
            self.pages.write_cpu_set(thread, cpu)?;
        }
        let answer = self.pages.control32(thread, control::ANSWER);
        let turn = answer.load(Ordering::Relaxed).wrapping_add(1);
        for (index, command) in commands.iter().enumerate() {
            for (word_index, word) in command.words().into_iter().enumerate() {
                let at = control::COMMANDS + index * control::COMMAND_SIZE + word_index * 8;
                self.pages
                    .control64(thread, at)
                    .store(word, Ordering::Relaxed);
            }
        }
        self.pages
            .control64(thread, control::COMMAND_COUNT)
            .store(commands.len() as u64, Ordering::Relaxed);
        self.pages
            .control64(thread, control::RESUME)
            .store(resume as u64, Ordering::Relaxed);
        answer.store(turn, Ordering::Release);
        futex::wake(answer, futex::Flags::empty(), 1)?;
        Ok(turn)
    }

    /// The exit the thread published, read once and checked.
    fn exit(&self, thread: usize) -> TurnEnd {
        let kind = self
            .pages
            .register64(thread, register::EXIT)
            .load(Ordering::Relaxed);
        let value = self
            .pages
            .register64(thread, register::EXIT_VALUE)
            .load(Ordering::Relaxed);
        let kind = ExitKind::from_word(kind);
        if matches!(
            kind,
            Some(ExitKind::Fault | ExitKind::Called | ExitKind::Interrupted)
        ) {
            self.count_vectors_by_return(thread);
        }
        match kind {
            Some(ExitKind::Done) => TurnEnd::Done {
                commands_applied: value,
            },
            Some(ExitKind::CommandRefused) => match value as u32 {
                errno @ 1..=MAX_ERRNO => TurnEnd::CommandRefused {
                    index: (value >> 32) as u32,
                    errno: Errno::from_raw_os_error(errno as i32),
                },
                _ => TurnEnd::Forged,
            },
            Some(ExitKind::BadTurn) => TurnEnd::BadTurn,
            Some(ExitKind::Stopped) => TurnEnd::Stopped {
                commands_applied: value,
            },
            Some(ExitKind::Fault) => {
                let detail = self
                    .pages
                    .register64(thread, register::EXIT_DETAIL)
                    .load(Ordering::Relaxed);
                TurnEnd::Fault {
                    address: value,
                    signal: detail as u32,
                    code: (detail >> 32) as u32,
                }
            }
            Some(ExitKind::Loaded) => TurnEnd::Loaded { value },
            Some(ExitKind::Called) => {
                #[cfg(target_arch = "aarch64")]
                self.put_back_call_scratch(thread);
                TurnEnd::Called { entry: value }
            }
            Some(ExitKind::Interrupted) => TurnEnd::Interrupted { at: value },
            None => TurnEnd::Forged,
        }
    }

    /// The syndrome of a thread's last fault in translated code, as the host's signal frame gave it; zero where it gave
    /// none (untrusted).
    pub fn fault_syndrome(&self, thread: usize) -> u64 {
        self.pages
            .register64(thread, register::EXIT_SYNDROME)
            .load(Ordering::Relaxed)
    }

    /// Puts x23 as the floor entry found it back into the block, where the entry's save put its own number.
    #[cfg(target_arch = "aarch64")]
    fn put_back_call_scratch(&self, thread: usize) {
        let kept = self
            .pages
            .register64(thread, register::CALL_SCRATCH)
            .load(Ordering::Relaxed);
        let scratch = usize::from(wcontainers_arm64::thread_state::SCRATCH);
        self.pages
            .register64(thread, register::REGISTERS + scratch * size_of::<u64>())
            .store(kept, Ordering::Relaxed);
    }

    /// Counts the exits the thread saved by a return through a frame without FP/SIMD registers, warning at the first.
    fn count_vectors_by_return(&self, thread: usize) {
        let seen = self
            .pages
            .register64(thread, register::VECTORS_BY_RETURN)
            .swap(0, Ordering::Relaxed);
        if seen > 0 && self.vectors_by_return.fetch_add(seen, Ordering::Relaxed) == 0 {
            eprintln!(
                "wcontainers warning: signal frame without FP/SIMD registers, restored by a return (counted, warned once per guest process)"
            );
        }
    }

    /// Whether the stub's socket has hung up, which only its end does.
    pub(crate) fn hung_up(&self) -> Result<bool, Errno> {
        let mut socket = [PollFd::new(&self.socket, PollFlags::IN)];
        let ready = rustix::event::poll(&mut socket, Some(&Timespec::default()))?;
        Ok(ready > 0 && socket[0].revents().contains(PollFlags::HUP))
    }

    /// Waits up to `limit` for the stub's socket to hang up; whether it did. Linux closes a process's files only after
    /// every thread of it has left its address space (do_exit releases the memory before the files), so once it has,
    /// no guest code runs on the stub's mappings.
    pub(crate) fn wait_hung_up(&self, limit: Duration) -> Result<bool, Errno> {
        let until = Instant::now() + limit;
        loop {
            if self.hung_up()? {
                return Ok(true);
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(false);
            }
            let mut socket = [PollFd::new(&self.socket, PollFlags::IN)];
            rustix::event::poll(&mut socket, Some(&timespec(left.min(LOOK_FOR_END))))?;
        }
    }
}

fn timeout() -> Timespec {
    timespec(LOOK_FOR_END)
}

fn timespec(duration: Duration) -> Timespec {
    Timespec {
        tv_sec: duration.as_secs() as i64,
        tv_nsec: i64::from(duration.subsec_nanos()),
    }
}

/// Where one thread's stops landed: those after a turn's commands ended it; the rest were stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopsLanded {
    /// In a turn, after its commands.
    pub in_turn: u64,
    /// While the thread published an exit, which it then published again.
    pub in_publish: u64,
    /// While it waited for a turn, to which it went back.
    pub in_wait: u64,
    /// While it ended, which it went on with.
    pub in_end: u64,
    /// In a floor entry or its save, which it finished from the signal frame.
    pub in_call: u64,
}

/// One stub thread's stop, to send from any thread.
#[derive(Debug, Clone, Copy)]
pub struct StopTarget {
    process: libc::pid_t,
    thread: libc::pid_t,
}

impl StopTarget {
    /// Sends the stop signal.
    pub fn stop(self) -> Result<(), Errno> {
        #[expect(
            unsafe_code,
            reason = "glibc's tgkill takes only numbers; neither rustix nor nix sends a signal to one thread of another process"
        )]
        // SAFETY: plain integers in, an integer out.
        let sent = unsafe { libc::tgkill(self.process, self.thread, STOP_SIGNAL as libc::c_int) };
        if sent == 0 {
            Ok(())
        } else {
            Err(Errno::from_io_error(&std::io::Error::last_os_error()).unwrap_or(Errno::INVAL))
        }
    }
}
