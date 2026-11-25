//! The calls the kernel waits on its system for. Each is the system's own future, kept in a cell
//! of its own until it finishes and polled only here, after the waker that names its cell fired.
//! Cells are made once and used again, and a call that brings bytes back brings them in a buffer
//! it was given, kept for the next call once read, so a call that waits allocates nothing.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::task::Wake;
use alloc::vec::Vec;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};

use smallbox::SmallBox;

/// What a waiting call is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WaitFor {
    /// A file backend request.
    Backend(crate::fs::backend::Sent),
    /// A name query of the container's name server, by its number.
    Name(u64),
    /// A TCP connection or listener to the outside becoming ready, by its number.
    Ready(u32),
    /// A stream endpoint of an end becoming ready, by the end's number.
    Stream(crate::ends::EndNumber),
    /// The file backend's next notice.
    Notice,
}

/// What a finished waiting call gives the kernel.
pub(crate) enum Answer {
    /// The backend's reply, as its error number (zero for none), and the request's own buffer,
    /// which now holds the reply's body.
    Backend(Result<i32, wcontainers_abi::HostError>, Vec<u8>),
    /// Whether the resolver answered, and the buffer the call was given, which then holds the
    /// answer's message.
    Name(Result<(), wcontainers_abi::HostError>, Vec<u8>),
    /// What a connection, listener or stream endpoint is ready for.
    Ready(Result<wcontainers_readiness::ReadinessReport, wcontainers_abi::HostError>),
    /// Whether the backend sent a notice, and the buffer the call was given, which then holds it.
    Notice(Result<(), wcontainers_abi::HostError>, Vec<u8>),
}

/// Reads a finished wait on a handle's readiness, a failed wait as an error on the handle.
pub(crate) fn report_of(
    ready: Result<wcontainers_readiness::ReadinessReport, wcontainers_abi::HostError>,
) -> wcontainers_readiness::ReadinessReport {
    ready.unwrap_or(wcontainers_readiness::ReadinessReport {
        ready_for: wcontainers_readiness::ReadyFor::empty(),
        unconditional: wcontainers_readiness::UnconditionalEvents::ERROR,
    })
}

/// The room a waiting call has in its cell; a larger one is kept on the heap (counted).
pub(crate) type Room = smallbox::space::S32;

/// A waiting call, made with [`waiting_call`].
pub(crate) type Call = SmallBox<dyn Future<Output = Answer> + Send, Room>;

/// The waiting call `future`.
pub(crate) fn waiting_call(future: impl Future<Output = Answer> + Send + 'static) -> Call {
    SmallBox::new(future)
}

/// The waker of one cell: it marks the cell, and the kernel's note that some cell is marked.
struct Mark {
    cell: AtomicBool,
    any: Arc<AtomicBool>,
}

impl Wake for Mark {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.cell.store(true, Ordering::Release);
        self.any.store(true, Ordering::Release);
    }
}

struct Cell {
    call: Pin<Box<Option<Call>>>,
    mark: Arc<Mark>,
    waker: Waker,
}

impl Cell {
    fn poll(&mut self) -> Poll<Answer> {
        let mut context = Context::from_waker(&self.waker);
        match self.call.as_mut().as_pin_mut() {
            Some(call) => call.poll(&mut context),
            None => Poll::Pending,
        }
    }
}

impl crate::Kernel {
    /// A call the kernel waited on has its answer.
    pub(crate) fn waiting_answered<S: crate::System>(
        &mut self,
        cx: &mut crate::Context<'_, S>,
        about: WaitFor,
        answer: Answer,
    ) {
        match (about, answer) {
            (WaitFor::Backend(sent), Answer::Backend(answer, bytes)) => {
                self.backend_answered(cx, sent, answer, bytes);
            }
            (WaitFor::Name(number), Answer::Name(answer, bytes)) => {
                crate::syscall::socket::name_answered(self, cx, number, answer, bytes);
            }
            (WaitFor::Ready(id), Answer::Ready(ready)) => {
                crate::syscall::socket::outside_ready(self, cx, id, ready);
            }
            (WaitFor::Stream(end), Answer::Ready(ready)) => {
                self.end_ready(cx, end, ready);
            }
            (WaitFor::Notice, Answer::Notice(notice, bytes)) => {
                self.notice_came(cx, notice, bytes);
            }
            (about, _) => {
                debug_assert!(false, "{about:?} answered as another call");
            }
        }
    }
}

/// Every call the kernel waits on, and the cells and buffers kept for the next ones.
#[derive(Default)]
pub(crate) struct Waiting {
    live: Vec<(WaitFor, Cell)>,
    free: Vec<Cell>,
    any: Arc<AtomicBool>,
    /// Buffers answers came back in, read and kept for the next calls'.
    spare: Vec<Vec<u8>>,
}

