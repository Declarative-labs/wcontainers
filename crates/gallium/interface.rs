// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers (p_defines.h, p_state.h, p_screen.h, p_context.h, u_transfer_helper.h; blend.h, shader_enums.h, u_formats.h and the build's u_format_gen.h for the values) and the plan in src/gallium.rs. Do not edit.
//! Gallium, the driver interface Mesa's OpenGL calls in our driver, as safe traits: `Gallium` (the screen) and
//! `GalliumContext` (a rendering context), limited to the function pointers our driver sets. Every object Mesa
//! passes is a value of one of the implementation's kinds, made only by the methods that make it and given back only
//! by those that take it by value; a shader of one stage is a type of its own, made from mesa-compiler's received
//! program of that stage. Plain data is owned, each object it names a reference of its kind. A kind Mesa counts
//! (Resource, Fence, SamplerView, StreamOutputTarget) is passed as an `Arc` of it: an implementation keeps one past
//! the call by cloning the handle, and the object ends where its last handle is dropped. Declared values are
//! `u32`, checked by the glue.

use std::sync::Arc;

pub mod values;

/// Mesa's `struct pipe_box`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PipeBox {
    /// The `x` of `pipe_box`.
    pub x: i32,
    /// The `width` of `pipe_box`.
    pub width: i32,
    /// The `y` of `pipe_box`.
    pub y: i32,
    /// The `height` of `pipe_box`.
    pub height: i32,
    /// The `z` of `pipe_box`.
    pub z: i16,
    /// The `depth` of `pipe_box`.
    pub depth: i16,
}

/// Mesa's `struct pipe_resource`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResourceTemplate {
    // `__cl_space` is not carried: the padding of the EXCLUSIVE_CACHELINE macro around the count (util/u_memory.h:103): no value.
    // `reference` is not carried: Mesa's count: the glue's, as every count (Role::Counts).
    /// The `width0` of `pipe_resource`.
    pub width0: u32,
    /// The `height0` of `pipe_resource`.
    pub height0: u32,
    /// The `depth0` of `pipe_resource`.
    pub depth0: u16,
    /// The `array_size` of `pipe_resource`.
    pub array_size: u16,
    /// The `format` of `pipe_resource` (16 bits in C: a value past them is refused).
    pub format: values::Format,
    /// The `target` of `pipe_resource` (8 bits in C: a value past them is refused).
    pub target: values::TextureTarget,
    /// The `last_level` of `pipe_resource`.
    pub last_level: u8,
    /// The `nr_samples` of `pipe_resource`.
    pub nr_samples: u8,
    /// The `nr_storage_samples` of `pipe_resource`.
    pub nr_storage_samples: u8,
    /// The `nr_sparse_levels` of `pipe_resource`.
    pub nr_sparse_levels: u8,
    /// The `compression_rate` of `pipe_resource` (4 bits in C: a value past them is refused).
    pub compression_rate: u32,
    /// The `usage` of `pipe_resource` (4 bits in C: a value past them is refused).
    pub usage: values::ResourceUsage,
    /// The `bind` of `pipe_resource`.
    pub bind: u32,
    /// The `flags` of `pipe_resource`.
    pub flags: u32,
    // `clientptr_heap` is not carried: the frontend's heap of client pointers, which the driver never reads (the glue's, zero).
    // `next` is not carried: a chain of planes the driver makes itself; resource_create's template has none (st_texture.c:106).
    // `screen` is not carried: the screen that made it: the glue's own, the implementation is the screen.
}

/// Mesa's `struct pipe_blit_info.dst`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct BlitInfoDst<'a, Resource> {
    /// The `resource` of `pipe_blit_info.dst`.
    pub resource: Option<&'a Arc<Resource>>,
    /// The `level` of `pipe_blit_info.dst`.
    pub level: u32,
    /// The `box` of `pipe_blit_info.dst`.
    pub box_: PipeBox,
    /// The `format` of `pipe_blit_info.dst`.
    pub format: values::Format,
}

/// Mesa's `struct pipe_blit_info.src`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct BlitInfoSrc<'a, Resource> {
    /// The `resource` of `pipe_blit_info.src`.
    pub resource: Option<&'a Arc<Resource>>,
    /// The `level` of `pipe_blit_info.src`.
    pub level: u32,
    /// The `box` of `pipe_blit_info.src`.
    pub box_: PipeBox,
    /// The `format` of `pipe_blit_info.src`.
    pub format: values::Format,
}

/// Mesa's `struct pipe_scissor_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScissorState {
    /// The `minx` of `pipe_scissor_state`.
    pub minx: u32,
    /// The `miny` of `pipe_scissor_state`.
    pub miny: u32,
    /// The `maxx` of `pipe_scissor_state`.
    pub maxx: u32,
    /// The `maxy` of `pipe_scissor_state`.
    pub maxy: u32,
}

/// Mesa's `struct pipe_blit_info`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct BlitInfo<'a, Resource> {
    /// The `dst` of `pipe_blit_info`.
    pub dst: BlitInfoDst<'a, Resource>,
    /// The `src` of `pipe_blit_info`.
    pub src: BlitInfoSrc<'a, Resource>,
    /// The `mask` of `pipe_blit_info`.
    pub mask: u32,
    /// The `filter` of `pipe_blit_info`.
    pub filter: values::TexFilter,
    /// The `dst_sample` of `pipe_blit_info`.
    pub dst_sample: u8,
    /// The `sample0_only` of `pipe_blit_info`.
    pub sample0_only: bool,
    /// The `scissor_enable` of `pipe_blit_info`.
    pub scissor_enable: bool,
    /// The `scissor` of `pipe_blit_info`.
    pub scissor: ScissorState,
    /// The `swizzle_enable` of `pipe_blit_info`.
    pub swizzle_enable: bool,
    /// The `swizzle` of `pipe_blit_info`.
    pub swizzle: [u8; 4],
    /// The `window_rectangle_include` of `pipe_blit_info`.
    pub window_rectangle_include: bool,
    /// The `num_window_rectangles` of `pipe_blit_info`.
    pub num_window_rectangles: u32,
    /// The `window_rectangles` of `pipe_blit_info`.
    pub window_rectangles: [ScissorState; 8],
    /// The `render_condition_enable` of `pipe_blit_info`.
    pub render_condition_enable: bool,
    /// The `alpha_blend` of `pipe_blit_info`.
    pub alpha_blend: bool,
}

/// Mesa's `struct pipe_reference`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reference {
    /// The `count` of `pipe_reference`.
    pub count: i32,
}

/// Mesa's `struct pipe_surface`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct Surface<'a, Resource> {
    /// The `reference` of `pipe_surface`.
    pub reference: Reference,
    /// The `format` of `pipe_surface` (16 bits in C: a value past them is refused).
    pub format: values::Format,
    /// The `nr_samples` of `pipe_surface` (16 bits in C: a value past them is refused).
    pub nr_samples: u32,
    /// The `first_layer` of `pipe_surface` (16 bits in C: a value past them is refused).
    pub first_layer: u32,
    /// The `last_layer` of `pipe_surface` (16 bits in C: a value past them is refused).
    pub last_layer: u32,
    /// The `level` of `pipe_surface`.
    pub level: u32,
    /// The `texture` of `pipe_surface`.
    pub texture: Option<&'a Arc<Resource>>,
}

/// Mesa's `struct pipe_rt_blend_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RtBlendState {
    /// The `blend_enable` of `pipe_rt_blend_state` (1 bits in C: a value past them is refused).
    pub blend_enable: u32,
    /// The `rgb_func` of `pipe_rt_blend_state` (3 bits in C: a value past them is refused).
    pub rgb_func: u32,
    /// The `rgb_src_factor` of `pipe_rt_blend_state` (5 bits in C: a value past them is refused).
    pub rgb_src_factor: values::Blendfactor,
    /// The `rgb_dst_factor` of `pipe_rt_blend_state` (5 bits in C: a value past them is refused).
    pub rgb_dst_factor: values::Blendfactor,
    /// The `alpha_func` of `pipe_rt_blend_state` (3 bits in C: a value past them is refused).
    pub alpha_func: u32,
    /// The `alpha_src_factor` of `pipe_rt_blend_state` (5 bits in C: a value past them is refused).
    pub alpha_src_factor: values::Blendfactor,
    /// The `alpha_dst_factor` of `pipe_rt_blend_state` (5 bits in C: a value past them is refused).
    pub alpha_dst_factor: values::Blendfactor,
    /// The `colormask` of `pipe_rt_blend_state` (4 bits in C: a value past them is refused).
    pub colormask: u32,
}

/// Mesa's `struct pipe_blend_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendState {
    /// The `independent_blend_enable` of `pipe_blend_state` (1 bits in C: a value past them is refused).
    pub independent_blend_enable: u32,
    /// The `logicop_enable` of `pipe_blend_state` (1 bits in C: a value past them is refused).
    pub logicop_enable: u32,
    /// The `logicop_func` of `pipe_blend_state` (4 bits in C: a value past them is refused).
    pub logicop_func: values::Logicop,
    /// The `dither` of `pipe_blend_state` (1 bits in C: a value past them is refused).
    pub dither: u32,
    /// The `alpha_to_coverage` of `pipe_blend_state` (1 bits in C: a value past them is refused).
    pub alpha_to_coverage: u32,
    /// The `alpha_to_coverage_dither` of `pipe_blend_state` (1 bits in C: a value past them is refused).
    pub alpha_to_coverage_dither: u32,
    /// The `alpha_to_one` of `pipe_blend_state` (1 bits in C: a value past them is refused).
    pub alpha_to_one: u32,
    /// The `max_rt` of `pipe_blend_state` (3 bits in C: a value past them is refused).
    pub max_rt: u32,
    /// The `advanced_blend_func` of `pipe_blend_state` (4 bits in C: a value past them is refused).
    pub advanced_blend_func: u32,
    /// The `blend_coherent` of `pipe_blend_state` (1 bits in C: a value past them is refused).
    pub blend_coherent: u32,
    /// The `is_yuv` of `pipe_blend_state` (1 bits in C: a value past them is refused).
    pub is_yuv: u32,
    /// The `rt` of `pipe_blend_state`.
    pub rt: [RtBlendState; 8],
}

/// Mesa's `struct pipe_compute_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputeState {
    // `ir_type` is not carried: always NIR from st/mesa, the received program's.
    // `prog` is not carried: the program: the method's received program.
    /// The `static_shared_mem` of `pipe_compute_state`.
    pub static_shared_mem: u32,
}

/// Mesa's `struct pipe_stencil_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StencilState {
    /// The `enabled` of `pipe_stencil_state` (1 bits in C: a value past them is refused).
    pub enabled: u32,
    /// The `func` of `pipe_stencil_state` (3 bits in C: a value past them is refused).
    pub func: values::CompareFunc,
    /// The `fail_op` of `pipe_stencil_state` (3 bits in C: a value past them is refused).
    pub fail_op: values::StencilOp,
    /// The `zpass_op` of `pipe_stencil_state` (3 bits in C: a value past them is refused).
    pub zpass_op: values::StencilOp,
    /// The `zfail_op` of `pipe_stencil_state` (3 bits in C: a value past them is refused).
    pub zfail_op: values::StencilOp,
    /// The `valuemask` of `pipe_stencil_state` (8 bits in C: a value past them is refused).
    pub valuemask: u32,
    /// The `writemask` of `pipe_stencil_state` (8 bits in C: a value past them is refused).
    pub writemask: u32,
}

/// Mesa's `struct pipe_depth_stencil_alpha_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthStencilAlphaState {
    /// The `stencil` of `pipe_depth_stencil_alpha_state`.
    pub stencil: [StencilState; 2],
    /// The `alpha_enabled` of `pipe_depth_stencil_alpha_state` (1 bits in C: a value past them is refused).
    pub alpha_enabled: u32,
    /// The `alpha_func` of `pipe_depth_stencil_alpha_state` (3 bits in C: a value past them is refused).
    pub alpha_func: values::CompareFunc,
    /// The `depth_enabled` of `pipe_depth_stencil_alpha_state` (1 bits in C: a value past them is refused).
    pub depth_enabled: u32,
    /// The `depth_writemask` of `pipe_depth_stencil_alpha_state` (1 bits in C: a value past them is refused).
    pub depth_writemask: u32,
    /// The `depth_func` of `pipe_depth_stencil_alpha_state` (3 bits in C: a value past them is refused).
    pub depth_func: values::CompareFunc,
    /// The `depth_bounds_test` of `pipe_depth_stencil_alpha_state` (1 bits in C: a value past them is refused).
    pub depth_bounds_test: u32,
    /// The `alpha_ref_value` of `pipe_depth_stencil_alpha_state`.
    pub alpha_ref_value: f32,
    /// The `depth_bounds_min` of `pipe_depth_stencil_alpha_state`.
    pub depth_bounds_min: f64,
    /// The `depth_bounds_max` of `pipe_depth_stencil_alpha_state`.
    pub depth_bounds_max: f64,
}

/// Mesa's `struct pipe_stream_output`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamOutput {
    /// The `register_index` of `pipe_stream_output` (6 bits in C: a value past them is refused).
    pub register_index: u32,
    /// The `start_component` of `pipe_stream_output` (2 bits in C: a value past them is refused).
    pub start_component: u32,
    /// The `num_components` of `pipe_stream_output` (3 bits in C: a value past them is refused).
    pub num_components: u32,
    /// The `output_buffer` of `pipe_stream_output` (3 bits in C: a value past them is refused).
    pub output_buffer: u32,
    /// The `dst_offset` of `pipe_stream_output` (16 bits in C: a value past them is refused).
    pub dst_offset: u32,
    /// The `stream` of `pipe_stream_output` (2 bits in C: a value past them is refused).
    pub stream: u32,
}

