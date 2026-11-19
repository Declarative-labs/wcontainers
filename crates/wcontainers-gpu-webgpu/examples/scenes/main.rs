//! Draws the rig's three reference scenes (and the quads again on depth and stencil, `draw::PICTURES`)
//! through the WebGPU back end and saves them as PNG files.
//!
//! Natively: `scenes <dx12|vulkan|metal> <out-dir> [none|blend-factor|depth-function]` writes
//! `webgpu-<api>-<picture>.png`. Built for WebAssembly, web/scenes.html calls `draw_scene` and
//! `adapter` and saves what they return, times first draws with and without `prepare`
//! (`prepare_timings`, prepare.rs), checks timestamps (`timestamps`, timestamps.rs) and the formats
//! held converted (`converted`, converted.rs) and the vertex formats WebGPU lacks
//! (`vertex_formats`, vertices.rs), checks query sets past 4096 and the query ring (`query_sets`,
//! queries.rs), checks subgroups, the primitive index and fine derivatives (`program_features`, features.rs),
//! measures instance data fetched in the program (`pulled_costs`,
//! pulled.rs), and web/suite.html runs the shared suite (suite.rs).

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
mod depth16;

#[cfg(target_arch = "wasm32")]
mod timestamps;

#[cfg(target_arch = "wasm32")]
mod converted;

#[cfg(target_arch = "wasm32")]
mod vertices;

#[cfg(target_arch = "wasm32")]
mod renames;

#[cfg(target_arch = "wasm32")]
mod features;

#[cfg(target_arch = "wasm32")]
mod gaps;

#[cfg(target_arch = "wasm32")]
mod pulled;

#[cfg(target_arch = "wasm32")]
mod queries;

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
    /// browser's WebGPU: one line per half, as tests/prepare.rs prints natively, and a line for a
    /// prepared strip drawn with indices (an error if it built anything while drawing).
    #[wasm_bindgen(jspi)]
    pub fn prepare_timings(programs: u32) -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        let unprepared = super::prepare::first_draws(&mut b, 0, programs, false, now);
        let prepared = super::prepare::first_draws(&mut b, 1000, programs, true, now);
        let strips = super::prepare::prepared_strips(&mut b).map_err(js)?;
        Ok(format!(
            "{}\n{}\n{strips}",
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

    /// The formats held converted, the three-channel ones too, checked on the browser's WebGPU:
    /// a line of the largest differences and one of the three-channel formats, or the failed
    /// check.
    #[wasm_bindgen(jspi)]
    pub fn converted(seed: u64, cases: u32) -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        let line = super::converted::check(&mut b, seed, cases).map_err(js)?;
        let three = super::converted::three_channels(&mut b, seed, cases).map_err(js)?;
        let widest = super::converted::three_channels_at_widest(&mut b, seed).map_err(js)?;
        Ok(format!("{line}\n{three}\n{widest}"))
    }

    /// The vertex formats WebGPU lacks, `n` seeded vertices each, read through shadow streams on
    /// the browser's WebGPU; fails at the first value read wrong.
    #[wasm_bindgen(jspi)]
    pub fn vertex_formats(seed: u64, n: u32) -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        super::vertices::lacking_vertex_formats(&mut b, seed, n);
        let bgra = super::vertices::bgra_vertices(&mut b, seed, n).map_err(js)?;
        Ok(format!(
            "vertex formats: seed {seed}, {n} vertices per format, all as they mean\n{bgra}"
        ))
    }

    /// Textures written with `Discard` behind recorded draws, renamed from the pool on the
    /// browser's WebGPU; fails at the first wrong pixel or count.
    #[wasm_bindgen(jspi)]
    pub fn texture_renames() -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        super::renames::discarded_textures_are_renamed_from_the_pool(&mut b);
        Ok(
            "texture renames: four frames of four discarded writes, three renames a frame, \
            from the pool after the first"
                .into(),
        )
    }

    /// Copies between `depth16unorm` and its typeless group's colour formats, whole and in part,
    /// on the browser's WebGPU: one line, or the failed check.
    #[wasm_bindgen(jspi)]
    pub fn depth_group_copies() -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        super::depth16::group_copies(&mut b).map_err(js)
    }

    /// The program features of `features.rs` on the browser's WebGPU: one line each, saying
    /// whether the device declares it and, where it does, that it drew as defined; or the failed
    /// check, also where the browser's adapter offers a feature (`offered`, its `GPUFeatureName`s)
    /// the device does not declare.
    #[wasm_bindgen(jspi)]
    pub fn program_features(offered: Vec<String>) -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        let mut lines = Vec::new();
        for f in super::features::Feature::ALL {
            let offers = f
                .web_name()
                .is_none_or(|name| offered.iter().any(|o| o == name));
            if offers && !b.caps().has(f.declared()) {
                return Err(js(format!(
                    "the adapter offers {}, the device does not declare it",
                    f.name()
                )));
            }
            lines.push(super::features::check(&mut b, f).map_err(js)?);
        }
        Ok(lines.join("\n"))
    }

    /// Texture reads into buffers that keep the bytes between rows (`gaps.rs`, `seed` picking the
    /// texels) on the browser's WebGPU: one line, or the failed check.
    #[wasm_bindgen(jspi)]
    pub fn copy_gaps(seed: u64) -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        super::gaps::reads_keep_the_bytes_between_rows(&mut b, seed).map_err(js)
    }

    /// Query sets past WebGPU's 4096 queries a set, and query results from one ring, on the
    /// browser's WebGPU: one line each, or the failed check.
    #[wasm_bindgen(jspi)]
    pub fn query_sets() -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        let paged = super::queries::paged_sets(&mut b).map_err(js)?;
        let ring = super::queries::results_ring(&mut b, 20).map_err(js)?;
        Ok(format!("{paged}\n{ring}"))
    }

    /// Instance data fetched in the program against vertex buffers on the browser's WebGPU, at 4,
    /// 8 and the device's most streams a draw, `frames` frames each: one line per route and count,
    /// or the failed check.
    #[wasm_bindgen(jspi)]
    pub fn pulled_costs(frames: u32) -> Result<String, JsError> {
        let (mut b, _) = block_on(draw::device(wgpu::Backends::BROWSER_WEBGPU)).map_err(js)?;
        let most = super::pulled::most_streams(&b);
        let mut streams = vec![4, 8.min(most), most];
        streams.dedup();
        let mut lines = Vec::new();
        for n in streams {
            for t in super::pulled::measure(&mut b, n, frames, now).map_err(js)? {
                lines.push(t.line());
            }
        }
        Ok(lines.join("\n"))
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
