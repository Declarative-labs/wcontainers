//! Draws the rig's three reference scenes (and the quads again on depth and stencil, `draw::PICTURES`)
//! through the WebGPU back end and saves them as PNG files.
//!
//! Natively: `scenes <dx12|vulkan|metal> <out-dir> [none|blend-factor|depth-function]` writes
//! `webgpu-<api>-<picture>.png`. Built for WebAssembly, web/scenes.html calls `draw_scene` and
//! `adapter` and saves what they return, and web/suite.html runs the shared suite (suite.rs).

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
