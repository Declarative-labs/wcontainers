//! The tests every implementation of wcontainers-runtime runs, written against its traits alone.

use wcontainers_kernel_interface::{ExecutionConfig, ReplayDeterminism, ThreadOrdering};
use wcontainers_runtime::{Container, ContainerConfig, Image, Interrupter, Runtime};
use wcontainers_system::{ProcessTracer, StreamTransport};

/// Calls each method of `Runtime` and of what it hands out as a placeholder that fails until the real checks are written.
pub fn call_runtime_methods<R>(runtime: R)
where
    R: Runtime,
    R::System: Default,
    R::ImageSource: Default,
    <R::System as StreamTransport>::Endpoint: Default,
    <R::System as ProcessTracer>::ProcessState: Default,
{
    if let Ok(image) = runtime.open_image(Default::default())
        && let Ok(spec) = Image::to_runtime_spec(&image)
    {
        let execution = ExecutionConfig {
            thread_ordering: ThreadOrdering::Serial,
            replay_determinism: ReplayDeterminism::Exact,
            translator: Default::default(),
        };
        let config = ContainerConfig {
            spec: &spec,
            image: &image,
            execution,
            state: Default::default(),
            standard_input: None,
            standard_output: Default::default(),
            standard_error: Default::default(),
            system: R::System::default(),
        };
        if let Ok((mut container, interrupter)) = runtime.make_container(config) {
            Interrupter::interrupt(&interrupter);
            Container::run(&mut container);
        }
    }
    todo!("this is a placeholder: the real checks of Runtime are not written yet");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn call_runtime_methods() {
            $crate::call_runtime_methods($value);
        }
    };
}
