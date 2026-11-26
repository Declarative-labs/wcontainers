//! The tests every implementation of wcontainers-runtime runs, written against its traits alone.

use std::num::NonZeroUsize;
use std::thread;
use std::time::Instant;

use oci_spec::runtime::Spec;
use wcontainers_kernel::{ExecutionConfig, ProcessEnd, StopReason};
use wcontainers_runtime::{
    Container, ContainerConfig, ContainerError, EndpointOf, ExecutorOf, Image, Interrupter, Runtime,
};
use wcontainers_system::ProcessTracer;

/// A runtime, with what each of its containers is made from.
pub struct Subject<R: Runtime> {
    pub runtime: R,
    /// How many runners each container of the runtime has.
    pub runners: NonZeroUsize,
    /// An image whose `PATH` holds a POSIX shell named `sh` and the utilities `cat` and `cmp`.
    pub image: Box<dyn Fn() -> R::ImageSource>,
    pub execution: Box<dyn Fn() -> ExecutionConfig<ExecutorOf<R>>>,
    pub state: Box<dyn Fn() -> <R::System as ProcessTracer>::ProcessState>,
    pub parts: Box<dyn Fn() -> Parts<R>>,
    /// Parts whose system panics when the container writes to its standard output.
    pub failing_parts: Box<dyn Fn() -> Parts<R>>,
}

/// A container's system, with the ends of its standard output and error.
pub struct Parts<R: Runtime> {
    pub system: R::System,
    pub standard_output: EndpointOf<R>,
    pub standard_error: EndpointOf<R>,
}

/// The exit status the shell scripts below end with: the largest Linux keeps, which no shell gives of its own.
///
/// A shell's own statuses are 0 to 2, 126, 127 and 128 plus a signal's number, at most 64.
const EXIT_CODE: u8 = u8::MAX;

/// The POSIX shell the image holds, and its option that runs a script.
const SHELL: &str = "sh";
const SCRIPT_OPTION: &str = "-c";

/// A program name no image holds.
const MISSING_PROGRAM: &str = "no-program-has-this-name";

/// One stage of a pipeline, which copies what it reads to what it writes.
const PIPELINE_STAGE: &str = " | cat";

/// A script that never ends by itself.
const ENDLESS_SCRIPT: &str = "while :; do :; done";

/// Returns the arguments that ask a POSIX shell to run `script`.
fn shell(script: &str) -> Vec<String> {
    [SHELL, SCRIPT_OPTION, script].map(String::from).to_vec()
}

fn spec_running(image: &impl Image, args: Vec<String>) -> Spec {
    let mut spec = image
        .to_runtime_spec()
        .expect("the image converts to a runtime configuration");
    let mut process = spec
        .process()
        .clone()
        .expect("the image's configuration has a process");
    process.set_args(Some(args));
    spec.set_process(Some(process));
    spec
}

fn make<R: Runtime>(
    subject: &Subject<R>,
    args: Vec<String>,
    parts: Parts<R>,
) -> Result<(R::Container, R::Interrupter), ContainerError> {
    let image = subject
        .runtime
        .open_image((subject.image)())
        .unwrap_or_else(|_| panic!("the subject's image opens"));
    let spec = spec_running(&image, args);
    subject.runtime.make_container(ContainerConfig {
        spec: &spec,
        image: &image,
        execution: (subject.execution)(),
        state: (subject.state)(),
        standard_input: None,
        standard_output: parts.standard_output,
        standard_error: parts.standard_error,
        system: parts.system,
    })
}

fn made<R: Runtime>(subject: &Subject<R>, args: Vec<String>) -> (R::Container, R::Interrupter) {
    make(subject, args, (subject.parts)())
        .unwrap_or_else(|error| panic!("the container is made: {error:?}"))
}

fn exited() -> StopReason {
    StopReason::Finished(ProcessEnd::Exited { code: EXIT_CODE })
}

/// Checks that a container left alone runs to its end and reports its first process's exit status.
pub fn a_container_runs_to_its_end<R: Runtime>(subject: Subject<R>) {
    let (mut container, _interrupter) = made(&subject, shell(&format!("exit {EXIT_CODE}")));
    assert_eq!(container.run(), exited());
}

/// Checks that a container that has finished reports the same end to every later run.
pub fn a_finished_container_stays_finished<R: Runtime>(subject: Subject<R>) {
    let (mut container, interrupter) = made(&subject, shell(&format!("exit {EXIT_CODE}")));
    assert_eq!(container.run(), exited());
    interrupter.interrupt();
    assert_eq!(container.run(), exited());
}