impl Waiting {
    /// An empty buffer for a call to bring its answer back in, one kept if there is one.
    pub(crate) fn buffer(&mut self) -> Vec<u8> {
        self.spare.pop().unwrap_or_default()
    }

    /// Keeps a buffer an answer came back in, once read, for a later call.
    pub(crate) fn reuse(&mut self, mut buffer: Vec<u8>) {
        buffer.clear();
        self.spare.push(buffer);
    }

    /// Whether a waiting call's waker fired since the last [`Waiting::next_answer`] found none.
    pub(crate) fn woken(&self) -> bool {
        !self.live.is_empty() && self.any.load(Ordering::Acquire)
    }

    fn cell(&mut self) -> Cell {
        self.free.pop().unwrap_or_else(|| {
            let mark = Arc::new(Mark {
                cell: AtomicBool::new(false),
                any: self.any.clone(),
            });
            Cell {
                call: Box::pin(None),
                waker: Waker::from(mark.clone()),
                mark,
            }
        })
    }

    fn recycle(&mut self, mut cell: Cell) {
        cell.call.set(None);
        cell.mark.cell.store(false, Ordering::Relaxed);
        self.free.push(cell);
    }

    /// Starts waiting on `call`: its answer if it has one at once, else it is kept until woken.
    pub(crate) fn start(&mut self, about: WaitFor, call: Call) -> Option<Answer> {
        let mut cell = self.cell();
        cell.call.set(Some(call));
        match cell.poll() {
            Poll::Ready(answer) => {
                self.recycle(cell);
                Some(answer)
            }
            Poll::Pending => {
                self.live.push((about, cell));
                None
            }
        }
    }

    /// Starts waiting on `call` as if already woken, so that its first poll, and any answer it has
    /// at once, comes when the kernel next settles.
    pub(crate) fn start_woken(&mut self, about: WaitFor, call: Call) {
        let mut cell = self.cell();
        cell.call.set(Some(call));
        cell.mark.wake_by_ref();
        self.live.push((about, cell));
    }

    /// The answer of a woken call that has finished, and what it was for.
    pub(crate) fn next_answer(&mut self) -> Option<(WaitFor, Answer)> {
        if !self.any.swap(false, Ordering::AcqRel) {
            return None;
        }
        let mut index = 0;
        while index < self.live.len() {
            let cell = &mut self.live[index].1;
            if cell.mark.cell.swap(false, Ordering::AcqRel)
                && let Poll::Ready(answer) = cell.poll()
            {
                let (about, cell) = self.live.swap_remove(index);
                self.recycle(cell);
                if self
                    .live
                    .iter()
                    .any(|(_, cell)| cell.mark.cell.load(Ordering::Acquire))
                {
                    self.any.store(true, Ordering::Release);
                }
                return Some((about, answer));
            }
            index += 1;
        }
        None
    }

    /// The answer of the kept call `about`, if polling it now, woken or not, finds one.
    pub(crate) fn poll_now(&mut self, about: WaitFor) -> Option<Answer> {
        let index = self.live.iter().position(|(kept, _)| *kept == about)?;
        let Poll::Ready(answer) = self.live[index].1.poll() else {
            return None;
        };
        let (_, cell) = self.live.swap_remove(index);
        self.recycle(cell);
        Some(answer)
    }

    /// Drops the calls `which` names, unanswered: each is cancelled as its system's future says.
    pub(crate) fn drop_where(&mut self, mut which: impl FnMut(&WaitFor) -> bool) {
        let mut index = 0;
        while index < self.live.len() {
            if which(&self.live[index].0) {
                let (_, cell) = self.live.swap_remove(index);
                self.recycle(cell);
            } else {
                index += 1;
            }
        }
    }

    /// Whether a call kept here waits for its answer on the heap, too large for its cell's room.
    pub(crate) fn on_heap(call: &Call) -> bool {
        call.is_heap()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crossbeam_utils::atomic::AtomicCell as Shared;

    extern crate std;

    /// The allocations `run` makes on this thread.
    pub(crate) fn allocations_of(run: impl FnOnce()) -> u64 {
        allocation_counter::measure(run).count_total
    }

    /// What a test system's call waits behind: shut, it keeps the waker it was last polled with;
    /// opened, it wakes that waker and lets every call through.
    #[derive(Default)]
    pub(crate) struct Door {
        open: AtomicBool,
        waker: std::sync::Mutex<Option<Waker>>,
    }

    impl Door {
        /// Waits until the door is open.
        pub(crate) async fn pass(&self) {
            core::future::poll_fn(|context| {
                if self.open.load(Ordering::Acquire) {
                    return Poll::Ready(());
                }
                *self.waker.lock().expect("a door") = Some(context.waker().clone());
                Poll::Pending
            })
            .await;
        }

        pub(crate) fn open(&self) {
            self.open.store(true, Ordering::Release);
            let waker = self.waker.lock().expect("a door").take();
            if let Some(waker) = waker {
                waker.wake();
            }
        }

        pub(crate) fn shut(&self) {
            self.open.store(false, Ordering::Release);
        }
    }

    /// What a test's call `number` waits for.
    fn about(number: u64) -> WaitFor {
        WaitFor::Backend(crate::fs::backend::Sent {
            unique: number,
            tid: None,
            opcode: 0,
            nodeid: 0,
            at: 0,
            answered: true,
        })
    }

    /// A future that finishes once `ready` is set, keeping the waker it was last given.
    struct Gate<'a> {
        ready: &'a Shared<bool>,
        waker: &'a atomic_refcell::AtomicRefCell<Option<Waker>>,
    }

