//! Steering the calling thread toward a performance level of Apple silicon through its quality-of-service class.

use gdt_cpus::{AppliedPriority, MechanismPolicy, ThreadPriority, set_thread_priority};

use crate::RunOnCoreError;

/// The level of the fastest cores, [level 0](https://developer.apple.com/documentation/kernel/1387446-sysctlbyname/determining_system_capabilities).
const FASTEST_LEVEL: usize = 0;

/// Returns the class that steers a thread toward the level: User Interactive for the fastest, Background for the slowest, Utility between.
fn level_priority(performance_level: usize, levels: usize) -> ThreadPriority {
    match performance_level {
        FASTEST_LEVEL => ThreadPriority::Highest,
        level if level + 1 == levels => ThreadPriority::Background,
        _ => ThreadPriority::Lowest,
    }
}

/// Checks that macOS applied the class asked for, as a quality-of-service class and not a fallback.
fn check_applied(applied: AppliedPriority, asked: ThreadPriority) -> Result<(), RunOnCoreError> {
    if applied.degraded()
        || applied.effective() != asked
        || applied.mechanism().policy != MechanismPolicy::Qos
    {
        return Err(RunOnCoreError::ClassNotApplied {
            applied: applied.to_string(),
        });
    }
    Ok(())
}

/// Runs the work with the thread in the level's class, then gives the thread the Default class.
pub(crate) fn run_with_level_class<Output>(
    performance_level: usize,
    levels: usize,
    work: impl FnOnce() -> Output,
) -> Result<Output, RunOnCoreError> {
    let asked = level_priority(performance_level, levels);
    let applied = set_thread_priority(asked).map_err(RunOnCoreError::ClassRefused)?;
    check_applied(applied, asked)?;
    let output = work();
    // BelowNormal is gdt-cpus's name for the Default class on macOS.
    let restored = set_thread_priority(ThreadPriority::BelowNormal)
        .map_err(RunOnCoreError::ClassNotRestored)?;
    check_applied(restored, ThreadPriority::BelowNormal)?;
    Ok(output)
}
