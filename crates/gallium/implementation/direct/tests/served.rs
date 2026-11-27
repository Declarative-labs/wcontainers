// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! The serving glue's first slice under its hostile C caller (caller.c): each case on a screen of the recording
//! implementation, alone in a process of its own; what C sees (the case's status) and what reached the implementation
//! (the recording's log, in order) are both checked. Built against the pinned Mesa with the `hostile` feature.
#![cfg(all(mesa, feature = "hostile"))]

use std::process::Command;
use std::sync::Arc;

use wcontainers_gallium_recording::Recording;

#[expect(unsafe_code, reason = "caller.c defines wc_caller_cases and wc_caller_case as declared here")]
unsafe extern "C" {
    fn wc_caller_cases() -> i32;
    fn wc_caller_case(address: usize, which: i32) -> i32;
    fn wc_caller_caps(address: usize) -> i32;
}

/// The environment variable naming the one case a child process runs.
const CASE_VARIABLE: &str = "WCONTAINERS_GALLIUM_CASE";

/// The methods serve.c calls while it makes the screen, before any case: each stage's capabilities (the eight of
/// MESA_SHADER_MESH_STAGES), the screen's, the compute capabilities.
const SCREEN_MADE: &[&str] = &["shader_caps", "shader_caps", "shader_caps", "shader_caps", "shader_caps", "shader_caps", "shader_caps", "shader_caps", "caps", "compute_caps"];

/// What each case must leave in the log after the screen is made: the methods that reached the implementation, in order.
const EXPECTED: &[&[&str]] = &[
    &[
        "context_create",
        "create_blend_state",
        "bind_blend_state",
        "bind_blend_state",
        "delete_blend_state",
        "create_depth_stencil_alpha_state",
        "bind_depth_stencil_alpha_state",
        "delete_depth_stencil_alpha_state",
        "create_rasterizer_state",
        "bind_rasterizer_state",
        "delete_rasterizer_state",
        "create_sampler_state",
        "create_sampler_state",
        "bind_sampler_states",
        "delete_sampler_state",
        "delete_sampler_state",
        "create_vertex_elements_state",
        "bind_vertex_elements_state",
        "delete_vertex_elements_state",
        "destroy",
        "destroy",
    ],
    &["context_create", "destroy", "destroy"],
    &["context_create", "create_blend_state", "delete_blend_state", "destroy", "destroy"],
    &["context_create", "create_rasterizer_state", "delete_rasterizer_state", "destroy", "destroy"],
    &["context_create", "destroy", "destroy"],
    &["context_create", "destroy", "destroy"],
];

