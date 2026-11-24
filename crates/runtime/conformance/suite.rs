//! The tests every implementation of wcontainers-runtime runs, written against its traits alone.

use std::thread;
use std::time::Duration;

use oci_spec::runtime::Spec;
use wcontainers_kernel::{ExecutionConfig, ProcessEnd, StopReason};
use wcontainers_runtime::{
    Container, ContainerConfig, ContainerError, EndpointOf, ExecutorOf, Image, Interrupter, Runtime,
};
use wcontainers_system::ProcessTracer;

/// A runtime, with what each of its containers is made from.
pub struct Subject<R: Runtime> {
    pub runtime: R,
    /// An image whose `PATH` holds a POSIX shell named `sh`.
    pub image: Box<dyn Fn() -> R::ImageSource>,
    pub execution: Box<dyn Fn() -> ExecutionConfig<ExecutorOf<R>>>,
    pub state: Box<dyn Fn() -> <R::System as ProcessTracer>::ProcessState>,
    pub parts: Box<dyn Fn() -> Parts<R>>,
    /// Parts whose system panics on the first question the kernel asks it once the container runs.
    pub failing_parts: Box<dyn Fn() -> Parts<R>>,
}

/// A container's system, with the ends of its standard output and error.
pub struct Parts<R: Runtime> {
    pub system: R::System,
    pub standard_output: EndpointOf<R>,
    pub standard_error: EndpointOf<R>,
}

/// The exit status the shell scripts below end with.
const EXIT_CODE: u8 = 7;

/// How long a thread waits before it interrupts a run that has started.
const BEFORE_INTERRUPTING: Duration = Duration::from_millis(200);

/// How many shells run side by side in the parallel check.
const SIDE_BY_SIDE: usize = 4;

/// How many times each of those shells goes round its loop.
const ROUNDS: usize = 2000;

/// Asks a POSIX shell to run `script`.
fn shell(script: &str) -> Vec<String> {
    ["sh", "-c", script].map(String::from).to_vec()
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

/// A container left alone runs to its end and reports its first process's exit status.
pub fn a_container_runs_to_its_end<R: Runtime>(subject: Subject<R>) {
    let (mut container, _interrupter) = made(&subject, shell(&format!("exit {EXIT_CODE}")));
    assert_eq!(container.run(), exited());
}

/// A container that has finished reports the same end to every later run.
pub fn a_finished_container_stays_finished<R: Runtime>(subject: Subject<R>) {
    let (mut container, interrupter) = made(&subject, shell(&format!("exit {EXIT_CODE}")));
    assert_eq!(container.run(), exited());
    interrupter.interrupt();
    assert_eq!(container.run(), exited());
}

/// A program named without a path and found in no directory of `PATH` is refused before anything runs.
pub fn a_program_that_is_nowhere_is_refused<R: Runtime>(subject: Subject<R>) {
    let made = make(
        &subject,
        vec!["no-program-has-this-name".to_string()],
        (subject.parts)(),
    );
    assert!(matches!(made, Err(ContainerError::ProgramNotFound)));
}

/// An interrupt from another thread ends a run that would never end by itself, every time it is asked.
pub fn an_interrupt_from_another_thread_stops_a_run<R: Runtime>(subject: Subject<R>) {
    let (mut container, interrupter) = made(&subject, shell("while :; do :; done"));
    thread::scope(|scope| {
        for _ in 0..2 {
            let interrupting = scope.spawn(|| {
                thread::sleep(BEFORE_INTERRUPTING);
                interrupter.interrupt();
                interrupter.interrupt();
            });
            assert_eq!(container.run(), StopReason::Interrupted);
            interrupting.join().expect("the interrupting thread ends");
        }
    });
}

/// An interrupt asked before a run ends that run alone; the next one goes on to the container's end.
pub fn a_run_goes_on_after_an_interrupt<R: Runtime>(subject: Subject<R>) {
    let (mut container, interrupter) = made(&subject, shell(&format!("exit {EXIT_CODE}")));
    interrupter.interrupt();
    assert_eq!(container.run(), StopReason::Interrupted);
    assert_eq!(container.run(), exited());
}

/// Several shells working at once all run to their ends.
pub fn work_side_by_side_runs_to_its_end<R: Runtime>(subject: Subject<R>) {
    let script = format!(
        "for i in $(seq {SIDE_BY_SIDE}); do (j=0; while [ $j -lt {ROUNDS} ]; do j=$((j+1)); done) & done; wait; exit {EXIT_CODE}"
    );
    let (mut container, _interrupter) = made(&subject, shell(&script));
    assert_eq!(container.run(), exited());
}

/// A host thread that dies inside a run ends the container as panicked, and nothing of it runs again.
pub fn a_host_thread_dying_inside_a_run_ends_the_container<R: Runtime>(subject: Subject<R>) {
    let (mut container, _interrupter) = make(
        &subject,
        shell(&format!("exit {EXIT_CODE}")),
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
        fn work_side_by_side_runs_to_its_end() {
            $crate::work_side_by_side_runs_to_its_end($value);
        }
        #[test]
        fn a_host_thread_dying_inside_a_run_ends_the_container() {
            $crate::a_host_thread_dying_inside_a_run_ends_the_container($value);
        }
    };
}
