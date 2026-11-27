/* Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit. */
/* Mesa's structures as the glue copies them: no bit-field, no union, every object its address. */
#ifndef WC_GALLIUM_FLAT_H
#define WC_GALLIUM_FLAT_H

#include <stddef.h>
#include <stdint.h>

/* Mesa's `pipe_blend_color`, flat. */
struct wc_flat_pipe_blend_color {
    float color[4];
};

/* Mesa's `pipe_rt_blend_state`, flat. */
struct wc_flat_pipe_rt_blend_state {
    uint32_t blend_enable;
    uint32_t rgb_func;
    uint32_t rgb_src_factor;
    uint32_t rgb_dst_factor;
    uint32_t alpha_func;
    uint32_t alpha_src_factor;
    uint32_t alpha_dst_factor;
    uint32_t colormask;
};

/* Mesa's `pipe_blend_state`, flat. */
struct wc_flat_pipe_blend_state {
    uint32_t independent_blend_enable;
    uint32_t logicop_enable;
    uint32_t logicop_func;
    uint32_t dither;
    uint32_t alpha_to_coverage;
    uint32_t alpha_to_coverage_dither;
    uint32_t alpha_to_one;
    uint32_t max_rt;
    uint32_t advanced_blend_func;
    uint32_t blend_coherent;
    uint32_t is_yuv;
    struct wc_flat_pipe_rt_blend_state rt[8];
};

/* Mesa's `pipe_box`, flat. */
struct wc_flat_pipe_box {
    int32_t x;
    int32_t width;
    int32_t y;
    int32_t height;
    int16_t z;
    int16_t depth;
};

/* Mesa's `pipe_blit_info.dst`, flat. */
struct wc_flat_pipe_blit_info__dst {
    uintptr_t resource;
    uint32_t level;
    struct wc_flat_pipe_box box;
    uint32_t format;
};

/* Mesa's `pipe_blit_info.src`, flat. */
struct wc_flat_pipe_blit_info__src {
    uintptr_t resource;
    uint32_t level;
    struct wc_flat_pipe_box box;
    uint32_t format;
};

/* Mesa's `pipe_scissor_state`, flat. */
struct wc_flat_pipe_scissor_state {
    uint32_t minx;
    uint32_t miny;
    uint32_t maxx;
    uint32_t maxy;
};

/* Mesa's `pipe_blit_info`, flat. */
struct wc_flat_pipe_blit_info {
    struct wc_flat_pipe_blit_info__dst dst;
    struct wc_flat_pipe_blit_info__src src;
    uint32_t mask;
    uint32_t filter;
    uint8_t dst_sample;
    uint8_t sample0_only;
    uint8_t scissor_enable;
    struct wc_flat_pipe_scissor_state scissor;
    uint8_t swizzle_enable;
    uint8_t swizzle[4];
    uint8_t window_rectangle_include;
    uint32_t num_window_rectangles;
    struct wc_flat_pipe_scissor_state window_rectangles[8];
    uint8_t render_condition_enable;
    uint8_t alpha_blend;
};

/* Mesa's `pipe_mesh_caps`, flat. */
struct wc_flat_pipe_mesh_caps {
    uint32_t max_task_work_group_total_count;
    uint32_t max_mesh_work_group_total_count;
    uint32_t max_mesh_work_group_invocations;
    uint32_t max_task_work_group_invocations;
    uint32_t max_task_payload_size;
    uint32_t max_task_shared_memory_size;
    uint32_t max_mesh_shared_memory_size;
    uint32_t max_task_payload_and_shared_memory_size;
    uint32_t max_mesh_payload_and_shared_memory_size;
    uint32_t max_mesh_output_memory_size;
    uint32_t max_mesh_payload_and_output_memory_size;
    uint32_t max_mesh_output_vertices;
    uint32_t max_mesh_output_primitives;
    uint32_t max_mesh_output_components;
    uint32_t max_mesh_output_layers;
    uint32_t max_mesh_multiview_view_count;
    uint32_t mesh_output_per_vertex_granularity;
    uint32_t mesh_output_per_primitive_granularity;
    uint32_t max_preferred_task_work_group_invocations;
    uint32_t max_preferred_mesh_work_group_invocations;
    uint8_t mesh_prefers_local_invocation_vertex_output;
    uint8_t mesh_prefers_local_invocation_primitive_output;
    uint8_t mesh_prefers_compact_vertex_output;
    uint8_t mesh_prefers_compact_primitive_output;
    uint32_t max_task_work_group_count[3];
    uint32_t max_mesh_work_group_count[3];
    uint32_t max_task_work_group_size[3];
    uint32_t max_mesh_work_group_size[3];
    uint8_t pipeline_statistic_queries;
};

