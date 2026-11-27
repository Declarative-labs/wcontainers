// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! Mesa's structures as the glue's C copies them (flat.h): mirrors with C's layout (no bit-field, no union, each
//! object its key), the keys and client memory each names, and the trait's owned structure made from each.

use wcontainers_gallium::Gallium;

use crate::FieldLayout;
use crate::held::{Held, Keys, ObjectKind, each, flag, within};

/// Mesa's `pipe_blend_color` as the glue's C copies it (`struct wc_flat_pipe_blend_color` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlendColor {
    pub color: [f32; 4],
}

impl FlatBlendColor {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::BlendColor> {
        Some(wcontainers_gallium::BlendColor {
            color: self.color,
        })
    }
}

/// Mesa's `pipe_rt_blend_state` as the glue's C copies it (`struct wc_flat_pipe_rt_blend_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatRtBlendState {
    pub blend_enable: u32,
    pub rgb_func: u32,
    pub rgb_src_factor: u32,
    pub rgb_dst_factor: u32,
    pub alpha_func: u32,
    pub alpha_src_factor: u32,
    pub alpha_dst_factor: u32,
    pub colormask: u32,
}

impl FlatRtBlendState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::RtBlendState> {
        Some(wcontainers_gallium::RtBlendState {
            blend_enable: within(self.blend_enable, 1)?,
            rgb_func: within(self.rgb_func, 3)?,
            rgb_src_factor: wcontainers_gallium::values::Blendfactor(within(self.rgb_src_factor, 5)?),
            rgb_dst_factor: wcontainers_gallium::values::Blendfactor(within(self.rgb_dst_factor, 5)?),
            alpha_func: within(self.alpha_func, 3)?,
            alpha_src_factor: wcontainers_gallium::values::Blendfactor(within(self.alpha_src_factor, 5)?),
            alpha_dst_factor: wcontainers_gallium::values::Blendfactor(within(self.alpha_dst_factor, 5)?),
            colormask: within(self.colormask, 4)?,
        })
    }
}

/// Mesa's `pipe_blend_state` as the glue's C copies it (`struct wc_flat_pipe_blend_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlendState {
    pub independent_blend_enable: u32,
    pub logicop_enable: u32,
    pub logicop_func: u32,
    pub dither: u32,
    pub alpha_to_coverage: u32,
    pub alpha_to_coverage_dither: u32,
    pub alpha_to_one: u32,
    pub max_rt: u32,
    pub advanced_blend_func: u32,
    pub blend_coherent: u32,
    pub is_yuv: u32,
    pub rt: [FlatRtBlendState; 8],
}

impl FlatBlendState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::BlendState> {
        Some(wcontainers_gallium::BlendState {
            independent_blend_enable: within(self.independent_blend_enable, 1)?,
            logicop_enable: within(self.logicop_enable, 1)?,
            logicop_func: wcontainers_gallium::values::Logicop(within(self.logicop_func, 4)?),
            dither: within(self.dither, 1)?,
            alpha_to_coverage: within(self.alpha_to_coverage, 1)?,
            alpha_to_coverage_dither: within(self.alpha_to_coverage_dither, 1)?,
            alpha_to_one: within(self.alpha_to_one, 1)?,
            max_rt: within(self.max_rt, 3)?,
            advanced_blend_func: within(self.advanced_blend_func, 4)?,
            blend_coherent: within(self.blend_coherent, 1)?,
            is_yuv: within(self.is_yuv, 1)?,
            rt: each(&self.rt, |element0| element0.owned(held))?,
        })
    }
}

/// Mesa's `pipe_box` as the glue's C copies it (`struct wc_flat_pipe_box` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatPipeBox {
    pub x: i32,
    pub width: i32,
    pub y: i32,
    pub height: i32,
    pub z: i16,
    pub depth: i16,
}

impl FlatPipeBox {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::PipeBox> {
        Some(wcontainers_gallium::PipeBox {
            x: self.x,
            width: self.width,
            y: self.y,
            height: self.height,
            z: self.z,
            depth: self.depth,
        })
    }
}

/// Mesa's `pipe_blit_info.dst` as the glue's C copies it (`struct wc_flat_pipe_blit_info__dst` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlitInfoDst {
    pub resource: usize,
    pub level: u32,
    pub box_: FlatPipeBox,
    pub format: u32,
}

impl FlatBlitInfoDst {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::BlitInfoDst<'a, G::Resource>> {
        Some(wcontainers_gallium::BlitInfoDst {
            resource: held.resource(self.resource)?,
            level: self.level,
            box_: self.box_.owned(held)?,
            format: wcontainers_gallium::values::Format(self.format),
        })
    }
}

/// Mesa's `pipe_blit_info.src` as the glue's C copies it (`struct wc_flat_pipe_blit_info__src` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlitInfoSrc {
    pub resource: usize,
    pub level: u32,
    pub box_: FlatPipeBox,
    pub format: u32,
}

impl FlatBlitInfoSrc {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::BlitInfoSrc<'a, G::Resource>> {
        Some(wcontainers_gallium::BlitInfoSrc {
            resource: held.resource(self.resource)?,
            level: self.level,
            box_: self.box_.owned(held)?,
            format: wcontainers_gallium::values::Format(self.format),
        })
    }
}

/// Mesa's `pipe_scissor_state` as the glue's C copies it (`struct wc_flat_pipe_scissor_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatScissorState {
    pub minx: u32,
    pub miny: u32,
    pub maxx: u32,
    pub maxy: u32,
}

impl FlatScissorState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ScissorState> {
        Some(wcontainers_gallium::ScissorState {
            minx: self.minx,
            miny: self.miny,
            maxx: self.maxx,
            maxy: self.maxy,
        })
    }
}

/// Mesa's `pipe_blit_info` as the glue's C copies it (`struct wc_flat_pipe_blit_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlitInfo {
    pub dst: FlatBlitInfoDst,
    pub src: FlatBlitInfoSrc,
    pub mask: u32,
    pub filter: u32,
    pub dst_sample: u8,
    pub sample0_only: u8,
    pub scissor_enable: u8,
    pub scissor: FlatScissorState,
    pub swizzle_enable: u8,
    pub swizzle: [u8; 4],
    pub window_rectangle_include: u8,
    pub num_window_rectangles: u32,
    pub window_rectangles: [FlatScissorState; 8],
    pub render_condition_enable: u8,
    pub alpha_blend: u8,
}

impl FlatBlitInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        self.dst.keys(keys);
        self.src.keys(keys);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::BlitInfo<'a, G::Resource>> {
        Some(wcontainers_gallium::BlitInfo {
            dst: self.dst.owned(held)?,
            src: self.src.owned(held)?,
            mask: self.mask,
            filter: wcontainers_gallium::values::TexFilter(self.filter),
            dst_sample: self.dst_sample,
            sample0_only: flag(self.sample0_only)?,
            scissor_enable: flag(self.scissor_enable)?,
            scissor: self.scissor.owned(held)?,
            swizzle_enable: flag(self.swizzle_enable)?,
            swizzle: self.swizzle,
            window_rectangle_include: flag(self.window_rectangle_include)?,
            num_window_rectangles: self.num_window_rectangles,
            window_rectangles: each(&self.window_rectangles, |element0| element0.owned(held))?,
            render_condition_enable: flag(self.render_condition_enable)?,
            alpha_blend: flag(self.alpha_blend)?,
        })
    }
}

/// Mesa's `pipe_mesh_caps` as the glue's C copies it (`struct wc_flat_pipe_mesh_caps` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatMeshCaps {
    pub max_task_work_group_total_count: u32,
    pub max_mesh_work_group_total_count: u32,
    pub max_mesh_work_group_invocations: u32,
    pub max_task_work_group_invocations: u32,
    pub max_task_payload_size: u32,
    pub max_task_shared_memory_size: u32,
    pub max_mesh_shared_memory_size: u32,
    pub max_task_payload_and_shared_memory_size: u32,
    pub max_mesh_payload_and_shared_memory_size: u32,
    pub max_mesh_output_memory_size: u32,
    pub max_mesh_payload_and_output_memory_size: u32,
    pub max_mesh_output_vertices: u32,
    pub max_mesh_output_primitives: u32,
    pub max_mesh_output_components: u32,
    pub max_mesh_output_layers: u32,
    pub max_mesh_multiview_view_count: u32,
    pub mesh_output_per_vertex_granularity: u32,
    pub mesh_output_per_primitive_granularity: u32,
    pub max_preferred_task_work_group_invocations: u32,
    pub max_preferred_mesh_work_group_invocations: u32,
    pub mesh_prefers_local_invocation_vertex_output: u8,
    pub mesh_prefers_local_invocation_primitive_output: u8,
    pub mesh_prefers_compact_vertex_output: u8,
    pub mesh_prefers_compact_primitive_output: u8,
    pub max_task_work_group_count: [u32; 3],
    pub max_mesh_work_group_count: [u32; 3],
    pub max_task_work_group_size: [u32; 3],
    pub max_mesh_work_group_size: [u32; 3],
    pub pipeline_statistic_queries: u8,
}

impl FlatMeshCaps {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::MeshCaps> {
        Some(wcontainers_gallium::MeshCaps {
            max_task_work_group_total_count: self.max_task_work_group_total_count,
            max_mesh_work_group_total_count: self.max_mesh_work_group_total_count,
            max_mesh_work_group_invocations: self.max_mesh_work_group_invocations,
            max_task_work_group_invocations: self.max_task_work_group_invocations,
            max_task_payload_size: self.max_task_payload_size,
            max_task_shared_memory_size: self.max_task_shared_memory_size,
            max_mesh_shared_memory_size: self.max_mesh_shared_memory_size,
            max_task_payload_and_shared_memory_size: self.max_task_payload_and_shared_memory_size,
            max_mesh_payload_and_shared_memory_size: self.max_mesh_payload_and_shared_memory_size,
            max_mesh_output_memory_size: self.max_mesh_output_memory_size,
            max_mesh_payload_and_output_memory_size: self.max_mesh_payload_and_output_memory_size,
            max_mesh_output_vertices: self.max_mesh_output_vertices,
            max_mesh_output_primitives: self.max_mesh_output_primitives,
            max_mesh_output_components: self.max_mesh_output_components,
            max_mesh_output_layers: self.max_mesh_output_layers,
            max_mesh_multiview_view_count: self.max_mesh_multiview_view_count,
            mesh_output_per_vertex_granularity: self.mesh_output_per_vertex_granularity,
            mesh_output_per_primitive_granularity: self.mesh_output_per_primitive_granularity,
            max_preferred_task_work_group_invocations: self.max_preferred_task_work_group_invocations,
            max_preferred_mesh_work_group_invocations: self.max_preferred_mesh_work_group_invocations,
            mesh_prefers_local_invocation_vertex_output: flag(self.mesh_prefers_local_invocation_vertex_output)?,
            mesh_prefers_local_invocation_primitive_output: flag(self.mesh_prefers_local_invocation_primitive_output)?,
            mesh_prefers_compact_vertex_output: flag(self.mesh_prefers_compact_vertex_output)?,
            mesh_prefers_compact_primitive_output: flag(self.mesh_prefers_compact_primitive_output)?,
            max_task_work_group_count: self.max_task_work_group_count,
            max_mesh_work_group_count: self.max_mesh_work_group_count,
            max_task_work_group_size: self.max_task_work_group_size,
            max_mesh_work_group_size: self.max_mesh_work_group_size,
            pipeline_statistic_queries: flag(self.pipeline_statistic_queries)?,
        })
    }
}

/// Mesa's `pipe_caps` as the glue's C copies it (`struct wc_flat_pipe_caps` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatCaps {
    pub graphics: u8,
    pub npot_textures: u8,
    pub anisotropic_filter: u8,
    pub occlusion_query: u8,
    pub query_time_elapsed: u8,
    pub texture_shadow_map: u8,
    pub texture_swizzle: u8,
    pub texture_mirror_clamp: u8,
    pub blend_equation_separate: u8,
    pub primitive_restart: u8,
    pub primitive_restart_fixed_index: u8,
    pub indep_blend_enable: u8,
    pub indep_blend_func: u8,
    pub fs_coord_origin_upper_left: u8,
    pub fs_coord_origin_lower_left: u8,
    pub fs_coord_pixel_center_half_integer: u8,
    pub fs_coord_pixel_center_integer: u8,
    pub depth_clip_disable: u8,
    pub depth_clip_disable_separate: u8,
    pub depth_clamp_enable: u8,
    pub shader_stencil_export: u8,
    pub vs_instanceid: u8,
    pub vertex_element_instance_divisor: u8,
    pub fragment_color_clamped: u8,
    pub mixed_colorbuffer_formats: u8,
    pub seamless_cube_map: u8,
    pub seamless_cube_map_per_texture: u8,
    pub conditional_render: u8,
    pub texture_barrier: u8,
    pub stream_output_pause_resume: u8,
    pub tgsi_can_compact_constants: u8,
    pub vertex_color_unclamped: u8,
    pub vertex_color_clamped: u8,
    pub quads_follow_provoking_vertex_convention: u8,
    pub user_vertex_buffers: u8,
    pub compute: u8,
    pub start_instance: u8,
    pub query_timestamp: u8,
    pub texture_multisample: u8,
    pub cube_map_array: u8,
    pub texture_buffer_objects: u8,
    pub buffer_sampler_view_rgba_only: u8,
    pub tgsi_texcoord: u8,
    pub query_pipeline_statistics: u8,
    pub mixed_framebuffer_sizes: u8,
    pub vs_layer_viewport: u8,
    pub texture_gather_sm5: u8,
    pub buffer_map_persistent_coherent: u8,
    pub fake_sw_msaa: u8,
    pub texture_query_lod: u8,
    pub sample_shading: u8,
    pub texture_gather_offsets: u8,
    pub vs_window_space_position: u8,
    pub draw_indirect: u8,
    pub fs_fine_derivative: u8,
    pub uma: u8,
    pub conditional_render_inverted: u8,
    pub sampler_view_target: u8,
    pub clip_halfz: u8,
    pub polygon_offset_clamp: u8,
    pub multisample_z_resolve: u8,
    pub resource_from_user_memory: u8,
    pub resource_from_user_memory_compute_only: u8,
    pub device_reset_status_query: u8,
    pub texture_float_linear: u8,
    pub texture_half_float_linear: u8,
    pub depth_bounds_test: u8,
    pub native_fp32_depth: u8,
    pub texture_query_samples: u8,
    pub force_persample_interp: u8,
    pub shareable_shaders: u8,
    pub copy_between_compressed_and_plain_formats: u8,
    pub clear_scissored: u8,
    pub clear_masked: u8,
    pub draw_parameters: u8,
    pub shader_pack_half_float: u8,
    pub multi_draw_indirect: u8,
    pub multi_draw_indirect_params: u8,
    pub multi_draw_indirect_partial_stride: u8,
    pub fs_position_is_sysval: u8,
    pub fs_point_is_sysval: u8,
    pub fs_face_is_integer_sysval: u8,
    pub invalidate_buffer: u8,
    pub generate_mipmap: u8,
    pub string_marker: u8,
    pub surface_no_compress: u8,
    pub surface_reinterpret_blocks: u8,
    pub compressed_surface_reinterpret_blocks_layered: u8,
    pub query_buffer_object: u8,
    pub query_memory_info: u8,
    pub framebuffer_no_attachment: u8,
    pub robust_buffer_access_behavior: u8,
    pub cull_distance: u8,
    pub shader_group_vote: u8,
    pub shader_array_components: u8,
    pub stream_output_interleave_buffers: u8,
    pub native_fence_fd: u8,
    pub glsl_tess_levels_as_inputs: u8,
    pub legacy_math_rules: u8,
    pub fp16: u8,
    pub doubles: u8,
    pub int64: u8,
    pub shader_clock: u8,
    pub shader_realtime_clock: u8,
    pub polygon_mode_fill_rectangle: u8,
    pub shader_ballot: u8,
    pub tes_layer_viewport: u8,
    pub can_bind_const_buffer_as_vertex: u8,
    pub allow_mapped_buffers_during_execution: u8,
    pub post_depth_coverage: u8,
    pub bindless_texture: u8,
    pub nir_samplers_as_deref: u8,
    pub query_so_overflow: u8,
    pub memobj: u8,
    pub load_constbuf: u8,
    pub tile_raster_order: u8,
    pub signed_vertex_buffer_offset: u8,
    pub fence_signal: u8,
    pub packed_uniforms: u8,
    pub conservative_raster_post_snap_triangles: u8,
    pub conservative_raster_post_snap_points_lines: u8,
    pub conservative_raster_pre_snap_triangles: u8,
    pub conservative_raster_pre_snap_points_lines: u8,
    pub conservative_raster_post_depth_coverage: u8,
    pub conservative_raster_inner_coverage: u8,
    pub programmable_sample_locations: u8,
    pub texture_mirror_clamp_to_edge: u8,
    pub surface_sample_count: u8,
    pub image_atomic_float_add: u8,
    pub query_pipeline_statistics_single: u8,
    pub dest_surface_srgb_control: u8,
    pub compute_grid_info_last_block: u8,
    pub compute_shader_derivatives: u8,
    pub image_load_formatted: u8,
    pub image_store_formatted: u8,
    pub throttle: u8,
    pub cl_gl_sharing: u8,
    pub prefer_compute_for_multimedia: u8,
    pub fragment_shader_interlock: u8,
    pub fbfetch_coherent: u8,
    pub atomic_float_minmax: u8,
    pub fragment_shader_texture_lod: u8,
    pub fragment_shader_derivatives: u8,
    pub fragment_shader_depth: u8,
    pub texture_shadow_lod: u8,
    pub shader_samples_identical: u8,
    pub image_atomic_inc_wrap: u8,
    pub prefer_imm_arrays_as_constbuf: u8,
    pub gl_spirv: u8,
    pub gl_spirv_variable_pointers: u8,
    pub demote_to_helper_invocation: u8,
    pub tgsi_tg4_component_in_swizzle: u8,
    pub flatshade: u8,
    pub alpha_test: u8,
    pub two_sided_color: u8,
    pub opencl_integer_functions: u8,
    pub integer_multiply_32x16: u8,
    pub frontend_noop: u8,
    pub nir_images_as_deref: u8,
    pub packed_stream_output: u8,
    pub viewport_transform_lowered: u8,
    pub psiz_clamped: u8,
    pub viewport_swizzle: u8,
    pub system_svm: u8,
    pub viewport_mask: u8,
    pub alpha_to_coverage_dither_control: u8,
    pub map_unsynchronized_thread_safe: u8,
    pub nir_atomics_as_deref: u8,
    pub no_clip_on_copy_tex: u8,
    pub shader_atomic_int64: u8,
    pub device_protected_surface: u8,
    pub prefer_real_buffer_in_constbuf0: u8,
    pub gl_clamp: u8,
    pub texrect: u8,
    pub sampler_reduction_minmax: u8,
    pub sampler_reduction_minmax_arb: u8,
    pub allow_dynamic_vao_fastpath: u8,
    pub emulate_nonfixed_primitive_restart: u8,
    pub prefer_back_buffer_reuse: u8,
    pub draw_vertex_state: u8,
    pub prefer_pot_aligned_varyings: u8,
    pub sparse_texture_full_array_cube_mipmaps: u8,
    pub query_sparse_texture_residency: u8,
    pub clamp_sparse_texture_lod: u8,
    pub allow_draw_out_of_order: u8,
    pub hardware_gl_select: u8,
    pub dithering: u8,
    pub fbfetch_zs: u8,
    pub timeline_semaphore_import: u8,
    pub device_protected_context: u8,
    pub allow_glthread_buffer_subdata_opt: u8,
    pub null_textures: u8,
    pub astc_void_extents_need_denorm_flush: u8,
    pub validate_all_dirty_states: u8,
    pub has_const_bw: u8,
    pub performance_monitor: u8,
    pub texture_sampler_independent: u8,
    pub astc_decode_mode: u8,
    pub shader_subgroup_quad_all_stages: u8,
    pub call_finalize_nir_in_linker: u8,
    pub mesh_shader: u8,
    pub representative_fragment_test: u8,
    pub prefer_persp: u8,
    pub blit_3d: u8,
    pub glsl_bindless_handles_are_32bit: u8,
    pub polygon_stipple: u8,
    pub disable_custom_border_color_caching: u8,
    pub needs_split_planar_samplers: u8,
    pub accelerated: i32,
    pub min_texel_offset: i32,
    pub max_texel_offset: i32,
    pub min_texture_gather_offset: i32,
    pub max_texture_gather_offset: i32,
    pub max_dual_source_render_targets: u32,
    pub max_render_targets: u32,
    pub max_texture_2d_size: u32,
    pub max_texture_3d_levels: u32,
    pub max_texture_cube_levels: u32,
    pub max_stream_output_buffers: u32,
    pub max_texture_array_layers: u32,
    pub max_stream_output_separate_components: u32,
    pub max_stream_output_interleaved_components: u32,
    pub glsl_feature_level: u32,
    pub glsl_feature_level_compatibility: u32,
    pub essl_feature_level: u32,
    pub constant_buffer_offset_alignment: u32,
    pub timer_resolution: u32,
    pub min_map_buffer_alignment: u32,
    pub texture_buffer_offset_alignment: u32,
    pub linear_image_pitch_alignment: u32,
    pub linear_image_base_address_alignment: u32,
    pub texture_transfer_modes: u32,
    pub texture_border_color_quirk: u32,
    pub max_texel_buffer_elements: u32,
    pub max_viewports: u32,
    pub max_geometry_output_vertices: u32,
    pub max_geometry_total_output_components: u32,
    pub max_texture_gather_components: u32,
    pub max_vertex_streams: u32,
    pub vendor_id: u32,
    pub device_id: u32,
    pub video_memory: u32,
    pub max_vertex_attrib_stride: u32,
    pub max_shader_patch_varyings: u32,
    pub shader_buffer_offset_alignment: u32,
    pub pci_group: u32,
    pub pci_bus: u32,
    pub pci_device: u32,
    pub pci_function: u32,
    pub max_window_rectangles: u32,
    pub viewport_subpixel_bits: u32,
    pub rasterizer_subpixel_bits: u32,
    pub mixed_color_depth_bits: u32,
    pub fbfetch: u32,
    pub blend_equation_advanced: u32,
    pub sparse_buffer_page_size: u32,
    pub max_combined_shader_output_resources: u32,
    pub framebuffer_msaa_constraints: u32,
    pub context_priority_mask: u32,
    pub constbuf0_flags: u32,
    pub max_conservative_raster_subpixel_precision_bias: u32,
    pub max_gs_invocations: u32,
    pub max_shader_buffer_size: u32,
    pub max_combined_shader_buffers: u32,
    pub max_combined_image_uniforms: u32,
    pub max_combined_hw_atomic_counters: u32,
    pub max_combined_hw_atomic_counter_buffers: u32,
    pub max_texture_upload_memory_budget: u32,
    pub max_vertex_element_src_offset: u32,
    pub max_varyings: u32,
    pub ignored_output_varyings: [u64; 8],
    pub dmabuf: u32,
    pub clip_planes: u32,
    pub max_vertex_buffers: u32,
    pub gl_begin_end_buffer_size: u32,
    pub glsl_zero_init: u32,
    pub max_texture_mb: u32,
    pub supported_prim_modes: u32,
    pub supported_prim_modes_with_restart: u32,
    pub max_sparse_texture_size: u32,
    pub max_sparse_3d_texture_size: u32,
    pub max_sparse_array_texture_layers: u32,
    pub max_constant_buffer_size: u32,
    pub query_timestamp_bits: u32,
    pub shader_subgroup_size: u32,
    pub shader_subgroup_supported_stages: u32,
    pub shader_subgroup_supported_features: u32,
    pub shader_pixel_local_storage_size: u32,
    pub shader_pixel_local_storage_fast_size: u32,
    pub multiview: u32,
    pub max_label_length: u32,
    pub max_timeline_semaphore_difference: u64,
    pub map_client_pointer: u32,
    pub min_vma: u64,
    pub max_vma: u64,
    pub hw_clear_buffer_sizes: u16,
    pub device_type: u32,
    pub vertex_input_alignment: u32,
    pub endianness: u32,
    pub point_size_fixed: u32,
    pub min_line_width: f32,
    pub min_line_width_aa: f32,
    pub max_line_width: f32,
    pub max_line_width_aa: f32,
    pub line_width_granularity: f32,
    pub min_point_size: f32,
    pub min_point_size_aa: f32,
    pub max_point_size: f32,
    pub max_point_size_aa: f32,
    pub point_size_granularity: f32,
    pub max_texture_anisotropy: f32,
    pub max_texture_lod_bias: f32,
    pub min_conservative_raster_dilate: f32,
    pub max_conservative_raster_dilate: f32,
    pub conservative_raster_dilate_granularity: f32,
    pub mesh: FlatMeshCaps,
}

