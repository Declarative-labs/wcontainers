//! A kernel made through the kernel interface runs its programs on its runners, each turn through
//! the executor interface's `execute`.

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

use wcontainers_executor_unicorn::driver::{
    FIRST_PROCESS, InterfaceRunner, program_exit_code, program_on_runners,
};
use wcontainers_executor_unicorn::standin::{SPACE_32_BIT, StandIn};
use wcontainers_kernel::{Kernel, ProcessEnd, Runner, StopReason};

/// How long a guest that never traps is let run before the kernel's interface is called.
const RUNNING_FOR: Duration = Duration::from_millis(300);

fn out() -> PathBuf {
    std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest")
}

/// Runs `runner` until the container ends, and returns how.
fn run_to_end(runner: &mut InterfaceRunner<u64>) -> StopReason {
    let mut executor = StandIn::with_space(SPACE_32_BIT);
    loop {
        match runner.run(&mut executor) {
            StopReason::Idle { until: Some(_) } | StopReason::Interrupted => {}
            stopped => return stopped,
        }
    }
}

#[test]
fn a_program_without_a_c_library_runs_to_its_end_on_a_runner() {
    let (_kernel, mut runners, _host) =
        program_on_runners("hello", &["-nostdlib"], &out(), NonZeroUsize::MIN);
    assert_eq!(
        run_to_end(&mut runners[0]),
        StopReason::Finished(ProcessEnd::Exited { code: 3 })
    );
}

#[test]
fn a_program_with_the_c_library_runs_to_its_end_on_one_of_four_runners() {
    let parallelism = NonZeroUsize::new(4).expect("some parallelism");
    let (_kernel, mut runners, _host) = program_on_runners("processors", &[], &out(), parallelism);
    let told = u8::try_from(parallelism.get()).expect("a small count");
    assert_eq!(
        run_to_end(&mut runners[0]),
        StopReason::Finished(ProcessEnd::Exited { code: told })
    );
}

#[test]
fn code_the_kernel_writes_over_code_that_ran_is_what_runs_next() {
    let (_kernel, mut runners, _host) =
        program_on_runners("codewrite", &[], &out(), NonZeroUsize::MIN);
    assert_eq!(
        run_to_end(&mut runners[0]),
        StopReason::Finished(ProcessEnd::Exited { code: 2 })
    );
    assert_eq!(
        program_exit_code("codewrite", &[], &out(), NonZeroUsize::MIN),
        2,
        "the same on the kernel's own driver"
    );
}

#[test]
fn a_program_that_never_traps_is_interrupted_for_the_kernels_interface() {
    let (kernel, mut runners, _host) =
        program_on_runners("spin", &["-nostdlib"], &out(), NonZeroUsize::MIN);
    let mut runner = runners.pop().expect("one runner");
    let running = std::thread::spawn(move || {
        let mut executor = StandIn::with_space(SPACE_32_BIT);
        runner.run(&mut executor)
    });
    std::thread::sleep(RUNNING_FOR);
    let taken: Vec<_> = kernel.suspend_tasks(&[FIRST_PROCESS]).collect();
    assert_eq!(
        running.join().expect("the runner's host thread"),
        StopReason::Interrupted
    );
    assert_eq!(
        taken.iter().map(|(task, _)| *task).collect::<Vec<_>>(),
        [FIRST_PROCESS],
        "the thread whose turn was ended is taken"
    );
}