/* Mesa's `pipe_caps`, flat. */
struct wc_flat_pipe_caps {
    uint8_t graphics;
    uint8_t npot_textures;
    uint8_t anisotropic_filter;
    uint8_t occlusion_query;
    uint8_t query_time_elapsed;
    uint8_t texture_shadow_map;
    uint8_t texture_swizzle;
    uint8_t texture_mirror_clamp;
    uint8_t blend_equation_separate;
    uint8_t primitive_restart;
    uint8_t primitive_restart_fixed_index;
    uint8_t indep_blend_enable;
    uint8_t indep_blend_func;
    uint8_t fs_coord_origin_upper_left;
    uint8_t fs_coord_origin_lower_left;
    uint8_t fs_coord_pixel_center_half_integer;
    uint8_t fs_coord_pixel_center_integer;
    uint8_t depth_clip_disable;
    uint8_t depth_clip_disable_separate;
    uint8_t depth_clamp_enable;
    uint8_t shader_stencil_export;
    uint8_t vs_instanceid;
    uint8_t vertex_element_instance_divisor;
    uint8_t fragment_color_clamped;
    uint8_t mixed_colorbuffer_formats;
    uint8_t seamless_cube_map;
    uint8_t seamless_cube_map_per_texture;
    uint8_t conditional_render;
    uint8_t texture_barrier;
    uint8_t stream_output_pause_resume;
    uint8_t tgsi_can_compact_constants;
    uint8_t vertex_color_unclamped;
    uint8_t vertex_color_clamped;
    uint8_t quads_follow_provoking_vertex_convention;
    uint8_t user_vertex_buffers;
    uint8_t compute;
    uint8_t start_instance;
    uint8_t query_timestamp;
    uint8_t texture_multisample;
    uint8_t cube_map_array;
    uint8_t texture_buffer_objects;
    uint8_t buffer_sampler_view_rgba_only;
    uint8_t tgsi_texcoord;
    uint8_t query_pipeline_statistics;
    uint8_t mixed_framebuffer_sizes;
    uint8_t vs_layer_viewport;
    uint8_t texture_gather_sm5;
    uint8_t buffer_map_persistent_coherent;
    uint8_t fake_sw_msaa;
    uint8_t texture_query_lod;
    uint8_t sample_shading;
    uint8_t texture_gather_offsets;
    uint8_t vs_window_space_position;
    uint8_t draw_indirect;
    uint8_t fs_fine_derivative;
    uint8_t uma;
    uint8_t conditional_render_inverted;
    uint8_t sampler_view_target;
    uint8_t clip_halfz;
    uint8_t polygon_offset_clamp;
    uint8_t multisample_z_resolve;
    uint8_t resource_from_user_memory;
    uint8_t resource_from_user_memory_compute_only;
    uint8_t device_reset_status_query;
    uint8_t texture_float_linear;
    uint8_t texture_half_float_linear;
    uint8_t depth_bounds_test;
    uint8_t native_fp32_depth;
    uint8_t texture_query_samples;
    uint8_t force_persample_interp;
    uint8_t shareable_shaders;
    uint8_t copy_between_compressed_and_plain_formats;
    uint8_t clear_scissored;
    uint8_t clear_masked;
    uint8_t draw_parameters;
    uint8_t shader_pack_half_float;
    uint8_t multi_draw_indirect;
    uint8_t multi_draw_indirect_params;
    uint8_t multi_draw_indirect_partial_stride;
    uint8_t fs_position_is_sysval;
    uint8_t fs_point_is_sysval;
    uint8_t fs_face_is_integer_sysval;
    uint8_t invalidate_buffer;
    uint8_t generate_mipmap;
    uint8_t string_marker;
    uint8_t surface_no_compress;
    uint8_t surface_reinterpret_blocks;
    uint8_t compressed_surface_reinterpret_blocks_layered;
    uint8_t query_buffer_object;
    uint8_t query_memory_info;
    uint8_t framebuffer_no_attachment;
    uint8_t robust_buffer_access_behavior;
    uint8_t cull_distance;
    uint8_t shader_group_vote;
    uint8_t shader_array_components;
    uint8_t stream_output_interleave_buffers;
    uint8_t native_fence_fd;
    uint8_t glsl_tess_levels_as_inputs;
    uint8_t legacy_math_rules;
    uint8_t fp16;
    uint8_t doubles;
    uint8_t int64;
    uint8_t shader_clock;
    uint8_t shader_realtime_clock;
    uint8_t polygon_mode_fill_rectangle;
    uint8_t shader_ballot;
    uint8_t tes_layer_viewport;
    uint8_t can_bind_const_buffer_as_vertex;
    uint8_t allow_mapped_buffers_during_execution;
    uint8_t post_depth_coverage;
    uint8_t bindless_texture;
    uint8_t nir_samplers_as_deref;
    uint8_t query_so_overflow;
    uint8_t memobj;
    uint8_t load_constbuf;
    uint8_t tile_raster_order;
    uint8_t signed_vertex_buffer_offset;
    uint8_t fence_signal;
    uint8_t packed_uniforms;
    uint8_t conservative_raster_post_snap_triangles;
    uint8_t conservative_raster_post_snap_points_lines;
    uint8_t conservative_raster_pre_snap_triangles;
    uint8_t conservative_raster_pre_snap_points_lines;
    uint8_t conservative_raster_post_depth_coverage;
    uint8_t conservative_raster_inner_coverage;
    uint8_t programmable_sample_locations;
    uint8_t texture_mirror_clamp_to_edge;
    uint8_t surface_sample_count;
    uint8_t image_atomic_float_add;
    uint8_t query_pipeline_statistics_single;
    uint8_t dest_surface_srgb_control;
    uint8_t compute_grid_info_last_block;
    uint8_t compute_shader_derivatives;
    uint8_t image_load_formatted;
    uint8_t image_store_formatted;
    uint8_t throttle;
    uint8_t cl_gl_sharing;
    uint8_t prefer_compute_for_multimedia;
    uint8_t fragment_shader_interlock;
    uint8_t fbfetch_coherent;
    uint8_t atomic_float_minmax;
    uint8_t fragment_shader_texture_lod;
    uint8_t fragment_shader_derivatives;
    uint8_t fragment_shader_depth;
    uint8_t texture_shadow_lod;
    uint8_t shader_samples_identical;
    uint8_t image_atomic_inc_wrap;
    uint8_t prefer_imm_arrays_as_constbuf;
    uint8_t gl_spirv;
    uint8_t gl_spirv_variable_pointers;
    uint8_t demote_to_helper_invocation;
    uint8_t tgsi_tg4_component_in_swizzle;
    uint8_t flatshade;
    uint8_t alpha_test;
    uint8_t two_sided_color;
    uint8_t opencl_integer_functions;
    uint8_t integer_multiply_32x16;
    uint8_t frontend_noop;
    uint8_t nir_images_as_deref;
    uint8_t packed_stream_output;
    uint8_t viewport_transform_lowered;
    uint8_t psiz_clamped;
    uint8_t viewport_swizzle;
    uint8_t system_svm;
    uint8_t viewport_mask;
    uint8_t alpha_to_coverage_dither_control;
    uint8_t map_unsynchronized_thread_safe;
    uint8_t nir_atomics_as_deref;
    uint8_t no_clip_on_copy_tex;
    uint8_t shader_atomic_int64;
    uint8_t device_protected_surface;
    uint8_t prefer_real_buffer_in_constbuf0;
    uint8_t gl_clamp;
    uint8_t texrect;
    uint8_t sampler_reduction_minmax;
    uint8_t sampler_reduction_minmax_arb;
    uint8_t allow_dynamic_vao_fastpath;
    uint8_t emulate_nonfixed_primitive_restart;
    uint8_t prefer_back_buffer_reuse;
    uint8_t draw_vertex_state;
    uint8_t prefer_pot_aligned_varyings;
    uint8_t sparse_texture_full_array_cube_mipmaps;
    uint8_t query_sparse_texture_residency;
    uint8_t clamp_sparse_texture_lod;
    uint8_t allow_draw_out_of_order;
    uint8_t hardware_gl_select;
    uint8_t dithering;
    uint8_t fbfetch_zs;
    uint8_t timeline_semaphore_import;
    uint8_t device_protected_context;
    uint8_t allow_glthread_buffer_subdata_opt;
    uint8_t null_textures;
    uint8_t astc_void_extents_need_denorm_flush;
    uint8_t validate_all_dirty_states;
    uint8_t has_const_bw;
    uint8_t performance_monitor;
    uint8_t texture_sampler_independent;
    uint8_t astc_decode_mode;
    uint8_t shader_subgroup_quad_all_stages;
    uint8_t call_finalize_nir_in_linker;
    uint8_t mesh_shader;
    uint8_t representative_fragment_test;
    uint8_t prefer_persp;
    uint8_t blit_3d;
    uint8_t glsl_bindless_handles_are_32bit;
    uint8_t polygon_stipple;
    uint8_t disable_custom_border_color_caching;
    uint8_t needs_split_planar_samplers;
    int32_t accelerated;
    int32_t min_texel_offset;
    int32_t max_texel_offset;
    int32_t min_texture_gather_offset;
    int32_t max_texture_gather_offset;
    uint32_t max_dual_source_render_targets;
    uint32_t max_render_targets;
    uint32_t max_texture_2d_size;
    uint32_t max_texture_3d_levels;
    uint32_t max_texture_cube_levels;
    uint32_t max_stream_output_buffers;
    uint32_t max_texture_array_layers;
    uint32_t max_stream_output_separate_components;
    uint32_t max_stream_output_interleaved_components;
    uint32_t glsl_feature_level;
    uint32_t glsl_feature_level_compatibility;
    uint32_t essl_feature_level;
    uint32_t constant_buffer_offset_alignment;
    uint32_t timer_resolution;
    uint32_t min_map_buffer_alignment;
    uint32_t texture_buffer_offset_alignment;
    uint32_t linear_image_pitch_alignment;
    uint32_t linear_image_base_address_alignment;
    uint32_t texture_transfer_modes;
    uint32_t texture_border_color_quirk;
    uint32_t max_texel_buffer_elements;
    uint32_t max_viewports;
    uint32_t max_geometry_output_vertices;
    uint32_t max_geometry_total_output_components;
    uint32_t max_texture_gather_components;
    uint32_t max_vertex_streams;
    uint32_t vendor_id;
    uint32_t device_id;
    uint32_t video_memory;
    uint32_t max_vertex_attrib_stride;
    uint32_t max_shader_patch_varyings;
    uint32_t shader_buffer_offset_alignment;
    uint32_t pci_group;
    uint32_t pci_bus;
    uint32_t pci_device;
    uint32_t pci_function;
    uint32_t max_window_rectangles;
    uint32_t viewport_subpixel_bits;
    uint32_t rasterizer_subpixel_bits;
    uint32_t mixed_color_depth_bits;
    uint32_t fbfetch;
    uint32_t blend_equation_advanced;
    uint32_t sparse_buffer_page_size;
    uint32_t max_combined_shader_output_resources;
    uint32_t framebuffer_msaa_constraints;
    uint32_t context_priority_mask;
    uint32_t constbuf0_flags;
    uint32_t max_conservative_raster_subpixel_precision_bias;
    uint32_t max_gs_invocations;
    uint32_t max_shader_buffer_size;
    uint32_t max_combined_shader_buffers;
    uint32_t max_combined_image_uniforms;
    uint32_t max_combined_hw_atomic_counters;
    uint32_t max_combined_hw_atomic_counter_buffers;
    uint32_t max_texture_upload_memory_budget;
    uint32_t max_vertex_element_src_offset;
    uint32_t max_varyings;
    uint64_t ignored_output_varyings[8];
    uint32_t dmabuf;
    uint32_t clip_planes;
    uint32_t max_vertex_buffers;
    uint32_t gl_begin_end_buffer_size;
    uint32_t glsl_zero_init;
    uint32_t max_texture_mb;
    uint32_t supported_prim_modes;
    uint32_t supported_prim_modes_with_restart;
    uint32_t max_sparse_texture_size;
    uint32_t max_sparse_3d_texture_size;
    uint32_t max_sparse_array_texture_layers;
    uint32_t max_constant_buffer_size;
    uint32_t query_timestamp_bits;
    uint32_t shader_subgroup_size;
    uint32_t shader_subgroup_supported_stages;
    uint32_t shader_subgroup_supported_features;
    uint32_t shader_pixel_local_storage_size;
    uint32_t shader_pixel_local_storage_fast_size;
    uint32_t multiview;
    uint32_t max_label_length;
    uint64_t max_timeline_semaphore_difference;
    uint32_t map_client_pointer;
    uint64_t min_vma;
    uint64_t max_vma;
    uint16_t hw_clear_buffer_sizes;
    uint32_t device_type;
    uint32_t vertex_input_alignment;
    uint32_t endianness;
    uint32_t point_size_fixed;
    float min_line_width;
    float min_line_width_aa;
    float max_line_width;
    float max_line_width_aa;
    float line_width_granularity;
    float min_point_size;
    float min_point_size_aa;
    float max_point_size;
    float max_point_size_aa;
    float point_size_granularity;
    float max_texture_anisotropy;
    float max_texture_lod_bias;
    float min_conservative_raster_dilate;
    float max_conservative_raster_dilate;
    float conservative_raster_dilate_granularity;
    struct wc_flat_pipe_mesh_caps mesh;
};

