/* Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit. */
/* Each structure Mesa passes, copied into its flat form; and each flat form's size, which the Rust side checks. */
#include <string.h>

#include "pipe/p_context.h"
#include "pipe/p_defines.h"
#include "pipe/p_screen.h"
#include "pipe/p_state.h"
#include "util/u_transfer_helper.h"

#include "flat.h"

void wc_flat_pipe_blend_color_from(const struct pipe_blend_color *in, struct wc_flat_pipe_blend_color *out);
void wc_flat_pipe_blend_state_from(const struct pipe_blend_state *in, struct wc_flat_pipe_blend_state *out);
void wc_flat_pipe_blit_info_from(const struct pipe_blit_info *in, struct wc_flat_pipe_blit_info *out);
void wc_flat_pipe_box_from(const struct pipe_box *in, struct wc_flat_pipe_box *out);
void wc_flat_pipe_clip_state_from(const struct pipe_clip_state *in, struct wc_flat_pipe_clip_state *out);
void wc_flat_pipe_compute_state_from(const struct pipe_compute_state *in, struct wc_flat_pipe_compute_state *out);
void wc_flat_pipe_constant_buffer_from(const struct pipe_constant_buffer *in, struct wc_flat_pipe_constant_buffer *out);
void wc_flat_pipe_depth_stencil_alpha_state_from(const struct pipe_depth_stencil_alpha_state *in, struct wc_flat_pipe_depth_stencil_alpha_state *out);
void wc_flat_pipe_draw_indirect_info_from(const struct pipe_draw_indirect_info *in, struct wc_flat_pipe_draw_indirect_info *out);
void wc_flat_pipe_draw_info_from(const struct pipe_draw_info *in, struct wc_flat_pipe_draw_info *out);
void wc_flat_pipe_draw_start_count_bias_from(const struct pipe_draw_start_count_bias *in, struct wc_flat_pipe_draw_start_count_bias *out);
void wc_flat_pipe_framebuffer_state_from(const struct pipe_framebuffer_state *in, struct wc_flat_pipe_framebuffer_state *out);
void wc_flat_pipe_grid_info_from(const struct pipe_grid_info *in, struct wc_flat_pipe_grid_info *out);
void wc_flat_pipe_image_view_from(const struct pipe_image_view *in, struct wc_flat_pipe_image_view *out);
void wc_flat_pipe_poly_stipple_from(const struct pipe_poly_stipple *in, struct wc_flat_pipe_poly_stipple *out);
void wc_flat_pipe_rasterizer_state_from(const struct pipe_rasterizer_state *in, struct wc_flat_pipe_rasterizer_state *out);
void wc_flat_pipe_reference_from(const struct pipe_reference *in, struct wc_flat_pipe_reference *out);
void wc_flat_pipe_resource_from(const struct pipe_resource *in, struct wc_flat_pipe_resource *out);
void wc_flat_pipe_rt_blend_state_from(const struct pipe_rt_blend_state *in, struct wc_flat_pipe_rt_blend_state *out);
void wc_flat_pipe_sampler_state_from(const struct pipe_sampler_state *in, struct wc_flat_pipe_sampler_state *out);
void wc_flat_pipe_sampler_view_from(const struct pipe_sampler_view *in, struct wc_flat_pipe_sampler_view *out);
void wc_flat_pipe_scissor_state_from(const struct pipe_scissor_state *in, struct wc_flat_pipe_scissor_state *out);
void wc_flat_pipe_shader_buffer_from(const struct pipe_shader_buffer *in, struct wc_flat_pipe_shader_buffer *out);
void wc_flat_pipe_shader_state_from(const struct pipe_shader_state *in, struct wc_flat_pipe_shader_state *out);
void wc_flat_pipe_stencil_ref_from(const struct pipe_stencil_ref *in, struct wc_flat_pipe_stencil_ref *out);
void wc_flat_pipe_stencil_state_from(const struct pipe_stencil_state *in, struct wc_flat_pipe_stencil_state *out);
void wc_flat_pipe_stream_output_from(const struct pipe_stream_output *in, struct wc_flat_pipe_stream_output *out);
void wc_flat_pipe_stream_output_info_from(const struct pipe_stream_output_info *in, struct wc_flat_pipe_stream_output_info *out);
void wc_flat_pipe_surface_from(const struct pipe_surface *in, struct wc_flat_pipe_surface *out);
void wc_flat_pipe_tex2d_from_buf_from(const struct pipe_tex2d_from_buf *in, struct wc_flat_pipe_tex2d_from_buf *out);
void wc_flat_pipe_vertex_buffer_from(const struct pipe_vertex_buffer *in, struct wc_flat_pipe_vertex_buffer *out);
void wc_flat_pipe_vertex_element_from(const struct pipe_vertex_element *in, struct wc_flat_pipe_vertex_element *out);
void wc_flat_pipe_viewport_state_from(const struct pipe_viewport_state *in, struct wc_flat_pipe_viewport_state *out);

size_t wc_flat_size(unsigned which);
size_t wc_flat_layout(unsigned which, unsigned what);

void wc_flat_pipe_blend_color_from(const struct pipe_blend_color *in, struct wc_flat_pipe_blend_color *out)
{
    memset(out, 0, sizeof *out);
    for (unsigned i0 = 0; i0 < 4; i0++) {
        out->color[i0] = in->color[i0];
    }
}

void wc_flat_pipe_blend_state_from(const struct pipe_blend_state *in, struct wc_flat_pipe_blend_state *out)
{
    memset(out, 0, sizeof *out);
    out->independent_blend_enable = in->independent_blend_enable;
    out->logicop_enable = in->logicop_enable;
    out->logicop_func = in->logicop_func;
    out->dither = in->dither;
    out->alpha_to_coverage = in->alpha_to_coverage;
    out->alpha_to_coverage_dither = in->alpha_to_coverage_dither;
    out->alpha_to_one = in->alpha_to_one;
    out->max_rt = in->max_rt;
    out->advanced_blend_func = in->advanced_blend_func;
    out->blend_coherent = in->blend_coherent;
    out->is_yuv = in->is_yuv;
    for (unsigned i0 = 0; i0 < 8; i0++) {
        out->rt[i0].blend_enable = in->rt[i0].blend_enable;
        out->rt[i0].rgb_func = in->rt[i0].rgb_func;
        out->rt[i0].rgb_src_factor = in->rt[i0].rgb_src_factor;
        out->rt[i0].rgb_dst_factor = in->rt[i0].rgb_dst_factor;
        out->rt[i0].alpha_func = in->rt[i0].alpha_func;
        out->rt[i0].alpha_src_factor = in->rt[i0].alpha_src_factor;
        out->rt[i0].alpha_dst_factor = in->rt[i0].alpha_dst_factor;
        out->rt[i0].colormask = in->rt[i0].colormask;
    }
}

void wc_flat_pipe_blit_info_from(const struct pipe_blit_info *in, struct wc_flat_pipe_blit_info *out)
{
    memset(out, 0, sizeof *out);
    out->dst.resource = (uintptr_t)in->dst.resource;
    out->dst.level = in->dst.level;
    out->dst.box.x = in->dst.box.x;
    out->dst.box.width = in->dst.box.width;
    out->dst.box.y = in->dst.box.y;
    out->dst.box.height = in->dst.box.height;
    out->dst.box.z = in->dst.box.z;
    out->dst.box.depth = in->dst.box.depth;
    out->dst.format = in->dst.format;
    out->src.resource = (uintptr_t)in->src.resource;
    out->src.level = in->src.level;
    out->src.box.x = in->src.box.x;
    out->src.box.width = in->src.box.width;
    out->src.box.y = in->src.box.y;
    out->src.box.height = in->src.box.height;
    out->src.box.z = in->src.box.z;
    out->src.box.depth = in->src.box.depth;
    out->src.format = in->src.format;
    out->mask = in->mask;
    out->filter = in->filter;
    out->dst_sample = in->dst_sample;
    out->sample0_only = in->sample0_only;
    out->scissor_enable = in->scissor_enable;
    out->scissor.minx = in->scissor.minx;
    out->scissor.miny = in->scissor.miny;
    out->scissor.maxx = in->scissor.maxx;
    out->scissor.maxy = in->scissor.maxy;
    out->swizzle_enable = in->swizzle_enable;
    for (unsigned i0 = 0; i0 < 4; i0++) {
        out->swizzle[i0] = in->swizzle[i0];
    }
    out->window_rectangle_include = in->window_rectangle_include;
    out->num_window_rectangles = in->num_window_rectangles;
    for (unsigned i0 = 0; i0 < 8; i0++) {
        out->window_rectangles[i0].minx = in->window_rectangles[i0].minx;
        out->window_rectangles[i0].miny = in->window_rectangles[i0].miny;
        out->window_rectangles[i0].maxx = in->window_rectangles[i0].maxx;
        out->window_rectangles[i0].maxy = in->window_rectangles[i0].maxy;
    }
    out->render_condition_enable = in->render_condition_enable;
    out->alpha_blend = in->alpha_blend;
}

void wc_flat_pipe_box_from(const struct pipe_box *in, struct wc_flat_pipe_box *out)
{
    memset(out, 0, sizeof *out);
    out->x = in->x;
    out->width = in->width;
    out->y = in->y;
    out->height = in->height;
    out->z = in->z;
    out->depth = in->depth;
}

void wc_flat_pipe_clip_state_from(const struct pipe_clip_state *in, struct wc_flat_pipe_clip_state *out)
{
    memset(out, 0, sizeof *out);
    for (unsigned i0 = 0; i0 < 8; i0++) {
        for (unsigned i1 = 0; i1 < 4; i1++) {
            out->ucp[i0][i1] = in->ucp[i0][i1];
        }
    }
}

void wc_flat_pipe_compute_state_from(const struct pipe_compute_state *in, struct wc_flat_pipe_compute_state *out)
{
    memset(out, 0, sizeof *out);
    out->static_shared_mem = in->static_shared_mem;
}

void wc_flat_pipe_constant_buffer_from(const struct pipe_constant_buffer *in, struct wc_flat_pipe_constant_buffer *out)
{
    memset(out, 0, sizeof *out);
    out->buffer = (uintptr_t)in->buffer;
    out->buffer_offset = in->buffer_offset;
    out->buffer_size = in->buffer_size;
    out->user_buffer = (uintptr_t)in->user_buffer;
}

void wc_flat_pipe_depth_stencil_alpha_state_from(const struct pipe_depth_stencil_alpha_state *in, struct wc_flat_pipe_depth_stencil_alpha_state *out)
{
    memset(out, 0, sizeof *out);
    for (unsigned i0 = 0; i0 < 2; i0++) {
        out->stencil[i0].enabled = in->stencil[i0].enabled;
        out->stencil[i0].func = in->stencil[i0].func;
        out->stencil[i0].fail_op = in->stencil[i0].fail_op;
        out->stencil[i0].zpass_op = in->stencil[i0].zpass_op;
        out->stencil[i0].zfail_op = in->stencil[i0].zfail_op;
        out->stencil[i0].valuemask = in->stencil[i0].valuemask;
        out->stencil[i0].writemask = in->stencil[i0].writemask;
    }
    out->alpha_enabled = in->alpha_enabled;
    out->alpha_func = in->alpha_func;
    out->depth_enabled = in->depth_enabled;
    out->depth_writemask = in->depth_writemask;
    out->depth_func = in->depth_func;
    out->depth_bounds_test = in->depth_bounds_test;
    out->alpha_ref_value = in->alpha_ref_value;
    out->depth_bounds_min = in->depth_bounds_min;
    out->depth_bounds_max = in->depth_bounds_max;
}

void wc_flat_pipe_draw_indirect_info_from(const struct pipe_draw_indirect_info *in, struct wc_flat_pipe_draw_indirect_info *out)
{
    memset(out, 0, sizeof *out);
    out->offset = in->offset;
    out->stride = in->stride;
    out->draw_count = in->draw_count;
    out->indirect_draw_count_offset = in->indirect_draw_count_offset;
    out->buffer = (uintptr_t)in->buffer;
    out->indirect_draw_count = (uintptr_t)in->indirect_draw_count;
    out->count_from_stream_output = (uintptr_t)in->count_from_stream_output;
}

void wc_flat_pipe_draw_info_from(const struct pipe_draw_info *in, struct wc_flat_pipe_draw_info *out)
{
    memset(out, 0, sizeof *out);
    out->mode = in->mode;
    out->index_size = in->index_size;
    out->primitive_restart = in->primitive_restart;
    out->has_user_indices = in->has_user_indices;
    out->index_bounds_valid = in->index_bounds_valid;
    out->increment_draw_id = in->increment_draw_id;
    out->index_bias_varies = in->index_bias_varies;
    out->was_line_loop = in->was_line_loop;
    out->_pad = in->_pad;
    out->start_instance = in->start_instance;
    out->instance_count = in->instance_count;
    out->restart_index = in->restart_index;
    out->index.resource = (uintptr_t)in->index.resource;
    out->index.user = (uintptr_t)in->index.user;
    out->min_index = in->min_index;
    out->max_index = in->max_index;
}

void wc_flat_pipe_draw_start_count_bias_from(const struct pipe_draw_start_count_bias *in, struct wc_flat_pipe_draw_start_count_bias *out)
{
    memset(out, 0, sizeof *out);
    out->start = in->start;
    out->count = in->count;
    out->index_bias = in->index_bias;
}

void wc_flat_pipe_framebuffer_state_from(const struct pipe_framebuffer_state *in, struct wc_flat_pipe_framebuffer_state *out)
{
    memset(out, 0, sizeof *out);
    out->width = in->width;
    out->height = in->height;
    out->layers = in->layers;
    out->samples = in->samples;
    out->nr_cbufs = in->nr_cbufs;
    out->pls_enabled = in->pls_enabled;
    out->downscale_cbufs = in->downscale_cbufs;
    out->_pad = in->_pad;
    out->viewmask = in->viewmask;
    for (unsigned i0 = 0; i0 < 8; i0++) {
        out->cbufs[i0].reference.count = in->cbufs[i0].reference.count;
        out->cbufs[i0].format = in->cbufs[i0].format;
        out->cbufs[i0].nr_samples = in->cbufs[i0].nr_samples;
        out->cbufs[i0].first_layer = in->cbufs[i0].first_layer;
        out->cbufs[i0].last_layer = in->cbufs[i0].last_layer;
        out->cbufs[i0].level = in->cbufs[i0].level;
        out->cbufs[i0].texture = (uintptr_t)in->cbufs[i0].texture;
    }
    out->zsbuf.reference.count = in->zsbuf.reference.count;
    out->zsbuf.format = in->zsbuf.format;
    out->zsbuf.nr_samples = in->zsbuf.nr_samples;
    out->zsbuf.first_layer = in->zsbuf.first_layer;
    out->zsbuf.last_layer = in->zsbuf.last_layer;
    out->zsbuf.level = in->zsbuf.level;
    out->zsbuf.texture = (uintptr_t)in->zsbuf.texture;
    out->resolve = (uintptr_t)in->resolve;
}

