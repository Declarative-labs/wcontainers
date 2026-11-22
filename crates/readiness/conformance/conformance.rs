//! The tests every implementation of wcontainers-readiness runs, each on a [`Waiting`] that a requiring trait's suite makes.

use core::fmt::Debug;
use core::future::Future;
use core::pin::{Pin, pin};
use core::task::{Context, Poll, Waker};
use std::sync::{Arc, Condvar, Mutex};
use std::task::Wake;
use std::time::Duration;

use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};

/// A handle not yet ready for the interest, and a call that makes it ready.
pub struct Waiting<R, Handle, MakeReady> {
    pub readiness: R,
    pub handle: Handle,
    pub interest: ReadyFor,
    /// Gives the handle a fact of the interest; for a check of failures, an error or a hang-up instead.
    pub make_ready: MakeReady,
}

/// How many waits a check makes on one handle at once.
pub const MANY_WAITS: usize = 32;

/// How long a check waits for a wake.
const PATIENCE: Duration = Duration::from_secs(1);

/// How long a check waits to see that no wake comes.
const SILENCE: Duration = Duration::from_millis(200);

/// The wakes a waiting call has given, counted, for a check that polls the call by hand.
#[derive(Default)]
struct Wakes {
    count: Mutex<usize>,
    changed: Condvar,
}

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        *self.count.lock().expect("the wakes") += 1;
        self.changed.notify_all();
    }
}

impl Wakes {
    fn count(&self) -> usize {
        *self.count.lock().expect("the wakes")
    }

    /// Whether the call has been woken `count` times before the patience of a check ends.
    fn reach(&self, count: usize) -> bool {
        let wakes = self.count.lock().expect("the wakes");
        let (wakes, _) = self
            .changed
            .wait_timeout_while(wakes, PATIENCE, |wakes| *wakes < count)
            .expect("the wakes");
        *wakes >= count
    }
}

/// Polls a waiting call once with a waker that counts into `wakes`.
fn poll_with<F: Future>(call: Pin<&mut F>, wakes: &Arc<Wakes>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::clone(wakes));
    call.poll(&mut Context::from_waker(&waker))
}

/// The report a finished wait gave.
fn finished<E: Debug>(poll: Poll<Result<ReadinessReport, E>>) -> ReadinessReport {
    match poll {
        Poll::Ready(facts) => facts.expect("the wait succeeds"),
        Poll::Pending => panic!("the wait did not finish"),
    }
}

/// The report of a wait polled until it finishes, within the patience of a check.
fn finished_soon<E: Debug, F: Future<Output = Result<ReadinessReport, E>>>(
    mut call: Pin<&mut F>,
) -> ReadinessReport {
    let wakes = Arc::new(Wakes::default());
    match poll_with(call.as_mut(), &wakes) {
        Poll::Pending => {
            assert!(wakes.reach(1), "the wait finished");
            finished(poll_with(call, &wakes))
        }
        ready => finished(ready),
    }
}

/// A wait on a handle not yet ready is not woken and does not finish.
pub fn a_wait_does_not_finish_before_the_handle_is_ready<R, Handle, MakeReady>(
    waiting: Waiting<R, Handle, MakeReady>,
) where
    R: Readiness<Handle>,
{
    let Waiting {
        readiness,
        handle,
        interest,
        ..
    } = waiting;
    let wakes = Arc::new(Wakes::default());
    let mut wait = pin!(readiness.wait_until_ready(&handle, interest));
    assert!(
        poll_with(wait.as_mut(), &wakes).is_pending(),
        "finished before the handle was ready"
    );
    std::thread::sleep(SILENCE);
    assert_eq!(wakes.count(), 0, "woken before the handle was ready");
    assert!(
        poll_with(wait, &wakes).is_pending(),
        "finished before the handle was ready"
    );
}