/// Mesa's `struct pipe_stream_output_info`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamOutputInfo {
    /// The `num_outputs` of `pipe_stream_output_info`.
    pub num_outputs: u32,
    /// The `stride` of `pipe_stream_output_info`.
    pub stride: [u16; 4],
    /// The `output` of `pipe_stream_output_info`.
    pub output: [StreamOutput; 128],
}

/// Mesa's `struct pipe_shader_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderState {
    // `type` is not carried: the program's form: always NIR from st/mesa (PIPE_SHADER_IR_NIR), the received program's.
    // `tokens` is not carried: TGSI tokens, which st/mesa does not pass with NIR.
    // `ir` is not carried: the program: the method's received program of its stage.
    /// The `stream_output` of `pipe_shader_state`.
    pub stream_output: StreamOutputInfo,
    /// The `report_compile_error` of `pipe_shader_state`.
    pub report_compile_error: bool,
    // `error_message` is not carried: a text the driver allocates for Mesa: the glue writes none.
}

/// Mesa's `struct pipe_rasterizer_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterizerState {
    /// The `flatshade` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub flatshade: u32,
    /// The `light_twoside` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub light_twoside: u32,
    /// The `clamp_vertex_color` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub clamp_vertex_color: u32,
    /// The `clamp_fragment_color` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub clamp_fragment_color: u32,
    /// The `front_ccw` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub front_ccw: u32,
    /// The `cull_face` of `pipe_rasterizer_state` (2 bits in C: a value past them is refused).
    pub cull_face: u32,
    /// The `fill_front` of `pipe_rasterizer_state` (2 bits in C: a value past them is refused).
    pub fill_front: u32,
    /// The `fill_back` of `pipe_rasterizer_state` (2 bits in C: a value past them is refused).
    pub fill_back: u32,
    /// The `offset_point` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub offset_point: u32,
    /// The `offset_line` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub offset_line: u32,
    /// The `offset_tri` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub offset_tri: u32,
    /// The `scissor` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub scissor: u32,
    /// The `poly_smooth` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub poly_smooth: u32,
    /// The `poly_stipple_enable` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub poly_stipple_enable: u32,
    /// The `point_smooth` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub point_smooth: u32,
    /// The `sprite_coord_mode` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub sprite_coord_mode: values::SpriteCoordMode,
    /// The `point_quad_rasterization` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub point_quad_rasterization: u32,
    /// The `point_line_tri_clip` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub point_line_tri_clip: u32,
    /// The `point_size_per_vertex` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub point_size_per_vertex: u32,
    /// The `multisample` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub multisample: u32,
    /// The `no_ms_sample_mask_out` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub no_ms_sample_mask_out: u32,
    /// The `force_persample_interp` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub force_persample_interp: u32,
    /// The `line_smooth` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub line_smooth: u32,
    /// The `line_stipple_enable` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub line_stipple_enable: u32,
    /// The `line_last_pixel` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub line_last_pixel: u32,
    /// The `line_rectangular` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub line_rectangular: u32,
    /// The `conservative_raster_mode` of `pipe_rasterizer_state` (2 bits in C: a value past them is refused).
    pub conservative_raster_mode: values::ConservativeRasterMode,
    /// The `flatshade_first` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub flatshade_first: u32,
    /// The `half_pixel_center` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub half_pixel_center: u32,
    /// The `bottom_edge_rule` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub bottom_edge_rule: u32,
    /// The `subpixel_precision_x` of `pipe_rasterizer_state` (4 bits in C: a value past them is refused).
    pub subpixel_precision_x: u32,
    /// The `subpixel_precision_y` of `pipe_rasterizer_state` (4 bits in C: a value past them is refused).
    pub subpixel_precision_y: u32,
    /// The `rasterizer_discard` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub rasterizer_discard: u32,
    /// The `rasterization_stream` of `pipe_rasterizer_state` (2 bits in C: a value past them is refused).
    pub rasterization_stream: u32,
    /// The `tile_raster_order_fixed` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub tile_raster_order_fixed: u32,
    /// The `tile_raster_order_increasing_x` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub tile_raster_order_increasing_x: u32,
    /// The `tile_raster_order_increasing_y` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub tile_raster_order_increasing_y: u32,
    /// The `depth_clip_near` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub depth_clip_near: u32,
    /// The `depth_clip_far` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub depth_clip_far: u32,
    /// The `depth_clamp` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub depth_clamp: u32,
    /// The `clip_halfz` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub clip_halfz: u32,
    /// The `offset_units_unscaled` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub offset_units_unscaled: u32,
    /// The `unclamped_fragment_depth_values` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub unclamped_fragment_depth_values: u32,
    /// The `clip_plane_enable` of `pipe_rasterizer_state` (PIPE_MAX_CLIP_PLANES bits in C: a value past them is refused).
    pub clip_plane_enable: u32,
    /// The `representative_fragment_test` of `pipe_rasterizer_state` (1 bits in C: a value past them is refused).
    pub representative_fragment_test: u32,
    /// The `line_stipple_factor` of `pipe_rasterizer_state` (8 bits in C: a value past them is refused).
    pub line_stipple_factor: u32,
    /// The `line_stipple_pattern` of `pipe_rasterizer_state` (16 bits in C: a value past them is refused).
    pub line_stipple_pattern: u32,
    /// The `sprite_coord_enable` of `pipe_rasterizer_state`.
    pub sprite_coord_enable: u16,
    /// The `line_width` of `pipe_rasterizer_state`.
    pub line_width: f32,
    /// The `point_size` of `pipe_rasterizer_state`.
    pub point_size: f32,
    /// The `offset_units` of `pipe_rasterizer_state`.
    pub offset_units: f32,
    /// The `offset_scale` of `pipe_rasterizer_state`.
    pub offset_scale: f32,
    /// The `offset_clamp` of `pipe_rasterizer_state`.
    pub offset_clamp: f32,
    /// The `conservative_raster_dilate` of `pipe_rasterizer_state`.
    pub conservative_raster_dilate: f32,
}

/// Mesa's `struct pipe_sampler_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerState {
    /// The `wrap_s` of `pipe_sampler_state` (3 bits in C: a value past them is refused).
    pub wrap_s: values::TexWrap,
    /// The `wrap_t` of `pipe_sampler_state` (3 bits in C: a value past them is refused).
    pub wrap_t: values::TexWrap,
    /// The `wrap_r` of `pipe_sampler_state` (3 bits in C: a value past them is refused).
    pub wrap_r: values::TexWrap,
    /// The `min_img_filter` of `pipe_sampler_state` (1 bits in C: a value past them is refused).
    pub min_img_filter: values::TexFilter,
    /// The `min_mip_filter` of `pipe_sampler_state` (2 bits in C: a value past them is refused).
    pub min_mip_filter: values::TexMipfilter,
    /// The `mag_img_filter` of `pipe_sampler_state` (1 bits in C: a value past them is refused).
    pub mag_img_filter: values::TexFilter,
    /// The `compare_mode` of `pipe_sampler_state` (1 bits in C: a value past them is refused).
    pub compare_mode: values::TexCompare,
    /// The `compare_func` of `pipe_sampler_state` (3 bits in C: a value past them is refused).
    pub compare_func: values::CompareFunc,
    /// The `unnormalized_coords` of `pipe_sampler_state` (1 bits in C: a value past them is refused).
    pub unnormalized_coords: u32,
    /// The `max_anisotropy` of `pipe_sampler_state` (5 bits in C: a value past them is refused).
    pub max_anisotropy: u32,
    /// The `seamless_cube_map` of `pipe_sampler_state` (1 bits in C: a value past them is refused).
    pub seamless_cube_map: u32,
    /// The `border_color_is_integer` of `pipe_sampler_state` (1 bits in C: a value past them is refused).
    pub border_color_is_integer: u32,
    /// The `reduction_mode` of `pipe_sampler_state` (2 bits in C: a value past them is refused).
    pub reduction_mode: values::TexReductionMode,
    /// The `pad` of `pipe_sampler_state` (5 bits in C: a value past them is refused).
    pub pad: u32,
    /// The `lod_bias` of `pipe_sampler_state`.
    pub lod_bias: f32,
    /// The `min_lod` of `pipe_sampler_state`.
    pub min_lod: f32,
    /// The `max_lod` of `pipe_sampler_state`.
    pub max_lod: f32,
    /// The `border_color` of `pipe_sampler_state`.
    pub border_color: [u32; 4],
    /// The `border_color_format` of `pipe_sampler_state`.
    pub border_color_format: values::Format,
}

/// Mesa's `struct pipe_sampler_view.u.tex`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerViewUTex {
    /// The `first_layer` of `pipe_sampler_view.u.tex` (16 bits in C: a value past them is refused).
    pub first_layer: u32,
    /// The `last_layer` of `pipe_sampler_view.u.tex` (16 bits in C: a value past them is refused).
    pub last_layer: u32,
    /// The `first_level` of `pipe_sampler_view.u.tex` (8 bits in C: a value past them is refused).
    pub first_level: u32,
    /// The `last_level` of `pipe_sampler_view.u.tex` (8 bits in C: a value past them is refused).
    pub last_level: u32,
    /// The `min_lod_clamp` of `pipe_sampler_view.u.tex`.
    pub min_lod_clamp: f32,
}

/// Mesa's `struct pipe_sampler_view.u.buf`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerViewUBuf {
    /// The `offset` of `pipe_sampler_view.u.buf`.
    pub offset: u32,
    /// The `size` of `pipe_sampler_view.u.buf`.
    pub size: u32,
}

/// Mesa's `struct pipe_tex2d_from_buf`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tex2dFromBuf {
    /// The `offset` of `pipe_tex2d_from_buf`.
    pub offset: u32,
    /// The `row_stride` of `pipe_tex2d_from_buf`.
    pub row_stride: u16,
    /// The `width` of `pipe_tex2d_from_buf`.
    pub width: u16,
    /// The `height` of `pipe_tex2d_from_buf`.
    pub height: u16,
}

/// Mesa's `union pipe_sampler_view.u`, owned. A union in C: the glue fills the one member the structure around it selects, the others zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerViewU {
    /// The `tex` of `pipe_sampler_view.u`.
    pub tex: SamplerViewUTex,
    /// The `buf` of `pipe_sampler_view.u`.
    pub buf: SamplerViewUBuf,
    /// The `tex2d_from_buf` of `pipe_sampler_view.u`.
    pub tex2d_from_buf: Tex2dFromBuf,
}

/// Mesa's `struct pipe_sampler_view`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerViewTemplate {
    // `reference` is not carried: Mesa's count: the glue's.
    /// The `format` of `pipe_sampler_view` (12 bits in C: a value past them is refused).
    pub format: values::Format,
    /// The `astc_decode_format` of `pipe_sampler_view` (2 bits in C: a value past them is refused).
    pub astc_decode_format: u32,
    /// The `is_tex2d_from_buf` of `pipe_sampler_view` (1 bits in C: a value past them is refused).
    pub is_tex2d_from_buf: bool,
    /// The `target` of `pipe_sampler_view` (5 bits in C: a value past them is refused).
    pub target: values::TextureTarget,
    /// The `swizzle_r` of `pipe_sampler_view` (3 bits in C: a value past them is refused).
    pub swizzle_r: values::Swizzle,
    /// The `swizzle_g` of `pipe_sampler_view` (3 bits in C: a value past them is refused).
    pub swizzle_g: values::Swizzle,
    /// The `swizzle_b` of `pipe_sampler_view` (3 bits in C: a value past them is refused).
    pub swizzle_b: values::Swizzle,
    /// The `swizzle_a` of `pipe_sampler_view` (3 bits in C: a value past them is refused).
    pub swizzle_a: values::Swizzle,
    // `texture` is not carried: the resource viewed: create_sampler_view's own parameter.
    // `context` is not carried: the context the view is made on: the method's receiver.
    /// The `u` of `pipe_sampler_view`.
    pub u: SamplerViewU,
}

/// Mesa's `struct pipe_vertex_element`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexElement {
    /// The `src_offset` of `pipe_vertex_element`.
    pub src_offset: u16,
    /// The `vertex_buffer_index` of `pipe_vertex_element` (7 bits in C: a value past them is refused).
    pub vertex_buffer_index: u8,
    /// The `dual_slot` of `pipe_vertex_element` (1 bits in C: a value past them is refused).
    pub dual_slot: bool,
    /// The `src_format` of `pipe_vertex_element`.
    pub src_format: u8,
    /// The `src_stride` of `pipe_vertex_element`.
    pub src_stride: u32,
    /// The `instance_divisor` of `pipe_vertex_element`.
    pub instance_divisor: u32,
}

/// Mesa's `union pipe_draw_info.index`, owned, each object it names a reference of its kind. A union in C: the glue fills the one member the structure around it selects, the others zero.
#[derive(Debug)]
pub struct DrawInfoIndex<'a, Resource> {
    /// The `resource` of `pipe_draw_info.index`.
    pub resource: Option<&'a Arc<Resource>>,
    /// The `user` of `pipe_draw_info.index`: the indices in client memory, when `has_user_indices`: from index 0 to the end of the furthest draw (start plus count, times index_size). No capability turns them off: st/mesa sets has_user_indices whenever no element buffer is bound (main/draw.c:1662, 2043; MEASURED on llvmpipe).
    pub user: Option<&'a [u8]>,
}