void wc_flat_pipe_grid_info_from(const struct pipe_grid_info *in, struct wc_flat_pipe_grid_info *out)
{
    memset(out, 0, sizeof *out);
    out->variable_shared_mem = in->variable_shared_mem;
    out->work_dim = in->work_dim;
    for (unsigned i0 = 0; i0 < 3; i0++) {
        out->block[i0] = in->block[i0];
    }
    for (unsigned i0 = 0; i0 < 3; i0++) {
        out->last_block[i0] = in->last_block[i0];
    }
    for (unsigned i0 = 0; i0 < 3; i0++) {
        out->grid[i0] = in->grid[i0];
    }
    for (unsigned i0 = 0; i0 < 3; i0++) {
        out->grid_base[i0] = in->grid_base[i0];
    }
    out->indirect = (uintptr_t)in->indirect;
    out->indirect_offset = in->indirect_offset;
    out->indirect_stride = in->indirect_stride;
    out->draw_count = in->draw_count;
    out->indirect_draw_count_offset = in->indirect_draw_count_offset;
    out->indirect_draw_count = (uintptr_t)in->indirect_draw_count;
}

void wc_flat_pipe_image_view_from(const struct pipe_image_view *in, struct wc_flat_pipe_image_view *out)
{
    memset(out, 0, sizeof *out);
    out->resource = (uintptr_t)in->resource;
    out->format = in->format;
    out->access = in->access;
    out->shader_access = in->shader_access;
    out->u.tex.first_layer = in->u.tex.first_layer;
    out->u.tex.last_layer = in->u.tex.last_layer;
    out->u.tex.level = in->u.tex.level;
    out->u.tex.single_layer_view = in->u.tex.single_layer_view;
    out->u.tex.is_2d_view_of_3d = in->u.tex.is_2d_view_of_3d;
    out->u.buf.offset = in->u.buf.offset;
    out->u.buf.size = in->u.buf.size;
    out->u.tex2d_from_buf.offset = in->u.tex2d_from_buf.offset;
    out->u.tex2d_from_buf.row_stride = in->u.tex2d_from_buf.row_stride;
    out->u.tex2d_from_buf.width = in->u.tex2d_from_buf.width;
    out->u.tex2d_from_buf.height = in->u.tex2d_from_buf.height;
}

void wc_flat_pipe_poly_stipple_from(const struct pipe_poly_stipple *in, struct wc_flat_pipe_poly_stipple *out)
{
    memset(out, 0, sizeof *out);
    for (unsigned i0 = 0; i0 < 32; i0++) {
        out->stipple[i0] = in->stipple[i0];
    }
}

void wc_flat_pipe_rasterizer_state_from(const struct pipe_rasterizer_state *in, struct wc_flat_pipe_rasterizer_state *out)
{
    memset(out, 0, sizeof *out);
    out->flatshade = in->flatshade;
    out->light_twoside = in->light_twoside;
    out->clamp_vertex_color = in->clamp_vertex_color;
    out->clamp_fragment_color = in->clamp_fragment_color;
    out->front_ccw = in->front_ccw;
    out->cull_face = in->cull_face;
    out->fill_front = in->fill_front;
    out->fill_back = in->fill_back;
    out->offset_point = in->offset_point;
    out->offset_line = in->offset_line;
    out->offset_tri = in->offset_tri;
    out->scissor = in->scissor;
    out->poly_smooth = in->poly_smooth;
    out->poly_stipple_enable = in->poly_stipple_enable;
    out->point_smooth = in->point_smooth;
    out->sprite_coord_mode = in->sprite_coord_mode;
    out->point_quad_rasterization = in->point_quad_rasterization;
    out->point_line_tri_clip = in->point_line_tri_clip;
    out->point_size_per_vertex = in->point_size_per_vertex;
    out->multisample = in->multisample;
    out->no_ms_sample_mask_out = in->no_ms_sample_mask_out;
    out->force_persample_interp = in->force_persample_interp;
    out->line_smooth = in->line_smooth;
    out->line_stipple_enable = in->line_stipple_enable;
    out->line_last_pixel = in->line_last_pixel;
    out->line_rectangular = in->line_rectangular;
    out->conservative_raster_mode = in->conservative_raster_mode;
    out->flatshade_first = in->flatshade_first;
    out->half_pixel_center = in->half_pixel_center;
    out->bottom_edge_rule = in->bottom_edge_rule;
    out->subpixel_precision_x = in->subpixel_precision_x;
    out->subpixel_precision_y = in->subpixel_precision_y;
    out->rasterizer_discard = in->rasterizer_discard;
    out->rasterization_stream = in->rasterization_stream;
    out->tile_raster_order_fixed = in->tile_raster_order_fixed;
    out->tile_raster_order_increasing_x = in->tile_raster_order_increasing_x;
    out->tile_raster_order_increasing_y = in->tile_raster_order_increasing_y;
    out->depth_clip_near = in->depth_clip_near;
    out->depth_clip_far = in->depth_clip_far;
    out->depth_clamp = in->depth_clamp;
    out->clip_halfz = in->clip_halfz;
    out->offset_units_unscaled = in->offset_units_unscaled;
    out->unclamped_fragment_depth_values = in->unclamped_fragment_depth_values;
    out->clip_plane_enable = in->clip_plane_enable;
    out->representative_fragment_test = in->representative_fragment_test;
    out->line_stipple_factor = in->line_stipple_factor;
    out->line_stipple_pattern = in->line_stipple_pattern;
    out->sprite_coord_enable = in->sprite_coord_enable;
    out->line_width = in->line_width;
    out->point_size = in->point_size;
    out->offset_units = in->offset_units;
    out->offset_scale = in->offset_scale;
    out->offset_clamp = in->offset_clamp;
    out->conservative_raster_dilate = in->conservative_raster_dilate;
}

void wc_flat_pipe_reference_from(const struct pipe_reference *in, struct wc_flat_pipe_reference *out)
{
    memset(out, 0, sizeof *out);
    out->count = in->count;
}

void wc_flat_pipe_resource_from(const struct pipe_resource *in, struct wc_flat_pipe_resource *out)
{
    memset(out, 0, sizeof *out);
    out->width0 = in->width0;
    out->height0 = in->height0;
    out->depth0 = in->depth0;
    out->array_size = in->array_size;
    out->format = in->format;
    out->target = in->target;
    out->last_level = in->last_level;
    out->nr_samples = in->nr_samples;
    out->nr_storage_samples = in->nr_storage_samples;
    out->nr_sparse_levels = in->nr_sparse_levels;
    out->compression_rate = in->compression_rate;
    out->usage = in->usage;
    out->bind = in->bind;
    out->flags = in->flags;
}

void wc_flat_pipe_rt_blend_state_from(const struct pipe_rt_blend_state *in, struct wc_flat_pipe_rt_blend_state *out)
{
    memset(out, 0, sizeof *out);
    out->blend_enable = in->blend_enable;
    out->rgb_func = in->rgb_func;
    out->rgb_src_factor = in->rgb_src_factor;
    out->rgb_dst_factor = in->rgb_dst_factor;
    out->alpha_func = in->alpha_func;
    out->alpha_src_factor = in->alpha_src_factor;
    out->alpha_dst_factor = in->alpha_dst_factor;
    out->colormask = in->colormask;
}

void wc_flat_pipe_sampler_state_from(const struct pipe_sampler_state *in, struct wc_flat_pipe_sampler_state *out)
{
    memset(out, 0, sizeof *out);
    out->wrap_s = in->wrap_s;
    out->wrap_t = in->wrap_t;
    out->wrap_r = in->wrap_r;
    out->min_img_filter = in->min_img_filter;
    out->min_mip_filter = in->min_mip_filter;
    out->mag_img_filter = in->mag_img_filter;
    out->compare_mode = in->compare_mode;
    out->compare_func = in->compare_func;
    out->unnormalized_coords = in->unnormalized_coords;
    out->max_anisotropy = in->max_anisotropy;
    out->seamless_cube_map = in->seamless_cube_map;
    out->border_color_is_integer = in->border_color_is_integer;
    out->reduction_mode = in->reduction_mode;
    out->pad = in->pad;
    out->lod_bias = in->lod_bias;
    out->min_lod = in->min_lod;
    out->max_lod = in->max_lod;
    memcpy(out->border_color, &in->border_color, sizeof out->border_color);
    out->border_color_format = in->border_color_format;
}

void wc_flat_pipe_sampler_view_from(const struct pipe_sampler_view *in, struct wc_flat_pipe_sampler_view *out)
{
    memset(out, 0, sizeof *out);
    out->format = in->format;
    out->astc_decode_format = in->astc_decode_format;
    out->is_tex2d_from_buf = in->is_tex2d_from_buf;
    out->target = in->target;
    out->swizzle_r = in->swizzle_r;
    out->swizzle_g = in->swizzle_g;
    out->swizzle_b = in->swizzle_b;
    out->swizzle_a = in->swizzle_a;
    out->u.tex.first_layer = in->u.tex.first_layer;
    out->u.tex.last_layer = in->u.tex.last_layer;
    out->u.tex.first_level = in->u.tex.first_level;
    out->u.tex.last_level = in->u.tex.last_level;
    out->u.tex.min_lod_clamp = in->u.tex.min_lod_clamp;
    out->u.buf.offset = in->u.buf.offset;
    out->u.buf.size = in->u.buf.size;
    out->u.tex2d_from_buf.offset = in->u.tex2d_from_buf.offset;
    out->u.tex2d_from_buf.row_stride = in->u.tex2d_from_buf.row_stride;
    out->u.tex2d_from_buf.width = in->u.tex2d_from_buf.width;
    out->u.tex2d_from_buf.height = in->u.tex2d_from_buf.height;
}

void wc_flat_pipe_scissor_state_from(const struct pipe_scissor_state *in, struct wc_flat_pipe_scissor_state *out)
{
    memset(out, 0, sizeof *out);
    out->minx = in->minx;
    out->miny = in->miny;
    out->maxx = in->maxx;
    out->maxy = in->maxy;
}

void wc_flat_pipe_shader_buffer_from(const struct pipe_shader_buffer *in, struct wc_flat_pipe_shader_buffer *out)
{
    memset(out, 0, sizeof *out);
    out->buffer = (uintptr_t)in->buffer;
    out->buffer_offset = in->buffer_offset;
    out->buffer_size = in->buffer_size;
}

void wc_flat_pipe_shader_state_from(const struct pipe_shader_state *in, struct wc_flat_pipe_shader_state *out)
{
    memset(out, 0, sizeof *out);
    out->stream_output.num_outputs = in->stream_output.num_outputs;
    for (unsigned i0 = 0; i0 < 4; i0++) {
        out->stream_output.stride[i0] = in->stream_output.stride[i0];
    }
    for (unsigned i0 = 0; i0 < 128; i0++) {
        out->stream_output.output[i0].register_index = in->stream_output.output[i0].register_index;
        out->stream_output.output[i0].start_component = in->stream_output.output[i0].start_component;
        out->stream_output.output[i0].num_components = in->stream_output.output[i0].num_components;
        out->stream_output.output[i0].output_buffer = in->stream_output.output[i0].output_buffer;
        out->stream_output.output[i0].dst_offset = in->stream_output.output[i0].dst_offset;
        out->stream_output.output[i0].stream = in->stream_output.output[i0].stream;
    }
    out->report_compile_error = in->report_compile_error;
}

void wc_flat_pipe_stencil_ref_from(const struct pipe_stencil_ref *in, struct wc_flat_pipe_stencil_ref *out)
{
    memset(out, 0, sizeof *out);
    for (unsigned i0 = 0; i0 < 2; i0++) {
        out->ref_value[i0] = in->ref_value[i0];
    }
}

void wc_flat_pipe_stencil_state_from(const struct pipe_stencil_state *in, struct wc_flat_pipe_stencil_state *out)
{
    memset(out, 0, sizeof *out);
    out->enabled = in->enabled;
    out->func = in->func;
    out->fail_op = in->fail_op;
    out->zpass_op = in->zpass_op;
    out->zfail_op = in->zfail_op;
    out->valuemask = in->valuemask;
    out->writemask = in->writemask;
}

void wc_flat_pipe_stream_output_from(const struct pipe_stream_output *in, struct wc_flat_pipe_stream_output *out)
{
    memset(out, 0, sizeof *out);
    out->register_index = in->register_index;
    out->start_component = in->start_component;
    out->num_components = in->num_components;
    out->output_buffer = in->output_buffer;
    out->dst_offset = in->dst_offset;
    out->stream = in->stream;
}

void wc_flat_pipe_stream_output_info_from(const struct pipe_stream_output_info *in, struct wc_flat_pipe_stream_output_info *out)
{
    memset(out, 0, sizeof *out);
    out->num_outputs = in->num_outputs;
    for (unsigned i0 = 0; i0 < 4; i0++) {
        out->stride[i0] = in->stride[i0];
    }
    for (unsigned i0 = 0; i0 < 128; i0++) {
        out->output[i0].register_index = in->output[i0].register_index;
        out->output[i0].start_component = in->output[i0].start_component;
        out->output[i0].num_components = in->output[i0].num_components;
        out->output[i0].output_buffer = in->output[i0].output_buffer;
        out->output[i0].dst_offset = in->output[i0].dst_offset;
        out->output[i0].stream = in->output[i0].stream;
    }
}

void wc_flat_pipe_surface_from(const struct pipe_surface *in, struct wc_flat_pipe_surface *out)
{
    memset(out, 0, sizeof *out);
    out->reference.count = in->reference.count;
    out->format = in->format;
    out->nr_samples = in->nr_samples;
    out->first_layer = in->first_layer;
    out->last_layer = in->last_layer;
    out->level = in->level;
    out->texture = (uintptr_t)in->texture;
}

void wc_flat_pipe_tex2d_from_buf_from(const struct pipe_tex2d_from_buf *in, struct wc_flat_pipe_tex2d_from_buf *out)
{
    memset(out, 0, sizeof *out);
    out->offset = in->offset;
    out->row_stride = in->row_stride;
    out->width = in->width;
    out->height = in->height;
}

