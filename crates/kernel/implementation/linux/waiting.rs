//! The calls the kernel waits on its system for. Each is the system's own future, kept in a cell
//! of its own until it finishes and polled only here, after the waker that names its cell fired.
//! Cells are made once and used again, so a call that waits allocates nothing.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::task::Wake;
use alloc::vec::Vec;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};

use smallbox::SmallBox;

pub(crate) use smallbox::smallbox as waiting_call;

/// What a waiting call is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WaitFor {
    /// A file backend request.
    Backend(crate::fs::backend::Sent),
}

/// What a finished waiting call gives the kernel.
pub(crate) enum Answer {
    /// The backend's reply, as its error number (zero for none) and its body.
    Backend(Result<(i32, alloc::rc::Rc<[u8]>), wcontainers_abi::HostError>),
}

/// The room a waiting call has in its cell; a larger one is kept on the heap (counted).
pub(crate) type Room = smallbox::space::S32;

/// A waiting call, made with [`waiting_call`].
pub(crate) type Call = SmallBox<dyn Future<Output = Answer>, Room>;

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
            (WaitFor::Backend(sent), Answer::Backend(answer)) => {
                self.backend_answered(cx, sent, answer);
            }
        }
    }
}

/// Every call the kernel waits on, and the cells kept for the next ones.
#[derive(Default)]
pub(crate) struct Waiting {
    live: Vec<(WaitFor, Cell)>,
    free: Vec<Cell>,
    any: Arc<AtomicBool>,
}

impl Waiting {
    /// Whether a waiting call's waker fired since the last [`Waiting::next_answer`] found none.
    pub fn woken(&self) -> bool {
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
    pub fn start(&mut self, about: WaitFor, call: Call) -> Option<Answer> {
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

    /// The answer of a woken call that has finished, and what it was for.
    pub fn next_answer(&mut self) -> Option<(WaitFor, Answer)> {
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

    /// Whether a call kept here waits for its answer on the heap, too large for its cell's room.
    pub fn on_heap(call: &Call) -> bool {
        call.is_heap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell as Shared;

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
        waker: &'a core::cell::RefCell<Option<Waker>>,
    }

    impl Future for Gate<'_> {
        type Output = Answer;

        fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Answer> {
            if self.ready.get() {
                return Poll::Ready(Answer::Backend(Err(wcontainers_abi::HostError::Closed)));
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
            waiting_call!(async { Answer::Backend(Err(wcontainers_abi::HostError::Closed)) }),
        );
        assert!(answer.is_some());
        assert!(waiting.live.is_empty());
        assert!(!waiting.woken());
    }

    #[test]
    fn a_woken_call_is_answered_and_its_cell_used_again() {
        let ready: &'static Shared<bool> = Box::leak(Box::new(Shared::new(false)));
        let waker: &'static core::cell::RefCell<Option<Waker>> = Box::leak(Box::default());
        let mut waiting = Waiting::default();
        for round in 0..3 {
            ready.set(false);
            let gate = Gate { ready, waker };
            assert!(waiting.start(about(round), waiting_call!(gate)).is_none());
            assert!(!waiting.woken());
            assert!(waiting.next_answer().is_none());
            ready.set(true);
            waker.borrow_mut().take().expect("a waker").wake();
            assert!(waiting.woken());
            let (answered, _) = waiting.next_answer().expect("answered");
            assert_eq!(answered, about(round));
            assert!(waiting.live.is_empty());
            assert_eq!(waiting.free.len(), 1);
        }
    }

    extern crate std;

    /// Counts the allocations of the thread that makes them.
    struct Counting;

    std::thread_local! {
        static ALLOCATIONS: Shared<u64> = const { Shared::new(0) };
    }

    // SAFETY: every call goes to the system allocator unchanged.
    unsafe impl core::alloc::GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
            // SAFETY: as the caller's contract.
            unsafe { std::alloc::System.alloc(layout) }
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: core::alloc::Layout) {
            // SAFETY: as the caller's contract.
            unsafe { std::alloc::System.dealloc(pointer, layout) }
        }
    }

    #[global_allocator]
    static COUNTING: Counting = Counting;

    #[test]
    fn a_call_that_waits_allocates_nothing_once_its_cell_exists() {
        let ready: &'static Shared<bool> = Box::leak(Box::new(Shared::new(false)));
        let waker: &'static core::cell::RefCell<Option<Waker>> = Box::leak(Box::default());
        let mut waiting = Waiting::default();
        let mut round = |number| {
            ready.set(false);
            waiting.start(about(number), waiting_call!(Gate { ready, waker }));
            ready.set(true);
            waker.borrow_mut().take().expect("a waker").wake();
            waiting.next_answer().expect("answered");
        };
        round(0);
        let before = ALLOCATIONS.with(Shared::get);
        for number in 1..100 {
            round(number);
        }
        assert_eq!(ALLOCATIONS.with(Shared::get), before);
    }
}