/// Mesa's `struct pipe_draw_info`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct DrawInfo<'a, Resource> {
    /// The `mode` of `pipe_draw_info`.
    pub mode: values::MesaPrim,
    /// The `index_size` of `pipe_draw_info`.
    pub index_size: u16,
    /// The `primitive_restart` of `pipe_draw_info` (1 bits in C: a value past them is refused).
    pub primitive_restart: bool,
    /// The `has_user_indices` of `pipe_draw_info` (1 bits in C: a value past them is refused).
    pub has_user_indices: bool,
    /// The `index_bounds_valid` of `pipe_draw_info` (1 bits in C: a value past them is refused).
    pub index_bounds_valid: bool,
    /// The `increment_draw_id` of `pipe_draw_info` (1 bits in C: a value past them is refused).
    pub increment_draw_id: bool,
    /// The `index_bias_varies` of `pipe_draw_info` (1 bits in C: a value past them is refused).
    pub index_bias_varies: bool,
    /// The `was_line_loop` of `pipe_draw_info` (1 bits in C: a value past them is refused).
    pub was_line_loop: bool,
    /// The `_pad` of `pipe_draw_info` (2 bits in C: a value past them is refused).
    pub _pad: u8,
    /// The `start_instance` of `pipe_draw_info`.
    pub start_instance: u32,
    /// The `instance_count` of `pipe_draw_info`.
    pub instance_count: u32,
    /// The `restart_index` of `pipe_draw_info`.
    pub restart_index: u32,
    /// The `index` of `pipe_draw_info`.
    pub index: DrawInfoIndex<'a, Resource>,
    /// The `min_index` of `pipe_draw_info`.
    pub min_index: u32,
    /// The `max_index` of `pipe_draw_info`.
    pub max_index: u32,
}

/// Mesa's `struct pipe_draw_indirect_info`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct DrawIndirectInfo<'a, Resource, StreamOutputTarget> {
    /// The `offset` of `pipe_draw_indirect_info`.
    pub offset: u32,
    /// The `stride` of `pipe_draw_indirect_info`.
    pub stride: u32,
    /// The `draw_count` of `pipe_draw_indirect_info`.
    pub draw_count: u32,
    /// The `indirect_draw_count_offset` of `pipe_draw_indirect_info`.
    pub indirect_draw_count_offset: u32,
    /// The `buffer` of `pipe_draw_indirect_info`.
    pub buffer: Option<&'a Arc<Resource>>,
    /// The `indirect_draw_count` of `pipe_draw_indirect_info`.
    pub indirect_draw_count: Option<&'a Arc<Resource>>,
    /// The `count_from_stream_output` of `pipe_draw_indirect_info`.
    pub count_from_stream_output: Option<&'a Arc<StreamOutputTarget>>,
}

/// Mesa's `struct pipe_draw_start_count_bias`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawStartCountBias {
    /// The `start` of `pipe_draw_start_count_bias`.
    pub start: u32,
    /// The `count` of `pipe_draw_start_count_bias`.
    pub count: u32,
    /// The `index_bias` of `pipe_draw_start_count_bias`.
    pub index_bias: i32,
}

/// Mesa's `struct pipe_grid_info`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct GridInfo<'a, Resource> {
    /// The `variable_shared_mem` of `pipe_grid_info`.
    pub variable_shared_mem: u32,
    /// The `work_dim` of `pipe_grid_info`.
    pub work_dim: u32,
    /// The `block` of `pipe_grid_info`.
    pub block: [u32; 3],
    /// The `last_block` of `pipe_grid_info`.
    pub last_block: [u32; 3],
    /// The `grid` of `pipe_grid_info`.
    pub grid: [u32; 3],
    /// The `grid_base` of `pipe_grid_info`.
    pub grid_base: [u32; 3],
    /// The `indirect` of `pipe_grid_info`.
    pub indirect: Option<&'a Arc<Resource>>,
    /// The `indirect_offset` of `pipe_grid_info`.
    pub indirect_offset: u32,
    /// The `indirect_stride` of `pipe_grid_info`.
    pub indirect_stride: u32,
    /// The `draw_count` of `pipe_grid_info`.
    pub draw_count: u32,
    /// The `indirect_draw_count_offset` of `pipe_grid_info`.
    pub indirect_draw_count_offset: u32,
    /// The `indirect_draw_count` of `pipe_grid_info`.
    pub indirect_draw_count: Option<&'a Arc<Resource>>,
    // `num_globals` is not carried: the count of `globals`, left out with it.
    // `globals` is not carried: global buffers of OpenCL's frontend (none from st/mesa).
}

/// Mesa's `struct pipe_blend_color`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendColor {
    /// The `color` of `pipe_blend_color`.
    pub color: [f32; 4],
}

/// Mesa's `struct pipe_clip_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipState {
    /// The `ucp` of `pipe_clip_state`.
    pub ucp: [[f32; 4]; 8],
}

/// Mesa's `struct pipe_constant_buffer`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct ConstantBuffer<'a, Resource> {
    /// The `buffer` of `pipe_constant_buffer`.
    pub buffer: Option<&'a Arc<Resource>>,
    /// The `buffer_offset` of `pipe_constant_buffer`.
    pub buffer_offset: u32,
    /// The `buffer_size` of `pipe_constant_buffer`.
    pub buffer_size: u32,
    /// The `user_buffer` of `pipe_constant_buffer`: the constants in client memory, `buffer_size` bytes from `buffer_offset`. st/mesa uploads its own when the driver reports prefer_real_buffer_in_constbuf0 (u_inlines.h:749, st_atom_constbuf.c:124), but Mesa's blitter, a helper the driver calls, passes its clear color so (u_blitter.c:1607, 2501, 2939).
    pub user_buffer: Option<&'a [u8]>,
}

/// Mesa's `struct pipe_framebuffer_state`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct FramebufferState<'a, Resource> {
    /// The `width` of `pipe_framebuffer_state`.
    pub width: u32,
    /// The `height` of `pipe_framebuffer_state`.
    pub height: u32,
    /// The `layers` of `pipe_framebuffer_state`.
    pub layers: u16,
    /// The `samples` of `pipe_framebuffer_state`.
    pub samples: u8,
    /// The `nr_cbufs` of `pipe_framebuffer_state`.
    pub nr_cbufs: u8,
    /// The `pls_enabled` of `pipe_framebuffer_state` (1 bits in C: a value past them is refused).
    pub pls_enabled: bool,
    /// The `downscale_cbufs` of `pipe_framebuffer_state` (1 bits in C: a value past them is refused).
    pub downscale_cbufs: bool,
    /// The `_pad` of `pipe_framebuffer_state` (6 bits in C: a value past them is refused).
    pub _pad: u8,
    /// The `viewmask` of `pipe_framebuffer_state`.
    pub viewmask: u8,
    /// The `cbufs` of `pipe_framebuffer_state`.
    pub cbufs: [Surface<'a, Resource>; 8],
    /// The `zsbuf` of `pipe_framebuffer_state`.
    pub zsbuf: Surface<'a, Resource>,
    /// The `resolve` of `pipe_framebuffer_state`.
    pub resolve: Option<&'a Arc<Resource>>,
}

/// Mesa's `struct pipe_poly_stipple`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PolyStipple {
    /// The `stipple` of `pipe_poly_stipple`.
    pub stipple: [u32; 32],
}

/// Mesa's `struct pipe_shader_buffer`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct ShaderBuffer<'a, Resource> {
    /// The `buffer` of `pipe_shader_buffer`.
    pub buffer: Option<&'a Arc<Resource>>,
    /// The `buffer_offset` of `pipe_shader_buffer`.
    pub buffer_offset: u32,
    /// The `buffer_size` of `pipe_shader_buffer`.
    pub buffer_size: u32,
}

/// Mesa's `struct pipe_image_view.u.tex`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageViewUTex {
    /// The `first_layer` of `pipe_image_view.u.tex` (16 bits in C: a value past them is refused).
    pub first_layer: u32,
    /// The `last_layer` of `pipe_image_view.u.tex` (16 bits in C: a value past them is refused).
    pub last_layer: u32,
    /// The `level` of `pipe_image_view.u.tex` (8 bits in C: a value past them is refused).
    pub level: u32,
    /// The `single_layer_view` of `pipe_image_view.u.tex`.
    pub single_layer_view: bool,
    /// The `is_2d_view_of_3d` of `pipe_image_view.u.tex`.
    pub is_2d_view_of_3d: bool,
}

/// Mesa's `struct pipe_image_view.u.buf`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageViewUBuf {
    /// The `offset` of `pipe_image_view.u.buf`.
    pub offset: u32,
    /// The `size` of `pipe_image_view.u.buf`.
    pub size: u32,
}

/// Mesa's `union pipe_image_view.u`, owned. A union in C: the glue fills the one member the structure around it selects, the others zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageViewU {
    /// The `tex` of `pipe_image_view.u`.
    pub tex: ImageViewUTex,
    /// The `buf` of `pipe_image_view.u`.
    pub buf: ImageViewUBuf,
    /// The `tex2d_from_buf` of `pipe_image_view.u`.
    pub tex2d_from_buf: Tex2dFromBuf,
}

/// Mesa's `struct pipe_image_view`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct ImageView<'a, Resource> {
    /// The `resource` of `pipe_image_view`.
    pub resource: Option<&'a Arc<Resource>>,
    /// The `format` of `pipe_image_view`.
    pub format: values::Format,
    /// The `access` of `pipe_image_view`.
    pub access: u16,
    /// The `shader_access` of `pipe_image_view`.
    pub shader_access: u16,
    /// The `u` of `pipe_image_view`.
    pub u: ImageViewU,
}

/// Mesa's `struct pipe_stencil_ref`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StencilRef {
    /// The `ref_value` of `pipe_stencil_ref`.
    pub ref_value: [u8; 2],
}

/// Mesa's `union pipe_vertex_buffer.buffer`, owned, each object it names a reference of its kind. A union in C: the glue fills the one member the structure around it selects, the others zero.
#[derive(Debug)]
pub struct VertexBufferBuffer<'a, Resource> {
    /// The `resource` of `pipe_vertex_buffer.buffer`.
    pub resource: Option<&'a Arc<Resource>>,
    // `user` is not carried: client memory as a vertex buffer: refused, the driver reports no user vertex buffers.
}

/// Mesa's `struct pipe_vertex_buffer`, owned, each object it names a reference of its kind.
#[derive(Debug)]
pub struct VertexBuffer<'a, Resource> {
    /// The `is_user_buffer` of `pipe_vertex_buffer`.
    pub is_user_buffer: bool,
    /// The `buffer_offset` of `pipe_vertex_buffer`.
    pub buffer_offset: u32,
    /// The `buffer` of `pipe_vertex_buffer`.
    pub buffer: VertexBufferBuffer<'a, Resource>,
}

/// Mesa's `struct pipe_viewport_state`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportState {
    /// The `scale` of `pipe_viewport_state`.
    pub scale: [f32; 3],
    /// The `translate` of `pipe_viewport_state`.
    pub translate: [f32; 3],
    /// The `swizzle_x` of `pipe_viewport_state` (8 bits in C: a value past them is refused).
    pub swizzle_x: values::ViewportSwizzle,
    /// The `swizzle_y` of `pipe_viewport_state` (8 bits in C: a value past them is refused).
    pub swizzle_y: values::ViewportSwizzle,
    /// The `swizzle_z` of `pipe_viewport_state` (8 bits in C: a value past them is refused).
    pub swizzle_z: values::ViewportSwizzle,
    /// The `swizzle_w` of `pipe_viewport_state` (8 bits in C: a value past them is refused).
    pub swizzle_w: values::ViewportSwizzle,
}

/// Mesa's `struct pipe_mesh_caps`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshCaps {
    /// The `max_task_work_group_total_count` of `pipe_mesh_caps`.
    pub max_task_work_group_total_count: u32,
    /// The `max_mesh_work_group_total_count` of `pipe_mesh_caps`.
    pub max_mesh_work_group_total_count: u32,
    /// The `max_mesh_work_group_invocations` of `pipe_mesh_caps`.
    pub max_mesh_work_group_invocations: u32,
    /// The `max_task_work_group_invocations` of `pipe_mesh_caps`.
    pub max_task_work_group_invocations: u32,
    /// The `max_task_payload_size` of `pipe_mesh_caps`.
    pub max_task_payload_size: u32,
    /// The `max_task_shared_memory_size` of `pipe_mesh_caps`.
    pub max_task_shared_memory_size: u32,
    /// The `max_mesh_shared_memory_size` of `pipe_mesh_caps`.
    pub max_mesh_shared_memory_size: u32,
    /// The `max_task_payload_and_shared_memory_size` of `pipe_mesh_caps`.
    pub max_task_payload_and_shared_memory_size: u32,
    /// The `max_mesh_payload_and_shared_memory_size` of `pipe_mesh_caps`.
    pub max_mesh_payload_and_shared_memory_size: u32,
    /// The `max_mesh_output_memory_size` of `pipe_mesh_caps`.
    pub max_mesh_output_memory_size: u32,
    /// The `max_mesh_payload_and_output_memory_size` of `pipe_mesh_caps`.
    pub max_mesh_payload_and_output_memory_size: u32,
    /// The `max_mesh_output_vertices` of `pipe_mesh_caps`.
    pub max_mesh_output_vertices: u32,
    /// The `max_mesh_output_primitives` of `pipe_mesh_caps`.
    pub max_mesh_output_primitives: u32,
    /// The `max_mesh_output_components` of `pipe_mesh_caps`.
    pub max_mesh_output_components: u32,
    /// The `max_mesh_output_layers` of `pipe_mesh_caps`.
    pub max_mesh_output_layers: u32,
    /// The `max_mesh_multiview_view_count` of `pipe_mesh_caps`.
    pub max_mesh_multiview_view_count: u32,
    /// The `mesh_output_per_vertex_granularity` of `pipe_mesh_caps`.
    pub mesh_output_per_vertex_granularity: u32,
    /// The `mesh_output_per_primitive_granularity` of `pipe_mesh_caps`.
    pub mesh_output_per_primitive_granularity: u32,
    /// The `max_preferred_task_work_group_invocations` of `pipe_mesh_caps`.
    pub max_preferred_task_work_group_invocations: u32,
    /// The `max_preferred_mesh_work_group_invocations` of `pipe_mesh_caps`.
    pub max_preferred_mesh_work_group_invocations: u32,
    /// The `mesh_prefers_local_invocation_vertex_output` of `pipe_mesh_caps`.
    pub mesh_prefers_local_invocation_vertex_output: bool,
    /// The `mesh_prefers_local_invocation_primitive_output` of `pipe_mesh_caps`.
    pub mesh_prefers_local_invocation_primitive_output: bool,
    /// The `mesh_prefers_compact_vertex_output` of `pipe_mesh_caps`.
    pub mesh_prefers_compact_vertex_output: bool,
    /// The `mesh_prefers_compact_primitive_output` of `pipe_mesh_caps`.
    pub mesh_prefers_compact_primitive_output: bool,
    /// The `max_task_work_group_count` of `pipe_mesh_caps`.
    pub max_task_work_group_count: [u32; 3],
    /// The `max_mesh_work_group_count` of `pipe_mesh_caps`.
    pub max_mesh_work_group_count: [u32; 3],
    /// The `max_task_work_group_size` of `pipe_mesh_caps`.
    pub max_task_work_group_size: [u32; 3],
    /// The `max_mesh_work_group_size` of `pipe_mesh_caps`.
    pub max_mesh_work_group_size: [u32; 3],
    /// The `pipeline_statistic_queries` of `pipe_mesh_caps`.
    pub pipeline_statistic_queries: bool,
}

