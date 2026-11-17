//! The scenes of `webgl-scenes` and the shared back-end suite, built for WebAssembly and run on canvases'
//! WebGL 2 contexts by browser/run.mjs (scenes, page.html) and browser/suite.mjs (the suite,
//! suite.html).
#![cfg(target_arch = "wasm32")]

#[path = "common/checks.rs"]
mod checks;
#[path = "common/kit.rs"]
mod kit;
#[path = "common/scenes.rs"]
mod scenes;

use scenes::{Change, Scene};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wcontainers_gpu::Backend;
use wcontainers_gpu_webgl::WebGl;

#[wasm_bindgen]
extern "C" {
    /// A WebGL 2 context on a new canvas, made by the page.
    #[wasm_bindgen(js_namespace = window, js_name = makeContext)]
    fn make_context() -> web_sys::WebGl2RenderingContext;
    /// The texels the last canvas made shows, read by the page through a 2D canvas, rows top first.
    #[wasm_bindgen(js_namespace = window, js_name = shownTexels)]
    fn shown_texels() -> Vec<u8>;
    /// Hands the page a panic's message before the module traps.
    #[wasm_bindgen(js_namespace = window)]
    fn panicked(message: &str);
}

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

    /// Draws a scene (`change` empty, or the name of a change) and returns its texels.
    pub fn draw(&mut self, scene: &str, change: &str) -> Result<Vec<u8>, JsError> {
        let scene = Scene::from_name(scene).ok_or_else(|| JsError::new("unknown scene"))?;
        let change = Change::from_name(change).ok_or_else(|| JsError::new("unknown change"))?;
        scenes::draw(&mut self.0, scene, change)
            .map_err(|e| JsError::new(&format!("{e}; GL log: {}", self.0.last_log())))
    }

    /// Every picture to draw, one `<scene> <change>` per line.
    pub fn pictures() -> String {
        Change::PICTURES
            .iter()
            .map(|(s, c)| format!("{} {}\n", s.name(), c.name()))
            .collect()
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

thread_local! {
    /// The extensions the suite's devices treat as missing.
    static OFF: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The suite's host in the browser: each device on a new canvas.
struct Chrome;

impl wcontainers_gpu_test::Host for Chrome {
    type Backend = WebGl;

    fn make() -> WebGl {
        OFF.with_borrow(|off| {
            let off: Vec<&str> = off.split_whitespace().collect();
            WebGl::on_canvas(make_context(), &off)
        })
    }

    fn window(_: &mut WebGl) -> u32 {
        0
    }

    fn shown(_: &mut WebGl, _: u32) -> Vec<u8> {
        shown_texels()
    }
}

/// Every suite test, then every check of examples/common/checks.rs, by name.
macro_rules! suite {
    ($($test:ident),* $(,)?; $($check:ident),* $(,)?) => {
        const SUITE: &[(&str, fn())] = &[
            $((stringify!($test), wcontainers_gpu_test::suite::$test::<Chrome>),)*
            $((stringify!($check), checks::$check::<Chrome>),)*
        ];
    };
}

suite!(
    buffer_writes_land_in_order_and_read_back,
    texture_writes_land_and_read_back,
    copies_move_bytes_exactly,
    clears_fill_exactly,
    triangle_covers_the_pixels_whose_centres_it_holds,
    textured_draw_with_depth_and_blending,
    edges_through_pixel_centres_follow_the_top_left_rule,
    strips_take_flat_values_from_the_provoking_vertex,
    depth_writes_off_leave_the_depth,
    a_fragment_covering_no_sample_writes_nothing,
    srgb_targets_convert_colour_but_not_alpha,
    generate_mips_averages_in_the_values_format,
    values_are_interpolated_with_perspective,
    the_scissor_bounds_the_draw,
    stencil_operations_and_tests,
    near_clipping_and_the_viewport_depth_range,
    linear_filtering_and_address_modes,
    instances_start_at_the_first_instance,
    indexed_strips_restart_and_add_the_base_vertex,
    fragments_write_storage_buffers,
    multisampled_targets_cover_their_samples_and_resolve,
    programs_run_per_sample_and_read_samples,
    occlusion_queries_answer_whether_samples_passed,
    present_shows_the_frame;
    vertex_and_instance_numbers_include_the_bases,
    discard_then_no_overwrite_writes,
    depth_and_stencil_read_back,
    stencil_sampled_and_read_back,
    copies_within_a_typeless_group,
    loads_count_levels_from_the_view,
    fractional_viewports_are_refused,
    timestamps_and_their_disjoint_flag,
    multisampled_copies_resolve_as_their_source,
    fractional_depth_bias,
    constant_alpha_blends,
);

thread_local! {
    /// The test `suite_test` chose, which `suite_run` runs.
    static CHOSEN: core::cell::Cell<Option<fn()>> = const { core::cell::Cell::new(None) };
}

/// Chooses the suite test `name`, with `off` (extensions to treat as missing, separated by spaces).
#[wasm_bindgen]
pub fn suite_test(name: &str, off: &str) -> Result<(), JsError> {
    std::panic::set_hook(Box::new(|info| panicked(&info.to_string())));
    let (_, test) = SUITE
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| JsError::new(&format!("the browser host has no suite test {name}")))?;
    OFF.set(off.to_string());
    CHOSEN.set(Some(*test));
    Ok(())
}

/// Runs the chosen test. A plain export, which the page calls through `WebAssembly.promising` so the
/// back end's waits can suspend it. A failing test panics: the page gets the message through
/// `window.panicked` and the module traps.
#[unsafe(no_mangle)]
pub extern "C" fn suite_run() {
    if let Some(test) = CHOSEN.take() {
        test();
    }
}