/// The calls the first case's states must reach the implementation as, in full: every field caller.c's `filled_*`
/// sets, with the value it sets.
const FILLED: &[&str] = &[
    "create_blend_state arg1 BlendState { independent_blend_enable: 1, logicop_enable: 1, logicop_func: Logicop(4), dither: 1, alpha_to_coverage: 1, alpha_to_coverage_dither: 1, alpha_to_one: 1, max_rt: 2, advanced_blend_func: 10, blend_coherent: 1, is_yuv: 1, rt: [RtBlendState { blend_enable: 1, rgb_func: 7, rgb_src_factor: Blendfactor(15), rgb_dst_factor: Blendfactor(16), alpha_func: 3, alpha_src_factor: Blendfactor(18), alpha_dst_factor: Blendfactor(19), colormask: 5 }, RtBlendState { blend_enable: 1, rgb_func: 1, rgb_src_factor: Blendfactor(23), rgb_dst_factor: Blendfactor(24), alpha_func: 4, alpha_src_factor: Blendfactor(26), alpha_dst_factor: Blendfactor(27), colormask: 13 }, RtBlendState { blend_enable: 1, rgb_func: 2, rgb_src_factor: Blendfactor(31), rgb_dst_factor: Blendfactor(1), alpha_func: 5, alpha_src_factor: Blendfactor(3), alpha_dst_factor: Blendfactor(4), colormask: 6 }, RtBlendState { blend_enable: 1, rgb_func: 3, rgb_src_factor: Blendfactor(8), rgb_dst_factor: Blendfactor(9), alpha_func: 6, alpha_src_factor: Blendfactor(11), alpha_dst_factor: Blendfactor(12), colormask: 14 }, RtBlendState { blend_enable: 1, rgb_func: 4, rgb_src_factor: Blendfactor(16), rgb_dst_factor: Blendfactor(17), alpha_func: 7, alpha_src_factor: Blendfactor(19), alpha_dst_factor: Blendfactor(20), colormask: 7 }, RtBlendState { blend_enable: 1, rgb_func: 5, rgb_src_factor: Blendfactor(24), rgb_dst_factor: Blendfactor(25), alpha_func: 1, alpha_src_factor: Blendfactor(27), alpha_dst_factor: Blendfactor(28), colormask: 15 }, RtBlendState { blend_enable: 1, rgb_func: 6, rgb_src_factor: Blendfactor(1), rgb_dst_factor: Blendfactor(2), alpha_func: 2, alpha_src_factor: Blendfactor(4), alpha_dst_factor: Blendfactor(5), colormask: 8 }, RtBlendState { blend_enable: 1, rgb_func: 7, rgb_src_factor: Blendfactor(9), rgb_dst_factor: Blendfactor(10), alpha_func: 3, alpha_src_factor: Blendfactor(12), alpha_dst_factor: Blendfactor(13), colormask: 1 }] }",
    "create_depth_stencil_alpha_state arg1 DepthStencilAlphaState { stencil: [StencilState { enabled: 1, func: CompareFunc(1), fail_op: StencilOp(2), zpass_op: StencilOp(3), zfail_op: StencilOp(4), valuemask: 82, writemask: 83 }, StencilState { enabled: 1, func: CompareFunc(1), fail_op: StencilOp(2), zpass_op: StencilOp(3), zfail_op: StencilOp(4), valuemask: 89, writemask: 90 }], alpha_enabled: 1, alpha_func: CompareFunc(1), depth_enabled: 1, depth_writemask: 1, depth_func: CompareFunc(4), depth_bounds_test: 1, alpha_ref_value: 1096.25, depth_bounds_min: 1097.25, depth_bounds_max: 1098.25 }",
    "create_rasterizer_state arg1 RasterizerState { flatshade: 1, light_twoside: 1, clamp_vertex_color: 1, clamp_fragment_color: 1, front_ccw: 1, cull_face: 3, fill_front: 1, fill_back: 2, offset_point: 1, offset_line: 1, offset_tri: 1, scissor: 1, poly_smooth: 1, poly_stipple_enable: 1, point_smooth: 1, sprite_coord_mode: SpriteCoordMode(1), point_quad_rasterization: 1, point_line_tri_clip: 1, point_size_per_vertex: 1, multisample: 1, no_ms_sample_mask_out: 1, force_persample_interp: 1, line_smooth: 1, line_stipple_enable: 1, line_last_pixel: 1, line_rectangular: 1, conservative_raster_mode: ConservativeRasterMode(3), flatshade_first: 1, half_pixel_center: 1, bottom_edge_rule: 1, subpixel_precision_x: 10, subpixel_precision_y: 11, rasterizer_discard: 1, rasterization_stream: 1, tile_raster_order_fixed: 1, tile_raster_order_increasing_x: 1, tile_raster_order_increasing_y: 1, depth_clip_near: 1, depth_clip_far: 1, depth_clamp: 1, clip_halfz: 1, offset_units_unscaled: 1, unclamped_fragment_depth_values: 1, clip_plane_enable: 143, representative_fragment_test: 1, line_stipple_factor: 145, line_stipple_pattern: 146, sprite_coord_enable: 1146, line_width: 1147.25, point_size: 1148.25, offset_units: 1149.25, offset_scale: 1150.25, offset_clamp: 1151.25, conservative_raster_dilate: 1152.25 }",
    "create_sampler_state arg1 SamplerState { wrap_s: TexWrap(7), wrap_t: TexWrap(1), wrap_r: TexWrap(2), min_img_filter: TexFilter(1), min_mip_filter: TexMipfilter(2), mag_img_filter: TexFilter(1), compare_mode: TexCompare(1), compare_func: CompareFunc(7), unnormalized_coords: 1, max_anisotropy: 8, seamless_cube_map: 1, border_color_is_integer: 1, reduction_mode: TexReductionMode(1), pad: 12, lod_bias: 1167.25, min_lod: 1168.25, max_lod: 1169.25, border_color: [1170, 1171, 1172, 1173], border_color_format: Format(1174) }",
    "create_sampler_state arg1 SamplerState { wrap_s: TexWrap(1), wrap_t: TexWrap(2), wrap_r: TexWrap(3), min_img_filter: TexFilter(1), min_mip_filter: TexMipfilter(3), mag_img_filter: TexFilter(1), compare_mode: TexCompare(1), compare_func: CompareFunc(1), unnormalized_coords: 1, max_anisotropy: 30, seamless_cube_map: 1, border_color_is_integer: 1, reduction_mode: TexReductionMode(2), pad: 3, lod_bias: 1189.25, min_lod: 1190.25, max_lod: 1191.25, border_color: [1192, 1193, 1194, 1195], border_color_format: Format(1196) }",
    "create_vertex_elements_state arg2 [VertexElement { src_offset: 1197, vertex_buffer_index: 72, dual_slot: true, src_format: 1, src_stride: 1201, instance_divisor: 1202 }, VertexElement { src_offset: 1203, vertex_buffer_index: 78, dual_slot: true, src_format: 7, src_stride: 1207, instance_divisor: 1208 }]",
];
/// How every case makes its context: caller.c's private pointer (0x5a5a) and PIPE_CONTEXT_ROBUST_BUFFER_ACCESS (1 << 2).
const CONTEXT_CREATED: &str = "context_create priv_ 23130 flags 4";

