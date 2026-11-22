//! The host's readiness of sockets, watched by one thread that wakes the calls waiting on them.

use std::collections::BTreeMap;
use std::future::Future;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};
use std::thread::JoinHandle;

use polling::{Event, Events, PollMode, Poller};
use rustix::event::{PollFd, PollFlags, Timespec};
use wcontainers_abi::HostError;
use wcontainers_kernel::errno::Errno;
use wcontainers_readiness::{ReadinessReport, ReadyFor, UnconditionalEvents};

use crate::{host_error, io_host_error};

/// The host's readiness of the sockets it watches, with the thread that waits on it and wakes their waiting calls.
pub struct Watcher {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    poller: Poller,
    watches: Mutex<BTreeMap<RawFd, Arc<Watch>>>,
    stopping: AtomicBool,
}

/// A socket the host's readiness watches, until it drops and its descriptor closes after its registration.
pub struct Watched {
    registration: Registration,
    fd: OwnedFd,
}

/// A watched socket's place in the watcher, withdrawn when it drops, before the socket's descriptor closes.
struct Registration {
    watch: Arc<Watch>,
    shared: Arc<Shared>,
}

struct Watch {
    fd: RawFd,
    key: usize,
    /// The facts this kind of socket can be ready for; one outside them is never asked of the host or reported.
    possible: ReadyFor,
    state: Mutex<WatchState>,
}

struct WatchState {
    open: bool,
    /// Facts the implementation holds for the socket beside the host's, as entries it has taken off a queue.
    held: ReadinessReport,
    next: u64,
    /// The calls waiting on the socket; its room is kept, so a wait allocates nothing once it has grown.
    waiters: Vec<Waiter>,
    /// The facts of the socket's edge-triggered registration in the poller's set, made while unwoken calls wait.
    armed: Option<ReadyFor>,
}

struct Waiter {
    id: u64,
    interest: ReadyFor,
    /// Taken when the waiter is woken, so that each poll is woken at most once.
    waker: Option<Waker>,
}

/// One call waiting until a watched socket is ready; dropping it withdraws the wait.
pub struct WaitUntilReady<'a> {
    watched: &'a Watched,
    interest: ReadyFor,
    id: Option<u64>,
}

/// The name of the thread that waits on the host's readiness, short enough for Linux to keep whole.
pub const WATCHER_THREAD: &str = "socket waits";

/// The facts a datagram socket can be ready for: it has no urgent data and no peer that finishes sending.
pub const DATAGRAM_FACTS: ReadyFor = ReadyFor::READABLE.union(ReadyFor::WRITABLE);

/// A report of no fact at all.
pub const NOTHING: ReadinessReport = ReadinessReport {
    ready_for: ReadyFor::empty(),
    unconditional: UnconditionalEvents::empty(),
};

/// A report of every fact, given when a socket's facts cannot be read, so that every call reads the error itself.
const EVERYTHING: ReadinessReport = ReadinessReport {
    ready_for: ReadyFor::all(),
    unconditional: UnconditionalEvents::all(),
};

/// Each fact that can be asked for and the host's poll bit for it.
const POLL_BITS: &[(ReadyFor, PollFlags)] = &[
    (ReadyFor::READABLE, PollFlags::IN),
    (ReadyFor::WRITABLE, PollFlags::OUT),
    (ReadyFor::URGENT_DATA, PollFlags::PRI),
    #[cfg(target_os = "linux")]
    (ReadyFor::PEER_FINISHED_SENDING, PollFlags::RDHUP),
];

/// Each event reported whether or not it is asked for and the host's poll bit for it.
const UNCONDITIONAL_BITS: &[(UnconditionalEvents, PollFlags)] = &[
    (UnconditionalEvents::ERROR, PollFlags::ERR),
    (UnconditionalEvents::HANG_UP, PollFlags::HUP),
];