void wc_flat_pipe_vertex_buffer_from(const struct pipe_vertex_buffer *in, struct wc_flat_pipe_vertex_buffer *out)
{
    memset(out, 0, sizeof *out);
    out->is_user_buffer = in->is_user_buffer;
    out->buffer_offset = in->buffer_offset;
    out->buffer.resource = (uintptr_t)in->buffer.resource;
}

void wc_flat_pipe_vertex_element_from(const struct pipe_vertex_element *in, struct wc_flat_pipe_vertex_element *out)
{
    memset(out, 0, sizeof *out);
    out->src_offset = in->src_offset;
    out->vertex_buffer_index = in->vertex_buffer_index;
    out->dual_slot = in->dual_slot;
    out->src_format = in->src_format;
    out->src_stride = in->src_stride;
    out->instance_divisor = in->instance_divisor;
}

void wc_flat_pipe_viewport_state_from(const struct pipe_viewport_state *in, struct wc_flat_pipe_viewport_state *out)
{
    memset(out, 0, sizeof *out);
    for (unsigned i0 = 0; i0 < 3; i0++) {
        out->scale[i0] = in->scale[i0];
    }
    for (unsigned i0 = 0; i0 < 3; i0++) {
        out->translate[i0] = in->translate[i0];
    }
    out->swizzle_x = in->swizzle_x;
    out->swizzle_y = in->swizzle_y;
    out->swizzle_z = in->swizzle_z;
    out->swizzle_w = in->swizzle_w;
}

size_t wc_flat_size(unsigned which)
{
    switch (which) {
        case 0: return sizeof(struct wc_flat_pipe_blend_color);
        case 1: return sizeof(struct wc_flat_pipe_blend_state);
        case 2: return sizeof(struct wc_flat_pipe_blit_info);
        case 3: return sizeof(struct wc_flat_pipe_blit_info__dst);
        case 4: return sizeof(struct wc_flat_pipe_blit_info__src);
        case 5: return sizeof(struct wc_flat_pipe_box);
        case 6: return sizeof(struct wc_flat_pipe_clip_state);
        case 7: return sizeof(struct wc_flat_pipe_compute_state);
        case 8: return sizeof(struct wc_flat_pipe_constant_buffer);
        case 9: return sizeof(struct wc_flat_pipe_depth_stencil_alpha_state);
        case 10: return sizeof(struct wc_flat_pipe_draw_indirect_info);
        case 11: return sizeof(struct wc_flat_pipe_draw_info);
        case 12: return sizeof(struct wc_flat_pipe_draw_info__index);
        case 13: return sizeof(struct wc_flat_pipe_draw_start_count_bias);
        case 14: return sizeof(struct wc_flat_pipe_framebuffer_state);
        case 15: return sizeof(struct wc_flat_pipe_grid_info);
        case 16: return sizeof(struct wc_flat_pipe_image_view);
        case 17: return sizeof(struct wc_flat_pipe_image_view__u);
        case 18: return sizeof(struct wc_flat_pipe_image_view__u__buf);
        case 19: return sizeof(struct wc_flat_pipe_image_view__u__tex);
        case 20: return sizeof(struct wc_flat_pipe_poly_stipple);
        case 21: return sizeof(struct wc_flat_pipe_rasterizer_state);
        case 22: return sizeof(struct wc_flat_pipe_reference);
        case 23: return sizeof(struct wc_flat_pipe_resource);
        case 24: return sizeof(struct wc_flat_pipe_rt_blend_state);
        case 25: return sizeof(struct wc_flat_pipe_sampler_state);
        case 26: return sizeof(struct wc_flat_pipe_sampler_view);
        case 27: return sizeof(struct wc_flat_pipe_sampler_view__u);
        case 28: return sizeof(struct wc_flat_pipe_sampler_view__u__buf);
        case 29: return sizeof(struct wc_flat_pipe_sampler_view__u__tex);
        case 30: return sizeof(struct wc_flat_pipe_scissor_state);
        case 31: return sizeof(struct wc_flat_pipe_shader_buffer);
        case 32: return sizeof(struct wc_flat_pipe_shader_state);
        case 33: return sizeof(struct wc_flat_pipe_stencil_ref);
        case 34: return sizeof(struct wc_flat_pipe_stencil_state);
        case 35: return sizeof(struct wc_flat_pipe_stream_output);
        case 36: return sizeof(struct wc_flat_pipe_stream_output_info);
        case 37: return sizeof(struct wc_flat_pipe_surface);
        case 38: return sizeof(struct wc_flat_pipe_tex2d_from_buf);
        case 39: return sizeof(struct wc_flat_pipe_vertex_buffer);
        case 40: return sizeof(struct wc_flat_pipe_vertex_buffer__buffer);
        case 41: return sizeof(struct wc_flat_pipe_vertex_element);
        case 42: return sizeof(struct wc_flat_pipe_viewport_state);
        default: return 0;
    }
}

/* Field `which` of FIELDS in flat.rs: `what` 0 its offset in the flat form, 1 its size there, 2 its offset in Mesa's
 * structure (from the structure holding it, for a nested one), 3 its size there; a bit-field's Mesa offset and size are
 * WC_BIT_FIELD (no offsetof); a field not listed, WC_NO_FIELD. */