/* Mesa's `pipe_clip_state`, flat. */
struct wc_flat_pipe_clip_state {
    float ucp[8][4];
};

/* Mesa's `pipe_compute_caps`, flat. */
struct wc_flat_pipe_compute_caps {
    uint32_t address_bits;
    uint32_t grid_dimension;
    uint32_t max_grid_size[3];
    uint32_t max_block_size[3];
    uint32_t max_threads_per_block;
    uint32_t max_local_size;
    uint32_t max_clock_frequency;
    uint32_t max_compute_units;
    uint32_t max_subgroups;
    uint32_t subgroup_sizes;
    uint32_t max_variable_threads_per_block;
    uint64_t max_mem_alloc_size;
    uint64_t max_global_size;
};

/* Mesa's `pipe_compute_state`, flat. */
struct wc_flat_pipe_compute_state {
    uint32_t static_shared_mem;
};

/* Mesa's `pipe_constant_buffer`, flat. */
struct wc_flat_pipe_constant_buffer {
    uintptr_t buffer;
    uint32_t buffer_offset;
    uint32_t buffer_size;
    uintptr_t user_buffer;
};

/* Mesa's `pipe_stencil_state`, flat. */
struct wc_flat_pipe_stencil_state {
    uint32_t enabled;
    uint32_t func;
    uint32_t fail_op;
    uint32_t zpass_op;
    uint32_t zfail_op;
    uint32_t valuemask;
    uint32_t writemask;
};