/// A poll that answers at once.
///
/// https://man7.org/linux/man-pages/man2/poll.2.html
const NO_WAIT: Timespec = Timespec {
    tv_sec: 0,
    tv_nsec: 0,
};

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn poll_flags(facts: ReadyFor) -> PollFlags {
    POLL_BITS
        .iter()
        .filter(|(fact, _)| facts.contains(*fact))
        .fold(PollFlags::empty(), |flags, (_, bit)| flags | *bit)
}

fn facts(flags: PollFlags) -> ReadinessReport {
    ReadinessReport {
        ready_for: POLL_BITS
            .iter()
            .filter(|(_, bit)| flags.contains(*bit))
            .fold(ReadyFor::empty(), |facts, (fact, _)| facts | *fact),
        unconditional: UNCONDITIONAL_BITS
            .iter()
            .filter(|(_, bit)| flags.contains(*bit))
            .fold(UnconditionalEvents::empty(), |events, (event, _)| {
                events | *event
            }),
    }
}

/// Gives both reports' facts together.
fn union(one: ReadinessReport, other: ReadinessReport) -> ReadinessReport {
    ReadinessReport {
        ready_for: one.ready_for | other.ready_for,
        unconditional: one.unconditional | other.unconditional,
    }
}

/// Tells whether a report ends a wait for the interest: a fact asked for, or any unconditional event.
fn ends(report: ReadinessReport, interest: ReadyFor) -> bool {
    report.ready_for.intersects(interest) || !report.unconditional.is_empty()
}

/// Gives the facts a socket has now, of those asked for and those always reported.
fn facts_now(fd: BorrowedFd<'_>, interest: ReadyFor) -> Result<ReadinessReport, HostError> {
    let mut polled = [PollFd::new(&fd, poll_flags(interest))];
    rustix::event::poll(&mut polled, Some(&NO_WAIT)).map_err(host_error)?;
    Ok(facts(polled[0].revents()))
}

/// Gives what the poller is asked to report for facts; it reports an error and a hang-up with any of them.
fn registration(key: usize, interest: ReadyFor) -> Event {
    let writable = interest.contains(ReadyFor::WRITABLE);
    // No peer shutdown is registrable, but it makes the socket readable; kqueue reports errors and hang-ups only with either.
    let readable = interest.intersects(ReadyFor::READABLE | ReadyFor::PEER_FINISHED_SENDING)
        || (!cfg!(target_os = "linux") && !writable);
    let mut event = Event::new(key, readable, writable);
    event.set_priority(interest.contains(ReadyFor::URGENT_DATA));
    event
}

impl Watcher {
    pub fn new() -> Result<Self, HostError> {
        let shared = Arc::new(Shared {
            poller: Poller::new().map_err(|error| io_host_error(&error))?,
            watches: Mutex::new(BTreeMap::new()),
            stopping: AtomicBool::new(false),
        });
        let thread = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name(WATCHER_THREAD.into())
                .spawn(move || shared.watch_events())
                .map_err(|error| io_host_error(&error))?
        };
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Watches a socket that can have the facts given; the poller is asked about it only while a call waits on it.
    pub fn watch(&self, fd: OwnedFd, possible: ReadyFor) -> Result<Watched, HostError> {
        let raw = fd.as_raw_fd();
        let key = usize::try_from(raw).map_err(|_| HostError::Errno(Errno::EBADF as i32))?;
        let watch = Arc::new(Watch {
            fd: raw,
            key,
            possible,
            state: Mutex::new(WatchState {
                open: true,
                held: NOTHING,
                next: u64::MIN,
                waiters: Vec::new(),
                armed: None,
            }),
        });
        locked(&self.shared.watches).insert(raw, Arc::clone(&watch));
        Ok(Watched {
            registration: Registration {
                watch,
                shared: Arc::clone(&self.shared),
            },
            fd,
        })
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::Release);
        // A watcher that cannot be told to stop is left waiting; it holds only what it shares.
        if self.shared.poller.notify().is_ok()
            && let Some(thread) = self.thread.take()
        {
            let _ = thread.join();
        }
    }
}