    impl Future for Gate<'_> {
        type Output = Answer;

        fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Answer> {
            if self.ready.load() {
                return Poll::Ready(Answer::Backend(
                    Err(wcontainers_abi::HostError::Closed),
                    Vec::new(),
                ));
            }
            *self.waker.borrow_mut() = Some(context.waker().clone());
            Poll::Pending
        }
    }

    #[test]
    fn a_call_answered_at_once_is_not_kept() {
        let mut waiting = Waiting::default();
        let answer = waiting.start(
            about(1),
            waiting_call(async {
                Answer::Backend(Err(wcontainers_abi::HostError::Closed), Vec::new())
            }),
        );
        assert!(answer.is_some());
        assert!(waiting.live.is_empty());
        assert!(!waiting.woken());
    }

    #[test]
    fn a_woken_call_is_answered_and_its_cell_used_again() {
        let ready: &'static Shared<bool> = Box::leak(Box::new(Shared::new(false)));
        let waker: &'static atomic_refcell::AtomicRefCell<Option<Waker>> =
            Box::leak(Box::default());
        let mut waiting = Waiting::default();
        for round in 0..3 {
            ready.store(false);
            let gate = Gate { ready, waker };
            assert!(waiting.start(about(round), waiting_call(gate)).is_none());
            assert!(!waiting.woken());
            assert!(waiting.next_answer().is_none());
            ready.store(true);
            waker.borrow_mut().take().expect("a waker").wake();
            assert!(waiting.woken());
            let (answered, _) = waiting.next_answer().expect("answered");
            assert_eq!(answered, about(round));
            assert!(waiting.live.is_empty());
            assert_eq!(waiting.free.len(), 1);
        }
    }

    #[test]
    fn a_call_polled_now_answers_before_its_waker_fires_and_only_itself() {
        let ready: &'static Shared<bool> = Box::leak(Box::new(Shared::new(false)));
        let waker: &'static atomic_refcell::AtomicRefCell<Option<Waker>> =
            Box::leak(Box::default());
        let other: &'static Shared<bool> = Box::leak(Box::new(Shared::new(false)));
        let other_waker: &'static atomic_refcell::AtomicRefCell<Option<Waker>> =
            Box::leak(Box::default());
        let mut waiting = Waiting::default();
        assert!(
            waiting
                .start(about(1), waiting_call(Gate { ready, waker }))
                .is_none()
        );
        let gate = Gate {
            ready: other,
            waker: other_waker,
        };
        assert!(waiting.start(about(2), waiting_call(gate)).is_none());
        assert!(waiting.poll_now(about(1)).is_none());
        assert!(waiting.poll_now(about(3)).is_none());
        ready.store(true);
        other.store(true);
        assert!(!waiting.woken());
        assert!(matches!(
            waiting.poll_now(about(1)),
            Some(Answer::Backend(Err(wcontainers_abi::HostError::Closed), _))
        ));
        assert!(waiting.poll_now(about(1)).is_none());
        assert_eq!(waiting.live.len(), 1);
        assert_eq!(waiting.live[0].0, about(2));
    }

    #[test]
    fn a_call_that_waits_allocates_nothing_once_its_cell_exists() {
        let ready: &'static Shared<bool> = Box::leak(Box::new(Shared::new(false)));
        let waker: &'static atomic_refcell::AtomicRefCell<Option<Waker>> =
            Box::leak(Box::default());
        let mut waiting = Waiting::default();
        let mut round = |number| {
            ready.store(false);
            waiting.start(about(number), waiting_call(Gate { ready, waker }));
            ready.store(true);
            waker.borrow_mut().take().expect("a waker").wake();
            waiting.next_answer().expect("answered");
        };
        round(0);
        let counted = allocations_of(|| {
            for number in 1..100 {
                round(number);
            }
        });
        assert_eq!(counted, 0);
    }
}