/* Mesa's `pipe_depth_stencil_alpha_state`, flat. */
struct wc_flat_pipe_depth_stencil_alpha_state {
    struct wc_flat_pipe_stencil_state stencil[2];
    uint32_t alpha_enabled;
    uint32_t alpha_func;
    uint32_t depth_enabled;
    uint32_t depth_writemask;
    uint32_t depth_func;
    uint32_t depth_bounds_test;
    float alpha_ref_value;
    double depth_bounds_min;
    double depth_bounds_max;
};

/* Mesa's `pipe_draw_indirect_info`, flat. */
struct wc_flat_pipe_draw_indirect_info {
    uint32_t offset;
    uint32_t stride;
    uint32_t draw_count;
    uint32_t indirect_draw_count_offset;
    uintptr_t buffer;
    uintptr_t indirect_draw_count;
    uintptr_t count_from_stream_output;
};

/* Mesa's `pipe_draw_info.index`, flat. */
struct wc_flat_pipe_draw_info__index {
    uintptr_t resource;
    uintptr_t user;
};

/* Mesa's `pipe_draw_info`, flat. */
struct wc_flat_pipe_draw_info {
    uint32_t mode;
    uint16_t index_size;
    uint8_t primitive_restart;
    uint8_t has_user_indices;
    uint8_t index_bounds_valid;
    uint8_t increment_draw_id;
    uint8_t index_bias_varies;
    uint8_t was_line_loop;
    uint8_t _pad;
    uint32_t start_instance;
    uint32_t instance_count;
    uint32_t restart_index;
    struct wc_flat_pipe_draw_info__index index;
    uint32_t min_index;
    uint32_t max_index;
};