#define WC_BIT_FIELD ((size_t)-1)
#define WC_NO_FIELD ((size_t)-2)
size_t wc_flat_layout(unsigned which, unsigned what)
{
    switch (which * 4 + what) {
        case 0: return offsetof(struct wc_flat_pipe_blend_color, color);
        case 1: return sizeof(((struct wc_flat_pipe_blend_color *)0)->color);
        case 2: return offsetof(struct pipe_blend_color, color);
        case 3: return sizeof(((struct pipe_blend_color *)0)->color);
        case 4: return offsetof(struct wc_flat_pipe_blend_state, independent_blend_enable);
        case 5: return sizeof(((struct wc_flat_pipe_blend_state *)0)->independent_blend_enable);
        case 6: case 7: return WC_BIT_FIELD;
        case 8: return offsetof(struct wc_flat_pipe_blend_state, logicop_enable);
        case 9: return sizeof(((struct wc_flat_pipe_blend_state *)0)->logicop_enable);
        case 10: case 11: return WC_BIT_FIELD;
        case 12: return offsetof(struct wc_flat_pipe_blend_state, logicop_func);
        case 13: return sizeof(((struct wc_flat_pipe_blend_state *)0)->logicop_func);
        case 14: case 15: return WC_BIT_FIELD;
        case 16: return offsetof(struct wc_flat_pipe_blend_state, dither);
        case 17: return sizeof(((struct wc_flat_pipe_blend_state *)0)->dither);
        case 18: case 19: return WC_BIT_FIELD;
        case 20: return offsetof(struct wc_flat_pipe_blend_state, alpha_to_coverage);
        case 21: return sizeof(((struct wc_flat_pipe_blend_state *)0)->alpha_to_coverage);
        case 22: case 23: return WC_BIT_FIELD;
        case 24: return offsetof(struct wc_flat_pipe_blend_state, alpha_to_coverage_dither);
        case 25: return sizeof(((struct wc_flat_pipe_blend_state *)0)->alpha_to_coverage_dither);
        case 26: case 27: return WC_BIT_FIELD;
        case 28: return offsetof(struct wc_flat_pipe_blend_state, alpha_to_one);
        case 29: return sizeof(((struct wc_flat_pipe_blend_state *)0)->alpha_to_one);
        case 30: case 31: return WC_BIT_FIELD;
        case 32: return offsetof(struct wc_flat_pipe_blend_state, max_rt);
        case 33: return sizeof(((struct wc_flat_pipe_blend_state *)0)->max_rt);
        case 34: case 35: return WC_BIT_FIELD;
        case 36: return offsetof(struct wc_flat_pipe_blend_state, advanced_blend_func);
        case 37: return sizeof(((struct wc_flat_pipe_blend_state *)0)->advanced_blend_func);
        case 38: case 39: return WC_BIT_FIELD;
        case 40: return offsetof(struct wc_flat_pipe_blend_state, blend_coherent);
        case 41: return sizeof(((struct wc_flat_pipe_blend_state *)0)->blend_coherent);
        case 42: case 43: return WC_BIT_FIELD;
        case 44: return offsetof(struct wc_flat_pipe_blend_state, is_yuv);
        case 45: return sizeof(((struct wc_flat_pipe_blend_state *)0)->is_yuv);
        case 46: case 47: return WC_BIT_FIELD;
        case 48: return offsetof(struct wc_flat_pipe_blend_state, rt);
        case 49: return sizeof(((struct wc_flat_pipe_blend_state *)0)->rt);
        case 50: return offsetof(struct pipe_blend_state, rt);
        case 51: return sizeof(((struct pipe_blend_state *)0)->rt);
        case 52: return offsetof(struct wc_flat_pipe_blit_info, dst);
        case 53: return sizeof(((struct wc_flat_pipe_blit_info *)0)->dst);
        case 54: return offsetof(struct pipe_blit_info, dst);
        case 55: return sizeof(((struct pipe_blit_info *)0)->dst);
        case 56: return offsetof(struct wc_flat_pipe_blit_info, src);
        case 57: return sizeof(((struct wc_flat_pipe_blit_info *)0)->src);
        case 58: return offsetof(struct pipe_blit_info, src);
        case 59: return sizeof(((struct pipe_blit_info *)0)->src);
        case 60: return offsetof(struct wc_flat_pipe_blit_info, mask);
        case 61: return sizeof(((struct wc_flat_pipe_blit_info *)0)->mask);
        case 62: return offsetof(struct pipe_blit_info, mask);
        case 63: return sizeof(((struct pipe_blit_info *)0)->mask);
        case 64: return offsetof(struct wc_flat_pipe_blit_info, filter);
        case 65: return sizeof(((struct wc_flat_pipe_blit_info *)0)->filter);
        case 66: return offsetof(struct pipe_blit_info, filter);
        case 67: return sizeof(((struct pipe_blit_info *)0)->filter);
        case 68: return offsetof(struct wc_flat_pipe_blit_info, dst_sample);
        case 69: return sizeof(((struct wc_flat_pipe_blit_info *)0)->dst_sample);
        case 70: return offsetof(struct pipe_blit_info, dst_sample);
        case 71: return sizeof(((struct pipe_blit_info *)0)->dst_sample);
        case 72: return offsetof(struct wc_flat_pipe_blit_info, sample0_only);
        case 73: return sizeof(((struct wc_flat_pipe_blit_info *)0)->sample0_only);
        case 74: return offsetof(struct pipe_blit_info, sample0_only);
        case 75: return sizeof(((struct pipe_blit_info *)0)->sample0_only);
        case 76: return offsetof(struct wc_flat_pipe_blit_info, scissor_enable);
        case 77: return sizeof(((struct wc_flat_pipe_blit_info *)0)->scissor_enable);
        case 78: return offsetof(struct pipe_blit_info, scissor_enable);
        case 79: return sizeof(((struct pipe_blit_info *)0)->scissor_enable);
        case 80: return offsetof(struct wc_flat_pipe_blit_info, scissor);
        case 81: return sizeof(((struct wc_flat_pipe_blit_info *)0)->scissor);
        case 82: return offsetof(struct pipe_blit_info, scissor);
        case 83: return sizeof(((struct pipe_blit_info *)0)->scissor);
        case 84: return offsetof(struct wc_flat_pipe_blit_info, swizzle_enable);
        case 85: return sizeof(((struct wc_flat_pipe_blit_info *)0)->swizzle_enable);
        case 86: return offsetof(struct pipe_blit_info, swizzle_enable);
        case 87: return sizeof(((struct pipe_blit_info *)0)->swizzle_enable);
        case 88: return offsetof(struct wc_flat_pipe_blit_info, swizzle);
        case 89: return sizeof(((struct wc_flat_pipe_blit_info *)0)->swizzle);
        case 90: return offsetof(struct pipe_blit_info, swizzle);
        case 91: return sizeof(((struct pipe_blit_info *)0)->swizzle);
        case 92: return offsetof(struct wc_flat_pipe_blit_info, window_rectangle_include);
        case 93: return sizeof(((struct wc_flat_pipe_blit_info *)0)->window_rectangle_include);
        case 94: return offsetof(struct pipe_blit_info, window_rectangle_include);
        case 95: return sizeof(((struct pipe_blit_info *)0)->window_rectangle_include);
        case 96: return offsetof(struct wc_flat_pipe_blit_info, num_window_rectangles);
        case 97: return sizeof(((struct wc_flat_pipe_blit_info *)0)->num_window_rectangles);
        case 98: return offsetof(struct pipe_blit_info, num_window_rectangles);
        case 99: return sizeof(((struct pipe_blit_info *)0)->num_window_rectangles);
        case 100: return offsetof(struct wc_flat_pipe_blit_info, window_rectangles);
        case 101: return sizeof(((struct wc_flat_pipe_blit_info *)0)->window_rectangles);
        case 102: return offsetof(struct pipe_blit_info, window_rectangles);
        case 103: return sizeof(((struct pipe_blit_info *)0)->window_rectangles);
        case 104: return offsetof(struct wc_flat_pipe_blit_info, render_condition_enable);
        case 105: return sizeof(((struct wc_flat_pipe_blit_info *)0)->render_condition_enable);
        case 106: return offsetof(struct pipe_blit_info, render_condition_enable);
        case 107: return sizeof(((struct pipe_blit_info *)0)->render_condition_enable);
        case 108: return offsetof(struct wc_flat_pipe_blit_info, alpha_blend);
        case 109: return sizeof(((struct wc_flat_pipe_blit_info *)0)->alpha_blend);
        case 110: return offsetof(struct pipe_blit_info, alpha_blend);
        case 111: return sizeof(((struct pipe_blit_info *)0)->alpha_blend);
        case 112: return offsetof(struct wc_flat_pipe_blit_info__dst, resource);
        case 113: return sizeof(((struct wc_flat_pipe_blit_info__dst *)0)->resource);
        case 114: return offsetof(struct pipe_blit_info, dst.resource) - offsetof(struct pipe_blit_info, dst);
        case 115: return sizeof(((struct pipe_blit_info *)0)->dst.resource);
        case 116: return offsetof(struct wc_flat_pipe_blit_info__dst, level);
        case 117: return sizeof(((struct wc_flat_pipe_blit_info__dst *)0)->level);
        case 118: return offsetof(struct pipe_blit_info, dst.level) - offsetof(struct pipe_blit_info, dst);
        case 119: return sizeof(((struct pipe_blit_info *)0)->dst.level);
        case 120: return offsetof(struct wc_flat_pipe_blit_info__dst, box);
        case 121: return sizeof(((struct wc_flat_pipe_blit_info__dst *)0)->box);
        case 122: return offsetof(struct pipe_blit_info, dst.box) - offsetof(struct pipe_blit_info, dst);
        case 123: return sizeof(((struct pipe_blit_info *)0)->dst.box);
        case 124: return offsetof(struct wc_flat_pipe_blit_info__dst, format);
        case 125: return sizeof(((struct wc_flat_pipe_blit_info__dst *)0)->format);
        case 126: return offsetof(struct pipe_blit_info, dst.format) - offsetof(struct pipe_blit_info, dst);
        case 127: return sizeof(((struct pipe_blit_info *)0)->dst.format);
        case 128: return offsetof(struct wc_flat_pipe_blit_info__src, resource);
        case 129: return sizeof(((struct wc_flat_pipe_blit_info__src *)0)->resource);
        case 130: return offsetof(struct pipe_blit_info, src.resource) - offsetof(struct pipe_blit_info, src);
        case 131: return sizeof(((struct pipe_blit_info *)0)->src.resource);
        case 132: return offsetof(struct wc_flat_pipe_blit_info__src, level);
        case 133: return sizeof(((struct wc_flat_pipe_blit_info__src *)0)->level);
        case 134: return offsetof(struct pipe_blit_info, src.level) - offsetof(struct pipe_blit_info, src);
        case 135: return sizeof(((struct pipe_blit_info *)0)->src.level);
        case 136: return offsetof(struct wc_flat_pipe_blit_info__src, box);
        case 137: return sizeof(((struct wc_flat_pipe_blit_info__src *)0)->box);
        case 138: return offsetof(struct pipe_blit_info, src.box) - offsetof(struct pipe_blit_info, src);
        case 139: return sizeof(((struct pipe_blit_info *)0)->src.box);
        case 140: return offsetof(struct wc_flat_pipe_blit_info__src, format);
        case 141: return sizeof(((struct wc_flat_pipe_blit_info__src *)0)->format);
        case 142: return offsetof(struct pipe_blit_info, src.format) - offsetof(struct pipe_blit_info, src);
        case 143: return sizeof(((struct pipe_blit_info *)0)->src.format);
        case 144: return offsetof(struct wc_flat_pipe_box, x);
        case 145: return sizeof(((struct wc_flat_pipe_box *)0)->x);
        case 146: return offsetof(struct pipe_box, x);
        case 147: return sizeof(((struct pipe_box *)0)->x);
        case 148: return offsetof(struct wc_flat_pipe_box, width);
        case 149: return sizeof(((struct wc_flat_pipe_box *)0)->width);
        case 150: return offsetof(struct pipe_box, width);
        case 151: return sizeof(((struct pipe_box *)0)->width);
        case 152: return offsetof(struct wc_flat_pipe_box, y);
        case 153: return sizeof(((struct wc_flat_pipe_box *)0)->y);
        case 154: return offsetof(struct pipe_box, y);
        case 155: return sizeof(((struct pipe_box *)0)->y);
        case 156: return offsetof(struct wc_flat_pipe_box, height);
        case 157: return sizeof(((struct wc_flat_pipe_box *)0)->height);
        case 158: return offsetof(struct pipe_box, height);
        case 159: return sizeof(((struct pipe_box *)0)->height);
        case 160: return offsetof(struct wc_flat_pipe_box, z);
        case 161: return sizeof(((struct wc_flat_pipe_box *)0)->z);
        case 162: return offsetof(struct pipe_box, z);
        case 163: return sizeof(((struct pipe_box *)0)->z);
        case 164: return offsetof(struct wc_flat_pipe_box, depth);
        case 165: return sizeof(((struct wc_flat_pipe_box *)0)->depth);
        case 166: return offsetof(struct pipe_box, depth);
        case 167: return sizeof(((struct pipe_box *)0)->depth);
        case 168: return offsetof(struct wc_flat_pipe_clip_state, ucp);
        case 169: return sizeof(((struct wc_flat_pipe_clip_state *)0)->ucp);
        case 170: return offsetof(struct pipe_clip_state, ucp);
        case 171: return sizeof(((struct pipe_clip_state *)0)->ucp);
        case 172: return offsetof(struct wc_flat_pipe_compute_state, static_shared_mem);
        case 173: return sizeof(((struct wc_flat_pipe_compute_state *)0)->static_shared_mem);
        case 174: return offsetof(struct pipe_compute_state, static_shared_mem);
        case 175: return sizeof(((struct pipe_compute_state *)0)->static_shared_mem);
        case 176: return offsetof(struct wc_flat_pipe_constant_buffer, buffer);
        case 177: return sizeof(((struct wc_flat_pipe_constant_buffer *)0)->buffer);
        case 178: return offsetof(struct pipe_constant_buffer, buffer);
        case 179: return sizeof(((struct pipe_constant_buffer *)0)->buffer);
        case 180: return offsetof(struct wc_flat_pipe_constant_buffer, buffer_offset);
        case 181: return sizeof(((struct wc_flat_pipe_constant_buffer *)0)->buffer_offset);
        case 182: return offsetof(struct pipe_constant_buffer, buffer_offset);
        case 183: return sizeof(((struct pipe_constant_buffer *)0)->buffer_offset);
        case 184: return offsetof(struct wc_flat_pipe_constant_buffer, buffer_size);
        case 185: return sizeof(((struct wc_flat_pipe_constant_buffer *)0)->buffer_size);
        case 186: return offsetof(struct pipe_constant_buffer, buffer_size);
        case 187: return sizeof(((struct pipe_constant_buffer *)0)->buffer_size);
        case 188: return offsetof(struct wc_flat_pipe_constant_buffer, user_buffer);
        case 189: return sizeof(((struct wc_flat_pipe_constant_buffer *)0)->user_buffer);
        case 190: return offsetof(struct pipe_constant_buffer, user_buffer);
        case 191: return sizeof(((struct pipe_constant_buffer *)0)->user_buffer);
        case 192: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, stencil);
        case 193: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->stencil);
        case 194: return offsetof(struct pipe_depth_stencil_alpha_state, stencil);
        case 195: return sizeof(((struct pipe_depth_stencil_alpha_state *)0)->stencil);
        case 196: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, alpha_enabled);
        case 197: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->alpha_enabled);
        case 198: case 199: return WC_BIT_FIELD;
        case 200: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, alpha_func);
        case 201: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->alpha_func);
        case 202: case 203: return WC_BIT_FIELD;
        case 204: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, depth_enabled);
        case 205: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->depth_enabled);
        case 206: case 207: return WC_BIT_FIELD;
        case 208: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, depth_writemask);
        case 209: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->depth_writemask);
        case 210: case 211: return WC_BIT_FIELD;
        case 212: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, depth_func);
        case 213: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->depth_func);
        case 214: case 215: return WC_BIT_FIELD;
        case 216: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, depth_bounds_test);
        case 217: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->depth_bounds_test);
        case 218: case 219: return WC_BIT_FIELD;
        case 220: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, alpha_ref_value);
        case 221: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->alpha_ref_value);
        case 222: return offsetof(struct pipe_depth_stencil_alpha_state, alpha_ref_value);
        case 223: return sizeof(((struct pipe_depth_stencil_alpha_state *)0)->alpha_ref_value);
        case 224: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, depth_bounds_min);
        case 225: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->depth_bounds_min);
        case 226: return offsetof(struct pipe_depth_stencil_alpha_state, depth_bounds_min);
        case 227: return sizeof(((struct pipe_depth_stencil_alpha_state *)0)->depth_bounds_min);
        case 228: return offsetof(struct wc_flat_pipe_depth_stencil_alpha_state, depth_bounds_max);
        case 229: return sizeof(((struct wc_flat_pipe_depth_stencil_alpha_state *)0)->depth_bounds_max);
        case 230: return offsetof(struct pipe_depth_stencil_alpha_state, depth_bounds_max);
        case 231: return sizeof(((struct pipe_depth_stencil_alpha_state *)0)->depth_bounds_max);
        case 232: return offsetof(struct wc_flat_pipe_draw_indirect_info, offset);
        case 233: return sizeof(((struct wc_flat_pipe_draw_indirect_info *)0)->offset);
        case 234: return offsetof(struct pipe_draw_indirect_info, offset);
        case 235: return sizeof(((struct pipe_draw_indirect_info *)0)->offset);
        case 236: return offsetof(struct wc_flat_pipe_draw_indirect_info, stride);
        case 237: return sizeof(((struct wc_flat_pipe_draw_indirect_info *)0)->stride);
        case 238: return offsetof(struct pipe_draw_indirect_info, stride);
        case 239: return sizeof(((struct pipe_draw_indirect_info *)0)->stride);
        case 240: return offsetof(struct wc_flat_pipe_draw_indirect_info, draw_count);
        case 241: return sizeof(((struct wc_flat_pipe_draw_indirect_info *)0)->draw_count);
        case 242: return offsetof(struct pipe_draw_indirect_info, draw_count);
        case 243: return sizeof(((struct pipe_draw_indirect_info *)0)->draw_count);
        case 244: return offsetof(struct wc_flat_pipe_draw_indirect_info, indirect_draw_count_offset);
        case 245: return sizeof(((struct wc_flat_pipe_draw_indirect_info *)0)->indirect_draw_count_offset);
        case 246: return offsetof(struct pipe_draw_indirect_info, indirect_draw_count_offset);
        case 247: return sizeof(((struct pipe_draw_indirect_info *)0)->indirect_draw_count_offset);
        case 248: return offsetof(struct wc_flat_pipe_draw_indirect_info, buffer);
        case 249: return sizeof(((struct wc_flat_pipe_draw_indirect_info *)0)->buffer);
        case 250: return offsetof(struct pipe_draw_indirect_info, buffer);
        case 251: return sizeof(((struct pipe_draw_indirect_info *)0)->buffer);
        case 252: return offsetof(struct wc_flat_pipe_draw_indirect_info, indirect_draw_count);
        case 253: return sizeof(((struct wc_flat_pipe_draw_indirect_info *)0)->indirect_draw_count);
        case 254: return offsetof(struct pipe_draw_indirect_info, indirect_draw_count);
        case 255: return sizeof(((struct pipe_draw_indirect_info *)0)->indirect_draw_count);
        case 256: return offsetof(struct wc_flat_pipe_draw_indirect_info, count_from_stream_output);
        case 257: return sizeof(((struct wc_flat_pipe_draw_indirect_info *)0)->count_from_stream_output);
        case 258: return offsetof(struct pipe_draw_indirect_info, count_from_stream_output);
        case 259: return sizeof(((struct pipe_draw_indirect_info *)0)->count_from_stream_output);
        case 260: return offsetof(struct wc_flat_pipe_draw_info, mode);
        case 261: return sizeof(((struct wc_flat_pipe_draw_info *)0)->mode);
        case 262: return offsetof(struct pipe_draw_info, mode);
        case 263: return sizeof(((struct pipe_draw_info *)0)->mode);
        case 264: return offsetof(struct wc_flat_pipe_draw_info, index_size);
        case 265: return sizeof(((struct wc_flat_pipe_draw_info *)0)->index_size);
        case 266: return offsetof(struct pipe_draw_info, index_size);
        case 267: return sizeof(((struct pipe_draw_info *)0)->index_size);
        case 268: return offsetof(struct wc_flat_pipe_draw_info, primitive_restart);
        case 269: return sizeof(((struct wc_flat_pipe_draw_info *)0)->primitive_restart);
        case 270: case 271: return WC_BIT_FIELD;
        case 272: return offsetof(struct wc_flat_pipe_draw_info, has_user_indices);
        case 273: return sizeof(((struct wc_flat_pipe_draw_info *)0)->has_user_indices);
        case 274: case 275: return WC_BIT_FIELD;
        case 276: return offsetof(struct wc_flat_pipe_draw_info, index_bounds_valid);
        case 277: return sizeof(((struct wc_flat_pipe_draw_info *)0)->index_bounds_valid);
        case 278: case 279: return WC_BIT_FIELD;
        case 280: return offsetof(struct wc_flat_pipe_draw_info, increment_draw_id);
        case 281: return sizeof(((struct wc_flat_pipe_draw_info *)0)->increment_draw_id);
        case 282: case 283: return WC_BIT_FIELD;
        case 284: return offsetof(struct wc_flat_pipe_draw_info, index_bias_varies);
        case 285: return sizeof(((struct wc_flat_pipe_draw_info *)0)->index_bias_varies);
        case 286: case 287: return WC_BIT_FIELD;
        case 288: return offsetof(struct wc_flat_pipe_draw_info, was_line_loop);
        case 289: return sizeof(((struct wc_flat_pipe_draw_info *)0)->was_line_loop);
        case 290: case 291: return WC_BIT_FIELD;
        case 292: return offsetof(struct wc_flat_pipe_draw_info, _pad);
        case 293: return sizeof(((struct wc_flat_pipe_draw_info *)0)->_pad);
        case 294: case 295: return WC_BIT_FIELD;
        case 296: return offsetof(struct wc_flat_pipe_draw_info, start_instance);
        case 297: return sizeof(((struct wc_flat_pipe_draw_info *)0)->start_instance);
        case 298: return offsetof(struct pipe_draw_info, start_instance);
        case 299: return sizeof(((struct pipe_draw_info *)0)->start_instance);
        case 300: return offsetof(struct wc_flat_pipe_draw_info, instance_count);
        case 301: return sizeof(((struct wc_flat_pipe_draw_info *)0)->instance_count);
        case 302: return offsetof(struct pipe_draw_info, instance_count);
        case 303: return sizeof(((struct pipe_draw_info *)0)->instance_count);
        case 304: return offsetof(struct wc_flat_pipe_draw_info, restart_index);
        case 305: return sizeof(((struct wc_flat_pipe_draw_info *)0)->restart_index);
        case 306: return offsetof(struct pipe_draw_info, restart_index);
        case 307: return sizeof(((struct pipe_draw_info *)0)->restart_index);
        case 308: return offsetof(struct wc_flat_pipe_draw_info, index);
        case 309: return sizeof(((struct wc_flat_pipe_draw_info *)0)->index);
        case 310: return offsetof(struct pipe_draw_info, index);
        case 311: return sizeof(((struct pipe_draw_info *)0)->index);
        case 312: return offsetof(struct wc_flat_pipe_draw_info, min_index);
        case 313: return sizeof(((struct wc_flat_pipe_draw_info *)0)->min_index);
        case 314: return offsetof(struct pipe_draw_info, min_index);
        case 315: return sizeof(((struct pipe_draw_info *)0)->min_index);
        case 316: return offsetof(struct wc_flat_pipe_draw_info, max_index);
        case 317: return sizeof(((struct wc_flat_pipe_draw_info *)0)->max_index);
        case 318: return offsetof(struct pipe_draw_info, max_index);
        case 319: return sizeof(((struct pipe_draw_info *)0)->max_index);
        case 320: return offsetof(struct wc_flat_pipe_draw_info__index, resource);
        case 321: return sizeof(((struct wc_flat_pipe_draw_info__index *)0)->resource);
        case 322: return offsetof(struct pipe_draw_info, index.resource) - offsetof(struct pipe_draw_info, index);
        case 323: return sizeof(((struct pipe_draw_info *)0)->index.resource);
        case 324: return offsetof(struct wc_flat_pipe_draw_info__index, user);
        case 325: return sizeof(((struct wc_flat_pipe_draw_info__index *)0)->user);
        case 326: return offsetof(struct pipe_draw_info, index.user) - offsetof(struct pipe_draw_info, index);
        case 327: return sizeof(((struct pipe_draw_info *)0)->index.user);
        case 328: return offsetof(struct wc_flat_pipe_draw_start_count_bias, start);
        case 329: return sizeof(((struct wc_flat_pipe_draw_start_count_bias *)0)->start);
        case 330: return offsetof(struct pipe_draw_start_count_bias, start);
        case 331: return sizeof(((struct pipe_draw_start_count_bias *)0)->start);
        case 332: return offsetof(struct wc_flat_pipe_draw_start_count_bias, count);
        case 333: return sizeof(((struct wc_flat_pipe_draw_start_count_bias *)0)->count);
        case 334: return offsetof(struct pipe_draw_start_count_bias, count);
        case 335: return sizeof(((struct pipe_draw_start_count_bias *)0)->count);
        case 336: return offsetof(struct wc_flat_pipe_draw_start_count_bias, index_bias);
        case 337: return sizeof(((struct wc_flat_pipe_draw_start_count_bias *)0)->index_bias);
        case 338: return offsetof(struct pipe_draw_start_count_bias, index_bias);
        case 339: return sizeof(((struct pipe_draw_start_count_bias *)0)->index_bias);
        case 340: return offsetof(struct wc_flat_pipe_framebuffer_state, width);
        case 341: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->width);
        case 342: return offsetof(struct pipe_framebuffer_state, width);
        case 343: return sizeof(((struct pipe_framebuffer_state *)0)->width);
        case 344: return offsetof(struct wc_flat_pipe_framebuffer_state, height);
        case 345: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->height);
        case 346: return offsetof(struct pipe_framebuffer_state, height);
        case 347: return sizeof(((struct pipe_framebuffer_state *)0)->height);
        case 348: return offsetof(struct wc_flat_pipe_framebuffer_state, layers);
        case 349: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->layers);
        case 350: return offsetof(struct pipe_framebuffer_state, layers);
        case 351: return sizeof(((struct pipe_framebuffer_state *)0)->layers);
        case 352: return offsetof(struct wc_flat_pipe_framebuffer_state, samples);
        case 353: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->samples);
        case 354: return offsetof(struct pipe_framebuffer_state, samples);
        case 355: return sizeof(((struct pipe_framebuffer_state *)0)->samples);
        case 356: return offsetof(struct wc_flat_pipe_framebuffer_state, nr_cbufs);
        case 357: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->nr_cbufs);
        case 358: return offsetof(struct pipe_framebuffer_state, nr_cbufs);
        case 359: return sizeof(((struct pipe_framebuffer_state *)0)->nr_cbufs);
        case 360: return offsetof(struct wc_flat_pipe_framebuffer_state, pls_enabled);
        case 361: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->pls_enabled);
        case 362: case 363: return WC_BIT_FIELD;
        case 364: return offsetof(struct wc_flat_pipe_framebuffer_state, downscale_cbufs);
        case 365: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->downscale_cbufs);
        case 366: case 367: return WC_BIT_FIELD;
        case 368: return offsetof(struct wc_flat_pipe_framebuffer_state, _pad);
        case 369: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->_pad);
        case 370: case 371: return WC_BIT_FIELD;
        case 372: return offsetof(struct wc_flat_pipe_framebuffer_state, viewmask);
        case 373: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->viewmask);
        case 374: return offsetof(struct pipe_framebuffer_state, viewmask);
        case 375: return sizeof(((struct pipe_framebuffer_state *)0)->viewmask);
        case 376: return offsetof(struct wc_flat_pipe_framebuffer_state, cbufs);
        case 377: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->cbufs);
        case 378: return offsetof(struct pipe_framebuffer_state, cbufs);
        case 379: return sizeof(((struct pipe_framebuffer_state *)0)->cbufs);
        case 380: return offsetof(struct wc_flat_pipe_framebuffer_state, zsbuf);
        case 381: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->zsbuf);
        case 382: return offsetof(struct pipe_framebuffer_state, zsbuf);
        case 383: return sizeof(((struct pipe_framebuffer_state *)0)->zsbuf);
        case 384: return offsetof(struct wc_flat_pipe_framebuffer_state, resolve);
        case 385: return sizeof(((struct wc_flat_pipe_framebuffer_state *)0)->resolve);
        case 386: return offsetof(struct pipe_framebuffer_state, resolve);
        case 387: return sizeof(((struct pipe_framebuffer_state *)0)->resolve);
        case 388: return offsetof(struct wc_flat_pipe_grid_info, variable_shared_mem);
        case 389: return sizeof(((struct wc_flat_pipe_grid_info *)0)->variable_shared_mem);
        case 390: return offsetof(struct pipe_grid_info, variable_shared_mem);
        case 391: return sizeof(((struct pipe_grid_info *)0)->variable_shared_mem);
        case 392: return offsetof(struct wc_flat_pipe_grid_info, work_dim);
        case 393: return sizeof(((struct wc_flat_pipe_grid_info *)0)->work_dim);
        case 394: return offsetof(struct pipe_grid_info, work_dim);
        case 395: return sizeof(((struct pipe_grid_info *)0)->work_dim);
        case 396: return offsetof(struct wc_flat_pipe_grid_info, block);
        case 397: return sizeof(((struct wc_flat_pipe_grid_info *)0)->block);
        case 398: return offsetof(struct pipe_grid_info, block);
        case 399: return sizeof(((struct pipe_grid_info *)0)->block);
        case 400: return offsetof(struct wc_flat_pipe_grid_info, last_block);
        case 401: return sizeof(((struct wc_flat_pipe_grid_info *)0)->last_block);
        case 402: return offsetof(struct pipe_grid_info, last_block);
        case 403: return sizeof(((struct pipe_grid_info *)0)->last_block);
        case 404: return offsetof(struct wc_flat_pipe_grid_info, grid);
        case 405: return sizeof(((struct wc_flat_pipe_grid_info *)0)->grid);
        case 406: return offsetof(struct pipe_grid_info, grid);
        case 407: return sizeof(((struct pipe_grid_info *)0)->grid);
        case 408: return offsetof(struct wc_flat_pipe_grid_info, grid_base);
        case 409: return sizeof(((struct wc_flat_pipe_grid_info *)0)->grid_base);
        case 410: return offsetof(struct pipe_grid_info, grid_base);
        case 411: return sizeof(((struct pipe_grid_info *)0)->grid_base);
        case 412: return offsetof(struct wc_flat_pipe_grid_info, indirect);
        case 413: return sizeof(((struct wc_flat_pipe_grid_info *)0)->indirect);
        case 414: return offsetof(struct pipe_grid_info, indirect);
        case 415: return sizeof(((struct pipe_grid_info *)0)->indirect);
        case 416: return offsetof(struct wc_flat_pipe_grid_info, indirect_offset);
        case 417: return sizeof(((struct wc_flat_pipe_grid_info *)0)->indirect_offset);
        case 418: return offsetof(struct pipe_grid_info, indirect_offset);
        case 419: return sizeof(((struct pipe_grid_info *)0)->indirect_offset);
        case 420: return offsetof(struct wc_flat_pipe_grid_info, indirect_stride);
        case 421: return sizeof(((struct wc_flat_pipe_grid_info *)0)->indirect_stride);
        case 422: return offsetof(struct pipe_grid_info, indirect_stride);
        case 423: return sizeof(((struct pipe_grid_info *)0)->indirect_stride);
        case 424: return offsetof(struct wc_flat_pipe_grid_info, draw_count);
        case 425: return sizeof(((struct wc_flat_pipe_grid_info *)0)->draw_count);
        case 426: return offsetof(struct pipe_grid_info, draw_count);
        case 427: return sizeof(((struct pipe_grid_info *)0)->draw_count);
        case 428: return offsetof(struct wc_flat_pipe_grid_info, indirect_draw_count_offset);
        case 429: return sizeof(((struct wc_flat_pipe_grid_info *)0)->indirect_draw_count_offset);
        case 430: return offsetof(struct pipe_grid_info, indirect_draw_count_offset);
        case 431: return sizeof(((struct pipe_grid_info *)0)->indirect_draw_count_offset);
        case 432: return offsetof(struct wc_flat_pipe_grid_info, indirect_draw_count);
        case 433: return sizeof(((struct wc_flat_pipe_grid_info *)0)->indirect_draw_count);
        case 434: return offsetof(struct pipe_grid_info, indirect_draw_count);
        case 435: return sizeof(((struct pipe_grid_info *)0)->indirect_draw_count);
        case 436: return offsetof(struct wc_flat_pipe_image_view, resource);
        case 437: return sizeof(((struct wc_flat_pipe_image_view *)0)->resource);
        case 438: return offsetof(struct pipe_image_view, resource);
        case 439: return sizeof(((struct pipe_image_view *)0)->resource);
        case 440: return offsetof(struct wc_flat_pipe_image_view, format);
        case 441: return sizeof(((struct wc_flat_pipe_image_view *)0)->format);
        case 442: return offsetof(struct pipe_image_view, format);
        case 443: return sizeof(((struct pipe_image_view *)0)->format);
        case 444: return offsetof(struct wc_flat_pipe_image_view, access);
        case 445: return sizeof(((struct wc_flat_pipe_image_view *)0)->access);
        case 446: return offsetof(struct pipe_image_view, access);
        case 447: return sizeof(((struct pipe_image_view *)0)->access);
        case 448: return offsetof(struct wc_flat_pipe_image_view, shader_access);
        case 449: return sizeof(((struct wc_flat_pipe_image_view *)0)->shader_access);
        case 450: return offsetof(struct pipe_image_view, shader_access);
        case 451: return sizeof(((struct pipe_image_view *)0)->shader_access);
        case 452: return offsetof(struct wc_flat_pipe_image_view, u);
        case 453: return sizeof(((struct wc_flat_pipe_image_view *)0)->u);
        case 454: return offsetof(struct pipe_image_view, u);
        case 455: return sizeof(((struct pipe_image_view *)0)->u);
        case 456: return offsetof(struct wc_flat_pipe_image_view__u, tex);
        case 457: return sizeof(((struct wc_flat_pipe_image_view__u *)0)->tex);
        case 458: return offsetof(struct pipe_image_view, u.tex) - offsetof(struct pipe_image_view, u);
        case 459: return sizeof(((struct pipe_image_view *)0)->u.tex);
        case 460: return offsetof(struct wc_flat_pipe_image_view__u, buf);
        case 461: return sizeof(((struct wc_flat_pipe_image_view__u *)0)->buf);
        case 462: return offsetof(struct pipe_image_view, u.buf) - offsetof(struct pipe_image_view, u);
        case 463: return sizeof(((struct pipe_image_view *)0)->u.buf);
        case 464: return offsetof(struct wc_flat_pipe_image_view__u, tex2d_from_buf);
        case 465: return sizeof(((struct wc_flat_pipe_image_view__u *)0)->tex2d_from_buf);
        case 466: return offsetof(struct pipe_image_view, u.tex2d_from_buf) - offsetof(struct pipe_image_view, u);
        case 467: return sizeof(((struct pipe_image_view *)0)->u.tex2d_from_buf);
        case 468: return offsetof(struct wc_flat_pipe_image_view__u__buf, offset);
        case 469: return sizeof(((struct wc_flat_pipe_image_view__u__buf *)0)->offset);
        case 470: return offsetof(struct pipe_image_view, u.buf.offset) - offsetof(struct pipe_image_view, u.buf);
        case 471: return sizeof(((struct pipe_image_view *)0)->u.buf.offset);
        case 472: return offsetof(struct wc_flat_pipe_image_view__u__buf, size);
        case 473: return sizeof(((struct wc_flat_pipe_image_view__u__buf *)0)->size);
        case 474: return offsetof(struct pipe_image_view, u.buf.size) - offsetof(struct pipe_image_view, u.buf);
        case 475: return sizeof(((struct pipe_image_view *)0)->u.buf.size);
        case 476: return offsetof(struct wc_flat_pipe_image_view__u__tex, first_layer);
        case 477: return sizeof(((struct wc_flat_pipe_image_view__u__tex *)0)->first_layer);
        case 478: case 479: return WC_BIT_FIELD;
        case 480: return offsetof(struct wc_flat_pipe_image_view__u__tex, last_layer);
        case 481: return sizeof(((struct wc_flat_pipe_image_view__u__tex *)0)->last_layer);
        case 482: case 483: return WC_BIT_FIELD;
        case 484: return offsetof(struct wc_flat_pipe_image_view__u__tex, level);
        case 485: return sizeof(((struct wc_flat_pipe_image_view__u__tex *)0)->level);
        case 486: case 487: return WC_BIT_FIELD;
        case 488: return offsetof(struct wc_flat_pipe_image_view__u__tex, single_layer_view);
        case 489: return sizeof(((struct wc_flat_pipe_image_view__u__tex *)0)->single_layer_view);
        case 490: return offsetof(struct pipe_image_view, u.tex.single_layer_view) - offsetof(struct pipe_image_view, u.tex);
        case 491: return sizeof(((struct pipe_image_view *)0)->u.tex.single_layer_view);
        case 492: return offsetof(struct wc_flat_pipe_image_view__u__tex, is_2d_view_of_3d);
        case 493: return sizeof(((struct wc_flat_pipe_image_view__u__tex *)0)->is_2d_view_of_3d);
        case 494: return offsetof(struct pipe_image_view, u.tex.is_2d_view_of_3d) - offsetof(struct pipe_image_view, u.tex);
        case 495: return sizeof(((struct pipe_image_view *)0)->u.tex.is_2d_view_of_3d);
        case 496: return offsetof(struct wc_flat_pipe_poly_stipple, stipple);
        case 497: return sizeof(((struct wc_flat_pipe_poly_stipple *)0)->stipple);
        case 498: return offsetof(struct pipe_poly_stipple, stipple);
        case 499: return sizeof(((struct pipe_poly_stipple *)0)->stipple);
        case 500: return offsetof(struct wc_flat_pipe_rasterizer_state, flatshade);
        case 501: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->flatshade);
        case 502: case 503: return WC_BIT_FIELD;
        case 504: return offsetof(struct wc_flat_pipe_rasterizer_state, light_twoside);
        case 505: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->light_twoside);
        case 506: case 507: return WC_BIT_FIELD;
        case 508: return offsetof(struct wc_flat_pipe_rasterizer_state, clamp_vertex_color);
        case 509: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->clamp_vertex_color);
        case 510: case 511: return WC_BIT_FIELD;
        case 512: return offsetof(struct wc_flat_pipe_rasterizer_state, clamp_fragment_color);
        case 513: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->clamp_fragment_color);
        case 514: case 515: return WC_BIT_FIELD;
        case 516: return offsetof(struct wc_flat_pipe_rasterizer_state, front_ccw);
        case 517: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->front_ccw);
        case 518: case 519: return WC_BIT_FIELD;
        case 520: return offsetof(struct wc_flat_pipe_rasterizer_state, cull_face);
        case 521: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->cull_face);
        case 522: case 523: return WC_BIT_FIELD;
        case 524: return offsetof(struct wc_flat_pipe_rasterizer_state, fill_front);
        case 525: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->fill_front);
        case 526: case 527: return WC_BIT_FIELD;
        case 528: return offsetof(struct wc_flat_pipe_rasterizer_state, fill_back);
        case 529: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->fill_back);
        case 530: case 531: return WC_BIT_FIELD;
        case 532: return offsetof(struct wc_flat_pipe_rasterizer_state, offset_point);
        case 533: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->offset_point);
        case 534: case 535: return WC_BIT_FIELD;
        case 536: return offsetof(struct wc_flat_pipe_rasterizer_state, offset_line);
        case 537: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->offset_line);
        case 538: case 539: return WC_BIT_FIELD;
        case 540: return offsetof(struct wc_flat_pipe_rasterizer_state, offset_tri);
        case 541: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->offset_tri);
        case 542: case 543: return WC_BIT_FIELD;
        case 544: return offsetof(struct wc_flat_pipe_rasterizer_state, scissor);
        case 545: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->scissor);
        case 546: case 547: return WC_BIT_FIELD;
        case 548: return offsetof(struct wc_flat_pipe_rasterizer_state, poly_smooth);
        case 549: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->poly_smooth);
        case 550: case 551: return WC_BIT_FIELD;
        case 552: return offsetof(struct wc_flat_pipe_rasterizer_state, poly_stipple_enable);
        case 553: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->poly_stipple_enable);
        case 554: case 555: return WC_BIT_FIELD;
        case 556: return offsetof(struct wc_flat_pipe_rasterizer_state, point_smooth);
        case 557: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->point_smooth);
        case 558: case 559: return WC_BIT_FIELD;
        case 560: return offsetof(struct wc_flat_pipe_rasterizer_state, sprite_coord_mode);
        case 561: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->sprite_coord_mode);
        case 562: case 563: return WC_BIT_FIELD;
        case 564: return offsetof(struct wc_flat_pipe_rasterizer_state, point_quad_rasterization);
        case 565: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->point_quad_rasterization);
        case 566: case 567: return WC_BIT_FIELD;
        case 568: return offsetof(struct wc_flat_pipe_rasterizer_state, point_line_tri_clip);
        case 569: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->point_line_tri_clip);
        case 570: case 571: return WC_BIT_FIELD;
        case 572: return offsetof(struct wc_flat_pipe_rasterizer_state, point_size_per_vertex);
        case 573: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->point_size_per_vertex);
        case 574: case 575: return WC_BIT_FIELD;
        case 576: return offsetof(struct wc_flat_pipe_rasterizer_state, multisample);
        case 577: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->multisample);
        case 578: case 579: return WC_BIT_FIELD;
        case 580: return offsetof(struct wc_flat_pipe_rasterizer_state, no_ms_sample_mask_out);
        case 581: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->no_ms_sample_mask_out);
        case 582: case 583: return WC_BIT_FIELD;
        case 584: return offsetof(struct wc_flat_pipe_rasterizer_state, force_persample_interp);
        case 585: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->force_persample_interp);
        case 586: case 587: return WC_BIT_FIELD;
        case 588: return offsetof(struct wc_flat_pipe_rasterizer_state, line_smooth);
        case 589: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->line_smooth);
        case 590: case 591: return WC_BIT_FIELD;
        case 592: return offsetof(struct wc_flat_pipe_rasterizer_state, line_stipple_enable);
        case 593: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->line_stipple_enable);
        case 594: case 595: return WC_BIT_FIELD;
        case 596: return offsetof(struct wc_flat_pipe_rasterizer_state, line_last_pixel);
        case 597: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->line_last_pixel);
        case 598: case 599: return WC_BIT_FIELD;
        case 600: return offsetof(struct wc_flat_pipe_rasterizer_state, line_rectangular);
        case 601: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->line_rectangular);
        case 602: case 603: return WC_BIT_FIELD;
        case 604: return offsetof(struct wc_flat_pipe_rasterizer_state, conservative_raster_mode);
        case 605: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->conservative_raster_mode);
        case 606: case 607: return WC_BIT_FIELD;
        case 608: return offsetof(struct wc_flat_pipe_rasterizer_state, flatshade_first);
        case 609: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->flatshade_first);
        case 610: case 611: return WC_BIT_FIELD;
        case 612: return offsetof(struct wc_flat_pipe_rasterizer_state, half_pixel_center);
        case 613: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->half_pixel_center);
        case 614: case 615: return WC_BIT_FIELD;
        case 616: return offsetof(struct wc_flat_pipe_rasterizer_state, bottom_edge_rule);
        case 617: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->bottom_edge_rule);
        case 618: case 619: return WC_BIT_FIELD;
        case 620: return offsetof(struct wc_flat_pipe_rasterizer_state, subpixel_precision_x);
        case 621: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->subpixel_precision_x);
        case 622: case 623: return WC_BIT_FIELD;
        case 624: return offsetof(struct wc_flat_pipe_rasterizer_state, subpixel_precision_y);
        case 625: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->subpixel_precision_y);
        case 626: case 627: return WC_BIT_FIELD;
        case 628: return offsetof(struct wc_flat_pipe_rasterizer_state, rasterizer_discard);
        case 629: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->rasterizer_discard);
        case 630: case 631: return WC_BIT_FIELD;
        case 632: return offsetof(struct wc_flat_pipe_rasterizer_state, rasterization_stream);
        case 633: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->rasterization_stream);
        case 634: case 635: return WC_BIT_FIELD;
        case 636: return offsetof(struct wc_flat_pipe_rasterizer_state, tile_raster_order_fixed);
        case 637: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->tile_raster_order_fixed);
        case 638: case 639: return WC_BIT_FIELD;
        case 640: return offsetof(struct wc_flat_pipe_rasterizer_state, tile_raster_order_increasing_x);
        case 641: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->tile_raster_order_increasing_x);
        case 642: case 643: return WC_BIT_FIELD;
        case 644: return offsetof(struct wc_flat_pipe_rasterizer_state, tile_raster_order_increasing_y);
        case 645: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->tile_raster_order_increasing_y);
        case 646: case 647: return WC_BIT_FIELD;
        case 648: return offsetof(struct wc_flat_pipe_rasterizer_state, depth_clip_near);
        case 649: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->depth_clip_near);
        case 650: case 651: return WC_BIT_FIELD;
        case 652: return offsetof(struct wc_flat_pipe_rasterizer_state, depth_clip_far);
        case 653: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->depth_clip_far);
        case 654: case 655: return WC_BIT_FIELD;
        case 656: return offsetof(struct wc_flat_pipe_rasterizer_state, depth_clamp);
        case 657: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->depth_clamp);
        case 658: case 659: return WC_BIT_FIELD;
        case 660: return offsetof(struct wc_flat_pipe_rasterizer_state, clip_halfz);
        case 661: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->clip_halfz);
        case 662: case 663: return WC_BIT_FIELD;
        case 664: return offsetof(struct wc_flat_pipe_rasterizer_state, offset_units_unscaled);
        case 665: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->offset_units_unscaled);
        case 666: case 667: return WC_BIT_FIELD;
        case 668: return offsetof(struct wc_flat_pipe_rasterizer_state, unclamped_fragment_depth_values);
        case 669: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->unclamped_fragment_depth_values);
        case 670: case 671: return WC_BIT_FIELD;
        case 672: return offsetof(struct wc_flat_pipe_rasterizer_state, clip_plane_enable);
        case 673: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->clip_plane_enable);
        case 674: case 675: return WC_BIT_FIELD;
        case 676: return offsetof(struct wc_flat_pipe_rasterizer_state, representative_fragment_test);
        case 677: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->representative_fragment_test);
        case 678: case 679: return WC_BIT_FIELD;
        case 680: return offsetof(struct wc_flat_pipe_rasterizer_state, line_stipple_factor);
        case 681: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->line_stipple_factor);
        case 682: case 683: return WC_BIT_FIELD;
        case 684: return offsetof(struct wc_flat_pipe_rasterizer_state, line_stipple_pattern);
        case 685: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->line_stipple_pattern);
        case 686: case 687: return WC_BIT_FIELD;
        case 688: return offsetof(struct wc_flat_pipe_rasterizer_state, sprite_coord_enable);
        case 689: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->sprite_coord_enable);
        case 690: return offsetof(struct pipe_rasterizer_state, sprite_coord_enable);
        case 691: return sizeof(((struct pipe_rasterizer_state *)0)->sprite_coord_enable);
        case 692: return offsetof(struct wc_flat_pipe_rasterizer_state, line_width);
        case 693: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->line_width);
        case 694: return offsetof(struct pipe_rasterizer_state, line_width);
        case 695: return sizeof(((struct pipe_rasterizer_state *)0)->line_width);
        case 696: return offsetof(struct wc_flat_pipe_rasterizer_state, point_size);
        case 697: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->point_size);
        case 698: return offsetof(struct pipe_rasterizer_state, point_size);
        case 699: return sizeof(((struct pipe_rasterizer_state *)0)->point_size);
        case 700: return offsetof(struct wc_flat_pipe_rasterizer_state, offset_units);
        case 701: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->offset_units);
        case 702: return offsetof(struct pipe_rasterizer_state, offset_units);
        case 703: return sizeof(((struct pipe_rasterizer_state *)0)->offset_units);
        case 704: return offsetof(struct wc_flat_pipe_rasterizer_state, offset_scale);
        case 705: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->offset_scale);
        case 706: return offsetof(struct pipe_rasterizer_state, offset_scale);
        case 707: return sizeof(((struct pipe_rasterizer_state *)0)->offset_scale);
        case 708: return offsetof(struct wc_flat_pipe_rasterizer_state, offset_clamp);
        case 709: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->offset_clamp);
        case 710: return offsetof(struct pipe_rasterizer_state, offset_clamp);
        case 711: return sizeof(((struct pipe_rasterizer_state *)0)->offset_clamp);
        case 712: return offsetof(struct wc_flat_pipe_rasterizer_state, conservative_raster_dilate);
        case 713: return sizeof(((struct wc_flat_pipe_rasterizer_state *)0)->conservative_raster_dilate);
        case 714: return offsetof(struct pipe_rasterizer_state, conservative_raster_dilate);
        case 715: return sizeof(((struct pipe_rasterizer_state *)0)->conservative_raster_dilate);
        case 716: return offsetof(struct wc_flat_pipe_reference, count);
        case 717: return sizeof(((struct wc_flat_pipe_reference *)0)->count);
        case 718: return offsetof(struct pipe_reference, count);
        case 719: return sizeof(((struct pipe_reference *)0)->count);
        case 720: return offsetof(struct wc_flat_pipe_resource, width0);
        case 721: return sizeof(((struct wc_flat_pipe_resource *)0)->width0);
        case 722: return offsetof(struct pipe_resource, width0);
        case 723: return sizeof(((struct pipe_resource *)0)->width0);
        case 724: return offsetof(struct wc_flat_pipe_resource, height0);
        case 725: return sizeof(((struct wc_flat_pipe_resource *)0)->height0);
        case 726: return offsetof(struct pipe_resource, height0);
        case 727: return sizeof(((struct pipe_resource *)0)->height0);
        case 728: return offsetof(struct wc_flat_pipe_resource, depth0);
        case 729: return sizeof(((struct wc_flat_pipe_resource *)0)->depth0);
        case 730: return offsetof(struct pipe_resource, depth0);
        case 731: return sizeof(((struct pipe_resource *)0)->depth0);
        case 732: return offsetof(struct wc_flat_pipe_resource, array_size);
        case 733: return sizeof(((struct wc_flat_pipe_resource *)0)->array_size);
        case 734: return offsetof(struct pipe_resource, array_size);
        case 735: return sizeof(((struct pipe_resource *)0)->array_size);
        case 736: return offsetof(struct wc_flat_pipe_resource, format);
        case 737: return sizeof(((struct wc_flat_pipe_resource *)0)->format);
        case 738: case 739: return WC_BIT_FIELD;
        case 740: return offsetof(struct wc_flat_pipe_resource, target);
        case 741: return sizeof(((struct wc_flat_pipe_resource *)0)->target);
        case 742: case 743: return WC_BIT_FIELD;
        case 744: return offsetof(struct wc_flat_pipe_resource, last_level);
        case 745: return sizeof(((struct wc_flat_pipe_resource *)0)->last_level);
        case 746: return offsetof(struct pipe_resource, last_level);
        case 747: return sizeof(((struct pipe_resource *)0)->last_level);
        case 748: return offsetof(struct wc_flat_pipe_resource, nr_samples);
        case 749: return sizeof(((struct wc_flat_pipe_resource *)0)->nr_samples);
        case 750: return offsetof(struct pipe_resource, nr_samples);
        case 751: return sizeof(((struct pipe_resource *)0)->nr_samples);
        case 752: return offsetof(struct wc_flat_pipe_resource, nr_storage_samples);
        case 753: return sizeof(((struct wc_flat_pipe_resource *)0)->nr_storage_samples);
        case 754: return offsetof(struct pipe_resource, nr_storage_samples);
        case 755: return sizeof(((struct pipe_resource *)0)->nr_storage_samples);
        case 756: return offsetof(struct wc_flat_pipe_resource, nr_sparse_levels);
        case 757: return sizeof(((struct wc_flat_pipe_resource *)0)->nr_sparse_levels);
        case 758: return offsetof(struct pipe_resource, nr_sparse_levels);
        case 759: return sizeof(((struct pipe_resource *)0)->nr_sparse_levels);
        case 760: return offsetof(struct wc_flat_pipe_resource, compression_rate);
        case 761: return sizeof(((struct wc_flat_pipe_resource *)0)->compression_rate);
        case 762: case 763: return WC_BIT_FIELD;
        case 764: return offsetof(struct wc_flat_pipe_resource, usage);
        case 765: return sizeof(((struct wc_flat_pipe_resource *)0)->usage);
        case 766: case 767: return WC_BIT_FIELD;
        case 768: return offsetof(struct wc_flat_pipe_resource, bind);
        case 769: return sizeof(((struct wc_flat_pipe_resource *)0)->bind);
        case 770: return offsetof(struct pipe_resource, bind);
        case 771: return sizeof(((struct pipe_resource *)0)->bind);
        case 772: return offsetof(struct wc_flat_pipe_resource, flags);
        case 773: return sizeof(((struct wc_flat_pipe_resource *)0)->flags);
        case 774: return offsetof(struct pipe_resource, flags);
        case 775: return sizeof(((struct pipe_resource *)0)->flags);
        case 776: return offsetof(struct wc_flat_pipe_rt_blend_state, blend_enable);
        case 777: return sizeof(((struct wc_flat_pipe_rt_blend_state *)0)->blend_enable);
        case 778: case 779: return WC_BIT_FIELD;
        case 780: return offsetof(struct wc_flat_pipe_rt_blend_state, rgb_func);
        case 781: return sizeof(((struct wc_flat_pipe_rt_blend_state *)0)->rgb_func);
        case 782: case 783: return WC_BIT_FIELD;
        case 784: return offsetof(struct wc_flat_pipe_rt_blend_state, rgb_src_factor);
        case 785: return sizeof(((struct wc_flat_pipe_rt_blend_state *)0)->rgb_src_factor);
        case 786: case 787: return WC_BIT_FIELD;
        case 788: return offsetof(struct wc_flat_pipe_rt_blend_state, rgb_dst_factor);
        case 789: return sizeof(((struct wc_flat_pipe_rt_blend_state *)0)->rgb_dst_factor);
        case 790: case 791: return WC_BIT_FIELD;
        case 792: return offsetof(struct wc_flat_pipe_rt_blend_state, alpha_func);
        case 793: return sizeof(((struct wc_flat_pipe_rt_blend_state *)0)->alpha_func);
        case 794: case 795: return WC_BIT_FIELD;
        case 796: return offsetof(struct wc_flat_pipe_rt_blend_state, alpha_src_factor);
        case 797: return sizeof(((struct wc_flat_pipe_rt_blend_state *)0)->alpha_src_factor);
        case 798: case 799: return WC_BIT_FIELD;
        case 800: return offsetof(struct wc_flat_pipe_rt_blend_state, alpha_dst_factor);
        case 801: return sizeof(((struct wc_flat_pipe_rt_blend_state *)0)->alpha_dst_factor);
        case 802: case 803: return WC_BIT_FIELD;
        case 804: return offsetof(struct wc_flat_pipe_rt_blend_state, colormask);
        case 805: return sizeof(((struct wc_flat_pipe_rt_blend_state *)0)->colormask);
        case 806: case 807: return WC_BIT_FIELD;
        case 808: return offsetof(struct wc_flat_pipe_sampler_state, wrap_s);
        case 809: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->wrap_s);
        case 810: case 811: return WC_BIT_FIELD;
        case 812: return offsetof(struct wc_flat_pipe_sampler_state, wrap_t);
        case 813: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->wrap_t);
        case 814: case 815: return WC_BIT_FIELD;
        case 816: return offsetof(struct wc_flat_pipe_sampler_state, wrap_r);
        case 817: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->wrap_r);
        case 818: case 819: return WC_BIT_FIELD;
        case 820: return offsetof(struct wc_flat_pipe_sampler_state, min_img_filter);
        case 821: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->min_img_filter);
        case 822: case 823: return WC_BIT_FIELD;
        case 824: return offsetof(struct wc_flat_pipe_sampler_state, min_mip_filter);
        case 825: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->min_mip_filter);
        case 826: case 827: return WC_BIT_FIELD;
        case 828: return offsetof(struct wc_flat_pipe_sampler_state, mag_img_filter);
        case 829: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->mag_img_filter);
        case 830: case 831: return WC_BIT_FIELD;
        case 832: return offsetof(struct wc_flat_pipe_sampler_state, compare_mode);
        case 833: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->compare_mode);
        case 834: case 835: return WC_BIT_FIELD;
        case 836: return offsetof(struct wc_flat_pipe_sampler_state, compare_func);
        case 837: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->compare_func);
        case 838: case 839: return WC_BIT_FIELD;
        case 840: return offsetof(struct wc_flat_pipe_sampler_state, unnormalized_coords);
        case 841: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->unnormalized_coords);
        case 842: case 843: return WC_BIT_FIELD;
        case 844: return offsetof(struct wc_flat_pipe_sampler_state, max_anisotropy);
        case 845: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->max_anisotropy);
        case 846: case 847: return WC_BIT_FIELD;
        case 848: return offsetof(struct wc_flat_pipe_sampler_state, seamless_cube_map);
        case 849: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->seamless_cube_map);
        case 850: case 851: return WC_BIT_FIELD;
        case 852: return offsetof(struct wc_flat_pipe_sampler_state, border_color_is_integer);
        case 853: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->border_color_is_integer);
        case 854: case 855: return WC_BIT_FIELD;
        case 856: return offsetof(struct wc_flat_pipe_sampler_state, reduction_mode);
        case 857: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->reduction_mode);
        case 858: case 859: return WC_BIT_FIELD;
        case 860: return offsetof(struct wc_flat_pipe_sampler_state, pad);
        case 861: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->pad);
        case 862: case 863: return WC_BIT_FIELD;
        case 864: return offsetof(struct wc_flat_pipe_sampler_state, lod_bias);
        case 865: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->lod_bias);
        case 866: return offsetof(struct pipe_sampler_state, lod_bias);
        case 867: return sizeof(((struct pipe_sampler_state *)0)->lod_bias);
        case 868: return offsetof(struct wc_flat_pipe_sampler_state, min_lod);
        case 869: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->min_lod);
        case 870: return offsetof(struct pipe_sampler_state, min_lod);
        case 871: return sizeof(((struct pipe_sampler_state *)0)->min_lod);
        case 872: return offsetof(struct wc_flat_pipe_sampler_state, max_lod);
        case 873: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->max_lod);
        case 874: return offsetof(struct pipe_sampler_state, max_lod);
        case 875: return sizeof(((struct pipe_sampler_state *)0)->max_lod);
        case 876: return offsetof(struct wc_flat_pipe_sampler_state, border_color);
        case 877: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->border_color);
        case 878: return offsetof(struct pipe_sampler_state, border_color);
        case 879: return sizeof(((struct pipe_sampler_state *)0)->border_color);
        case 880: return offsetof(struct wc_flat_pipe_sampler_state, border_color_format);
        case 881: return sizeof(((struct wc_flat_pipe_sampler_state *)0)->border_color_format);
        case 882: return offsetof(struct pipe_sampler_state, border_color_format);
        case 883: return sizeof(((struct pipe_sampler_state *)0)->border_color_format);
        case 884: return offsetof(struct wc_flat_pipe_sampler_view, format);
        case 885: return sizeof(((struct wc_flat_pipe_sampler_view *)0)->format);
        case 886: case 887: return WC_BIT_FIELD;
        case 888: return offsetof(struct wc_flat_pipe_sampler_view, astc_decode_format);
        case 889: return sizeof(((struct wc_flat_pipe_sampler_view *)0)->astc_decode_format);
        case 890: case 891: return WC_BIT_FIELD;
        case 892: return offsetof(struct wc_flat_pipe_sampler_view, is_tex2d_from_buf);
        case 893: return sizeof(((struct wc_flat_pipe_sampler_view *)0)->is_tex2d_from_buf);
        case 894: case 895: return WC_BIT_FIELD;
        case 896: return offsetof(struct wc_flat_pipe_sampler_view, target);
        case 897: return sizeof(((struct wc_flat_pipe_sampler_view *)0)->target);
        case 898: case 899: return WC_BIT_FIELD;
        case 900: return offsetof(struct wc_flat_pipe_sampler_view, swizzle_r);
        case 901: return sizeof(((struct wc_flat_pipe_sampler_view *)0)->swizzle_r);
        case 902: case 903: return WC_BIT_FIELD;
        case 904: return offsetof(struct wc_flat_pipe_sampler_view, swizzle_g);
        case 905: return sizeof(((struct wc_flat_pipe_sampler_view *)0)->swizzle_g);
        case 906: case 907: return WC_BIT_FIELD;
        case 908: return offsetof(struct wc_flat_pipe_sampler_view, swizzle_b);
        case 909: return sizeof(((struct wc_flat_pipe_sampler_view *)0)->swizzle_b);
        case 910: case 911: return WC_BIT_FIELD;
        case 912: return offsetof(struct wc_flat_pipe_sampler_view, swizzle_a);
        case 913: return sizeof(((struct wc_flat_pipe_sampler_view *)0)->swizzle_a);
        case 914: case 915: return WC_BIT_FIELD;
        case 916: return offsetof(struct wc_flat_pipe_sampler_view, u);
        case 917: return sizeof(((struct wc_flat_pipe_sampler_view *)0)->u);
        case 918: return offsetof(struct pipe_sampler_view, u);
        case 919: return sizeof(((struct pipe_sampler_view *)0)->u);
        case 920: return offsetof(struct wc_flat_pipe_sampler_view__u, tex);
        case 921: return sizeof(((struct wc_flat_pipe_sampler_view__u *)0)->tex);
        case 922: return offsetof(struct pipe_sampler_view, u.tex) - offsetof(struct pipe_sampler_view, u);
        case 923: return sizeof(((struct pipe_sampler_view *)0)->u.tex);
        case 924: return offsetof(struct wc_flat_pipe_sampler_view__u, buf);
        case 925: return sizeof(((struct wc_flat_pipe_sampler_view__u *)0)->buf);
        case 926: return offsetof(struct pipe_sampler_view, u.buf) - offsetof(struct pipe_sampler_view, u);
        case 927: return sizeof(((struct pipe_sampler_view *)0)->u.buf);
        case 928: return offsetof(struct wc_flat_pipe_sampler_view__u, tex2d_from_buf);
        case 929: return sizeof(((struct wc_flat_pipe_sampler_view__u *)0)->tex2d_from_buf);
        case 930: return offsetof(struct pipe_sampler_view, u.tex2d_from_buf) - offsetof(struct pipe_sampler_view, u);
        case 931: return sizeof(((struct pipe_sampler_view *)0)->u.tex2d_from_buf);
        case 932: return offsetof(struct wc_flat_pipe_sampler_view__u__buf, offset);
        case 933: return sizeof(((struct wc_flat_pipe_sampler_view__u__buf *)0)->offset);
        case 934: return offsetof(struct pipe_sampler_view, u.buf.offset) - offsetof(struct pipe_sampler_view, u.buf);
        case 935: return sizeof(((struct pipe_sampler_view *)0)->u.buf.offset);
        case 936: return offsetof(struct wc_flat_pipe_sampler_view__u__buf, size);
        case 937: return sizeof(((struct wc_flat_pipe_sampler_view__u__buf *)0)->size);
        case 938: return offsetof(struct pipe_sampler_view, u.buf.size) - offsetof(struct pipe_sampler_view, u.buf);
        case 939: return sizeof(((struct pipe_sampler_view *)0)->u.buf.size);
        case 940: return offsetof(struct wc_flat_pipe_sampler_view__u__tex, first_layer);
        case 941: return sizeof(((struct wc_flat_pipe_sampler_view__u__tex *)0)->first_layer);
        case 942: case 943: return WC_BIT_FIELD;
        case 944: return offsetof(struct wc_flat_pipe_sampler_view__u__tex, last_layer);
        case 945: return sizeof(((struct wc_flat_pipe_sampler_view__u__tex *)0)->last_layer);
        case 946: case 947: return WC_BIT_FIELD;
        case 948: return offsetof(struct wc_flat_pipe_sampler_view__u__tex, first_level);
        case 949: return sizeof(((struct wc_flat_pipe_sampler_view__u__tex *)0)->first_level);
        case 950: case 951: return WC_BIT_FIELD;
        case 952: return offsetof(struct wc_flat_pipe_sampler_view__u__tex, last_level);
        case 953: return sizeof(((struct wc_flat_pipe_sampler_view__u__tex *)0)->last_level);
        case 954: case 955: return WC_BIT_FIELD;
        case 956: return offsetof(struct wc_flat_pipe_sampler_view__u__tex, min_lod_clamp);
        case 957: return sizeof(((struct wc_flat_pipe_sampler_view__u__tex *)0)->min_lod_clamp);
        case 958: return offsetof(struct pipe_sampler_view, u.tex.min_lod_clamp) - offsetof(struct pipe_sampler_view, u.tex);
        case 959: return sizeof(((struct pipe_sampler_view *)0)->u.tex.min_lod_clamp);
        case 960: return offsetof(struct wc_flat_pipe_scissor_state, minx);
        case 961: return sizeof(((struct wc_flat_pipe_scissor_state *)0)->minx);
        case 962: return offsetof(struct pipe_scissor_state, minx);
        case 963: return sizeof(((struct pipe_scissor_state *)0)->minx);
        case 964: return offsetof(struct wc_flat_pipe_scissor_state, miny);
        case 965: return sizeof(((struct wc_flat_pipe_scissor_state *)0)->miny);
        case 966: return offsetof(struct pipe_scissor_state, miny);
        case 967: return sizeof(((struct pipe_scissor_state *)0)->miny);
        case 968: return offsetof(struct wc_flat_pipe_scissor_state, maxx);
        case 969: return sizeof(((struct wc_flat_pipe_scissor_state *)0)->maxx);
        case 970: return offsetof(struct pipe_scissor_state, maxx);
        case 971: return sizeof(((struct pipe_scissor_state *)0)->maxx);
        case 972: return offsetof(struct wc_flat_pipe_scissor_state, maxy);
        case 973: return sizeof(((struct wc_flat_pipe_scissor_state *)0)->maxy);
        case 974: return offsetof(struct pipe_scissor_state, maxy);
        case 975: return sizeof(((struct pipe_scissor_state *)0)->maxy);
        case 976: return offsetof(struct wc_flat_pipe_shader_buffer, buffer);
        case 977: return sizeof(((struct wc_flat_pipe_shader_buffer *)0)->buffer);
        case 978: return offsetof(struct pipe_shader_buffer, buffer);
        case 979: return sizeof(((struct pipe_shader_buffer *)0)->buffer);
        case 980: return offsetof(struct wc_flat_pipe_shader_buffer, buffer_offset);
        case 981: return sizeof(((struct wc_flat_pipe_shader_buffer *)0)->buffer_offset);
        case 982: return offsetof(struct pipe_shader_buffer, buffer_offset);
        case 983: return sizeof(((struct pipe_shader_buffer *)0)->buffer_offset);
        case 984: return offsetof(struct wc_flat_pipe_shader_buffer, buffer_size);
        case 985: return sizeof(((struct wc_flat_pipe_shader_buffer *)0)->buffer_size);
        case 986: return offsetof(struct pipe_shader_buffer, buffer_size);
        case 987: return sizeof(((struct pipe_shader_buffer *)0)->buffer_size);
        case 988: return offsetof(struct wc_flat_pipe_shader_state, stream_output);
        case 989: return sizeof(((struct wc_flat_pipe_shader_state *)0)->stream_output);
        case 990: return offsetof(struct pipe_shader_state, stream_output);
        case 991: return sizeof(((struct pipe_shader_state *)0)->stream_output);
        case 992: return offsetof(struct wc_flat_pipe_shader_state, report_compile_error);
        case 993: return sizeof(((struct wc_flat_pipe_shader_state *)0)->report_compile_error);
        case 994: return offsetof(struct pipe_shader_state, report_compile_error);
        case 995: return sizeof(((struct pipe_shader_state *)0)->report_compile_error);
        case 996: return offsetof(struct wc_flat_pipe_stencil_ref, ref_value);
        case 997: return sizeof(((struct wc_flat_pipe_stencil_ref *)0)->ref_value);
        case 998: return offsetof(struct pipe_stencil_ref, ref_value);
        case 999: return sizeof(((struct pipe_stencil_ref *)0)->ref_value);
        case 1000: return offsetof(struct wc_flat_pipe_stencil_state, enabled);
        case 1001: return sizeof(((struct wc_flat_pipe_stencil_state *)0)->enabled);
        case 1002: case 1003: return WC_BIT_FIELD;
        case 1004: return offsetof(struct wc_flat_pipe_stencil_state, func);
        case 1005: return sizeof(((struct wc_flat_pipe_stencil_state *)0)->func);
        case 1006: case 1007: return WC_BIT_FIELD;
        case 1008: return offsetof(struct wc_flat_pipe_stencil_state, fail_op);
        case 1009: return sizeof(((struct wc_flat_pipe_stencil_state *)0)->fail_op);
        case 1010: case 1011: return WC_BIT_FIELD;
        case 1012: return offsetof(struct wc_flat_pipe_stencil_state, zpass_op);
        case 1013: return sizeof(((struct wc_flat_pipe_stencil_state *)0)->zpass_op);
        case 1014: case 1015: return WC_BIT_FIELD;
        case 1016: return offsetof(struct wc_flat_pipe_stencil_state, zfail_op);
        case 1017: return sizeof(((struct wc_flat_pipe_stencil_state *)0)->zfail_op);
        case 1018: case 1019: return WC_BIT_FIELD;
        case 1020: return offsetof(struct wc_flat_pipe_stencil_state, valuemask);
        case 1021: return sizeof(((struct wc_flat_pipe_stencil_state *)0)->valuemask);
        case 1022: case 1023: return WC_BIT_FIELD;
        case 1024: return offsetof(struct wc_flat_pipe_stencil_state, writemask);
        case 1025: return sizeof(((struct wc_flat_pipe_stencil_state *)0)->writemask);
        case 1026: case 1027: return WC_BIT_FIELD;
        case 1028: return offsetof(struct wc_flat_pipe_stream_output, register_index);
        case 1029: return sizeof(((struct wc_flat_pipe_stream_output *)0)->register_index);
        case 1030: case 1031: return WC_BIT_FIELD;
        case 1032: return offsetof(struct wc_flat_pipe_stream_output, start_component);
        case 1033: return sizeof(((struct wc_flat_pipe_stream_output *)0)->start_component);
        case 1034: case 1035: return WC_BIT_FIELD;
        case 1036: return offsetof(struct wc_flat_pipe_stream_output, num_components);
        case 1037: return sizeof(((struct wc_flat_pipe_stream_output *)0)->num_components);
        case 1038: case 1039: return WC_BIT_FIELD;
        case 1040: return offsetof(struct wc_flat_pipe_stream_output, output_buffer);
        case 1041: return sizeof(((struct wc_flat_pipe_stream_output *)0)->output_buffer);
        case 1042: case 1043: return WC_BIT_FIELD;
        case 1044: return offsetof(struct wc_flat_pipe_stream_output, dst_offset);
        case 1045: return sizeof(((struct wc_flat_pipe_stream_output *)0)->dst_offset);
        case 1046: case 1047: return WC_BIT_FIELD;
        case 1048: return offsetof(struct wc_flat_pipe_stream_output, stream);
        case 1049: return sizeof(((struct wc_flat_pipe_stream_output *)0)->stream);
        case 1050: case 1051: return WC_BIT_FIELD;
        case 1052: return offsetof(struct wc_flat_pipe_stream_output_info, num_outputs);
        case 1053: return sizeof(((struct wc_flat_pipe_stream_output_info *)0)->num_outputs);
        case 1054: return offsetof(struct pipe_stream_output_info, num_outputs);
        case 1055: return sizeof(((struct pipe_stream_output_info *)0)->num_outputs);
        case 1056: return offsetof(struct wc_flat_pipe_stream_output_info, stride);
        case 1057: return sizeof(((struct wc_flat_pipe_stream_output_info *)0)->stride);
        case 1058: return offsetof(struct pipe_stream_output_info, stride);
        case 1059: return sizeof(((struct pipe_stream_output_info *)0)->stride);
        case 1060: return offsetof(struct wc_flat_pipe_stream_output_info, output);
        case 1061: return sizeof(((struct wc_flat_pipe_stream_output_info *)0)->output);
        case 1062: return offsetof(struct pipe_stream_output_info, output);
        case 1063: return sizeof(((struct pipe_stream_output_info *)0)->output);
        case 1064: return offsetof(struct wc_flat_pipe_surface, reference);
        case 1065: return sizeof(((struct wc_flat_pipe_surface *)0)->reference);
        case 1066: return offsetof(struct pipe_surface, reference);
        case 1067: return sizeof(((struct pipe_surface *)0)->reference);
        case 1068: return offsetof(struct wc_flat_pipe_surface, format);
        case 1069: return sizeof(((struct wc_flat_pipe_surface *)0)->format);
        case 1070: case 1071: return WC_BIT_FIELD;
        case 1072: return offsetof(struct wc_flat_pipe_surface, nr_samples);
        case 1073: return sizeof(((struct wc_flat_pipe_surface *)0)->nr_samples);
        case 1074: case 1075: return WC_BIT_FIELD;
        case 1076: return offsetof(struct wc_flat_pipe_surface, first_layer);
        case 1077: return sizeof(((struct wc_flat_pipe_surface *)0)->first_layer);
        case 1078: case 1079: return WC_BIT_FIELD;
        case 1080: return offsetof(struct wc_flat_pipe_surface, last_layer);
        case 1081: return sizeof(((struct wc_flat_pipe_surface *)0)->last_layer);
        case 1082: case 1083: return WC_BIT_FIELD;
        case 1084: return offsetof(struct wc_flat_pipe_surface, level);
        case 1085: return sizeof(((struct wc_flat_pipe_surface *)0)->level);
        case 1086: return offsetof(struct pipe_surface, level);
        case 1087: return sizeof(((struct pipe_surface *)0)->level);
        case 1088: return offsetof(struct wc_flat_pipe_surface, texture);
        case 1089: return sizeof(((struct wc_flat_pipe_surface *)0)->texture);
        case 1090: return offsetof(struct pipe_surface, texture);
        case 1091: return sizeof(((struct pipe_surface *)0)->texture);
        case 1092: return offsetof(struct wc_flat_pipe_tex2d_from_buf, offset);
        case 1093: return sizeof(((struct wc_flat_pipe_tex2d_from_buf *)0)->offset);
        case 1094: return offsetof(struct pipe_tex2d_from_buf, offset);
        case 1095: return sizeof(((struct pipe_tex2d_from_buf *)0)->offset);
        case 1096: return offsetof(struct wc_flat_pipe_tex2d_from_buf, row_stride);
        case 1097: return sizeof(((struct wc_flat_pipe_tex2d_from_buf *)0)->row_stride);
        case 1098: return offsetof(struct pipe_tex2d_from_buf, row_stride);
        case 1099: return sizeof(((struct pipe_tex2d_from_buf *)0)->row_stride);
        case 1100: return offsetof(struct wc_flat_pipe_tex2d_from_buf, width);
        case 1101: return sizeof(((struct wc_flat_pipe_tex2d_from_buf *)0)->width);
        case 1102: return offsetof(struct pipe_tex2d_from_buf, width);
        case 1103: return sizeof(((struct pipe_tex2d_from_buf *)0)->width);
        case 1104: return offsetof(struct wc_flat_pipe_tex2d_from_buf, height);
        case 1105: return sizeof(((struct wc_flat_pipe_tex2d_from_buf *)0)->height);
        case 1106: return offsetof(struct pipe_tex2d_from_buf, height);
        case 1107: return sizeof(((struct pipe_tex2d_from_buf *)0)->height);
        case 1108: return offsetof(struct wc_flat_pipe_vertex_buffer, is_user_buffer);
        case 1109: return sizeof(((struct wc_flat_pipe_vertex_buffer *)0)->is_user_buffer);
        case 1110: return offsetof(struct pipe_vertex_buffer, is_user_buffer);
        case 1111: return sizeof(((struct pipe_vertex_buffer *)0)->is_user_buffer);
        case 1112: return offsetof(struct wc_flat_pipe_vertex_buffer, buffer_offset);
        case 1113: return sizeof(((struct wc_flat_pipe_vertex_buffer *)0)->buffer_offset);
        case 1114: return offsetof(struct pipe_vertex_buffer, buffer_offset);
        case 1115: return sizeof(((struct pipe_vertex_buffer *)0)->buffer_offset);
        case 1116: return offsetof(struct wc_flat_pipe_vertex_buffer, buffer);
        case 1117: return sizeof(((struct wc_flat_pipe_vertex_buffer *)0)->buffer);
        case 1118: return offsetof(struct pipe_vertex_buffer, buffer);
        case 1119: return sizeof(((struct pipe_vertex_buffer *)0)->buffer);
        case 1120: return offsetof(struct wc_flat_pipe_vertex_buffer__buffer, resource);
        case 1121: return sizeof(((struct wc_flat_pipe_vertex_buffer__buffer *)0)->resource);
        case 1122: return offsetof(struct pipe_vertex_buffer, buffer.resource) - offsetof(struct pipe_vertex_buffer, buffer);
        case 1123: return sizeof(((struct pipe_vertex_buffer *)0)->buffer.resource);
        case 1124: return offsetof(struct wc_flat_pipe_vertex_element, src_offset);
        case 1125: return sizeof(((struct wc_flat_pipe_vertex_element *)0)->src_offset);
        case 1126: return offsetof(struct pipe_vertex_element, src_offset);
        case 1127: return sizeof(((struct pipe_vertex_element *)0)->src_offset);
        case 1128: return offsetof(struct wc_flat_pipe_vertex_element, vertex_buffer_index);
        case 1129: return sizeof(((struct wc_flat_pipe_vertex_element *)0)->vertex_buffer_index);
        case 1130: case 1131: return WC_BIT_FIELD;
        case 1132: return offsetof(struct wc_flat_pipe_vertex_element, dual_slot);
        case 1133: return sizeof(((struct wc_flat_pipe_vertex_element *)0)->dual_slot);
        case 1134: case 1135: return WC_BIT_FIELD;
        case 1136: return offsetof(struct wc_flat_pipe_vertex_element, src_format);
        case 1137: return sizeof(((struct wc_flat_pipe_vertex_element *)0)->src_format);
        case 1138: return offsetof(struct pipe_vertex_element, src_format);
        case 1139: return sizeof(((struct pipe_vertex_element *)0)->src_format);
        case 1140: return offsetof(struct wc_flat_pipe_vertex_element, src_stride);
        case 1141: return sizeof(((struct wc_flat_pipe_vertex_element *)0)->src_stride);
        case 1142: return offsetof(struct pipe_vertex_element, src_stride);
        case 1143: return sizeof(((struct pipe_vertex_element *)0)->src_stride);
        case 1144: return offsetof(struct wc_flat_pipe_vertex_element, instance_divisor);
        case 1145: return sizeof(((struct wc_flat_pipe_vertex_element *)0)->instance_divisor);
        case 1146: return offsetof(struct pipe_vertex_element, instance_divisor);
        case 1147: return sizeof(((struct pipe_vertex_element *)0)->instance_divisor);
        case 1148: return offsetof(struct wc_flat_pipe_viewport_state, scale);
        case 1149: return sizeof(((struct wc_flat_pipe_viewport_state *)0)->scale);
        case 1150: return offsetof(struct pipe_viewport_state, scale);
        case 1151: return sizeof(((struct pipe_viewport_state *)0)->scale);
        case 1152: return offsetof(struct wc_flat_pipe_viewport_state, translate);
        case 1153: return sizeof(((struct wc_flat_pipe_viewport_state *)0)->translate);
        case 1154: return offsetof(struct pipe_viewport_state, translate);
        case 1155: return sizeof(((struct pipe_viewport_state *)0)->translate);
        case 1156: return offsetof(struct wc_flat_pipe_viewport_state, swizzle_x);
        case 1157: return sizeof(((struct wc_flat_pipe_viewport_state *)0)->swizzle_x);
        case 1158: case 1159: return WC_BIT_FIELD;
        case 1160: return offsetof(struct wc_flat_pipe_viewport_state, swizzle_y);
        case 1161: return sizeof(((struct wc_flat_pipe_viewport_state *)0)->swizzle_y);
        case 1162: case 1163: return WC_BIT_FIELD;
        case 1164: return offsetof(struct wc_flat_pipe_viewport_state, swizzle_z);
        case 1165: return sizeof(((struct wc_flat_pipe_viewport_state *)0)->swizzle_z);
        case 1166: case 1167: return WC_BIT_FIELD;
        case 1168: return offsetof(struct wc_flat_pipe_viewport_state, swizzle_w);
        case 1169: return sizeof(((struct wc_flat_pipe_viewport_state *)0)->swizzle_w);
        case 1170: case 1171: return WC_BIT_FIELD;
        default: return WC_NO_FIELD;
    }
}