impl FlatCaps {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::Caps> {
        Some(wcontainers_gallium::Caps {
            graphics: flag(self.graphics)?,
            npot_textures: flag(self.npot_textures)?,
            anisotropic_filter: flag(self.anisotropic_filter)?,
            occlusion_query: flag(self.occlusion_query)?,
            query_time_elapsed: flag(self.query_time_elapsed)?,
            texture_shadow_map: flag(self.texture_shadow_map)?,
            texture_swizzle: flag(self.texture_swizzle)?,
            texture_mirror_clamp: flag(self.texture_mirror_clamp)?,
            blend_equation_separate: flag(self.blend_equation_separate)?,
            primitive_restart: flag(self.primitive_restart)?,
            primitive_restart_fixed_index: flag(self.primitive_restart_fixed_index)?,
            indep_blend_enable: flag(self.indep_blend_enable)?,
            indep_blend_func: flag(self.indep_blend_func)?,
            fs_coord_origin_upper_left: flag(self.fs_coord_origin_upper_left)?,
            fs_coord_origin_lower_left: flag(self.fs_coord_origin_lower_left)?,
            fs_coord_pixel_center_half_integer: flag(self.fs_coord_pixel_center_half_integer)?,
            fs_coord_pixel_center_integer: flag(self.fs_coord_pixel_center_integer)?,
            depth_clip_disable: flag(self.depth_clip_disable)?,
            depth_clip_disable_separate: flag(self.depth_clip_disable_separate)?,
            depth_clamp_enable: flag(self.depth_clamp_enable)?,
            shader_stencil_export: flag(self.shader_stencil_export)?,
            vs_instanceid: flag(self.vs_instanceid)?,
            vertex_element_instance_divisor: flag(self.vertex_element_instance_divisor)?,
            fragment_color_clamped: flag(self.fragment_color_clamped)?,
            mixed_colorbuffer_formats: flag(self.mixed_colorbuffer_formats)?,
            seamless_cube_map: flag(self.seamless_cube_map)?,
            seamless_cube_map_per_texture: flag(self.seamless_cube_map_per_texture)?,
            conditional_render: flag(self.conditional_render)?,
            texture_barrier: flag(self.texture_barrier)?,
            stream_output_pause_resume: flag(self.stream_output_pause_resume)?,
            tgsi_can_compact_constants: flag(self.tgsi_can_compact_constants)?,
            vertex_color_unclamped: flag(self.vertex_color_unclamped)?,
            vertex_color_clamped: flag(self.vertex_color_clamped)?,
            quads_follow_provoking_vertex_convention: flag(self.quads_follow_provoking_vertex_convention)?,
            user_vertex_buffers: flag(self.user_vertex_buffers)?,
            compute: flag(self.compute)?,
            start_instance: flag(self.start_instance)?,
            query_timestamp: flag(self.query_timestamp)?,
            texture_multisample: flag(self.texture_multisample)?,
            cube_map_array: flag(self.cube_map_array)?,
            texture_buffer_objects: flag(self.texture_buffer_objects)?,
            buffer_sampler_view_rgba_only: flag(self.buffer_sampler_view_rgba_only)?,
            tgsi_texcoord: flag(self.tgsi_texcoord)?,
            query_pipeline_statistics: flag(self.query_pipeline_statistics)?,
            mixed_framebuffer_sizes: flag(self.mixed_framebuffer_sizes)?,
            vs_layer_viewport: flag(self.vs_layer_viewport)?,
            texture_gather_sm5: flag(self.texture_gather_sm5)?,
            buffer_map_persistent_coherent: flag(self.buffer_map_persistent_coherent)?,
            fake_sw_msaa: flag(self.fake_sw_msaa)?,
            texture_query_lod: flag(self.texture_query_lod)?,
            sample_shading: flag(self.sample_shading)?,
            texture_gather_offsets: flag(self.texture_gather_offsets)?,
            vs_window_space_position: flag(self.vs_window_space_position)?,
            draw_indirect: flag(self.draw_indirect)?,
            fs_fine_derivative: flag(self.fs_fine_derivative)?,
            uma: flag(self.uma)?,
            conditional_render_inverted: flag(self.conditional_render_inverted)?,
            sampler_view_target: flag(self.sampler_view_target)?,
            clip_halfz: flag(self.clip_halfz)?,
            polygon_offset_clamp: flag(self.polygon_offset_clamp)?,
            multisample_z_resolve: flag(self.multisample_z_resolve)?,
            resource_from_user_memory: flag(self.resource_from_user_memory)?,
            resource_from_user_memory_compute_only: flag(self.resource_from_user_memory_compute_only)?,
            device_reset_status_query: flag(self.device_reset_status_query)?,
            texture_float_linear: flag(self.texture_float_linear)?,
            texture_half_float_linear: flag(self.texture_half_float_linear)?,
            depth_bounds_test: flag(self.depth_bounds_test)?,
            native_fp32_depth: flag(self.native_fp32_depth)?,
            texture_query_samples: flag(self.texture_query_samples)?,
            force_persample_interp: flag(self.force_persample_interp)?,
            shareable_shaders: flag(self.shareable_shaders)?,
            copy_between_compressed_and_plain_formats: flag(self.copy_between_compressed_and_plain_formats)?,
            clear_scissored: flag(self.clear_scissored)?,
            clear_masked: flag(self.clear_masked)?,
            draw_parameters: flag(self.draw_parameters)?,
            shader_pack_half_float: flag(self.shader_pack_half_float)?,
            multi_draw_indirect: flag(self.multi_draw_indirect)?,
            multi_draw_indirect_params: flag(self.multi_draw_indirect_params)?,
            multi_draw_indirect_partial_stride: flag(self.multi_draw_indirect_partial_stride)?,
            fs_position_is_sysval: flag(self.fs_position_is_sysval)?,
            fs_point_is_sysval: flag(self.fs_point_is_sysval)?,
            fs_face_is_integer_sysval: flag(self.fs_face_is_integer_sysval)?,
            invalidate_buffer: flag(self.invalidate_buffer)?,
            generate_mipmap: flag(self.generate_mipmap)?,
            string_marker: flag(self.string_marker)?,
            surface_no_compress: flag(self.surface_no_compress)?,
            surface_reinterpret_blocks: flag(self.surface_reinterpret_blocks)?,
            compressed_surface_reinterpret_blocks_layered: flag(self.compressed_surface_reinterpret_blocks_layered)?,
            query_buffer_object: flag(self.query_buffer_object)?,
            query_memory_info: flag(self.query_memory_info)?,
            framebuffer_no_attachment: flag(self.framebuffer_no_attachment)?,
            robust_buffer_access_behavior: flag(self.robust_buffer_access_behavior)?,
            cull_distance: flag(self.cull_distance)?,
            shader_group_vote: flag(self.shader_group_vote)?,
            shader_array_components: flag(self.shader_array_components)?,
            stream_output_interleave_buffers: flag(self.stream_output_interleave_buffers)?,
            native_fence_fd: flag(self.native_fence_fd)?,
            glsl_tess_levels_as_inputs: flag(self.glsl_tess_levels_as_inputs)?,
            legacy_math_rules: flag(self.legacy_math_rules)?,
            fp16: flag(self.fp16)?,
            doubles: flag(self.doubles)?,
            int64: flag(self.int64)?,
            shader_clock: flag(self.shader_clock)?,
            shader_realtime_clock: flag(self.shader_realtime_clock)?,
            polygon_mode_fill_rectangle: flag(self.polygon_mode_fill_rectangle)?,
            shader_ballot: flag(self.shader_ballot)?,
            tes_layer_viewport: flag(self.tes_layer_viewport)?,
            can_bind_const_buffer_as_vertex: flag(self.can_bind_const_buffer_as_vertex)?,
            allow_mapped_buffers_during_execution: flag(self.allow_mapped_buffers_during_execution)?,
            post_depth_coverage: flag(self.post_depth_coverage)?,
            bindless_texture: flag(self.bindless_texture)?,
            nir_samplers_as_deref: flag(self.nir_samplers_as_deref)?,
            query_so_overflow: flag(self.query_so_overflow)?,
            memobj: flag(self.memobj)?,
            load_constbuf: flag(self.load_constbuf)?,
            tile_raster_order: flag(self.tile_raster_order)?,
            signed_vertex_buffer_offset: flag(self.signed_vertex_buffer_offset)?,
            fence_signal: flag(self.fence_signal)?,
            packed_uniforms: flag(self.packed_uniforms)?,
            conservative_raster_post_snap_triangles: flag(self.conservative_raster_post_snap_triangles)?,
            conservative_raster_post_snap_points_lines: flag(self.conservative_raster_post_snap_points_lines)?,
            conservative_raster_pre_snap_triangles: flag(self.conservative_raster_pre_snap_triangles)?,
            conservative_raster_pre_snap_points_lines: flag(self.conservative_raster_pre_snap_points_lines)?,
            conservative_raster_post_depth_coverage: flag(self.conservative_raster_post_depth_coverage)?,
            conservative_raster_inner_coverage: flag(self.conservative_raster_inner_coverage)?,
            programmable_sample_locations: flag(self.programmable_sample_locations)?,
            texture_mirror_clamp_to_edge: flag(self.texture_mirror_clamp_to_edge)?,
            surface_sample_count: flag(self.surface_sample_count)?,
            image_atomic_float_add: flag(self.image_atomic_float_add)?,
            query_pipeline_statistics_single: flag(self.query_pipeline_statistics_single)?,
            dest_surface_srgb_control: flag(self.dest_surface_srgb_control)?,
            compute_grid_info_last_block: flag(self.compute_grid_info_last_block)?,
            compute_shader_derivatives: flag(self.compute_shader_derivatives)?,
            image_load_formatted: flag(self.image_load_formatted)?,
            image_store_formatted: flag(self.image_store_formatted)?,
            throttle: flag(self.throttle)?,
            cl_gl_sharing: flag(self.cl_gl_sharing)?,
            prefer_compute_for_multimedia: flag(self.prefer_compute_for_multimedia)?,
            fragment_shader_interlock: flag(self.fragment_shader_interlock)?,
            fbfetch_coherent: flag(self.fbfetch_coherent)?,
            atomic_float_minmax: flag(self.atomic_float_minmax)?,
            fragment_shader_texture_lod: flag(self.fragment_shader_texture_lod)?,
            fragment_shader_derivatives: flag(self.fragment_shader_derivatives)?,
            fragment_shader_depth: flag(self.fragment_shader_depth)?,
            texture_shadow_lod: flag(self.texture_shadow_lod)?,
            shader_samples_identical: flag(self.shader_samples_identical)?,
            image_atomic_inc_wrap: flag(self.image_atomic_inc_wrap)?,
            prefer_imm_arrays_as_constbuf: flag(self.prefer_imm_arrays_as_constbuf)?,
            gl_spirv: flag(self.gl_spirv)?,
            gl_spirv_variable_pointers: flag(self.gl_spirv_variable_pointers)?,
            demote_to_helper_invocation: flag(self.demote_to_helper_invocation)?,
            tgsi_tg4_component_in_swizzle: flag(self.tgsi_tg4_component_in_swizzle)?,
            flatshade: flag(self.flatshade)?,
            alpha_test: flag(self.alpha_test)?,
            two_sided_color: flag(self.two_sided_color)?,
            opencl_integer_functions: flag(self.opencl_integer_functions)?,
            integer_multiply_32x16: flag(self.integer_multiply_32x16)?,
            frontend_noop: flag(self.frontend_noop)?,
            nir_images_as_deref: flag(self.nir_images_as_deref)?,
            packed_stream_output: flag(self.packed_stream_output)?,
            viewport_transform_lowered: flag(self.viewport_transform_lowered)?,
            psiz_clamped: flag(self.psiz_clamped)?,
            viewport_swizzle: flag(self.viewport_swizzle)?,
            system_svm: flag(self.system_svm)?,
            viewport_mask: flag(self.viewport_mask)?,
            alpha_to_coverage_dither_control: flag(self.alpha_to_coverage_dither_control)?,
            map_unsynchronized_thread_safe: flag(self.map_unsynchronized_thread_safe)?,
            nir_atomics_as_deref: flag(self.nir_atomics_as_deref)?,
            no_clip_on_copy_tex: flag(self.no_clip_on_copy_tex)?,
            shader_atomic_int64: flag(self.shader_atomic_int64)?,
            device_protected_surface: flag(self.device_protected_surface)?,
            prefer_real_buffer_in_constbuf0: flag(self.prefer_real_buffer_in_constbuf0)?,
            gl_clamp: flag(self.gl_clamp)?,
            texrect: flag(self.texrect)?,
            sampler_reduction_minmax: flag(self.sampler_reduction_minmax)?,
            sampler_reduction_minmax_arb: flag(self.sampler_reduction_minmax_arb)?,
            allow_dynamic_vao_fastpath: flag(self.allow_dynamic_vao_fastpath)?,
            emulate_nonfixed_primitive_restart: flag(self.emulate_nonfixed_primitive_restart)?,
            prefer_back_buffer_reuse: flag(self.prefer_back_buffer_reuse)?,
            draw_vertex_state: flag(self.draw_vertex_state)?,
            prefer_pot_aligned_varyings: flag(self.prefer_pot_aligned_varyings)?,
            sparse_texture_full_array_cube_mipmaps: flag(self.sparse_texture_full_array_cube_mipmaps)?,
            query_sparse_texture_residency: flag(self.query_sparse_texture_residency)?,
            clamp_sparse_texture_lod: flag(self.clamp_sparse_texture_lod)?,
            allow_draw_out_of_order: flag(self.allow_draw_out_of_order)?,
            hardware_gl_select: flag(self.hardware_gl_select)?,
            dithering: flag(self.dithering)?,
            fbfetch_zs: flag(self.fbfetch_zs)?,
            timeline_semaphore_import: flag(self.timeline_semaphore_import)?,
            device_protected_context: flag(self.device_protected_context)?,
            allow_glthread_buffer_subdata_opt: flag(self.allow_glthread_buffer_subdata_opt)?,
            null_textures: flag(self.null_textures)?,
            astc_void_extents_need_denorm_flush: flag(self.astc_void_extents_need_denorm_flush)?,
            validate_all_dirty_states: flag(self.validate_all_dirty_states)?,
            has_const_bw: flag(self.has_const_bw)?,
            performance_monitor: flag(self.performance_monitor)?,
            texture_sampler_independent: flag(self.texture_sampler_independent)?,
            astc_decode_mode: flag(self.astc_decode_mode)?,
            shader_subgroup_quad_all_stages: flag(self.shader_subgroup_quad_all_stages)?,
            call_finalize_nir_in_linker: flag(self.call_finalize_nir_in_linker)?,
            mesh_shader: flag(self.mesh_shader)?,
            representative_fragment_test: flag(self.representative_fragment_test)?,
            prefer_persp: flag(self.prefer_persp)?,
            blit_3d: flag(self.blit_3d)?,
            glsl_bindless_handles_are_32bit: flag(self.glsl_bindless_handles_are_32bit)?,
            polygon_stipple: flag(self.polygon_stipple)?,
            disable_custom_border_color_caching: flag(self.disable_custom_border_color_caching)?,
            needs_split_planar_samplers: flag(self.needs_split_planar_samplers)?,
            accelerated: self.accelerated,
            min_texel_offset: self.min_texel_offset,
            max_texel_offset: self.max_texel_offset,
            min_texture_gather_offset: self.min_texture_gather_offset,
            max_texture_gather_offset: self.max_texture_gather_offset,
            max_dual_source_render_targets: self.max_dual_source_render_targets,
            max_render_targets: self.max_render_targets,
            max_texture_2d_size: self.max_texture_2d_size,
            max_texture_3d_levels: self.max_texture_3d_levels,
            max_texture_cube_levels: self.max_texture_cube_levels,
            max_stream_output_buffers: self.max_stream_output_buffers,
            max_texture_array_layers: self.max_texture_array_layers,
            max_stream_output_separate_components: self.max_stream_output_separate_components,
            max_stream_output_interleaved_components: self.max_stream_output_interleaved_components,
            glsl_feature_level: self.glsl_feature_level,
            glsl_feature_level_compatibility: self.glsl_feature_level_compatibility,
            essl_feature_level: self.essl_feature_level,
            constant_buffer_offset_alignment: self.constant_buffer_offset_alignment,
            timer_resolution: self.timer_resolution,
            min_map_buffer_alignment: self.min_map_buffer_alignment,
            texture_buffer_offset_alignment: self.texture_buffer_offset_alignment,
            linear_image_pitch_alignment: self.linear_image_pitch_alignment,
            linear_image_base_address_alignment: self.linear_image_base_address_alignment,
            texture_transfer_modes: self.texture_transfer_modes,
            texture_border_color_quirk: self.texture_border_color_quirk,
            max_texel_buffer_elements: self.max_texel_buffer_elements,
            max_viewports: self.max_viewports,
            max_geometry_output_vertices: self.max_geometry_output_vertices,
            max_geometry_total_output_components: self.max_geometry_total_output_components,
            max_texture_gather_components: self.max_texture_gather_components,
            max_vertex_streams: self.max_vertex_streams,
            vendor_id: self.vendor_id,
            device_id: self.device_id,
            video_memory: self.video_memory,
            max_vertex_attrib_stride: self.max_vertex_attrib_stride,
            max_shader_patch_varyings: self.max_shader_patch_varyings,
            shader_buffer_offset_alignment: self.shader_buffer_offset_alignment,
            pci_group: self.pci_group,
            pci_bus: self.pci_bus,
            pci_device: self.pci_device,
            pci_function: self.pci_function,
            max_window_rectangles: self.max_window_rectangles,
            viewport_subpixel_bits: self.viewport_subpixel_bits,
            rasterizer_subpixel_bits: self.rasterizer_subpixel_bits,
            mixed_color_depth_bits: self.mixed_color_depth_bits,
            fbfetch: self.fbfetch,
            blend_equation_advanced: self.blend_equation_advanced,
            sparse_buffer_page_size: self.sparse_buffer_page_size,
            max_combined_shader_output_resources: self.max_combined_shader_output_resources,
            framebuffer_msaa_constraints: self.framebuffer_msaa_constraints,
            context_priority_mask: self.context_priority_mask,
            constbuf0_flags: self.constbuf0_flags,
            max_conservative_raster_subpixel_precision_bias: self.max_conservative_raster_subpixel_precision_bias,
            max_gs_invocations: self.max_gs_invocations,
            max_shader_buffer_size: self.max_shader_buffer_size,
            max_combined_shader_buffers: self.max_combined_shader_buffers,
            max_combined_image_uniforms: self.max_combined_image_uniforms,
            max_combined_hw_atomic_counters: self.max_combined_hw_atomic_counters,
            max_combined_hw_atomic_counter_buffers: self.max_combined_hw_atomic_counter_buffers,
            max_texture_upload_memory_budget: self.max_texture_upload_memory_budget,
            max_vertex_element_src_offset: self.max_vertex_element_src_offset,
            max_varyings: self.max_varyings,
            ignored_output_varyings: self.ignored_output_varyings,
            dmabuf: self.dmabuf,
            clip_planes: self.clip_planes,
            max_vertex_buffers: self.max_vertex_buffers,
            gl_begin_end_buffer_size: self.gl_begin_end_buffer_size,
            glsl_zero_init: self.glsl_zero_init,
            max_texture_mb: self.max_texture_mb,
            supported_prim_modes: self.supported_prim_modes,
            supported_prim_modes_with_restart: self.supported_prim_modes_with_restart,
            max_sparse_texture_size: self.max_sparse_texture_size,
            max_sparse_3d_texture_size: self.max_sparse_3d_texture_size,
            max_sparse_array_texture_layers: self.max_sparse_array_texture_layers,
            max_constant_buffer_size: self.max_constant_buffer_size,
            query_timestamp_bits: self.query_timestamp_bits,
            shader_subgroup_size: self.shader_subgroup_size,
            shader_subgroup_supported_stages: self.shader_subgroup_supported_stages,
            shader_subgroup_supported_features: self.shader_subgroup_supported_features,
            shader_pixel_local_storage_size: self.shader_pixel_local_storage_size,
            shader_pixel_local_storage_fast_size: self.shader_pixel_local_storage_fast_size,
            multiview: self.multiview,
            max_label_length: self.max_label_length,
            max_timeline_semaphore_difference: self.max_timeline_semaphore_difference,
            map_client_pointer: self.map_client_pointer,
            min_vma: self.min_vma,
            max_vma: self.max_vma,
            hw_clear_buffer_sizes: self.hw_clear_buffer_sizes,
            device_type: wcontainers_gallium::values::DeviceType(self.device_type),
            vertex_input_alignment: wcontainers_gallium::values::VertexInputAlignment(self.vertex_input_alignment),
            endianness: wcontainers_gallium::values::Endian(self.endianness),
            point_size_fixed: wcontainers_gallium::values::PointSizeLowerMode(self.point_size_fixed),
            min_line_width: self.min_line_width,
            min_line_width_aa: self.min_line_width_aa,
            max_line_width: self.max_line_width,
            max_line_width_aa: self.max_line_width_aa,
            line_width_granularity: self.line_width_granularity,
            min_point_size: self.min_point_size,
            min_point_size_aa: self.min_point_size_aa,
            max_point_size: self.max_point_size,
            max_point_size_aa: self.max_point_size_aa,
            point_size_granularity: self.point_size_granularity,
            max_texture_anisotropy: self.max_texture_anisotropy,
            max_texture_lod_bias: self.max_texture_lod_bias,
            min_conservative_raster_dilate: self.min_conservative_raster_dilate,
            max_conservative_raster_dilate: self.max_conservative_raster_dilate,
            conservative_raster_dilate_granularity: self.conservative_raster_dilate_granularity,
            mesh: self.mesh.owned(held)?,
        })
    }
}

/// Mesa's `pipe_clip_state` as the glue's C copies it (`struct wc_flat_pipe_clip_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatClipState {
    pub ucp: [[f32; 4]; 8],
}

impl FlatClipState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ClipState> {
        Some(wcontainers_gallium::ClipState {
            ucp: self.ucp,
        })
    }
}

/// Mesa's `pipe_compute_caps` as the glue's C copies it (`struct wc_flat_pipe_compute_caps` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatComputeCaps {
    pub address_bits: u32,
    pub grid_dimension: u32,
    pub max_grid_size: [u32; 3],
    pub max_block_size: [u32; 3],
    pub max_threads_per_block: u32,
    pub max_local_size: u32,
    pub max_clock_frequency: u32,
    pub max_compute_units: u32,
    pub max_subgroups: u32,
    pub subgroup_sizes: u32,
    pub max_variable_threads_per_block: u32,
    pub max_mem_alloc_size: u64,
    pub max_global_size: u64,
}

impl FlatComputeCaps {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ComputeCaps> {
        Some(wcontainers_gallium::ComputeCaps {
            address_bits: self.address_bits,
            grid_dimension: self.grid_dimension,
            max_grid_size: self.max_grid_size,
            max_block_size: self.max_block_size,
            max_threads_per_block: self.max_threads_per_block,
            max_local_size: self.max_local_size,
            max_clock_frequency: self.max_clock_frequency,
            max_compute_units: self.max_compute_units,
            max_subgroups: self.max_subgroups,
            subgroup_sizes: self.subgroup_sizes,
            max_variable_threads_per_block: self.max_variable_threads_per_block,
            max_mem_alloc_size: self.max_mem_alloc_size,
            max_global_size: self.max_global_size,
        })
    }
}

/// Mesa's `pipe_compute_state` as the glue's C copies it (`struct wc_flat_pipe_compute_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatComputeState {
    pub static_shared_mem: u32,
}

impl FlatComputeState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ComputeState> {
        Some(wcontainers_gallium::ComputeState {
            static_shared_mem: self.static_shared_mem,
        })
    }
}

/// Mesa's `pipe_constant_buffer` as the glue's C copies it (`struct wc_flat_pipe_constant_buffer` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatConstantBuffer {
    pub buffer: usize,
    pub buffer_offset: u32,
    pub buffer_size: u32,
    pub user_buffer: usize,
}

impl FlatConstantBuffer {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.buffer);
        keys.memory(self.user_buffer, usize::try_from(self.buffer_offset).unwrap_or(usize::MAX), usize::try_from(self.buffer_size).unwrap_or(usize::MAX));
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::ConstantBuffer<'a, G::Resource>> {
        Some(wcontainers_gallium::ConstantBuffer {
            buffer: held.resource(self.buffer)?,
            buffer_offset: self.buffer_offset,
            buffer_size: self.buffer_size,
            user_buffer: held.memory(self.user_buffer, usize::try_from(self.buffer_offset).unwrap_or(usize::MAX), usize::try_from(self.buffer_size).unwrap_or(usize::MAX))?,
        })
    }
}

/// Mesa's `pipe_stencil_state` as the glue's C copies it (`struct wc_flat_pipe_stencil_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatStencilState {
    pub enabled: u32,
    pub func: u32,
    pub fail_op: u32,
    pub zpass_op: u32,
    pub zfail_op: u32,
    pub valuemask: u32,
    pub writemask: u32,
}

impl FlatStencilState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::StencilState> {
        Some(wcontainers_gallium::StencilState {
            enabled: within(self.enabled, 1)?,
            func: wcontainers_gallium::values::CompareFunc(within(self.func, 3)?),
            fail_op: wcontainers_gallium::values::StencilOp(within(self.fail_op, 3)?),
            zpass_op: wcontainers_gallium::values::StencilOp(within(self.zpass_op, 3)?),
            zfail_op: wcontainers_gallium::values::StencilOp(within(self.zfail_op, 3)?),
            valuemask: within(self.valuemask, 8)?,
            writemask: within(self.writemask, 8)?,
        })
    }
}

/// Mesa's `pipe_depth_stencil_alpha_state` as the glue's C copies it (`struct wc_flat_pipe_depth_stencil_alpha_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDepthStencilAlphaState {
    pub stencil: [FlatStencilState; 2],
    pub alpha_enabled: u32,
    pub alpha_func: u32,
    pub depth_enabled: u32,
    pub depth_writemask: u32,
    pub depth_func: u32,
    pub depth_bounds_test: u32,
    pub alpha_ref_value: f32,
    pub depth_bounds_min: f64,
    pub depth_bounds_max: f64,
}

impl FlatDepthStencilAlphaState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::DepthStencilAlphaState> {
        Some(wcontainers_gallium::DepthStencilAlphaState {
            stencil: each(&self.stencil, |element0| element0.owned(held))?,
            alpha_enabled: within(self.alpha_enabled, 1)?,
            alpha_func: wcontainers_gallium::values::CompareFunc(within(self.alpha_func, 3)?),
            depth_enabled: within(self.depth_enabled, 1)?,
            depth_writemask: within(self.depth_writemask, 1)?,
            depth_func: wcontainers_gallium::values::CompareFunc(within(self.depth_func, 3)?),
            depth_bounds_test: within(self.depth_bounds_test, 1)?,
            alpha_ref_value: self.alpha_ref_value,
            depth_bounds_min: self.depth_bounds_min,
            depth_bounds_max: self.depth_bounds_max,
        })
    }
}

/// Mesa's `pipe_draw_indirect_info` as the glue's C copies it (`struct wc_flat_pipe_draw_indirect_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDrawIndirectInfo {
    pub offset: u32,
    pub stride: u32,
    pub draw_count: u32,
    pub indirect_draw_count_offset: u32,
    pub buffer: usize,
    pub indirect_draw_count: usize,
    pub count_from_stream_output: usize,
}

impl FlatDrawIndirectInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.buffer);
        keys.object(ObjectKind::Resource, self.indirect_draw_count);
        keys.object(ObjectKind::StreamOutputTarget, self.count_from_stream_output);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::DrawIndirectInfo<'a, G::Resource, G::StreamOutputTarget>> {
        Some(wcontainers_gallium::DrawIndirectInfo {
            offset: self.offset,
            stride: self.stride,
            draw_count: self.draw_count,
            indirect_draw_count_offset: self.indirect_draw_count_offset,
            buffer: held.resource(self.buffer)?,
            indirect_draw_count: held.resource(self.indirect_draw_count)?,
            count_from_stream_output: held.stream_output_target(self.count_from_stream_output)?,
        })
    }
}

/// Mesa's `pipe_draw_info.index` as the glue's C copies it (`struct wc_flat_pipe_draw_info__index` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDrawInfoIndex {
    pub resource: usize,
    pub user: usize,
}

impl FlatDrawInfoIndex {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
        keys.memory(self.user, 0, keys.index_bytes);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::DrawInfoIndex<'a, G::Resource>> {
        Some(wcontainers_gallium::DrawInfoIndex {
            resource: held.resource(self.resource)?,
            user: held.memory(self.user, 0, held.index_bytes)?,
        })
    }
}

/// Mesa's `pipe_draw_info` as the glue's C copies it (`struct wc_flat_pipe_draw_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDrawInfo {
    pub mode: u32,
    pub index_size: u16,
    pub primitive_restart: u8,
    pub has_user_indices: u8,
    pub index_bounds_valid: u8,
    pub increment_draw_id: u8,
    pub index_bias_varies: u8,
    pub was_line_loop: u8,
    pub _pad: u8,
    pub start_instance: u32,
    pub instance_count: u32,
    pub restart_index: u32,
    pub index: FlatDrawInfoIndex,
    pub min_index: u32,
    pub max_index: u32,
}

impl FlatDrawInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        self.index.keys(keys);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::DrawInfo<'a, G::Resource>> {
        Some(wcontainers_gallium::DrawInfo {
            mode: wcontainers_gallium::values::MesaPrim(self.mode),
            index_size: self.index_size,
            primitive_restart: flag(self.primitive_restart)?,
            has_user_indices: flag(self.has_user_indices)?,
            index_bounds_valid: flag(self.index_bounds_valid)?,
            increment_draw_id: flag(self.increment_draw_id)?,
            index_bias_varies: flag(self.index_bias_varies)?,
            was_line_loop: flag(self.was_line_loop)?,
            _pad: within(self._pad, 2)?,
            start_instance: self.start_instance,
            instance_count: self.instance_count,
            restart_index: self.restart_index,
            index: self.index.owned(held)?,
            min_index: self.min_index,
            max_index: self.max_index,
        })
    }
}

/// Mesa's `pipe_draw_start_count_bias` as the glue's C copies it (`struct wc_flat_pipe_draw_start_count_bias` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDrawStartCountBias {
    pub start: u32,
    pub count: u32,
    pub index_bias: i32,
}

impl FlatDrawStartCountBias {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::DrawStartCountBias> {
        Some(wcontainers_gallium::DrawStartCountBias {
            start: self.start,
            count: self.count,
            index_bias: self.index_bias,
        })
    }
}

/// Mesa's `pipe_reference` as the glue's C copies it (`struct wc_flat_pipe_reference` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatReference {
    pub count: i32,
}

impl FlatReference {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::Reference> {
        Some(wcontainers_gallium::Reference {
            count: self.count,
        })
    }
}

/// Mesa's `pipe_surface` as the glue's C copies it (`struct wc_flat_pipe_surface` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSurface {
    pub reference: FlatReference,
    pub format: u32,
    pub nr_samples: u32,
    pub first_layer: u32,
    pub last_layer: u32,
    pub level: u32,
    pub texture: usize,
}

impl FlatSurface {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.texture);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::Surface<'a, G::Resource>> {
        Some(wcontainers_gallium::Surface {
            reference: self.reference.owned(held)?,
            format: wcontainers_gallium::values::Format(within(self.format, 16)?),
            nr_samples: within(self.nr_samples, 16)?,
            first_layer: within(self.first_layer, 16)?,
            last_layer: within(self.last_layer, 16)?,
            level: self.level,
            texture: held.resource(self.texture)?,
        })
    }
}

/// Mesa's `pipe_framebuffer_state` as the glue's C copies it (`struct wc_flat_pipe_framebuffer_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatFramebufferState {
    pub width: u32,
    pub height: u32,
    pub layers: u16,
    pub samples: u8,
    pub nr_cbufs: u8,
    pub pls_enabled: u8,
    pub downscale_cbufs: u8,
    pub _pad: u8,
    pub viewmask: u8,
    pub cbufs: [FlatSurface; 8],
    pub zsbuf: FlatSurface,
    pub resolve: usize,
}

impl FlatFramebufferState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        for element0 in &self.cbufs {
            element0.keys(keys);
        }
        self.zsbuf.keys(keys);
        keys.object(ObjectKind::Resource, self.resolve);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::FramebufferState<'a, G::Resource>> {
        Some(wcontainers_gallium::FramebufferState {
            width: self.width,
            height: self.height,
            layers: self.layers,
            samples: self.samples,
            nr_cbufs: self.nr_cbufs,
            pls_enabled: flag(self.pls_enabled)?,
            downscale_cbufs: flag(self.downscale_cbufs)?,
            _pad: within(self._pad, 6)?,
            viewmask: self.viewmask,
            cbufs: each(&self.cbufs, |element0| element0.owned(held))?,
            zsbuf: self.zsbuf.owned(held)?,
            resolve: held.resource(self.resolve)?,
        })
    }
}

/// Mesa's `pipe_grid_info` as the glue's C copies it (`struct wc_flat_pipe_grid_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatGridInfo {
    pub variable_shared_mem: u32,
    pub work_dim: u32,
    pub block: [u32; 3],
    pub last_block: [u32; 3],
    pub grid: [u32; 3],
    pub grid_base: [u32; 3],
    pub indirect: usize,
    pub indirect_offset: u32,
    pub indirect_stride: u32,
    pub draw_count: u32,
    pub indirect_draw_count_offset: u32,
    pub indirect_draw_count: usize,
}

impl FlatGridInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.indirect);
        keys.object(ObjectKind::Resource, self.indirect_draw_count);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::GridInfo<'a, G::Resource>> {
        Some(wcontainers_gallium::GridInfo {
            variable_shared_mem: self.variable_shared_mem,
            work_dim: self.work_dim,
            block: self.block,
            last_block: self.last_block,
            grid: self.grid,
            grid_base: self.grid_base,
            indirect: held.resource(self.indirect)?,
            indirect_offset: self.indirect_offset,
            indirect_stride: self.indirect_stride,
            draw_count: self.draw_count,
            indirect_draw_count_offset: self.indirect_draw_count_offset,
            indirect_draw_count: held.resource(self.indirect_draw_count)?,
        })
    }
}

/// Mesa's `pipe_image_view.u.tex` as the glue's C copies it (`struct wc_flat_pipe_image_view__u__tex` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatImageViewUTex {
    pub first_layer: u32,
    pub last_layer: u32,
    pub level: u32,
    pub single_layer_view: u8,
    pub is_2d_view_of_3d: u8,
}

impl FlatImageViewUTex {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ImageViewUTex> {
        Some(wcontainers_gallium::ImageViewUTex {
            first_layer: within(self.first_layer, 16)?,
            last_layer: within(self.last_layer, 16)?,
            level: within(self.level, 8)?,
            single_layer_view: flag(self.single_layer_view)?,
            is_2d_view_of_3d: flag(self.is_2d_view_of_3d)?,
        })
    }
}

/// Mesa's `pipe_image_view.u.buf` as the glue's C copies it (`struct wc_flat_pipe_image_view__u__buf` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatImageViewUBuf {
    pub offset: u32,
    pub size: u32,
}

impl FlatImageViewUBuf {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ImageViewUBuf> {
        Some(wcontainers_gallium::ImageViewUBuf {
            offset: self.offset,
            size: self.size,
        })
    }
}

/// Mesa's `pipe_tex2d_from_buf` as the glue's C copies it (`struct wc_flat_pipe_tex2d_from_buf` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatTex2dFromBuf {
    pub offset: u32,
    pub row_stride: u16,
    pub width: u16,
    pub height: u16,
}

impl FlatTex2dFromBuf {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::Tex2dFromBuf> {
        Some(wcontainers_gallium::Tex2dFromBuf {
            offset: self.offset,
            row_stride: self.row_stride,
            width: self.width,
            height: self.height,
        })
    }
}

/// Mesa's `pipe_image_view.u` as the glue's C copies it (`struct wc_flat_pipe_image_view__u` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatImageViewU {
    pub tex: FlatImageViewUTex,
    pub buf: FlatImageViewUBuf,
    pub tex2d_from_buf: FlatTex2dFromBuf,
}

impl FlatImageViewU {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::ImageViewU> {
        Some(wcontainers_gallium::ImageViewU {
            tex: self.tex.owned(held)?,
            buf: self.buf.owned(held)?,
            tex2d_from_buf: self.tex2d_from_buf.owned(held)?,
        })
    }
}

/// Mesa's `pipe_image_view` as the glue's C copies it (`struct wc_flat_pipe_image_view` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatImageView {
    pub resource: usize,
    pub format: u32,
    pub access: u16,
    pub shader_access: u16,
    pub u: FlatImageViewU,
}

impl FlatImageView {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::ImageView<'a, G::Resource>> {
        Some(wcontainers_gallium::ImageView {
            resource: held.resource(self.resource)?,
            format: wcontainers_gallium::values::Format(self.format),
            access: self.access,
            shader_access: self.shader_access,
            u: self.u.owned(held)?,
        })
    }
}

/// Mesa's `pipe_poly_stipple` as the glue's C copies it (`struct wc_flat_pipe_poly_stipple` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatPolyStipple {
    pub stipple: [u32; 32],
}

impl FlatPolyStipple {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::PolyStipple> {
        Some(wcontainers_gallium::PolyStipple {
            stipple: self.stipple,
        })
    }
}

/// Mesa's `pipe_rasterizer_state` as the glue's C copies it (`struct wc_flat_pipe_rasterizer_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatRasterizerState {
    pub flatshade: u32,
    pub light_twoside: u32,
    pub clamp_vertex_color: u32,
    pub clamp_fragment_color: u32,
    pub front_ccw: u32,
    pub cull_face: u32,
    pub fill_front: u32,
    pub fill_back: u32,
    pub offset_point: u32,
    pub offset_line: u32,
    pub offset_tri: u32,
    pub scissor: u32,
    pub poly_smooth: u32,
    pub poly_stipple_enable: u32,
    pub point_smooth: u32,
    pub sprite_coord_mode: u32,
    pub point_quad_rasterization: u32,
    pub point_line_tri_clip: u32,
    pub point_size_per_vertex: u32,
    pub multisample: u32,
    pub no_ms_sample_mask_out: u32,
    pub force_persample_interp: u32,
    pub line_smooth: u32,
    pub line_stipple_enable: u32,
    pub line_last_pixel: u32,
    pub line_rectangular: u32,
    pub conservative_raster_mode: u32,
    pub flatshade_first: u32,
    pub half_pixel_center: u32,
    pub bottom_edge_rule: u32,
    pub subpixel_precision_x: u32,
    pub subpixel_precision_y: u32,
    pub rasterizer_discard: u32,
    pub rasterization_stream: u32,
    pub tile_raster_order_fixed: u32,
    pub tile_raster_order_increasing_x: u32,
    pub tile_raster_order_increasing_y: u32,
    pub depth_clip_near: u32,
    pub depth_clip_far: u32,
    pub depth_clamp: u32,
    pub clip_halfz: u32,
    pub offset_units_unscaled: u32,
    pub unclamped_fragment_depth_values: u32,
    pub clip_plane_enable: u32,
    pub representative_fragment_test: u32,
    pub line_stipple_factor: u32,
    pub line_stipple_pattern: u32,
    pub sprite_coord_enable: u16,
    pub line_width: f32,
    pub point_size: f32,
    pub offset_units: f32,
    pub offset_scale: f32,
    pub offset_clamp: f32,
    pub conservative_raster_dilate: f32,
}

impl FlatRasterizerState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::RasterizerState> {
        Some(wcontainers_gallium::RasterizerState {
            flatshade: within(self.flatshade, 1)?,
            light_twoside: within(self.light_twoside, 1)?,
            clamp_vertex_color: within(self.clamp_vertex_color, 1)?,
            clamp_fragment_color: within(self.clamp_fragment_color, 1)?,
            front_ccw: within(self.front_ccw, 1)?,
            cull_face: within(self.cull_face, 2)?,
            fill_front: within(self.fill_front, 2)?,
            fill_back: within(self.fill_back, 2)?,
            offset_point: within(self.offset_point, 1)?,
            offset_line: within(self.offset_line, 1)?,
            offset_tri: within(self.offset_tri, 1)?,
            scissor: within(self.scissor, 1)?,
            poly_smooth: within(self.poly_smooth, 1)?,
            poly_stipple_enable: within(self.poly_stipple_enable, 1)?,
            point_smooth: within(self.point_smooth, 1)?,
            sprite_coord_mode: wcontainers_gallium::values::SpriteCoordMode(within(self.sprite_coord_mode, 1)?),
            point_quad_rasterization: within(self.point_quad_rasterization, 1)?,
            point_line_tri_clip: within(self.point_line_tri_clip, 1)?,
            point_size_per_vertex: within(self.point_size_per_vertex, 1)?,
            multisample: within(self.multisample, 1)?,
            no_ms_sample_mask_out: within(self.no_ms_sample_mask_out, 1)?,
            force_persample_interp: within(self.force_persample_interp, 1)?,
            line_smooth: within(self.line_smooth, 1)?,
            line_stipple_enable: within(self.line_stipple_enable, 1)?,
            line_last_pixel: within(self.line_last_pixel, 1)?,
            line_rectangular: within(self.line_rectangular, 1)?,
            conservative_raster_mode: wcontainers_gallium::values::ConservativeRasterMode(within(self.conservative_raster_mode, 2)?),
            flatshade_first: within(self.flatshade_first, 1)?,
            half_pixel_center: within(self.half_pixel_center, 1)?,
            bottom_edge_rule: within(self.bottom_edge_rule, 1)?,
            subpixel_precision_x: within(self.subpixel_precision_x, 4)?,
            subpixel_precision_y: within(self.subpixel_precision_y, 4)?,
            rasterizer_discard: within(self.rasterizer_discard, 1)?,
            rasterization_stream: within(self.rasterization_stream, 2)?,
            tile_raster_order_fixed: within(self.tile_raster_order_fixed, 1)?,
            tile_raster_order_increasing_x: within(self.tile_raster_order_increasing_x, 1)?,
            tile_raster_order_increasing_y: within(self.tile_raster_order_increasing_y, 1)?,
            depth_clip_near: within(self.depth_clip_near, 1)?,
            depth_clip_far: within(self.depth_clip_far, 1)?,
            depth_clamp: within(self.depth_clamp, 1)?,
            clip_halfz: within(self.clip_halfz, 1)?,
            offset_units_unscaled: within(self.offset_units_unscaled, 1)?,
            unclamped_fragment_depth_values: within(self.unclamped_fragment_depth_values, 1)?,
            clip_plane_enable: within(self.clip_plane_enable, 8)?,
            representative_fragment_test: within(self.representative_fragment_test, 1)?,
            line_stipple_factor: within(self.line_stipple_factor, 8)?,
            line_stipple_pattern: within(self.line_stipple_pattern, 16)?,
            sprite_coord_enable: self.sprite_coord_enable,
            line_width: self.line_width,
            point_size: self.point_size,
            offset_units: self.offset_units,
            offset_scale: self.offset_scale,
            offset_clamp: self.offset_clamp,
            conservative_raster_dilate: self.conservative_raster_dilate,
        })
    }
}

/// Mesa's `pipe_resource` as the glue's C copies it (`struct wc_flat_pipe_resource` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatResourceTemplate {
    pub width0: u32,
    pub height0: u32,
    pub depth0: u16,
    pub array_size: u16,
    pub format: u32,
    pub target: u32,
    pub last_level: u8,
    pub nr_samples: u8,
    pub nr_storage_samples: u8,
    pub nr_sparse_levels: u8,
    pub compression_rate: u32,
    pub usage: u32,
    pub bind: u32,
    pub flags: u32,
}

impl FlatResourceTemplate {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ResourceTemplate> {
        Some(wcontainers_gallium::ResourceTemplate {
            width0: self.width0,
            height0: self.height0,
            depth0: self.depth0,
            array_size: self.array_size,
            format: wcontainers_gallium::values::Format(within(self.format, 16)?),
            target: wcontainers_gallium::values::TextureTarget(within(self.target, 8)?),
            last_level: self.last_level,
            nr_samples: self.nr_samples,
            nr_storage_samples: self.nr_storage_samples,
            nr_sparse_levels: self.nr_sparse_levels,
            compression_rate: within(self.compression_rate, 4)?,
            usage: wcontainers_gallium::values::ResourceUsage(within(self.usage, 4)?),
            bind: self.bind,
            flags: self.flags,
        })
    }
}

/// Mesa's `pipe_sampler_state` as the glue's C copies it (`struct wc_flat_pipe_sampler_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerState {
    pub wrap_s: u32,
    pub wrap_t: u32,
    pub wrap_r: u32,
    pub min_img_filter: u32,
    pub min_mip_filter: u32,
    pub mag_img_filter: u32,
    pub compare_mode: u32,
    pub compare_func: u32,
    pub unnormalized_coords: u32,
    pub max_anisotropy: u32,
    pub seamless_cube_map: u32,
    pub border_color_is_integer: u32,
    pub reduction_mode: u32,
    pub pad: u32,
    pub lod_bias: f32,
    pub min_lod: f32,
    pub max_lod: f32,
    pub border_color: [u32; 4],
    pub border_color_format: u32,
}