/* Mesa's `pipe_draw_start_count_bias`, flat. */
struct wc_flat_pipe_draw_start_count_bias {
    uint32_t start;
    uint32_t count;
    int32_t index_bias;
};

/* Mesa's `pipe_reference`, flat. */
struct wc_flat_pipe_reference {
    int32_t count;
};

/* Mesa's `pipe_surface`, flat. */
struct wc_flat_pipe_surface {
    struct wc_flat_pipe_reference reference;
    uint32_t format;
    uint32_t nr_samples;
    uint32_t first_layer;
    uint32_t last_layer;
    uint32_t level;
    uintptr_t texture;
};

/* Mesa's `pipe_framebuffer_state`, flat. */
struct wc_flat_pipe_framebuffer_state {
    uint32_t width;
    uint32_t height;
    uint16_t layers;
    uint8_t samples;
    uint8_t nr_cbufs;
    uint8_t pls_enabled;
    uint8_t downscale_cbufs;
    uint8_t _pad;
    uint8_t viewmask;
    struct wc_flat_pipe_surface cbufs[8];
    struct wc_flat_pipe_surface zsbuf;
    uintptr_t resolve;
};

/* Mesa's `pipe_grid_info`, flat. */
struct wc_flat_pipe_grid_info {
    uint32_t variable_shared_mem;
    uint32_t work_dim;
    uint32_t block[3];
    uint32_t last_block[3];
    uint32_t grid[3];
    uint32_t grid_base[3];
    uintptr_t indirect;
    uint32_t indirect_offset;
    uint32_t indirect_stride;
    uint32_t draw_count;
    uint32_t indirect_draw_count_offset;
    uintptr_t indirect_draw_count;
};

/* Mesa's `pipe_image_view.u.tex`, flat. */
struct wc_flat_pipe_image_view__u__tex {
    uint32_t first_layer;
    uint32_t last_layer;
    uint32_t level;
    uint8_t single_layer_view;
    uint8_t is_2d_view_of_3d;
};

/* Mesa's `pipe_image_view.u.buf`, flat. */
struct wc_flat_pipe_image_view__u__buf {
    uint32_t offset;
    uint32_t size;
};

/* Mesa's `pipe_tex2d_from_buf`, flat. */
struct wc_flat_pipe_tex2d_from_buf {
    uint32_t offset;
    uint16_t row_stride;
    uint16_t width;
    uint16_t height;
};

/* Mesa's `pipe_image_view.u`, flat. */
struct wc_flat_pipe_image_view__u {
    struct wc_flat_pipe_image_view__u__tex tex;
    struct wc_flat_pipe_image_view__u__buf buf;
    struct wc_flat_pipe_tex2d_from_buf tex2d_from_buf;
};

/* Mesa's `pipe_image_view`, flat. */
struct wc_flat_pipe_image_view {
    uintptr_t resource;
    uint32_t format;
    uint16_t access;
    uint16_t shader_access;
    struct wc_flat_pipe_image_view__u u;
};

