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

/* Mesa's `pipe_clip_state`, flat. */
struct wc_flat_pipe_clip_state {
    float ucp[8][4];
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


#endif