impl Shared {
    fn watch_events(&self) {
        let mut events = Events::new();
        let mut woken = Vec::new();
        while !self.stopping.load(Ordering::Acquire) {
            events.clear();
            // An interrupted wait is waited again.
            if self.poller.wait(&mut events, None).is_err() {
                continue;
            }
            for event in events.iter() {
                let Ok(fd) = RawFd::try_from(event.key) else {
                    continue;
                };
                let watch = locked(&self.watches).get(&fd).cloned();
                if let Some(watch) = watch {
                    self.take_woken(&watch, &mut woken);
                }
                wake(&mut woken.drain(..));
            }
        }
    }

    /// Moves the wakers of the calls whose facts the socket now has into `woken`, disarming it once none waits unwoken.
    fn take_woken(&self, watch: &Watch, woken: &mut Vec<Waker>) {
        let mut state = locked(&watch.state);
        if !state.open || state.armed.is_none() {
            return;
        }
        // SAFETY: the descriptor stays open while the state says so, under its lock.
        let fd = unsafe { BorrowedFd::borrow_raw(watch.fd) };
        if let Some(waiting) = waiting_interest(&state) {
            let now = match facts_now(fd, waiting) {
                Ok(now) => union(now, state.held),
                Err(_) => EVERYTHING,
            };
            for waiter in &mut state.waiters {
                if ends(now, waiter.interest)
                    && let Some(waker) = waiter.waker.take()
                {
                    woken.push(waker);
                }
            }
        }
        // An edge-triggered registration reports every change until it is deleted, which a socket whose calls
        // were all woken no longer needs.
        if waiting_interest(&state).is_none() {
            self.disarm(fd, &mut state);
        }
    }

    /// Registers the socket for the facts of the calls that wait unwoken, if any do and it is not already.
    fn arm(&self, watch: &Watch, state: &mut WatchState) -> Result<(), HostError> {
        let Some(interest) = waiting_interest(state) else {
            return Ok(());
        };
        if state.armed.is_some_and(|armed| armed.contains(interest)) {
            return Ok(());
        }
        // Edge-triggered, so a registration wider than the facts waited for, as readable for a peer's shutdown, is
        // looked at once per change.
        let event = registration(watch.key, interest);
        let armed = if state.armed.is_some() {
            // SAFETY: the descriptor stays open while the state says so, under its lock.
            let fd = unsafe { BorrowedFd::borrow_raw(watch.fd) };
            self.poller.modify_with_mode(fd, event, PollMode::Edge)
        } else {
            // SAFETY: the registration is deleted when the Watched drops, before the descriptor it owns closes.
            unsafe { self.poller.add_with_mode(watch.fd, event, PollMode::Edge) }
        };
        armed.map_err(|error| io_host_error(&error))?;
        state.armed = Some(interest);
        Ok(())
    }

    /// Takes the socket out of the poller's set, so that it reports nothing for a socket no call waits on.
    fn disarm(&self, fd: BorrowedFd<'_>, state: &mut WatchState) {
        if state.armed.is_some() && self.poller.delete(fd).is_ok() {
            state.armed = None;
        }
    }
}

/// Gives the facts the calls waiting unwoken on a socket ask for, or none when no call waits unwoken.
fn waiting_interest(state: &WatchState) -> Option<ReadyFor> {
    state
        .waiters
        .iter()
        .filter(|waiter| waiter.waker.is_some())
        .map(|waiter| waiter.interest)
        .reduce(|all, interest| all | interest)
}

/// Wakes the calls whose facts have come, the one place a waiting call's wake is fired.
fn wake(woken: &mut impl Iterator<Item = Waker>) {
    for waker in woken {
        waker.wake();
    }
}

