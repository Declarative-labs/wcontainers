//! The scenes of `webgl-scenes` and the shared back-end suite, built for WebAssembly and run on canvases'
//! WebGL 2 contexts by browser/run.mjs (scenes, page.html) and browser/suite.mjs (the suite,
//! suite.html); and the comparisons with the WebGPU back end on Chrome's WebGPU, by
//! browser/against.mjs (against.html).
#![cfg(target_arch = "wasm32")]

#[path = "common/against.rs"]
mod against;
#[path = "common/checks.rs"]
mod checks;
#[path = "common/kit.rs"]
mod kit;
#[path = "common/rig.rs"]
mod rig;
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
    /// The rig's pictures are scene `rig`, their names as the change; one the back end lacks a
    /// feature for gives no texels.
    pub fn draw(&mut self, scene: &str, change: &str) -> Result<Vec<u8>, JsError> {
        if scene == "rig" {
            let output = rig::OUTPUTS
                .iter()
                .find(|o| o.name == change)
                .ok_or_else(|| JsError::new("unknown rig picture"))?;
            return rig::draw(&mut self.0, output)
                .map(Option::unwrap_or_default)
                .map_err(|e| JsError::new(&format!("{e}; GL log: {}", self.0.last_log())));
        }
        let scene = Scene::from_name(scene).ok_or_else(|| JsError::new("unknown scene"))?;
        let change = Change::from_name(change).ok_or_else(|| JsError::new("unknown change"))?;
        scenes::draw(&mut self.0, scene, change)
            .map_err(|e| JsError::new(&format!("{e}; GL log: {}", self.0.last_log())))
    }

    /// Every picture to draw, one `<scene> <change>` per line.
    pub fn pictures() -> String {
        let scenes = Change::PICTURES
            .iter()
            .map(|(s, c)| format!("{} {}\n", s.name(), c.name()));
        let rig = rig::OUTPUTS.iter().map(|o| format!("rig {}\n", o.name));
        scenes.chain(rig).collect()
    }

    /// The renderer, the extensions, and the capability list, as text.
    pub fn report(&self) -> String {
        let used: Vec<_> = self.0.extensions().collect();
        format!(
            "renderer: {}\nextensions used: {}\ncontext extensions: {}\nexact float blits: {}\n{}",
            self.0.renderer(),
            used.join(" "),
            self.0.context_extensions().join(" "),
            self.0.exact_float_blits(),
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
            $((stringify!($check), || {
                let (seed, cases) = SAMPLING.get();
                checks::seeded(checks::$check::<Chrome>, seed, cases)
            }),)*
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
    a_scissor_past_the_target_is_clipped_to_it,
    clear_rects_clear_each_rectangle_clipped_to_the_view,
    copies_between_depth_stencil_textures_move_both_aspects,
    stencil_operations_and_tests,
    near_clipping_and_the_viewport_depth_range,
    linear_filtering_and_address_modes,
    instances_start_at_the_first_instance,
    indexed_strips_restart_and_add_the_base_vertex,
    a_stream_of_stride_zero_reads_its_first_element,
    comparison_samplers_compare_with_the_stored_depth,
    fragments_write_storage_buffers,
    multisampled_targets_cover_their_samples_and_resolve,
    resolves_average_in_the_format_named,
    programs_run_per_sample_and_read_samples,
    occlusion_queries_answer_whether_samples_passed,
    fill_modes_draw_edges_or_corners,
    values_pass_between_programs_by_location,
    vertices_pulled_from_storage_match_the_fetched_ones,
    constants_read_through_a_texture_lie_256_rows_to_a_texture_row,
    missing_components_read_their_defaults,
    typed_buffers_through_storage_read_their_views_place_and_length,
    typed_buffers_through_a_texture_lie_2048_elements_to_a_row,
    captures_write_location_outputs_packed_in_draw_order,
    a_copy_within_one_subresource_moves_the_region,
    present_shows_the_frame;
    vertex_and_instance_numbers_include_the_bases,
    discard_then_no_overwrite_writes,
    depth_and_stencil_read_back,
    stencil_sampled_and_read_back,
    copies_within_a_typeless_group,
    cube_faces_copied_between_typeless_formats,
    copies_within_one_subresource,
    copies_within_one_float_format_take_the_probed_route,
    depth_and_stencil_written_read_back_as_written,
    copies_between_depth_and_colour_formats,
    copies_between_index_and_data_buffers,
    loads_count_levels_from_the_view,
    fractional_viewports_are_refused,
    viewports_past_their_bounds_are_refused,
    immediates_reach_both_stages,
    packed_vertex_formats_are_converted,
    timestamps_and_their_disjoint_flag,
    multisampled_copies_resolve_as_their_source,
    fractional_depth_bias,
    constant_alpha_blends,
    vertices_captured_with_and_without_rasterising,
    captured_indices_drawn_both_ways,
    block_compressed_and_shared_exponent_textures_read_back_as_written,
    interpolated_bc4_and_bc5_values,
    fragment_depth_follows_the_clip_convention,
    strips_fans_loops_and_indexed_draws_captured_in_list_order,
    absent_features_are_refused,
    copies_into_and_out_of_host_kept_textures,
    srgb_views_of_block_compressed_textures,
    bgra_textures_keep_their_byte_order,
    sampled_views_are_whole_textures_of_one_mip_range,
    generate_mipmap_equals_the_draws,
    sixteen_bit_normalised_formats_follow_their_extension,
    bgra_surfaces_present,
    signed_normalised_formats_follow_their_extension,
    levels_stay_as_written_while_others_are_read_and_drawn,
    passes_without_attachments_rasterise,
    independent_blends_per_target,
    second_source_blends,
    depth_clamped_to_the_viewport_range,
    depth_bias_clamped,
    polygons_filled_with_lines,
    mirror_once_addressing,
    clip_and_cull_distances,
    points_sized_by_the_program,
    broken_rules_lose_the_device,
);

thread_local! {
    /// The seed of the checks' patterns and how many seeds from it each check runs.
    static SAMPLING: core::cell::Cell<(u32, u32)> = const { core::cell::Cell::new((0, 1)) };
    /// The test `suite_test` chose, which `suite_run` runs.
    static CHOSEN: core::cell::Cell<Option<fn()>> = const { core::cell::Cell::new(None) };
}

/// Chooses the suite test `name`, with `off` (extensions to treat as missing, separated by spaces);
/// a check of checks.rs runs `cases` times, with its patterns' seeds from `seed` up.
#[wasm_bindgen]
pub fn suite_test(name: &str, off: &str, seed: u32, cases: u32) -> Result<(), JsError> {
    SAMPLING.set((seed, cases));
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
/// The features whose check did not run because the back end declares them absent, empty when
/// every route of the chosen test ran.
#[wasm_bindgen]
pub fn suite_not_run() -> String {
    let absent = checks::take_not_run();
    if absent.is_empty() {
        String::new()
    } else {
        format!("{absent:?} absent")
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn suite_run() {
    if let Some(test) = CHOSEN.take() {
        test();
    }
}

/// The comparisons with the WebGPU back end, by name, separated by spaces.
#[wasm_bindgen]
pub fn against_names() -> String {
    let names: Vec<_> = against::COMPARISONS.iter().map(|c| c.name).collect();
    names.join(" ")
}

// JSPI exports, which wasm-bindgen still marks experimental.
#[allow(deprecated)]
mod comparisons {
    use super::*;
    use wcontainers_gpu_webgpu::{WebGpu, block_on};

    /// Runs the comparison `name`: the reference on the browser's WebGPU, each route on a new
    /// canvas. One line for the adapter, then `<route>: ok` or `<route>: FAILED <why>` per route, each
    /// on one line. A
    /// JSPI export: the WebGPU back end waits for its probe by suspending.
    #[wasm_bindgen(jspi)]
    pub fn against_run(name: String) -> Result<String, JsError> {
        std::panic::set_hook(Box::new(|info| panicked(&info.to_string())));
        let c = against::COMPARISONS
            .iter()
            .find(|c| c.name == name)
            .ok_or_else(|| JsError::new(&format!("no comparison {name}")))?;
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::BROWSER_WEBGPU;
        let instance = wgpu::Instance::new(desc);
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| JsError::new(&format!("no WebGPU adapter: {e}")))?;
        let info = adapter.get_info();
        let mut device_desc = WebGpu::device_descriptor(&adapter);
        if c.reference == against::Reference::WithoutFloatFiltering {
            device_desc
                .required_features
                .remove(wgpu::Features::FLOAT32_FILTERABLE);
        }
        let (device, queue) = block_on(adapter.request_device(&device_desc))
            .map_err(|e| JsError::new(&format!("no WebGPU device: {e}")))?;
        let mut gpu = WebGpu::new(&adapter, device, queue);
        let verdicts = against::compare(c, &mut gpu, |off| WebGl::on_canvas(make_context(), off));
        let mut lines = vec![format!(
            "webgpu adapter: {} ({:?}, {:?})",
            info.name, info.device_type, info.backend
        )];
        lines.extend(verdicts.into_iter().map(|(route, v)| match v {
            Ok(()) => format!("{route}: ok"),
            Err(e) => format!("{route}: FAILED {}", e.replace('\n', " ")),
        }));
        Ok(lines.join("\n"))
    }
}