/// A wait is woken once when its handle becomes ready, and finishes with a fact it asked for.
pub fn a_wait_finishes_when_the_handle_becomes_ready<R, Handle, MakeReady>(
    waiting: Waiting<R, Handle, MakeReady>,
) where
    R: Readiness<Handle>,
    MakeReady: FnOnce(&R, &Handle),
{
    let Waiting {
        readiness,
        handle,
        interest,
        make_ready,
    } = waiting;
    let wakes = Arc::new(Wakes::default());
    let mut wait = pin!(readiness.wait_until_ready(&handle, interest));
    assert!(poll_with(wait.as_mut(), &wakes).is_pending());
    make_ready(&readiness, &handle);
    assert!(wakes.reach(1), "woken when the handle became ready");
    let report = finished(poll_with(wait, &wakes));
    assert!(
        report.ready_for.intersects(interest),
        "{report:?} for {interest:?}"
    );
    assert_eq!(wakes.count(), 1, "woken more than once");
}

/// A wait made on a handle already ready finishes at its first poll, unwoken.
pub fn a_wait_made_when_ready_finishes_at_once<R, Handle, MakeReady>(
    waiting: Waiting<R, Handle, MakeReady>,
) where
    R: Readiness<Handle>,
    MakeReady: FnOnce(&R, &Handle),
{
    let Waiting {
        readiness,
        handle,
        interest,
        make_ready,
    } = waiting;
    make_ready(&readiness, &handle);
    // The first wait lets the readiness arrive, which on a host may come a moment after the call that makes it.
    let settled = finished_soon(pin!(readiness.wait_until_ready(&handle, interest)));
    assert!(
        settled.ready_for.intersects(interest),
        "{settled:?} for {interest:?}"
    );
    let wakes = Arc::new(Wakes::default());
    let report = finished(poll_with(
        pin!(readiness.wait_until_ready(&handle, interest)),
        &wakes,
    ));
    assert!(
        report.ready_for.intersects(interest),
        "{report:?} for {interest:?}"
    );
    assert_eq!(wakes.count(), 0, "a wait that finished at once was woken");
}

/// A wait dropped before its handle becomes ready is not woken, and a wait made after finishes.
pub fn a_dropped_wait_is_withdrawn<R, Handle, MakeReady>(waiting: Waiting<R, Handle, MakeReady>)
where
    R: Readiness<Handle>,
    MakeReady: FnOnce(&R, &Handle),
{
    let Waiting {
        readiness,
        handle,
        interest,
        make_ready,
    } = waiting;
    let wakes = Arc::new(Wakes::default());
    {
        let wait = pin!(readiness.wait_until_ready(&handle, interest));
        assert!(poll_with(wait, &wakes).is_pending());
    }
    make_ready(&readiness, &handle);
    std::thread::sleep(SILENCE);
    assert_eq!(wakes.count(), 0, "a dropped wait was woken");
    let report = finished_soon(pin!(readiness.wait_until_ready(&handle, interest)));
    assert!(
        report.ready_for.intersects(interest),
        "{report:?} for {interest:?}"
    );
}

/// Many waits on one handle are each woken once when it becomes ready, and each finishes.
pub fn many_waits_each_wake_once<R, Handle, MakeReady>(waiting: Waiting<R, Handle, MakeReady>)
where
    R: Readiness<Handle>,
    MakeReady: FnOnce(&R, &Handle),
{
    let Waiting {
        readiness,
        handle,
        interest,
        make_ready,
    } = waiting;
    let mut waits: Vec<_> = (0..MANY_WAITS)
        .map(|_| {
            (
                Box::pin(readiness.wait_until_ready(&handle, interest)),
                Arc::new(Wakes::default()),
            )
        })
        .collect();
    for (wait, wakes) in &mut waits {
        assert!(poll_with(wait.as_mut(), wakes).is_pending());
    }
    make_ready(&readiness, &handle);
    for (_, wakes) in &waits {
        assert!(wakes.reach(1), "every wait woken");
    }
    std::thread::sleep(SILENCE);
    for (wait, wakes) in &mut waits {
        assert_eq!(wakes.count(), 1, "a wait woken more than once");
        let report = finished(poll_with(wait.as_mut(), wakes));
        assert!(
            report.ready_for.intersects(interest),
            "{report:?} for {interest:?}"
        );
    }
}