/// The first case's whole log, as the implementation must receive it: each state as caller.c filled it (`FILLED`),
/// each object the recording made (numbered in the order made) bound, unbound and given back, the samplers bound at the
/// geometry stage (MESA_SHADER_GEOMETRY, 3) from slot 3.
fn first_case() -> Vec<String> {
    let object = |kind: &str, number: u32| format!("Object {{ kind: {kind:?}, number: {number} }}");
    // The screen made: each stage's capabilities, then the screen's and the compute ones, each as Mesa's defaults (the
    // trait's Default; `caps_are_mesas_defaults` holds them to Mesa's own function).
    let mut log: Vec<String> = (0..8).map(|stage| format!("shader_caps stage {:?} caps {:?}", wcontainers_gallium::values::MesaShaderStage(stage), wcontainers_gallium::ShaderCaps::default())).collect();
    log.push(format!("caps caps {:?}", wcontainers_gallium::Caps::default()));
    log.push(format!("compute_caps caps {:?}", wcontainers_gallium::ComputeCaps::default()));
    log.push(CONTEXT_CREATED.to_string());
    for (at, kind, call) in [(0, "Blend", "blend"), (1, "DepthStencilAlpha", "depth_stencil_alpha"), (2, "Rasterizer", "rasterizer")] {
        log.push(FILLED[at].to_string());
        log.push(format!("bind_{call}_state arg1 Some({})", object(kind, at.try_into().unwrap_or(0))));
        if at == 0 {
            log.push("bind_blend_state arg1 None".to_string());
        }
        log.push(format!("delete_{call}_state arg1 {}", object(kind, at.try_into().unwrap_or(0))));
    }
    log.push(FILLED[3].to_string());
    log.push(FILLED[4].to_string());
    log.push(format!("bind_sampler_states shader MesaShaderStage(3) start_slot 3 samplers [Some({}), Some({})]", object("Sampler", 3), object("Sampler", 4)));
    log.push(format!("delete_sampler_state arg1 {}", object("Sampler", 3)));
    log.push(format!("delete_sampler_state arg1 {}", object("Sampler", 4)));
    log.push(FILLED[5].to_string());
    log.push(format!("bind_vertex_elements_state arg1 Some({})", object("VertexElements", 5)));
    log.push(format!("delete_vertex_elements_state arg1 {}", object("VertexElements", 5)));
    log.push("destroy".to_string());
    log.push("destroy".to_string());
    log
}