/// Mesa's `struct pipe_caps`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caps {
    /// The `graphics` of `pipe_caps`.
    pub graphics: bool,
    /// The `npot_textures` of `pipe_caps`.
    pub npot_textures: bool,
    /// The `anisotropic_filter` of `pipe_caps`.
    pub anisotropic_filter: bool,
    /// The `occlusion_query` of `pipe_caps`.
    pub occlusion_query: bool,
    /// The `query_time_elapsed` of `pipe_caps`.
    pub query_time_elapsed: bool,
    /// The `texture_shadow_map` of `pipe_caps`.
    pub texture_shadow_map: bool,
    /// The `texture_swizzle` of `pipe_caps`.
    pub texture_swizzle: bool,
    /// The `texture_mirror_clamp` of `pipe_caps`.
    pub texture_mirror_clamp: bool,
    /// The `blend_equation_separate` of `pipe_caps`.
    pub blend_equation_separate: bool,
    /// The `primitive_restart` of `pipe_caps`.
    pub primitive_restart: bool,
    /// The `primitive_restart_fixed_index` of `pipe_caps`.
    pub primitive_restart_fixed_index: bool,
    /// The `indep_blend_enable` of `pipe_caps`.
    pub indep_blend_enable: bool,
    /// The `indep_blend_func` of `pipe_caps`.
    pub indep_blend_func: bool,
    /// The `fs_coord_origin_upper_left` of `pipe_caps`.
    pub fs_coord_origin_upper_left: bool,
    /// The `fs_coord_origin_lower_left` of `pipe_caps`.
    pub fs_coord_origin_lower_left: bool,
    /// The `fs_coord_pixel_center_half_integer` of `pipe_caps`.
    pub fs_coord_pixel_center_half_integer: bool,
    /// The `fs_coord_pixel_center_integer` of `pipe_caps`.
    pub fs_coord_pixel_center_integer: bool,
    /// The `depth_clip_disable` of `pipe_caps`.
    pub depth_clip_disable: bool,
    /// The `depth_clip_disable_separate` of `pipe_caps`.
    pub depth_clip_disable_separate: bool,
    /// The `depth_clamp_enable` of `pipe_caps`.
    pub depth_clamp_enable: bool,
    /// The `shader_stencil_export` of `pipe_caps`.
    pub shader_stencil_export: bool,
    /// The `vs_instanceid` of `pipe_caps`.
    pub vs_instanceid: bool,
    /// The `vertex_element_instance_divisor` of `pipe_caps`.
    pub vertex_element_instance_divisor: bool,
    /// The `fragment_color_clamped` of `pipe_caps`.
    pub fragment_color_clamped: bool,
    /// The `mixed_colorbuffer_formats` of `pipe_caps`.
    pub mixed_colorbuffer_formats: bool,
    /// The `seamless_cube_map` of `pipe_caps`.
    pub seamless_cube_map: bool,
    /// The `seamless_cube_map_per_texture` of `pipe_caps`.
    pub seamless_cube_map_per_texture: bool,
    /// The `conditional_render` of `pipe_caps`.
    pub conditional_render: bool,
    /// The `texture_barrier` of `pipe_caps`.
    pub texture_barrier: bool,
    /// The `stream_output_pause_resume` of `pipe_caps`.
    pub stream_output_pause_resume: bool,
    /// The `tgsi_can_compact_constants` of `pipe_caps`.
    pub tgsi_can_compact_constants: bool,
    /// The `vertex_color_unclamped` of `pipe_caps`.
    pub vertex_color_unclamped: bool,
    /// The `vertex_color_clamped` of `pipe_caps`.
    pub vertex_color_clamped: bool,
    /// The `quads_follow_provoking_vertex_convention` of `pipe_caps`.
    pub quads_follow_provoking_vertex_convention: bool,
    /// The `user_vertex_buffers` of `pipe_caps`.
    pub user_vertex_buffers: bool,
    /// The `compute` of `pipe_caps`.
    pub compute: bool,
    /// The `start_instance` of `pipe_caps`.
    pub start_instance: bool,
    /// The `query_timestamp` of `pipe_caps`.
    pub query_timestamp: bool,
    /// The `texture_multisample` of `pipe_caps`.
    pub texture_multisample: bool,
    /// The `cube_map_array` of `pipe_caps`.
    pub cube_map_array: bool,
    /// The `texture_buffer_objects` of `pipe_caps`.
    pub texture_buffer_objects: bool,
    /// The `buffer_sampler_view_rgba_only` of `pipe_caps`.
    pub buffer_sampler_view_rgba_only: bool,
    /// The `tgsi_texcoord` of `pipe_caps`.
    pub tgsi_texcoord: bool,
    /// The `query_pipeline_statistics` of `pipe_caps`.
    pub query_pipeline_statistics: bool,
    /// The `mixed_framebuffer_sizes` of `pipe_caps`.
    pub mixed_framebuffer_sizes: bool,
    /// The `vs_layer_viewport` of `pipe_caps`.
    pub vs_layer_viewport: bool,
    /// The `texture_gather_sm5` of `pipe_caps`.
    pub texture_gather_sm5: bool,
    /// The `buffer_map_persistent_coherent` of `pipe_caps`.
    pub buffer_map_persistent_coherent: bool,
    /// The `fake_sw_msaa` of `pipe_caps`.
    pub fake_sw_msaa: bool,
    /// The `texture_query_lod` of `pipe_caps`.
    pub texture_query_lod: bool,
    /// The `sample_shading` of `pipe_caps`.
    pub sample_shading: bool,
    /// The `texture_gather_offsets` of `pipe_caps`.
    pub texture_gather_offsets: bool,
    /// The `vs_window_space_position` of `pipe_caps`.
    pub vs_window_space_position: bool,
    /// The `draw_indirect` of `pipe_caps`.
    pub draw_indirect: bool,
    /// The `fs_fine_derivative` of `pipe_caps`.
    pub fs_fine_derivative: bool,
    /// The `uma` of `pipe_caps`.
    pub uma: bool,
    /// The `conditional_render_inverted` of `pipe_caps`.
    pub conditional_render_inverted: bool,
    /// The `sampler_view_target` of `pipe_caps`.
    pub sampler_view_target: bool,
    /// The `clip_halfz` of `pipe_caps`.
    pub clip_halfz: bool,
    /// The `polygon_offset_clamp` of `pipe_caps`.
    pub polygon_offset_clamp: bool,
    /// The `multisample_z_resolve` of `pipe_caps`.
    pub multisample_z_resolve: bool,
    /// The `resource_from_user_memory` of `pipe_caps`.
    pub resource_from_user_memory: bool,
    /// The `resource_from_user_memory_compute_only` of `pipe_caps`.
    pub resource_from_user_memory_compute_only: bool,
    /// The `device_reset_status_query` of `pipe_caps`.
    pub device_reset_status_query: bool,
    /// The `texture_float_linear` of `pipe_caps`.
    pub texture_float_linear: bool,
    /// The `texture_half_float_linear` of `pipe_caps`.
    pub texture_half_float_linear: bool,
    /// The `depth_bounds_test` of `pipe_caps`.
    pub depth_bounds_test: bool,
    /// The `native_fp32_depth` of `pipe_caps`.
    pub native_fp32_depth: bool,
    /// The `texture_query_samples` of `pipe_caps`.
    pub texture_query_samples: bool,
    /// The `force_persample_interp` of `pipe_caps`.
    pub force_persample_interp: bool,
    /// The `shareable_shaders` of `pipe_caps`.
    pub shareable_shaders: bool,
    /// The `copy_between_compressed_and_plain_formats` of `pipe_caps`.
    pub copy_between_compressed_and_plain_formats: bool,
    /// The `clear_scissored` of `pipe_caps`.
    pub clear_scissored: bool,
    /// The `clear_masked` of `pipe_caps`.
    pub clear_masked: bool,
    /// The `draw_parameters` of `pipe_caps`.
    pub draw_parameters: bool,
    /// The `shader_pack_half_float` of `pipe_caps`.
    pub shader_pack_half_float: bool,
    /// The `multi_draw_indirect` of `pipe_caps`.
    pub multi_draw_indirect: bool,
    /// The `multi_draw_indirect_params` of `pipe_caps`.
    pub multi_draw_indirect_params: bool,
    /// The `multi_draw_indirect_partial_stride` of `pipe_caps`.
    pub multi_draw_indirect_partial_stride: bool,
    /// The `fs_position_is_sysval` of `pipe_caps`.
    pub fs_position_is_sysval: bool,
    /// The `fs_point_is_sysval` of `pipe_caps`.
    pub fs_point_is_sysval: bool,
    /// The `fs_face_is_integer_sysval` of `pipe_caps`.
    pub fs_face_is_integer_sysval: bool,
    /// The `invalidate_buffer` of `pipe_caps`.
    pub invalidate_buffer: bool,
    /// The `generate_mipmap` of `pipe_caps`.
    pub generate_mipmap: bool,
    /// The `string_marker` of `pipe_caps`.
    pub string_marker: bool,
    /// The `surface_no_compress` of `pipe_caps`.
    pub surface_no_compress: bool,
    /// The `surface_reinterpret_blocks` of `pipe_caps`.
    pub surface_reinterpret_blocks: bool,
    /// The `compressed_surface_reinterpret_blocks_layered` of `pipe_caps`.
    pub compressed_surface_reinterpret_blocks_layered: bool,
    /// The `query_buffer_object` of `pipe_caps`.
    pub query_buffer_object: bool,
    /// The `query_memory_info` of `pipe_caps`.
    pub query_memory_info: bool,
    /// The `framebuffer_no_attachment` of `pipe_caps`.
    pub framebuffer_no_attachment: bool,
    /// The `robust_buffer_access_behavior` of `pipe_caps`.
    pub robust_buffer_access_behavior: bool,
    /// The `cull_distance` of `pipe_caps`.
    pub cull_distance: bool,
    /// The `shader_group_vote` of `pipe_caps`.
    pub shader_group_vote: bool,
    /// The `shader_array_components` of `pipe_caps`.
    pub shader_array_components: bool,
    /// The `stream_output_interleave_buffers` of `pipe_caps`.
    pub stream_output_interleave_buffers: bool,
    /// The `native_fence_fd` of `pipe_caps`.
    pub native_fence_fd: bool,
    /// The `glsl_tess_levels_as_inputs` of `pipe_caps`.
    pub glsl_tess_levels_as_inputs: bool,
    /// The `legacy_math_rules` of `pipe_caps`.
    pub legacy_math_rules: bool,
    /// The `fp16` of `pipe_caps`.
    pub fp16: bool,
    /// The `doubles` of `pipe_caps`.
    pub doubles: bool,
    /// The `int64` of `pipe_caps`.
    pub int64: bool,
    /// The `shader_clock` of `pipe_caps`.
    pub shader_clock: bool,
    /// The `shader_realtime_clock` of `pipe_caps`.
    pub shader_realtime_clock: bool,
    /// The `polygon_mode_fill_rectangle` of `pipe_caps`.
    pub polygon_mode_fill_rectangle: bool,
    /// The `shader_ballot` of `pipe_caps`.
    pub shader_ballot: bool,
    /// The `tes_layer_viewport` of `pipe_caps`.
    pub tes_layer_viewport: bool,
    /// The `can_bind_const_buffer_as_vertex` of `pipe_caps`.
    pub can_bind_const_buffer_as_vertex: bool,
    /// The `allow_mapped_buffers_during_execution` of `pipe_caps`.
    pub allow_mapped_buffers_during_execution: bool,
    /// The `post_depth_coverage` of `pipe_caps`.
    pub post_depth_coverage: bool,
    /// The `bindless_texture` of `pipe_caps`.
    pub bindless_texture: bool,
    /// The `nir_samplers_as_deref` of `pipe_caps`.
    pub nir_samplers_as_deref: bool,
    /// The `query_so_overflow` of `pipe_caps`.
    pub query_so_overflow: bool,
    /// The `memobj` of `pipe_caps`.
    pub memobj: bool,
    /// The `load_constbuf` of `pipe_caps`.
    pub load_constbuf: bool,
    /// The `tile_raster_order` of `pipe_caps`.
    pub tile_raster_order: bool,
    /// The `signed_vertex_buffer_offset` of `pipe_caps`.
    pub signed_vertex_buffer_offset: bool,
    /// The `fence_signal` of `pipe_caps`.
    pub fence_signal: bool,
    /// The `packed_uniforms` of `pipe_caps`.
    pub packed_uniforms: bool,
    /// The `conservative_raster_post_snap_triangles` of `pipe_caps`.
    pub conservative_raster_post_snap_triangles: bool,
    /// The `conservative_raster_post_snap_points_lines` of `pipe_caps`.
    pub conservative_raster_post_snap_points_lines: bool,
    /// The `conservative_raster_pre_snap_triangles` of `pipe_caps`.
    pub conservative_raster_pre_snap_triangles: bool,
    /// The `conservative_raster_pre_snap_points_lines` of `pipe_caps`.
    pub conservative_raster_pre_snap_points_lines: bool,
    /// The `conservative_raster_post_depth_coverage` of `pipe_caps`.
    pub conservative_raster_post_depth_coverage: bool,
    /// The `conservative_raster_inner_coverage` of `pipe_caps`.
    pub conservative_raster_inner_coverage: bool,
    /// The `programmable_sample_locations` of `pipe_caps`.
    pub programmable_sample_locations: bool,
    /// The `texture_mirror_clamp_to_edge` of `pipe_caps`.
    pub texture_mirror_clamp_to_edge: bool,
    /// The `surface_sample_count` of `pipe_caps`.
    pub surface_sample_count: bool,
    /// The `image_atomic_float_add` of `pipe_caps`.
    pub image_atomic_float_add: bool,
    /// The `query_pipeline_statistics_single` of `pipe_caps`.
    pub query_pipeline_statistics_single: bool,
    /// The `dest_surface_srgb_control` of `pipe_caps`.
    pub dest_surface_srgb_control: bool,
    /// The `compute_grid_info_last_block` of `pipe_caps`.
    pub compute_grid_info_last_block: bool,
    /// The `compute_shader_derivatives` of `pipe_caps`.
    pub compute_shader_derivatives: bool,
    /// The `image_load_formatted` of `pipe_caps`.
    pub image_load_formatted: bool,
    /// The `image_store_formatted` of `pipe_caps`.
    pub image_store_formatted: bool,
    /// The `throttle` of `pipe_caps`.
    pub throttle: bool,
    /// The `cl_gl_sharing` of `pipe_caps`.
    pub cl_gl_sharing: bool,
    /// The `prefer_compute_for_multimedia` of `pipe_caps`.
    pub prefer_compute_for_multimedia: bool,
    /// The `fragment_shader_interlock` of `pipe_caps`.
    pub fragment_shader_interlock: bool,
    /// The `fbfetch_coherent` of `pipe_caps`.
    pub fbfetch_coherent: bool,
    /// The `atomic_float_minmax` of `pipe_caps`.
    pub atomic_float_minmax: bool,
    /// The `fragment_shader_texture_lod` of `pipe_caps`.
    pub fragment_shader_texture_lod: bool,
    /// The `fragment_shader_derivatives` of `pipe_caps`.
    pub fragment_shader_derivatives: bool,
    /// The `fragment_shader_depth` of `pipe_caps`.
    pub fragment_shader_depth: bool,
    /// The `texture_shadow_lod` of `pipe_caps`.
    pub texture_shadow_lod: bool,
    /// The `shader_samples_identical` of `pipe_caps`.
    pub shader_samples_identical: bool,
    /// The `image_atomic_inc_wrap` of `pipe_caps`.
    pub image_atomic_inc_wrap: bool,
    /// The `prefer_imm_arrays_as_constbuf` of `pipe_caps`.
    pub prefer_imm_arrays_as_constbuf: bool,
    /// The `gl_spirv` of `pipe_caps`.
    pub gl_spirv: bool,
    /// The `gl_spirv_variable_pointers` of `pipe_caps`.
    pub gl_spirv_variable_pointers: bool,
    /// The `demote_to_helper_invocation` of `pipe_caps`.
    pub demote_to_helper_invocation: bool,
    /// The `tgsi_tg4_component_in_swizzle` of `pipe_caps`.
    pub tgsi_tg4_component_in_swizzle: bool,
    /// The `flatshade` of `pipe_caps`.
    pub flatshade: bool,
    /// The `alpha_test` of `pipe_caps`.
    pub alpha_test: bool,
    /// The `two_sided_color` of `pipe_caps`.
    pub two_sided_color: bool,
    /// The `opencl_integer_functions` of `pipe_caps`.
    pub opencl_integer_functions: bool,
    /// The `integer_multiply_32x16` of `pipe_caps`.
    pub integer_multiply_32x16: bool,
    /// The `frontend_noop` of `pipe_caps`.
    pub frontend_noop: bool,
    /// The `nir_images_as_deref` of `pipe_caps`.
    pub nir_images_as_deref: bool,
    /// The `packed_stream_output` of `pipe_caps`.
    pub packed_stream_output: bool,
    /// The `viewport_transform_lowered` of `pipe_caps`.
    pub viewport_transform_lowered: bool,
    /// The `psiz_clamped` of `pipe_caps`.
    pub psiz_clamped: bool,
    /// The `viewport_swizzle` of `pipe_caps`.
    pub viewport_swizzle: bool,
    /// The `system_svm` of `pipe_caps`.
    pub system_svm: bool,
    /// The `viewport_mask` of `pipe_caps`.
    pub viewport_mask: bool,
    /// The `alpha_to_coverage_dither_control` of `pipe_caps`.
    pub alpha_to_coverage_dither_control: bool,
    /// The `map_unsynchronized_thread_safe` of `pipe_caps`.
    pub map_unsynchronized_thread_safe: bool,
    /// The `nir_atomics_as_deref` of `pipe_caps`.
    pub nir_atomics_as_deref: bool,
    /// The `no_clip_on_copy_tex` of `pipe_caps`.
    pub no_clip_on_copy_tex: bool,
    /// The `shader_atomic_int64` of `pipe_caps`.
    pub shader_atomic_int64: bool,
    /// The `device_protected_surface` of `pipe_caps`.
    pub device_protected_surface: bool,
    /// The `prefer_real_buffer_in_constbuf0` of `pipe_caps`.
    pub prefer_real_buffer_in_constbuf0: bool,
    /// The `gl_clamp` of `pipe_caps`.
    pub gl_clamp: bool,
    /// The `texrect` of `pipe_caps`.
    pub texrect: bool,
    /// The `sampler_reduction_minmax` of `pipe_caps`.
    pub sampler_reduction_minmax: bool,
    /// The `sampler_reduction_minmax_arb` of `pipe_caps`.
    pub sampler_reduction_minmax_arb: bool,
    /// The `allow_dynamic_vao_fastpath` of `pipe_caps`.
    pub allow_dynamic_vao_fastpath: bool,
    /// The `emulate_nonfixed_primitive_restart` of `pipe_caps`.
    pub emulate_nonfixed_primitive_restart: bool,
    /// The `prefer_back_buffer_reuse` of `pipe_caps`.
    pub prefer_back_buffer_reuse: bool,
    /// The `draw_vertex_state` of `pipe_caps`.
    pub draw_vertex_state: bool,
    /// The `prefer_pot_aligned_varyings` of `pipe_caps`.
    pub prefer_pot_aligned_varyings: bool,
    /// The `sparse_texture_full_array_cube_mipmaps` of `pipe_caps`.
    pub sparse_texture_full_array_cube_mipmaps: bool,
    /// The `query_sparse_texture_residency` of `pipe_caps`.
    pub query_sparse_texture_residency: bool,
    /// The `clamp_sparse_texture_lod` of `pipe_caps`.
    pub clamp_sparse_texture_lod: bool,
    /// The `allow_draw_out_of_order` of `pipe_caps`.
    pub allow_draw_out_of_order: bool,
    /// The `hardware_gl_select` of `pipe_caps`.
    pub hardware_gl_select: bool,
    /// The `dithering` of `pipe_caps`.
    pub dithering: bool,
    /// The `fbfetch_zs` of `pipe_caps`.
    pub fbfetch_zs: bool,
    /// The `timeline_semaphore_import` of `pipe_caps`.
    pub timeline_semaphore_import: bool,
    /// The `device_protected_context` of `pipe_caps`.
    pub device_protected_context: bool,
    /// The `allow_glthread_buffer_subdata_opt` of `pipe_caps`.
    pub allow_glthread_buffer_subdata_opt: bool,
    /// The `null_textures` of `pipe_caps`.
    pub null_textures: bool,
    /// The `astc_void_extents_need_denorm_flush` of `pipe_caps`.
    pub astc_void_extents_need_denorm_flush: bool,
    /// The `validate_all_dirty_states` of `pipe_caps`.
    pub validate_all_dirty_states: bool,
    /// The `has_const_bw` of `pipe_caps`.
    pub has_const_bw: bool,
    /// The `performance_monitor` of `pipe_caps`.
    pub performance_monitor: bool,
    /// The `texture_sampler_independent` of `pipe_caps`.
    pub texture_sampler_independent: bool,
    /// The `astc_decode_mode` of `pipe_caps`.
    pub astc_decode_mode: bool,
    /// The `shader_subgroup_quad_all_stages` of `pipe_caps`.
    pub shader_subgroup_quad_all_stages: bool,
    /// The `call_finalize_nir_in_linker` of `pipe_caps`.
    pub call_finalize_nir_in_linker: bool,
    /// The `mesh_shader` of `pipe_caps`.
    pub mesh_shader: bool,
    /// The `representative_fragment_test` of `pipe_caps`.
    pub representative_fragment_test: bool,
    /// The `prefer_persp` of `pipe_caps`.
    pub prefer_persp: bool,
    /// The `blit_3d` of `pipe_caps`.
    pub blit_3d: bool,
    /// The `glsl_bindless_handles_are_32bit` of `pipe_caps`.
    pub glsl_bindless_handles_are_32bit: bool,
    /// The `polygon_stipple` of `pipe_caps`.
    pub polygon_stipple: bool,
    /// The `disable_custom_border_color_caching` of `pipe_caps`.
    pub disable_custom_border_color_caching: bool,
    /// The `needs_split_planar_samplers` of `pipe_caps`.
    pub needs_split_planar_samplers: bool,
    /// The `accelerated` of `pipe_caps`.
    pub accelerated: i32,
    /// The `min_texel_offset` of `pipe_caps`.
    pub min_texel_offset: i32,
    /// The `max_texel_offset` of `pipe_caps`.
    pub max_texel_offset: i32,
    /// The `min_texture_gather_offset` of `pipe_caps`.
    pub min_texture_gather_offset: i32,
    /// The `max_texture_gather_offset` of `pipe_caps`.
    pub max_texture_gather_offset: i32,
    /// The `max_dual_source_render_targets` of `pipe_caps`.
    pub max_dual_source_render_targets: u32,
    /// The `max_render_targets` of `pipe_caps`.
    pub max_render_targets: u32,
    /// The `max_texture_2d_size` of `pipe_caps`.
    pub max_texture_2d_size: u32,
    /// The `max_texture_3d_levels` of `pipe_caps`.
    pub max_texture_3d_levels: u32,
    /// The `max_texture_cube_levels` of `pipe_caps`.
    pub max_texture_cube_levels: u32,
    /// The `max_stream_output_buffers` of `pipe_caps`.
    pub max_stream_output_buffers: u32,
    /// The `max_texture_array_layers` of `pipe_caps`.
    pub max_texture_array_layers: u32,
    /// The `max_stream_output_separate_components` of `pipe_caps`.
    pub max_stream_output_separate_components: u32,
    /// The `max_stream_output_interleaved_components` of `pipe_caps`.
    pub max_stream_output_interleaved_components: u32,
    /// The `glsl_feature_level` of `pipe_caps`.
    pub glsl_feature_level: u32,
    /// The `glsl_feature_level_compatibility` of `pipe_caps`.
    pub glsl_feature_level_compatibility: u32,
    /// The `essl_feature_level` of `pipe_caps`.
    pub essl_feature_level: u32,
    /// The `constant_buffer_offset_alignment` of `pipe_caps`.
    pub constant_buffer_offset_alignment: u32,
    /// The `timer_resolution` of `pipe_caps`.
    pub timer_resolution: u32,
    /// The `min_map_buffer_alignment` of `pipe_caps`.
    pub min_map_buffer_alignment: u32,
    /// The `texture_buffer_offset_alignment` of `pipe_caps`.
    pub texture_buffer_offset_alignment: u32,
    /// The `linear_image_pitch_alignment` of `pipe_caps`.
    pub linear_image_pitch_alignment: u32,
    /// The `linear_image_base_address_alignment` of `pipe_caps`.
    pub linear_image_base_address_alignment: u32,
    /// The `texture_transfer_modes` of `pipe_caps`.
    pub texture_transfer_modes: u32,
    /// The `texture_border_color_quirk` of `pipe_caps`.
    pub texture_border_color_quirk: u32,
    /// The `max_texel_buffer_elements` of `pipe_caps`.
    pub max_texel_buffer_elements: u32,
    /// The `max_viewports` of `pipe_caps`.
    pub max_viewports: u32,
    /// The `max_geometry_output_vertices` of `pipe_caps`.
    pub max_geometry_output_vertices: u32,
    /// The `max_geometry_total_output_components` of `pipe_caps`.
    pub max_geometry_total_output_components: u32,
    /// The `max_texture_gather_components` of `pipe_caps`.
    pub max_texture_gather_components: u32,
    /// The `max_vertex_streams` of `pipe_caps`.
    pub max_vertex_streams: u32,
    /// The `vendor_id` of `pipe_caps`.
    pub vendor_id: u32,
    /// The `device_id` of `pipe_caps`.
    pub device_id: u32,
    /// The `video_memory` of `pipe_caps`.
    pub video_memory: u32,
    /// The `max_vertex_attrib_stride` of `pipe_caps`.
    pub max_vertex_attrib_stride: u32,
    /// The `max_shader_patch_varyings` of `pipe_caps`.
    pub max_shader_patch_varyings: u32,
    /// The `shader_buffer_offset_alignment` of `pipe_caps`.
    pub shader_buffer_offset_alignment: u32,
    /// The `pci_group` of `pipe_caps`.
    pub pci_group: u32,
    /// The `pci_bus` of `pipe_caps`.
    pub pci_bus: u32,
    /// The `pci_device` of `pipe_caps`.
    pub pci_device: u32,
    /// The `pci_function` of `pipe_caps`.
    pub pci_function: u32,
    /// The `max_window_rectangles` of `pipe_caps`.
    pub max_window_rectangles: u32,
    /// The `viewport_subpixel_bits` of `pipe_caps`.
    pub viewport_subpixel_bits: u32,
    /// The `rasterizer_subpixel_bits` of `pipe_caps`.
    pub rasterizer_subpixel_bits: u32,
    /// The `mixed_color_depth_bits` of `pipe_caps`.
    pub mixed_color_depth_bits: u32,
    /// The `fbfetch` of `pipe_caps`.
    pub fbfetch: u32,
    /// The `blend_equation_advanced` of `pipe_caps`.
    pub blend_equation_advanced: u32,
    /// The `sparse_buffer_page_size` of `pipe_caps`.
    pub sparse_buffer_page_size: u32,
    /// The `max_combined_shader_output_resources` of `pipe_caps`.
    pub max_combined_shader_output_resources: u32,
    /// The `framebuffer_msaa_constraints` of `pipe_caps`.
    pub framebuffer_msaa_constraints: u32,
    /// The `context_priority_mask` of `pipe_caps`.
    pub context_priority_mask: u32,
    /// The `constbuf0_flags` of `pipe_caps`.
    pub constbuf0_flags: u32,
    /// The `max_conservative_raster_subpixel_precision_bias` of `pipe_caps`.
    pub max_conservative_raster_subpixel_precision_bias: u32,
    /// The `max_gs_invocations` of `pipe_caps`.
    pub max_gs_invocations: u32,
    /// The `max_shader_buffer_size` of `pipe_caps`.
    pub max_shader_buffer_size: u32,
    /// The `max_combined_shader_buffers` of `pipe_caps`.
    pub max_combined_shader_buffers: u32,
    /// The `max_combined_image_uniforms` of `pipe_caps`.
    pub max_combined_image_uniforms: u32,
    /// The `max_combined_hw_atomic_counters` of `pipe_caps`.
    pub max_combined_hw_atomic_counters: u32,
    /// The `max_combined_hw_atomic_counter_buffers` of `pipe_caps`.
    pub max_combined_hw_atomic_counter_buffers: u32,
    /// The `max_texture_upload_memory_budget` of `pipe_caps`.
    pub max_texture_upload_memory_budget: u32,
    /// The `max_vertex_element_src_offset` of `pipe_caps`.
    pub max_vertex_element_src_offset: u32,
    /// The `max_varyings` of `pipe_caps`.
    pub max_varyings: u32,
    /// The `ignored_output_varyings` of `pipe_caps`.
    pub ignored_output_varyings: [u64; 8],
    /// The `dmabuf` of `pipe_caps`.
    pub dmabuf: u32,
    /// The `clip_planes` of `pipe_caps`.
    pub clip_planes: u32,
    /// The `max_vertex_buffers` of `pipe_caps`.
    pub max_vertex_buffers: u32,
    /// The `gl_begin_end_buffer_size` of `pipe_caps`.
    pub gl_begin_end_buffer_size: u32,
    /// The `glsl_zero_init` of `pipe_caps`.
    pub glsl_zero_init: u32,
    /// The `max_texture_mb` of `pipe_caps`.
    pub max_texture_mb: u32,
    /// The `supported_prim_modes` of `pipe_caps`.
    pub supported_prim_modes: u32,
    /// The `supported_prim_modes_with_restart` of `pipe_caps`.
    pub supported_prim_modes_with_restart: u32,
    /// The `max_sparse_texture_size` of `pipe_caps`.
    pub max_sparse_texture_size: u32,
    /// The `max_sparse_3d_texture_size` of `pipe_caps`.
    pub max_sparse_3d_texture_size: u32,
    /// The `max_sparse_array_texture_layers` of `pipe_caps`.
    pub max_sparse_array_texture_layers: u32,
    /// The `max_constant_buffer_size` of `pipe_caps`.
    pub max_constant_buffer_size: u32,
    /// The `query_timestamp_bits` of `pipe_caps`.
    pub query_timestamp_bits: u32,
    /// The `shader_subgroup_size` of `pipe_caps`.
    pub shader_subgroup_size: u32,
    /// The `shader_subgroup_supported_stages` of `pipe_caps`.
    pub shader_subgroup_supported_stages: u32,
    /// The `shader_subgroup_supported_features` of `pipe_caps`.
    pub shader_subgroup_supported_features: u32,
    /// The `shader_pixel_local_storage_size` of `pipe_caps`.
    pub shader_pixel_local_storage_size: u32,
    /// The `shader_pixel_local_storage_fast_size` of `pipe_caps`.
    pub shader_pixel_local_storage_fast_size: u32,
    /// The `multiview` of `pipe_caps`.
    pub multiview: u32,
    /// The `max_label_length` of `pipe_caps`.
    pub max_label_length: u32,
    /// The `max_timeline_semaphore_difference` of `pipe_caps`.
    pub max_timeline_semaphore_difference: u64,
    /// The `map_client_pointer` of `pipe_caps`.
    pub map_client_pointer: u32,
    /// The `min_vma` of `pipe_caps`.
    pub min_vma: u64,
    /// The `max_vma` of `pipe_caps`.
    pub max_vma: u64,
    /// The `hw_clear_buffer_sizes` of `pipe_caps`.
    pub hw_clear_buffer_sizes: u16,
    /// The `device_type` of `pipe_caps`.
    pub device_type: values::DeviceType,
    /// The `vertex_input_alignment` of `pipe_caps`.
    pub vertex_input_alignment: values::VertexInputAlignment,
    /// The `endianness` of `pipe_caps`.
    pub endianness: values::Endian,
    /// The `point_size_fixed` of `pipe_caps`.
    pub point_size_fixed: values::PointSizeLowerMode,
    /// The `min_line_width` of `pipe_caps`.
    pub min_line_width: f32,
    /// The `min_line_width_aa` of `pipe_caps`.
    pub min_line_width_aa: f32,
    /// The `max_line_width` of `pipe_caps`.
    pub max_line_width: f32,
    /// The `max_line_width_aa` of `pipe_caps`.
    pub max_line_width_aa: f32,
    /// The `line_width_granularity` of `pipe_caps`.
    pub line_width_granularity: f32,
    /// The `min_point_size` of `pipe_caps`.
    pub min_point_size: f32,
    /// The `min_point_size_aa` of `pipe_caps`.
    pub min_point_size_aa: f32,
    /// The `max_point_size` of `pipe_caps`.
    pub max_point_size: f32,
    /// The `max_point_size_aa` of `pipe_caps`.
    pub max_point_size_aa: f32,
    /// The `point_size_granularity` of `pipe_caps`.
    pub point_size_granularity: f32,
    /// The `max_texture_anisotropy` of `pipe_caps`.
    pub max_texture_anisotropy: f32,
    /// The `max_texture_lod_bias` of `pipe_caps`.
    pub max_texture_lod_bias: f32,
    /// The `min_conservative_raster_dilate` of `pipe_caps`.
    pub min_conservative_raster_dilate: f32,
    /// The `max_conservative_raster_dilate` of `pipe_caps`.
    pub max_conservative_raster_dilate: f32,
    /// The `conservative_raster_dilate_granularity` of `pipe_caps`.
    pub conservative_raster_dilate_granularity: f32,
    /// The `mesh` of `pipe_caps`.
    pub mesh: MeshCaps,
}