impl FlatSamplerState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::SamplerState> {
        Some(wcontainers_gallium::SamplerState {
            wrap_s: wcontainers_gallium::values::TexWrap(within(self.wrap_s, 3)?),
            wrap_t: wcontainers_gallium::values::TexWrap(within(self.wrap_t, 3)?),
            wrap_r: wcontainers_gallium::values::TexWrap(within(self.wrap_r, 3)?),
            min_img_filter: wcontainers_gallium::values::TexFilter(within(self.min_img_filter, 1)?),
            min_mip_filter: wcontainers_gallium::values::TexMipfilter(within(self.min_mip_filter, 2)?),
            mag_img_filter: wcontainers_gallium::values::TexFilter(within(self.mag_img_filter, 1)?),
            compare_mode: wcontainers_gallium::values::TexCompare(within(self.compare_mode, 1)?),
            compare_func: wcontainers_gallium::values::CompareFunc(within(self.compare_func, 3)?),
            unnormalized_coords: within(self.unnormalized_coords, 1)?,
            max_anisotropy: within(self.max_anisotropy, 5)?,
            seamless_cube_map: within(self.seamless_cube_map, 1)?,
            border_color_is_integer: within(self.border_color_is_integer, 1)?,
            reduction_mode: wcontainers_gallium::values::TexReductionMode(within(self.reduction_mode, 2)?),
            pad: within(self.pad, 5)?,
            lod_bias: self.lod_bias,
            min_lod: self.min_lod,
            max_lod: self.max_lod,
            border_color: self.border_color,
            border_color_format: wcontainers_gallium::values::Format(self.border_color_format),
        })
    }
}

/// Mesa's `pipe_sampler_view.u.tex` as the glue's C copies it (`struct wc_flat_pipe_sampler_view__u__tex` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerViewUTex {
    pub first_layer: u32,
    pub last_layer: u32,
    pub first_level: u32,
    pub last_level: u32,
    pub min_lod_clamp: f32,
}

impl FlatSamplerViewUTex {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::SamplerViewUTex> {
        Some(wcontainers_gallium::SamplerViewUTex {
            first_layer: within(self.first_layer, 16)?,
            last_layer: within(self.last_layer, 16)?,
            first_level: within(self.first_level, 8)?,
            last_level: within(self.last_level, 8)?,
            min_lod_clamp: self.min_lod_clamp,
        })
    }
}

/// Mesa's `pipe_sampler_view.u.buf` as the glue's C copies it (`struct wc_flat_pipe_sampler_view__u__buf` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerViewUBuf {
    pub offset: u32,
    pub size: u32,
}

impl FlatSamplerViewUBuf {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::SamplerViewUBuf> {
        Some(wcontainers_gallium::SamplerViewUBuf {
            offset: self.offset,
            size: self.size,
        })
    }
}

/// Mesa's `pipe_sampler_view.u` as the glue's C copies it (`struct wc_flat_pipe_sampler_view__u` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerViewU {
    pub tex: FlatSamplerViewUTex,
    pub buf: FlatSamplerViewUBuf,
    pub tex2d_from_buf: FlatTex2dFromBuf,
}

impl FlatSamplerViewU {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::SamplerViewU> {
        Some(wcontainers_gallium::SamplerViewU {
            tex: self.tex.owned(held)?,
            buf: self.buf.owned(held)?,
            tex2d_from_buf: self.tex2d_from_buf.owned(held)?,
        })
    }
}

/// Mesa's `pipe_sampler_view` as the glue's C copies it (`struct wc_flat_pipe_sampler_view` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerViewTemplate {
    pub format: u32,
    pub astc_decode_format: u32,
    pub is_tex2d_from_buf: u8,
    pub target: u32,
    pub swizzle_r: u32,
    pub swizzle_g: u32,
    pub swizzle_b: u32,
    pub swizzle_a: u32,
    pub u: FlatSamplerViewU,
}

impl FlatSamplerViewTemplate {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::SamplerViewTemplate> {
        Some(wcontainers_gallium::SamplerViewTemplate {
            format: wcontainers_gallium::values::Format(within(self.format, 12)?),
            astc_decode_format: within(self.astc_decode_format, 2)?,
            is_tex2d_from_buf: flag(self.is_tex2d_from_buf)?,
            target: wcontainers_gallium::values::TextureTarget(within(self.target, 5)?),
            swizzle_r: wcontainers_gallium::values::Swizzle(within(self.swizzle_r, 3)?),
            swizzle_g: wcontainers_gallium::values::Swizzle(within(self.swizzle_g, 3)?),
            swizzle_b: wcontainers_gallium::values::Swizzle(within(self.swizzle_b, 3)?),
            swizzle_a: wcontainers_gallium::values::Swizzle(within(self.swizzle_a, 3)?),
            u: self.u.owned(held)?,
        })
    }
}

/// Mesa's `pipe_shader_buffer` as the glue's C copies it (`struct wc_flat_pipe_shader_buffer` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatShaderBuffer {
    pub buffer: usize,
    pub buffer_offset: u32,
    pub buffer_size: u32,
}

impl FlatShaderBuffer {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.buffer);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::ShaderBuffer<'a, G::Resource>> {
        Some(wcontainers_gallium::ShaderBuffer {
            buffer: held.resource(self.buffer)?,
            buffer_offset: self.buffer_offset,
            buffer_size: self.buffer_size,
        })
    }
}

/// Mesa's `pipe_shader_caps` as the glue's C copies it (`struct wc_flat_pipe_shader_caps` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatShaderCaps {
    pub max_instructions: u32,
    pub max_alu_instructions: u32,
    pub max_tex_instructions: u32,
    pub max_tex_indirections: u32,
    pub max_control_flow_depth: u32,
    pub max_inputs: u32,
    pub max_outputs: u32,
    pub max_const_buffer0_size: u32,
    pub max_const_buffers: u32,
    pub max_temps: u32,
    pub max_texture_samplers: u32,
    pub max_sampler_views: u32,
    pub max_shader_buffers: u32,
    pub max_shader_images: u32,
    pub max_hw_atomic_counters: u32,
    pub max_hw_atomic_counter_buffers: u32,
    pub supported_irs: u32,
    pub cont_supported: u8,
    pub indirect_temp_addr: u8,
    pub indirect_const_addr: u8,
    pub subroutines: u8,
    pub integers: u8,
    pub int64_atomics: u8,
    pub fp16: u8,
    pub fp16_derivatives: u8,
    pub fp16_const_buffers: u8,
    pub fp16_no_denorms: u8,
    pub int16: u8,
    pub glsl_16bit_consts: u8,
    pub glsl_16bit_load_dst: u8,
    pub tgsi_sqrt_supported: u8,
    pub tgsi_any_inout_decl_range: u8,
}

impl FlatShaderCaps {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ShaderCaps> {
        Some(wcontainers_gallium::ShaderCaps {
            max_instructions: self.max_instructions,
            max_alu_instructions: self.max_alu_instructions,
            max_tex_instructions: self.max_tex_instructions,
            max_tex_indirections: self.max_tex_indirections,
            max_control_flow_depth: self.max_control_flow_depth,
            max_inputs: self.max_inputs,
            max_outputs: self.max_outputs,
            max_const_buffer0_size: self.max_const_buffer0_size,
            max_const_buffers: self.max_const_buffers,
            max_temps: self.max_temps,
            max_texture_samplers: self.max_texture_samplers,
            max_sampler_views: self.max_sampler_views,
            max_shader_buffers: self.max_shader_buffers,
            max_shader_images: self.max_shader_images,
            max_hw_atomic_counters: self.max_hw_atomic_counters,
            max_hw_atomic_counter_buffers: self.max_hw_atomic_counter_buffers,
            supported_irs: self.supported_irs,
            cont_supported: flag(self.cont_supported)?,
            indirect_temp_addr: flag(self.indirect_temp_addr)?,
            indirect_const_addr: flag(self.indirect_const_addr)?,
            subroutines: flag(self.subroutines)?,
            integers: flag(self.integers)?,
            int64_atomics: flag(self.int64_atomics)?,
            fp16: flag(self.fp16)?,
            fp16_derivatives: flag(self.fp16_derivatives)?,
            fp16_const_buffers: flag(self.fp16_const_buffers)?,
            fp16_no_denorms: flag(self.fp16_no_denorms)?,
            int16: flag(self.int16)?,
            glsl_16bit_consts: flag(self.glsl_16bit_consts)?,
            glsl_16bit_load_dst: flag(self.glsl_16bit_load_dst)?,
            tgsi_sqrt_supported: flag(self.tgsi_sqrt_supported)?,
            tgsi_any_inout_decl_range: flag(self.tgsi_any_inout_decl_range)?,
        })
    }
}

/// Mesa's `pipe_stream_output` as the glue's C copies it (`struct wc_flat_pipe_stream_output` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatStreamOutput {
    pub register_index: u32,
    pub start_component: u32,
    pub num_components: u32,
    pub output_buffer: u32,
    pub dst_offset: u32,
    pub stream: u32,
}

impl FlatStreamOutput {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::StreamOutput> {
        Some(wcontainers_gallium::StreamOutput {
            register_index: within(self.register_index, 6)?,
            start_component: within(self.start_component, 2)?,
            num_components: within(self.num_components, 3)?,
            output_buffer: within(self.output_buffer, 3)?,
            dst_offset: within(self.dst_offset, 16)?,
            stream: within(self.stream, 2)?,
        })
    }
}

/// Mesa's `pipe_stream_output_info` as the glue's C copies it (`struct wc_flat_pipe_stream_output_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatStreamOutputInfo {
    pub num_outputs: u32,
    pub stride: [u16; 4],
    pub output: [FlatStreamOutput; 128],
}

impl FlatStreamOutputInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::StreamOutputInfo> {
        Some(wcontainers_gallium::StreamOutputInfo {
            num_outputs: self.num_outputs,
            stride: self.stride,
            output: each(&self.output, |element0| element0.owned(held))?,
        })
    }
}

/// Mesa's `pipe_shader_state` as the glue's C copies it (`struct wc_flat_pipe_shader_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatShaderState {
    pub stream_output: FlatStreamOutputInfo,
    pub report_compile_error: u8,
}

impl FlatShaderState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::ShaderState> {
        Some(wcontainers_gallium::ShaderState {
            stream_output: self.stream_output.owned(held)?,
            report_compile_error: flag(self.report_compile_error)?,
        })
    }
}

/// Mesa's `pipe_stencil_ref` as the glue's C copies it (`struct wc_flat_pipe_stencil_ref` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatStencilRef {
    pub ref_value: [u8; 2],
}

impl FlatStencilRef {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::StencilRef> {
        Some(wcontainers_gallium::StencilRef {
            ref_value: self.ref_value,
        })
    }
}

/// Mesa's `pipe_vertex_buffer.buffer` as the glue's C copies it (`struct wc_flat_pipe_vertex_buffer__buffer` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatVertexBufferBuffer {
    pub resource: usize,
}

impl FlatVertexBufferBuffer {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::VertexBufferBuffer<'a, G::Resource>> {
        Some(wcontainers_gallium::VertexBufferBuffer {
            resource: held.resource(self.resource)?,
        })
    }
}

/// Mesa's `pipe_vertex_buffer` as the glue's C copies it (`struct wc_flat_pipe_vertex_buffer` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatVertexBuffer {
    pub is_user_buffer: u8,
    pub buffer_offset: u32,
    pub buffer: FlatVertexBufferBuffer,
}

impl FlatVertexBuffer {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        self.buffer.keys(keys);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::VertexBuffer<'a, G::Resource>> {
        Some(wcontainers_gallium::VertexBuffer {
            is_user_buffer: flag(self.is_user_buffer)?,
            buffer_offset: self.buffer_offset,
            buffer: self.buffer.owned(held)?,
        })
    }
}

/// Mesa's `pipe_vertex_element` as the glue's C copies it (`struct wc_flat_pipe_vertex_element` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatVertexElement {
    pub src_offset: u16,
    pub vertex_buffer_index: u8,
    pub dual_slot: u8,
    pub src_format: u8,
    pub src_stride: u32,
    pub instance_divisor: u32,
}

impl FlatVertexElement {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::VertexElement> {
        Some(wcontainers_gallium::VertexElement {
            src_offset: self.src_offset,
            vertex_buffer_index: within(self.vertex_buffer_index, 7)?,
            dual_slot: flag(self.dual_slot)?,
            src_format: self.src_format,
            src_stride: self.src_stride,
            instance_divisor: self.instance_divisor,
        })
    }
}

/// Mesa's `pipe_viewport_state` as the glue's C copies it (`struct wc_flat_pipe_viewport_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatViewportState {
    pub scale: [f32; 3],
    pub translate: [f32; 3],
    pub swizzle_x: u32,
    pub swizzle_y: u32,
    pub swizzle_z: u32,
    pub swizzle_w: u32,
}

impl FlatViewportState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ViewportState> {
        Some(wcontainers_gallium::ViewportState {
            scale: self.scale,
            translate: self.translate,
            swizzle_x: wcontainers_gallium::values::ViewportSwizzle(within(self.swizzle_x, 8)?),
            swizzle_y: wcontainers_gallium::values::ViewportSwizzle(within(self.swizzle_y, 8)?),
            swizzle_z: wcontainers_gallium::values::ViewportSwizzle(within(self.swizzle_z, 8)?),
            swizzle_w: wcontainers_gallium::values::ViewportSwizzle(within(self.swizzle_w, 8)?),
        })
    }
}

/// Each flat form's index in flat.c's `wc_flat_size` and its size in Rust.
pub const SIZES: &[(u32, usize)] = &[
    (0, core::mem::size_of::<FlatBlendColor>()),
    (1, core::mem::size_of::<FlatBlendState>()),
    (2, core::mem::size_of::<FlatBlitInfo>()),
    (3, core::mem::size_of::<FlatBlitInfoDst>()),
    (4, core::mem::size_of::<FlatBlitInfoSrc>()),
    (5, core::mem::size_of::<FlatPipeBox>()),
    (6, core::mem::size_of::<FlatCaps>()),
    (7, core::mem::size_of::<FlatClipState>()),
    (8, core::mem::size_of::<FlatComputeCaps>()),
    (9, core::mem::size_of::<FlatComputeState>()),
    (10, core::mem::size_of::<FlatConstantBuffer>()),
    (11, core::mem::size_of::<FlatDepthStencilAlphaState>()),
    (12, core::mem::size_of::<FlatDrawIndirectInfo>()),
    (13, core::mem::size_of::<FlatDrawInfo>()),
    (14, core::mem::size_of::<FlatDrawInfoIndex>()),
    (15, core::mem::size_of::<FlatDrawStartCountBias>()),
    (16, core::mem::size_of::<FlatFramebufferState>()),
    (17, core::mem::size_of::<FlatGridInfo>()),
    (18, core::mem::size_of::<FlatImageView>()),
    (19, core::mem::size_of::<FlatImageViewU>()),
    (20, core::mem::size_of::<FlatImageViewUBuf>()),
    (21, core::mem::size_of::<FlatImageViewUTex>()),
    (22, core::mem::size_of::<FlatMeshCaps>()),
    (23, core::mem::size_of::<FlatPolyStipple>()),
    (24, core::mem::size_of::<FlatRasterizerState>()),
    (25, core::mem::size_of::<FlatReference>()),
    (26, core::mem::size_of::<FlatResourceTemplate>()),
    (27, core::mem::size_of::<FlatRtBlendState>()),
    (28, core::mem::size_of::<FlatSamplerState>()),
    (29, core::mem::size_of::<FlatSamplerViewTemplate>()),
    (30, core::mem::size_of::<FlatSamplerViewU>()),
    (31, core::mem::size_of::<FlatSamplerViewUBuf>()),
    (32, core::mem::size_of::<FlatSamplerViewUTex>()),
    (33, core::mem::size_of::<FlatScissorState>()),
    (34, core::mem::size_of::<FlatShaderBuffer>()),
    (35, core::mem::size_of::<FlatShaderCaps>()),
    (36, core::mem::size_of::<FlatShaderState>()),
    (37, core::mem::size_of::<FlatStencilRef>()),
    (38, core::mem::size_of::<FlatStencilState>()),
    (39, core::mem::size_of::<FlatStreamOutput>()),
    (40, core::mem::size_of::<FlatStreamOutputInfo>()),
    (41, core::mem::size_of::<FlatSurface>()),
    (42, core::mem::size_of::<FlatTex2dFromBuf>()),
    (43, core::mem::size_of::<FlatVertexBuffer>()),
    (44, core::mem::size_of::<FlatVertexBufferBuffer>()),
    (45, core::mem::size_of::<FlatVertexElement>()),
    (46, core::mem::size_of::<FlatViewportState>()),
];

