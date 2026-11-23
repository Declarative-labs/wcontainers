//! A stub: one guest memory as a host process forked by the zygote, each of its threads driven through its pages.

use std::io::IoSlice;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::Ordering;
use std::time::Duration;

use linux_raw_sys::general as linux;
use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
use rustix::process::Pid;
use rustix::thread::futex;

use crate::filter::stub_filter;
use crate::layout::THREADS_PER_PROCESS;
use crate::pages::{Command, ExitKind, Resume, ThreadPages, control, register};
use crate::windows::FileKey;

/// The stub's socket's fixed descriptor number, above the few the zygote holds when it forks.
pub const SOCKET_FD: u32 = 8;
/// The memory memfd's fixed descriptor number.
pub const MEMORY_FD: u32 = 9;
/// The number a shared block's memfd gets in the stub while it is mapped: the lowest, as the stub holds only the two
/// above.
pub const BLOCK_FD: u32 = 0;

/// How long a runner waits for a thread before it looks whether the stub has ended.
const LOOK_FOR_END: Duration = Duration::from_millis(100);

/// One stub, owned by the executor; the zygote that forked it reaps it, and its owner makes the window cache forget
/// its memory before dropping it.
pub struct Stub {
    process: Pid,
    /// The number of the zygote that forked it.
    zygote: u64,
    socket: OwnedFd,
    pages: ThreadPages,
    memory: OwnedFd,
    memory_key: FileKey,
    /// One bit per thread index the executor holds, from its `new_thread` to its `end_thread`.
    held: Vec<u64>,
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

/// The signal that stops a stub's thread: the kernel's first real-time signal, since a stub holds no C library
/// that claims it.
pub const STOP_SIGNAL: u32 = linux::SIGRTMIN;

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
    /// The stub reported an exit it has no name for.
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
            memory_key: FileKey::new(),
            held,
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
        let until = std::time::Instant::now() + limit;
        loop {
            let seen = word.load(Ordering::Acquire);
            if seen == 0 {
                return Ok(true);
            }
            let left = until.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Ok(false);
            }
            match futex::wait(word, futex::Flags::empty(), seen, Some(&timespec(left))) {
                Ok(()) | Err(Errno::AGAIN | Errno::INTR | Errno::TIMEDOUT) => {}
                Err(other) => return Err(other),
            }
        }
    }

    /// Writes the stub's start into thread 0's control page: the filter, which the stub reads, and the start's turn.
    pub(crate) fn write_start(pages: &ThreadPages) {
        let filter = stub_filter();
        assert!(
            filter.len() * size_of::<u64>() <= crate::layout::THREAD_PAGE_SIZE - control::COMMANDS,
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

    /// The memory memfd's key in the window cache.
    pub fn memory_key(&self) -> FileKey {
        self.memory_key
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
        }
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
        self.pages
            .control64(thread, control::TOUCH_ADDRESS)
            .store(address as u64, Ordering::Relaxed);
        self.pages
            .control64(thread, control::TOUCH_VALUE)
            .store(value, Ordering::Relaxed);
        let turn = self.give(thread, commands, resume)?;
        self.wait(thread, turn)
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

    /// Reads a thread's register block, word by word, as the stub left it (untrusted).
    pub fn read_registers(&self, thread: usize, into: &mut [u8]) {
        let (words, _) = into.as_chunks_mut::<{ size_of::<u64>() }>();
        for (index, word) in words.iter_mut().enumerate() {
            let value = self
                .pages
                .register64(thread, register::REGISTERS + index * size_of::<u64>())
                .load(Ordering::Relaxed);
            *word = value.to_le_bytes();
        }
    }

    /// Replaces a thread's register block, word by word.
    pub fn write_registers(&self, thread: usize, from: &[u8]) {
        let (words, _) = from.as_chunks::<{ size_of::<u64>() }>();
        for (index, word) in words.iter().enumerate() {
            let value = u64::from_le_bytes(*word);
            self.pages
                .register64(thread, register::REGISTERS + index * size_of::<u64>())
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

    /// Waits up to `limit` for a thread to publish the end of `turn`; `None` if it has not.
    pub fn wait_within(
        &self,
        thread: usize,
        turn: u32,
        limit: Duration,
    ) -> Result<Option<TurnEnd>, Errno> {
        let request = self.pages.register32(thread, register::REQUEST);
        let until = std::time::Instant::now() + limit;
        loop {
            let seen = request.load(Ordering::Acquire);
            if seen == turn {
                return Ok(Some(self.exit(thread)));
            }
            let left = until.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            match futex::wait(request, futex::Flags::empty(), seen, Some(&timespec(left))) {
                Ok(()) | Err(Errno::AGAIN | Errno::INTR | Errno::TIMEDOUT) => {}
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
        match ExitKind::from_word(kind) {
            Some(ExitKind::Done) => TurnEnd::Done {
                commands_applied: value,
            },
            Some(ExitKind::CommandRefused) => TurnEnd::CommandRefused {
                index: (value >> 32) as u32,
                errno: Errno::from_raw_os_error(value as u32 as i32),
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
            None => TurnEnd::Forged,
        }
    }

    /// Whether the stub's socket has hung up, which only its end does.
    fn hung_up(&self) -> Result<bool, Errno> {
        let mut socket = [PollFd::new(&self.socket, PollFlags::IN)];
        let ready = rustix::event::poll(&mut socket, Some(&Timespec::default()))?;
        Ok(ready > 0 && socket[0].revents().contains(PollFlags::HUP))
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
