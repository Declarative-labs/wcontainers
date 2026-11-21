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
use wcontainers_kernel::signal_information::PollEvents;

use crate::{host_error, io_host_error};

/// The host's readiness of the sockets it watches, with the thread that waits on it and wakes their waiting calls.
pub struct Readiness {
    shared: Arc<Shared>,
    watcher: Option<JoinHandle<()>>,
}

struct Shared {
    poller: Poller,
    watches: Mutex<BTreeMap<RawFd, Arc<Watch>>>,
    stopping: AtomicBool,
}

/// A socket the host's readiness watches, until it drops and its descriptor closes after its registration.
pub struct Watched {
    watch: Arc<Watch>,
    shared: Arc<Shared>,
    fd: OwnedFd,
}

struct Watch {
    fd: RawFd,
    key: usize,
    state: Mutex<WatchState>,
}

struct WatchState {
    open: bool,
    /// Events the implementation holds for the socket beside the host's, as entries it has taken off a queue.
    held: PollFlags,
    next: u64,
    /// The calls waiting on the socket; its room is kept, so a wait allocates nothing once it has grown.
    waiters: Vec<Waiter>,
    /// The events of the socket's edge-triggered registration in the poller's set, made while unwoken calls wait.
    armed: Option<PollFlags>,
}

struct Waiter {
    id: u64,
    interest: PollFlags,
    /// Taken when the waiter is woken, so that each poll is woken at most once.
    waker: Option<Waker>,
}

/// One call waiting for events of a watched socket; dropping it withdraws the wait.
pub struct WaitForEvents<'a> {
    watched: &'a Watched,
    interest: PollFlags,
    id: Option<u64>,
}

/// The name of the thread that waits on the host's readiness, short enough for Linux to keep whole.
pub const WATCHER: &str = "socket waits";

/// The events poll reports whether or not they are asked for.
const ALWAYS_REPORTED: PollFlags = PollFlags::ERR.union(PollFlags::HUP).union(PollFlags::NVAL);

/// The poll bits that the poller reports as the socket being readable.
#[cfg(target_os = "linux")]
const READING: PollFlags = PollFlags::IN
    .union(PollFlags::RDNORM)
    .union(PollFlags::RDBAND)
    .union(PollFlags::RDHUP)
    .union(PollFlags::from_bits_retain(
        linux_raw_sys::general::POLLMSG as u16,
    ));
#[cfg(not(target_os = "linux"))]
const READING: PollFlags = PollFlags::IN
    .union(PollFlags::RDNORM)
    .union(PollFlags::RDBAND)
    .union(PollFlags::PRI);

/// The poll bits that the poller reports as the socket being writable.
const WRITING: PollFlags = PollFlags::OUT
    .union(PollFlags::WRNORM)
    .union(PollFlags::WRBAND);

/// A poll that answers at once.
const NO_WAIT: Timespec = Timespec {
    tv_sec: 0,
    tv_nsec: 0,
};

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The field of PollEvents that stands for one poll bit.
type PollField = fn(&mut PollEvents) -> &mut bool;

/// The poll bits and the fields of PollEvents that stand for them.
const POLL_BITS: &[(PollFlags, PollField)] = &[
    (PollFlags::IN, |events| &mut events.data_to_read),
    (PollFlags::PRI, |events| &mut events.exceptional_condition),
    (PollFlags::OUT, |events| &mut events.writing_possible),
    (PollFlags::ERR, |events| &mut events.error_condition),
    (PollFlags::HUP, |events| &mut events.hang_up),
    (PollFlags::NVAL, |events| &mut events.descriptor_not_open),
    (PollFlags::RDNORM, |events| &mut events.normal_data_to_read),
    (PollFlags::RDBAND, |events| {
        &mut events.priority_band_data_to_read
    }),
    (PollFlags::WRNORM, |events| &mut events.normal_data_writable),
    (PollFlags::WRBAND, |events| {
        &mut events.priority_data_writable
    }),
    #[cfg(target_os = "linux")]
    (
        PollFlags::from_bits_retain(linux_raw_sys::general::POLLMSG as u16),
        |events| &mut events.message_available,
    ),
    #[cfg(target_os = "linux")]
    (PollFlags::RDHUP, |events| {
        &mut events.peer_shut_down_writing
    }),
];