/// Every field of every flat form, in flat.c's `wc_flat_layout` order: its Mesa structure, its name, its offset and size
/// in the Rust mirror, its width if Mesa declares it a bit-field, and whether its size in Mesa is its size here (a
/// number, a flag, an object, client memory or a color; not a structure, flattened, nor a bit-field), or at least its
/// size there (an enumeration, which Mesa may pack).
pub const FIELDS: &[FieldLayout] = &[
    FieldLayout { structure: "pipe_blend_color", field: "color", flat_offset: core::mem::offset_of!(FlatBlendColor, color), flat_size: core::mem::size_of::<[f32; 4]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "independent_blend_enable", flat_offset: core::mem::offset_of!(FlatBlendState, independent_blend_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "logicop_enable", flat_offset: core::mem::offset_of!(FlatBlendState, logicop_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "logicop_func", flat_offset: core::mem::offset_of!(FlatBlendState, logicop_func), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "dither", flat_offset: core::mem::offset_of!(FlatBlendState, dither), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "alpha_to_coverage", flat_offset: core::mem::offset_of!(FlatBlendState, alpha_to_coverage), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "alpha_to_coverage_dither", flat_offset: core::mem::offset_of!(FlatBlendState, alpha_to_coverage_dither), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "alpha_to_one", flat_offset: core::mem::offset_of!(FlatBlendState, alpha_to_one), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "max_rt", flat_offset: core::mem::offset_of!(FlatBlendState, max_rt), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "advanced_blend_func", flat_offset: core::mem::offset_of!(FlatBlendState, advanced_blend_func), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "blend_coherent", flat_offset: core::mem::offset_of!(FlatBlendState, blend_coherent), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "is_yuv", flat_offset: core::mem::offset_of!(FlatBlendState, is_yuv), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "rt", flat_offset: core::mem::offset_of!(FlatBlendState, rt), flat_size: core::mem::size_of::<[FlatRtBlendState; 8]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "dst", flat_offset: core::mem::offset_of!(FlatBlitInfo, dst), flat_size: core::mem::size_of::<FlatBlitInfoDst>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "src", flat_offset: core::mem::offset_of!(FlatBlitInfo, src), flat_size: core::mem::size_of::<FlatBlitInfoSrc>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "mask", flat_offset: core::mem::offset_of!(FlatBlitInfo, mask), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "filter", flat_offset: core::mem::offset_of!(FlatBlitInfo, filter), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "dst_sample", flat_offset: core::mem::offset_of!(FlatBlitInfo, dst_sample), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "sample0_only", flat_offset: core::mem::offset_of!(FlatBlitInfo, sample0_only), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "scissor_enable", flat_offset: core::mem::offset_of!(FlatBlitInfo, scissor_enable), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "scissor", flat_offset: core::mem::offset_of!(FlatBlitInfo, scissor), flat_size: core::mem::size_of::<FlatScissorState>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "swizzle_enable", flat_offset: core::mem::offset_of!(FlatBlitInfo, swizzle_enable), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "swizzle", flat_offset: core::mem::offset_of!(FlatBlitInfo, swizzle), flat_size: core::mem::size_of::<[u8; 4]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "window_rectangle_include", flat_offset: core::mem::offset_of!(FlatBlitInfo, window_rectangle_include), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "num_window_rectangles", flat_offset: core::mem::offset_of!(FlatBlitInfo, num_window_rectangles), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "window_rectangles", flat_offset: core::mem::offset_of!(FlatBlitInfo, window_rectangles), flat_size: core::mem::size_of::<[FlatScissorState; 8]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "render_condition_enable", flat_offset: core::mem::offset_of!(FlatBlitInfo, render_condition_enable), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "alpha_blend", flat_offset: core::mem::offset_of!(FlatBlitInfo, alpha_blend), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.dst", field: "resource", flat_offset: core::mem::offset_of!(FlatBlitInfoDst, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.dst", field: "level", flat_offset: core::mem::offset_of!(FlatBlitInfoDst, level), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.dst", field: "box", flat_offset: core::mem::offset_of!(FlatBlitInfoDst, box_), flat_size: core::mem::size_of::<FlatPipeBox>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info.dst", field: "format", flat_offset: core::mem::offset_of!(FlatBlitInfoDst, format), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_blit_info.src", field: "resource", flat_offset: core::mem::offset_of!(FlatBlitInfoSrc, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.src", field: "level", flat_offset: core::mem::offset_of!(FlatBlitInfoSrc, level), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.src", field: "box", flat_offset: core::mem::offset_of!(FlatBlitInfoSrc, box_), flat_size: core::mem::size_of::<FlatPipeBox>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info.src", field: "format", flat_offset: core::mem::offset_of!(FlatBlitInfoSrc, format), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_box", field: "x", flat_offset: core::mem::offset_of!(FlatPipeBox, x), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "width", flat_offset: core::mem::offset_of!(FlatPipeBox, width), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "y", flat_offset: core::mem::offset_of!(FlatPipeBox, y), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "height", flat_offset: core::mem::offset_of!(FlatPipeBox, height), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "z", flat_offset: core::mem::offset_of!(FlatPipeBox, z), flat_size: core::mem::size_of::<i16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "depth", flat_offset: core::mem::offset_of!(FlatPipeBox, depth), flat_size: core::mem::size_of::<i16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "graphics", flat_offset: core::mem::offset_of!(FlatCaps, graphics), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "npot_textures", flat_offset: core::mem::offset_of!(FlatCaps, npot_textures), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "anisotropic_filter", flat_offset: core::mem::offset_of!(FlatCaps, anisotropic_filter), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "occlusion_query", flat_offset: core::mem::offset_of!(FlatCaps, occlusion_query), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "query_time_elapsed", flat_offset: core::mem::offset_of!(FlatCaps, query_time_elapsed), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_shadow_map", flat_offset: core::mem::offset_of!(FlatCaps, texture_shadow_map), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_swizzle", flat_offset: core::mem::offset_of!(FlatCaps, texture_swizzle), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_mirror_clamp", flat_offset: core::mem::offset_of!(FlatCaps, texture_mirror_clamp), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "blend_equation_separate", flat_offset: core::mem::offset_of!(FlatCaps, blend_equation_separate), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "primitive_restart", flat_offset: core::mem::offset_of!(FlatCaps, primitive_restart), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "primitive_restart_fixed_index", flat_offset: core::mem::offset_of!(FlatCaps, primitive_restart_fixed_index), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "indep_blend_enable", flat_offset: core::mem::offset_of!(FlatCaps, indep_blend_enable), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "indep_blend_func", flat_offset: core::mem::offset_of!(FlatCaps, indep_blend_func), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fs_coord_origin_upper_left", flat_offset: core::mem::offset_of!(FlatCaps, fs_coord_origin_upper_left), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fs_coord_origin_lower_left", flat_offset: core::mem::offset_of!(FlatCaps, fs_coord_origin_lower_left), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fs_coord_pixel_center_half_integer", flat_offset: core::mem::offset_of!(FlatCaps, fs_coord_pixel_center_half_integer), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fs_coord_pixel_center_integer", flat_offset: core::mem::offset_of!(FlatCaps, fs_coord_pixel_center_integer), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "depth_clip_disable", flat_offset: core::mem::offset_of!(FlatCaps, depth_clip_disable), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "depth_clip_disable_separate", flat_offset: core::mem::offset_of!(FlatCaps, depth_clip_disable_separate), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "depth_clamp_enable", flat_offset: core::mem::offset_of!(FlatCaps, depth_clamp_enable), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_stencil_export", flat_offset: core::mem::offset_of!(FlatCaps, shader_stencil_export), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "vs_instanceid", flat_offset: core::mem::offset_of!(FlatCaps, vs_instanceid), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "vertex_element_instance_divisor", flat_offset: core::mem::offset_of!(FlatCaps, vertex_element_instance_divisor), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fragment_color_clamped", flat_offset: core::mem::offset_of!(FlatCaps, fragment_color_clamped), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "mixed_colorbuffer_formats", flat_offset: core::mem::offset_of!(FlatCaps, mixed_colorbuffer_formats), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "seamless_cube_map", flat_offset: core::mem::offset_of!(FlatCaps, seamless_cube_map), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "seamless_cube_map_per_texture", flat_offset: core::mem::offset_of!(FlatCaps, seamless_cube_map_per_texture), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "conditional_render", flat_offset: core::mem::offset_of!(FlatCaps, conditional_render), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_barrier", flat_offset: core::mem::offset_of!(FlatCaps, texture_barrier), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "stream_output_pause_resume", flat_offset: core::mem::offset_of!(FlatCaps, stream_output_pause_resume), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "tgsi_can_compact_constants", flat_offset: core::mem::offset_of!(FlatCaps, tgsi_can_compact_constants), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "vertex_color_unclamped", flat_offset: core::mem::offset_of!(FlatCaps, vertex_color_unclamped), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "vertex_color_clamped", flat_offset: core::mem::offset_of!(FlatCaps, vertex_color_clamped), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "quads_follow_provoking_vertex_convention", flat_offset: core::mem::offset_of!(FlatCaps, quads_follow_provoking_vertex_convention), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "user_vertex_buffers", flat_offset: core::mem::offset_of!(FlatCaps, user_vertex_buffers), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "compute", flat_offset: core::mem::offset_of!(FlatCaps, compute), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "start_instance", flat_offset: core::mem::offset_of!(FlatCaps, start_instance), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "query_timestamp", flat_offset: core::mem::offset_of!(FlatCaps, query_timestamp), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_multisample", flat_offset: core::mem::offset_of!(FlatCaps, texture_multisample), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "cube_map_array", flat_offset: core::mem::offset_of!(FlatCaps, cube_map_array), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_buffer_objects", flat_offset: core::mem::offset_of!(FlatCaps, texture_buffer_objects), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "buffer_sampler_view_rgba_only", flat_offset: core::mem::offset_of!(FlatCaps, buffer_sampler_view_rgba_only), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "tgsi_texcoord", flat_offset: core::mem::offset_of!(FlatCaps, tgsi_texcoord), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "query_pipeline_statistics", flat_offset: core::mem::offset_of!(FlatCaps, query_pipeline_statistics), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "mixed_framebuffer_sizes", flat_offset: core::mem::offset_of!(FlatCaps, mixed_framebuffer_sizes), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "vs_layer_viewport", flat_offset: core::mem::offset_of!(FlatCaps, vs_layer_viewport), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_gather_sm5", flat_offset: core::mem::offset_of!(FlatCaps, texture_gather_sm5), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "buffer_map_persistent_coherent", flat_offset: core::mem::offset_of!(FlatCaps, buffer_map_persistent_coherent), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fake_sw_msaa", flat_offset: core::mem::offset_of!(FlatCaps, fake_sw_msaa), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_query_lod", flat_offset: core::mem::offset_of!(FlatCaps, texture_query_lod), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "sample_shading", flat_offset: core::mem::offset_of!(FlatCaps, sample_shading), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_gather_offsets", flat_offset: core::mem::offset_of!(FlatCaps, texture_gather_offsets), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "vs_window_space_position", flat_offset: core::mem::offset_of!(FlatCaps, vs_window_space_position), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "draw_indirect", flat_offset: core::mem::offset_of!(FlatCaps, draw_indirect), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fs_fine_derivative", flat_offset: core::mem::offset_of!(FlatCaps, fs_fine_derivative), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "uma", flat_offset: core::mem::offset_of!(FlatCaps, uma), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "conditional_render_inverted", flat_offset: core::mem::offset_of!(FlatCaps, conditional_render_inverted), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "sampler_view_target", flat_offset: core::mem::offset_of!(FlatCaps, sampler_view_target), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "clip_halfz", flat_offset: core::mem::offset_of!(FlatCaps, clip_halfz), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "polygon_offset_clamp", flat_offset: core::mem::offset_of!(FlatCaps, polygon_offset_clamp), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "multisample_z_resolve", flat_offset: core::mem::offset_of!(FlatCaps, multisample_z_resolve), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "resource_from_user_memory", flat_offset: core::mem::offset_of!(FlatCaps, resource_from_user_memory), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "resource_from_user_memory_compute_only", flat_offset: core::mem::offset_of!(FlatCaps, resource_from_user_memory_compute_only), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "device_reset_status_query", flat_offset: core::mem::offset_of!(FlatCaps, device_reset_status_query), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_float_linear", flat_offset: core::mem::offset_of!(FlatCaps, texture_float_linear), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_half_float_linear", flat_offset: core::mem::offset_of!(FlatCaps, texture_half_float_linear), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "depth_bounds_test", flat_offset: core::mem::offset_of!(FlatCaps, depth_bounds_test), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "native_fp32_depth", flat_offset: core::mem::offset_of!(FlatCaps, native_fp32_depth), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_query_samples", flat_offset: core::mem::offset_of!(FlatCaps, texture_query_samples), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "force_persample_interp", flat_offset: core::mem::offset_of!(FlatCaps, force_persample_interp), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shareable_shaders", flat_offset: core::mem::offset_of!(FlatCaps, shareable_shaders), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "copy_between_compressed_and_plain_formats", flat_offset: core::mem::offset_of!(FlatCaps, copy_between_compressed_and_plain_formats), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "clear_scissored", flat_offset: core::mem::offset_of!(FlatCaps, clear_scissored), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "clear_masked", flat_offset: core::mem::offset_of!(FlatCaps, clear_masked), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "draw_parameters", flat_offset: core::mem::offset_of!(FlatCaps, draw_parameters), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_pack_half_float", flat_offset: core::mem::offset_of!(FlatCaps, shader_pack_half_float), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "multi_draw_indirect", flat_offset: core::mem::offset_of!(FlatCaps, multi_draw_indirect), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "multi_draw_indirect_params", flat_offset: core::mem::offset_of!(FlatCaps, multi_draw_indirect_params), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "multi_draw_indirect_partial_stride", flat_offset: core::mem::offset_of!(FlatCaps, multi_draw_indirect_partial_stride), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fs_position_is_sysval", flat_offset: core::mem::offset_of!(FlatCaps, fs_position_is_sysval), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fs_point_is_sysval", flat_offset: core::mem::offset_of!(FlatCaps, fs_point_is_sysval), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fs_face_is_integer_sysval", flat_offset: core::mem::offset_of!(FlatCaps, fs_face_is_integer_sysval), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "invalidate_buffer", flat_offset: core::mem::offset_of!(FlatCaps, invalidate_buffer), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "generate_mipmap", flat_offset: core::mem::offset_of!(FlatCaps, generate_mipmap), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "string_marker", flat_offset: core::mem::offset_of!(FlatCaps, string_marker), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "surface_no_compress", flat_offset: core::mem::offset_of!(FlatCaps, surface_no_compress), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "surface_reinterpret_blocks", flat_offset: core::mem::offset_of!(FlatCaps, surface_reinterpret_blocks), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "compressed_surface_reinterpret_blocks_layered", flat_offset: core::mem::offset_of!(FlatCaps, compressed_surface_reinterpret_blocks_layered), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "query_buffer_object", flat_offset: core::mem::offset_of!(FlatCaps, query_buffer_object), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "query_memory_info", flat_offset: core::mem::offset_of!(FlatCaps, query_memory_info), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "framebuffer_no_attachment", flat_offset: core::mem::offset_of!(FlatCaps, framebuffer_no_attachment), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "robust_buffer_access_behavior", flat_offset: core::mem::offset_of!(FlatCaps, robust_buffer_access_behavior), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "cull_distance", flat_offset: core::mem::offset_of!(FlatCaps, cull_distance), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_group_vote", flat_offset: core::mem::offset_of!(FlatCaps, shader_group_vote), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_array_components", flat_offset: core::mem::offset_of!(FlatCaps, shader_array_components), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "stream_output_interleave_buffers", flat_offset: core::mem::offset_of!(FlatCaps, stream_output_interleave_buffers), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "native_fence_fd", flat_offset: core::mem::offset_of!(FlatCaps, native_fence_fd), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "glsl_tess_levels_as_inputs", flat_offset: core::mem::offset_of!(FlatCaps, glsl_tess_levels_as_inputs), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "legacy_math_rules", flat_offset: core::mem::offset_of!(FlatCaps, legacy_math_rules), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fp16", flat_offset: core::mem::offset_of!(FlatCaps, fp16), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "doubles", flat_offset: core::mem::offset_of!(FlatCaps, doubles), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "int64", flat_offset: core::mem::offset_of!(FlatCaps, int64), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_clock", flat_offset: core::mem::offset_of!(FlatCaps, shader_clock), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_realtime_clock", flat_offset: core::mem::offset_of!(FlatCaps, shader_realtime_clock), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "polygon_mode_fill_rectangle", flat_offset: core::mem::offset_of!(FlatCaps, polygon_mode_fill_rectangle), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_ballot", flat_offset: core::mem::offset_of!(FlatCaps, shader_ballot), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "tes_layer_viewport", flat_offset: core::mem::offset_of!(FlatCaps, tes_layer_viewport), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "can_bind_const_buffer_as_vertex", flat_offset: core::mem::offset_of!(FlatCaps, can_bind_const_buffer_as_vertex), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "allow_mapped_buffers_during_execution", flat_offset: core::mem::offset_of!(FlatCaps, allow_mapped_buffers_during_execution), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "post_depth_coverage", flat_offset: core::mem::offset_of!(FlatCaps, post_depth_coverage), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "bindless_texture", flat_offset: core::mem::offset_of!(FlatCaps, bindless_texture), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "nir_samplers_as_deref", flat_offset: core::mem::offset_of!(FlatCaps, nir_samplers_as_deref), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "query_so_overflow", flat_offset: core::mem::offset_of!(FlatCaps, query_so_overflow), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "memobj", flat_offset: core::mem::offset_of!(FlatCaps, memobj), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "load_constbuf", flat_offset: core::mem::offset_of!(FlatCaps, load_constbuf), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "tile_raster_order", flat_offset: core::mem::offset_of!(FlatCaps, tile_raster_order), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "signed_vertex_buffer_offset", flat_offset: core::mem::offset_of!(FlatCaps, signed_vertex_buffer_offset), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fence_signal", flat_offset: core::mem::offset_of!(FlatCaps, fence_signal), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "packed_uniforms", flat_offset: core::mem::offset_of!(FlatCaps, packed_uniforms), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "conservative_raster_post_snap_triangles", flat_offset: core::mem::offset_of!(FlatCaps, conservative_raster_post_snap_triangles), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "conservative_raster_post_snap_points_lines", flat_offset: core::mem::offset_of!(FlatCaps, conservative_raster_post_snap_points_lines), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "conservative_raster_pre_snap_triangles", flat_offset: core::mem::offset_of!(FlatCaps, conservative_raster_pre_snap_triangles), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "conservative_raster_pre_snap_points_lines", flat_offset: core::mem::offset_of!(FlatCaps, conservative_raster_pre_snap_points_lines), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "conservative_raster_post_depth_coverage", flat_offset: core::mem::offset_of!(FlatCaps, conservative_raster_post_depth_coverage), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "conservative_raster_inner_coverage", flat_offset: core::mem::offset_of!(FlatCaps, conservative_raster_inner_coverage), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "programmable_sample_locations", flat_offset: core::mem::offset_of!(FlatCaps, programmable_sample_locations), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_mirror_clamp_to_edge", flat_offset: core::mem::offset_of!(FlatCaps, texture_mirror_clamp_to_edge), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "surface_sample_count", flat_offset: core::mem::offset_of!(FlatCaps, surface_sample_count), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "image_atomic_float_add", flat_offset: core::mem::offset_of!(FlatCaps, image_atomic_float_add), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "query_pipeline_statistics_single", flat_offset: core::mem::offset_of!(FlatCaps, query_pipeline_statistics_single), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "dest_surface_srgb_control", flat_offset: core::mem::offset_of!(FlatCaps, dest_surface_srgb_control), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "compute_grid_info_last_block", flat_offset: core::mem::offset_of!(FlatCaps, compute_grid_info_last_block), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "compute_shader_derivatives", flat_offset: core::mem::offset_of!(FlatCaps, compute_shader_derivatives), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "image_load_formatted", flat_offset: core::mem::offset_of!(FlatCaps, image_load_formatted), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "image_store_formatted", flat_offset: core::mem::offset_of!(FlatCaps, image_store_formatted), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "throttle", flat_offset: core::mem::offset_of!(FlatCaps, throttle), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "cl_gl_sharing", flat_offset: core::mem::offset_of!(FlatCaps, cl_gl_sharing), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "prefer_compute_for_multimedia", flat_offset: core::mem::offset_of!(FlatCaps, prefer_compute_for_multimedia), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fragment_shader_interlock", flat_offset: core::mem::offset_of!(FlatCaps, fragment_shader_interlock), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fbfetch_coherent", flat_offset: core::mem::offset_of!(FlatCaps, fbfetch_coherent), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "atomic_float_minmax", flat_offset: core::mem::offset_of!(FlatCaps, atomic_float_minmax), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fragment_shader_texture_lod", flat_offset: core::mem::offset_of!(FlatCaps, fragment_shader_texture_lod), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fragment_shader_derivatives", flat_offset: core::mem::offset_of!(FlatCaps, fragment_shader_derivatives), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fragment_shader_depth", flat_offset: core::mem::offset_of!(FlatCaps, fragment_shader_depth), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_shadow_lod", flat_offset: core::mem::offset_of!(FlatCaps, texture_shadow_lod), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_samples_identical", flat_offset: core::mem::offset_of!(FlatCaps, shader_samples_identical), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "image_atomic_inc_wrap", flat_offset: core::mem::offset_of!(FlatCaps, image_atomic_inc_wrap), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "prefer_imm_arrays_as_constbuf", flat_offset: core::mem::offset_of!(FlatCaps, prefer_imm_arrays_as_constbuf), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "gl_spirv", flat_offset: core::mem::offset_of!(FlatCaps, gl_spirv), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "gl_spirv_variable_pointers", flat_offset: core::mem::offset_of!(FlatCaps, gl_spirv_variable_pointers), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "demote_to_helper_invocation", flat_offset: core::mem::offset_of!(FlatCaps, demote_to_helper_invocation), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "tgsi_tg4_component_in_swizzle", flat_offset: core::mem::offset_of!(FlatCaps, tgsi_tg4_component_in_swizzle), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "flatshade", flat_offset: core::mem::offset_of!(FlatCaps, flatshade), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "alpha_test", flat_offset: core::mem::offset_of!(FlatCaps, alpha_test), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "two_sided_color", flat_offset: core::mem::offset_of!(FlatCaps, two_sided_color), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "opencl_integer_functions", flat_offset: core::mem::offset_of!(FlatCaps, opencl_integer_functions), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "integer_multiply_32x16", flat_offset: core::mem::offset_of!(FlatCaps, integer_multiply_32x16), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "frontend_noop", flat_offset: core::mem::offset_of!(FlatCaps, frontend_noop), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "nir_images_as_deref", flat_offset: core::mem::offset_of!(FlatCaps, nir_images_as_deref), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "packed_stream_output", flat_offset: core::mem::offset_of!(FlatCaps, packed_stream_output), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "viewport_transform_lowered", flat_offset: core::mem::offset_of!(FlatCaps, viewport_transform_lowered), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "psiz_clamped", flat_offset: core::mem::offset_of!(FlatCaps, psiz_clamped), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "viewport_swizzle", flat_offset: core::mem::offset_of!(FlatCaps, viewport_swizzle), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "system_svm", flat_offset: core::mem::offset_of!(FlatCaps, system_svm), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "viewport_mask", flat_offset: core::mem::offset_of!(FlatCaps, viewport_mask), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "alpha_to_coverage_dither_control", flat_offset: core::mem::offset_of!(FlatCaps, alpha_to_coverage_dither_control), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "map_unsynchronized_thread_safe", flat_offset: core::mem::offset_of!(FlatCaps, map_unsynchronized_thread_safe), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "nir_atomics_as_deref", flat_offset: core::mem::offset_of!(FlatCaps, nir_atomics_as_deref), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "no_clip_on_copy_tex", flat_offset: core::mem::offset_of!(FlatCaps, no_clip_on_copy_tex), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_atomic_int64", flat_offset: core::mem::offset_of!(FlatCaps, shader_atomic_int64), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "device_protected_surface", flat_offset: core::mem::offset_of!(FlatCaps, device_protected_surface), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "prefer_real_buffer_in_constbuf0", flat_offset: core::mem::offset_of!(FlatCaps, prefer_real_buffer_in_constbuf0), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "gl_clamp", flat_offset: core::mem::offset_of!(FlatCaps, gl_clamp), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texrect", flat_offset: core::mem::offset_of!(FlatCaps, texrect), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "sampler_reduction_minmax", flat_offset: core::mem::offset_of!(FlatCaps, sampler_reduction_minmax), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "sampler_reduction_minmax_arb", flat_offset: core::mem::offset_of!(FlatCaps, sampler_reduction_minmax_arb), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "allow_dynamic_vao_fastpath", flat_offset: core::mem::offset_of!(FlatCaps, allow_dynamic_vao_fastpath), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "emulate_nonfixed_primitive_restart", flat_offset: core::mem::offset_of!(FlatCaps, emulate_nonfixed_primitive_restart), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "prefer_back_buffer_reuse", flat_offset: core::mem::offset_of!(FlatCaps, prefer_back_buffer_reuse), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "draw_vertex_state", flat_offset: core::mem::offset_of!(FlatCaps, draw_vertex_state), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "prefer_pot_aligned_varyings", flat_offset: core::mem::offset_of!(FlatCaps, prefer_pot_aligned_varyings), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "sparse_texture_full_array_cube_mipmaps", flat_offset: core::mem::offset_of!(FlatCaps, sparse_texture_full_array_cube_mipmaps), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "query_sparse_texture_residency", flat_offset: core::mem::offset_of!(FlatCaps, query_sparse_texture_residency), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "clamp_sparse_texture_lod", flat_offset: core::mem::offset_of!(FlatCaps, clamp_sparse_texture_lod), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "allow_draw_out_of_order", flat_offset: core::mem::offset_of!(FlatCaps, allow_draw_out_of_order), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "hardware_gl_select", flat_offset: core::mem::offset_of!(FlatCaps, hardware_gl_select), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "dithering", flat_offset: core::mem::offset_of!(FlatCaps, dithering), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fbfetch_zs", flat_offset: core::mem::offset_of!(FlatCaps, fbfetch_zs), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "timeline_semaphore_import", flat_offset: core::mem::offset_of!(FlatCaps, timeline_semaphore_import), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "device_protected_context", flat_offset: core::mem::offset_of!(FlatCaps, device_protected_context), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "allow_glthread_buffer_subdata_opt", flat_offset: core::mem::offset_of!(FlatCaps, allow_glthread_buffer_subdata_opt), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "null_textures", flat_offset: core::mem::offset_of!(FlatCaps, null_textures), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "astc_void_extents_need_denorm_flush", flat_offset: core::mem::offset_of!(FlatCaps, astc_void_extents_need_denorm_flush), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "validate_all_dirty_states", flat_offset: core::mem::offset_of!(FlatCaps, validate_all_dirty_states), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "has_const_bw", flat_offset: core::mem::offset_of!(FlatCaps, has_const_bw), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "performance_monitor", flat_offset: core::mem::offset_of!(FlatCaps, performance_monitor), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_sampler_independent", flat_offset: core::mem::offset_of!(FlatCaps, texture_sampler_independent), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "astc_decode_mode", flat_offset: core::mem::offset_of!(FlatCaps, astc_decode_mode), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_subgroup_quad_all_stages", flat_offset: core::mem::offset_of!(FlatCaps, shader_subgroup_quad_all_stages), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "call_finalize_nir_in_linker", flat_offset: core::mem::offset_of!(FlatCaps, call_finalize_nir_in_linker), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "mesh_shader", flat_offset: core::mem::offset_of!(FlatCaps, mesh_shader), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "representative_fragment_test", flat_offset: core::mem::offset_of!(FlatCaps, representative_fragment_test), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "prefer_persp", flat_offset: core::mem::offset_of!(FlatCaps, prefer_persp), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "blit_3d", flat_offset: core::mem::offset_of!(FlatCaps, blit_3d), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "glsl_bindless_handles_are_32bit", flat_offset: core::mem::offset_of!(FlatCaps, glsl_bindless_handles_are_32bit), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "polygon_stipple", flat_offset: core::mem::offset_of!(FlatCaps, polygon_stipple), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "disable_custom_border_color_caching", flat_offset: core::mem::offset_of!(FlatCaps, disable_custom_border_color_caching), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "needs_split_planar_samplers", flat_offset: core::mem::offset_of!(FlatCaps, needs_split_planar_samplers), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "accelerated", flat_offset: core::mem::offset_of!(FlatCaps, accelerated), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "min_texel_offset", flat_offset: core::mem::offset_of!(FlatCaps, min_texel_offset), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texel_offset", flat_offset: core::mem::offset_of!(FlatCaps, max_texel_offset), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "min_texture_gather_offset", flat_offset: core::mem::offset_of!(FlatCaps, min_texture_gather_offset), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_gather_offset", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_gather_offset), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_dual_source_render_targets", flat_offset: core::mem::offset_of!(FlatCaps, max_dual_source_render_targets), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_render_targets", flat_offset: core::mem::offset_of!(FlatCaps, max_render_targets), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_2d_size", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_2d_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_3d_levels", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_3d_levels), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_cube_levels", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_cube_levels), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_stream_output_buffers", flat_offset: core::mem::offset_of!(FlatCaps, max_stream_output_buffers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_array_layers", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_array_layers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_stream_output_separate_components", flat_offset: core::mem::offset_of!(FlatCaps, max_stream_output_separate_components), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_stream_output_interleaved_components", flat_offset: core::mem::offset_of!(FlatCaps, max_stream_output_interleaved_components), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "glsl_feature_level", flat_offset: core::mem::offset_of!(FlatCaps, glsl_feature_level), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "glsl_feature_level_compatibility", flat_offset: core::mem::offset_of!(FlatCaps, glsl_feature_level_compatibility), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "essl_feature_level", flat_offset: core::mem::offset_of!(FlatCaps, essl_feature_level), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "constant_buffer_offset_alignment", flat_offset: core::mem::offset_of!(FlatCaps, constant_buffer_offset_alignment), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "timer_resolution", flat_offset: core::mem::offset_of!(FlatCaps, timer_resolution), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "min_map_buffer_alignment", flat_offset: core::mem::offset_of!(FlatCaps, min_map_buffer_alignment), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_buffer_offset_alignment", flat_offset: core::mem::offset_of!(FlatCaps, texture_buffer_offset_alignment), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "linear_image_pitch_alignment", flat_offset: core::mem::offset_of!(FlatCaps, linear_image_pitch_alignment), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "linear_image_base_address_alignment", flat_offset: core::mem::offset_of!(FlatCaps, linear_image_base_address_alignment), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_transfer_modes", flat_offset: core::mem::offset_of!(FlatCaps, texture_transfer_modes), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "texture_border_color_quirk", flat_offset: core::mem::offset_of!(FlatCaps, texture_border_color_quirk), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texel_buffer_elements", flat_offset: core::mem::offset_of!(FlatCaps, max_texel_buffer_elements), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_viewports", flat_offset: core::mem::offset_of!(FlatCaps, max_viewports), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_geometry_output_vertices", flat_offset: core::mem::offset_of!(FlatCaps, max_geometry_output_vertices), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_geometry_total_output_components", flat_offset: core::mem::offset_of!(FlatCaps, max_geometry_total_output_components), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_gather_components", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_gather_components), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_vertex_streams", flat_offset: core::mem::offset_of!(FlatCaps, max_vertex_streams), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "vendor_id", flat_offset: core::mem::offset_of!(FlatCaps, vendor_id), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "device_id", flat_offset: core::mem::offset_of!(FlatCaps, device_id), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "video_memory", flat_offset: core::mem::offset_of!(FlatCaps, video_memory), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_vertex_attrib_stride", flat_offset: core::mem::offset_of!(FlatCaps, max_vertex_attrib_stride), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_shader_patch_varyings", flat_offset: core::mem::offset_of!(FlatCaps, max_shader_patch_varyings), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_buffer_offset_alignment", flat_offset: core::mem::offset_of!(FlatCaps, shader_buffer_offset_alignment), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "pci_group", flat_offset: core::mem::offset_of!(FlatCaps, pci_group), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "pci_bus", flat_offset: core::mem::offset_of!(FlatCaps, pci_bus), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "pci_device", flat_offset: core::mem::offset_of!(FlatCaps, pci_device), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "pci_function", flat_offset: core::mem::offset_of!(FlatCaps, pci_function), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_window_rectangles", flat_offset: core::mem::offset_of!(FlatCaps, max_window_rectangles), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "viewport_subpixel_bits", flat_offset: core::mem::offset_of!(FlatCaps, viewport_subpixel_bits), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "rasterizer_subpixel_bits", flat_offset: core::mem::offset_of!(FlatCaps, rasterizer_subpixel_bits), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "mixed_color_depth_bits", flat_offset: core::mem::offset_of!(FlatCaps, mixed_color_depth_bits), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "fbfetch", flat_offset: core::mem::offset_of!(FlatCaps, fbfetch), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "blend_equation_advanced", flat_offset: core::mem::offset_of!(FlatCaps, blend_equation_advanced), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "sparse_buffer_page_size", flat_offset: core::mem::offset_of!(FlatCaps, sparse_buffer_page_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_combined_shader_output_resources", flat_offset: core::mem::offset_of!(FlatCaps, max_combined_shader_output_resources), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "framebuffer_msaa_constraints", flat_offset: core::mem::offset_of!(FlatCaps, framebuffer_msaa_constraints), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "context_priority_mask", flat_offset: core::mem::offset_of!(FlatCaps, context_priority_mask), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "constbuf0_flags", flat_offset: core::mem::offset_of!(FlatCaps, constbuf0_flags), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_conservative_raster_subpixel_precision_bias", flat_offset: core::mem::offset_of!(FlatCaps, max_conservative_raster_subpixel_precision_bias), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_gs_invocations", flat_offset: core::mem::offset_of!(FlatCaps, max_gs_invocations), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_shader_buffer_size", flat_offset: core::mem::offset_of!(FlatCaps, max_shader_buffer_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_combined_shader_buffers", flat_offset: core::mem::offset_of!(FlatCaps, max_combined_shader_buffers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_combined_image_uniforms", flat_offset: core::mem::offset_of!(FlatCaps, max_combined_image_uniforms), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_combined_hw_atomic_counters", flat_offset: core::mem::offset_of!(FlatCaps, max_combined_hw_atomic_counters), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_combined_hw_atomic_counter_buffers", flat_offset: core::mem::offset_of!(FlatCaps, max_combined_hw_atomic_counter_buffers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_upload_memory_budget", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_upload_memory_budget), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_vertex_element_src_offset", flat_offset: core::mem::offset_of!(FlatCaps, max_vertex_element_src_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_varyings", flat_offset: core::mem::offset_of!(FlatCaps, max_varyings), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "ignored_output_varyings", flat_offset: core::mem::offset_of!(FlatCaps, ignored_output_varyings), flat_size: core::mem::size_of::<[u64; 8]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "dmabuf", flat_offset: core::mem::offset_of!(FlatCaps, dmabuf), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "clip_planes", flat_offset: core::mem::offset_of!(FlatCaps, clip_planes), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_vertex_buffers", flat_offset: core::mem::offset_of!(FlatCaps, max_vertex_buffers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "gl_begin_end_buffer_size", flat_offset: core::mem::offset_of!(FlatCaps, gl_begin_end_buffer_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "glsl_zero_init", flat_offset: core::mem::offset_of!(FlatCaps, glsl_zero_init), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_mb", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_mb), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "supported_prim_modes", flat_offset: core::mem::offset_of!(FlatCaps, supported_prim_modes), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "supported_prim_modes_with_restart", flat_offset: core::mem::offset_of!(FlatCaps, supported_prim_modes_with_restart), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_sparse_texture_size", flat_offset: core::mem::offset_of!(FlatCaps, max_sparse_texture_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_sparse_3d_texture_size", flat_offset: core::mem::offset_of!(FlatCaps, max_sparse_3d_texture_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_sparse_array_texture_layers", flat_offset: core::mem::offset_of!(FlatCaps, max_sparse_array_texture_layers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_constant_buffer_size", flat_offset: core::mem::offset_of!(FlatCaps, max_constant_buffer_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "query_timestamp_bits", flat_offset: core::mem::offset_of!(FlatCaps, query_timestamp_bits), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_subgroup_size", flat_offset: core::mem::offset_of!(FlatCaps, shader_subgroup_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_subgroup_supported_stages", flat_offset: core::mem::offset_of!(FlatCaps, shader_subgroup_supported_stages), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_subgroup_supported_features", flat_offset: core::mem::offset_of!(FlatCaps, shader_subgroup_supported_features), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_pixel_local_storage_size", flat_offset: core::mem::offset_of!(FlatCaps, shader_pixel_local_storage_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "shader_pixel_local_storage_fast_size", flat_offset: core::mem::offset_of!(FlatCaps, shader_pixel_local_storage_fast_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "multiview", flat_offset: core::mem::offset_of!(FlatCaps, multiview), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_label_length", flat_offset: core::mem::offset_of!(FlatCaps, max_label_length), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_timeline_semaphore_difference", flat_offset: core::mem::offset_of!(FlatCaps, max_timeline_semaphore_difference), flat_size: core::mem::size_of::<u64>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "map_client_pointer", flat_offset: core::mem::offset_of!(FlatCaps, map_client_pointer), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "min_vma", flat_offset: core::mem::offset_of!(FlatCaps, min_vma), flat_size: core::mem::size_of::<u64>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_vma", flat_offset: core::mem::offset_of!(FlatCaps, max_vma), flat_size: core::mem::size_of::<u64>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "hw_clear_buffer_sizes", flat_offset: core::mem::offset_of!(FlatCaps, hw_clear_buffer_sizes), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "device_type", flat_offset: core::mem::offset_of!(FlatCaps, device_type), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_caps", field: "vertex_input_alignment", flat_offset: core::mem::offset_of!(FlatCaps, vertex_input_alignment), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_caps", field: "endianness", flat_offset: core::mem::offset_of!(FlatCaps, endianness), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_caps", field: "point_size_fixed", flat_offset: core::mem::offset_of!(FlatCaps, point_size_fixed), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_caps", field: "min_line_width", flat_offset: core::mem::offset_of!(FlatCaps, min_line_width), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "min_line_width_aa", flat_offset: core::mem::offset_of!(FlatCaps, min_line_width_aa), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_line_width", flat_offset: core::mem::offset_of!(FlatCaps, max_line_width), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_line_width_aa", flat_offset: core::mem::offset_of!(FlatCaps, max_line_width_aa), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "line_width_granularity", flat_offset: core::mem::offset_of!(FlatCaps, line_width_granularity), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "min_point_size", flat_offset: core::mem::offset_of!(FlatCaps, min_point_size), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "min_point_size_aa", flat_offset: core::mem::offset_of!(FlatCaps, min_point_size_aa), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_point_size", flat_offset: core::mem::offset_of!(FlatCaps, max_point_size), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_point_size_aa", flat_offset: core::mem::offset_of!(FlatCaps, max_point_size_aa), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "point_size_granularity", flat_offset: core::mem::offset_of!(FlatCaps, point_size_granularity), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_anisotropy", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_anisotropy), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_texture_lod_bias", flat_offset: core::mem::offset_of!(FlatCaps, max_texture_lod_bias), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "min_conservative_raster_dilate", flat_offset: core::mem::offset_of!(FlatCaps, min_conservative_raster_dilate), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "max_conservative_raster_dilate", flat_offset: core::mem::offset_of!(FlatCaps, max_conservative_raster_dilate), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "conservative_raster_dilate_granularity", flat_offset: core::mem::offset_of!(FlatCaps, conservative_raster_dilate_granularity), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_caps", field: "mesh", flat_offset: core::mem::offset_of!(FlatCaps, mesh), flat_size: core::mem::size_of::<FlatMeshCaps>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_clip_state", field: "ucp", flat_offset: core::mem::offset_of!(FlatClipState, ucp), flat_size: core::mem::size_of::<[[f32; 4]; 8]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "address_bits", flat_offset: core::mem::offset_of!(FlatComputeCaps, address_bits), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "grid_dimension", flat_offset: core::mem::offset_of!(FlatComputeCaps, grid_dimension), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_grid_size", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_grid_size), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_block_size", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_block_size), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_threads_per_block", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_threads_per_block), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_local_size", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_local_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_clock_frequency", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_clock_frequency), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_compute_units", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_compute_units), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_subgroups", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_subgroups), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "subgroup_sizes", flat_offset: core::mem::offset_of!(FlatComputeCaps, subgroup_sizes), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_variable_threads_per_block", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_variable_threads_per_block), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_mem_alloc_size", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_mem_alloc_size), flat_size: core::mem::size_of::<u64>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_caps", field: "max_global_size", flat_offset: core::mem::offset_of!(FlatComputeCaps, max_global_size), flat_size: core::mem::size_of::<u64>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_state", field: "static_shared_mem", flat_offset: core::mem::offset_of!(FlatComputeState, static_shared_mem), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_constant_buffer", field: "buffer", flat_offset: core::mem::offset_of!(FlatConstantBuffer, buffer), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_constant_buffer", field: "buffer_offset", flat_offset: core::mem::offset_of!(FlatConstantBuffer, buffer_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_constant_buffer", field: "buffer_size", flat_offset: core::mem::offset_of!(FlatConstantBuffer, buffer_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_constant_buffer", field: "user_buffer", flat_offset: core::mem::offset_of!(FlatConstantBuffer, user_buffer), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "stencil", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, stencil), flat_size: core::mem::size_of::<[FlatStencilState; 2]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "alpha_enabled", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, alpha_enabled), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "alpha_func", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, alpha_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_enabled", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_enabled), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_writemask", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_writemask), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_func", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_bounds_test", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_bounds_test), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "alpha_ref_value", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, alpha_ref_value), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_bounds_min", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_bounds_min), flat_size: core::mem::size_of::<f64>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_bounds_max", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_bounds_max), flat_size: core::mem::size_of::<f64>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "offset", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "stride", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, stride), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "draw_count", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, draw_count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "indirect_draw_count_offset", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, indirect_draw_count_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "buffer", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, buffer), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "indirect_draw_count", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, indirect_draw_count), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "count_from_stream_output", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, count_from_stream_output), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "mode", flat_offset: core::mem::offset_of!(FlatDrawInfo, mode), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_draw_info", field: "index_size", flat_offset: core::mem::offset_of!(FlatDrawInfo, index_size), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "primitive_restart", flat_offset: core::mem::offset_of!(FlatDrawInfo, primitive_restart), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "has_user_indices", flat_offset: core::mem::offset_of!(FlatDrawInfo, has_user_indices), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "index_bounds_valid", flat_offset: core::mem::offset_of!(FlatDrawInfo, index_bounds_valid), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "increment_draw_id", flat_offset: core::mem::offset_of!(FlatDrawInfo, increment_draw_id), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "index_bias_varies", flat_offset: core::mem::offset_of!(FlatDrawInfo, index_bias_varies), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "was_line_loop", flat_offset: core::mem::offset_of!(FlatDrawInfo, was_line_loop), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "_pad", flat_offset: core::mem::offset_of!(FlatDrawInfo, _pad), flat_size: core::mem::size_of::<u8>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "start_instance", flat_offset: core::mem::offset_of!(FlatDrawInfo, start_instance), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "instance_count", flat_offset: core::mem::offset_of!(FlatDrawInfo, instance_count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "restart_index", flat_offset: core::mem::offset_of!(FlatDrawInfo, restart_index), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "index", flat_offset: core::mem::offset_of!(FlatDrawInfo, index), flat_size: core::mem::size_of::<FlatDrawInfoIndex>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "min_index", flat_offset: core::mem::offset_of!(FlatDrawInfo, min_index), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "max_index", flat_offset: core::mem::offset_of!(FlatDrawInfo, max_index), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info.index", field: "resource", flat_offset: core::mem::offset_of!(FlatDrawInfoIndex, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info.index", field: "user", flat_offset: core::mem::offset_of!(FlatDrawInfoIndex, user), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_start_count_bias", field: "start", flat_offset: core::mem::offset_of!(FlatDrawStartCountBias, start), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_start_count_bias", field: "count", flat_offset: core::mem::offset_of!(FlatDrawStartCountBias, count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_start_count_bias", field: "index_bias", flat_offset: core::mem::offset_of!(FlatDrawStartCountBias, index_bias), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "width", flat_offset: core::mem::offset_of!(FlatFramebufferState, width), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "height", flat_offset: core::mem::offset_of!(FlatFramebufferState, height), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "layers", flat_offset: core::mem::offset_of!(FlatFramebufferState, layers), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "samples", flat_offset: core::mem::offset_of!(FlatFramebufferState, samples), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "nr_cbufs", flat_offset: core::mem::offset_of!(FlatFramebufferState, nr_cbufs), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "pls_enabled", flat_offset: core::mem::offset_of!(FlatFramebufferState, pls_enabled), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "downscale_cbufs", flat_offset: core::mem::offset_of!(FlatFramebufferState, downscale_cbufs), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "_pad", flat_offset: core::mem::offset_of!(FlatFramebufferState, _pad), flat_size: core::mem::size_of::<u8>(), bits: 6, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "viewmask", flat_offset: core::mem::offset_of!(FlatFramebufferState, viewmask), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "cbufs", flat_offset: core::mem::offset_of!(FlatFramebufferState, cbufs), flat_size: core::mem::size_of::<[FlatSurface; 8]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "zsbuf", flat_offset: core::mem::offset_of!(FlatFramebufferState, zsbuf), flat_size: core::mem::size_of::<FlatSurface>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "resolve", flat_offset: core::mem::offset_of!(FlatFramebufferState, resolve), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "variable_shared_mem", flat_offset: core::mem::offset_of!(FlatGridInfo, variable_shared_mem), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "work_dim", flat_offset: core::mem::offset_of!(FlatGridInfo, work_dim), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "block", flat_offset: core::mem::offset_of!(FlatGridInfo, block), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "last_block", flat_offset: core::mem::offset_of!(FlatGridInfo, last_block), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "grid", flat_offset: core::mem::offset_of!(FlatGridInfo, grid), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "grid_base", flat_offset: core::mem::offset_of!(FlatGridInfo, grid_base), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect_offset", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect_stride", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect_stride), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "draw_count", flat_offset: core::mem::offset_of!(FlatGridInfo, draw_count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect_draw_count_offset", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect_draw_count_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect_draw_count", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect_draw_count), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view", field: "resource", flat_offset: core::mem::offset_of!(FlatImageView, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view", field: "format", flat_offset: core::mem::offset_of!(FlatImageView, format), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_image_view", field: "access", flat_offset: core::mem::offset_of!(FlatImageView, access), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view", field: "shader_access", flat_offset: core::mem::offset_of!(FlatImageView, shader_access), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view", field: "u", flat_offset: core::mem::offset_of!(FlatImageView, u), flat_size: core::mem::size_of::<FlatImageViewU>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u", field: "tex", flat_offset: core::mem::offset_of!(FlatImageViewU, tex), flat_size: core::mem::size_of::<FlatImageViewUTex>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u", field: "buf", flat_offset: core::mem::offset_of!(FlatImageViewU, buf), flat_size: core::mem::size_of::<FlatImageViewUBuf>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u", field: "tex2d_from_buf", flat_offset: core::mem::offset_of!(FlatImageViewU, tex2d_from_buf), flat_size: core::mem::size_of::<FlatTex2dFromBuf>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.buf", field: "offset", flat_offset: core::mem::offset_of!(FlatImageViewUBuf, offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.buf", field: "size", flat_offset: core::mem::offset_of!(FlatImageViewUBuf, size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "first_layer", flat_offset: core::mem::offset_of!(FlatImageViewUTex, first_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "last_layer", flat_offset: core::mem::offset_of!(FlatImageViewUTex, last_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "level", flat_offset: core::mem::offset_of!(FlatImageViewUTex, level), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "single_layer_view", flat_offset: core::mem::offset_of!(FlatImageViewUTex, single_layer_view), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "is_2d_view_of_3d", flat_offset: core::mem::offset_of!(FlatImageViewUTex, is_2d_view_of_3d), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_task_work_group_total_count", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_task_work_group_total_count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_work_group_total_count", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_work_group_total_count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_work_group_invocations", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_work_group_invocations), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_task_work_group_invocations", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_task_work_group_invocations), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_task_payload_size", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_task_payload_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_task_shared_memory_size", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_task_shared_memory_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_shared_memory_size", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_shared_memory_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_task_payload_and_shared_memory_size", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_task_payload_and_shared_memory_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_payload_and_shared_memory_size", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_payload_and_shared_memory_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_output_memory_size", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_output_memory_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_payload_and_output_memory_size", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_payload_and_output_memory_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_output_vertices", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_output_vertices), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_output_primitives", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_output_primitives), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_output_components", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_output_components), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_output_layers", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_output_layers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_multiview_view_count", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_multiview_view_count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "mesh_output_per_vertex_granularity", flat_offset: core::mem::offset_of!(FlatMeshCaps, mesh_output_per_vertex_granularity), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "mesh_output_per_primitive_granularity", flat_offset: core::mem::offset_of!(FlatMeshCaps, mesh_output_per_primitive_granularity), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_preferred_task_work_group_invocations", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_preferred_task_work_group_invocations), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_preferred_mesh_work_group_invocations", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_preferred_mesh_work_group_invocations), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "mesh_prefers_local_invocation_vertex_output", flat_offset: core::mem::offset_of!(FlatMeshCaps, mesh_prefers_local_invocation_vertex_output), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "mesh_prefers_local_invocation_primitive_output", flat_offset: core::mem::offset_of!(FlatMeshCaps, mesh_prefers_local_invocation_primitive_output), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "mesh_prefers_compact_vertex_output", flat_offset: core::mem::offset_of!(FlatMeshCaps, mesh_prefers_compact_vertex_output), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "mesh_prefers_compact_primitive_output", flat_offset: core::mem::offset_of!(FlatMeshCaps, mesh_prefers_compact_primitive_output), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_task_work_group_count", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_task_work_group_count), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_work_group_count", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_work_group_count), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_task_work_group_size", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_task_work_group_size), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "max_mesh_work_group_size", flat_offset: core::mem::offset_of!(FlatMeshCaps, max_mesh_work_group_size), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_mesh_caps", field: "pipeline_statistic_queries", flat_offset: core::mem::offset_of!(FlatMeshCaps, pipeline_statistic_queries), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_poly_stipple", field: "stipple", flat_offset: core::mem::offset_of!(FlatPolyStipple, stipple), flat_size: core::mem::size_of::<[u32; 32]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "flatshade", flat_offset: core::mem::offset_of!(FlatRasterizerState, flatshade), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "light_twoside", flat_offset: core::mem::offset_of!(FlatRasterizerState, light_twoside), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "clamp_vertex_color", flat_offset: core::mem::offset_of!(FlatRasterizerState, clamp_vertex_color), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "clamp_fragment_color", flat_offset: core::mem::offset_of!(FlatRasterizerState, clamp_fragment_color), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "front_ccw", flat_offset: core::mem::offset_of!(FlatRasterizerState, front_ccw), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "cull_face", flat_offset: core::mem::offset_of!(FlatRasterizerState, cull_face), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "fill_front", flat_offset: core::mem::offset_of!(FlatRasterizerState, fill_front), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "fill_back", flat_offset: core::mem::offset_of!(FlatRasterizerState, fill_back), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_point", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_point), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_line", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_line), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_tri", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_tri), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "scissor", flat_offset: core::mem::offset_of!(FlatRasterizerState, scissor), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "poly_smooth", flat_offset: core::mem::offset_of!(FlatRasterizerState, poly_smooth), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "poly_stipple_enable", flat_offset: core::mem::offset_of!(FlatRasterizerState, poly_stipple_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_smooth", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_smooth), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "sprite_coord_mode", flat_offset: core::mem::offset_of!(FlatRasterizerState, sprite_coord_mode), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_quad_rasterization", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_quad_rasterization), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_line_tri_clip", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_line_tri_clip), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_size_per_vertex", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_size_per_vertex), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "multisample", flat_offset: core::mem::offset_of!(FlatRasterizerState, multisample), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "no_ms_sample_mask_out", flat_offset: core::mem::offset_of!(FlatRasterizerState, no_ms_sample_mask_out), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "force_persample_interp", flat_offset: core::mem::offset_of!(FlatRasterizerState, force_persample_interp), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_smooth", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_smooth), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_stipple_enable", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_stipple_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_last_pixel", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_last_pixel), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_rectangular", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_rectangular), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "conservative_raster_mode", flat_offset: core::mem::offset_of!(FlatRasterizerState, conservative_raster_mode), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "flatshade_first", flat_offset: core::mem::offset_of!(FlatRasterizerState, flatshade_first), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "half_pixel_center", flat_offset: core::mem::offset_of!(FlatRasterizerState, half_pixel_center), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "bottom_edge_rule", flat_offset: core::mem::offset_of!(FlatRasterizerState, bottom_edge_rule), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "subpixel_precision_x", flat_offset: core::mem::offset_of!(FlatRasterizerState, subpixel_precision_x), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "subpixel_precision_y", flat_offset: core::mem::offset_of!(FlatRasterizerState, subpixel_precision_y), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "rasterizer_discard", flat_offset: core::mem::offset_of!(FlatRasterizerState, rasterizer_discard), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "rasterization_stream", flat_offset: core::mem::offset_of!(FlatRasterizerState, rasterization_stream), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "tile_raster_order_fixed", flat_offset: core::mem::offset_of!(FlatRasterizerState, tile_raster_order_fixed), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "tile_raster_order_increasing_x", flat_offset: core::mem::offset_of!(FlatRasterizerState, tile_raster_order_increasing_x), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "tile_raster_order_increasing_y", flat_offset: core::mem::offset_of!(FlatRasterizerState, tile_raster_order_increasing_y), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "depth_clip_near", flat_offset: core::mem::offset_of!(FlatRasterizerState, depth_clip_near), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "depth_clip_far", flat_offset: core::mem::offset_of!(FlatRasterizerState, depth_clip_far), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "depth_clamp", flat_offset: core::mem::offset_of!(FlatRasterizerState, depth_clamp), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "clip_halfz", flat_offset: core::mem::offset_of!(FlatRasterizerState, clip_halfz), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_units_unscaled", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_units_unscaled), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "unclamped_fragment_depth_values", flat_offset: core::mem::offset_of!(FlatRasterizerState, unclamped_fragment_depth_values), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "clip_plane_enable", flat_offset: core::mem::offset_of!(FlatRasterizerState, clip_plane_enable), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "representative_fragment_test", flat_offset: core::mem::offset_of!(FlatRasterizerState, representative_fragment_test), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_stipple_factor", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_stipple_factor), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_stipple_pattern", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_stipple_pattern), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "sprite_coord_enable", flat_offset: core::mem::offset_of!(FlatRasterizerState, sprite_coord_enable), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_width", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_width), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_size", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_size), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_units", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_units), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_scale", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_scale), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_clamp", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_clamp), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "conservative_raster_dilate", flat_offset: core::mem::offset_of!(FlatRasterizerState, conservative_raster_dilate), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_reference", field: "count", flat_offset: core::mem::offset_of!(FlatReference, count), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "width0", flat_offset: core::mem::offset_of!(FlatResourceTemplate, width0), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "height0", flat_offset: core::mem::offset_of!(FlatResourceTemplate, height0), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "depth0", flat_offset: core::mem::offset_of!(FlatResourceTemplate, depth0), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "array_size", flat_offset: core::mem::offset_of!(FlatResourceTemplate, array_size), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "format", flat_offset: core::mem::offset_of!(FlatResourceTemplate, format), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_resource", field: "target", flat_offset: core::mem::offset_of!(FlatResourceTemplate, target), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_resource", field: "last_level", flat_offset: core::mem::offset_of!(FlatResourceTemplate, last_level), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "nr_samples", flat_offset: core::mem::offset_of!(FlatResourceTemplate, nr_samples), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "nr_storage_samples", flat_offset: core::mem::offset_of!(FlatResourceTemplate, nr_storage_samples), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "nr_sparse_levels", flat_offset: core::mem::offset_of!(FlatResourceTemplate, nr_sparse_levels), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "compression_rate", flat_offset: core::mem::offset_of!(FlatResourceTemplate, compression_rate), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "usage", flat_offset: core::mem::offset_of!(FlatResourceTemplate, usage), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_resource", field: "bind", flat_offset: core::mem::offset_of!(FlatResourceTemplate, bind), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "flags", flat_offset: core::mem::offset_of!(FlatResourceTemplate, flags), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "blend_enable", flat_offset: core::mem::offset_of!(FlatRtBlendState, blend_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "rgb_func", flat_offset: core::mem::offset_of!(FlatRtBlendState, rgb_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "rgb_src_factor", flat_offset: core::mem::offset_of!(FlatRtBlendState, rgb_src_factor), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "rgb_dst_factor", flat_offset: core::mem::offset_of!(FlatRtBlendState, rgb_dst_factor), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "alpha_func", flat_offset: core::mem::offset_of!(FlatRtBlendState, alpha_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "alpha_src_factor", flat_offset: core::mem::offset_of!(FlatRtBlendState, alpha_src_factor), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "alpha_dst_factor", flat_offset: core::mem::offset_of!(FlatRtBlendState, alpha_dst_factor), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "colormask", flat_offset: core::mem::offset_of!(FlatRtBlendState, colormask), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "wrap_s", flat_offset: core::mem::offset_of!(FlatSamplerState, wrap_s), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "wrap_t", flat_offset: core::mem::offset_of!(FlatSamplerState, wrap_t), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "wrap_r", flat_offset: core::mem::offset_of!(FlatSamplerState, wrap_r), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "min_img_filter", flat_offset: core::mem::offset_of!(FlatSamplerState, min_img_filter), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "min_mip_filter", flat_offset: core::mem::offset_of!(FlatSamplerState, min_mip_filter), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "mag_img_filter", flat_offset: core::mem::offset_of!(FlatSamplerState, mag_img_filter), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "compare_mode", flat_offset: core::mem::offset_of!(FlatSamplerState, compare_mode), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "compare_func", flat_offset: core::mem::offset_of!(FlatSamplerState, compare_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "unnormalized_coords", flat_offset: core::mem::offset_of!(FlatSamplerState, unnormalized_coords), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "max_anisotropy", flat_offset: core::mem::offset_of!(FlatSamplerState, max_anisotropy), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "seamless_cube_map", flat_offset: core::mem::offset_of!(FlatSamplerState, seamless_cube_map), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "border_color_is_integer", flat_offset: core::mem::offset_of!(FlatSamplerState, border_color_is_integer), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "reduction_mode", flat_offset: core::mem::offset_of!(FlatSamplerState, reduction_mode), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "pad", flat_offset: core::mem::offset_of!(FlatSamplerState, pad), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "lod_bias", flat_offset: core::mem::offset_of!(FlatSamplerState, lod_bias), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "min_lod", flat_offset: core::mem::offset_of!(FlatSamplerState, min_lod), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "max_lod", flat_offset: core::mem::offset_of!(FlatSamplerState, max_lod), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "border_color", flat_offset: core::mem::offset_of!(FlatSamplerState, border_color), flat_size: core::mem::size_of::<[u32; 4]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "border_color_format", flat_offset: core::mem::offset_of!(FlatSamplerState, border_color_format), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_sampler_view", field: "format", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, format), flat_size: core::mem::size_of::<u32>(), bits: 12, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_sampler_view", field: "astc_decode_format", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, astc_decode_format), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "is_tex2d_from_buf", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, is_tex2d_from_buf), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "target", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, target), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_sampler_view", field: "swizzle_r", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, swizzle_r), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "swizzle_g", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, swizzle_g), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "swizzle_b", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, swizzle_b), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "swizzle_a", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, swizzle_a), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "u", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, u), flat_size: core::mem::size_of::<FlatSamplerViewU>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u", field: "tex", flat_offset: core::mem::offset_of!(FlatSamplerViewU, tex), flat_size: core::mem::size_of::<FlatSamplerViewUTex>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u", field: "buf", flat_offset: core::mem::offset_of!(FlatSamplerViewU, buf), flat_size: core::mem::size_of::<FlatSamplerViewUBuf>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u", field: "tex2d_from_buf", flat_offset: core::mem::offset_of!(FlatSamplerViewU, tex2d_from_buf), flat_size: core::mem::size_of::<FlatTex2dFromBuf>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.buf", field: "offset", flat_offset: core::mem::offset_of!(FlatSamplerViewUBuf, offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.buf", field: "size", flat_offset: core::mem::offset_of!(FlatSamplerViewUBuf, size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "first_layer", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, first_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "last_layer", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, last_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "first_level", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, first_level), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "last_level", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, last_level), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "min_lod_clamp", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, min_lod_clamp), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_scissor_state", field: "minx", flat_offset: core::mem::offset_of!(FlatScissorState, minx), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_scissor_state", field: "miny", flat_offset: core::mem::offset_of!(FlatScissorState, miny), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_scissor_state", field: "maxx", flat_offset: core::mem::offset_of!(FlatScissorState, maxx), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_scissor_state", field: "maxy", flat_offset: core::mem::offset_of!(FlatScissorState, maxy), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_buffer", field: "buffer", flat_offset: core::mem::offset_of!(FlatShaderBuffer, buffer), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_buffer", field: "buffer_offset", flat_offset: core::mem::offset_of!(FlatShaderBuffer, buffer_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_buffer", field: "buffer_size", flat_offset: core::mem::offset_of!(FlatShaderBuffer, buffer_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_instructions", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_instructions), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_alu_instructions", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_alu_instructions), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_tex_instructions", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_tex_instructions), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_tex_indirections", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_tex_indirections), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_control_flow_depth", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_control_flow_depth), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_inputs", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_inputs), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_outputs", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_outputs), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_const_buffer0_size", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_const_buffer0_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_const_buffers", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_const_buffers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_temps", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_temps), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_texture_samplers", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_texture_samplers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_sampler_views", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_sampler_views), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_shader_buffers", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_shader_buffers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_shader_images", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_shader_images), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_hw_atomic_counters", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_hw_atomic_counters), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "max_hw_atomic_counter_buffers", flat_offset: core::mem::offset_of!(FlatShaderCaps, max_hw_atomic_counter_buffers), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "supported_irs", flat_offset: core::mem::offset_of!(FlatShaderCaps, supported_irs), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "cont_supported", flat_offset: core::mem::offset_of!(FlatShaderCaps, cont_supported), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "indirect_temp_addr", flat_offset: core::mem::offset_of!(FlatShaderCaps, indirect_temp_addr), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "indirect_const_addr", flat_offset: core::mem::offset_of!(FlatShaderCaps, indirect_const_addr), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "subroutines", flat_offset: core::mem::offset_of!(FlatShaderCaps, subroutines), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "integers", flat_offset: core::mem::offset_of!(FlatShaderCaps, integers), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "int64_atomics", flat_offset: core::mem::offset_of!(FlatShaderCaps, int64_atomics), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "fp16", flat_offset: core::mem::offset_of!(FlatShaderCaps, fp16), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "fp16_derivatives", flat_offset: core::mem::offset_of!(FlatShaderCaps, fp16_derivatives), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "fp16_const_buffers", flat_offset: core::mem::offset_of!(FlatShaderCaps, fp16_const_buffers), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "fp16_no_denorms", flat_offset: core::mem::offset_of!(FlatShaderCaps, fp16_no_denorms), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "int16", flat_offset: core::mem::offset_of!(FlatShaderCaps, int16), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "glsl_16bit_consts", flat_offset: core::mem::offset_of!(FlatShaderCaps, glsl_16bit_consts), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "glsl_16bit_load_dst", flat_offset: core::mem::offset_of!(FlatShaderCaps, glsl_16bit_load_dst), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "tgsi_sqrt_supported", flat_offset: core::mem::offset_of!(FlatShaderCaps, tgsi_sqrt_supported), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_caps", field: "tgsi_any_inout_decl_range", flat_offset: core::mem::offset_of!(FlatShaderCaps, tgsi_any_inout_decl_range), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_state", field: "stream_output", flat_offset: core::mem::offset_of!(FlatShaderState, stream_output), flat_size: core::mem::size_of::<FlatStreamOutputInfo>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_shader_state", field: "report_compile_error", flat_offset: core::mem::offset_of!(FlatShaderState, report_compile_error), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_stencil_ref", field: "ref_value", flat_offset: core::mem::offset_of!(FlatStencilRef, ref_value), flat_size: core::mem::size_of::<[u8; 2]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "enabled", flat_offset: core::mem::offset_of!(FlatStencilState, enabled), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "func", flat_offset: core::mem::offset_of!(FlatStencilState, func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "fail_op", flat_offset: core::mem::offset_of!(FlatStencilState, fail_op), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "zpass_op", flat_offset: core::mem::offset_of!(FlatStencilState, zpass_op), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "zfail_op", flat_offset: core::mem::offset_of!(FlatStencilState, zfail_op), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "valuemask", flat_offset: core::mem::offset_of!(FlatStencilState, valuemask), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "writemask", flat_offset: core::mem::offset_of!(FlatStencilState, writemask), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "register_index", flat_offset: core::mem::offset_of!(FlatStreamOutput, register_index), flat_size: core::mem::size_of::<u32>(), bits: 6, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "start_component", flat_offset: core::mem::offset_of!(FlatStreamOutput, start_component), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "num_components", flat_offset: core::mem::offset_of!(FlatStreamOutput, num_components), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "output_buffer", flat_offset: core::mem::offset_of!(FlatStreamOutput, output_buffer), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "dst_offset", flat_offset: core::mem::offset_of!(FlatStreamOutput, dst_offset), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "stream", flat_offset: core::mem::offset_of!(FlatStreamOutput, stream), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output_info", field: "num_outputs", flat_offset: core::mem::offset_of!(FlatStreamOutputInfo, num_outputs), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_stream_output_info", field: "stride", flat_offset: core::mem::offset_of!(FlatStreamOutputInfo, stride), flat_size: core::mem::size_of::<[u16; 4]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_stream_output_info", field: "output", flat_offset: core::mem::offset_of!(FlatStreamOutputInfo, output), flat_size: core::mem::size_of::<[FlatStreamOutput; 128]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "reference", flat_offset: core::mem::offset_of!(FlatSurface, reference), flat_size: core::mem::size_of::<FlatReference>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "format", flat_offset: core::mem::offset_of!(FlatSurface, format), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_surface", field: "nr_samples", flat_offset: core::mem::offset_of!(FlatSurface, nr_samples), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "first_layer", flat_offset: core::mem::offset_of!(FlatSurface, first_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "last_layer", flat_offset: core::mem::offset_of!(FlatSurface, last_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "level", flat_offset: core::mem::offset_of!(FlatSurface, level), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "texture", flat_offset: core::mem::offset_of!(FlatSurface, texture), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_tex2d_from_buf", field: "offset", flat_offset: core::mem::offset_of!(FlatTex2dFromBuf, offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_tex2d_from_buf", field: "row_stride", flat_offset: core::mem::offset_of!(FlatTex2dFromBuf, row_stride), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_tex2d_from_buf", field: "width", flat_offset: core::mem::offset_of!(FlatTex2dFromBuf, width), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_tex2d_from_buf", field: "height", flat_offset: core::mem::offset_of!(FlatTex2dFromBuf, height), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_buffer", field: "is_user_buffer", flat_offset: core::mem::offset_of!(FlatVertexBuffer, is_user_buffer), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_buffer", field: "buffer_offset", flat_offset: core::mem::offset_of!(FlatVertexBuffer, buffer_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_buffer", field: "buffer", flat_offset: core::mem::offset_of!(FlatVertexBuffer, buffer), flat_size: core::mem::size_of::<FlatVertexBufferBuffer>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_vertex_buffer.buffer", field: "resource", flat_offset: core::mem::offset_of!(FlatVertexBufferBuffer, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "src_offset", flat_offset: core::mem::offset_of!(FlatVertexElement, src_offset), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "vertex_buffer_index", flat_offset: core::mem::offset_of!(FlatVertexElement, vertex_buffer_index), flat_size: core::mem::size_of::<u8>(), bits: 7, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "dual_slot", flat_offset: core::mem::offset_of!(FlatVertexElement, dual_slot), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "src_format", flat_offset: core::mem::offset_of!(FlatVertexElement, src_format), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "src_stride", flat_offset: core::mem::offset_of!(FlatVertexElement, src_stride), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "instance_divisor", flat_offset: core::mem::offset_of!(FlatVertexElement, instance_divisor), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_viewport_state", field: "scale", flat_offset: core::mem::offset_of!(FlatViewportState, scale), flat_size: core::mem::size_of::<[f32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_viewport_state", field: "translate", flat_offset: core::mem::offset_of!(FlatViewportState, translate), flat_size: core::mem::size_of::<[f32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_viewport_state", field: "swizzle_x", flat_offset: core::mem::offset_of!(FlatViewportState, swizzle_x), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_viewport_state", field: "swizzle_y", flat_offset: core::mem::offset_of!(FlatViewportState, swizzle_y), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_viewport_state", field: "swizzle_z", flat_offset: core::mem::offset_of!(FlatViewportState, swizzle_z), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_viewport_state", field: "swizzle_w", flat_offset: core::mem::offset_of!(FlatViewportState, swizzle_w), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
];

/// The fields of Mesa's structures the flat forms do not carry (`FIELDS_LEFT_OUT` of the generator, each with its reason
/// in interface.rs): with FIELDS, every field of each structure.
pub const LEFT_OUT: &[(&str, &str)] = &[
    ("pipe_compute_state", "ir_type"),
    ("pipe_compute_state", "prog"),
    ("pipe_grid_info", "num_globals"),
    ("pipe_grid_info", "globals"),
    ("pipe_resource", "__cl_space"),
    ("pipe_resource", "reference"),
    ("pipe_resource", "clientptr_heap"),
    ("pipe_resource", "next"),
    ("pipe_resource", "screen"),
    ("pipe_sampler_view", "reference"),
    ("pipe_sampler_view", "texture"),
    ("pipe_sampler_view", "context"),
    ("pipe_shader_state", "type"),
    ("pipe_shader_state", "tokens"),
    ("pipe_shader_state", "ir"),
    ("pipe_shader_state", "error_message"),
    ("pipe_vertex_buffer.buffer", "user"),
];

/// Every declared value of the trait (values.rs) with its C name, in flat.c's `wc_value` order.
pub fn values() -> Vec<(&'static str, i64)> {
    vec![
        ("MESA_SHADER_NONE", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_NONE.0)),
        ("MESA_SHADER_VERTEX", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_VERTEX.0)),
        ("MESA_SHADER_TESS_CTRL", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_TESS_CTRL.0)),
        ("MESA_SHADER_TESS_EVAL", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_TESS_EVAL.0)),
        ("MESA_SHADER_GEOMETRY", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_GEOMETRY.0)),
        ("MESA_SHADER_FRAGMENT", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_FRAGMENT.0)),
        ("MESA_SHADER_COMPUTE", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_COMPUTE.0)),
        ("MESA_SHADER_TASK", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_TASK.0)),
        ("MESA_SHADER_MESH", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_MESH.0)),
        ("MESA_SHADER_RAYGEN", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_RAYGEN.0)),
        ("MESA_SHADER_ANY_HIT", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_ANY_HIT.0)),
        ("MESA_SHADER_CLOSEST_HIT", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_CLOSEST_HIT.0)),
        ("MESA_SHADER_MISS", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_MISS.0)),
        ("MESA_SHADER_INTERSECTION", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_INTERSECTION.0)),
        ("MESA_SHADER_CALLABLE", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_CALLABLE.0)),
        ("MESA_SHADER_KERNEL", i64::from(wcontainers_gallium::values::MesaShaderStage::MESA_SHADER_KERNEL.0)),
        ("MESA_PRIM_POINTS", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_POINTS.0)),
        ("MESA_PRIM_LINES", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_LINES.0)),
        ("MESA_PRIM_LINE_LOOP", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_LINE_LOOP.0)),
        ("MESA_PRIM_LINE_STRIP", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_LINE_STRIP.0)),
        ("MESA_PRIM_TRIANGLES", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_TRIANGLES.0)),
        ("MESA_PRIM_TRIANGLE_STRIP", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_TRIANGLE_STRIP.0)),
        ("MESA_PRIM_TRIANGLE_FAN", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_TRIANGLE_FAN.0)),
        ("MESA_PRIM_QUADS", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_QUADS.0)),
        ("MESA_PRIM_QUAD_STRIP", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_QUAD_STRIP.0)),
        ("MESA_PRIM_POLYGON", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_POLYGON.0)),
        ("MESA_PRIM_LINES_ADJACENCY", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_LINES_ADJACENCY.0)),
        ("MESA_PRIM_LINE_STRIP_ADJACENCY", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_LINE_STRIP_ADJACENCY.0)),
        ("MESA_PRIM_TRIANGLES_ADJACENCY", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_TRIANGLES_ADJACENCY.0)),
        ("MESA_PRIM_TRIANGLE_STRIP_ADJACENCY", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_TRIANGLE_STRIP_ADJACENCY.0)),
        ("MESA_PRIM_PATCHES", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_PATCHES.0)),
        ("MESA_PRIM_MAX", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_MAX.0)),
        ("MESA_PRIM_COUNT", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_COUNT.0)),
        ("MESA_PRIM_UNKNOWN", i64::from(wcontainers_gallium::values::MesaPrim::MESA_PRIM_UNKNOWN.0)),
        ("PIPE_FORMAT_NONE", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_NONE.0)),
        ("PIPE_FORMAT_R64_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64_UINT.0)),
        ("PIPE_FORMAT_VERTEX_START", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_VERTEX_START.0)),
        ("PIPE_FORMAT_R64G64_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64G64_UINT.0)),
        ("PIPE_FORMAT_R64G64B64_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64G64B64_UINT.0)),
        ("PIPE_FORMAT_R64G64B64A64_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64G64B64A64_UINT.0)),
        ("PIPE_FORMAT_R64_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64_SINT.0)),
        ("PIPE_FORMAT_R64G64_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64G64_SINT.0)),
        ("PIPE_FORMAT_R64G64B64_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64G64B64_SINT.0)),
        ("PIPE_FORMAT_R64G64B64A64_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64G64B64A64_SINT.0)),
        ("PIPE_FORMAT_R64_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64_FLOAT.0)),
        ("PIPE_FORMAT_R64G64_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64G64_FLOAT.0)),
        ("PIPE_FORMAT_R64G64B64_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64G64B64_FLOAT.0)),
        ("PIPE_FORMAT_R64G64B64A64_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R64G64B64A64_FLOAT.0)),
        ("PIPE_FORMAT_R32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32_UINT.0)),
        ("PIPE_FORMAT_R32G32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32_UINT.0)),
        ("PIPE_FORMAT_R32G32B32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32_UINT.0)),
        ("PIPE_FORMAT_R32G32B32A32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32A32_UINT.0)),
        ("PIPE_FORMAT_R32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32_SINT.0)),
        ("PIPE_FORMAT_R32G32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32_SINT.0)),
        ("PIPE_FORMAT_R32G32B32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32_SINT.0)),
        ("PIPE_FORMAT_R32G32B32A32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32A32_SINT.0)),
        ("PIPE_FORMAT_R32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32_FLOAT.0)),
        ("PIPE_FORMAT_R32G32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32_FLOAT.0)),
        ("PIPE_FORMAT_R32G32B32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32_FLOAT.0)),
        ("PIPE_FORMAT_R32G32B32A32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32A32_FLOAT.0)),
        ("PIPE_FORMAT_R32_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32_UNORM.0)),
        ("PIPE_FORMAT_R32G32_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32_UNORM.0)),
        ("PIPE_FORMAT_R32G32B32_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32_UNORM.0)),
        ("PIPE_FORMAT_R32G32B32A32_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32A32_UNORM.0)),
        ("PIPE_FORMAT_R32_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32_SNORM.0)),
        ("PIPE_FORMAT_R32G32_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32_SNORM.0)),
        ("PIPE_FORMAT_R32G32B32_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32_SNORM.0)),
        ("PIPE_FORMAT_R32G32B32A32_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32A32_SNORM.0)),
        ("PIPE_FORMAT_R32_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32_USCALED.0)),
        ("PIPE_FORMAT_R32G32_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32_USCALED.0)),
        ("PIPE_FORMAT_R32G32B32_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32_USCALED.0)),
        ("PIPE_FORMAT_R32G32B32A32_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32A32_USCALED.0)),
        ("PIPE_FORMAT_R32_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32_SSCALED.0)),
        ("PIPE_FORMAT_R32G32_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32_SSCALED.0)),
        ("PIPE_FORMAT_R32G32B32_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32_SSCALED.0)),
        ("PIPE_FORMAT_R32G32B32A32_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32A32_SSCALED.0)),
        ("PIPE_FORMAT_R32_FIXED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32_FIXED.0)),
        ("PIPE_FORMAT_R32G32_FIXED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32_FIXED.0)),
        ("PIPE_FORMAT_R32G32B32_FIXED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32_FIXED.0)),
        ("PIPE_FORMAT_R32G32B32A32_FIXED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32A32_FIXED.0)),
        ("PIPE_FORMAT_R16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16_UINT.0)),
        ("PIPE_FORMAT_R16G16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16_UINT.0)),
        ("PIPE_FORMAT_R16G16B16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16_UINT.0)),
        ("PIPE_FORMAT_R16G16B16A16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16A16_UINT.0)),
        ("PIPE_FORMAT_R16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16_SINT.0)),
        ("PIPE_FORMAT_R16G16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16_SINT.0)),
        ("PIPE_FORMAT_R16G16B16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16_SINT.0)),
        ("PIPE_FORMAT_R16G16B16A16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16A16_SINT.0)),
        ("PIPE_FORMAT_R16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16_FLOAT.0)),
        ("PIPE_FORMAT_R16G16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16_FLOAT.0)),
        ("PIPE_FORMAT_R16G16B16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16_FLOAT.0)),
        ("PIPE_FORMAT_R16G16B16A16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16A16_FLOAT.0)),
        ("PIPE_FORMAT_R16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16_UNORM.0)),
        ("PIPE_FORMAT_R16G16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16_UNORM.0)),
        ("PIPE_FORMAT_R16G16B16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16_UNORM.0)),
        ("PIPE_FORMAT_R16G16B16A16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16A16_UNORM.0)),
        ("PIPE_FORMAT_R16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16_SNORM.0)),
        ("PIPE_FORMAT_R16G16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16_SNORM.0)),
        ("PIPE_FORMAT_R16G16B16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16_SNORM.0)),
        ("PIPE_FORMAT_R16G16B16A16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16A16_SNORM.0)),
        ("PIPE_FORMAT_R16_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16_USCALED.0)),
        ("PIPE_FORMAT_R16G16_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16_USCALED.0)),
        ("PIPE_FORMAT_R16G16B16_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16_USCALED.0)),
        ("PIPE_FORMAT_R16G16B16A16_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16A16_USCALED.0)),
        ("PIPE_FORMAT_R16_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16_SSCALED.0)),
        ("PIPE_FORMAT_R16G16_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16_SSCALED.0)),
        ("PIPE_FORMAT_R16G16B16_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16_SSCALED.0)),
        ("PIPE_FORMAT_R16G16B16A16_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16A16_SSCALED.0)),
        ("PIPE_FORMAT_R8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_UINT.0)),
        ("PIPE_FORMAT_R8G8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8_UINT.0)),
        ("PIPE_FORMAT_R8G8B8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8_UINT.0)),
        ("PIPE_FORMAT_R8G8B8A8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8A8_UINT.0)),
        ("PIPE_FORMAT_R8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_SINT.0)),
        ("PIPE_FORMAT_R8G8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8_SINT.0)),
        ("PIPE_FORMAT_R8G8B8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8_SINT.0)),
        ("PIPE_FORMAT_R8G8B8A8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8A8_SINT.0)),
        ("PIPE_FORMAT_R8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_UNORM.0)),
        ("PIPE_FORMAT_R8G8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8_UNORM.0)),
        ("PIPE_FORMAT_R8G8B8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8_UNORM.0)),
        ("PIPE_FORMAT_R8G8B8A8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8A8_UNORM.0)),
        ("PIPE_FORMAT_R8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_SNORM.0)),
        ("PIPE_FORMAT_R8G8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8_SNORM.0)),
        ("PIPE_FORMAT_R8G8B8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8_SNORM.0)),
        ("PIPE_FORMAT_R8G8B8A8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8A8_SNORM.0)),
        ("PIPE_FORMAT_R8_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_USCALED.0)),
        ("PIPE_FORMAT_R8G8_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8_USCALED.0)),
        ("PIPE_FORMAT_R8G8B8_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8_USCALED.0)),
        ("PIPE_FORMAT_R8G8B8A8_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8A8_USCALED.0)),
        ("PIPE_FORMAT_R8_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_SSCALED.0)),
        ("PIPE_FORMAT_R8G8_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8_SSCALED.0)),
        ("PIPE_FORMAT_R8G8B8_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8_SSCALED.0)),
        ("PIPE_FORMAT_R8G8B8A8_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8A8_SSCALED.0)),
        ("PIPE_FORMAT_B8G8R8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8_UINT.0)),
        ("PIPE_FORMAT_B8G8R8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8_SINT.0)),
        ("PIPE_FORMAT_B8G8R8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8_UNORM.0)),
        ("PIPE_FORMAT_B8G8R8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8_SNORM.0)),
        ("PIPE_FORMAT_B8G8R8_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8_USCALED.0)),
        ("PIPE_FORMAT_B8G8R8_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8_SSCALED.0)),
        ("PIPE_FORMAT_B8G8R8A8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8A8_UINT.0)),
        ("PIPE_FORMAT_B8G8R8A8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8A8_SINT.0)),
        ("PIPE_FORMAT_B8G8R8A8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8A8_UNORM.0)),
        ("PIPE_FORMAT_B8G8R8A8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8A8_SNORM.0)),
        ("PIPE_FORMAT_B8G8R8A8_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8A8_USCALED.0)),
        ("PIPE_FORMAT_B8G8R8A8_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8A8_SSCALED.0)),
        ("PIPE_FORMAT_A8B8G8R8_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8B8G8R8_USCALED.0)),
        ("PIPE_FORMAT_A8B8G8R8_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8B8G8R8_SSCALED.0)),
        ("PIPE_FORMAT_A8R8G8B8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8R8G8B8_UNORM.0)),
        ("PIPE_FORMAT_R10G10B10A2_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10A2_UINT.0)),
        ("PIPE_FORMAT_R10G10B10A2_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10A2_SINT.0)),
        ("PIPE_FORMAT_R10G10B10A2_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10A2_UNORM.0)),
        ("PIPE_FORMAT_R10G10B10A2_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10A2_SNORM.0)),
        ("PIPE_FORMAT_R10G10B10A2_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10A2_USCALED.0)),
        ("PIPE_FORMAT_R10G10B10A2_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10A2_SSCALED.0)),
        ("PIPE_FORMAT_B10G10R10A2_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B10G10R10A2_UINT.0)),
        ("PIPE_FORMAT_B10G10R10A2_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B10G10R10A2_SINT.0)),
        ("PIPE_FORMAT_B10G10R10A2_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B10G10R10A2_UNORM.0)),
        ("PIPE_FORMAT_B10G10R10A2_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B10G10R10A2_SNORM.0)),
        ("PIPE_FORMAT_B10G10R10A2_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B10G10R10A2_USCALED.0)),
        ("PIPE_FORMAT_B10G10R10A2_SSCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B10G10R10A2_SSCALED.0)),
        ("PIPE_FORMAT_R11G11B10_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R11G11B10_FLOAT.0)),
        ("PIPE_FORMAT_VERTEX_END", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_VERTEX_END.0)),
        ("PIPE_FORMAT_U8Y8V8Y8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_U8Y8V8Y8_422_UNORM.0)),
        ("PIPE_FORMAT_YUV_START", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_YUV_START.0)),
        ("PIPE_FORMAT_V8Y8U8Y8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_V8Y8U8Y8_422_UNORM.0)),
        ("PIPE_FORMAT_Y8U8Y8V8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8U8Y8V8_422_UNORM.0)),
        ("PIPE_FORMAT_Y8V8Y8U8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8V8Y8U8_422_UNORM.0)),
        ("PIPE_FORMAT_A8Y8U8V8_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8Y8U8V8_444_UNORM.0)),
        ("PIPE_FORMAT_X8Y8U8V8_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8Y8U8V8_444_UNORM.0)),
        ("PIPE_FORMAT_Y8_V8_U8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_V8_U8_420_UNORM.0)),
        ("PIPE_FORMAT_Y8_V8_U8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_V8_U8_422_UNORM.0)),
        ("PIPE_FORMAT_Y8_U8_V8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_U8_V8_420_UNORM.0)),
        ("PIPE_FORMAT_Y8_U8V8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_U8V8_420_UNORM.0)),
        ("PIPE_FORMAT_Y8_V8U8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_V8U8_420_UNORM.0)),
        ("PIPE_FORMAT_Y8_400_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_400_UNORM.0)),
        ("PIPE_FORMAT_Y10_U10V10_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y10_U10V10_420_UNORM.0)),
        ("PIPE_FORMAT_Y10_U10V10_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y10_U10V10_422_UNORM.0)),
        ("PIPE_FORMAT_Y16_U16V16_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y16_U16V16_444_UNORM.0)),
        ("PIPE_FORMAT_Y8_U8_V8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_U8_V8_422_UNORM.0)),
        ("PIPE_FORMAT_Y8_U8V8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_U8V8_422_UNORM.0)),
        ("PIPE_FORMAT_Y8_V8U8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_V8U8_422_UNORM.0)),
        ("PIPE_FORMAT_Y8_U8V8_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_U8V8_444_UNORM.0)),
        ("PIPE_FORMAT_Y8_V8U8_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_V8U8_444_UNORM.0)),
        ("PIPE_FORMAT_Y8_U8_V8_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_U8_V8_444_UNORM.0)),
        ("PIPE_FORMAT_Y8_U8_V8_440_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8_U8_V8_440_UNORM.0)),
        ("PIPE_FORMAT_Y10X6_U10X6_V10X6_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y10X6_U10X6_V10X6_420_UNORM.0)),
        ("PIPE_FORMAT_Y12X4_U12X4_V12X4_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y12X4_U12X4_V12X4_420_UNORM.0)),
        ("PIPE_FORMAT_Y16_U16_V16_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y16_U16_V16_420_UNORM.0)),
        ("PIPE_FORMAT_Y10X6_U10X6_V10X6_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y10X6_U10X6_V10X6_422_UNORM.0)),
        ("PIPE_FORMAT_Y12X4_U12X4_V12X4_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y12X4_U12X4_V12X4_422_UNORM.0)),
        ("PIPE_FORMAT_Y16_U16_V16_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y16_U16_V16_422_UNORM.0)),
        ("PIPE_FORMAT_Y10X6_U10X6_V10X6_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y10X6_U10X6_V10X6_444_UNORM.0)),
        ("PIPE_FORMAT_Y12X4_U12X4_V12X4_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y12X4_U12X4_V12X4_444_UNORM.0)),
        ("PIPE_FORMAT_Y16_U16_V16_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y16_U16_V16_444_UNORM.0)),
        ("PIPE_FORMAT_Y16_U16V16_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y16_U16V16_422_UNORM.0)),
        ("PIPE_FORMAT_X6Y10_X6U10X6V10_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6Y10_X6U10X6V10_420_UNORM.0)),
        ("PIPE_FORMAT_X4Y12_X4U12X4V12_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X4Y12_X4U12X4V12_420_UNORM.0)),
        ("PIPE_FORMAT_Y16_U16V16_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y16_U16V16_420_UNORM.0)),
        ("PIPE_FORMAT_X6Y10_X6U10X6V10_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6Y10_X6U10X6V10_422_UNORM.0)),
        ("PIPE_FORMAT_Y10Y10Y10X2_U10V10U10X2V10U10V10X2_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y10Y10Y10X2_U10V10U10X2V10U10V10X2_420_UNORM.0)),
        ("PIPE_FORMAT_X6Y10X6U10X6Y10X6V10_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6Y10X6U10X6Y10X6V10_422_UNORM.0)),
        ("PIPE_FORMAT_X4Y12X4U12X4Y12X4V12_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X4Y12X4U12X4Y12X4V12_422_UNORM.0)),
        ("PIPE_FORMAT_Y16U16Y16V16_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y16U16Y16V16_422_UNORM.0)),
        ("PIPE_FORMAT_U10Y10V10A2_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_U10Y10V10A2_444_UNORM.0)),
        ("PIPE_FORMAT_X4U12X4Y12X4V12X4A12_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X4U12X4Y12X4V12X4A12_444_UNORM.0)),
        ("PIPE_FORMAT_U16Y16V16A16_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_U16Y16V16A16_444_UNORM.0)),
        ("PIPE_FORMAT_Y8U8V8_420_UNORM_PACKED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y8U8V8_420_UNORM_PACKED.0)),
        ("PIPE_FORMAT_Y10U10V10_420_UNORM_PACKED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Y10U10V10_420_UNORM_PACKED.0)),
        ("PIPE_FORMAT_YUV_END", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_YUV_END.0)),
        ("PIPE_FORMAT_DXT1_RGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_DXT1_RGB.0)),
        ("PIPE_FORMAT_COMPR_START", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_COMPR_START.0)),
        ("PIPE_FORMAT_DXT1_RGBA", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_DXT1_RGBA.0)),
        ("PIPE_FORMAT_DXT3_RGBA", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_DXT3_RGBA.0)),
        ("PIPE_FORMAT_DXT5_RGBA", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_DXT5_RGBA.0)),
        ("PIPE_FORMAT_DXT1_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_DXT1_SRGB.0)),
        ("PIPE_FORMAT_DXT1_SRGBA", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_DXT1_SRGBA.0)),
        ("PIPE_FORMAT_DXT3_SRGBA", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_DXT3_SRGBA.0)),
        ("PIPE_FORMAT_DXT5_SRGBA", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_DXT5_SRGBA.0)),
        ("PIPE_FORMAT_RGTC1_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_RGTC1_UNORM.0)),
        ("PIPE_FORMAT_RGTC1_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_RGTC1_SNORM.0)),
        ("PIPE_FORMAT_RGTC2_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_RGTC2_UNORM.0)),
        ("PIPE_FORMAT_RGTC2_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_RGTC2_SNORM.0)),
        ("PIPE_FORMAT_LATC1_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_LATC1_UNORM.0)),
        ("PIPE_FORMAT_LATC1_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_LATC1_SNORM.0)),
        ("PIPE_FORMAT_LATC2_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_LATC2_UNORM.0)),
        ("PIPE_FORMAT_LATC2_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_LATC2_SNORM.0)),
        ("PIPE_FORMAT_ETC1_RGB8", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC1_RGB8.0)),
        ("PIPE_FORMAT_ETC2_RGB8", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_RGB8.0)),
        ("PIPE_FORMAT_ETC2_RGB8A1", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_RGB8A1.0)),
        ("PIPE_FORMAT_ETC2_RGBA8", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_RGBA8.0)),
        ("PIPE_FORMAT_ETC2_R11_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_R11_UNORM.0)),
        ("PIPE_FORMAT_ETC2_R11_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_R11_SNORM.0)),
        ("PIPE_FORMAT_ETC2_RG11_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_RG11_UNORM.0)),
        ("PIPE_FORMAT_ETC2_RG11_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_RG11_SNORM.0)),
        ("PIPE_FORMAT_ETC2_SRGB8", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_SRGB8.0)),
        ("PIPE_FORMAT_ETC2_SRGB8A1", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_SRGB8A1.0)),
        ("PIPE_FORMAT_ETC2_SRGBA8", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ETC2_SRGBA8.0)),
        ("PIPE_FORMAT_BPTC_RGBA_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_BPTC_RGBA_UNORM.0)),
        ("PIPE_FORMAT_BPTC_RGB_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_BPTC_RGB_FLOAT.0)),
        ("PIPE_FORMAT_BPTC_RGB_UFLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_BPTC_RGB_UFLOAT.0)),
        ("PIPE_FORMAT_BPTC_SRGBA", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_BPTC_SRGBA.0)),
        ("PIPE_FORMAT_ASTC_4x4", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_4X4.0)),
        ("PIPE_FORMAT_ASTC_5x4", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X4.0)),
        ("PIPE_FORMAT_ASTC_5x5", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X5.0)),
        ("PIPE_FORMAT_ASTC_6x5", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X5.0)),
        ("PIPE_FORMAT_ASTC_6x6", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X6.0)),
        ("PIPE_FORMAT_ASTC_8x5", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_8X5.0)),
        ("PIPE_FORMAT_ASTC_8x6", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_8X6.0)),
        ("PIPE_FORMAT_ASTC_8x8", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_8X8.0)),
        ("PIPE_FORMAT_ASTC_10x5", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X5.0)),
        ("PIPE_FORMAT_ASTC_10x6", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X6.0)),
        ("PIPE_FORMAT_ASTC_10x8", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X8.0)),
        ("PIPE_FORMAT_ASTC_10x10", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X10.0)),
        ("PIPE_FORMAT_ASTC_12x10", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_12X10.0)),
        ("PIPE_FORMAT_ASTC_12x12", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_12X12.0)),
        ("PIPE_FORMAT_ASTC_3x3x3", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_3X3X3.0)),
        ("PIPE_FORMAT_ASTC_4x3x3", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_4X3X3.0)),
        ("PIPE_FORMAT_ASTC_4x4x3", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_4X4X3.0)),
        ("PIPE_FORMAT_ASTC_4x4x4", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_4X4X4.0)),
        ("PIPE_FORMAT_ASTC_5x4x4", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X4X4.0)),
        ("PIPE_FORMAT_ASTC_5x5x4", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X5X4.0)),
        ("PIPE_FORMAT_ASTC_5x5x5", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X5X5.0)),
        ("PIPE_FORMAT_ASTC_6x5x5", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X5X5.0)),
        ("PIPE_FORMAT_ASTC_6x6x5", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X6X5.0)),
        ("PIPE_FORMAT_ASTC_6x6x6", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X6X6.0)),
        ("PIPE_FORMAT_ASTC_4x4_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_4X4_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_5x4_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X4_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_5x5_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X5_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_6x5_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X5_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_6x6_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X6_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_8x5_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_8X5_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_8x6_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_8X6_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_8x8_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_8X8_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_10x5_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X5_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_10x6_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X6_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_10x8_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X8_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_10x10_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X10_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_12x10_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_12X10_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_12x12_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_12X12_FLOAT.0)),
        ("PIPE_FORMAT_ASTC_4x4_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_4X4_SRGB.0)),
        ("PIPE_FORMAT_ASTC_5x4_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X4_SRGB.0)),
        ("PIPE_FORMAT_ASTC_5x5_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X5_SRGB.0)),
        ("PIPE_FORMAT_ASTC_6x5_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X5_SRGB.0)),
        ("PIPE_FORMAT_ASTC_6x6_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X6_SRGB.0)),
        ("PIPE_FORMAT_ASTC_8x5_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_8X5_SRGB.0)),
        ("PIPE_FORMAT_ASTC_8x6_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_8X6_SRGB.0)),
        ("PIPE_FORMAT_ASTC_8x8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_8X8_SRGB.0)),
        ("PIPE_FORMAT_ASTC_10x5_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X5_SRGB.0)),
        ("PIPE_FORMAT_ASTC_10x6_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X6_SRGB.0)),
        ("PIPE_FORMAT_ASTC_10x8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X8_SRGB.0)),
        ("PIPE_FORMAT_ASTC_10x10_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_10X10_SRGB.0)),
        ("PIPE_FORMAT_ASTC_12x10_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_12X10_SRGB.0)),
        ("PIPE_FORMAT_ASTC_12x12_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_12X12_SRGB.0)),
        ("PIPE_FORMAT_ASTC_3x3x3_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_3X3X3_SRGB.0)),
        ("PIPE_FORMAT_ASTC_4x3x3_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_4X3X3_SRGB.0)),
        ("PIPE_FORMAT_ASTC_4x4x3_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_4X4X3_SRGB.0)),
        ("PIPE_FORMAT_ASTC_4x4x4_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_4X4X4_SRGB.0)),
        ("PIPE_FORMAT_ASTC_5x4x4_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X4X4_SRGB.0)),
        ("PIPE_FORMAT_ASTC_5x5x4_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X5X4_SRGB.0)),
        ("PIPE_FORMAT_ASTC_5x5x5_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_5X5X5_SRGB.0)),
        ("PIPE_FORMAT_ASTC_6x5x5_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X5X5_SRGB.0)),
        ("PIPE_FORMAT_ASTC_6x6x5_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X6X5_SRGB.0)),
        ("PIPE_FORMAT_ASTC_6x6x6_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ASTC_6X6X6_SRGB.0)),
        ("PIPE_FORMAT_ATC_RGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ATC_RGB.0)),
        ("PIPE_FORMAT_ATC_RGBA_EXPLICIT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ATC_RGBA_EXPLICIT.0)),
        ("PIPE_FORMAT_ATC_RGBA_INTERPOLATED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ATC_RGBA_INTERPOLATED.0)),
        ("PIPE_FORMAT_FXT1_RGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_FXT1_RGB.0)),
        ("PIPE_FORMAT_FXT1_RGBA", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_FXT1_RGBA.0)),
        ("PIPE_FORMAT_COMPR_END", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_COMPR_END.0)),
        ("PIPE_FORMAT_S8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_S8_UINT.0)),
        ("PIPE_FORMAT_ZS_START", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ZS_START.0)),
        ("PIPE_FORMAT_Z16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Z16_UNORM.0)),
        ("PIPE_FORMAT_Z16_UNORM_S8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Z16_UNORM_S8_UINT.0)),
        ("PIPE_FORMAT_Z32_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Z32_UNORM.0)),
        ("PIPE_FORMAT_Z32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Z32_FLOAT.0)),
        ("PIPE_FORMAT_Z24_UNORM_S8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Z24_UNORM_S8_UINT.0)),
        ("PIPE_FORMAT_S8_UINT_Z24_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_S8_UINT_Z24_UNORM.0)),
        ("PIPE_FORMAT_X24S8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X24S8_UINT.0)),
        ("PIPE_FORMAT_S8X24_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_S8X24_UINT.0)),
        ("PIPE_FORMAT_Z24X8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Z24X8_UNORM.0)),
        ("PIPE_FORMAT_X8Z24_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8Z24_UNORM.0)),
        ("PIPE_FORMAT_Z24_UNORM_PACKED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Z24_UNORM_PACKED.0)),
        ("PIPE_FORMAT_Z32_FLOAT_S8X24_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Z32_FLOAT_S8X24_UINT.0)),
        ("PIPE_FORMAT_X32_S8X24_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X32_S8X24_UINT.0)),
        ("PIPE_FORMAT_ZS_END", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_ZS_END.0)),
        ("PIPE_FORMAT_B8G8R8X8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8X8_UNORM.0)),
        ("PIPE_FORMAT_B8G8R8X8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8X8_SNORM.0)),
        ("PIPE_FORMAT_B8G8R8X8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8X8_UINT.0)),
        ("PIPE_FORMAT_B8G8R8X8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8X8_SINT.0)),
        ("PIPE_FORMAT_A8R8G8B8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8R8G8B8_SNORM.0)),
        ("PIPE_FORMAT_A8R8G8B8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8R8G8B8_SINT.0)),
        ("PIPE_FORMAT_X8R8G8B8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8R8G8B8_UNORM.0)),
        ("PIPE_FORMAT_X8R8G8B8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8R8G8B8_SNORM.0)),
        ("PIPE_FORMAT_X8R8G8B8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8R8G8B8_SINT.0)),
        ("PIPE_FORMAT_A8B8G8R8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8B8G8R8_UNORM.0)),
        ("PIPE_FORMAT_X8B8G8R8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8B8G8R8_UNORM.0)),
        ("PIPE_FORMAT_R8G8B8X8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8X8_UNORM.0)),
        ("PIPE_FORMAT_R5G5B5A1_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R5G5B5A1_UNORM.0)),
        ("PIPE_FORMAT_R5G5B5X1_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R5G5B5X1_UNORM.0)),
        ("PIPE_FORMAT_B5G5R5X1_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B5G5R5X1_UNORM.0)),
        ("PIPE_FORMAT_B5G5R5A1_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B5G5R5A1_UNORM.0)),
        ("PIPE_FORMAT_X1B5G5R5_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X1B5G5R5_UNORM.0)),
        ("PIPE_FORMAT_A1R5G5B5_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A1R5G5B5_UNORM.0)),
        ("PIPE_FORMAT_X1R5G5B5_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X1R5G5B5_UNORM.0)),
        ("PIPE_FORMAT_A1B5G5R5_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A1B5G5R5_UNORM.0)),
        ("PIPE_FORMAT_R4G4B4A4_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R4G4B4A4_UNORM.0)),
        ("PIPE_FORMAT_R4G4B4X4_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R4G4B4X4_UNORM.0)),
        ("PIPE_FORMAT_B4G4R4A4_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B4G4R4A4_UNORM.0)),
        ("PIPE_FORMAT_B4G4R4X4_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B4G4R4X4_UNORM.0)),
        ("PIPE_FORMAT_A4R4G4B4_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A4R4G4B4_UNORM.0)),
        ("PIPE_FORMAT_A4B4G4R4_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A4B4G4R4_UNORM.0)),
        ("PIPE_FORMAT_R5G6B5_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R5G6B5_UNORM.0)),
        ("PIPE_FORMAT_B5G6R5_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B5G6R5_UNORM.0)),
        ("PIPE_FORMAT_R10G10B10X2_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10X2_UNORM.0)),
        ("PIPE_FORMAT_A2R10G10B10_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A2R10G10B10_UNORM.0)),
        ("PIPE_FORMAT_A2B10G10R10_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A2B10G10R10_UNORM.0)),
        ("PIPE_FORMAT_R3G3B2_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R3G3B2_UNORM.0)),
        ("PIPE_FORMAT_B2G3R3_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B2G3R3_UNORM.0)),
        ("PIPE_FORMAT_L8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8_UNORM.0)),
        ("PIPE_FORMAT_A8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8_UNORM.0)),
        ("PIPE_FORMAT_I8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I8_UNORM.0)),
        ("PIPE_FORMAT_L4A4_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L4A4_UNORM.0)),
        ("PIPE_FORMAT_L8A8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8A8_UNORM.0)),
        ("PIPE_FORMAT_L16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16_UNORM.0)),
        ("PIPE_FORMAT_A16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A16_UNORM.0)),
        ("PIPE_FORMAT_I16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I16_UNORM.0)),
        ("PIPE_FORMAT_L16A16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16A16_UNORM.0)),
        ("PIPE_FORMAT_A8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8_SNORM.0)),
        ("PIPE_FORMAT_L8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8_SNORM.0)),
        ("PIPE_FORMAT_L8A8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8A8_SNORM.0)),
        ("PIPE_FORMAT_I8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I8_SNORM.0)),
        ("PIPE_FORMAT_A16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A16_SNORM.0)),
        ("PIPE_FORMAT_L16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16_SNORM.0)),
        ("PIPE_FORMAT_L16A16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16A16_SNORM.0)),
        ("PIPE_FORMAT_I16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I16_SNORM.0)),
        ("PIPE_FORMAT_A16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A16_FLOAT.0)),
        ("PIPE_FORMAT_L16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16_FLOAT.0)),
        ("PIPE_FORMAT_L16A16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16A16_FLOAT.0)),
        ("PIPE_FORMAT_I16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I16_FLOAT.0)),
        ("PIPE_FORMAT_A32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A32_FLOAT.0)),
        ("PIPE_FORMAT_L32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L32_FLOAT.0)),
        ("PIPE_FORMAT_L32A32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L32A32_FLOAT.0)),
        ("PIPE_FORMAT_I32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I32_FLOAT.0)),
        ("PIPE_FORMAT_L8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8_SRGB.0)),
        ("PIPE_FORMAT_R8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_SRGB.0)),
        ("PIPE_FORMAT_L8A8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8A8_SRGB.0)),
        ("PIPE_FORMAT_R8G8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8_SRGB.0)),
        ("PIPE_FORMAT_R8G8B8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8_SRGB.0)),
        ("PIPE_FORMAT_B8G8R8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8_SRGB.0)),
        ("PIPE_FORMAT_R8G8B8A8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8A8_SRGB.0)),
        ("PIPE_FORMAT_A8B8G8R8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8B8G8R8_SRGB.0)),
        ("PIPE_FORMAT_X8B8G8R8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8B8G8R8_SRGB.0)),
        ("PIPE_FORMAT_B8G8R8A8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8A8_SRGB.0)),
        ("PIPE_FORMAT_B8G8R8X8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8R8X8_SRGB.0)),
        ("PIPE_FORMAT_A8R8G8B8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8R8G8B8_SRGB.0)),
        ("PIPE_FORMAT_X8R8G8B8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8R8G8B8_SRGB.0)),
        ("PIPE_FORMAT_R8SG8SB8UX8U_NORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8SG8SB8UX8U_NORM.0)),
        ("PIPE_FORMAT_R10SG10SB10SA2U_NORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10SG10SB10SA2U_NORM.0)),
        ("PIPE_FORMAT_R5SG5SB6U_NORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R5SG5SB6U_NORM.0)),
        ("PIPE_FORMAT_Z24_UNORM_S8_UINT_AS_R8G8B8A8", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_Z24_UNORM_S8_UINT_AS_R8G8B8A8.0)),
        ("PIPE_FORMAT_R8G8_B8G8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8_B8G8_422_UNORM.0)),
        ("PIPE_FORMAT_G8R8_G8B8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8R8_G8B8_422_UNORM.0)),
        ("PIPE_FORMAT_G8R8_B8R8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8R8_B8R8_422_UNORM.0)),
        ("PIPE_FORMAT_R8G8_R8B8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8_R8B8_422_UNORM.0)),
        ("PIPE_FORMAT_B8R8_G8R8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8R8_G8R8_422_UNORM.0)),
        ("PIPE_FORMAT_R8B8_R8G8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8B8_R8G8_422_UNORM.0)),
        ("PIPE_FORMAT_G8B8_G8R8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8B8_G8R8_422_UNORM.0)),
        ("PIPE_FORMAT_B8G8_R8G8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B8G8_R8G8_422_UNORM.0)),
        ("PIPE_FORMAT_R16G16_R16B16_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16_R16B16_422_UNORM.0)),
        ("PIPE_FORMAT_X6R10X6G10_X6R10X6B10_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6R10X6G10_X6R10X6B10_422_UNORM.0)),
        ("PIPE_FORMAT_R9G9B9E5_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R9G9B9E5_FLOAT.0)),
        ("PIPE_FORMAT_R1_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R1_UNORM.0)),
        ("PIPE_FORMAT_R8G8Bx_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8BX_SNORM.0)),
        ("PIPE_FORMAT_R10G10B10X2_USCALED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10X2_USCALED.0)),
        ("PIPE_FORMAT_R10G10B10X2_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10X2_SNORM.0)),
        ("PIPE_FORMAT_R10G10B10X2_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10X2_SINT.0)),
        ("PIPE_FORMAT_R8_G8B8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_G8B8_420_UNORM.0)),
        ("PIPE_FORMAT_R8_B8G8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_B8G8_420_UNORM.0)),
        ("PIPE_FORMAT_G8_B8R8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8_B8R8_420_UNORM.0)),
        ("PIPE_FORMAT_R8_G8B8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_G8B8_422_UNORM.0)),
        ("PIPE_FORMAT_R8_B8G8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_B8G8_422_UNORM.0)),
        ("PIPE_FORMAT_G8_B8R8_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8_B8R8_422_UNORM.0)),
        ("PIPE_FORMAT_R10_G10B10_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10_G10B10_420_UNORM.0)),
        ("PIPE_FORMAT_R10_G10B10_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10_G10B10_422_UNORM.0)),
        ("PIPE_FORMAT_R8_G8_B8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_G8_B8_420_UNORM.0)),
        ("PIPE_FORMAT_R8_B8_G8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_B8_G8_420_UNORM.0)),
        ("PIPE_FORMAT_G8_B8_R8_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8_B8_R8_420_UNORM.0)),
        ("PIPE_FORMAT_R8_G8_B8_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8_G8_B8_444_UNORM.0)),
        ("PIPE_FORMAT_X6G10_X6B10X6R10_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6G10_X6B10X6R10_420_UNORM.0)),
        ("PIPE_FORMAT_X4G12_X4B12X4R12_420_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X4G12_X4B12X4R12_420_UNORM.0)),
        ("PIPE_FORMAT_X6G10_X6B10X6R10_422_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6G10_X6B10X6R10_422_UNORM.0)),
        ("PIPE_FORMAT_G8_B8R8_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8_B8R8_444_UNORM.0)),
        ("PIPE_FORMAT_X6G10_X6B10X6R10_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6G10_X6B10X6R10_444_UNORM.0)),
        ("PIPE_FORMAT_X4G12_X4B12X4R12_444_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X4G12_X4B12X4R12_444_UNORM.0)),
        ("PIPE_FORMAT_R8G8B8_420_UNORM_PACKED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8_420_UNORM_PACKED.0)),
        ("PIPE_FORMAT_R10G10B10_420_UNORM_PACKED", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R10G10B10_420_UNORM_PACKED.0)),
        ("PIPE_FORMAT_X6R10X6G10X6B10X6A10_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6R10X6G10X6B10X6A10_UNORM.0)),
        ("PIPE_FORMAT_X6R10_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6R10_UNORM.0)),
        ("PIPE_FORMAT_X6R10X6G10_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X6R10X6G10_UNORM.0)),
        ("PIPE_FORMAT_X4R12_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X4R12_UNORM.0)),
        ("PIPE_FORMAT_X4R12X4G12_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X4R12X4G12_UNORM.0)),
        ("PIPE_FORMAT_A4R4_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A4R4_UNORM.0)),
        ("PIPE_FORMAT_R4A4_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R4A4_UNORM.0)),
        ("PIPE_FORMAT_R8A8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8A8_UNORM.0)),
        ("PIPE_FORMAT_A8R8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8R8_UNORM.0)),
        ("PIPE_FORMAT_A8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8_UINT.0)),
        ("PIPE_FORMAT_I8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I8_UINT.0)),
        ("PIPE_FORMAT_L8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8_UINT.0)),
        ("PIPE_FORMAT_L8A8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8A8_UINT.0)),
        ("PIPE_FORMAT_A8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8_SINT.0)),
        ("PIPE_FORMAT_I8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I8_SINT.0)),
        ("PIPE_FORMAT_L8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8_SINT.0)),
        ("PIPE_FORMAT_L8A8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L8A8_SINT.0)),
        ("PIPE_FORMAT_A16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A16_UINT.0)),
        ("PIPE_FORMAT_I16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I16_UINT.0)),
        ("PIPE_FORMAT_L16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16_UINT.0)),
        ("PIPE_FORMAT_L16A16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16A16_UINT.0)),
        ("PIPE_FORMAT_A16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A16_SINT.0)),
        ("PIPE_FORMAT_I16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I16_SINT.0)),
        ("PIPE_FORMAT_L16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16_SINT.0)),
        ("PIPE_FORMAT_L16A16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L16A16_SINT.0)),
        ("PIPE_FORMAT_A32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A32_UINT.0)),
        ("PIPE_FORMAT_I32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I32_UINT.0)),
        ("PIPE_FORMAT_L32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L32_UINT.0)),
        ("PIPE_FORMAT_L32A32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L32A32_UINT.0)),
        ("PIPE_FORMAT_A32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A32_SINT.0)),
        ("PIPE_FORMAT_I32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_I32_SINT.0)),
        ("PIPE_FORMAT_L32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L32_SINT.0)),
        ("PIPE_FORMAT_L32A32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_L32A32_SINT.0)),
        ("PIPE_FORMAT_A8R8G8B8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8R8G8B8_UINT.0)),
        ("PIPE_FORMAT_A8B8G8R8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8B8G8R8_UINT.0)),
        ("PIPE_FORMAT_A2R10G10B10_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A2R10G10B10_UINT.0)),
        ("PIPE_FORMAT_A2B10G10R10_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A2B10G10R10_UINT.0)),
        ("PIPE_FORMAT_R5G6B5_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R5G6B5_UINT.0)),
        ("PIPE_FORMAT_B5G6R5_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B5G6R5_UINT.0)),
        ("PIPE_FORMAT_R3G3B2_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R3G3B2_UINT.0)),
        ("PIPE_FORMAT_B2G3R3_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B2G3R3_UINT.0)),
        ("PIPE_FORMAT_R4G4B4A4_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R4G4B4A4_UINT.0)),
        ("PIPE_FORMAT_B4G4R4A4_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B4G4R4A4_UINT.0)),
        ("PIPE_FORMAT_A4R4G4B4_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A4R4G4B4_UINT.0)),
        ("PIPE_FORMAT_A4B4G4R4_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A4B4G4R4_UINT.0)),
        ("PIPE_FORMAT_A1R5G5B5_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A1R5G5B5_UINT.0)),
        ("PIPE_FORMAT_A1B5G5R5_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A1B5G5R5_UINT.0)),
        ("PIPE_FORMAT_R5G5B5A1_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R5G5B5A1_UINT.0)),
        ("PIPE_FORMAT_B5G5R5A1_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B5G5R5A1_UINT.0)),
        ("PIPE_FORMAT_R8G8B8X8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8X8_SNORM.0)),
        ("PIPE_FORMAT_R8G8B8X8_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8X8_SRGB.0)),
        ("PIPE_FORMAT_R8G8B8X8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8X8_UINT.0)),
        ("PIPE_FORMAT_R8G8B8X8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8G8B8X8_SINT.0)),
        ("PIPE_FORMAT_B10G10R10X2_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B10G10R10X2_UNORM.0)),
        ("PIPE_FORMAT_B10G10R10X2_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B10G10R10X2_SNORM.0)),
        ("PIPE_FORMAT_B10G10R10X2_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B10G10R10X2_SINT.0)),
        ("PIPE_FORMAT_R16G16B16X16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16X16_UNORM.0)),
        ("PIPE_FORMAT_R16G16B16X16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16X16_SNORM.0)),
        ("PIPE_FORMAT_R16G16B16X16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16X16_FLOAT.0)),
        ("PIPE_FORMAT_R16G16B16X16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16X16_UINT.0)),
        ("PIPE_FORMAT_R16G16B16X16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16G16B16X16_SINT.0)),
        ("PIPE_FORMAT_R32G32B32X32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32X32_FLOAT.0)),
        ("PIPE_FORMAT_R32G32B32X32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32X32_UINT.0)),
        ("PIPE_FORMAT_R32G32B32X32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32G32B32X32_SINT.0)),
        ("PIPE_FORMAT_R8A8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8A8_SNORM.0)),
        ("PIPE_FORMAT_R16A16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16A16_UNORM.0)),
        ("PIPE_FORMAT_R16A16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16A16_SNORM.0)),
        ("PIPE_FORMAT_R16A16_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16A16_FLOAT.0)),
        ("PIPE_FORMAT_R32A32_FLOAT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32A32_FLOAT.0)),
        ("PIPE_FORMAT_R8A8_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8A8_UINT.0)),
        ("PIPE_FORMAT_R8A8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R8A8_SINT.0)),
        ("PIPE_FORMAT_R16A16_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16A16_UINT.0)),
        ("PIPE_FORMAT_R16A16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R16A16_SINT.0)),
        ("PIPE_FORMAT_R32A32_UINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32A32_UINT.0)),
        ("PIPE_FORMAT_R32A32_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R32A32_SINT.0)),
        ("PIPE_FORMAT_B5G6R5_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_B5G6R5_SRGB.0)),
        ("PIPE_FORMAT_R5G6B5_SRGB", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_R5G6B5_SRGB.0)),
        ("PIPE_FORMAT_G8R8_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8R8_UNORM.0)),
        ("PIPE_FORMAT_G8R8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8R8_SNORM.0)),
        ("PIPE_FORMAT_G8R8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G8R8_SINT.0)),
        ("PIPE_FORMAT_G16R16_UNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G16R16_UNORM.0)),
        ("PIPE_FORMAT_G16R16_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G16R16_SNORM.0)),
        ("PIPE_FORMAT_G16R16_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_G16R16_SINT.0)),
        ("PIPE_FORMAT_A8B8G8R8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8B8G8R8_SNORM.0)),
        ("PIPE_FORMAT_A8B8G8R8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_A8B8G8R8_SINT.0)),
        ("PIPE_FORMAT_X8B8G8R8_SNORM", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8B8G8R8_SNORM.0)),
        ("PIPE_FORMAT_X8B8G8R8_SINT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_X8B8G8R8_SINT.0)),
        ("PIPE_FORMAT_COUNT", i64::from(wcontainers_gallium::values::Format::PIPE_FORMAT_COUNT.0)),
        ("PIPE_SWIZZLE_X", i64::from(wcontainers_gallium::values::Swizzle::PIPE_SWIZZLE_X.0)),
        ("PIPE_SWIZZLE_Y", i64::from(wcontainers_gallium::values::Swizzle::PIPE_SWIZZLE_Y.0)),
        ("PIPE_SWIZZLE_Z", i64::from(wcontainers_gallium::values::Swizzle::PIPE_SWIZZLE_Z.0)),
        ("PIPE_SWIZZLE_W", i64::from(wcontainers_gallium::values::Swizzle::PIPE_SWIZZLE_W.0)),
        ("PIPE_SWIZZLE_0", i64::from(wcontainers_gallium::values::Swizzle::PIPE_SWIZZLE_0.0)),
        ("PIPE_SWIZZLE_1", i64::from(wcontainers_gallium::values::Swizzle::PIPE_SWIZZLE_1.0)),
        ("PIPE_SWIZZLE_NONE", i64::from(wcontainers_gallium::values::Swizzle::PIPE_SWIZZLE_NONE.0)),
        ("PIPE_SWIZZLE_MAX", i64::from(wcontainers_gallium::values::Swizzle::PIPE_SWIZZLE_MAX.0)),
        ("PIPE_BLENDFACTOR_ONE", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_ONE.0)),
        ("PIPE_BLENDFACTOR_SRC_COLOR", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_SRC_COLOR.0)),
        ("PIPE_BLENDFACTOR_SRC_ALPHA", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_SRC_ALPHA.0)),
        ("PIPE_BLENDFACTOR_DST_ALPHA", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_DST_ALPHA.0)),
        ("PIPE_BLENDFACTOR_DST_COLOR", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_DST_COLOR.0)),
        ("PIPE_BLENDFACTOR_SRC_ALPHA_SATURATE", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_SRC_ALPHA_SATURATE.0)),
        ("PIPE_BLENDFACTOR_CONST_COLOR", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_CONST_COLOR.0)),
        ("PIPE_BLENDFACTOR_CONST_ALPHA", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_CONST_ALPHA.0)),
        ("PIPE_BLENDFACTOR_SRC1_COLOR", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_SRC1_COLOR.0)),
        ("PIPE_BLENDFACTOR_SRC1_ALPHA", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_SRC1_ALPHA.0)),
        ("PIPE_BLENDFACTOR_ZERO", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_ZERO.0)),
        ("PIPE_BLENDFACTOR_INV_SRC_COLOR", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_INV_SRC_COLOR.0)),
        ("PIPE_BLENDFACTOR_INV_SRC_ALPHA", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_INV_SRC_ALPHA.0)),
        ("PIPE_BLENDFACTOR_INV_DST_ALPHA", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_INV_DST_ALPHA.0)),
        ("PIPE_BLENDFACTOR_INV_DST_COLOR", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_INV_DST_COLOR.0)),
        ("PIPE_BLENDFACTOR_INV_CONST_COLOR", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_INV_CONST_COLOR.0)),
        ("PIPE_BLENDFACTOR_INV_CONST_ALPHA", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_INV_CONST_ALPHA.0)),
        ("PIPE_BLENDFACTOR_INV_SRC1_COLOR", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_INV_SRC1_COLOR.0)),
        ("PIPE_BLENDFACTOR_INV_SRC1_ALPHA", i64::from(wcontainers_gallium::values::Blendfactor::PIPE_BLENDFACTOR_INV_SRC1_ALPHA.0)),
        ("PIPE_BLEND_ADD", i64::from(wcontainers_gallium::values::BlendFunc::PIPE_BLEND_ADD.0)),
        ("PIPE_BLEND_SUBTRACT", i64::from(wcontainers_gallium::values::BlendFunc::PIPE_BLEND_SUBTRACT.0)),
        ("PIPE_BLEND_REVERSE_SUBTRACT", i64::from(wcontainers_gallium::values::BlendFunc::PIPE_BLEND_REVERSE_SUBTRACT.0)),
        ("PIPE_BLEND_MIN", i64::from(wcontainers_gallium::values::BlendFunc::PIPE_BLEND_MIN.0)),
        ("PIPE_BLEND_MAX", i64::from(wcontainers_gallium::values::BlendFunc::PIPE_BLEND_MAX.0)),
        ("PIPE_LOGICOP_CLEAR", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_CLEAR.0)),
        ("PIPE_LOGICOP_NOR", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_NOR.0)),
        ("PIPE_LOGICOP_AND_INVERTED", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_AND_INVERTED.0)),
        ("PIPE_LOGICOP_COPY_INVERTED", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_COPY_INVERTED.0)),
        ("PIPE_LOGICOP_AND_REVERSE", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_AND_REVERSE.0)),
        ("PIPE_LOGICOP_INVERT", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_INVERT.0)),
        ("PIPE_LOGICOP_XOR", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_XOR.0)),
        ("PIPE_LOGICOP_NAND", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_NAND.0)),
        ("PIPE_LOGICOP_AND", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_AND.0)),
        ("PIPE_LOGICOP_EQUIV", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_EQUIV.0)),
        ("PIPE_LOGICOP_NOOP", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_NOOP.0)),
        ("PIPE_LOGICOP_OR_INVERTED", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_OR_INVERTED.0)),
        ("PIPE_LOGICOP_COPY", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_COPY.0)),
        ("PIPE_LOGICOP_OR_REVERSE", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_OR_REVERSE.0)),
        ("PIPE_LOGICOP_OR", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_OR.0)),
        ("PIPE_LOGICOP_SET", i64::from(wcontainers_gallium::values::Logicop::PIPE_LOGICOP_SET.0)),
        ("PIPE_ADVANCED_BLEND_NONE", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_NONE.0)),
        ("PIPE_ADVANCED_BLEND_MULTIPLY", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_MULTIPLY.0)),
        ("PIPE_ADVANCED_BLEND_SCREEN", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_SCREEN.0)),
        ("PIPE_ADVANCED_BLEND_OVERLAY", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_OVERLAY.0)),
        ("PIPE_ADVANCED_BLEND_DARKEN", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_DARKEN.0)),
        ("PIPE_ADVANCED_BLEND_LIGHTEN", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_LIGHTEN.0)),
        ("PIPE_ADVANCED_BLEND_COLORDODGE", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_COLORDODGE.0)),
        ("PIPE_ADVANCED_BLEND_COLORBURN", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_COLORBURN.0)),
        ("PIPE_ADVANCED_BLEND_HARDLIGHT", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_HARDLIGHT.0)),
        ("PIPE_ADVANCED_BLEND_SOFTLIGHT", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_SOFTLIGHT.0)),
        ("PIPE_ADVANCED_BLEND_DIFFERENCE", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_DIFFERENCE.0)),
        ("PIPE_ADVANCED_BLEND_EXCLUSION", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_EXCLUSION.0)),
        ("PIPE_ADVANCED_BLEND_HSL_HUE", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_HSL_HUE.0)),
        ("PIPE_ADVANCED_BLEND_HSL_SATURATION", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_HSL_SATURATION.0)),
        ("PIPE_ADVANCED_BLEND_HSL_COLOR", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_HSL_COLOR.0)),
        ("PIPE_ADVANCED_BLEND_HSL_LUMINOSITY", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_HSL_LUMINOSITY.0)),
        ("PIPE_ADVANCED_BLEND_SRC", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_SRC.0)),
        ("PIPE_ADVANCED_BLEND_DST", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_DST.0)),
        ("PIPE_ADVANCED_BLEND_SRC_OVER", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_SRC_OVER.0)),
        ("PIPE_ADVANCED_BLEND_DST_OVER", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_DST_OVER.0)),
        ("PIPE_ADVANCED_BLEND_SRC_IN", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_SRC_IN.0)),
        ("PIPE_ADVANCED_BLEND_DST_IN", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_DST_IN.0)),
        ("PIPE_ADVANCED_BLEND_SRC_OUT", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_SRC_OUT.0)),
        ("PIPE_ADVANCED_BLEND_DST_OUT", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_DST_OUT.0)),
        ("PIPE_ADVANCED_BLEND_SRC_ATOP", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_SRC_ATOP.0)),
        ("PIPE_ADVANCED_BLEND_DST_ATOP", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_DST_ATOP.0)),
        ("PIPE_ADVANCED_BLEND_XOR", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_XOR.0)),
        ("PIPE_ADVANCED_BLEND_INVERT", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_INVERT.0)),
        ("PIPE_ADVANCED_BLEND_INVERT_RGB", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_INVERT_RGB.0)),
        ("PIPE_ADVANCED_BLEND_LINEARDODGE", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_LINEARDODGE.0)),
        ("PIPE_ADVANCED_BLEND_LINEARBURN", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_LINEARBURN.0)),
        ("PIPE_ADVANCED_BLEND_VIVIDLIGHT", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_VIVIDLIGHT.0)),
        ("PIPE_ADVANCED_BLEND_LINEARLIGHT", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_LINEARLIGHT.0)),
        ("PIPE_ADVANCED_BLEND_PINLIGHT", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_PINLIGHT.0)),
        ("PIPE_ADVANCED_BLEND_HARDMIX", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_HARDMIX.0)),
        ("PIPE_ADVANCED_BLEND_PLUS", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_PLUS.0)),
        ("PIPE_ADVANCED_BLEND_PLUS_CLAMPED", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_PLUS_CLAMPED.0)),
        ("PIPE_ADVANCED_BLEND_PLUS_CLAMPED_ALPHA", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_PLUS_CLAMPED_ALPHA.0)),
        ("PIPE_ADVANCED_BLEND_PLUS_DARKER", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_PLUS_DARKER.0)),
        ("PIPE_ADVANCED_BLEND_MINUS", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_MINUS.0)),
        ("PIPE_ADVANCED_BLEND_MINUS_CLAMPED", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_MINUS_CLAMPED.0)),
        ("PIPE_ADVANCED_BLEND_CONTRAST", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_CONTRAST.0)),
        ("PIPE_ADVANCED_BLEND_INVERT_OVG", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_INVERT_OVG.0)),
        ("PIPE_ADVANCED_BLEND_RED", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_RED.0)),
        ("PIPE_ADVANCED_BLEND_GREEN", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_GREEN.0)),
        ("PIPE_ADVANCED_BLEND_BLUE", i64::from(wcontainers_gallium::values::AdvancedBlendMode::PIPE_ADVANCED_BLEND_BLUE.0)),
        ("PIPE_BLEND_OVERLAP_UNCORRELATED", i64::from(wcontainers_gallium::values::BlendOverlapMode::PIPE_BLEND_OVERLAP_UNCORRELATED.0)),
        ("PIPE_BLEND_OVERLAP_CONJOINT", i64::from(wcontainers_gallium::values::BlendOverlapMode::PIPE_BLEND_OVERLAP_CONJOINT.0)),
        ("PIPE_BLEND_OVERLAP_DISJOINT", i64::from(wcontainers_gallium::values::BlendOverlapMode::PIPE_BLEND_OVERLAP_DISJOINT.0)),
        ("PIPE_OK", i64::from(wcontainers_gallium::values::Error::PIPE_OK.0)),
        ("PIPE_ERROR", i64::from(wcontainers_gallium::values::Error::PIPE_ERROR.0)),
        ("PIPE_ERROR_BAD_INPUT", i64::from(wcontainers_gallium::values::Error::PIPE_ERROR_BAD_INPUT.0)),
        ("PIPE_ERROR_OUT_OF_MEMORY", i64::from(wcontainers_gallium::values::Error::PIPE_ERROR_OUT_OF_MEMORY.0)),
        ("PIPE_ERROR_RETRY", i64::from(wcontainers_gallium::values::Error::PIPE_ERROR_RETRY.0)),
        ("PIPE_FUNC_NEVER", i64::from(wcontainers_gallium::values::CompareFunc::PIPE_FUNC_NEVER.0)),
        ("PIPE_FUNC_LESS", i64::from(wcontainers_gallium::values::CompareFunc::PIPE_FUNC_LESS.0)),
        ("PIPE_FUNC_EQUAL", i64::from(wcontainers_gallium::values::CompareFunc::PIPE_FUNC_EQUAL.0)),
        ("PIPE_FUNC_LEQUAL", i64::from(wcontainers_gallium::values::CompareFunc::PIPE_FUNC_LEQUAL.0)),
        ("PIPE_FUNC_GREATER", i64::from(wcontainers_gallium::values::CompareFunc::PIPE_FUNC_GREATER.0)),
        ("PIPE_FUNC_NOTEQUAL", i64::from(wcontainers_gallium::values::CompareFunc::PIPE_FUNC_NOTEQUAL.0)),
        ("PIPE_FUNC_GEQUAL", i64::from(wcontainers_gallium::values::CompareFunc::PIPE_FUNC_GEQUAL.0)),
        ("PIPE_FUNC_ALWAYS", i64::from(wcontainers_gallium::values::CompareFunc::PIPE_FUNC_ALWAYS.0)),
        ("PIPE_STENCIL_OP_KEEP", i64::from(wcontainers_gallium::values::StencilOp::PIPE_STENCIL_OP_KEEP.0)),
        ("PIPE_STENCIL_OP_ZERO", i64::from(wcontainers_gallium::values::StencilOp::PIPE_STENCIL_OP_ZERO.0)),
        ("PIPE_STENCIL_OP_REPLACE", i64::from(wcontainers_gallium::values::StencilOp::PIPE_STENCIL_OP_REPLACE.0)),
        ("PIPE_STENCIL_OP_INCR", i64::from(wcontainers_gallium::values::StencilOp::PIPE_STENCIL_OP_INCR.0)),
        ("PIPE_STENCIL_OP_DECR", i64::from(wcontainers_gallium::values::StencilOp::PIPE_STENCIL_OP_DECR.0)),
        ("PIPE_STENCIL_OP_INCR_WRAP", i64::from(wcontainers_gallium::values::StencilOp::PIPE_STENCIL_OP_INCR_WRAP.0)),
        ("PIPE_STENCIL_OP_DECR_WRAP", i64::from(wcontainers_gallium::values::StencilOp::PIPE_STENCIL_OP_DECR_WRAP.0)),
        ("PIPE_STENCIL_OP_INVERT", i64::from(wcontainers_gallium::values::StencilOp::PIPE_STENCIL_OP_INVERT.0)),
        ("PIPE_BUFFER", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_BUFFER.0)),
        ("PIPE_TEXTURE_1D", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_TEXTURE_1D.0)),
        ("PIPE_TEXTURE_2D", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_TEXTURE_2D.0)),
        ("PIPE_TEXTURE_3D", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_TEXTURE_3D.0)),
        ("PIPE_TEXTURE_CUBE", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_TEXTURE_CUBE.0)),
        ("PIPE_TEXTURE_RECT", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_TEXTURE_RECT.0)),
        ("PIPE_TEXTURE_1D_ARRAY", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_TEXTURE_1D_ARRAY.0)),
        ("PIPE_TEXTURE_2D_ARRAY", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_TEXTURE_2D_ARRAY.0)),
        ("PIPE_TEXTURE_CUBE_ARRAY", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_TEXTURE_CUBE_ARRAY.0)),
        ("PIPE_MAX_TEXTURE_TYPES", i64::from(wcontainers_gallium::values::TextureTarget::PIPE_MAX_TEXTURE_TYPES.0)),
        ("PIPE_TEX_FACE_POS_X", i64::from(wcontainers_gallium::values::TexFace::PIPE_TEX_FACE_POS_X.0)),
        ("PIPE_TEX_FACE_NEG_X", i64::from(wcontainers_gallium::values::TexFace::PIPE_TEX_FACE_NEG_X.0)),
        ("PIPE_TEX_FACE_POS_Y", i64::from(wcontainers_gallium::values::TexFace::PIPE_TEX_FACE_POS_Y.0)),
        ("PIPE_TEX_FACE_NEG_Y", i64::from(wcontainers_gallium::values::TexFace::PIPE_TEX_FACE_NEG_Y.0)),
        ("PIPE_TEX_FACE_POS_Z", i64::from(wcontainers_gallium::values::TexFace::PIPE_TEX_FACE_POS_Z.0)),
        ("PIPE_TEX_FACE_NEG_Z", i64::from(wcontainers_gallium::values::TexFace::PIPE_TEX_FACE_NEG_Z.0)),
        ("PIPE_TEX_FACE_MAX", i64::from(wcontainers_gallium::values::TexFace::PIPE_TEX_FACE_MAX.0)),
        ("PIPE_TEX_WRAP_REPEAT", i64::from(wcontainers_gallium::values::TexWrap::PIPE_TEX_WRAP_REPEAT.0)),
        ("PIPE_TEX_WRAP_CLAMP", i64::from(wcontainers_gallium::values::TexWrap::PIPE_TEX_WRAP_CLAMP.0)),
        ("PIPE_TEX_WRAP_CLAMP_TO_EDGE", i64::from(wcontainers_gallium::values::TexWrap::PIPE_TEX_WRAP_CLAMP_TO_EDGE.0)),
        ("PIPE_TEX_WRAP_CLAMP_TO_BORDER", i64::from(wcontainers_gallium::values::TexWrap::PIPE_TEX_WRAP_CLAMP_TO_BORDER.0)),
        ("PIPE_TEX_WRAP_MIRROR_REPEAT", i64::from(wcontainers_gallium::values::TexWrap::PIPE_TEX_WRAP_MIRROR_REPEAT.0)),
        ("PIPE_TEX_WRAP_MIRROR_CLAMP", i64::from(wcontainers_gallium::values::TexWrap::PIPE_TEX_WRAP_MIRROR_CLAMP.0)),
        ("PIPE_TEX_WRAP_MIRROR_CLAMP_TO_EDGE", i64::from(wcontainers_gallium::values::TexWrap::PIPE_TEX_WRAP_MIRROR_CLAMP_TO_EDGE.0)),
        ("PIPE_TEX_WRAP_MIRROR_CLAMP_TO_BORDER", i64::from(wcontainers_gallium::values::TexWrap::PIPE_TEX_WRAP_MIRROR_CLAMP_TO_BORDER.0)),
        ("PIPE_TEX_MIPFILTER_NEAREST", i64::from(wcontainers_gallium::values::TexMipfilter::PIPE_TEX_MIPFILTER_NEAREST.0)),
        ("PIPE_TEX_MIPFILTER_LINEAR", i64::from(wcontainers_gallium::values::TexMipfilter::PIPE_TEX_MIPFILTER_LINEAR.0)),
        ("PIPE_TEX_MIPFILTER_NONE", i64::from(wcontainers_gallium::values::TexMipfilter::PIPE_TEX_MIPFILTER_NONE.0)),
        ("PIPE_TEX_FILTER_NEAREST", i64::from(wcontainers_gallium::values::TexFilter::PIPE_TEX_FILTER_NEAREST.0)),
        ("PIPE_TEX_FILTER_LINEAR", i64::from(wcontainers_gallium::values::TexFilter::PIPE_TEX_FILTER_LINEAR.0)),
        ("PIPE_TEX_COMPARE_NONE", i64::from(wcontainers_gallium::values::TexCompare::PIPE_TEX_COMPARE_NONE.0)),
        ("PIPE_TEX_COMPARE_R_TO_TEXTURE", i64::from(wcontainers_gallium::values::TexCompare::PIPE_TEX_COMPARE_R_TO_TEXTURE.0)),
        ("PIPE_TEX_REDUCTION_WEIGHTED_AVERAGE", i64::from(wcontainers_gallium::values::TexReductionMode::PIPE_TEX_REDUCTION_WEIGHTED_AVERAGE.0)),
        ("PIPE_TEX_REDUCTION_MIN", i64::from(wcontainers_gallium::values::TexReductionMode::PIPE_TEX_REDUCTION_MIN.0)),
        ("PIPE_TEX_REDUCTION_MAX", i64::from(wcontainers_gallium::values::TexReductionMode::PIPE_TEX_REDUCTION_MAX.0)),
        ("PIPE_MAP_NONE", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_NONE.0)),
        ("PIPE_MAP_READ", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_READ.0)),
        ("PIPE_MAP_WRITE", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_WRITE.0)),
        ("PIPE_MAP_READ_WRITE", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_READ_WRITE.0)),
        ("PIPE_MAP_DIRECTLY", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_DIRECTLY.0)),
        ("PIPE_MAP_DISCARD_RANGE", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_DISCARD_RANGE.0)),
        ("PIPE_MAP_DONTBLOCK", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_DONTBLOCK.0)),
        ("PIPE_MAP_UNSYNCHRONIZED", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_UNSYNCHRONIZED.0)),
        ("PIPE_MAP_FLUSH_EXPLICIT", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_FLUSH_EXPLICIT.0)),
        ("PIPE_MAP_DISCARD_WHOLE_RESOURCE", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_DISCARD_WHOLE_RESOURCE.0)),
        ("PIPE_MAP_PERSISTENT", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_PERSISTENT.0)),
        ("PIPE_MAP_COHERENT", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_COHERENT.0)),
        ("PIPE_MAP_THREAD_SAFE", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_THREAD_SAFE.0)),
        ("PIPE_MAP_DEPTH_ONLY", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_DEPTH_ONLY.0)),
        ("PIPE_MAP_STENCIL_ONLY", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_STENCIL_ONLY.0)),
        ("PIPE_MAP_ONCE", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_ONCE.0)),
        ("PIPE_MAP_CLIENT_POINTER", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_CLIENT_POINTER.0)),
        ("PIPE_MAP_SHORT_LIVED", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_SHORT_LIVED.0)),
        ("PIPE_MAP_DRV_PRV", i64::from(wcontainers_gallium::values::MapFlags::PIPE_MAP_DRV_PRV.0)),
        ("PIPE_FLUSH_END_OF_FRAME", i64::from(wcontainers_gallium::values::FlushFlags::PIPE_FLUSH_END_OF_FRAME.0)),
        ("PIPE_FLUSH_DEFERRED", i64::from(wcontainers_gallium::values::FlushFlags::PIPE_FLUSH_DEFERRED.0)),
        ("PIPE_FLUSH_FENCE_FD", i64::from(wcontainers_gallium::values::FlushFlags::PIPE_FLUSH_FENCE_FD.0)),
        ("PIPE_FLUSH_ASYNC", i64::from(wcontainers_gallium::values::FlushFlags::PIPE_FLUSH_ASYNC.0)),
        ("PIPE_FLUSH_HINT_FINISH", i64::from(wcontainers_gallium::values::FlushFlags::PIPE_FLUSH_HINT_FINISH.0)),
        ("PIPE_FLUSH_TOP_OF_PIPE", i64::from(wcontainers_gallium::values::FlushFlags::PIPE_FLUSH_TOP_OF_PIPE.0)),
        ("PIPE_FLUSH_BOTTOM_OF_PIPE", i64::from(wcontainers_gallium::values::FlushFlags::PIPE_FLUSH_BOTTOM_OF_PIPE.0)),
        ("PIPE_FLUSH_INTERMEDIATE", i64::from(wcontainers_gallium::values::FlushFlags::PIPE_FLUSH_INTERMEDIATE.0)),
        ("PIPE_USAGE_DEFAULT", i64::from(wcontainers_gallium::values::ResourceUsage::PIPE_USAGE_DEFAULT.0)),
        ("PIPE_USAGE_IMMUTABLE", i64::from(wcontainers_gallium::values::ResourceUsage::PIPE_USAGE_IMMUTABLE.0)),
        ("PIPE_USAGE_DYNAMIC", i64::from(wcontainers_gallium::values::ResourceUsage::PIPE_USAGE_DYNAMIC.0)),
        ("PIPE_USAGE_STREAM", i64::from(wcontainers_gallium::values::ResourceUsage::PIPE_USAGE_STREAM.0)),
        ("PIPE_USAGE_STAGING", i64::from(wcontainers_gallium::values::ResourceUsage::PIPE_USAGE_STAGING.0)),
        ("PIPE_TESS_SPACING_FRACTIONAL_ODD", i64::from(wcontainers_gallium::values::TessSpacing::PIPE_TESS_SPACING_FRACTIONAL_ODD.0)),
        ("PIPE_TESS_SPACING_FRACTIONAL_EVEN", i64::from(wcontainers_gallium::values::TessSpacing::PIPE_TESS_SPACING_FRACTIONAL_EVEN.0)),
        ("PIPE_TESS_SPACING_EQUAL", i64::from(wcontainers_gallium::values::TessSpacing::PIPE_TESS_SPACING_EQUAL.0)),
        ("PIPE_QUERY_OCCLUSION_COUNTER", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_OCCLUSION_COUNTER.0)),
        ("PIPE_QUERY_OCCLUSION_PREDICATE", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_OCCLUSION_PREDICATE.0)),
        ("PIPE_QUERY_OCCLUSION_PREDICATE_CONSERVATIVE", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_OCCLUSION_PREDICATE_CONSERVATIVE.0)),
        ("PIPE_QUERY_TIMESTAMP", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_TIMESTAMP.0)),
        ("PIPE_QUERY_TIMESTAMP_DISJOINT", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_TIMESTAMP_DISJOINT.0)),
        ("PIPE_QUERY_TIME_ELAPSED", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_TIME_ELAPSED.0)),
        ("PIPE_QUERY_PRIMITIVES_GENERATED", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_PRIMITIVES_GENERATED.0)),
        ("PIPE_QUERY_PRIMITIVES_EMITTED", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_PRIMITIVES_EMITTED.0)),
        ("PIPE_QUERY_SO_STATISTICS", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_SO_STATISTICS.0)),
        ("PIPE_QUERY_SO_OVERFLOW_PREDICATE", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_SO_OVERFLOW_PREDICATE.0)),
        ("PIPE_QUERY_SO_OVERFLOW_ANY_PREDICATE", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_SO_OVERFLOW_ANY_PREDICATE.0)),
        ("PIPE_QUERY_GPU_FINISHED", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_GPU_FINISHED.0)),
        ("PIPE_QUERY_PIPELINE_STATISTICS", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_PIPELINE_STATISTICS.0)),
        ("PIPE_QUERY_PIPELINE_STATISTICS_SINGLE", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_PIPELINE_STATISTICS_SINGLE.0)),
        ("PIPE_QUERY_TIMESTAMP_RAW", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_TIMESTAMP_RAW.0)),
        ("PIPE_QUERY_TYPES", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_TYPES.0)),
        ("PIPE_QUERY_DRIVER_SPECIFIC", i64::from(wcontainers_gallium::values::QueryType::PIPE_QUERY_DRIVER_SPECIFIC.0)),
        ("PIPE_STAT_QUERY_IA_VERTICES", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_IA_VERTICES.0)),
        ("PIPE_STAT_QUERY_IA_PRIMITIVES", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_IA_PRIMITIVES.0)),
        ("PIPE_STAT_QUERY_VS_INVOCATIONS", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_VS_INVOCATIONS.0)),
        ("PIPE_STAT_QUERY_GS_INVOCATIONS", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_GS_INVOCATIONS.0)),
        ("PIPE_STAT_QUERY_GS_PRIMITIVES", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_GS_PRIMITIVES.0)),
        ("PIPE_STAT_QUERY_C_INVOCATIONS", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_C_INVOCATIONS.0)),
        ("PIPE_STAT_QUERY_C_PRIMITIVES", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_C_PRIMITIVES.0)),
        ("PIPE_STAT_QUERY_PS_INVOCATIONS", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_PS_INVOCATIONS.0)),
        ("PIPE_STAT_QUERY_HS_INVOCATIONS", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_HS_INVOCATIONS.0)),
        ("PIPE_STAT_QUERY_DS_INVOCATIONS", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_DS_INVOCATIONS.0)),
        ("PIPE_STAT_QUERY_CS_INVOCATIONS", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_CS_INVOCATIONS.0)),
        ("PIPE_STAT_QUERY_TS_INVOCATIONS", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_TS_INVOCATIONS.0)),
        ("PIPE_STAT_QUERY_MS_INVOCATIONS", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_MS_INVOCATIONS.0)),
        ("PIPE_STAT_QUERY_MS_PRIMITIVES", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_MS_PRIMITIVES.0)),
        ("PIPE_STAT_QUERY_COUNT", i64::from(wcontainers_gallium::values::StatisticsQueryIndex::PIPE_STAT_QUERY_COUNT.0)),
        ("PIPE_RENDER_COND_WAIT", i64::from(wcontainers_gallium::values::RenderCondFlag::PIPE_RENDER_COND_WAIT.0)),
        ("PIPE_RENDER_COND_NO_WAIT", i64::from(wcontainers_gallium::values::RenderCondFlag::PIPE_RENDER_COND_NO_WAIT.0)),
        ("PIPE_RENDER_COND_BY_REGION_WAIT", i64::from(wcontainers_gallium::values::RenderCondFlag::PIPE_RENDER_COND_BY_REGION_WAIT.0)),
        ("PIPE_RENDER_COND_BY_REGION_NO_WAIT", i64::from(wcontainers_gallium::values::RenderCondFlag::PIPE_RENDER_COND_BY_REGION_NO_WAIT.0)),
        ("PIPE_SPRITE_COORD_UPPER_LEFT", i64::from(wcontainers_gallium::values::SpriteCoordMode::PIPE_SPRITE_COORD_UPPER_LEFT.0)),
        ("PIPE_SPRITE_COORD_LOWER_LEFT", i64::from(wcontainers_gallium::values::SpriteCoordMode::PIPE_SPRITE_COORD_LOWER_LEFT.0)),
        ("PIPE_VIEWPORT_SWIZZLE_POSITIVE_X", i64::from(wcontainers_gallium::values::ViewportSwizzle::PIPE_VIEWPORT_SWIZZLE_POSITIVE_X.0)),
        ("PIPE_VIEWPORT_SWIZZLE_NEGATIVE_X", i64::from(wcontainers_gallium::values::ViewportSwizzle::PIPE_VIEWPORT_SWIZZLE_NEGATIVE_X.0)),
        ("PIPE_VIEWPORT_SWIZZLE_POSITIVE_Y", i64::from(wcontainers_gallium::values::ViewportSwizzle::PIPE_VIEWPORT_SWIZZLE_POSITIVE_Y.0)),
        ("PIPE_VIEWPORT_SWIZZLE_NEGATIVE_Y", i64::from(wcontainers_gallium::values::ViewportSwizzle::PIPE_VIEWPORT_SWIZZLE_NEGATIVE_Y.0)),
        ("PIPE_VIEWPORT_SWIZZLE_POSITIVE_Z", i64::from(wcontainers_gallium::values::ViewportSwizzle::PIPE_VIEWPORT_SWIZZLE_POSITIVE_Z.0)),
        ("PIPE_VIEWPORT_SWIZZLE_NEGATIVE_Z", i64::from(wcontainers_gallium::values::ViewportSwizzle::PIPE_VIEWPORT_SWIZZLE_NEGATIVE_Z.0)),
        ("PIPE_VIEWPORT_SWIZZLE_POSITIVE_W", i64::from(wcontainers_gallium::values::ViewportSwizzle::PIPE_VIEWPORT_SWIZZLE_POSITIVE_W.0)),
        ("PIPE_VIEWPORT_SWIZZLE_NEGATIVE_W", i64::from(wcontainers_gallium::values::ViewportSwizzle::PIPE_VIEWPORT_SWIZZLE_NEGATIVE_W.0)),
        ("PIPE_NO_RESET", i64::from(wcontainers_gallium::values::ResetStatus::PIPE_NO_RESET.0)),
        ("PIPE_GUILTY_CONTEXT_RESET", i64::from(wcontainers_gallium::values::ResetStatus::PIPE_GUILTY_CONTEXT_RESET.0)),
        ("PIPE_INNOCENT_CONTEXT_RESET", i64::from(wcontainers_gallium::values::ResetStatus::PIPE_INNOCENT_CONTEXT_RESET.0)),
        ("PIPE_UNKNOWN_CONTEXT_RESET", i64::from(wcontainers_gallium::values::ResetStatus::PIPE_UNKNOWN_CONTEXT_RESET.0)),
        ("PIPE_VERTEX_INPUT_ALIGNMENT_NONE", i64::from(wcontainers_gallium::values::VertexInputAlignment::PIPE_VERTEX_INPUT_ALIGNMENT_NONE.0)),
        ("PIPE_VERTEX_INPUT_ALIGNMENT_4BYTE", i64::from(wcontainers_gallium::values::VertexInputAlignment::PIPE_VERTEX_INPUT_ALIGNMENT_4BYTE.0)),
        ("PIPE_VERTEX_INPUT_ALIGNMENT_ELEMENT", i64::from(wcontainers_gallium::values::VertexInputAlignment::PIPE_VERTEX_INPUT_ALIGNMENT_ELEMENT.0)),
        ("PIPE_CONSERVATIVE_RASTER_OFF", i64::from(wcontainers_gallium::values::ConservativeRasterMode::PIPE_CONSERVATIVE_RASTER_OFF.0)),
        ("PIPE_CONSERVATIVE_RASTER_POST_SNAP", i64::from(wcontainers_gallium::values::ConservativeRasterMode::PIPE_CONSERVATIVE_RASTER_POST_SNAP.0)),
        ("PIPE_CONSERVATIVE_RASTER_PRE_SNAP", i64::from(wcontainers_gallium::values::ConservativeRasterMode::PIPE_CONSERVATIVE_RASTER_PRE_SNAP.0)),
        ("PIPE_POINT_SIZE_LOWER_ALWAYS", i64::from(wcontainers_gallium::values::PointSizeLowerMode::PIPE_POINT_SIZE_LOWER_ALWAYS.0)),
        ("PIPE_POINT_SIZE_LOWER_NEVER", i64::from(wcontainers_gallium::values::PointSizeLowerMode::PIPE_POINT_SIZE_LOWER_NEVER.0)),
        ("PIPE_POINT_SIZE_LOWER_USER_ONLY", i64::from(wcontainers_gallium::values::PointSizeLowerMode::PIPE_POINT_SIZE_LOWER_USER_ONLY.0)),
        ("PIPE_TEXTURE_TRANSFER_DEFAULT", i64::from(wcontainers_gallium::values::TextureTransferMode::PIPE_TEXTURE_TRANSFER_DEFAULT.0)),
        ("PIPE_TEXTURE_TRANSFER_BLIT", i64::from(wcontainers_gallium::values::TextureTransferMode::PIPE_TEXTURE_TRANSFER_BLIT.0)),
        ("PIPE_TEXTURE_TRANSFER_COMPUTE", i64::from(wcontainers_gallium::values::TextureTransferMode::PIPE_TEXTURE_TRANSFER_COMPUTE.0)),
        ("PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_NV50", i64::from(wcontainers_gallium::values::QuirkTextureBorderColorSwizzle::PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_NV50.0)),
        ("PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_R600", i64::from(wcontainers_gallium::values::QuirkTextureBorderColorSwizzle::PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_R600.0)),
        ("PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_FREEDRENO", i64::from(wcontainers_gallium::values::QuirkTextureBorderColorSwizzle::PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_FREEDRENO.0)),
        ("PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_ALPHA_NOT_W", i64::from(wcontainers_gallium::values::QuirkTextureBorderColorSwizzle::PIPE_QUIRK_TEXTURE_BORDER_COLOR_SWIZZLE_ALPHA_NOT_W.0)),
        ("PIPE_DEVICE_TYPE_UNKNOWN", i64::from(wcontainers_gallium::values::DeviceType::PIPE_DEVICE_TYPE_UNKNOWN.0)),
        ("PIPE_DEVICE_TYPE_INTEGRATED_GPU", i64::from(wcontainers_gallium::values::DeviceType::PIPE_DEVICE_TYPE_INTEGRATED_GPU.0)),
        ("PIPE_DEVICE_TYPE_DISCRETE_GPU", i64::from(wcontainers_gallium::values::DeviceType::PIPE_DEVICE_TYPE_DISCRETE_GPU.0)),
        ("PIPE_DEVICE_TYPE_CPU", i64::from(wcontainers_gallium::values::DeviceType::PIPE_DEVICE_TYPE_CPU.0)),
        ("PIPE_ENDIAN_LITTLE", i64::from(wcontainers_gallium::values::Endian::PIPE_ENDIAN_LITTLE.0)),
        ("PIPE_ENDIAN_BIG", i64::from(wcontainers_gallium::values::Endian::PIPE_ENDIAN_BIG.0)),
        ("PIPE_ENDIAN_NATIVE", i64::from(wcontainers_gallium::values::Endian::PIPE_ENDIAN_NATIVE.0)),
        ("PIPE_SHADER_IR_TGSI", i64::from(wcontainers_gallium::values::ShaderIr::PIPE_SHADER_IR_TGSI.0)),
        ("PIPE_SHADER_IR_NIR", i64::from(wcontainers_gallium::values::ShaderIr::PIPE_SHADER_IR_NIR.0)),
        ("PIPE_RESOURCE_PARAM_NPLANES", i64::from(wcontainers_gallium::values::ResourceParam::PIPE_RESOURCE_PARAM_NPLANES.0)),
        ("PIPE_RESOURCE_PARAM_STRIDE", i64::from(wcontainers_gallium::values::ResourceParam::PIPE_RESOURCE_PARAM_STRIDE.0)),
        ("PIPE_RESOURCE_PARAM_OFFSET", i64::from(wcontainers_gallium::values::ResourceParam::PIPE_RESOURCE_PARAM_OFFSET.0)),
        ("PIPE_RESOURCE_PARAM_MODIFIER", i64::from(wcontainers_gallium::values::ResourceParam::PIPE_RESOURCE_PARAM_MODIFIER.0)),
        ("PIPE_RESOURCE_PARAM_HANDLE_TYPE_SHARED", i64::from(wcontainers_gallium::values::ResourceParam::PIPE_RESOURCE_PARAM_HANDLE_TYPE_SHARED.0)),
        ("PIPE_RESOURCE_PARAM_HANDLE_TYPE_KMS", i64::from(wcontainers_gallium::values::ResourceParam::PIPE_RESOURCE_PARAM_HANDLE_TYPE_KMS.0)),
        ("PIPE_RESOURCE_PARAM_HANDLE_TYPE_FD", i64::from(wcontainers_gallium::values::ResourceParam::PIPE_RESOURCE_PARAM_HANDLE_TYPE_FD.0)),
        ("PIPE_RESOURCE_PARAM_LAYER_STRIDE", i64::from(wcontainers_gallium::values::ResourceParam::PIPE_RESOURCE_PARAM_LAYER_STRIDE.0)),
        ("PIPE_RESOURCE_PARAM_DISJOINT_PLANES", i64::from(wcontainers_gallium::values::ResourceParam::PIPE_RESOURCE_PARAM_DISJOINT_PLANES.0)),
        ("PIPE_CONTEXT_PARAM_UPDATE_THREAD_SCHEDULING", i64::from(wcontainers_gallium::values::ContextParam::PIPE_CONTEXT_PARAM_UPDATE_THREAD_SCHEDULING.0)),
        ("PIPE_CONTEXT_PARAM_MAG_SWITCHOVER_HALF", i64::from(wcontainers_gallium::values::ContextParam::PIPE_CONTEXT_PARAM_MAG_SWITCHOVER_HALF.0)),
        ("PIPE_QUERY_TYPE_I32", i64::from(wcontainers_gallium::values::QueryValueType::PIPE_QUERY_TYPE_I32.0)),
        ("PIPE_QUERY_TYPE_U32", i64::from(wcontainers_gallium::values::QueryValueType::PIPE_QUERY_TYPE_U32.0)),
        ("PIPE_QUERY_TYPE_I64", i64::from(wcontainers_gallium::values::QueryValueType::PIPE_QUERY_TYPE_I64.0)),
        ("PIPE_QUERY_TYPE_U64", i64::from(wcontainers_gallium::values::QueryValueType::PIPE_QUERY_TYPE_U64.0)),
        ("PIPE_QUERY_WAIT", i64::from(wcontainers_gallium::values::QueryFlags::PIPE_QUERY_WAIT.0)),
        ("PIPE_QUERY_PARTIAL", i64::from(wcontainers_gallium::values::QueryFlags::PIPE_QUERY_PARTIAL.0)),
        ("PIPE_DRIVER_QUERY_TYPE_UINT64", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_UINT64.0)),
        ("PIPE_DRIVER_QUERY_TYPE_UINT", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_UINT.0)),
        ("PIPE_DRIVER_QUERY_TYPE_FLOAT", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_FLOAT.0)),
        ("PIPE_DRIVER_QUERY_TYPE_PERCENTAGE", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_PERCENTAGE.0)),
        ("PIPE_DRIVER_QUERY_TYPE_BYTES", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_BYTES.0)),
        ("PIPE_DRIVER_QUERY_TYPE_MICROSECONDS", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_MICROSECONDS.0)),
        ("PIPE_DRIVER_QUERY_TYPE_HZ", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_HZ.0)),
        ("PIPE_DRIVER_QUERY_TYPE_DBM", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_DBM.0)),
        ("PIPE_DRIVER_QUERY_TYPE_TEMPERATURE", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_TEMPERATURE.0)),
        ("PIPE_DRIVER_QUERY_TYPE_VOLTS", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_VOLTS.0)),
        ("PIPE_DRIVER_QUERY_TYPE_AMPS", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_AMPS.0)),
        ("PIPE_DRIVER_QUERY_TYPE_WATTS", i64::from(wcontainers_gallium::values::DriverQueryType::PIPE_DRIVER_QUERY_TYPE_WATTS.0)),
        ("PIPE_DRIVER_QUERY_RESULT_TYPE_AVERAGE", i64::from(wcontainers_gallium::values::DriverQueryResultType::PIPE_DRIVER_QUERY_RESULT_TYPE_AVERAGE.0)),
        ("PIPE_DRIVER_QUERY_RESULT_TYPE_CUMULATIVE", i64::from(wcontainers_gallium::values::DriverQueryResultType::PIPE_DRIVER_QUERY_RESULT_TYPE_CUMULATIVE.0)),
        ("PIPE_FD_TYPE_NATIVE_SYNC", i64::from(wcontainers_gallium::values::FdType::PIPE_FD_TYPE_NATIVE_SYNC.0)),
        ("PIPE_FD_TYPE_SYNCOBJ", i64::from(wcontainers_gallium::values::FdType::PIPE_FD_TYPE_SYNCOBJ.0)),
        ("PIPE_FD_TYPE_TIMELINE_SEMAPHORE_D3D12", i64::from(wcontainers_gallium::values::FdType::PIPE_FD_TYPE_TIMELINE_SEMAPHORE_D3D12.0)),
        ("PIPE_FD_TYPE_TIMELINE_SEMAPHORE_VK", i64::from(wcontainers_gallium::values::FdType::PIPE_FD_TYPE_TIMELINE_SEMAPHORE_VK.0)),
        ("PIPE_PERF_COUNTER_TYPE_EVENT", i64::from(wcontainers_gallium::values::PerfCounterType::PIPE_PERF_COUNTER_TYPE_EVENT.0)),
        ("PIPE_PERF_COUNTER_TYPE_DURATION_NORM", i64::from(wcontainers_gallium::values::PerfCounterType::PIPE_PERF_COUNTER_TYPE_DURATION_NORM.0)),
        ("PIPE_PERF_COUNTER_TYPE_DURATION_RAW", i64::from(wcontainers_gallium::values::PerfCounterType::PIPE_PERF_COUNTER_TYPE_DURATION_RAW.0)),
        ("PIPE_PERF_COUNTER_TYPE_THROUGHPUT", i64::from(wcontainers_gallium::values::PerfCounterType::PIPE_PERF_COUNTER_TYPE_THROUGHPUT.0)),
        ("PIPE_PERF_COUNTER_TYPE_RAW", i64::from(wcontainers_gallium::values::PerfCounterType::PIPE_PERF_COUNTER_TYPE_RAW.0)),
        ("PIPE_PERF_COUNTER_TYPE_TIMESTAMP", i64::from(wcontainers_gallium::values::PerfCounterType::PIPE_PERF_COUNTER_TYPE_TIMESTAMP.0)),
        ("PIPE_PERF_COUNTER_DATA_TYPE_BOOL32", i64::from(wcontainers_gallium::values::PerfCounterDataType::PIPE_PERF_COUNTER_DATA_TYPE_BOOL32.0)),
        ("PIPE_PERF_COUNTER_DATA_TYPE_UINT32", i64::from(wcontainers_gallium::values::PerfCounterDataType::PIPE_PERF_COUNTER_DATA_TYPE_UINT32.0)),
        ("PIPE_PERF_COUNTER_DATA_TYPE_UINT64", i64::from(wcontainers_gallium::values::PerfCounterDataType::PIPE_PERF_COUNTER_DATA_TYPE_UINT64.0)),
        ("PIPE_PERF_COUNTER_DATA_TYPE_FLOAT", i64::from(wcontainers_gallium::values::PerfCounterDataType::PIPE_PERF_COUNTER_DATA_TYPE_FLOAT.0)),
        ("PIPE_PERF_COUNTER_DATA_TYPE_DOUBLE", i64::from(wcontainers_gallium::values::PerfCounterDataType::PIPE_PERF_COUNTER_DATA_TYPE_DOUBLE.0)),
        ("PIPE_ML_OPERATION_TYPE_ADD", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_ADD.0)),
        ("PIPE_ML_OPERATION_TYPE_CONVOLUTION", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_CONVOLUTION.0)),
        ("PIPE_ML_OPERATION_TYPE_POOLING", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_POOLING.0)),
        ("PIPE_ML_OPERATION_TYPE_CONCATENATION", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_CONCATENATION.0)),
        ("PIPE_ML_OPERATION_TYPE_PACK", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_PACK.0)),
        ("PIPE_ML_OPERATION_TYPE_SPLIT", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_SPLIT.0)),
        ("PIPE_ML_OPERATION_TYPE_UNPACK", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_UNPACK.0)),
        ("PIPE_ML_OPERATION_TYPE_PAD", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_PAD.0)),
        ("PIPE_ML_OPERATION_TYPE_SCATTER_ND", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_SCATTER_ND.0)),
        ("PIPE_ML_OPERATION_TYPE_FULLY_CONNECTED", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_FULLY_CONNECTED.0)),
        ("PIPE_ML_OPERATION_TYPE_BATCH_MATMUL", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_BATCH_MATMUL.0)),
        ("PIPE_ML_OPERATION_TYPE_RESHAPE", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_RESHAPE.0)),
        ("PIPE_ML_OPERATION_TYPE_RELU", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_RELU.0)),
        ("PIPE_ML_OPERATION_TYPE_ABSOLUTE", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_ABSOLUTE.0)),
        ("PIPE_ML_OPERATION_TYPE_LOGISTIC", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_LOGISTIC.0)),
        ("PIPE_ML_OPERATION_TYPE_TANH", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_TANH.0)),
        ("PIPE_ML_OPERATION_TYPE_RSQRT", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_RSQRT.0)),
        ("PIPE_ML_OPERATION_TYPE_HSWISH", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_HSWISH.0)),
        ("PIPE_ML_OPERATION_TYPE_SUBTRACT", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_SUBTRACT.0)),
        ("PIPE_ML_OPERATION_TYPE_TRANSPOSE", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_TRANSPOSE.0)),
        ("PIPE_ML_OPERATION_TYPE_STRIDED_SLICE", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_STRIDED_SLICE.0)),
        ("PIPE_ML_OPERATION_TYPE_RESIZE", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_RESIZE.0)),
        ("PIPE_ML_OPERATION_TYPE_RESIZE_BILINEAR", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_RESIZE_BILINEAR.0)),
        ("PIPE_ML_OPERATION_TYPE_ARGMAX", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_ARGMAX.0)),
        ("PIPE_ML_OPERATION_TYPE_SPACE_TO_BATCH", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_SPACE_TO_BATCH.0)),
        ("PIPE_ML_OPERATION_TYPE_BATCH_TO_SPACE", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_BATCH_TO_SPACE.0)),
        ("PIPE_ML_OPERATION_TYPE_MUL", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_MUL.0)),
        ("PIPE_ML_OPERATION_TYPE_LEAKY_RELU", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_LEAKY_RELU.0)),
        ("PIPE_ML_OPERATION_TYPE_QUANTIZE", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_QUANTIZE.0)),
        ("PIPE_ML_OPERATION_TYPE_MAXIMUM", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_MAXIMUM.0)),
        ("PIPE_ML_OPERATION_TYPE_MINIMUM", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_MINIMUM.0)),
        ("PIPE_ML_OPERATION_TYPE_SOFTMAX", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_SOFTMAX.0)),
        ("PIPE_ML_OPERATION_TYPE_MEAN", i64::from(wcontainers_gallium::values::MlOperationType::PIPE_ML_OPERATION_TYPE_MEAN.0)),
        ("PIPE_ML_POOLING_TYPE_AVG", i64::from(wcontainers_gallium::values::MlPoolingType::PIPE_ML_POOLING_TYPE_AVG.0)),
        ("PIPE_ML_POOLING_TYPE_MAX", i64::from(wcontainers_gallium::values::MlPoolingType::PIPE_ML_POOLING_TYPE_MAX.0)),
        ("PIPE_POLYGON_MODE_FILL", i64::from(wcontainers_gallium::values::PIPE_POLYGON_MODE_FILL)),
        ("PIPE_POLYGON_MODE_LINE", i64::from(wcontainers_gallium::values::PIPE_POLYGON_MODE_LINE)),
        ("PIPE_POLYGON_MODE_POINT", i64::from(wcontainers_gallium::values::PIPE_POLYGON_MODE_POINT)),
        ("PIPE_POLYGON_MODE_FILL_RECTANGLE", i64::from(wcontainers_gallium::values::PIPE_POLYGON_MODE_FILL_RECTANGLE)),
        ("PIPE_ASTC_DECODE_FORMAT_FLOAT16", i64::from(wcontainers_gallium::values::PIPE_ASTC_DECODE_FORMAT_FLOAT16)),
        ("PIPE_ASTC_DECODE_FORMAT_RGB9E5", i64::from(wcontainers_gallium::values::PIPE_ASTC_DECODE_FORMAT_RGB9E5)),
        ("PIPE_ASTC_DECODE_FORMAT_UNORM8", i64::from(wcontainers_gallium::values::PIPE_ASTC_DECODE_FORMAT_UNORM8)),
        ("PIPE_BARRIER_ALL", i64::from(wcontainers_gallium::values::PIPE_BARRIER_ALL)),
        ("PIPE_BARRIER_CONSTANT_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_CONSTANT_BUFFER)),
        ("PIPE_BARRIER_FRAMEBUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_FRAMEBUFFER)),
        ("PIPE_BARRIER_IMAGE", i64::from(wcontainers_gallium::values::PIPE_BARRIER_IMAGE)),
        ("PIPE_BARRIER_INDEX_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_INDEX_BUFFER)),
        ("PIPE_BARRIER_INDIRECT_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_INDIRECT_BUFFER)),
        ("PIPE_BARRIER_MAPPED_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_MAPPED_BUFFER)),
        ("PIPE_BARRIER_QUERY_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_QUERY_BUFFER)),
        ("PIPE_BARRIER_SHADER_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_SHADER_BUFFER)),
        ("PIPE_BARRIER_STREAMOUT_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_STREAMOUT_BUFFER)),
        ("PIPE_BARRIER_TEXTURE", i64::from(wcontainers_gallium::values::PIPE_BARRIER_TEXTURE)),
        ("PIPE_BARRIER_UPDATE", i64::from(wcontainers_gallium::values::PIPE_BARRIER_UPDATE)),
        ("PIPE_BARRIER_UPDATE_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_UPDATE_BUFFER)),
        ("PIPE_BARRIER_UPDATE_TEXTURE", i64::from(wcontainers_gallium::values::PIPE_BARRIER_UPDATE_TEXTURE)),
        ("PIPE_BARRIER_VERTEX_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BARRIER_VERTEX_BUFFER)),
        ("PIPE_BIND_BLENDABLE", i64::from(wcontainers_gallium::values::PIPE_BIND_BLENDABLE)),
        ("PIPE_BIND_COMMAND_ARGS_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BIND_COMMAND_ARGS_BUFFER)),
        ("PIPE_BIND_CONSTANT_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BIND_CONSTANT_BUFFER)),
        ("PIPE_BIND_CONST_BW", i64::from(wcontainers_gallium::values::PIPE_BIND_CONST_BW)),
        ("PIPE_BIND_CURSOR", i64::from(wcontainers_gallium::values::PIPE_BIND_CURSOR)),
        ("PIPE_BIND_CUSTOM", i64::from(wcontainers_gallium::values::PIPE_BIND_CUSTOM)),
        ("PIPE_BIND_DEPTH_STENCIL", i64::from(wcontainers_gallium::values::PIPE_BIND_DEPTH_STENCIL)),
        ("PIPE_BIND_DISPLAY_TARGET", i64::from(wcontainers_gallium::values::PIPE_BIND_DISPLAY_TARGET)),
        ("PIPE_BIND_GLOBAL", i64::from(wcontainers_gallium::values::PIPE_BIND_GLOBAL)),
        ("PIPE_BIND_INDEX_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BIND_INDEX_BUFFER)),
        ("PIPE_BIND_LINEAR", i64::from(wcontainers_gallium::values::PIPE_BIND_LINEAR)),
        ("PIPE_BIND_OPENCL", i64::from(wcontainers_gallium::values::PIPE_BIND_OPENCL)),
        ("PIPE_BIND_PRIME_BLIT_DST", i64::from(wcontainers_gallium::values::PIPE_BIND_PRIME_BLIT_DST)),
        ("PIPE_BIND_PROTECTED", i64::from(wcontainers_gallium::values::PIPE_BIND_PROTECTED)),
        ("PIPE_BIND_QUERY_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BIND_QUERY_BUFFER)),
        ("PIPE_BIND_RENDER_TARGET", i64::from(wcontainers_gallium::values::PIPE_BIND_RENDER_TARGET)),
        ("PIPE_BIND_SAMPLER_REDUCTION_MINMAX", i64::from(wcontainers_gallium::values::PIPE_BIND_SAMPLER_REDUCTION_MINMAX)),
        ("PIPE_BIND_SAMPLER_VIEW", i64::from(wcontainers_gallium::values::PIPE_BIND_SAMPLER_VIEW)),
        ("PIPE_BIND_SAMPLER_VIEW_SUBOPTIMAL", i64::from(wcontainers_gallium::values::PIPE_BIND_SAMPLER_VIEW_SUBOPTIMAL)),
        ("PIPE_BIND_SCANOUT", i64::from(wcontainers_gallium::values::PIPE_BIND_SCANOUT)),
        ("PIPE_BIND_SHADER_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BIND_SHADER_BUFFER)),
        ("PIPE_BIND_SHADER_IMAGE", i64::from(wcontainers_gallium::values::PIPE_BIND_SHADER_IMAGE)),
        ("PIPE_BIND_SHARED", i64::from(wcontainers_gallium::values::PIPE_BIND_SHARED)),
        ("PIPE_BIND_STREAM_OUTPUT", i64::from(wcontainers_gallium::values::PIPE_BIND_STREAM_OUTPUT)),
        ("PIPE_BIND_USE_FRONT_RENDERING", i64::from(wcontainers_gallium::values::PIPE_BIND_USE_FRONT_RENDERING)),
        ("PIPE_BIND_VERTEX_BUFFER", i64::from(wcontainers_gallium::values::PIPE_BIND_VERTEX_BUFFER)),
        ("PIPE_BIND_VERTEX_STATE", i64::from(wcontainers_gallium::values::PIPE_BIND_VERTEX_STATE)),
        ("PIPE_BIND_VIDEO_DECODE_DPB", i64::from(wcontainers_gallium::values::PIPE_BIND_VIDEO_DECODE_DPB)),
        ("PIPE_BIND_VIDEO_ENCODE_DPB", i64::from(wcontainers_gallium::values::PIPE_BIND_VIDEO_ENCODE_DPB)),
        ("PIPE_BLENDFACTOR_INVERT_BIT", i64::from(wcontainers_gallium::values::PIPE_BLENDFACTOR_INVERT_BIT)),
        ("PIPE_CLEAR_COLOR", i64::from(wcontainers_gallium::values::PIPE_CLEAR_COLOR)),
        ("PIPE_CLEAR_COLOR0", i64::from(wcontainers_gallium::values::PIPE_CLEAR_COLOR0)),
        ("PIPE_CLEAR_COLOR1", i64::from(wcontainers_gallium::values::PIPE_CLEAR_COLOR1)),
        ("PIPE_CLEAR_COLOR2", i64::from(wcontainers_gallium::values::PIPE_CLEAR_COLOR2)),
        ("PIPE_CLEAR_COLOR3", i64::from(wcontainers_gallium::values::PIPE_CLEAR_COLOR3)),
        ("PIPE_CLEAR_COLOR4", i64::from(wcontainers_gallium::values::PIPE_CLEAR_COLOR4)),
        ("PIPE_CLEAR_COLOR5", i64::from(wcontainers_gallium::values::PIPE_CLEAR_COLOR5)),
        ("PIPE_CLEAR_COLOR6", i64::from(wcontainers_gallium::values::PIPE_CLEAR_COLOR6)),
        ("PIPE_CLEAR_COLOR7", i64::from(wcontainers_gallium::values::PIPE_CLEAR_COLOR7)),
        ("PIPE_CLEAR_DEPTH", i64::from(wcontainers_gallium::values::PIPE_CLEAR_DEPTH)),
        ("PIPE_CLEAR_DEPTHSTENCIL", i64::from(wcontainers_gallium::values::PIPE_CLEAR_DEPTHSTENCIL)),
        ("PIPE_CLEAR_STENCIL", i64::from(wcontainers_gallium::values::PIPE_CLEAR_STENCIL)),
        ("PIPE_COMPRESSION_FIXED_RATE_DEFAULT", i64::from(wcontainers_gallium::values::PIPE_COMPRESSION_FIXED_RATE_DEFAULT)),
        ("PIPE_COMPRESSION_FIXED_RATE_NONE", i64::from(wcontainers_gallium::values::PIPE_COMPRESSION_FIXED_RATE_NONE)),
        ("PIPE_CONTEXT_COMPUTE_ONLY", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_COMPUTE_ONLY)),
        ("PIPE_CONTEXT_DEBUG", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_DEBUG)),
        ("PIPE_CONTEXT_HIGH_PRIORITY", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_HIGH_PRIORITY)),
        ("PIPE_CONTEXT_LOSE_CONTEXT_ON_RESET", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_LOSE_CONTEXT_ON_RESET)),
        ("PIPE_CONTEXT_LOW_PRIORITY", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_LOW_PRIORITY)),
        ("PIPE_CONTEXT_MEDIA_ONLY", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_MEDIA_ONLY)),
        ("PIPE_CONTEXT_NO_LOD_BIAS", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_NO_LOD_BIAS)),
        ("PIPE_CONTEXT_PREFER_THREADED", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_PREFER_THREADED)),
        ("PIPE_CONTEXT_PRIORITY_HIGH", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_PRIORITY_HIGH)),
        ("PIPE_CONTEXT_PRIORITY_LOW", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_PRIORITY_LOW)),
        ("PIPE_CONTEXT_PRIORITY_MEDIUM", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_PRIORITY_MEDIUM)),
        ("PIPE_CONTEXT_PRIORITY_REALTIME", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_PRIORITY_REALTIME)),
        ("PIPE_CONTEXT_PROTECTED", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_PROTECTED)),
        ("PIPE_CONTEXT_REALTIME_PRIORITY", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_REALTIME_PRIORITY)),
        ("PIPE_CONTEXT_ROBUST_BUFFER_ACCESS", i64::from(wcontainers_gallium::values::PIPE_CONTEXT_ROBUST_BUFFER_ACCESS)),
        ("PIPE_DRIVER_QUERY_FLAG_BATCH", i64::from(wcontainers_gallium::values::PIPE_DRIVER_QUERY_FLAG_BATCH)),
        ("PIPE_DRIVER_QUERY_FLAG_DONT_LIST", i64::from(wcontainers_gallium::values::PIPE_DRIVER_QUERY_FLAG_DONT_LIST)),
        ("PIPE_DUMP_DEVICE_STATUS_REGISTERS", i64::from(wcontainers_gallium::values::PIPE_DUMP_DEVICE_STATUS_REGISTERS)),
        ("PIPE_FACE_BACK", i64::from(wcontainers_gallium::values::PIPE_FACE_BACK)),
        ("PIPE_FACE_FRONT", i64::from(wcontainers_gallium::values::PIPE_FACE_FRONT)),
        ("PIPE_FACE_FRONT_AND_BACK", i64::from(wcontainers_gallium::values::PIPE_FACE_FRONT_AND_BACK)),
        ("PIPE_FACE_NONE", i64::from(wcontainers_gallium::values::PIPE_FACE_NONE)),
        ("PIPE_FORMAT_ABGR8888_SNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_ABGR8888_SNORM)),
        ("PIPE_FORMAT_ABGR8888_SRGB", i64::from(wcontainers_gallium::values::PIPE_FORMAT_ABGR8888_SRGB)),
        ("PIPE_FORMAT_ABGR8888_UINT", i64::from(wcontainers_gallium::values::PIPE_FORMAT_ABGR8888_UINT)),
        ("PIPE_FORMAT_ABGR8888_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_ABGR8888_UNORM)),
        ("PIPE_FORMAT_ARGB8888_SRGB", i64::from(wcontainers_gallium::values::PIPE_FORMAT_ARGB8888_SRGB)),
        ("PIPE_FORMAT_ARGB8888_UINT", i64::from(wcontainers_gallium::values::PIPE_FORMAT_ARGB8888_UINT)),
        ("PIPE_FORMAT_ARGB8888_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_ARGB8888_UNORM)),
        ("PIPE_FORMAT_AYUV", i64::from(wcontainers_gallium::values::PIPE_FORMAT_AYUV)),
        ("PIPE_FORMAT_B8G8_R8G8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_B8G8_R8G8_UNORM)),
        ("PIPE_FORMAT_B8R8_G8R8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_B8R8_G8R8_UNORM)),
        ("PIPE_FORMAT_BGRA8888_SRGB", i64::from(wcontainers_gallium::values::PIPE_FORMAT_BGRA8888_SRGB)),
        ("PIPE_FORMAT_BGRA8888_UINT", i64::from(wcontainers_gallium::values::PIPE_FORMAT_BGRA8888_UINT)),
        ("PIPE_FORMAT_BGRA8888_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_BGRA8888_UNORM)),
        ("PIPE_FORMAT_BGRX8888_SRGB", i64::from(wcontainers_gallium::values::PIPE_FORMAT_BGRX8888_SRGB)),
        ("PIPE_FORMAT_BGRX8888_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_BGRX8888_UNORM)),
        ("PIPE_FORMAT_G8B8_G8R8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_G8B8_G8R8_UNORM)),
        ("PIPE_FORMAT_G8R8_B8R8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_G8R8_B8R8_UNORM)),
        ("PIPE_FORMAT_G8R8_G8B8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_G8R8_G8B8_UNORM)),
        ("PIPE_FORMAT_GR1616_SNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_GR1616_SNORM)),
        ("PIPE_FORMAT_GR1616_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_GR1616_UNORM)),
        ("PIPE_FORMAT_GR88_SNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_GR88_SNORM)),
        ("PIPE_FORMAT_GR88_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_GR88_UNORM)),
        ("PIPE_FORMAT_IYUV", i64::from(wcontainers_gallium::values::PIPE_FORMAT_IYUV)),
        ("PIPE_FORMAT_NV12", i64::from(wcontainers_gallium::values::PIPE_FORMAT_NV12)),
        ("PIPE_FORMAT_NV15", i64::from(wcontainers_gallium::values::PIPE_FORMAT_NV15)),
        ("PIPE_FORMAT_NV16", i64::from(wcontainers_gallium::values::PIPE_FORMAT_NV16)),
        ("PIPE_FORMAT_NV20", i64::from(wcontainers_gallium::values::PIPE_FORMAT_NV20)),
        ("PIPE_FORMAT_NV21", i64::from(wcontainers_gallium::values::PIPE_FORMAT_NV21)),
        ("PIPE_FORMAT_NV24", i64::from(wcontainers_gallium::values::PIPE_FORMAT_NV24)),
        ("PIPE_FORMAT_NV42", i64::from(wcontainers_gallium::values::PIPE_FORMAT_NV42)),
        ("PIPE_FORMAT_NV61", i64::from(wcontainers_gallium::values::PIPE_FORMAT_NV61)),
        ("PIPE_FORMAT_P010", i64::from(wcontainers_gallium::values::PIPE_FORMAT_P010)),
        ("PIPE_FORMAT_P012", i64::from(wcontainers_gallium::values::PIPE_FORMAT_P012)),
        ("PIPE_FORMAT_P016", i64::from(wcontainers_gallium::values::PIPE_FORMAT_P016)),
        ("PIPE_FORMAT_P030", i64::from(wcontainers_gallium::values::PIPE_FORMAT_P030)),
        ("PIPE_FORMAT_P210", i64::from(wcontainers_gallium::values::PIPE_FORMAT_P210)),
        ("PIPE_FORMAT_R8B8_R8G8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_R8B8_R8G8_UNORM)),
        ("PIPE_FORMAT_R8G8_B8G8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_R8G8_B8G8_UNORM)),
        ("PIPE_FORMAT_R8G8_R8B8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_R8G8_R8B8_UNORM)),
        ("PIPE_FORMAT_R8_G8_B8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_R8_G8_B8_UNORM)),
        ("PIPE_FORMAT_RG1616_SNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RG1616_SNORM)),
        ("PIPE_FORMAT_RG1616_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RG1616_UNORM)),
        ("PIPE_FORMAT_RG88_SNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RG88_SNORM)),
        ("PIPE_FORMAT_RG88_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RG88_UNORM)),
        ("PIPE_FORMAT_RGBA8888_SINT", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBA8888_SINT)),
        ("PIPE_FORMAT_RGBA8888_SNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBA8888_SNORM)),
        ("PIPE_FORMAT_RGBA8888_SRGB", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBA8888_SRGB)),
        ("PIPE_FORMAT_RGBA8888_SSCALED", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBA8888_SSCALED)),
        ("PIPE_FORMAT_RGBA8888_UINT", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBA8888_UINT)),
        ("PIPE_FORMAT_RGBA8888_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBA8888_UNORM)),
        ("PIPE_FORMAT_RGBA8888_USCALED", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBA8888_USCALED)),
        ("PIPE_FORMAT_RGBX8888_SNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBX8888_SNORM)),
        ("PIPE_FORMAT_RGBX8888_SRGB", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBX8888_SRGB)),
        ("PIPE_FORMAT_RGBX8888_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_RGBX8888_UNORM)),
        ("PIPE_FORMAT_UYVY", i64::from(wcontainers_gallium::values::PIPE_FORMAT_UYVY)),
        ("PIPE_FORMAT_VYUY", i64::from(wcontainers_gallium::values::PIPE_FORMAT_VYUY)),
        ("PIPE_FORMAT_XBGR8888_SNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_XBGR8888_SNORM)),
        ("PIPE_FORMAT_XBGR8888_SRGB", i64::from(wcontainers_gallium::values::PIPE_FORMAT_XBGR8888_SRGB)),
        ("PIPE_FORMAT_XBGR8888_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_XBGR8888_UNORM)),
        ("PIPE_FORMAT_XRGB8888_SRGB", i64::from(wcontainers_gallium::values::PIPE_FORMAT_XRGB8888_SRGB)),
        ("PIPE_FORMAT_XRGB8888_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_XRGB8888_UNORM)),
        ("PIPE_FORMAT_XYUV", i64::from(wcontainers_gallium::values::PIPE_FORMAT_XYUV)),
        ("PIPE_FORMAT_Y210", i64::from(wcontainers_gallium::values::PIPE_FORMAT_Y210)),
        ("PIPE_FORMAT_Y212", i64::from(wcontainers_gallium::values::PIPE_FORMAT_Y212)),
        ("PIPE_FORMAT_Y216", i64::from(wcontainers_gallium::values::PIPE_FORMAT_Y216)),
        ("PIPE_FORMAT_Y410", i64::from(wcontainers_gallium::values::PIPE_FORMAT_Y410)),
        ("PIPE_FORMAT_Y412", i64::from(wcontainers_gallium::values::PIPE_FORMAT_Y412)),
        ("PIPE_FORMAT_Y416", i64::from(wcontainers_gallium::values::PIPE_FORMAT_Y416)),
        ("PIPE_FORMAT_Y8_UNORM", i64::from(wcontainers_gallium::values::PIPE_FORMAT_Y8_UNORM)),
        ("PIPE_FORMAT_YUYV", i64::from(wcontainers_gallium::values::PIPE_FORMAT_YUYV)),
        ("PIPE_FORMAT_YV12", i64::from(wcontainers_gallium::values::PIPE_FORMAT_YV12)),
        ("PIPE_FORMAT_YV16", i64::from(wcontainers_gallium::values::PIPE_FORMAT_YV16)),
        ("PIPE_FORMAT_YVYU", i64::from(wcontainers_gallium::values::PIPE_FORMAT_YVYU)),
        ("PIPE_HANDLE_USAGE_EXPLICIT_FLUSH", i64::from(wcontainers_gallium::values::PIPE_HANDLE_USAGE_EXPLICIT_FLUSH)),
        ("PIPE_HANDLE_USAGE_FRAMEBUFFER_WRITE", i64::from(wcontainers_gallium::values::PIPE_HANDLE_USAGE_FRAMEBUFFER_WRITE)),
        ("PIPE_HANDLE_USAGE_SHADER_WRITE", i64::from(wcontainers_gallium::values::PIPE_HANDLE_USAGE_SHADER_WRITE)),
        ("PIPE_IMAGE_ACCESS_COHERENT", i64::from(wcontainers_gallium::values::PIPE_IMAGE_ACCESS_COHERENT)),
        ("PIPE_IMAGE_ACCESS_DRIVER_INTERNAL", i64::from(wcontainers_gallium::values::PIPE_IMAGE_ACCESS_DRIVER_INTERNAL)),
        ("PIPE_IMAGE_ACCESS_READ", i64::from(wcontainers_gallium::values::PIPE_IMAGE_ACCESS_READ)),
        ("PIPE_IMAGE_ACCESS_READ_WRITE", i64::from(wcontainers_gallium::values::PIPE_IMAGE_ACCESS_READ_WRITE)),
        ("PIPE_IMAGE_ACCESS_TEX2D_FROM_BUFFER", i64::from(wcontainers_gallium::values::PIPE_IMAGE_ACCESS_TEX2D_FROM_BUFFER)),
        ("PIPE_IMAGE_ACCESS_VOLATILE", i64::from(wcontainers_gallium::values::PIPE_IMAGE_ACCESS_VOLATILE)),
        ("PIPE_IMAGE_ACCESS_WRITE", i64::from(wcontainers_gallium::values::PIPE_IMAGE_ACCESS_WRITE)),
        ("PIPE_LUID_SIZE", i64::from(wcontainers_gallium::values::PIPE_LUID_SIZE)),
        ("PIPE_MASK_A", i64::from(wcontainers_gallium::values::PIPE_MASK_A)),
        ("PIPE_MASK_B", i64::from(wcontainers_gallium::values::PIPE_MASK_B)),
        ("PIPE_MASK_G", i64::from(wcontainers_gallium::values::PIPE_MASK_G)),
        ("PIPE_MASK_R", i64::from(wcontainers_gallium::values::PIPE_MASK_R)),
        ("PIPE_MASK_RGB", i64::from(wcontainers_gallium::values::PIPE_MASK_RGB)),
        ("PIPE_MASK_RGBA", i64::from(wcontainers_gallium::values::PIPE_MASK_RGBA)),
        ("PIPE_MASK_RGBAZS", i64::from(wcontainers_gallium::values::PIPE_MASK_RGBAZS)),
        ("PIPE_MASK_S", i64::from(wcontainers_gallium::values::PIPE_MASK_S)),
        ("PIPE_MASK_Z", i64::from(wcontainers_gallium::values::PIPE_MASK_Z)),
        ("PIPE_MASK_ZS", i64::from(wcontainers_gallium::values::PIPE_MASK_ZS)),
        ("PIPE_MAX_ATTRIBS", i64::from(wcontainers_gallium::values::PIPE_MAX_ATTRIBS)),
        ("PIPE_MAX_CLIP_OR_CULL_DISTANCE_COUNT", i64::from(wcontainers_gallium::values::PIPE_MAX_CLIP_OR_CULL_DISTANCE_COUNT)),
        ("PIPE_MAX_CLIP_OR_CULL_DISTANCE_ELEMENT_COUNT", i64::from(wcontainers_gallium::values::PIPE_MAX_CLIP_OR_CULL_DISTANCE_ELEMENT_COUNT)),
        ("PIPE_MAX_CLIP_PLANES", i64::from(wcontainers_gallium::values::PIPE_MAX_CLIP_PLANES)),
        ("PIPE_MAX_COLOR_BUFS", i64::from(wcontainers_gallium::values::PIPE_MAX_COLOR_BUFS)),
        ("PIPE_MAX_CONSTANT_BUFFERS", i64::from(wcontainers_gallium::values::PIPE_MAX_CONSTANT_BUFFERS)),
        ("PIPE_MAX_HW_ATOMIC_BUFFERS", i64::from(wcontainers_gallium::values::PIPE_MAX_HW_ATOMIC_BUFFERS)),
        ("PIPE_MAX_SAMPLERS", i64::from(wcontainers_gallium::values::PIPE_MAX_SAMPLERS)),
        ("PIPE_MAX_SAMPLE_LOCATION_GRID_SIZE", i64::from(wcontainers_gallium::values::PIPE_MAX_SAMPLE_LOCATION_GRID_SIZE)),
        ("PIPE_MAX_SHADER_BUFFERS", i64::from(wcontainers_gallium::values::PIPE_MAX_SHADER_BUFFERS)),
        ("PIPE_MAX_SHADER_IMAGES", i64::from(wcontainers_gallium::values::PIPE_MAX_SHADER_IMAGES)),
        ("PIPE_MAX_SHADER_INPUTS", i64::from(wcontainers_gallium::values::PIPE_MAX_SHADER_INPUTS)),
        ("PIPE_MAX_SHADER_OUTPUTS", i64::from(wcontainers_gallium::values::PIPE_MAX_SHADER_OUTPUTS)),
        ("PIPE_MAX_SHADER_SAMPLER_VIEWS", i64::from(wcontainers_gallium::values::PIPE_MAX_SHADER_SAMPLER_VIEWS)),
        ("PIPE_MAX_SO_BUFFERS", i64::from(wcontainers_gallium::values::PIPE_MAX_SO_BUFFERS)),
        ("PIPE_MAX_SO_OUTPUTS", i64::from(wcontainers_gallium::values::PIPE_MAX_SO_OUTPUTS)),
        ("PIPE_MAX_TEXTURE_LEVELS", i64::from(wcontainers_gallium::values::PIPE_MAX_TEXTURE_LEVELS)),
        ("PIPE_MAX_VERTEX_STREAMS", i64::from(wcontainers_gallium::values::PIPE_MAX_VERTEX_STREAMS)),
        ("PIPE_MAX_VIEWPORTS", i64::from(wcontainers_gallium::values::PIPE_MAX_VIEWPORTS)),
        ("PIPE_MAX_WINDOW_RECTANGLES", i64::from(wcontainers_gallium::values::PIPE_MAX_WINDOW_RECTANGLES)),
        ("PIPE_RESOURCE_FLAG_DONT_MAP_DIRECTLY", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_DONT_MAP_DIRECTLY)),
        ("PIPE_RESOURCE_FLAG_DONT_OVER_ALLOCATE", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_DONT_OVER_ALLOCATE)),
        ("PIPE_RESOURCE_FLAG_DRV_PRIV", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_DRV_PRIV)),
        ("PIPE_RESOURCE_FLAG_ENCRYPTED", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_ENCRYPTED)),
        ("PIPE_RESOURCE_FLAG_FIXED_ADDRESS", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_FIXED_ADDRESS)),
        ("PIPE_RESOURCE_FLAG_FRONTEND_PRIV", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_FRONTEND_PRIV)),
        ("PIPE_RESOURCE_FLAG_FRONTEND_VM", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_FRONTEND_VM)),
        ("PIPE_RESOURCE_FLAG_MAP_CLIENT_POINTER", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_MAP_CLIENT_POINTER)),
        ("PIPE_RESOURCE_FLAG_MAP_COHERENT", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_MAP_COHERENT)),
        ("PIPE_RESOURCE_FLAG_MAP_PERSISTENT", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_MAP_PERSISTENT)),
        ("PIPE_RESOURCE_FLAG_MAP_UNSYNCHRONIZED", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_MAP_UNSYNCHRONIZED)),
        ("PIPE_RESOURCE_FLAG_SINGLE_THREAD_USE", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_SINGLE_THREAD_USE)),
        ("PIPE_RESOURCE_FLAG_SPARSE", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_SPARSE)),
        ("PIPE_RESOURCE_FLAG_TEXTURING_MORE_LIKELY", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_TEXTURING_MORE_LIKELY)),
        ("PIPE_RESOURCE_FLAG_UNMAPPABLE", i64::from(wcontainers_gallium::values::PIPE_RESOURCE_FLAG_UNMAPPABLE)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_ARITHMETIC", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_ARITHMETIC)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_BALLOT", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_BALLOT)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_BASIC", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_BASIC)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_CLUSTERED", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_CLUSTERED)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_QUAD", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_QUAD)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_ROTATE", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_ROTATE)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_ROTATE_CLUSTERED", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_ROTATE_CLUSTERED)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_SHUFFLE", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_SHUFFLE)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_SHUFFLE_RELATIVE", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_SHUFFLE_RELATIVE)),
        ("PIPE_SHADER_SUBGROUP_FEATURE_VOTE", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_FEATURE_VOTE)),
        ("PIPE_SHADER_SUBGROUP_NUM_FEATURES", i64::from(wcontainers_gallium::values::PIPE_SHADER_SUBGROUP_NUM_FEATURES)),
        ("PIPE_TEXTURE_BARRIER_FRAMEBUFFER", i64::from(wcontainers_gallium::values::PIPE_TEXTURE_BARRIER_FRAMEBUFFER)),
        ("PIPE_TEXTURE_BARRIER_SAMPLER", i64::from(wcontainers_gallium::values::PIPE_TEXTURE_BARRIER_SAMPLER)),
        ("PIPE_UUID_SIZE", i64::from(wcontainers_gallium::values::PIPE_UUID_SIZE)),
    ]
}