/* Mesa's `pipe_poly_stipple`, flat. */
struct wc_flat_pipe_poly_stipple {
    uint32_t stipple[32];
};

/* Mesa's `pipe_rasterizer_state`, flat. */
struct wc_flat_pipe_rasterizer_state {
    uint32_t flatshade;
    uint32_t light_twoside;
    uint32_t clamp_vertex_color;
    uint32_t clamp_fragment_color;
    uint32_t front_ccw;
    uint32_t cull_face;
    uint32_t fill_front;
    uint32_t fill_back;
    uint32_t offset_point;
    uint32_t offset_line;
    uint32_t offset_tri;
    uint32_t scissor;
    uint32_t poly_smooth;
    uint32_t poly_stipple_enable;
    uint32_t point_smooth;
    uint32_t sprite_coord_mode;
    uint32_t point_quad_rasterization;
    uint32_t point_line_tri_clip;
    uint32_t point_size_per_vertex;
    uint32_t multisample;
    uint32_t no_ms_sample_mask_out;
    uint32_t force_persample_interp;
    uint32_t line_smooth;
    uint32_t line_stipple_enable;
    uint32_t line_last_pixel;
    uint32_t line_rectangular;
    uint32_t conservative_raster_mode;
    uint32_t flatshade_first;
    uint32_t half_pixel_center;
    uint32_t bottom_edge_rule;
    uint32_t subpixel_precision_x;
    uint32_t subpixel_precision_y;
    uint32_t rasterizer_discard;
    uint32_t rasterization_stream;
    uint32_t tile_raster_order_fixed;
    uint32_t tile_raster_order_increasing_x;
    uint32_t tile_raster_order_increasing_y;
    uint32_t depth_clip_near;
    uint32_t depth_clip_far;
    uint32_t depth_clamp;
    uint32_t clip_halfz;
    uint32_t offset_units_unscaled;
    uint32_t unclamped_fragment_depth_values;
    uint32_t clip_plane_enable;
    uint32_t representative_fragment_test;
    uint32_t line_stipple_factor;
    uint32_t line_stipple_pattern;
    uint16_t sprite_coord_enable;
    float line_width;
    float point_size;
    float offset_units;
    float offset_scale;
    float offset_clamp;
    float conservative_raster_dilate;
};

/* Mesa's `pipe_resource`, flat. */
struct wc_flat_pipe_resource {
    uint32_t width0;
    uint32_t height0;
    uint16_t depth0;
    uint16_t array_size;
    uint32_t format;
    uint32_t target;
    uint8_t last_level;
    uint8_t nr_samples;
    uint8_t nr_storage_samples;
    uint8_t nr_sparse_levels;
    uint32_t compression_rate;
    uint32_t usage;
    uint32_t bind;
    uint32_t flags;
};

/* Mesa's `pipe_sampler_state`, flat. */
struct wc_flat_pipe_sampler_state {
    uint32_t wrap_s;
    uint32_t wrap_t;
    uint32_t wrap_r;
    uint32_t min_img_filter;
    uint32_t min_mip_filter;
    uint32_t mag_img_filter;
    uint32_t compare_mode;
    uint32_t compare_func;
    uint32_t unnormalized_coords;
    uint32_t max_anisotropy;
    uint32_t seamless_cube_map;
    uint32_t border_color_is_integer;
    uint32_t reduction_mode;
    uint32_t pad;
    float lod_bias;
    float min_lod;
    float max_lod;
    uint32_t border_color[4];
    uint32_t border_color_format;
};

/* Mesa's `pipe_sampler_view.u.tex`, flat. */
struct wc_flat_pipe_sampler_view__u__tex {
    uint32_t first_layer;
    uint32_t last_layer;
    uint32_t first_level;
    uint32_t last_level;
    float min_lod_clamp;
};

/* Mesa's `pipe_sampler_view.u.buf`, flat. */
struct wc_flat_pipe_sampler_view__u__buf {
    uint32_t offset;
    uint32_t size;
};

/* Mesa's `pipe_sampler_view.u`, flat. */
struct wc_flat_pipe_sampler_view__u {
    struct wc_flat_pipe_sampler_view__u__tex tex;
    struct wc_flat_pipe_sampler_view__u__buf buf;
    struct wc_flat_pipe_tex2d_from_buf tex2d_from_buf;
};

/* Mesa's `pipe_sampler_view`, flat. */
struct wc_flat_pipe_sampler_view {
    uint32_t format;
    uint32_t astc_decode_format;
    uint8_t is_tex2d_from_buf;
    uint32_t target;
    uint32_t swizzle_r;
    uint32_t swizzle_g;
    uint32_t swizzle_b;
    uint32_t swizzle_a;
    struct wc_flat_pipe_sampler_view__u u;
};

/* Mesa's `pipe_shader_buffer`, flat. */
struct wc_flat_pipe_shader_buffer {
    uintptr_t buffer;
    uint32_t buffer_offset;
    uint32_t buffer_size;
};

