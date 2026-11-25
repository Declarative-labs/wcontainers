//! Pausing the device thread until the GPU answers (plan section 4.8): natively by waiting on the
//! device; in the browser by suspending the WebAssembly stack through JSPI until a callback of the
//! browser's WebGPU wakes it. In the browser the call that waits must run inside an export marked
//! `#[wasm_bindgen(jspi)]`.

/// Pauses until `done` holds, or natively until waiting on the device fails.
pub(crate) fn until(
    device: &wgpu::Device,
    index: Option<wgpu::SubmissionIndex>,
    done: impl Fn() -> bool,
) {
    while !done() {
        #[cfg(not(target_arch = "wasm32"))]
        if device
            .poll(wgpu::PollType::Wait {
                submission_index: index.clone(),
                timeout: None,
            })
            .is_err()
        {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (device, &index);
            jspi::suspend();
        }
    }
}

/// Wakes a paused device thread: called by every callback that may end a pause.
pub(crate) fn wake() {
    #[cfg(target_arch = "wasm32")]
    jspi::wake();
}

/// Runs a future to its end on the device thread, suspending through JSPI while it waits (the
/// browser's adapter and device requests, for one). Only inside a `#[wasm_bindgen(jspi)]` export.
#[cfg(target_arch = "wasm32")]
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    struct Wakes;
    impl Wake for Wakes {
        fn wake(self: Arc<Self>) {
            jspi::wake();
        }
    }
    let waker = Waker::from(Arc::new(Wakes));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return v;
        }
        jspi::suspend();
    }
}

// JSPI is in every browser the plan targets (4.8); wasm-bindgen still marks it experimental.
#[cfg(target_arch = "wasm32")]
#[allow(deprecated)]
mod jspi {
    use std::cell::{Cell, RefCell};
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        /// Suspends the calling stack until the promise settles.
        #[wasm_bindgen(js_namespace = Promise, js_name = resolve, suspending)]
        fn settled(promise: &js_sys::Promise);
    }

    thread_local! {
        /// Resolves the promise the suspended stack waits on.
        static RESOLVE: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
        /// A wake that came while nothing was suspended, so the next suspension returns at once.
        static WOKEN: Cell<bool> = const { Cell::new(false) };
    }

    /// Suspends until the next wake, or returns at once if one came since the last suspension.
    pub fn suspend() {
        if WOKEN.take() {
            return;
        }
        let promise = js_sys::Promise::new(&mut |resolve, _| RESOLVE.set(Some(resolve)));
        settled(&promise);
        WOKEN.set(false);
    }

    pub fn wake() {
        match RESOLVE.take() {
            Some(resolve) => {
                let _ = resolve.call0(&JsValue::UNDEFINED);
            }
            None => WOKEN.set(true),
        }
    }
}
