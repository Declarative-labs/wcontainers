//! A recorded call stream (a real program's frames through a front, recorded by `Traced`) replayed in the
//! browser on the WebGL 2 or the WebGPU back end, frame by frame, for browser/replay.mjs
//! (replay.html in the WebGL 2 back end's browser folder), which times them.
#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wcontainers_gpu::{
    caps::*, draw::*, error::*, format::*, helpers::*, occlusion::*, passes::*, reflect::*,
    resource::*, shader::*, slow::*, state::*, *,
};
use wcontainers_gpu_trace::{Frame, Replay};
use wcontainers_gpu_webgl::GlConfig;
use wcontainers_opengl_es_webgl::WebGl;

/// The WebGL 2 back end on the OpenGL ES crate's implementation over a canvas's WebGL 2 context.
type Gl = wcontainers_gpu_webgl::Gl<WebGl>;
use wcontainers_gpu_webgpu::{WebGpu, block_on};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance)]
    fn now() -> f64;
    /// A new canvas on the page, for a surface the stream makes on the WebGPU back end.
    #[wasm_bindgen(js_namespace = window, js_name = makeCanvas)]
    fn make_canvas() -> web_sys::HtmlCanvasElement;
}

enum Device {
    Gl(Box<Gl>, Replay<Gl>),
    Gpu(Box<WebGpu>, Replay<WebGpu>, wgpu::Instance, String),
}

thread_local! {
    static DEVICE: RefCell<Option<Device>> = const { RefCell::new(None) };
}

/// The frame's calls as `kind count ms` lines, then `failed <n> <first failure>`.
fn lines(f: &Frame) -> String {
    let mut out = String::new();
    for (name, count, ms) in &f.kinds {
        out += &format!("{name} {count} {ms:.6}\n");
    }
    out += &format!(
        "failed {} {}",
        f.failed,
        f.first_failure.as_deref().unwrap_or("")
    );
    out
}

/// Starts replaying `stream` on the WebGL 2 back end of `context`.
#[wasm_bindgen]
pub fn replay_on_webgl(context: web_sys::WebGl2RenderingContext, stream: Vec<u8>) {
    let config = GlConfig {
        gl: WebGl::new(context, &[]),
        off: Vec::new(),
    };
    let Ok(b) = <Gl as StandaloneGpu>::new(config);
    DEVICE.set(Some(Device::Gl(Box::new(b), Replay::new(stream))));
}

// JSPI exports, which wasm-bindgen still marks experimental.
#[allow(deprecated)]
mod jspi {
    use super::*;

    /// Starts replaying `stream` on the WebGPU back end of the browser's GPU; it waits for its
    /// probe by suspending.
    #[wasm_bindgen(jspi)]
    pub fn replay_on_webgpu(stream: Vec<u8>) -> Result<(), JsError> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::BROWSER_WEBGPU;
        let instance = wgpu::Instance::new(desc);
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| JsError::new(&format!("no WebGPU adapter: {e}")))?;
        let info = adapter.get_info();
        let (device, queue) =
            block_on(adapter.request_device(&WebGpu::device_descriptor(&adapter)))
                .map_err(|e| JsError::new(&format!("no WebGPU device: {e}")))?;
        let b = WebGpu::new(&adapter, device, queue);
        let name = format!("{} ({:?})", info.name, info.device_type);
        DEVICE.set(Some(Device::Gpu(
            Box::new(b),
            Replay::new(stream),
            instance,
            name,
        )));
        Ok(())
    }

    /// Replays the next frame, each call timed inside the back end when `timed`; answers its lines
    /// and the frame's whole time (`frame <ms>`), or nothing at the stream's end. Read backs may
    /// wait by suspending.
    #[wasm_bindgen(jspi)]
    pub fn replay_frame(timed: bool) -> Result<Option<String>, JsError> {
        let clock: &dyn Fn() -> f64 = &now;
        let clock = timed.then_some(clock);
        DEVICE.with_borrow_mut(|d| {
            let start = now();
            let frame = match d
                .as_mut()
                .ok_or_else(|| JsError::new("no replay started"))?
            {
                Device::Gl(b, r) => r.frame(b, clock, &mut |_, _| {}),
                Device::Gpu(b, r, instance, _) => r.frame(b, clock, &mut |b, window| {
                    let canvas = wgpu::SurfaceTarget::Canvas(make_canvas());
                    match instance.create_surface(canvas) {
                        Ok(s) => b.add_window(window, s),
                        Err(e) => web_sys::console::log_1(&format!("no surface: {e}").into()),
                    }
                }),
            };
            let ms = now() - start;
            match frame {
                None => Ok(None),
                Some(Err(e)) => Err(JsError::new(&e)),
                Some(Ok(f)) => Ok(Some(format!("{}\nframe {ms:.6}", lines(&f)))),
            }
        })
    }
}

/// Drops the replay and its device, so the next starts on a device of its own alone.
#[wasm_bindgen]
pub fn replay_end() {
    DEVICE.set(None);
}

/// The device's state, its GPU and the slow paths it took, after the frames.
#[wasm_bindgen]
pub fn replay_report() -> String {
    DEVICE.with_borrow_mut(|d| {
        let (status, name, slow) = match d.as_mut() {
            None => return "no replay started".to_string(),
            Some(Device::Gl(b, _)) => (b.status(), b.renderer().to_string(), b.slow_paths()),
            Some(Device::Gpu(b, _, _, name)) => (b.status(), name.clone(), b.slow_paths()),
        };
        let slow: Vec<String> = SLOW_PATHS
            .iter()
            .filter(|&&p| slow.counts[p as usize] > 0)
            .map(|&p| format!("{:?} {}", p, slow.counts[p as usize]))
            .collect();
        format!("{status:?}, {name}; slow paths: {}", slow.join(", "))
    })
}
