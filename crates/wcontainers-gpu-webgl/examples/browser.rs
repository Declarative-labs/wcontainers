//! The scenes of `webgl-scenes`, built for WebAssembly and drawn on a canvas's WebGL 2 context by
//! browser/run.mjs.
#![cfg(target_arch = "wasm32")]

#[path = "common/scenes.rs"]
mod scenes;

use scenes::{Change, Scene};
use wasm_bindgen::prelude::*;
use wcontainers_gpu::Backend;
use wcontainers_gpu_webgl::WebGl;

/// One back end on one canvas, kept between calls as a front would keep it.
#[wasm_bindgen]
pub struct Device(WebGl);

#[wasm_bindgen]
impl Device {
    /// Makes the back end; `off` lists extensions to treat as missing, separated by spaces.
    #[wasm_bindgen(constructor)]
    pub fn new(context: web_sys::WebGl2RenderingContext, off: &str) -> Device {
        let off: Vec<&str> = off.split_whitespace().collect();
        Device(WebGl::on_canvas(context, &off))
    }

    /// Draws a scene (`change` empty, or the name of a change to the quads) and returns its texels.
    pub fn draw(&mut self, scene: &str, change: &str) -> Result<Vec<u8>, JsError> {
        let scene = Scene::from_name(scene).ok_or_else(|| JsError::new("unknown scene"))?;
        let change = std::iter::once(Change::None)
            .chain(Change::ALTERED)
            .find(|c| c.name() == change)
            .ok_or_else(|| JsError::new("unknown change"))?;
        scenes::draw(&mut self.0, scene, change)
            .map_err(|e| JsError::new(&format!("{e}; GL log: {}", self.0.last_log())))
    }

    /// The renderer, the extensions, and the capability list, as text.
    pub fn report(&self) -> String {
        let used: Vec<_> = self.0.extensions().collect();
        format!(
            "renderer: {}\nextensions used: {}\ncontext extensions: {}\n{}",
            self.0.renderer(),
            used.join(" "),
            self.0.context_extensions().join(" "),
            scenes::caps_report(self.0.caps()),
        )
    }
}