/// Mesa's `struct pipe_shader_caps`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShaderCaps {
    /// The `max_instructions` of `pipe_shader_caps`.
    pub max_instructions: u32,
    /// The `max_alu_instructions` of `pipe_shader_caps`.
    pub max_alu_instructions: u32,
    /// The `max_tex_instructions` of `pipe_shader_caps`.
    pub max_tex_instructions: u32,
    /// The `max_tex_indirections` of `pipe_shader_caps`.
    pub max_tex_indirections: u32,
    /// The `max_control_flow_depth` of `pipe_shader_caps`.
    pub max_control_flow_depth: u32,
    /// The `max_inputs` of `pipe_shader_caps`.
    pub max_inputs: u32,
    /// The `max_outputs` of `pipe_shader_caps`.
    pub max_outputs: u32,
    /// The `max_const_buffer0_size` of `pipe_shader_caps`.
    pub max_const_buffer0_size: u32,
    /// The `max_const_buffers` of `pipe_shader_caps`.
    pub max_const_buffers: u32,
    /// The `max_temps` of `pipe_shader_caps`.
    pub max_temps: u32,
    /// The `max_texture_samplers` of `pipe_shader_caps`.
    pub max_texture_samplers: u32,
    /// The `max_sampler_views` of `pipe_shader_caps`.
    pub max_sampler_views: u32,
    /// The `max_shader_buffers` of `pipe_shader_caps`.
    pub max_shader_buffers: u32,
    /// The `max_shader_images` of `pipe_shader_caps`.
    pub max_shader_images: u32,
    /// The `max_hw_atomic_counters` of `pipe_shader_caps`.
    pub max_hw_atomic_counters: u32,
    /// The `max_hw_atomic_counter_buffers` of `pipe_shader_caps`.
    pub max_hw_atomic_counter_buffers: u32,
    /// The `supported_irs` of `pipe_shader_caps`.
    pub supported_irs: u32,
    /// The `cont_supported` of `pipe_shader_caps`.
    pub cont_supported: bool,
    /// The `indirect_temp_addr` of `pipe_shader_caps`.
    pub indirect_temp_addr: bool,
    /// The `indirect_const_addr` of `pipe_shader_caps`.
    pub indirect_const_addr: bool,
    /// The `subroutines` of `pipe_shader_caps`.
    pub subroutines: bool,
    /// The `integers` of `pipe_shader_caps`.
    pub integers: bool,
    /// The `int64_atomics` of `pipe_shader_caps`.
    pub int64_atomics: bool,
    /// The `fp16` of `pipe_shader_caps`.
    pub fp16: bool,
    /// The `fp16_derivatives` of `pipe_shader_caps`.
    pub fp16_derivatives: bool,
    /// The `fp16_const_buffers` of `pipe_shader_caps`.
    pub fp16_const_buffers: bool,
    /// The `fp16_no_denorms` of `pipe_shader_caps`.
    pub fp16_no_denorms: bool,
    /// The `int16` of `pipe_shader_caps`.
    pub int16: bool,
    /// The `glsl_16bit_consts` of `pipe_shader_caps`.
    pub glsl_16bit_consts: bool,
    /// The `glsl_16bit_load_dst` of `pipe_shader_caps`.
    pub glsl_16bit_load_dst: bool,
    /// The `tgsi_sqrt_supported` of `pipe_shader_caps`.
    pub tgsi_sqrt_supported: bool,
    /// The `tgsi_any_inout_decl_range` of `pipe_shader_caps`.
    pub tgsi_any_inout_decl_range: bool,
}