/* Mesa's `pipe_shader_caps`, flat. */
struct wc_flat_pipe_shader_caps {
    uint32_t max_instructions;
    uint32_t max_alu_instructions;
    uint32_t max_tex_instructions;
    uint32_t max_tex_indirections;
    uint32_t max_control_flow_depth;
    uint32_t max_inputs;
    uint32_t max_outputs;
    uint32_t max_const_buffer0_size;
    uint32_t max_const_buffers;
    uint32_t max_temps;
    uint32_t max_texture_samplers;
    uint32_t max_sampler_views;
    uint32_t max_shader_buffers;
    uint32_t max_shader_images;
    uint32_t max_hw_atomic_counters;
    uint32_t max_hw_atomic_counter_buffers;
    uint32_t supported_irs;
    uint8_t cont_supported;
    uint8_t indirect_temp_addr;
    uint8_t indirect_const_addr;
    uint8_t subroutines;
    uint8_t integers;
    uint8_t int64_atomics;
    uint8_t fp16;
    uint8_t fp16_derivatives;
    uint8_t fp16_const_buffers;
    uint8_t fp16_no_denorms;
    uint8_t int16;
    uint8_t glsl_16bit_consts;
    uint8_t glsl_16bit_load_dst;
    uint8_t tgsi_sqrt_supported;
    uint8_t tgsi_any_inout_decl_range;
};

/* Mesa's `pipe_stream_output`, flat. */
struct wc_flat_pipe_stream_output {
    uint32_t register_index;
    uint32_t start_component;
    uint32_t num_components;
    uint32_t output_buffer;
    uint32_t dst_offset;
    uint32_t stream;
};

/* Mesa's `pipe_stream_output_info`, flat. */
struct wc_flat_pipe_stream_output_info {
    uint32_t num_outputs;
    uint16_t stride[4];
    struct wc_flat_pipe_stream_output output[128];
};

/* Mesa's `pipe_shader_state`, flat. */
struct wc_flat_pipe_shader_state {
    struct wc_flat_pipe_stream_output_info stream_output;
    uint8_t report_compile_error;
};

/* Mesa's `pipe_stencil_ref`, flat. */
struct wc_flat_pipe_stencil_ref {
    uint8_t ref_value[2];
};

/* Mesa's `pipe_vertex_buffer.buffer`, flat. */
struct wc_flat_pipe_vertex_buffer__buffer {
    uintptr_t resource;
};

/* Mesa's `pipe_vertex_buffer`, flat. */
struct wc_flat_pipe_vertex_buffer {
    uint8_t is_user_buffer;
    uint32_t buffer_offset;
    struct wc_flat_pipe_vertex_buffer__buffer buffer;
};

/* Mesa's `pipe_vertex_element`, flat. */
struct wc_flat_pipe_vertex_element {
    uint16_t src_offset;
    uint8_t vertex_buffer_index;
    uint8_t dual_slot;
    uint8_t src_format;
    uint32_t src_stride;
    uint32_t instance_divisor;
};

/* Mesa's `pipe_viewport_state`, flat. */
struct wc_flat_pipe_viewport_state {
    float scale[3];
    float translate[3];
    uint32_t swizzle_x;
    uint32_t swizzle_y;
    uint32_t swizzle_z;
    uint32_t swizzle_w;
};


