//! Pinning the calling thread to one CPU for the length of some work, and giving its earlier CPUs back after.

use rustix::thread::{CpuSet, sched_getaffinity, sched_getcpu, sched_setaffinity};

use crate::RunOnCoreError;

/// The thread's set of allowed CPUs from before it was pinned, given back when this is dropped (also when the work panics).
struct EarlierPlacement {
    allowed: CpuSet,
    restored: bool,
}

impl EarlierPlacement {
    fn restore(mut self) -> Result<(), RunOnCoreError> {
        self.restored = true;
        sched_setaffinity(None, &self.allowed).map_err(RunOnCoreError::PlacementNotRestored)
    }
}

impl Drop for EarlierPlacement {
    fn drop(&mut self) {
        if !self.restored {
            // A panic is already unwinding, so a failure here has no caller to go to.
            let _ = sched_setaffinity(None, &self.allowed);
        }
    }
}

/// Runs the work with the calling thread allowed on the CPU alone, confirming with sched_getcpu before and after that it is there.
pub(crate) fn run_pinned<Output>(
    core: usize,
    work: impl FnOnce() -> Output,
) -> Result<Output, RunOnCoreError> {
    // CpuSet::set panics at CPU_SETSIZE and above.
    if core >= CpuSet::MAX_CPU {
        return Err(RunOnCoreError::CoreOutOfRange { core });
    }
    let allowed = sched_getaffinity(None).map_err(RunOnCoreError::PlacementUnreadable)?;
    let mut only = CpuSet::new();
    only.set(core);
    sched_setaffinity(None, &only).map_err(RunOnCoreError::PinRefused)?;
    let earlier = EarlierPlacement {
        allowed,
        restored: false,
    };
    let before = sched_getcpu();
    if before != core {
        earlier.restore()?;
        return Err(RunOnCoreError::MovedBeforeWork { found: before });
    }
    let output = work();
    let after = sched_getcpu();
    earlier.restore()?;
    if after != core {
        return Err(RunOnCoreError::MovedDuringWork { found: after });
    }
    Ok(output)
}