/// Checks that a program named without a path and in no directory of `PATH` is refused before anything runs.
pub fn a_program_that_is_nowhere_is_refused<R: Runtime>(subject: Subject<R>) {
    let made = make(
        &subject,
        vec![MISSING_PROGRAM.to_string()],
        (subject.parts)(),
    );
    assert!(matches!(made, Err(ContainerError::ProgramNotFound)));
}

/// Checks that an interrupt from another thread ends a run that would never end by itself, every time.
///
/// The interrupt waits as long as a container that only exits took to make and run, so that the run has started.
pub fn an_interrupt_from_another_thread_stops_a_run<R: Runtime>(subject: Subject<R>) {
    let started = Instant::now();
    let (mut exiting, _interrupter) = made(&subject, shell(&format!("exit {EXIT_CODE}")));
    assert_eq!(exiting.run(), exited());
    let before_interrupting = started.elapsed();
    let (mut container, interrupter) = made(&subject, shell(ENDLESS_SCRIPT));
    thread::scope(|scope| {
        for _ in 0..2 {
            let interrupting = scope.spawn(|| {
                thread::sleep(before_interrupting);
                interrupter.interrupt();
                interrupter.interrupt();
            });
            assert_eq!(container.run(), StopReason::Interrupted);
            interrupting.join().expect("the interrupting thread ends");
        }
    });
}

/// Checks that an interrupt asked before a run ends that run alone, and the next goes on to the container's end.
pub fn a_run_goes_on_after_an_interrupt<R: Runtime>(subject: Subject<R>) {
    let (mut container, interrupter) = made(&subject, shell(&format!("exit {EXIT_CODE}")));
    interrupter.interrupt();
    assert_eq!(container.run(), StopReason::Interrupted);
    assert_eq!(container.run(), exited());
}

/// Checks that a first process that exits ends its container while a process it started in the background never stops.
///
/// With one runner, the busy process holds it until its slice ends, so the container ends only if slices end.
pub fn a_busy_background_process_does_not_keep_its_container_running<R: Runtime>(
    subject: Subject<R>,
) {
    let script = format!("({ENDLESS_SCRIPT}) & exit {EXIT_CODE}");
    let (mut container, _interrupter) = made(&subject, shell(&script));
    assert_eq!(container.run(), exited());
}

/// Checks that a pipeline of processes working at once runs to its end and passes every byte on unchanged.
///
/// It copies the shell's own program through one more stage than the runtime has runners, so the runners take turns.
pub fn work_side_by_side_runs_to_its_end<R: Runtime>(subject: Subject<R>) {
    let stages = PIPELINE_STAGE.repeat(subject.runners.get() + 1);
    let script =
        format!("s=$(command -v {SHELL}); cat \"$s\"{stages} | cmp - \"$s\" && exit {EXIT_CODE}");
    let (mut container, _interrupter) = made(&subject, shell(&script));
    assert_eq!(container.run(), exited());
}

/// Checks that a host thread dying inside a run ends the container as panicked, and nothing of it runs again.
pub fn a_host_thread_dying_inside_a_run_ends_the_container<R: Runtime>(subject: Subject<R>) {
    let (mut container, _interrupter) = make(
        &subject,
        shell(&format!("echo; exit {EXIT_CODE}")),
        (subject.failing_parts)(),
    )
    .unwrap_or_else(|error| panic!("the container is made: {error:?}"));
    assert_eq!(container.run(), StopReason::Panicked);
    assert_eq!(container.run(), StopReason::Panicked);
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn a_container_runs_to_its_end() {
            $crate::a_container_runs_to_its_end($value);
        }
        #[test]
        fn a_finished_container_stays_finished() {
            $crate::a_finished_container_stays_finished($value);
        }
        #[test]
        fn a_program_that_is_nowhere_is_refused() {
            $crate::a_program_that_is_nowhere_is_refused($value);
        }
        #[test]
        fn an_interrupt_from_another_thread_stops_a_run() {
            $crate::an_interrupt_from_another_thread_stops_a_run($value);
        }
        #[test]
        fn a_run_goes_on_after_an_interrupt() {
            $crate::a_run_goes_on_after_an_interrupt($value);
        }
        #[test]
        fn a_busy_background_process_does_not_keep_its_container_running() {
            $crate::a_busy_background_process_does_not_keep_its_container_running($value);
        }
        #[test]
        fn work_side_by_side_runs_to_its_end() {
            $crate::work_side_by_side_runs_to_its_end($value);
        }
        #[test]
        fn a_host_thread_dying_inside_a_run_ends_the_container() {
            $crate::a_host_thread_dying_inside_a_run_ends_the_container($value);
        }
    };
}