/// An error or a hang-up ends a wait that cannot ask for either, and the wait reports it as unconditional.
pub fn a_failure_ends_a_wait_that_did_not_ask_for_it<R, Handle, MakeReady>(
    waiting: Waiting<R, Handle, MakeReady>,
) where
    R: Readiness<Handle>,
    MakeReady: FnOnce(&R, &Handle),
{
    let Waiting {
        readiness,
        handle,
        interest,
        make_ready,
    } = waiting;
    let wakes = Arc::new(Wakes::default());
    let mut wait = pin!(readiness.wait_until_ready(&handle, interest));
    assert!(poll_with(wait.as_mut(), &wakes).is_pending());
    make_ready(&readiness, &handle);
    assert!(wakes.reach(1), "woken by the failure");
    let report = finished(poll_with(wait, &wakes));
    assert!(!report.unconditional.is_empty(), "{report:?}");
}

/// Runs each check above as a test in the module named, on a new `ready` bundle, or `failing` (an error or hang-up).
#[macro_export]
macro_rules! tests {
    ($module:ident, ready: $ready:expr, failing: $failing:expr) => {
        mod $module {
            #[allow(unused_imports)]
            use super::*;

            $crate::tests!(@each $ready;
                a_wait_does_not_finish_before_the_handle_is_ready
                a_wait_finishes_when_the_handle_becomes_ready
                a_wait_made_when_ready_finishes_at_once
                a_dropped_wait_is_withdrawn
                many_waits_each_wake_once
            );
            $crate::tests!(@each $failing; a_failure_ends_a_wait_that_did_not_ask_for_it);
        }
    };
    (@each $value:expr; $($name:ident)*) => {
        $(
            #[test]
            fn $name() {
                $crate::$name($value);
            }
        )*
    };
}