/// One case on a new screen of the recording implementation: the case's status and the log.
fn run(which: i32) -> (i32, Vec<String>) {
    let recording = Recording::new();
    let log = Arc::clone(&recording.log);
    let address = wcontainers_gallium_direct::serve::serve(recording).expect("the screen is served");
    #[expect(unsafe_code, reason = "caller.c: wc_caller_case takes the address serve returned (serve.c's live struct pipe_screen) and a case number, and calls only that screen's pointers")]
    // SAFETY: the address is the screen serve made just now, not yet destroyed; the case destroys it last.
    let status = unsafe { wc_caller_case(address, which) };
    (status, log.calls())
}

/// Checks one case: its status 0, the methods that reached the implementation exactly those expected, and the first
/// case's arguments.
fn holds(which: i32) -> Result<(), String> {
    let (status, calls) = run(which);
    let names: Vec<&str> = calls.iter().map(|call| call.split(' ').next().unwrap_or_default()).collect();
    let case = EXPECTED.get(usize::try_from(which).unwrap_or(usize::MAX)).ok_or("no such case")?;
    let expected: Vec<&str> = SCREEN_MADE.iter().chain(case.iter()).copied().collect();
    if status != 0 {
        return Err(format!("case {which}: C saw status {status}; calls {names:?}"));
    }
    if names != *expected {
        return Err(format!("case {which}: calls {names:?}, expected {expected:?}"));
    }
    if calls.get(SCREEN_MADE.len()).map(String::as_str) != Some(CONTEXT_CREATED) {
        return Err(format!("case {which}: the context made as {:?}, expected {CONTEXT_CREATED}", calls.get(SCREEN_MADE.len())));
    }
    if which == 0 {
        // The whole log, line by line: every argument of every call as caller.c made it.
        let expected = first_case();
        if calls != expected {
            return Err(format!("case 0: the log differs from what caller.c made:\nreceived:\n{}\nexpected:\n{}", calls.join("\n"), expected.join("\n")));
        }
    }
    Ok(())
}

/// The capabilities serve.c writes into the screen are Mesa's own: a screen of the recording implementation (which
/// changes no capability) holds, byte for byte, the `caps` u_init_pipe_screen_caps writes for its stages'
/// capabilities.
#[test]
fn caps_are_mesas_defaults() {
    let address = wcontainers_gallium_direct::serve::serve(Recording::new()).expect("the screen is served");
    #[expect(unsafe_code, reason = "caller.c: wc_caller_caps takes the address serve returned (serve.c's live struct pipe_screen), reads its capabilities and destroys it")]
    // SAFETY: the address is the screen serve made just now, not yet destroyed; the call destroys it last.
    let status = unsafe { wc_caller_caps(address) };
    println!("CAPS status {status}");
    assert_eq!(status, 0, "the screen's caps differ from Mesa's u_init_pipe_screen_caps first at byte {}", status - 2);
}

/// Every case, each alone in a process of its own (this test binary run again for one case).
#[test]
fn every_case_alone() {
    #[expect(unsafe_code, reason = "caller.c: wc_caller_cases returns a number, reading nothing")]
    // SAFETY: a plain call of our own C function.
    let cases = unsafe { wc_caller_cases() };
    assert_eq!(usize::try_from(cases).ok(), Some(EXPECTED.len()), "a log expected for every case of caller.c");
    let binary = std::env::current_exe().expect("the test binary");
    for which in 0..cases {
        let output = Command::new(&binary).args(["--exact", "one_case", "--nocapture", "--test-threads=1"]).env(CASE_VARIABLE, which.to_string()).output().expect("the case's process runs");
        let text = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
        println!("SERVED CASE {which}: {}", if output.status.success() && text.contains(&format!("CASE {which} HELD")) { "held".to_string() } else { format!("FAILED {:?} {text}", output.status) });
        assert!(output.status.success() && text.contains(&format!("CASE {which} HELD")), "case {which}: {text}");
    }
}

/// The case WCONTAINERS_GALLIUM_CASE names; where it names none, every case in this process.
#[test]
fn one_case() {
    let cases: Vec<i32> = match std::env::var(CASE_VARIABLE) {
        Ok(which) => vec![which.parse().expect("a case number")],
        Err(_) => (0..i32::try_from(EXPECTED.len()).unwrap_or(0)).collect(),
    };
    for which in cases {
        match holds(which) {
            Ok(()) => println!("CASE {which} HELD"),
            Err(problem) => panic!("{problem}"),
        }
    }
}