/// Mesa's `struct pipe_compute_caps`, owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputeCaps {
    /// The `address_bits` of `pipe_compute_caps`.
    pub address_bits: u32,
    /// The `grid_dimension` of `pipe_compute_caps`.
    pub grid_dimension: u32,
    /// The `max_grid_size` of `pipe_compute_caps`.
    pub max_grid_size: [u32; 3],
    /// The `max_block_size` of `pipe_compute_caps`.
    pub max_block_size: [u32; 3],
    /// The `max_threads_per_block` of `pipe_compute_caps`.
    pub max_threads_per_block: u32,
    /// The `max_local_size` of `pipe_compute_caps`.
    pub max_local_size: u32,
    /// The `max_clock_frequency` of `pipe_compute_caps`.
    pub max_clock_frequency: u32,
    /// The `max_compute_units` of `pipe_compute_caps`.
    pub max_compute_units: u32,
    /// The `max_subgroups` of `pipe_compute_caps`.
    pub max_subgroups: u32,
    /// The `subgroup_sizes` of `pipe_compute_caps`.
    pub subgroup_sizes: u32,
    /// The `max_variable_threads_per_block` of `pipe_compute_caps`.
    pub max_variable_threads_per_block: u32,
    /// The `max_mem_alloc_size` of `pipe_compute_caps`.
    pub max_mem_alloc_size: u64,
    /// The `max_global_size` of `pipe_compute_caps`.
    pub max_global_size: u64,
}


