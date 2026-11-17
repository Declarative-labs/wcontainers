//! Draws the rig's three reference scenes through the WebGPU back end and saves them as PNG files.
//!
//! Natively: `scenes <dx12|vulkan|metal> <out-dir> [none|blend-factor|depth-function]` writes
//! `webgpu-<api>-<scene>.png`. Built for WebAssembly, the page (web/scenes.html) calls `draw_scene`
//! and `adapter` and saves what they return.

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
    for which in scene::Scene::ALL {
        let rb = draw::render(&mut b, which, twist)?;
        b.wait_read(&rb);
        let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
        b.finish_read(rb, &mut rgba);
        anyhow::ensure!(
            b.status() == DeviceStatus::Ok,
            "device lost: {:?}",
            b.status()
        );
        let path = std::path::Path::new(out).join(format!("webgpu-{api}-{}.png", which.name()));
        std::fs::write(&path, draw::png(&rgba)?)?;
        println!("{}", path.display());
    }
    println!("counters: {:?}", b.counters());
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::{draw, scene};
    use wasm_bindgen::prelude::*;
    use wcontainers_gpu::{Backend, DeviceStatus, ReadState};

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = setTimeout)]
        fn set_timeout(f: &js_sys::Function, ms: i32);
    }

    /// Lets the browser's event loop run, which delivers mapAsync.
    async fn yield_to_browser() {
        let p = js_sys::Promise::new(&mut |resolve, _| set_timeout(&resolve, 1));
        let _ = wasm_bindgen_futures::JsFuture::from(p).await;
    }

    fn js(e: impl std::fmt::Display) -> JsValue {
        JsValue::from_str(&e.to_string())
    }

    /// The adapter the browser gives WebGPU, as the back end sees it.
    #[wasm_bindgen]
    pub async fn adapter() -> Result<String, JsValue> {
        Ok(draw::device(wgpu::Backends::BROWSER_WEBGPU)
            .await
            .map_err(js)?
            .1)
    }

    /// Draws one scene on the browser's WebGPU and returns it as a PNG file.
    #[wasm_bindgen]
    pub async fn draw_scene(name: String, twist: String) -> Result<Vec<u8>, JsValue> {
        let which = scene::Scene::from_name(&name).ok_or_else(|| js("unknown scene"))?;
        let twist = draw::Twist::parse(&twist).map_err(js)?;
        let (mut b, _) = draw::device(wgpu::Backends::BROWSER_WEBGPU)
            .await
            .map_err(js)?;
        let rb = draw::render(&mut b, which, twist).map_err(js)?;
        // In-page polling; the device thread's own wait is the JSPI suspension of plan section 4.8.
        while b.read_state(&rb) == ReadState::Pending {
            if b.status() != DeviceStatus::Ok {
                return Err(js(format!("device lost: {:?}", b.status())));
            }
            yield_to_browser().await;
        }
        let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
        b.finish_read(rb, &mut rgba);
        if b.status() != DeviceStatus::Ok {
            return Err(js(format!("device lost: {:?}", b.status())));
        }
        draw::png(&rgba).map_err(js)
    }
}
