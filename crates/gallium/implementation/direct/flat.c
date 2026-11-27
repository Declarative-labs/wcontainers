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
