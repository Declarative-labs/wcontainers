//! Draws the rig's three reference scenes (and the quads again on depth and stencil, `draw::PICTURES`)
//! through the WebGPU back end and saves them as PNG files.
//!
//! Natively: `scenes <dx12|vulkan|metal> <out-dir> [none|blend-factor|depth-function]` writes
//! `webgpu-<api>-<picture>.png`. Built for WebAssembly, web/scenes.html calls `draw_scene` and
//! `adapter` and saves what they return, times first draws with and without `prepare`
//! (`prepare_timings`, prepare.rs), checks timestamps (`timestamps`, timestamps.rs) and the formats
//! held converted (`converted`, converted.rs) and the vertex formats WebGPU lacks
//! (`vertex_formats`, vertices.rs), and web/suite.html runs the shared suite (suite.rs).

#[path = "../../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

mod draw;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use wcontainers_gpu::{Backend, DeviceStatus};
    let args: Vec<String> = std::env::args().collect();
    let usage = "usage: scenes <dx12|vulkan|metal> <out-dir> [none|blend-factor|depth-function]";
    let (Some(api), Some(out)) = (args.get(1), args.get(2)) else {
        anyhow::bail!(usage);
    };
    let backends = match api.as_str() {
        "dx12" => wgpu::Backends::DX12,
        "vulkan" => wgpu::Backends::VULKAN,
        "metal" => wgpu::Backends::METAL,
        _ => anyhow::bail!(usage),
    };
    let twist = draw::Twist::parse(args.get(3).map_or("none", String::as_str))?;
    std::fs::create_dir_all(out)?;
    let (mut b, adapter) = pollster::block_on(draw::device(backends))?;
    println!("adapter: {adapter}");
    for (name, which, depth) in draw::PICTURES {
        let rb = draw::render_on(&mut b, which, twist, depth)?;
        b.wait_read(&rb);
        let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
        b.finish_read(rb, &mut rgba);
        anyhow::ensure!(
            b.status() == DeviceStatus::Ok,
            "device lost: {:?}",
            b.status()
        );
        let path = std::path::Path::new(out).join(format!("webgpu-{api}-{name}.png"));
        std::fs::write(&path, draw::png(&rgba)?)?;
        println!("{}", path.display());
    }
    println!("counters: {:?}", b.counters());
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
mod viewports;

#[cfg(target_arch = "wasm32")]
mod prepare;

#[cfg(target_arch = "wasm32")]
mod timestamps;

#[cfg(target_arch = "wasm32")]
mod converted;

#[cfg(target_arch = "wasm32")]
mod vertices;

#[cfg(target_arch = "wasm32")]
#[macro_use]
mod names;

// JSPI exports and imports, which wasm-bindgen still marks experimental.
#[cfg(target_arch = "wasm32")]
#[allow(deprecated)]
mod suite;

#[cfg(target_arch = "wasm32")]
#[allow(deprecated)]
mod web {
    use super::{draw, scene};
    use wasm_bindgen::prelude::*;
    use wcontainers_gpu::{Backend, DeviceStatus};
    use wcontainers_gpu_webgpu::block_on;

    fn js(e: impl std::fmt::Display) -> JsError {
        JsError::new(&e.to_string())
    }

    /// The adapter the browser gives WebGPU, as the back end sees it, and what the depth probe
    /// found there.
    #[wasm_bindgen(jspi)]
    pub fn adapter() -> Result<String, JsError> {
        let (b, name) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        Ok(format!("{name}; depth24plus {:?}", b.depth24()))
    }

    /// Draws one picture of `draw::PICTURES` on the browser's WebGPU and returns it as a PNG file.
    #[wasm_bindgen(jspi)]
    pub fn draw_scene(name: String, twist: String) -> Result<Vec<u8>, JsError> {
        let (_, which, depth) = draw::PICTURES
            .into_iter()
            .find(|p| p.0 == name)
            .ok_or_else(|| js("unknown picture"))?;
        let twist = draw::Twist::parse(&twist).map_err(js)?;
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        let rb = draw::render_on(&mut b, which, twist, depth).map_err(js)?;
        b.wait_read(&rb);
        let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
        b.finish_read(rb, &mut rgba);
        if b.status() != DeviceStatus::Ok {
            return Err(js(format!("device lost: {:?}", b.status())));
        }
        draw::png(&rgba).map_err(js)
    }

    /// Milliseconds by the page's `performance.now()`.
    fn now() -> f64 {
        let get = |of: &JsValue, name: &str| js_sys::Reflect::get(of, &JsValue::from_str(name));
        let performance = get(&js_sys::global(), "performance").expect("a performance clock");
        let call: js_sys::Function = get(&performance, "now").expect("performance.now").into();
        call.call0(&performance)
            .ok()
            .and_then(|t| t.as_f64())
            .expect("a time")
    }

    /// `programs` new programs drawn first unprepared and then, others, after `prepare`, on the
    /// browser's WebGPU: one line per half, as tests/prepare.rs prints natively.
    #[wasm_bindgen(jspi)]
    pub fn prepare_timings(programs: u32) -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        let unprepared = super::prepare::first_draws(&mut b, 0, programs, false, now);
        let prepared = super::prepare::first_draws(&mut b, 1000, programs, true, now);
        Ok(format!(
            "{}\n{}",
            unprepared.line(false),
            prepared.line(true)
        ))
    }

    /// Timestamps around work and the GPU clock against `performance.now()` on the browser's
    /// WebGPU: one line of what was measured, or the failed check and that line.
    #[wasm_bindgen(jspi)]
    pub fn timestamps() -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        let stamps = super::timestamps::measure(&mut b, now).map_err(js)?;
        stamps
            .check()
            .map_err(|e| js(format!("{e}; {}", stamps.line())))?;
        Ok(stamps.line())
    }

    /// The formats held converted, checked on the browser's WebGPU: one line of the largest
    /// differences, or the failed check.
    #[wasm_bindgen(jspi)]
    pub fn converted() -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        super::converted::check(&mut b).map_err(js)
    }

    /// The vertex formats WebGPU lacks, `n` seeded vertices each, read through shadow streams on
    /// the browser's WebGPU; fails at the first value read wrong.
    #[wasm_bindgen(jspi)]
    pub fn vertex_formats(seed: u64, n: u32) -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        super::vertices::lacking_vertex_formats(&mut b, seed, n);
        Ok(format!(
            "vertex formats: seed {seed}, {n} vertices per format, all as they mean"
        ))
    }

    /// Draws the eight-viewport scene (`rounded`: its viewports moved to whole pixels) on the
    /// browser's WebGPU and returns it as a PNG file.
    #[wasm_bindgen(jspi)]
    pub fn draw_viewports(rounded: bool) -> Result<Vec<u8>, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        let rb = super::viewports::draw(&mut b, rounded);
        b.wait_read(&rb);
        let (w, h) = super::viewports::SIZE;
        let mut rgba = vec![0; (w * h * 4) as usize];
        b.finish_read(rb, &mut rgba);
        if b.status() != DeviceStatus::Ok {
            return Err(js(format!("device lost: {:?}", b.status())));
        }
        draw::png_of(&rgba, w, h).map_err(js)
    }
}