/// A resource's bytes mapped for the CPU: the implementation's mapping and the bytes, a copy the glue lends Mesa until the unmap.
#[derive(Debug)]
pub struct Mapped<Mapping> {
    /// The implementation's mapping, given back at the unmap.
    pub mapping: Mapping,
    /// The mapped bytes as the implementation has them.
    pub bytes: Vec<u8>,
    /// Bytes from one row to the next.
    pub stride: u32,
    /// Bytes from one layer to the next.
    pub layer_stride: u64,
}

/// The screen: the library Mesa's frontend loads, holding the kinds of its objects.
pub trait Gallium: Sized {
    /// The shader compiler the programs come from.
    type Compiler: wcontainers_mesa_compiler::MesaCompiler;

    /// A rendering context.
    type Context: GalliumContext<Screen = Self>;

    /// A buffer or texture, counted (Mesa's pipe_reference): the implementation keeps one by cloning its handle, and the last handle dropped ends it.
    type Resource;

    /// A point in a context's commands, counted (fence_reference): kept by cloning its handle.
    type Fence;

    /// A blend state.
    type Blend;

    /// A depth, stencil and alpha state.
    type DepthStencilAlpha;

    /// A rasterizer state.
    type Rasterizer;

    /// A sampler state.
    type Sampler;

    /// A vertex elements state.
    type VertexElements;

    /// A vertex program, made from mesa-compiler's Received of that stage.
    type VertexShader;

    /// A fragment program, from a Received of that stage.
    type FragmentShader;

    /// A geometry program, from a Received of that stage.
    type GeometryShader;

    /// A tessellation control program, from a Received of that stage.
    type TessControlShader;

    /// A tessellation evaluation program, from a Received of that stage.
    type TessEvaluationShader;

    /// A compute program, from a Received of that stage.
    type ComputeShader;

    /// A view of a resource for sampling, counted (pipe_sampler_view_reference): kept by cloning its handle.
    type SamplerView;

    /// A range of a buffer that streamed outputs are written to, counted (pipe_so_target_reference): kept by cloning its handle.
    type StreamOutputTarget;

    /// A query that is begun and ended (begin_query, end_query).
    type Query;

    /// A query between its begin and its end.
    type ActiveQuery;

    /// A query only ended (PIPE_QUERY_TIMESTAMP, PIPE_QUERY_GPU_FINISHED: queryobj.c:269 to 277).
    type EndOnlyQuery;

    /// A resource's bytes mapped for the CPU, between a map and its unmap.
    type Mapping;

    /// Mesa's `pipe_screen.context_create`, called first at mesa/state_tracker/st_manager.c:1023.
    fn context_create(&self, priv_: u64, flags: u32) -> Option<Self::Context>;

    /// Mesa's `pipe_screen.destroy`, called first at gallium/frontends/dri/dri_screen.c:580.
    fn destroy(&self);

    /// Mesa's `pipe_screen.fence_finish`, called first at mesa/state_tracker/st_manager.c:819.
    fn fence_finish(&self, ctx: Option<&Self::Context>, fence: &Arc<Self::Fence>, timeout: u64) -> bool;

    /// Mesa's `pipe_screen.flush_frontbuffer`, called first at gallium/frontends/dri/drisw.c:173.
    fn flush_frontbuffer(&self, ctx: Option<&Self::Context>, resource: &Arc<Self::Resource>, level: u32, layer: u32, winsys_drawable_handle: u64, subbox: Option<&PipeBox>);

    /// Mesa's `pipe_screen.get_device_vendor`, called first at gallium/frontends/dri/dri_util.c:1140.
    fn get_device_vendor(&self) -> Vec<u8>;

    /// Mesa's `pipe_screen.get_name`, called first at mesa/main/getstring.c:149.
    fn get_name(&self) -> Vec<u8>;

    /// Mesa's `pipe_screen.get_vendor`, called first at mesa/main/getstring.c:143.
    fn get_vendor(&self) -> Vec<u8>;

    /// Mesa's `pipe_screen.is_format_supported`, called first at mesa/state_tracker/st_cb_eglimage.c:48.
    fn is_format_supported(&self, format: values::Format, target: values::TextureTarget, sample_count: u32, storage_sample_count: u32, bindings: u32) -> bool;

    /// Mesa's `pipe_screen.resource_create`, called first at mesa/state_tracker/st_texture.c:106.
    fn resource_create(&self, templat: &ResourceTemplate) -> Option<Self::Resource>;

    /// Mesa's `pipe_screen.caps`, filled once when the screen is made and read from then on (first at mesa/state_tracker/st_extensions.c:136): `caps`
    /// holds Mesa's defaults, as u_init_pipe_screen_caps sets them (gallium/auxiliary/util/u_screen.c:41) for the implementation to change.
    fn caps(&self, caps: &mut Caps);

    /// Mesa's `pipe_screen.shader_caps`, one by shader stage (MESA_SHADER_*, up to MESA_SHADER_MESH_STAGES), filled once when the screen is made and read from then on (first at mesa/state_tracker/st_atom_atomicbuf.c:74): `caps`
    /// holds zero (Mesa sets no default) for the implementation to change.
    fn shader_caps(&self, stage: values::MesaShaderStage, caps: &mut ShaderCaps);

    /// Mesa's `pipe_screen.compute_caps`, filled once when the screen is made and read from then on (first at mesa/state_tracker/st_extensions.c:1741): `caps`
    /// holds zero (Mesa sets no default) for the implementation to change.
    fn compute_caps(&self, caps: &mut ComputeCaps);

}

/// A rendering context of a screen.
pub trait GalliumContext: Sized {
    /// The screen it was made on.
    type Screen: Gallium<Context = Self>;

    /// Mesa's `pipe_context.begin_query`, called first at mesa/main/performance_monitor.c:198.
    ///
    /// A failure hands back what it took: a failed begin leaves q->pq set and free_queries destroys it, never begun (mesa/main/queryobj.c:239 to 247).
    fn begin_query(&self, q: <Self::Screen as Gallium>::Query) -> Result<<Self::Screen as Gallium>::ActiveQuery, <Self::Screen as Gallium>::Query>;

    /// Mesa's `pipe_context.bind_blend_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:449.
    fn bind_blend_state(&self, arg1: Option<&<Self::Screen as Gallium>::Blend>);

    /// Mesa's `pipe_context.bind_compute_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:525.
    fn bind_compute_state(&self, arg1: Option<&<Self::Screen as Gallium>::ComputeShader>);

    /// Mesa's `pipe_context.bind_depth_stencil_alpha_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:510.
    fn bind_depth_stencil_alpha_state(&self, arg1: Option<&<Self::Screen as Gallium>::DepthStencilAlpha>);

    /// Mesa's `pipe_context.bind_fs_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:513.
    fn bind_fs_state(&self, arg1: Option<&<Self::Screen as Gallium>::FragmentShader>);

    /// Mesa's `pipe_context.bind_gs_state`, called first at mesa/state_tracker/st_context.c:957.
    fn bind_gs_state(&self, arg1: Option<&<Self::Screen as Gallium>::GeometryShader>);

    /// Mesa's `pipe_context.bind_rasterizer_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:450.
    fn bind_rasterizer_state(&self, arg1: Option<&<Self::Screen as Gallium>::Rasterizer>);

    /// Mesa's `pipe_context.bind_sampler_states`, called first at gallium/auxiliary/cso_cache/cso_context.c:493.
    fn bind_sampler_states(&self, shader: values::MesaShaderStage, start_slot: u32, samplers: &[Option<&<Self::Screen as Gallium>::Sampler>]);

    /// Mesa's `pipe_context.bind_tcs_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:521.
    fn bind_tcs_state(&self, arg1: Option<&<Self::Screen as Gallium>::TessControlShader>);

    /// Mesa's `pipe_context.bind_tes_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:522.
    fn bind_tes_state(&self, arg1: Option<&<Self::Screen as Gallium>::TessEvaluationShader>);

    /// Mesa's `pipe_context.bind_vertex_elements_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:531.
    fn bind_vertex_elements_state(&self, arg1: Option<&<Self::Screen as Gallium>::VertexElements>);

    /// Mesa's `pipe_context.bind_vs_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:515.
    fn bind_vs_state(&self, arg1: Option<&<Self::Screen as Gallium>::VertexShader>);

