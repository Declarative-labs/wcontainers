//! A stub: one guest memory as a host process forked by the zygote, each of its threads driven through its pages.

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::Ordering;
use std::time::Duration;

use linux_raw_sys::general as linux;
use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::Errno;
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
            let wait = Timespec {
                tv_sec: left.as_secs() as i64,
                tv_nsec: i64::from(left.subsec_nanos()),
            };
            match futex::wait(word, futex::Flags::empty(), seen, Some(&wait)) {
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

    /// Gives a thread a turn that applies `commands` in order and reports back, and waits for its end.
    pub fn turn(&self, thread: usize, commands: &[Command]) -> Result<TurnEnd, Errno> {
        let turn = self.give(thread, commands, Resume::Ping)?;
        self.wait(thread, turn)
    }

    /// Waits for a thread to publish the end of `turn`, looking every 100 ms whether the stub has ended.
    fn wait(&self, thread: usize, turn: u32) -> Result<TurnEnd, Errno> {
        let request = self.pages.register32(thread, register::REQUEST);
        loop {
            let seen = request.load(Ordering::Acquire);
            if seen == turn {
                return Ok(self.exit(thread));
            }
            if let Err(Errno::TIMEDOUT) =
                futex::wait(request, futex::Flags::empty(), seen, Some(&timeout()))
                && self.hung_up()?
            {
                return Ok(TurnEnd::StubEnded);
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
    Timespec {
        tv_sec: 0,
        tv_nsec: LOOK_FOR_END.as_nanos() as i64,
    }
}