impl Watched {
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// Sets the facts the implementation holds for the socket beside the host's, waking the calls they finish.
    pub fn hold(&self, held: ReadinessReport) {
        locked(&self.registration.watch.state).held = held;
        // One waker at a time, each woken outside the lock.
        loop {
            let waker = locked(&self.registration.watch.state)
                .waiters
                .iter_mut()
                .filter(|waiter| ends(held, waiter.interest))
                .find_map(|waiter| waiter.waker.take());
            match waker {
                Some(waker) => wake(&mut core::iter::once(waker)),
                None => return,
            }
        }
    }

    /// Stops watching the socket and gives it back open.
    pub fn unwatch(self) -> OwnedFd {
        let Self { registration, fd } = self;
        drop(registration);
        fd
    }

    /// Makes a call that finishes when the socket has one of the facts asked for, giving every fact it has.
    pub fn wait_until_ready(&self, interest: ReadyFor) -> WaitUntilReady<'_> {
        WaitUntilReady {
            watched: self,
            interest: interest & self.registration.watch.possible,
            id: None,
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        {
            let mut state = locked(&self.watch.state);
            state.open = false;
            // SAFETY: a Watched's descriptor closes after its registration drops.
            let fd = unsafe { BorrowedFd::borrow_raw(self.watch.fd) };
            self.shared.disarm(fd, &mut state);
        }
        locked(&self.shared.watches).remove(&self.watch.fd);
    }
}

impl WaitUntilReady<'_> {
    fn withdraw(&mut self) {
        if let Some(id) = self.id.take() {
            let mut state = locked(&self.watched.registration.watch.state);
            state.waiters.retain(|waiter| waiter.id != id);
            // The last call waiting unwoken takes the registration with it.
            if waiting_interest(&state).is_none() {
                self.watched
                    .registration
                    .shared
                    .disarm(self.watched.fd(), &mut state);
            }
        }
    }

    /// Gives every fact the socket has once one asked for or always reported holds, withdrawing the wait, or none yet.
    fn answer(&mut self) -> Option<Result<ReadinessReport, HostError>> {
        let watch = &self.watched.registration.watch;
        let held = locked(&watch.state).held;
        match facts_now(self.watched.fd(), watch.possible).map(|now| union(now, held)) {
            Ok(now) if !ends(now, self.interest) => None,
            answer => {
                self.withdraw();
                Some(answer)
            }
        }
    }
}

impl Future for WaitUntilReady<'_> {
    type Output = Result<ReadinessReport, HostError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        // A call polled again whose facts have come finishes before it gives a waker that a later event would wake.
        if this.id.is_some()
            && let Some(answer) = this.answer()
        {
            return Poll::Ready(answer);
        }
        {
            // Listed before the socket is asked, so a fact after the asking finds the waiter.
            let mut state = locked(&this.watched.registration.watch.state);
            match this.id {
                None => {
                    let id = state.next;
                    state.next = state.next.wrapping_add(1);
                    state.waiters.push(Waiter {
                        id,
                        interest: this.interest,
                        waker: Some(context.waker().clone()),
                    });
                    this.id = Some(id);
                }
                Some(id) => {
                    if let Some(waiter) = state.waiters.iter_mut().find(|waiter| waiter.id == id)
                        && !waiter
                            .waker
                            .as_ref()
                            .is_some_and(|waker| waker.will_wake(context.waker()))
                    {
                        waiter.waker = Some(context.waker().clone());
                    }
                }
            }
        }
        if let Some(answer) = this.answer() {
            return Poll::Ready(answer);
        }
        // Armed after the asking: a registration made or changed reports a fact already there.
        let armed = {
            let mut state = locked(&this.watched.registration.watch.state);
            this.watched
                .registration
                .shared
                .arm(&this.watched.registration.watch, &mut state)
        };
        match armed {
            Ok(()) => Poll::Pending,
            Err(error) => {
                this.withdraw();
                Poll::Ready(Err(error))
            }
        }
    }
}

impl Drop for WaitUntilReady<'_> {
    fn drop(&mut self) {
        self.withdraw();
    }
}