pub fn poll_flags(mut events: PollEvents) -> PollFlags {
    POLL_BITS
        .iter()
        .filter(|(_, field)| *field(&mut events))
        .fold(PollFlags::empty(), |flags, (bit, _)| flags | *bit)
}

pub const NO_EVENTS: PollEvents = PollEvents {
    data_to_read: false,
    exceptional_condition: false,
    writing_possible: false,
    error_condition: false,
    hang_up: false,
    descriptor_not_open: false,
    normal_data_to_read: false,
    priority_band_data_to_read: false,
    normal_data_writable: false,
    priority_data_writable: false,
    message_available: false,
    peer_shut_down_writing: false,
};

pub fn poll_events(flags: PollFlags) -> PollEvents {
    let mut events = NO_EVENTS;
    for (bit, field) in POLL_BITS {
        *field(&mut events) = flags.contains(*bit);
    }
    events
}

/// The events a socket has now, of those asked for and those always reported.
fn events_now(fd: BorrowedFd<'_>, interest: PollFlags) -> Result<PollFlags, HostError> {
    let mut polled = [PollFd::new(&fd, interest)];
    rustix::event::poll(&mut polled, Some(&NO_WAIT)).map_err(host_error)?;
    Ok(polled[0].revents())
}

/// What the poller is asked to report for poll bits; it reports an error and a hang-up with any of them.
fn registration(key: usize, interest: PollFlags) -> Event {
    let writable = interest.intersects(WRITING);
    // epoll reports an error and a hang-up whatever is asked; kqueue only with the socket's readable or writable events.
    let readable = interest.intersects(READING) || (!cfg!(target_os = "linux") && !writable);
    let mut event = Event::new(key, readable, writable);
    event.set_priority(interest.contains(PollFlags::PRI));
    event
}

impl Readiness {
    pub fn new() -> Result<Self, HostError> {
        let shared = Arc::new(Shared {
            poller: Poller::new().map_err(|error| io_host_error(&error))?,
            watches: Mutex::new(BTreeMap::new()),
            stopping: AtomicBool::new(false),
        });
        let watcher = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name(WATCHER.into())
                .spawn(move || shared.watch_events())
                .map_err(|error| io_host_error(&error))?
        };
        Ok(Self {
            shared,
            watcher: Some(watcher),
        })
    }

    /// Watches a socket from now on; the poller is asked about it only while a call waits on it.
    pub fn watch(&self, fd: OwnedFd) -> Result<Watched, HostError> {
        let raw = fd.as_raw_fd();
        let key = usize::try_from(raw).map_err(|_| HostError::Errno(Errno::EBADF as i32))?;
        let watch = Arc::new(Watch {
            fd: raw,
            key,
            state: Mutex::new(WatchState {
                open: true,
                held: PollFlags::empty(),
                next: u64::MIN,
                waiters: Vec::new(),
                armed: None,
            }),
        });
        locked(&self.shared.watches).insert(raw, Arc::clone(&watch));
        Ok(Watched {
            watch,
            shared: Arc::clone(&self.shared),
            fd,
        })
    }
}