/* Each structure Mesa passes, flattened (flat.c). */
struct pipe_blend_color;
void wc_flat_pipe_blend_color_from(const struct pipe_blend_color *in, struct wc_flat_pipe_blend_color *out);
struct pipe_blend_state;
void wc_flat_pipe_blend_state_from(const struct pipe_blend_state *in, struct wc_flat_pipe_blend_state *out);
struct pipe_blit_info;
void wc_flat_pipe_blit_info_from(const struct pipe_blit_info *in, struct wc_flat_pipe_blit_info *out);
struct pipe_box;
void wc_flat_pipe_box_from(const struct pipe_box *in, struct wc_flat_pipe_box *out);
struct pipe_caps;
void wc_flat_pipe_caps_from(const struct pipe_caps *in, struct wc_flat_pipe_caps *out);
struct pipe_clip_state;
void wc_flat_pipe_clip_state_from(const struct pipe_clip_state *in, struct wc_flat_pipe_clip_state *out);
struct pipe_compute_caps;
void wc_flat_pipe_compute_caps_from(const struct pipe_compute_caps *in, struct wc_flat_pipe_compute_caps *out);
struct pipe_compute_state;
void wc_flat_pipe_compute_state_from(const struct pipe_compute_state *in, struct wc_flat_pipe_compute_state *out);
struct pipe_constant_buffer;
void wc_flat_pipe_constant_buffer_from(const struct pipe_constant_buffer *in, struct wc_flat_pipe_constant_buffer *out);
struct pipe_depth_stencil_alpha_state;
void wc_flat_pipe_depth_stencil_alpha_state_from(const struct pipe_depth_stencil_alpha_state *in, struct wc_flat_pipe_depth_stencil_alpha_state *out);
struct pipe_draw_indirect_info;
void wc_flat_pipe_draw_indirect_info_from(const struct pipe_draw_indirect_info *in, struct wc_flat_pipe_draw_indirect_info *out);
struct pipe_draw_info;
void wc_flat_pipe_draw_info_from(const struct pipe_draw_info *in, struct wc_flat_pipe_draw_info *out);
struct pipe_draw_start_count_bias;
void wc_flat_pipe_draw_start_count_bias_from(const struct pipe_draw_start_count_bias *in, struct wc_flat_pipe_draw_start_count_bias *out);
struct pipe_framebuffer_state;
void wc_flat_pipe_framebuffer_state_from(const struct pipe_framebuffer_state *in, struct wc_flat_pipe_framebuffer_state *out);
struct pipe_grid_info;
void wc_flat_pipe_grid_info_from(const struct pipe_grid_info *in, struct wc_flat_pipe_grid_info *out);
struct pipe_image_view;
void wc_flat_pipe_image_view_from(const struct pipe_image_view *in, struct wc_flat_pipe_image_view *out);
struct pipe_mesh_caps;
void wc_flat_pipe_mesh_caps_from(const struct pipe_mesh_caps *in, struct wc_flat_pipe_mesh_caps *out);
struct pipe_poly_stipple;
void wc_flat_pipe_poly_stipple_from(const struct pipe_poly_stipple *in, struct wc_flat_pipe_poly_stipple *out);
struct pipe_rasterizer_state;
void wc_flat_pipe_rasterizer_state_from(const struct pipe_rasterizer_state *in, struct wc_flat_pipe_rasterizer_state *out);
struct pipe_reference;
void wc_flat_pipe_reference_from(const struct pipe_reference *in, struct wc_flat_pipe_reference *out);
struct pipe_resource;
void wc_flat_pipe_resource_from(const struct pipe_resource *in, struct wc_flat_pipe_resource *out);
struct pipe_rt_blend_state;
void wc_flat_pipe_rt_blend_state_from(const struct pipe_rt_blend_state *in, struct wc_flat_pipe_rt_blend_state *out);
struct pipe_sampler_state;
void wc_flat_pipe_sampler_state_from(const struct pipe_sampler_state *in, struct wc_flat_pipe_sampler_state *out);
struct pipe_sampler_view;
void wc_flat_pipe_sampler_view_from(const struct pipe_sampler_view *in, struct wc_flat_pipe_sampler_view *out);
struct pipe_scissor_state;
void wc_flat_pipe_scissor_state_from(const struct pipe_scissor_state *in, struct wc_flat_pipe_scissor_state *out);
struct pipe_shader_buffer;
void wc_flat_pipe_shader_buffer_from(const struct pipe_shader_buffer *in, struct wc_flat_pipe_shader_buffer *out);
struct pipe_shader_caps;
void wc_flat_pipe_shader_caps_from(const struct pipe_shader_caps *in, struct wc_flat_pipe_shader_caps *out);
struct pipe_shader_state;
void wc_flat_pipe_shader_state_from(const struct pipe_shader_state *in, struct wc_flat_pipe_shader_state *out);
struct pipe_stencil_ref;
void wc_flat_pipe_stencil_ref_from(const struct pipe_stencil_ref *in, struct wc_flat_pipe_stencil_ref *out);
struct pipe_stencil_state;
void wc_flat_pipe_stencil_state_from(const struct pipe_stencil_state *in, struct wc_flat_pipe_stencil_state *out);
struct pipe_stream_output;
void wc_flat_pipe_stream_output_from(const struct pipe_stream_output *in, struct wc_flat_pipe_stream_output *out);
struct pipe_stream_output_info;
void wc_flat_pipe_stream_output_info_from(const struct pipe_stream_output_info *in, struct wc_flat_pipe_stream_output_info *out);
struct pipe_surface;
void wc_flat_pipe_surface_from(const struct pipe_surface *in, struct wc_flat_pipe_surface *out);
struct pipe_tex2d_from_buf;
void wc_flat_pipe_tex2d_from_buf_from(const struct pipe_tex2d_from_buf *in, struct wc_flat_pipe_tex2d_from_buf *out);
struct pipe_vertex_buffer;
void wc_flat_pipe_vertex_buffer_from(const struct pipe_vertex_buffer *in, struct wc_flat_pipe_vertex_buffer *out);
struct pipe_vertex_element;
void wc_flat_pipe_vertex_element_from(const struct pipe_vertex_element *in, struct wc_flat_pipe_vertex_element *out);
struct pipe_viewport_state;
void wc_flat_pipe_viewport_state_from(const struct pipe_viewport_state *in, struct wc_flat_pipe_viewport_state *out);

#endif