    /// Mesa's `pipe_context.blit`, called first at mesa/state_tracker/st_cb_readpixels.c:350.
    fn blit(&self, info: &BlitInfo<'_, <Self::Screen as Gallium>::Resource>);

    /// Mesa's `pipe_context.buffer_map`, called first at gallium/auxiliary/util/u_inlines.h:530.
    fn buffer_map(&self, resource: &Arc<<Self::Screen as Gallium>::Resource>, level: u32, usage: values::MapFlags, arg4: &PipeBox) -> Option<Mapped<<Self::Screen as Gallium>::Mapping>>;

    /// Mesa's `pipe_context.buffer_subdata`, called first at mesa/main/bufferobj.c:116.
    fn buffer_subdata(&self, arg1: &Arc<<Self::Screen as Gallium>::Resource>, usage: values::MapFlags, offset: u32, size: u32, data: &[u8]);

    /// Mesa's `pipe_context.buffer_unmap`, called first at gallium/auxiliary/util/u_inlines.h:565.
    fn buffer_unmap(&self, transfer: <Self::Screen as Gallium>::Mapping, written: &[u8]);

    /// Mesa's `pipe_context.clear`, called first at mesa/state_tracker/st_cb_clear.c:521.
    #[expect(clippy::too_many_arguments, reason = "the pointer's parameters, as Mesa's header declares them")]
    fn clear(&self, buffers: u32, color_clear_mask: u32, stencil_clear_mask: u8, scissor_state: Option<&ScissorState>, color: &[u32; 4], depth: f64, stencil: u32);

    /// Mesa's `pipe_context.clear_depth_stencil`, called first at gallium/auxiliary/util/u_threaded_context.c:5469.
    #[expect(clippy::too_many_arguments, reason = "the pointer's parameters, as Mesa's header declares them")]
    fn clear_depth_stencil(&self, dst: &Surface<'_, <Self::Screen as Gallium>::Resource>, clear_flags: u32, depth: f64, stencil: u32, dstx: u32, dsty: u32, width: u32, height: u32, render_condition_enabled: bool);

    /// Mesa's `pipe_context.clear_render_target`, called first at gallium/auxiliary/util/u_threaded_context.c:5422.
    #[expect(clippy::too_many_arguments, reason = "the pointer's parameters, as Mesa's header declares them")]
    fn clear_render_target(&self, dst: &Surface<'_, <Self::Screen as Gallium>::Resource>, color: &[u32; 4], dstx: u32, dsty: u32, width: u32, height: u32, render_condition_enabled: bool);

    /// Mesa's `pipe_context.create_blend_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:649.
    fn create_blend_state(&self, arg1: &BlendState) -> Option<<Self::Screen as Gallium>::Blend>;

    /// Mesa's `pipe_context.create_compute_state`, called first at gallium/auxiliary/util/u_tests.c:844.
    fn create_compute_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedCompute, arg1: &ComputeState) -> Option<<Self::Screen as Gallium>::ComputeShader>;

    /// Mesa's `pipe_context.create_depth_stencil_alpha_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:709.
    fn create_depth_stencil_alpha_state(&self, arg1: &DepthStencilAlphaState) -> Option<<Self::Screen as Gallium>::DepthStencilAlpha>;

    /// Mesa's `pipe_context.create_fs_state`, called first at mesa/state_tracker/st_program.c:573.
    fn create_fs_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedFragment, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::FragmentShader>;

    /// Mesa's `pipe_context.create_gs_state`, called first at mesa/state_tracker/st_program.c:570.
    fn create_gs_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedGeometry, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::GeometryShader>;

    /// Mesa's `pipe_context.create_query`, on a Query, called first at mesa/main/performance_monitor.c:125.
    fn create_query(&self, query_type: values::QueryType, index: u32) -> Option<<Self::Screen as Gallium>::Query>;

    /// Mesa's `pipe_context.create_query`, on a EndOnlyQuery, called first at mesa/main/performance_monitor.c:125.
    fn create_query_end_only_query(&self, query_type: values::QueryType, index: u32) -> Option<<Self::Screen as Gallium>::EndOnlyQuery>;

    /// Mesa's `pipe_context.create_rasterizer_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:777.
    fn create_rasterizer_state(&self, arg1: &RasterizerState) -> Option<<Self::Screen as Gallium>::Rasterizer>;

    /// Mesa's `pipe_context.create_sampler_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:1591.
    fn create_sampler_state(&self, arg1: &SamplerState) -> Option<<Self::Screen as Gallium>::Sampler>;

    /// Mesa's `pipe_context.create_sampler_view`, called first at mesa/state_tracker/st_sampler_view.c:524.
    fn create_sampler_view(&self, texture: &Arc<<Self::Screen as Gallium>::Resource>, templat: &SamplerViewTemplate) -> Option<<Self::Screen as Gallium>::SamplerView>;

    /// Mesa's `pipe_context.create_stream_output_target`, called first at mesa/main/transformfeedback.c:445.
    fn create_stream_output_target(&self, arg1: &Arc<<Self::Screen as Gallium>::Resource>, buffer_offset: u32, buffer_size: u32) -> Option<<Self::Screen as Gallium>::StreamOutputTarget>;

    /// Mesa's `pipe_context.create_tcs_state`, called first at mesa/state_tracker/st_program.c:564.
    fn create_tcs_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedTessControl, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::TessControlShader>;

    /// Mesa's `pipe_context.create_tes_state`, called first at mesa/state_tracker/st_program.c:567.
    fn create_tes_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedTessEvaluation, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::TessEvaluationShader>;

    /// Mesa's `pipe_context.create_vertex_elements_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:1409.
    fn create_vertex_elements_state(&self, arg2: &[VertexElement]) -> Option<<Self::Screen as Gallium>::VertexElements>;

    /// Mesa's `pipe_context.create_vs_state`, called first at mesa/state_tracker/st_program.c:561.
    fn create_vs_state(&self, program: <<Self::Screen as Gallium>::Compiler as wcontainers_mesa_compiler::MesaCompiler>::ReceivedVertex, arg1: &ShaderState) -> Option<<Self::Screen as Gallium>::VertexShader>;

    /// Mesa's `pipe_context.delete_blend_state`, called first at gallium/auxiliary/cso_cache/cso_cache.c:46.
    fn delete_blend_state(&self, arg1: <Self::Screen as Gallium>::Blend);

    /// Mesa's `pipe_context.delete_compute_state`, called first at mesa/state_tracker/st_context.c:312.
    fn delete_compute_state(&self, arg1: <Self::Screen as Gallium>::ComputeShader);

    /// Mesa's `pipe_context.delete_depth_stencil_alpha_state`, called first at gallium/auxiliary/cso_cache/cso_cache.c:52.
    fn delete_depth_stencil_alpha_state(&self, arg1: <Self::Screen as Gallium>::DepthStencilAlpha);

    /// Mesa's `pipe_context.delete_fs_state`, called first at mesa/state_tracker/st_context.c:296.
    fn delete_fs_state(&self, arg1: <Self::Screen as Gallium>::FragmentShader);

    /// Mesa's `pipe_context.delete_gs_state`, called first at mesa/state_tracker/st_context.c:300.
    fn delete_gs_state(&self, arg1: <Self::Screen as Gallium>::GeometryShader);

    /// Mesa's `pipe_context.delete_rasterizer_state`, called first at gallium/auxiliary/cso_cache/cso_cache.c:56.
    fn delete_rasterizer_state(&self, arg1: <Self::Screen as Gallium>::Rasterizer);

    /// Mesa's `pipe_context.delete_sampler_state`, called first at gallium/auxiliary/cso_cache/cso_cache.c:49.
    fn delete_sampler_state(&self, arg1: <Self::Screen as Gallium>::Sampler);

    /// Mesa's `pipe_context.delete_tcs_state`, called first at mesa/state_tracker/st_context.c:304.
    fn delete_tcs_state(&self, arg1: <Self::Screen as Gallium>::TessControlShader);

    /// Mesa's `pipe_context.delete_tes_state`, called first at mesa/state_tracker/st_context.c:308.
    fn delete_tes_state(&self, arg1: <Self::Screen as Gallium>::TessEvaluationShader);

    /// Mesa's `pipe_context.delete_vertex_elements_state`, called first at gallium/auxiliary/cso_cache/cso_cache.c:59.
    fn delete_vertex_elements_state(&self, arg1: <Self::Screen as Gallium>::VertexElements);

    /// Mesa's `pipe_context.delete_vs_state`, called first at mesa/state_tracker/st_context.c:292.
    fn delete_vs_state(&self, arg1: <Self::Screen as Gallium>::VertexShader);

    /// Mesa's `pipe_context.destroy`, called first at mesa/state_tracker/st_context.c:370.
    fn destroy(&self);

    /// Mesa's `pipe_context.destroy_query`, on a Query, called first at mesa/main/performance_monitor.c:159.
    fn destroy_query(&self, q: <Self::Screen as Gallium>::Query);

    /// Mesa's `pipe_context.destroy_query`, on a EndOnlyQuery, called first at mesa/main/performance_monitor.c:159.
    fn destroy_query_end_only_query(&self, q: <Self::Screen as Gallium>::EndOnlyQuery);

    /// Mesa's `pipe_context.destroy_query`, on a ActiveQuery, called first at mesa/main/performance_monitor.c:159.
    fn destroy_query_active_query(&self, q: <Self::Screen as Gallium>::ActiveQuery);

    /// Mesa's `pipe_context.draw_vbo`, called first at gallium/auxiliary/cso_cache/cso_context.h:314.
    fn draw_vbo(&self, info: &DrawInfo<'_, <Self::Screen as Gallium>::Resource>, drawid_offset: u32, indirect: Option<&DrawIndirectInfo<'_, <Self::Screen as Gallium>::Resource, <Self::Screen as Gallium>::StreamOutputTarget>>, draws: &[DrawStartCountBias]);

    /// Mesa's `pipe_context.end_query`, on a ActiveQuery, called first at mesa/main/performance_monitor.c:223.
    ///
    /// A failure hands back what it took: a failed end leaves q->pq set, and st/mesa ends, reads or destroys that same query later (mesa/main/queryobj.c:277 to 282).
    fn end_query(&self, q: <Self::Screen as Gallium>::ActiveQuery) -> Result<<Self::Screen as Gallium>::Query, <Self::Screen as Gallium>::ActiveQuery>;

    /// Mesa's `pipe_context.end_query`, on a EndOnlyQuery, called first at mesa/main/performance_monitor.c:223.
    fn end_query_end_only_query(&self, q: &<Self::Screen as Gallium>::EndOnlyQuery) -> bool;

    /// Mesa's `pipe_context.flush`, called first at mesa/state_tracker/st_interop.c:438.
    fn flush(&self, flags: values::FlushFlags) -> Option<<Self::Screen as Gallium>::Fence>;

    /// Mesa's `pipe_context.flush_resource`, called first at mesa/state_tracker/st_interop.c:373.
    fn flush_resource(&self, resource: &Arc<<Self::Screen as Gallium>::Resource>);

    /// Mesa's `pipe_context.generate_mipmap`, called first at mesa/state_tracker/st_gen_mipmap.c:145.
    fn generate_mipmap(&self, resource: &Arc<<Self::Screen as Gallium>::Resource>, format: values::Format, base_level: u32, last_level: u32, first_layer: u32, last_layer: u32) -> bool;

    /// Mesa's `pipe_context.get_device_reset_status`, called first at mesa/state_tracker/st_cb_flush.c:162.
    fn get_device_reset_status(&self) -> values::ResetStatus;

    /// Mesa's `pipe_context.get_query_result`, on a Query, called first at mesa/main/performance_monitor.c:259.
    fn get_query_result(&self, q: &<Self::Screen as Gallium>::Query, wait: bool, result: &mut [u8]) -> bool;

    /// Mesa's `pipe_context.get_query_result`, on a EndOnlyQuery, called first at mesa/main/performance_monitor.c:259.
    fn get_query_result_end_only_query(&self, q: &<Self::Screen as Gallium>::EndOnlyQuery, wait: bool, result: &mut [u8]) -> bool;

    /// Mesa's `pipe_context.get_timestamp`, called first at mesa/main/queryobj.c:417.
    fn get_timestamp(&self) -> u64;

    /// Mesa's `pipe_context.launch_grid`, called first at mesa/state_tracker/st_pbo_compute.c:1170.
    fn launch_grid(&self, info: &GridInfo<'_, <Self::Screen as Gallium>::Resource>);

    /// Mesa's `pipe_context.memory_barrier`, called first at mesa/state_tracker/st_cb_readpixels.c:245.
    fn memory_barrier(&self, flags: u32);

    /// Mesa's `pipe_context.render_condition`, on a Query, called first at gallium/auxiliary/cso_cache/cso_context.c:1084.
    fn render_condition(&self, query: Option<&<Self::Screen as Gallium>::Query>, condition: bool, mode: values::RenderCondFlag);

    /// Mesa's `pipe_context.render_condition`, on a EndOnlyQuery, called first at gallium/auxiliary/cso_cache/cso_context.c:1084.
    fn render_condition_end_only_query(&self, query: &<Self::Screen as Gallium>::EndOnlyQuery, condition: bool, mode: values::RenderCondFlag);

    /// Mesa's `pipe_context.resource_copy_region`, called first at mesa/state_tracker/st_texture.c:423.
    #[expect(clippy::too_many_arguments, reason = "the pointer's parameters, as Mesa's header declares them")]
    fn resource_copy_region(&self, dst: &Arc<<Self::Screen as Gallium>::Resource>, dst_level: u32, dstx: u32, dsty: u32, dstz: u32, src: &Arc<<Self::Screen as Gallium>::Resource>, src_level: u32, src_box: &PipeBox);

    /// Mesa's `pipe_context.set_active_query_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:1919.
    fn set_active_query_state(&self, enable: bool);

    /// Mesa's `pipe_context.set_blend_color`, called first at mesa/state_tracker/st_atom_blend.c:367.
    fn set_blend_color(&self, arg1: &BlendColor);

    /// Mesa's `pipe_context.set_clip_state`, called first at mesa/state_tracker/st_atom_clip.c:67.
    fn set_clip_state(&self, arg1: &ClipState);

    /// Mesa's `pipe_context.set_constant_buffer`, called first at mesa/state_tracker/st_atom_constbuf.c:60.
    fn set_constant_buffer(&self, shader: values::MesaShaderStage, index: u32, buf: Option<&ConstantBuffer<'_, <Self::Screen as Gallium>::Resource>>);

    /// Mesa's `pipe_context.set_framebuffer_state`, called first at gallium/auxiliary/cso_cache/cso_context.c:538.
    fn set_framebuffer_state(&self, arg1: &FramebufferState<'_, <Self::Screen as Gallium>::Resource>);

    /// Mesa's `pipe_context.set_patch_vertices`, called first at mesa/state_tracker/st_atom_tess.c:53.
    fn set_patch_vertices(&self, patch_vertices: u8);

    /// Mesa's `pipe_context.set_polygon_stipple`, called first at mesa/state_tracker/st_atom_stipple.c:86.
    fn set_polygon_stipple(&self, arg1: &PolyStipple);

    /// Mesa's `pipe_context.set_sample_mask`, called first at gallium/auxiliary/cso_cache/cso_context.c:573.
    fn set_sample_mask(&self, sample_mask: u32);

    /// Mesa's `pipe_context.set_sampler_views`, called first at mesa/state_tracker/st_cb_readpixels.c:184.
    fn set_sampler_views(&self, shader: values::MesaShaderStage, start_slot: u32, num_views: u32, unbind_num_trailing_slots: u32, views: Option<&[Option<&Arc<<Self::Screen as Gallium>::SamplerView>>]>);

    /// Mesa's `pipe_context.set_scissor_states`, called first at mesa/state_tracker/st_atom_scissor.c:106.
    fn set_scissor_states(&self, start_slot: u32, arg3: &[ScissorState]);

    /// Mesa's `pipe_context.set_shader_buffers`, called first at mesa/state_tracker/st_draw_hw_select.c:703.
    fn set_shader_buffers(&self, shader: values::MesaShaderStage, start_slot: u32, count: u32, buffers: Option<&[ShaderBuffer<'_, <Self::Screen as Gallium>::Resource>]>, writable_bitmask: u32);

    /// Mesa's `pipe_context.set_shader_images`, called first at mesa/state_tracker/st_atom_image.c:161.
    fn set_shader_images(&self, shader: values::MesaShaderStage, start_slot: u32, count: u32, unbind_num_trailing_slots: u32, images: Option<&[ImageView<'_, <Self::Screen as Gallium>::Resource>]>);

    /// Mesa's `pipe_context.set_stencil_ref`, called first at gallium/auxiliary/cso_cache/cso_context.c:512.
    fn set_stencil_ref(&self, ref_: StencilRef);

    /// Mesa's `pipe_context.set_stream_output_targets`, called first at gallium/auxiliary/cso_cache/cso_context.c:535.
    fn set_stream_output_targets(&self, targets: &[Option<&Arc<<Self::Screen as Gallium>::StreamOutputTarget>>], offsets: &[u32], output_prim: values::MesaPrim);

    /// Mesa's `pipe_context.set_vertex_buffers`, called first at gallium/auxiliary/cso_cache/cso_context.c:532.
    fn set_vertex_buffers(&self, arg2: &[VertexBuffer<'_, <Self::Screen as Gallium>::Resource>]);

    /// Mesa's `pipe_context.set_viewport_states`, called first at mesa/state_tracker/st_atom_viewport.c:81.
    fn set_viewport_states(&self, start_slot: u32, arg3: &[ViewportState]);

    /// Mesa's `pipe_context.texture_barrier`, called first at mesa/main/barrier.c:118.
    fn texture_barrier(&self, flags: u32);

    /// Mesa's `pipe_context.texture_subdata`, called first at mesa/state_tracker/st_texcompress_compute.c:534.
    #[expect(clippy::too_many_arguments, reason = "the pointer's parameters, as Mesa's header declares them")]
    fn texture_subdata(&self, arg1: &Arc<<Self::Screen as Gallium>::Resource>, level: u32, usage: values::MapFlags, arg4: &PipeBox, data: &[u8], stride: u32, layer_stride: u64);

    /// Mesa's `u_transfer_vtbl.transfer_map`, called first at gallium/auxiliary/util/u_transfer_helper.c:376.
    fn helper_transfer_map(&self, prsc: &Arc<<Self::Screen as Gallium>::Resource>, level: u32, usage: values::MapFlags, box_: &PipeBox) -> Option<Mapped<<Self::Screen as Gallium>::Mapping>>;

    /// Mesa's `u_transfer_vtbl.transfer_unmap`, called first at gallium/auxiliary/util/u_transfer_helper.c:519.
    fn helper_transfer_unmap(&self, ptrans: <Self::Screen as Gallium>::Mapping, written: &[u8]);

    /// Mesa's `u_transfer_vtbl.transfer_flush_region`, called first at gallium/auxiliary/util/u_inlines.h:589.
    fn helper_transfer_flush_region(&self, ptrans: &<Self::Screen as Gallium>::Mapping, written: &[u8], box_: &PipeBox);

}