#[cfg(test)]
mod suite {
    use core::cell::{Cell, RefCell};
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll, Waker};

    use wcontainers_abi::HostError;
    use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor, UnconditionalEvents};

    use crate::Waiting;

    /// A report of no fact at all.
    const NOTHING: ReadinessReport = ReadinessReport {
        ready_for: ReadyFor::empty(),
        unconditional: UnconditionalEvents::empty(),
    };

    /// A report of the facts asked for alone.
    fn ready_for(facts: ReadyFor) -> ReadinessReport {
        ReadinessReport {
            ready_for: facts,
            ..NOTHING
        }
    }

    /// A report of the unconditional events alone.
    fn unconditional(events: UnconditionalEvents) -> ReadinessReport {
        ReadinessReport {
            unconditional: events,
            ..NOTHING
        }
    }

    /// What a double gets wrong, each of which a check must catch.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Flaw {
        None,
        FinishesAtOnce,
        NeverWakes,
        WaitsWhenReady,
        KeepsDroppedWaits,
        WakesOnlyTheLatest,
        IgnoresFailures,
        GivesNothing,
    }

    struct Double(Flaw);

    /// A handle whose facts a check sets, with the wakers of the waits on it.
    struct Handle {
        facts: Cell<ReadinessReport>,
        waiters: RefCell<Vec<Option<Waker>>>,
    }

    impl Handle {
        fn become_ready(&self, facts: ReadinessReport) {
            self.facts.set(facts);
            let woken: Vec<Waker> = self
                .waiters
                .borrow_mut()
                .iter_mut()
                .filter_map(Option::take)
                .collect();
            woken.into_iter().for_each(Waker::wake);
        }
    }

    struct Wait<'a> {
        flaw: Flaw,
        handle: &'a Handle,
        interest: ReadyFor,
        slot: Option<usize>,
        polled: bool,
    }

    impl Future for Wait<'_> {
        type Output = Result<ReadinessReport, HostError>;

        fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();
            let first = !core::mem::replace(&mut this.polled, true);
            let facts = this.handle.facts.get();
            let ends = facts.ready_for.intersects(this.interest)
                || (this.flaw != Flaw::IgnoresFailures && !facts.unconditional.is_empty());
            if this.flaw == Flaw::FinishesAtOnce {
                return Poll::Ready(Ok(ready_for(this.interest)));
            }
            if ends && !(first && this.flaw == Flaw::WaitsWhenReady) {
                return Poll::Ready(Ok(if this.flaw == Flaw::GivesNothing {
                    NOTHING
                } else {
                    facts
                }));
            }
            if this.flaw == Flaw::WaitsWhenReady && first {
                context.waker().wake_by_ref();
                return Poll::Pending;
            }
            if this.flaw == Flaw::NeverWakes {
                return Poll::Pending;
            }
            let mut waiters = this.handle.waiters.borrow_mut();
            if this.flaw == Flaw::WakesOnlyTheLatest {
                waiters.clear();
            }
            match this.slot.filter(|&slot| slot < waiters.len()) {
                Some(slot) => waiters[slot] = Some(context.waker().clone()),
                None => {
                    this.slot = Some(waiters.len());
                    waiters.push(Some(context.waker().clone()));
                }
            }
            Poll::Pending
        }
    }

    impl Drop for Wait<'_> {
        fn drop(&mut self) {
            if self.flaw != Flaw::KeepsDroppedWaits
                && let Some(slot) = self.slot
                && let Some(waiter) = self.handle.waiters.borrow_mut().get_mut(slot)
            {
                *waiter = None;
            }
        }
    }

    impl Readiness<Handle> for Double {
        async fn wait_until_ready(
            &self,
            handle: &Handle,
            interest: ReadyFor,
        ) -> Result<ReadinessReport, HostError> {
            Wait {
                flaw: self.0,
                handle,
                interest,
                slot: None,
                polled: false,
            }
            .await
        }
    }

    /// A double's handle, waited on for being readable, which the bundle's call makes have the facts given.
    fn waiting(
        flaw: Flaw,
        becomes: ReadinessReport,
    ) -> Waiting<Double, Handle, impl FnOnce(&Double, &Handle)> {
        Waiting {
            readiness: Double(flaw),
            handle: Handle {
                facts: Cell::new(NOTHING),
                waiters: RefCell::new(Vec::new()),
            },
            interest: ReadyFor::READABLE,
            make_ready: move |_: &Double, handle: &Handle| handle.become_ready(becomes),
        }
    }

    crate::tests!(
        correct,
        ready: waiting(Flaw::None, ready_for(ReadyFor::READABLE | ReadyFor::WRITABLE)),
        failing: waiting(Flaw::None, unconditional(UnconditionalEvents::HANG_UP))
    );

    #[test]
    #[should_panic]
    fn catches_a_wait_that_finishes_at_once() {
        crate::a_wait_does_not_finish_before_the_handle_is_ready(waiting(
            Flaw::FinishesAtOnce,
            ready_for(ReadyFor::READABLE),
        ));
    }

    #[test]
    #[should_panic]
    fn catches_a_wait_never_woken() {
        crate::a_wait_finishes_when_the_handle_becomes_ready(waiting(
            Flaw::NeverWakes,
            ready_for(ReadyFor::READABLE),
        ));
    }

    #[test]
    #[should_panic]
    fn catches_a_ready_handle_that_still_waits() {
        crate::a_wait_made_when_ready_finishes_at_once(waiting(
            Flaw::WaitsWhenReady,
            ready_for(ReadyFor::READABLE),
        ));
    }

    #[test]
    #[should_panic]
    fn catches_a_dropped_wait_still_woken() {
        crate::a_dropped_wait_is_withdrawn(waiting(
            Flaw::KeepsDroppedWaits,
            ready_for(ReadyFor::READABLE),
        ));
    }

    #[test]
    #[should_panic]
    fn catches_waits_of_which_only_the_latest_wakes() {
        crate::many_waits_each_wake_once(waiting(
            Flaw::WakesOnlyTheLatest,
            ready_for(ReadyFor::READABLE),
        ));
    }

    #[test]
    #[should_panic]
    fn catches_a_failure_that_does_not_end_a_wait() {
        crate::a_failure_ends_a_wait_that_did_not_ask_for_it(waiting(
            Flaw::IgnoresFailures,
            unconditional(UnconditionalEvents::ERROR),
        ));
    }

    #[test]
    #[should_panic]
    fn catches_a_wait_that_gives_no_fact() {
        crate::a_wait_finishes_when_the_handle_becomes_ready(waiting(
            Flaw::GivesNothing,
            ready_for(ReadyFor::READABLE),
        ));
    }
}