impl Drop for Readiness {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::Release);
        // A watcher that cannot be told to stop is left waiting; it holds only what it shares.
        if self.shared.poller.notify().is_ok()
            && let Some(watcher) = self.watcher.take()
        {
            let _ = watcher.join();
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

    /// Moves the wakers of the calls whose events the socket now has into `woken`, and disarms it once none waits
    /// unwoken.
    fn take_woken(&self, watch: &Watch, woken: &mut Vec<Waker>) {
        let mut state = locked(&watch.state);
        if !state.open || state.armed.is_none() {
            return;
        }
        // SAFETY: the descriptor stays open while the state says so, under its lock.
        let fd = unsafe { BorrowedFd::borrow_raw(watch.fd) };
        if let Some(waiting) = waiting_interest(&state) {
            let now = match events_now(fd, waiting) {
                Ok(now) => now | state.held,
                // A socket whose events cannot be read wakes every call, which reads the error itself.
                Err(_) => PollFlags::all(),
            };
            for waiter in &mut state.waiters {
                if now.intersects(waiter.interest | ALWAYS_REPORTED)
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

    /// Registers the socket for the events of the calls that wait unwoken, if any do and it is not already.
    fn arm(&self, watch: &Watch, state: &mut WatchState) -> Result<(), HostError> {
        let Some(interest) = waiting_interest(state) else {
            return Ok(());
        };
        if state.armed.is_some_and(|armed| armed.contains(interest)) {
            return Ok(());
        }
        // Edge-triggered, so that a call waiting for an event the poller can only approximate, as POLLRDBAND by
        // the socket being readable, is looked at again at the socket's next change and not at once forever.
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

/// The events the calls waiting unwoken on a socket ask for, or none when no call waits unwoken.
fn waiting_interest(state: &WatchState) -> Option<PollFlags> {
    state
        .waiters
        .iter()
        .filter(|waiter| waiter.waker.is_some())
        .map(|waiter| waiter.interest)
        .reduce(|all, interest| all | interest)
}

/// Wakes the calls whose events have come, the one place a waiting call's wake is fired.
fn wake(woken: &mut impl Iterator<Item = Waker>) {
    for waker in woken {
        waker.wake();
    }
}

impl Watched {
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// Sets the events the implementation holds for the socket beside the host's, waking the calls they finish.
    pub fn hold(&self, events: PollEvents) {
        let held = poll_flags(events);
        locked(&self.watch.state).held = held;
        // One waker at a time, each woken outside the lock.
        loop {
            let waker = locked(&self.watch.state)
                .waiters
                .iter_mut()
                .filter(|waiter| held.intersects(waiter.interest | ALWAYS_REPORTED))
                .find_map(|waiter| waiter.waker.take());
            match waker {
                Some(waker) => wake(&mut core::iter::once(waker)),
                None => return,
            }
        }
    }

    /// The events the socket has now, of those asked for and those always reported.
    pub fn events_now(&self, interest: PollEvents) -> Result<PollEvents, HostError> {
        let interest = poll_flags(interest);
        let held = locked(&self.watch.state).held;
        let now = events_now(self.fd(), interest)?;
        Ok(poll_events(now | (held & (interest | ALWAYS_REPORTED))))
    }

    /// A call that finishes when the socket has one of the events asked for, giving the events it has.
    pub fn wait_for_events(&self, interest: PollEvents) -> WaitForEvents<'_> {
        WaitForEvents {
            watched: self,
            interest: poll_flags(interest),
            id: None,
        }
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        {
            let mut state = locked(&self.watch.state);
            state.open = false;
            self.shared.disarm(self.fd.as_fd(), &mut state);
        }
        locked(&self.shared.watches).remove(&self.watch.fd);
    }
}

impl WaitForEvents<'_> {
    fn withdraw(&mut self) {
        if let Some(id) = self.id.take() {
            let mut state = locked(&self.watched.watch.state);
            state.waiters.retain(|waiter| waiter.id != id);
            // The last call waiting unwoken takes the registration with it.
            if waiting_interest(&state).is_none() {
                self.watched.shared.disarm(self.watched.fd(), &mut state);
            }
        }
    }

    /// The events the socket has now, withdrawing the wait, or none yet.
    fn answer(&mut self) -> Option<Result<PollEvents, HostError>> {
        let held = locked(&self.watched.watch.state).held & (self.interest | ALWAYS_REPORTED);
        match events_now(self.watched.fd(), self.interest).map(|now| now | held) {
            Ok(now) if now.is_empty() => None,
            answer => {
                self.withdraw();
                Some(answer.map(poll_events))
            }
        }
    }
}

impl Future for WaitForEvents<'_> {
    type Output = Result<PollEvents, HostError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        // A call polled again whose events have come finishes before it gives a waker that a later event would wake.
        if this.id.is_some()
            && let Some(answer) = this.answer()
        {
            return Poll::Ready(answer);
        }
        {
            // Listed before the socket is asked, so an event after the asking finds the waiter.
            let mut state = locked(&this.watched.watch.state);
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
        // Armed after the asking: a registration made or changed reports an event already there.
        let armed = {
            let mut state = locked(&this.watched.watch.state);
            this.watched.shared.arm(&this.watched.watch, &mut state)
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

impl Drop for WaitForEvents<'_> {
    fn drop(&mut self) {
        self.withdraw();
    }
}
