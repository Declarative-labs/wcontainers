//! Refusals and the guest calls they end: each of Gallium's calls into the driver runs as one guest
//! call, which a refusal or a failure inside it ends alone, never the process.

use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::pipe::{PIPE_GUILTY_CONTEXT_RESET, PIPE_NO_RESET, PIPE_UNKNOWN_CONTEXT_RESET};
use crate::screen::DRIVER_NAME;

/// The key of the screen's own calls, which name no context.
pub const SCREEN: usize = 0;

/// What a refusal unwinds with to the guest call it ends ([`guest_call`]), its reason already
/// warned of.
struct Refused;

/// Refuses what the driver does not take yet: warns with the reason and ends the guest call it is
/// part of ([`guest_call`]), never a call that quietly ignores it.
pub fn refuse(what: impl std::fmt::Display) -> ! {
    eprintln!("{}: refused: {what}", DRIVER_NAME.to_string_lossy());
    std::panic::resume_unwind(Box::new(Refused))
}

/// A refusal as an error, for the draw or call that refuses with it ([`refuse`]).
pub fn refused<T>(why: std::fmt::Arguments<'_>) -> Result<T, String> {
    Err(why.to_string())
}

/// The contexts, by key, one of whose calls was refused or failed: each reports itself reset by
/// its own fault until destroyed ([`reset_status`]).
static GUILTY: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Guest calls refused or failed in the life of the process.
pub static FAILED_CALLS: AtomicU64 = AtomicU64::new(0);

/// The back end's device was lost: every context reports a reset from then on.
pub static DEVICE_LOST: AtomicBool = AtomicBool::new(false);

fn guilty() -> MutexGuard<'static, Vec<usize>> {
    // The list is whole between its own calls, which take no other lock.
    GUILTY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs one of Gallium's calls into the driver for the context keyed `context` ([`SCREEN`] for the
/// screen's): a refusal or a failure inside it ends that call alone, which answers `dropped`, and
/// the context reports itself reset by its own fault, as a device reset ends the work that caused
/// it. No guest call ends the process.
pub fn guest_call<T>(context: usize, dropped: T, call: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(AssertUnwindSafe(call)) {
        Ok(done) => done,
        Err(failure) => {
            FAILED_CALLS.fetch_add(1, Ordering::Relaxed);
            if !failure.is::<Refused>() {
                // The panic's own message was printed by the process's hook.
                eprintln!(
                    "{}: refused: a call that failed",
                    DRIVER_NAME.to_string_lossy()
                );
            }
            let mut guilty = guilty();
            if context != SCREEN && !guilty.contains(&context) {
                guilty.push(context);
            }
            dropped
        }
    }
}

/// Gallium's reset status of the context keyed `context`: one of whose calls was refused was reset
/// by its own fault, and every other one by an unknown cause once the device is lost (st/mesa then
/// loses a context made to be told).
pub fn reset_status(context: usize) -> u32 {
    if guilty().contains(&context) {
        PIPE_GUILTY_CONTEXT_RESET
    } else if DEVICE_LOST.load(Ordering::Relaxed) {
        PIPE_UNKNOWN_CONTEXT_RESET
    } else {
        PIPE_NO_RESET
    }
}

/// Forgets a destroyed context's reset, so that one made with its key starts afresh.
pub fn forget_reset(context: usize) {
    guilty().retain(|&c| c != context);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key no other test uses.
    const CONTEXT: usize = 0x5eed;

    #[test]
    fn a_refused_call_answers_its_dropped_value_and_its_context_reports_its_own_reset() {
        assert_eq!(reset_status(CONTEXT), PIPE_NO_RESET);
        let done = guest_call(CONTEXT, None::<u32>, || {
            refuse(format_args!("a test's refusal"))
        });
        assert!(done.is_none());
        assert_eq!(reset_status(CONTEXT), PIPE_GUILTY_CONTEXT_RESET);
        // The context goes on: a later call is made.
        assert!(guest_call(CONTEXT, false, || true));
        forget_reset(CONTEXT);
        assert_eq!(reset_status(CONTEXT), PIPE_NO_RESET);
    }

    #[test]
    fn a_call_that_panics_ends_alone_and_the_screens_calls_name_no_context() {
        let failed = FAILED_CALLS.load(Ordering::Relaxed);
        let done = guest_call(SCREEN, 0, || -> u64 { panic!("a test's failure") });
        assert_eq!(done, 0);
        assert!(FAILED_CALLS.load(Ordering::Relaxed) > failed);
        assert!(!guilty().contains(&SCREEN));
    }

    #[test]
    fn a_lock_held_by_a_refused_call_is_taken_again() {
        let lock = Mutex::new(Vec::<u32>::new());
        guest_call(SCREEN, (), || {
            let mut held = crate::screen::whole(&lock);
            held.push(1);
            refuse(format_args!("a test's refusal while holding a lock"))
        });
        assert!(lock.is_poisoned());
        assert_eq!(*crate::screen::whole(&lock), [1]);
    }
}
