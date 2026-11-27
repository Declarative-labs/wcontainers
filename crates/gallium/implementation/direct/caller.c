/* Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit. */
/* The hostile C caller of the serving glue's first slice: it calls a served screen's pointers as Mesa would, and as
 * Mesa never would (a key never handed out, a key of another kind, a state given back twice, no template, lists past
 * their bound). Each case returns 0 when what C sees holds; the Rust test (tests/served.rs) checks what reached the
 * implementation. Built with the direct crate's `hostile` feature only. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "pipe/p_context.h"
#include "pipe/p_defines.h"
#include "pipe/p_screen.h"
#include "pipe/p_state.h"
#include "util/blend.h"
#include "util/u_screen.h"

int wc_caller_cases(void);
int wc_caller_case(uintptr_t address, int which);
int wc_caller_caps(uintptr_t address);
int wc_caller_two_screens(uintptr_t first, uintptr_t second);
int drmGetCap(int fd, uint64_t capability, uint64_t *value);

/* Every field the trait carries of each served state set to a value of its own (written by the generator, which
 * states in tests/served.rs the owned structure each must reach the implementation as). */
static void filled_blend(struct pipe_blend_state *blend)
{
    memset(blend, 0, sizeof *blend);
    blend->independent_blend_enable = 1;
    blend->logicop_enable = 1;
    blend->logicop_func = 4;
    blend->dither = 1;
    blend->alpha_to_coverage = 1;
    blend->alpha_to_coverage_dither = 1;
    blend->alpha_to_one = 1;
    blend->max_rt = 2;
    blend->advanced_blend_func = 10;
    blend->blend_coherent = 1;
    blend->is_yuv = 1;
    blend->rt[0].blend_enable = 1;
    blend->rt[0].rgb_func = 7;
    blend->rt[0].rgb_src_factor = 15;
    blend->rt[0].rgb_dst_factor = 16;
    blend->rt[0].alpha_func = 3;
    blend->rt[0].alpha_src_factor = 18;
    blend->rt[0].alpha_dst_factor = 19;
    blend->rt[0].colormask = 5;
    blend->rt[1].blend_enable = 1;
    blend->rt[1].rgb_func = 1;
    blend->rt[1].rgb_src_factor = 23;
    blend->rt[1].rgb_dst_factor = 24;
    blend->rt[1].alpha_func = 4;
    blend->rt[1].alpha_src_factor = 26;
    blend->rt[1].alpha_dst_factor = 27;
    blend->rt[1].colormask = 13;
    blend->rt[2].blend_enable = 1;
    blend->rt[2].rgb_func = 2;
    blend->rt[2].rgb_src_factor = 31;
    blend->rt[2].rgb_dst_factor = 1;
    blend->rt[2].alpha_func = 5;
    blend->rt[2].alpha_src_factor = 3;
    blend->rt[2].alpha_dst_factor = 4;
    blend->rt[2].colormask = 6;
    blend->rt[3].blend_enable = 1;
    blend->rt[3].rgb_func = 3;
    blend->rt[3].rgb_src_factor = 8;
    blend->rt[3].rgb_dst_factor = 9;
    blend->rt[3].alpha_func = 6;
    blend->rt[3].alpha_src_factor = 11;
    blend->rt[3].alpha_dst_factor = 12;
    blend->rt[3].colormask = 14;
    blend->rt[4].blend_enable = 1;
    blend->rt[4].rgb_func = 4;
    blend->rt[4].rgb_src_factor = 16;
    blend->rt[4].rgb_dst_factor = 17;
    blend->rt[4].alpha_func = 7;
    blend->rt[4].alpha_src_factor = 19;
    blend->rt[4].alpha_dst_factor = 20;
    blend->rt[4].colormask = 7;
    blend->rt[5].blend_enable = 1;
    blend->rt[5].rgb_func = 5;
    blend->rt[5].rgb_src_factor = 24;
    blend->rt[5].rgb_dst_factor = 25;
    blend->rt[5].alpha_func = 1;
    blend->rt[5].alpha_src_factor = 27;
    blend->rt[5].alpha_dst_factor = 28;
    blend->rt[5].colormask = 15;
    blend->rt[6].blend_enable = 1;
    blend->rt[6].rgb_func = 6;
    blend->rt[6].rgb_src_factor = 1;
    blend->rt[6].rgb_dst_factor = 2;
    blend->rt[6].alpha_func = 2;
    blend->rt[6].alpha_src_factor = 4;
    blend->rt[6].alpha_dst_factor = 5;
    blend->rt[6].colormask = 8;
    blend->rt[7].blend_enable = 1;
    blend->rt[7].rgb_func = 7;
    blend->rt[7].rgb_src_factor = 9;
    blend->rt[7].rgb_dst_factor = 10;
    blend->rt[7].alpha_func = 3;
    blend->rt[7].alpha_src_factor = 12;
    blend->rt[7].alpha_dst_factor = 13;
    blend->rt[7].colormask = 1;
}

static void filled_depth_stencil_alpha(struct pipe_depth_stencil_alpha_state *dsa)
{
    memset(dsa, 0, sizeof *dsa);
    dsa->stencil[0].enabled = 1;
    dsa->stencil[0].func = 1;
    dsa->stencil[0].fail_op = 2;
    dsa->stencil[0].zpass_op = 3;
    dsa->stencil[0].zfail_op = 4;
    dsa->stencil[0].valuemask = 82;
    dsa->stencil[0].writemask = 83;
    dsa->stencil[1].enabled = 1;
    dsa->stencil[1].func = 1;
    dsa->stencil[1].fail_op = 2;
    dsa->stencil[1].zpass_op = 3;
    dsa->stencil[1].zfail_op = 4;
    dsa->stencil[1].valuemask = 89;
    dsa->stencil[1].writemask = 90;
    dsa->alpha_enabled = 1;
    dsa->alpha_func = 1;
    dsa->depth_enabled = 1;
    dsa->depth_writemask = 1;
    dsa->depth_func = 4;
    dsa->depth_bounds_test = 1;
    dsa->alpha_ref_value = 1096.25f;
    dsa->depth_bounds_min = 1097.25f;
    dsa->depth_bounds_max = 1098.25f;
}

static void filled_rasterizer(struct pipe_rasterizer_state *rasterizer)
{
    memset(rasterizer, 0, sizeof *rasterizer);
    rasterizer->flatshade = 1;
    rasterizer->light_twoside = 1;
    rasterizer->clamp_vertex_color = 1;
    rasterizer->clamp_fragment_color = 1;
    rasterizer->front_ccw = 1;
    rasterizer->cull_face = 3;
    rasterizer->fill_front = 1;
    rasterizer->fill_back = 2;
    rasterizer->offset_point = 1;
    rasterizer->offset_line = 1;
    rasterizer->offset_tri = 1;
    rasterizer->scissor = 1;
    rasterizer->poly_smooth = 1;
    rasterizer->poly_stipple_enable = 1;
    rasterizer->point_smooth = 1;
    rasterizer->sprite_coord_mode = 1;
    rasterizer->point_quad_rasterization = 1;
    rasterizer->point_line_tri_clip = 1;
    rasterizer->point_size_per_vertex = 1;
    rasterizer->multisample = 1;
    rasterizer->no_ms_sample_mask_out = 1;
    rasterizer->force_persample_interp = 1;
    rasterizer->line_smooth = 1;
    rasterizer->line_stipple_enable = 1;
    rasterizer->line_last_pixel = 1;
    rasterizer->line_rectangular = 1;
    rasterizer->conservative_raster_mode = 3;
    rasterizer->flatshade_first = 1;
    rasterizer->half_pixel_center = 1;
    rasterizer->bottom_edge_rule = 1;
    rasterizer->subpixel_precision_x = 10;
    rasterizer->subpixel_precision_y = 11;
    rasterizer->rasterizer_discard = 1;
    rasterizer->rasterization_stream = 1;
    rasterizer->tile_raster_order_fixed = 1;
    rasterizer->tile_raster_order_increasing_x = 1;
    rasterizer->tile_raster_order_increasing_y = 1;
    rasterizer->depth_clip_near = 1;
    rasterizer->depth_clip_far = 1;
    rasterizer->depth_clamp = 1;
    rasterizer->clip_halfz = 1;
    rasterizer->offset_units_unscaled = 1;
    rasterizer->unclamped_fragment_depth_values = 1;
    rasterizer->clip_plane_enable = 143;
    rasterizer->representative_fragment_test = 1;
    rasterizer->line_stipple_factor = 145;
    rasterizer->line_stipple_pattern = 146;
    rasterizer->sprite_coord_enable = 1146;
    rasterizer->line_width = 1147.25f;
    rasterizer->point_size = 1148.25f;
    rasterizer->offset_units = 1149.25f;
    rasterizer->offset_scale = 1150.25f;
    rasterizer->offset_clamp = 1151.25f;
    rasterizer->conservative_raster_dilate = 1152.25f;
}

static void filled_sampler_first(struct pipe_sampler_state *sampler)
{
    memset(sampler, 0, sizeof *sampler);
    sampler->wrap_s = 7;
    sampler->wrap_t = 1;
    sampler->wrap_r = 2;
    sampler->min_img_filter = 1;
    sampler->min_mip_filter = 2;
    sampler->mag_img_filter = 1;
    sampler->compare_mode = 1;
    sampler->compare_func = 7;
    sampler->unnormalized_coords = 1;
    sampler->max_anisotropy = 8;
    sampler->seamless_cube_map = 1;
    sampler->border_color_is_integer = 1;
    sampler->reduction_mode = 1;
    sampler->pad = 12;
    sampler->lod_bias = 1167.25f;
    sampler->min_lod = 1168.25f;
    sampler->max_lod = 1169.25f;
    sampler->border_color.ui[0] = 1170u;
    sampler->border_color.ui[1] = 1171u;
    sampler->border_color.ui[2] = 1172u;
    sampler->border_color.ui[3] = 1173u;
    sampler->border_color_format = 1174;
}

static void filled_sampler_second(struct pipe_sampler_state *sampler)
{
    memset(sampler, 0, sizeof *sampler);
    sampler->wrap_s = 1;
    sampler->wrap_t = 2;
    sampler->wrap_r = 3;
    sampler->min_img_filter = 1;
    sampler->min_mip_filter = 3;
    sampler->mag_img_filter = 1;
    sampler->compare_mode = 1;
    sampler->compare_func = 1;
    sampler->unnormalized_coords = 1;
    sampler->max_anisotropy = 30;
    sampler->seamless_cube_map = 1;
    sampler->border_color_is_integer = 1;
    sampler->reduction_mode = 2;
    sampler->pad = 3;
    sampler->lod_bias = 1189.25f;
    sampler->min_lod = 1190.25f;
    sampler->max_lod = 1191.25f;
    sampler->border_color.ui[0] = 1192u;
    sampler->border_color.ui[1] = 1193u;
    sampler->border_color.ui[2] = 1194u;
    sampler->border_color.ui[3] = 1195u;
    sampler->border_color_format = 1196;
}

static void filled_vertex_elements(struct pipe_vertex_element elements[2])
{
    memset(elements, 0, 2 * sizeof elements[0]);
    elements[0].src_offset = 1197;
    elements[0].vertex_buffer_index = 72;
    elements[0].dual_slot = 1;
    elements[0].src_format = 1;
    elements[0].src_stride = 1201;
    elements[0].instance_divisor = 1202;
    elements[1].src_offset = 1203;
    elements[1].vertex_buffer_index = 78;
    elements[1].dual_slot = 0;
    elements[1].src_format = 7;
    elements[1].src_stride = 1207;
    elements[1].instance_divisor = 1208;
}

/* A key no call ever handed out. */
#define WC_NEVER_HANDED_OUT ((void *)(uintptr_t)0x7f00ff)

static void *made_blend(struct pipe_context *ctx)
{
    struct pipe_blend_state blend;
    memset(&blend, 0, sizeof blend);
    blend.independent_blend_enable = 1;
    blend.rt[0].blend_enable = 1;
    blend.rt[0].rgb_func = PIPE_BLEND_ADD;
    blend.rt[0].rgb_src_factor = PIPE_BLENDFACTOR_ONE;
    blend.rt[0].colormask = 0xf;
    return ctx->create_blend_state(ctx, &blend);
}

static void *made_rasterizer(struct pipe_context *ctx)
{
    struct pipe_rasterizer_state rasterizer;
    memset(&rasterizer, 0, sizeof rasterizer);
    rasterizer.cull_face = PIPE_FACE_BACK;
    rasterizer.line_width = 1.0f;
    return ctx->create_rasterizer_state(ctx, &rasterizer);
}

static void *made_sampler(struct pipe_context *ctx, float min_lod, float max_lod)
{
    struct pipe_sampler_state sampler;
    memset(&sampler, 0, sizeof sampler);
    sampler.wrap_s = PIPE_TEX_WRAP_CLAMP_TO_EDGE;
    sampler.min_lod = min_lod;
    sampler.max_lod = max_lod;
    return ctx->create_sampler_state(ctx, &sampler);
}

/* Every state made with every field set, bound (and unbound) and given back, as st/mesa does. */
static int states(struct pipe_context *ctx)
{
    struct pipe_blend_state blend;
    filled_blend(&blend);
    void *made = ctx->create_blend_state(ctx, &blend);
    if (!made)
        return 10;
    ctx->bind_blend_state(ctx, made);
    ctx->bind_blend_state(ctx, NULL);
    ctx->delete_blend_state(ctx, made);

    struct pipe_depth_stencil_alpha_state dsa;
    filled_depth_stencil_alpha(&dsa);
    void *depth = ctx->create_depth_stencil_alpha_state(ctx, &dsa);
    if (!depth)
        return 11;
    ctx->bind_depth_stencil_alpha_state(ctx, depth);
    ctx->delete_depth_stencil_alpha_state(ctx, depth);

    struct pipe_rasterizer_state rasterizer;
    filled_rasterizer(&rasterizer);
    void *raster = ctx->create_rasterizer_state(ctx, &rasterizer);
    if (!raster)
        return 12;
    ctx->bind_rasterizer_state(ctx, raster);
    ctx->delete_rasterizer_state(ctx, raster);

    struct pipe_sampler_state first, second;
    filled_sampler_first(&first);
    filled_sampler_second(&second);
    void *samplers[2] = {ctx->create_sampler_state(ctx, &first), ctx->create_sampler_state(ctx, &second)};
    if (!samplers[0] || !samplers[1])
        return 13;
    /* At slots 3 and 4 of the geometry stage, so the stage and the first slot are values of their own. */
    ctx->bind_sampler_states(ctx, MESA_SHADER_GEOMETRY, 3, 2, samplers);
    ctx->delete_sampler_state(ctx, samplers[0]);
    ctx->delete_sampler_state(ctx, samplers[1]);

    struct pipe_vertex_element elements[2];
    filled_vertex_elements(elements);
    void *vertex = ctx->create_vertex_elements_state(ctx, 2, elements);
    if (!vertex)
        return 14;
    ctx->bind_vertex_elements_state(ctx, vertex);
    ctx->delete_vertex_elements_state(ctx, vertex);
    return 0;
}

/* A key never handed out, bound: refused before the implementation. */
static int unknown_key(struct pipe_context *ctx)
{
    ctx->bind_blend_state(ctx, WC_NEVER_HANDED_OUT);
    ctx->delete_blend_state(ctx, WC_NEVER_HANDED_OUT);
    return 0;
}

/* A state given back twice: the second refused. */
static int given_back_twice(struct pipe_context *ctx)
{
    void *blend = made_blend(ctx);
    if (!blend)
        return 20;
    ctx->delete_blend_state(ctx, blend);
    ctx->delete_blend_state(ctx, blend);
    ctx->bind_blend_state(ctx, blend);
    return 0;
}

/* A rasterizer's key bound and given back as a blend state: refused (each kind's keys are its own). */
static int other_kind(struct pipe_context *ctx)
{
    void *rasterizer = made_rasterizer(ctx);
    if (!rasterizer)
        return 30;
    ctx->bind_blend_state(ctx, rasterizer);
    ctx->delete_blend_state(ctx, rasterizer);
    ctx->delete_rasterizer_state(ctx, rasterizer);
    return 0;
}

/* No template: refused in C, nothing made. */
static int no_template(struct pipe_context *ctx)
{
    if (ctx->create_blend_state(ctx, NULL) || ctx->create_rasterizer_state(ctx, NULL) || ctx->create_sampler_state(ctx, NULL) ||
        ctx->create_depth_stencil_alpha_state(ctx, NULL) || ctx->create_vertex_elements_state(ctx, 1, NULL))
        return 40;
    return 0;
}

/* Lists past their bound: refused in C. */
static int past_the_bound(struct pipe_context *ctx)
{
    void *samplers[PIPE_MAX_SAMPLERS + 1];
    memset(samplers, 0, sizeof samplers);
    ctx->bind_sampler_states(ctx, MESA_SHADER_FRAGMENT, 0, PIPE_MAX_SAMPLERS + 1, samplers);
    struct pipe_vertex_element elements[PIPE_MAX_ATTRIBS + 1];
    memset(elements, 0, sizeof elements);
    if (ctx->create_vertex_elements_state(ctx, PIPE_MAX_ATTRIBS + 1, elements))
        return 50;
    return 0;
}

static int (*const cases[])(struct pipe_context *) = {states, unknown_key, given_back_twice, other_kind, no_template, past_the_bound};

int wc_caller_cases(void)
{
    return (int)(sizeof cases / sizeof cases[0]);
}

/* One case on a served screen: a context made, the case, the context and the screen destroyed. */
int wc_caller_case(uintptr_t address, int which)
{
    if (which < 0 || which >= wc_caller_cases())
        return 1;
    struct pipe_screen *screen = (struct pipe_screen *)address;
    /* A private pointer and flags of their own, which the implementation must receive as they are. */
    struct pipe_context *ctx = screen->context_create(screen, (void *)(uintptr_t)0x5a5a, PIPE_CONTEXT_ROBUST_BUFFER_ACCESS);
    if (!ctx)
        return 2;
    const int status = cases[which](ctx);
    ctx->destroy(ctx);
    screen->destroy(screen);
    return status;
}

/* u_init_pipe_screen_caps's one call into libdrm, made only through a screen's get_screen_fd, which neither the served
 * screen nor the reference sets: a stand-in in the test's own caller, so that the test links Mesa's function without
 * libdrm. */
int drmGetCap(int fd, uint64_t capability, uint64_t *value)
{
    (void)fd;
    (void)capability;
    (void)value;
    return -1;
}

/* The served screen's capabilities against Mesa's own: a reference screen given the served stages' capabilities and
 * filled by u_init_pipe_screen_caps as a driver accelerated by hardware fills it (accel 1). 0 when every byte of the
 * two `caps` is equal, else 2 plus the offset of the first that differs; the served screen is destroyed. */
int wc_caller_caps(uintptr_t address)
{
    struct pipe_screen *screen = (struct pipe_screen *)address;
    struct pipe_screen *reference = calloc(1, sizeof *reference);
    if (!reference)
        return 1;
    memcpy((void *)reference->shader_caps, screen->shader_caps, sizeof screen->shader_caps);
    u_init_pipe_screen_caps(reference, 1);
    int status = 0;
    const unsigned char *ours = (const unsigned char *)&screen->caps, *mesas = (const unsigned char *)&reference->caps;
    for (size_t at = 0; at < sizeof screen->caps; at++) {
        if (ours[at] != mesas[at]) {
            status = 2 + (int)at;
            break;
        }
    }
    free(reference);
    screen->destroy(screen);
    return status;
}

/* Mesa's screen held, field by field, to the capabilities the recording implementation sets for `seed`
 * (Recording::filling_caps): 0 when every field holds its value, else the number of the first that does not
 * (tests/served.rs's CAPS_CHECKS names each). The screen is destroyed. */
int wc_caller_caps_filled(uintptr_t address, int seed);
int wc_caller_caps_filled(uintptr_t address, int seed)
{
    struct pipe_screen *screen = (struct pipe_screen *)address;
    int status = 0;
    if (seed == 0) {
        if (status == 0 && screen->shader_caps[0].max_instructions != 1001)
            status = 1;
        if (status == 0 && screen->shader_caps[0].max_alu_instructions != 1002)
            status = 2;
        if (status == 0 && screen->shader_caps[0].max_tex_instructions != 1003)
            status = 3;
        if (status == 0 && screen->shader_caps[0].max_tex_indirections != 1004)
            status = 4;
        if (status == 0 && screen->shader_caps[0].max_control_flow_depth != 1005)
            status = 5;
        if (status == 0 && screen->shader_caps[0].max_inputs != 1006)
            status = 6;
        if (status == 0 && screen->shader_caps[0].max_outputs != 1007)
            status = 7;
        if (status == 0 && screen->shader_caps[0].max_const_buffer0_size != 1008)
            status = 8;
        if (status == 0 && screen->shader_caps[0].max_const_buffers != 1009)
            status = 9;
        if (status == 0 && screen->shader_caps[0].max_temps != 1010)
            status = 10;
        if (status == 0 && screen->shader_caps[0].max_texture_samplers != 1011)
            status = 11;
        if (status == 0 && screen->shader_caps[0].max_sampler_views != 1012)
            status = 12;
        if (status == 0 && screen->shader_caps[0].max_shader_buffers != 1013)
            status = 13;
        if (status == 0 && screen->shader_caps[0].max_shader_images != 1014)
            status = 14;
        if (status == 0 && screen->shader_caps[0].max_hw_atomic_counters != 1015)
            status = 15;
        if (status == 0 && screen->shader_caps[0].max_hw_atomic_counter_buffers != 1016)
            status = 16;
        if (status == 0 && screen->shader_caps[0].supported_irs != 1017)
            status = 17;
        if (status == 0 && screen->shader_caps[0].cont_supported != 1)
            status = 18;
        if (status == 0 && screen->shader_caps[0].indirect_temp_addr != 1)
            status = 19;
        if (status == 0 && screen->shader_caps[0].indirect_const_addr != 0)
            status = 20;
        if (status == 0 && screen->shader_caps[0].subroutines != 0)
            status = 21;
        if (status == 0 && screen->shader_caps[0].integers != 1)
            status = 22;
        if (status == 0 && screen->shader_caps[0].int64_atomics != 1)
            status = 23;
        if (status == 0 && screen->shader_caps[0].fp16 != 0)
            status = 24;
        if (status == 0 && screen->shader_caps[0].fp16_derivatives != 0)
            status = 25;
        if (status == 0 && screen->shader_caps[0].fp16_const_buffers != 1)
            status = 26;
        if (status == 0 && screen->shader_caps[0].fp16_no_denorms != 1)
            status = 27;
        if (status == 0 && screen->shader_caps[0].int16 != 0)
            status = 28;
        if (status == 0 && screen->shader_caps[0].glsl_16bit_consts != 0)
            status = 29;
        if (status == 0 && screen->shader_caps[0].glsl_16bit_load_dst != 1)
            status = 30;
        if (status == 0 && screen->shader_caps[0].tgsi_sqrt_supported != 1)
            status = 31;
        if (status == 0 && screen->shader_caps[0].tgsi_any_inout_decl_range != 0)
            status = 32;
        if (status == 0 && screen->shader_caps[1].max_instructions != 1033)
            status = 33;
        if (status == 0 && screen->shader_caps[1].max_alu_instructions != 1034)
            status = 34;
        if (status == 0 && screen->shader_caps[1].max_tex_instructions != 1035)
            status = 35;
        if (status == 0 && screen->shader_caps[1].max_tex_indirections != 1036)
            status = 36;
        if (status == 0 && screen->shader_caps[1].max_control_flow_depth != 1037)
            status = 37;
        if (status == 0 && screen->shader_caps[1].max_inputs != 1038)
            status = 38;
        if (status == 0 && screen->shader_caps[1].max_outputs != 1039)
            status = 39;
        if (status == 0 && screen->shader_caps[1].max_const_buffer0_size != 1040)
            status = 40;
        if (status == 0 && screen->shader_caps[1].max_const_buffers != 1041)
            status = 41;
        if (status == 0 && screen->shader_caps[1].max_temps != 1042)
            status = 42;
        if (status == 0 && screen->shader_caps[1].max_texture_samplers != 1043)
            status = 43;
        if (status == 0 && screen->shader_caps[1].max_sampler_views != 1044)
            status = 44;
        if (status == 0 && screen->shader_caps[1].max_shader_buffers != 1045)
            status = 45;
        if (status == 0 && screen->shader_caps[1].max_shader_images != 1046)
            status = 46;
        if (status == 0 && screen->shader_caps[1].max_hw_atomic_counters != 1047)
            status = 47;
        if (status == 0 && screen->shader_caps[1].max_hw_atomic_counter_buffers != 1048)
            status = 48;
        if (status == 0 && screen->shader_caps[1].supported_irs != 1049)
            status = 49;
        if (status == 0 && screen->shader_caps[1].cont_supported != 1)
            status = 50;
        if (status == 0 && screen->shader_caps[1].indirect_temp_addr != 1)
            status = 51;
        if (status == 0 && screen->shader_caps[1].indirect_const_addr != 0)
            status = 52;
        if (status == 0 && screen->shader_caps[1].subroutines != 0)
            status = 53;
        if (status == 0 && screen->shader_caps[1].integers != 1)
            status = 54;
        if (status == 0 && screen->shader_caps[1].int64_atomics != 1)
            status = 55;
        if (status == 0 && screen->shader_caps[1].fp16 != 0)
            status = 56;
        if (status == 0 && screen->shader_caps[1].fp16_derivatives != 0)
            status = 57;
        if (status == 0 && screen->shader_caps[1].fp16_const_buffers != 1)
            status = 58;
        if (status == 0 && screen->shader_caps[1].fp16_no_denorms != 1)
            status = 59;
        if (status == 0 && screen->shader_caps[1].int16 != 0)
            status = 60;
        if (status == 0 && screen->shader_caps[1].glsl_16bit_consts != 0)
            status = 61;
        if (status == 0 && screen->shader_caps[1].glsl_16bit_load_dst != 1)
            status = 62;
        if (status == 0 && screen->shader_caps[1].tgsi_sqrt_supported != 1)
            status = 63;
        if (status == 0 && screen->shader_caps[1].tgsi_any_inout_decl_range != 0)
            status = 64;
        if (status == 0 && screen->shader_caps[2].max_instructions != 1065)
            status = 65;
        if (status == 0 && screen->shader_caps[2].max_alu_instructions != 1066)
            status = 66;
        if (status == 0 && screen->shader_caps[2].max_tex_instructions != 1067)
            status = 67;
        if (status == 0 && screen->shader_caps[2].max_tex_indirections != 1068)
            status = 68;
        if (status == 0 && screen->shader_caps[2].max_control_flow_depth != 1069)
            status = 69;
        if (status == 0 && screen->shader_caps[2].max_inputs != 1070)
            status = 70;
        if (status == 0 && screen->shader_caps[2].max_outputs != 1071)
            status = 71;
        if (status == 0 && screen->shader_caps[2].max_const_buffer0_size != 1072)
            status = 72;
        if (status == 0 && screen->shader_caps[2].max_const_buffers != 1073)
            status = 73;
        if (status == 0 && screen->shader_caps[2].max_temps != 1074)
            status = 74;
        if (status == 0 && screen->shader_caps[2].max_texture_samplers != 1075)
            status = 75;
        if (status == 0 && screen->shader_caps[2].max_sampler_views != 1076)
            status = 76;
        if (status == 0 && screen->shader_caps[2].max_shader_buffers != 1077)
            status = 77;
        if (status == 0 && screen->shader_caps[2].max_shader_images != 1078)
            status = 78;
        if (status == 0 && screen->shader_caps[2].max_hw_atomic_counters != 1079)
            status = 79;
        if (status == 0 && screen->shader_caps[2].max_hw_atomic_counter_buffers != 1080)
            status = 80;
        if (status == 0 && screen->shader_caps[2].supported_irs != 1081)
            status = 81;
        if (status == 0 && screen->shader_caps[2].cont_supported != 1)
            status = 82;
        if (status == 0 && screen->shader_caps[2].indirect_temp_addr != 1)
            status = 83;
        if (status == 0 && screen->shader_caps[2].indirect_const_addr != 0)
            status = 84;
        if (status == 0 && screen->shader_caps[2].subroutines != 0)
            status = 85;
        if (status == 0 && screen->shader_caps[2].integers != 1)
            status = 86;
        if (status == 0 && screen->shader_caps[2].int64_atomics != 1)
            status = 87;
        if (status == 0 && screen->shader_caps[2].fp16 != 0)
            status = 88;
        if (status == 0 && screen->shader_caps[2].fp16_derivatives != 0)
            status = 89;
        if (status == 0 && screen->shader_caps[2].fp16_const_buffers != 1)
            status = 90;
        if (status == 0 && screen->shader_caps[2].fp16_no_denorms != 1)
            status = 91;
        if (status == 0 && screen->shader_caps[2].int16 != 0)
            status = 92;
        if (status == 0 && screen->shader_caps[2].glsl_16bit_consts != 0)
            status = 93;
        if (status == 0 && screen->shader_caps[2].glsl_16bit_load_dst != 1)
            status = 94;
        if (status == 0 && screen->shader_caps[2].tgsi_sqrt_supported != 1)
            status = 95;
        if (status == 0 && screen->shader_caps[2].tgsi_any_inout_decl_range != 0)
            status = 96;
        if (status == 0 && screen->shader_caps[3].max_instructions != 1097)
            status = 97;
        if (status == 0 && screen->shader_caps[3].max_alu_instructions != 1098)
            status = 98;
        if (status == 0 && screen->shader_caps[3].max_tex_instructions != 1099)
            status = 99;
        if (status == 0 && screen->shader_caps[3].max_tex_indirections != 1100)
            status = 100;
        if (status == 0 && screen->shader_caps[3].max_control_flow_depth != 1101)
            status = 101;
        if (status == 0 && screen->shader_caps[3].max_inputs != 1102)
            status = 102;
        if (status == 0 && screen->shader_caps[3].max_outputs != 1103)
            status = 103;
        if (status == 0 && screen->shader_caps[3].max_const_buffer0_size != 1104)
            status = 104;
        if (status == 0 && screen->shader_caps[3].max_const_buffers != 1105)
            status = 105;
        if (status == 0 && screen->shader_caps[3].max_temps != 1106)
            status = 106;
        if (status == 0 && screen->shader_caps[3].max_texture_samplers != 1107)
            status = 107;
        if (status == 0 && screen->shader_caps[3].max_sampler_views != 1108)
            status = 108;
        if (status == 0 && screen->shader_caps[3].max_shader_buffers != 1109)
            status = 109;
        if (status == 0 && screen->shader_caps[3].max_shader_images != 1110)
            status = 110;
        if (status == 0 && screen->shader_caps[3].max_hw_atomic_counters != 1111)
            status = 111;
        if (status == 0 && screen->shader_caps[3].max_hw_atomic_counter_buffers != 1112)
            status = 112;
        if (status == 0 && screen->shader_caps[3].supported_irs != 1113)
            status = 113;
        if (status == 0 && screen->shader_caps[3].cont_supported != 1)
            status = 114;
        if (status == 0 && screen->shader_caps[3].indirect_temp_addr != 1)
            status = 115;
        if (status == 0 && screen->shader_caps[3].indirect_const_addr != 0)
            status = 116;
        if (status == 0 && screen->shader_caps[3].subroutines != 0)
            status = 117;
        if (status == 0 && screen->shader_caps[3].integers != 1)
            status = 118;
        if (status == 0 && screen->shader_caps[3].int64_atomics != 1)
            status = 119;
        if (status == 0 && screen->shader_caps[3].fp16 != 0)
            status = 120;
        if (status == 0 && screen->shader_caps[3].fp16_derivatives != 0)
            status = 121;
        if (status == 0 && screen->shader_caps[3].fp16_const_buffers != 1)
            status = 122;
        if (status == 0 && screen->shader_caps[3].fp16_no_denorms != 1)
            status = 123;
        if (status == 0 && screen->shader_caps[3].int16 != 0)
            status = 124;
        if (status == 0 && screen->shader_caps[3].glsl_16bit_consts != 0)
            status = 125;
        if (status == 0 && screen->shader_caps[3].glsl_16bit_load_dst != 1)
            status = 126;
        if (status == 0 && screen->shader_caps[3].tgsi_sqrt_supported != 1)
            status = 127;
        if (status == 0 && screen->shader_caps[3].tgsi_any_inout_decl_range != 0)
            status = 128;
        if (status == 0 && screen->shader_caps[4].max_instructions != 1129)
            status = 129;
        if (status == 0 && screen->shader_caps[4].max_alu_instructions != 1130)
            status = 130;
        if (status == 0 && screen->shader_caps[4].max_tex_instructions != 1131)
            status = 131;
        if (status == 0 && screen->shader_caps[4].max_tex_indirections != 1132)
            status = 132;
        if (status == 0 && screen->shader_caps[4].max_control_flow_depth != 1133)
            status = 133;
        if (status == 0 && screen->shader_caps[4].max_inputs != 1134)
            status = 134;
        if (status == 0 && screen->shader_caps[4].max_outputs != 1135)
            status = 135;
        if (status == 0 && screen->shader_caps[4].max_const_buffer0_size != 1136)
            status = 136;
        if (status == 0 && screen->shader_caps[4].max_const_buffers != 1137)
            status = 137;
        if (status == 0 && screen->shader_caps[4].max_temps != 1138)
            status = 138;
        if (status == 0 && screen->shader_caps[4].max_texture_samplers != 1139)
            status = 139;
        if (status == 0 && screen->shader_caps[4].max_sampler_views != 1140)
            status = 140;
        if (status == 0 && screen->shader_caps[4].max_shader_buffers != 1141)
            status = 141;
        if (status == 0 && screen->shader_caps[4].max_shader_images != 1142)
            status = 142;
        if (status == 0 && screen->shader_caps[4].max_hw_atomic_counters != 1143)
            status = 143;
        if (status == 0 && screen->shader_caps[4].max_hw_atomic_counter_buffers != 1144)
            status = 144;
        if (status == 0 && screen->shader_caps[4].supported_irs != 1145)
            status = 145;
        if (status == 0 && screen->shader_caps[4].cont_supported != 1)
            status = 146;
        if (status == 0 && screen->shader_caps[4].indirect_temp_addr != 1)
            status = 147;
        if (status == 0 && screen->shader_caps[4].indirect_const_addr != 0)
            status = 148;
        if (status == 0 && screen->shader_caps[4].subroutines != 0)
            status = 149;
        if (status == 0 && screen->shader_caps[4].integers != 1)
            status = 150;
        if (status == 0 && screen->shader_caps[4].int64_atomics != 1)
            status = 151;
        if (status == 0 && screen->shader_caps[4].fp16 != 0)
            status = 152;
        if (status == 0 && screen->shader_caps[4].fp16_derivatives != 0)
            status = 153;
        if (status == 0 && screen->shader_caps[4].fp16_const_buffers != 1)
            status = 154;
        if (status == 0 && screen->shader_caps[4].fp16_no_denorms != 1)
            status = 155;
        if (status == 0 && screen->shader_caps[4].int16 != 0)
            status = 156;
        if (status == 0 && screen->shader_caps[4].glsl_16bit_consts != 0)
            status = 157;
        if (status == 0 && screen->shader_caps[4].glsl_16bit_load_dst != 1)
            status = 158;
        if (status == 0 && screen->shader_caps[4].tgsi_sqrt_supported != 1)
            status = 159;
        if (status == 0 && screen->shader_caps[4].tgsi_any_inout_decl_range != 0)
            status = 160;
        if (status == 0 && screen->shader_caps[5].max_instructions != 1161)
            status = 161;
        if (status == 0 && screen->shader_caps[5].max_alu_instructions != 1162)
            status = 162;
        if (status == 0 && screen->shader_caps[5].max_tex_instructions != 1163)
            status = 163;
        if (status == 0 && screen->shader_caps[5].max_tex_indirections != 1164)
            status = 164;
        if (status == 0 && screen->shader_caps[5].max_control_flow_depth != 1165)
            status = 165;
        if (status == 0 && screen->shader_caps[5].max_inputs != 1166)
            status = 166;
        if (status == 0 && screen->shader_caps[5].max_outputs != 1167)
            status = 167;
        if (status == 0 && screen->shader_caps[5].max_const_buffer0_size != 1168)
            status = 168;
        if (status == 0 && screen->shader_caps[5].max_const_buffers != 1169)
            status = 169;
        if (status == 0 && screen->shader_caps[5].max_temps != 1170)
            status = 170;
        if (status == 0 && screen->shader_caps[5].max_texture_samplers != 1171)
            status = 171;
        if (status == 0 && screen->shader_caps[5].max_sampler_views != 1172)
            status = 172;
        if (status == 0 && screen->shader_caps[5].max_shader_buffers != 1173)
            status = 173;
        if (status == 0 && screen->shader_caps[5].max_shader_images != 1174)
            status = 174;
        if (status == 0 && screen->shader_caps[5].max_hw_atomic_counters != 1175)
            status = 175;
        if (status == 0 && screen->shader_caps[5].max_hw_atomic_counter_buffers != 1176)
            status = 176;
        if (status == 0 && screen->shader_caps[5].supported_irs != 1177)
            status = 177;
        if (status == 0 && screen->shader_caps[5].cont_supported != 1)
            status = 178;
        if (status == 0 && screen->shader_caps[5].indirect_temp_addr != 1)
            status = 179;
        if (status == 0 && screen->shader_caps[5].indirect_const_addr != 0)
            status = 180;
        if (status == 0 && screen->shader_caps[5].subroutines != 0)
            status = 181;
        if (status == 0 && screen->shader_caps[5].integers != 1)
            status = 182;
        if (status == 0 && screen->shader_caps[5].int64_atomics != 1)
            status = 183;
        if (status == 0 && screen->shader_caps[5].fp16 != 0)
            status = 184;
        if (status == 0 && screen->shader_caps[5].fp16_derivatives != 0)
            status = 185;
        if (status == 0 && screen->shader_caps[5].fp16_const_buffers != 1)
            status = 186;
        if (status == 0 && screen->shader_caps[5].fp16_no_denorms != 1)
            status = 187;
        if (status == 0 && screen->shader_caps[5].int16 != 0)
            status = 188;
        if (status == 0 && screen->shader_caps[5].glsl_16bit_consts != 0)
            status = 189;
        if (status == 0 && screen->shader_caps[5].glsl_16bit_load_dst != 1)
            status = 190;
        if (status == 0 && screen->shader_caps[5].tgsi_sqrt_supported != 1)
            status = 191;
        if (status == 0 && screen->shader_caps[5].tgsi_any_inout_decl_range != 0)
            status = 192;
        if (status == 0 && screen->shader_caps[6].max_instructions != 1193)
            status = 193;
        if (status == 0 && screen->shader_caps[6].max_alu_instructions != 1194)
            status = 194;
        if (status == 0 && screen->shader_caps[6].max_tex_instructions != 1195)
            status = 195;
        if (status == 0 && screen->shader_caps[6].max_tex_indirections != 1196)
            status = 196;
        if (status == 0 && screen->shader_caps[6].max_control_flow_depth != 1197)
            status = 197;
        if (status == 0 && screen->shader_caps[6].max_inputs != 1198)
            status = 198;
        if (status == 0 && screen->shader_caps[6].max_outputs != 1199)
            status = 199;
        if (status == 0 && screen->shader_caps[6].max_const_buffer0_size != 1200)
            status = 200;
        if (status == 0 && screen->shader_caps[6].max_const_buffers != 1201)
            status = 201;
        if (status == 0 && screen->shader_caps[6].max_temps != 1202)
            status = 202;
        if (status == 0 && screen->shader_caps[6].max_texture_samplers != 1203)
            status = 203;
        if (status == 0 && screen->shader_caps[6].max_sampler_views != 1204)
            status = 204;
        if (status == 0 && screen->shader_caps[6].max_shader_buffers != 1205)
            status = 205;
        if (status == 0 && screen->shader_caps[6].max_shader_images != 1206)
            status = 206;
        if (status == 0 && screen->shader_caps[6].max_hw_atomic_counters != 1207)
            status = 207;
        if (status == 0 && screen->shader_caps[6].max_hw_atomic_counter_buffers != 1208)
            status = 208;
        if (status == 0 && screen->shader_caps[6].supported_irs != 1209)
            status = 209;
        if (status == 0 && screen->shader_caps[6].cont_supported != 1)
            status = 210;
        if (status == 0 && screen->shader_caps[6].indirect_temp_addr != 1)
            status = 211;
        if (status == 0 && screen->shader_caps[6].indirect_const_addr != 0)
            status = 212;
        if (status == 0 && screen->shader_caps[6].subroutines != 0)
            status = 213;
        if (status == 0 && screen->shader_caps[6].integers != 1)
            status = 214;
        if (status == 0 && screen->shader_caps[6].int64_atomics != 1)
            status = 215;
        if (status == 0 && screen->shader_caps[6].fp16 != 0)
            status = 216;
        if (status == 0 && screen->shader_caps[6].fp16_derivatives != 0)
            status = 217;
        if (status == 0 && screen->shader_caps[6].fp16_const_buffers != 1)
            status = 218;
        if (status == 0 && screen->shader_caps[6].fp16_no_denorms != 1)
            status = 219;
        if (status == 0 && screen->shader_caps[6].int16 != 0)
            status = 220;
        if (status == 0 && screen->shader_caps[6].glsl_16bit_consts != 0)
            status = 221;
        if (status == 0 && screen->shader_caps[6].glsl_16bit_load_dst != 1)
            status = 222;
        if (status == 0 && screen->shader_caps[6].tgsi_sqrt_supported != 1)
            status = 223;
        if (status == 0 && screen->shader_caps[6].tgsi_any_inout_decl_range != 0)
            status = 224;
        if (status == 0 && screen->shader_caps[7].max_instructions != 1225)
            status = 225;
        if (status == 0 && screen->shader_caps[7].max_alu_instructions != 1226)
            status = 226;
        if (status == 0 && screen->shader_caps[7].max_tex_instructions != 1227)
            status = 227;
        if (status == 0 && screen->shader_caps[7].max_tex_indirections != 1228)
            status = 228;
        if (status == 0 && screen->shader_caps[7].max_control_flow_depth != 1229)
            status = 229;
        if (status == 0 && screen->shader_caps[7].max_inputs != 1230)
            status = 230;
        if (status == 0 && screen->shader_caps[7].max_outputs != 1231)
            status = 231;
        if (status == 0 && screen->shader_caps[7].max_const_buffer0_size != 1232)
            status = 232;
        if (status == 0 && screen->shader_caps[7].max_const_buffers != 1233)
            status = 233;
        if (status == 0 && screen->shader_caps[7].max_temps != 1234)
            status = 234;
        if (status == 0 && screen->shader_caps[7].max_texture_samplers != 1235)
            status = 235;
        if (status == 0 && screen->shader_caps[7].max_sampler_views != 1236)
            status = 236;
        if (status == 0 && screen->shader_caps[7].max_shader_buffers != 1237)
            status = 237;
        if (status == 0 && screen->shader_caps[7].max_shader_images != 1238)
            status = 238;
        if (status == 0 && screen->shader_caps[7].max_hw_atomic_counters != 1239)
            status = 239;
        if (status == 0 && screen->shader_caps[7].max_hw_atomic_counter_buffers != 1240)
            status = 240;
        if (status == 0 && screen->shader_caps[7].supported_irs != 1241)
            status = 241;
        if (status == 0 && screen->shader_caps[7].cont_supported != 1)
            status = 242;
        if (status == 0 && screen->shader_caps[7].indirect_temp_addr != 1)
            status = 243;
        if (status == 0 && screen->shader_caps[7].indirect_const_addr != 0)
            status = 244;
        if (status == 0 && screen->shader_caps[7].subroutines != 0)
            status = 245;
        if (status == 0 && screen->shader_caps[7].integers != 1)
            status = 246;
        if (status == 0 && screen->shader_caps[7].int64_atomics != 1)
            status = 247;
        if (status == 0 && screen->shader_caps[7].fp16 != 0)
            status = 248;
        if (status == 0 && screen->shader_caps[7].fp16_derivatives != 0)
            status = 249;
        if (status == 0 && screen->shader_caps[7].fp16_const_buffers != 1)
            status = 250;
        if (status == 0 && screen->shader_caps[7].fp16_no_denorms != 1)
            status = 251;
        if (status == 0 && screen->shader_caps[7].int16 != 0)
            status = 252;
        if (status == 0 && screen->shader_caps[7].glsl_16bit_consts != 0)
            status = 253;
        if (status == 0 && screen->shader_caps[7].glsl_16bit_load_dst != 1)
            status = 254;
        if (status == 0 && screen->shader_caps[7].tgsi_sqrt_supported != 1)
            status = 255;
        if (status == 0 && screen->shader_caps[7].tgsi_any_inout_decl_range != 0)
            status = 256;
        if (status == 0 && screen->caps.graphics != 0)
            status = 257;
        if (status == 0 && screen->caps.npot_textures != 1)
            status = 258;
        if (status == 0 && screen->caps.anisotropic_filter != 1)
            status = 259;
        if (status == 0 && screen->caps.occlusion_query != 0)
            status = 260;
        if (status == 0 && screen->caps.query_time_elapsed != 0)
            status = 261;
        if (status == 0 && screen->caps.texture_shadow_map != 1)
            status = 262;
        if (status == 0 && screen->caps.texture_swizzle != 1)
            status = 263;
        if (status == 0 && screen->caps.texture_mirror_clamp != 0)
            status = 264;
        if (status == 0 && screen->caps.blend_equation_separate != 0)
            status = 265;
        if (status == 0 && screen->caps.primitive_restart != 1)
            status = 266;
        if (status == 0 && screen->caps.primitive_restart_fixed_index != 1)
            status = 267;
        if (status == 0 && screen->caps.indep_blend_enable != 0)
            status = 268;
        if (status == 0 && screen->caps.indep_blend_func != 0)
            status = 269;
        if (status == 0 && screen->caps.fs_coord_origin_upper_left != 1)
            status = 270;
        if (status == 0 && screen->caps.fs_coord_origin_lower_left != 1)
            status = 271;
        if (status == 0 && screen->caps.fs_coord_pixel_center_half_integer != 0)
            status = 272;
        if (status == 0 && screen->caps.fs_coord_pixel_center_integer != 0)
            status = 273;
        if (status == 0 && screen->caps.depth_clip_disable != 1)
            status = 274;
        if (status == 0 && screen->caps.depth_clip_disable_separate != 1)
            status = 275;
        if (status == 0 && screen->caps.depth_clamp_enable != 0)
            status = 276;
        if (status == 0 && screen->caps.shader_stencil_export != 0)
            status = 277;
        if (status == 0 && screen->caps.vs_instanceid != 1)
            status = 278;
        if (status == 0 && screen->caps.vertex_element_instance_divisor != 1)
            status = 279;
        if (status == 0 && screen->caps.fragment_color_clamped != 0)
            status = 280;
        if (status == 0 && screen->caps.mixed_colorbuffer_formats != 0)
            status = 281;
        if (status == 0 && screen->caps.seamless_cube_map != 1)
            status = 282;
        if (status == 0 && screen->caps.seamless_cube_map_per_texture != 1)
            status = 283;
        if (status == 0 && screen->caps.conditional_render != 0)
            status = 284;
        if (status == 0 && screen->caps.texture_barrier != 0)
            status = 285;
        if (status == 0 && screen->caps.stream_output_pause_resume != 1)
            status = 286;
        if (status == 0 && screen->caps.tgsi_can_compact_constants != 1)
            status = 287;
        if (status == 0 && screen->caps.vertex_color_unclamped != 0)
            status = 288;
        if (status == 0 && screen->caps.vertex_color_clamped != 0)
            status = 289;
        if (status == 0 && screen->caps.quads_follow_provoking_vertex_convention != 1)
            status = 290;
        if (status == 0 && screen->caps.user_vertex_buffers != 1)
            status = 291;
        if (status == 0 && screen->caps.compute != 0)
            status = 292;
        if (status == 0 && screen->caps.start_instance != 0)
            status = 293;
        if (status == 0 && screen->caps.query_timestamp != 1)
            status = 294;
        if (status == 0 && screen->caps.texture_multisample != 1)
            status = 295;
        if (status == 0 && screen->caps.cube_map_array != 0)
            status = 296;
        if (status == 0 && screen->caps.texture_buffer_objects != 0)
            status = 297;
        if (status == 0 && screen->caps.buffer_sampler_view_rgba_only != 1)
            status = 298;
        if (status == 0 && screen->caps.tgsi_texcoord != 1)
            status = 299;
        if (status == 0 && screen->caps.query_pipeline_statistics != 0)
            status = 300;
        if (status == 0 && screen->caps.mixed_framebuffer_sizes != 0)
            status = 301;
        if (status == 0 && screen->caps.vs_layer_viewport != 1)
            status = 302;
        if (status == 0 && screen->caps.texture_gather_sm5 != 1)
            status = 303;
        if (status == 0 && screen->caps.buffer_map_persistent_coherent != 0)
            status = 304;
        if (status == 0 && screen->caps.fake_sw_msaa != 0)
            status = 305;
        if (status == 0 && screen->caps.texture_query_lod != 1)
            status = 306;
        if (status == 0 && screen->caps.sample_shading != 1)
            status = 307;
        if (status == 0 && screen->caps.texture_gather_offsets != 0)
            status = 308;
        if (status == 0 && screen->caps.vs_window_space_position != 0)
            status = 309;
        if (status == 0 && screen->caps.draw_indirect != 1)
            status = 310;
        if (status == 0 && screen->caps.fs_fine_derivative != 1)
            status = 311;
        if (status == 0 && screen->caps.uma != 0)
            status = 312;
        if (status == 0 && screen->caps.conditional_render_inverted != 0)
            status = 313;
        if (status == 0 && screen->caps.sampler_view_target != 1)
            status = 314;
        if (status == 0 && screen->caps.clip_halfz != 1)
            status = 315;
        if (status == 0 && screen->caps.polygon_offset_clamp != 0)
            status = 316;
        if (status == 0 && screen->caps.multisample_z_resolve != 0)
            status = 317;
        if (status == 0 && screen->caps.resource_from_user_memory != 1)
            status = 318;
        if (status == 0 && screen->caps.resource_from_user_memory_compute_only != 1)
            status = 319;
        if (status == 0 && screen->caps.device_reset_status_query != 0)
            status = 320;
        if (status == 0 && screen->caps.texture_float_linear != 0)
            status = 321;
        if (status == 0 && screen->caps.texture_half_float_linear != 1)
            status = 322;
        if (status == 0 && screen->caps.depth_bounds_test != 1)
            status = 323;
        if (status == 0 && screen->caps.native_fp32_depth != 0)
            status = 324;
        if (status == 0 && screen->caps.texture_query_samples != 0)
            status = 325;
        if (status == 0 && screen->caps.force_persample_interp != 1)
            status = 326;
        if (status == 0 && screen->caps.shareable_shaders != 1)
            status = 327;
        if (status == 0 && screen->caps.copy_between_compressed_and_plain_formats != 0)
            status = 328;
        if (status == 0 && screen->caps.clear_scissored != 0)
            status = 329;
        if (status == 0 && screen->caps.clear_masked != 1)
            status = 330;
        if (status == 0 && screen->caps.draw_parameters != 1)
            status = 331;
        if (status == 0 && screen->caps.shader_pack_half_float != 0)
            status = 332;
        if (status == 0 && screen->caps.multi_draw_indirect != 0)
            status = 333;
        if (status == 0 && screen->caps.multi_draw_indirect_params != 1)
            status = 334;
        if (status == 0 && screen->caps.multi_draw_indirect_partial_stride != 1)
            status = 335;
        if (status == 0 && screen->caps.fs_position_is_sysval != 0)
            status = 336;
        if (status == 0 && screen->caps.fs_point_is_sysval != 0)
            status = 337;
        if (status == 0 && screen->caps.fs_face_is_integer_sysval != 1)
            status = 338;
        if (status == 0 && screen->caps.invalidate_buffer != 1)
            status = 339;
        if (status == 0 && screen->caps.generate_mipmap != 0)
            status = 340;
        if (status == 0 && screen->caps.string_marker != 0)
            status = 341;
        if (status == 0 && screen->caps.surface_no_compress != 1)
            status = 342;
        if (status == 0 && screen->caps.surface_reinterpret_blocks != 1)
            status = 343;
        if (status == 0 && screen->caps.compressed_surface_reinterpret_blocks_layered != 0)
            status = 344;
        if (status == 0 && screen->caps.query_buffer_object != 0)
            status = 345;
        if (status == 0 && screen->caps.query_memory_info != 1)
            status = 346;
        if (status == 0 && screen->caps.framebuffer_no_attachment != 1)
            status = 347;
        if (status == 0 && screen->caps.robust_buffer_access_behavior != 0)
            status = 348;
        if (status == 0 && screen->caps.cull_distance != 0)
            status = 349;
        if (status == 0 && screen->caps.shader_group_vote != 1)
            status = 350;
        if (status == 0 && screen->caps.shader_array_components != 1)
            status = 351;
        if (status == 0 && screen->caps.stream_output_interleave_buffers != 0)
            status = 352;
        if (status == 0 && screen->caps.native_fence_fd != 0)
            status = 353;
        if (status == 0 && screen->caps.glsl_tess_levels_as_inputs != 1)
            status = 354;
        if (status == 0 && screen->caps.legacy_math_rules != 1)
            status = 355;
        if (status == 0 && screen->caps.fp16 != 0)
            status = 356;
        if (status == 0 && screen->caps.doubles != 0)
            status = 357;
        if (status == 0 && screen->caps.int64 != 1)
            status = 358;
        if (status == 0 && screen->caps.shader_clock != 1)
            status = 359;
        if (status == 0 && screen->caps.shader_realtime_clock != 0)
            status = 360;
        if (status == 0 && screen->caps.polygon_mode_fill_rectangle != 0)
            status = 361;
        if (status == 0 && screen->caps.shader_ballot != 1)
            status = 362;
        if (status == 0 && screen->caps.tes_layer_viewport != 1)
            status = 363;
        if (status == 0 && screen->caps.can_bind_const_buffer_as_vertex != 0)
            status = 364;
        if (status == 0 && screen->caps.allow_mapped_buffers_during_execution != 0)
            status = 365;
        if (status == 0 && screen->caps.post_depth_coverage != 1)
            status = 366;
        if (status == 0 && screen->caps.bindless_texture != 1)
            status = 367;
        if (status == 0 && screen->caps.nir_samplers_as_deref != 0)
            status = 368;
        if (status == 0 && screen->caps.query_so_overflow != 0)
            status = 369;
        if (status == 0 && screen->caps.memobj != 1)
            status = 370;
        if (status == 0 && screen->caps.load_constbuf != 1)
            status = 371;
        if (status == 0 && screen->caps.tile_raster_order != 0)
            status = 372;
        if (status == 0 && screen->caps.signed_vertex_buffer_offset != 0)
            status = 373;
        if (status == 0 && screen->caps.fence_signal != 1)
            status = 374;
        if (status == 0 && screen->caps.packed_uniforms != 1)
            status = 375;
        if (status == 0 && screen->caps.conservative_raster_post_snap_triangles != 0)
            status = 376;
        if (status == 0 && screen->caps.conservative_raster_post_snap_points_lines != 0)
            status = 377;
        if (status == 0 && screen->caps.conservative_raster_pre_snap_triangles != 1)
            status = 378;
        if (status == 0 && screen->caps.conservative_raster_pre_snap_points_lines != 1)
            status = 379;
        if (status == 0 && screen->caps.conservative_raster_post_depth_coverage != 0)
            status = 380;
        if (status == 0 && screen->caps.conservative_raster_inner_coverage != 0)
            status = 381;
        if (status == 0 && screen->caps.programmable_sample_locations != 1)
            status = 382;
        if (status == 0 && screen->caps.texture_mirror_clamp_to_edge != 1)
            status = 383;
        if (status == 0 && screen->caps.surface_sample_count != 0)
            status = 384;
        if (status == 0 && screen->caps.image_atomic_float_add != 0)
            status = 385;
        if (status == 0 && screen->caps.query_pipeline_statistics_single != 1)
            status = 386;
        if (status == 0 && screen->caps.dest_surface_srgb_control != 1)
            status = 387;
        if (status == 0 && screen->caps.compute_grid_info_last_block != 0)
            status = 388;
        if (status == 0 && screen->caps.compute_shader_derivatives != 0)
            status = 389;
        if (status == 0 && screen->caps.image_load_formatted != 1)
            status = 390;
        if (status == 0 && screen->caps.image_store_formatted != 1)
            status = 391;
        if (status == 0 && screen->caps.throttle != 0)
            status = 392;
        if (status == 0 && screen->caps.cl_gl_sharing != 0)
            status = 393;
        if (status == 0 && screen->caps.prefer_compute_for_multimedia != 1)
            status = 394;
        if (status == 0 && screen->caps.fragment_shader_interlock != 1)
            status = 395;
        if (status == 0 && screen->caps.fbfetch_coherent != 0)
            status = 396;
        if (status == 0 && screen->caps.atomic_float_minmax != 0)
            status = 397;
        if (status == 0 && screen->caps.fragment_shader_texture_lod != 1)
            status = 398;
        if (status == 0 && screen->caps.fragment_shader_derivatives != 1)
            status = 399;
        if (status == 0 && screen->caps.fragment_shader_depth != 0)
            status = 400;
        if (status == 0 && screen->caps.texture_shadow_lod != 0)
            status = 401;
        if (status == 0 && screen->caps.shader_samples_identical != 1)
            status = 402;
        if (status == 0 && screen->caps.image_atomic_inc_wrap != 1)
            status = 403;
        if (status == 0 && screen->caps.prefer_imm_arrays_as_constbuf != 0)
            status = 404;
        if (status == 0 && screen->caps.gl_spirv != 0)
            status = 405;
        if (status == 0 && screen->caps.gl_spirv_variable_pointers != 1)
            status = 406;
        if (status == 0 && screen->caps.demote_to_helper_invocation != 1)
            status = 407;
        if (status == 0 && screen->caps.tgsi_tg4_component_in_swizzle != 0)
            status = 408;
        if (status == 0 && screen->caps.flatshade != 0)
            status = 409;
        if (status == 0 && screen->caps.alpha_test != 1)
            status = 410;
        if (status == 0 && screen->caps.two_sided_color != 1)
            status = 411;
        if (status == 0 && screen->caps.opencl_integer_functions != 0)
            status = 412;
        if (status == 0 && screen->caps.integer_multiply_32x16 != 0)
            status = 413;
        if (status == 0 && screen->caps.frontend_noop != 1)
            status = 414;
        if (status == 0 && screen->caps.nir_images_as_deref != 1)
            status = 415;
        if (status == 0 && screen->caps.packed_stream_output != 0)
            status = 416;
        if (status == 0 && screen->caps.viewport_transform_lowered != 0)
            status = 417;
        if (status == 0 && screen->caps.psiz_clamped != 1)
            status = 418;
        if (status == 0 && screen->caps.viewport_swizzle != 1)
            status = 419;
        if (status == 0 && screen->caps.system_svm != 0)
            status = 420;
        if (status == 0 && screen->caps.viewport_mask != 0)
            status = 421;
        if (status == 0 && screen->caps.alpha_to_coverage_dither_control != 1)
            status = 422;
        if (status == 0 && screen->caps.map_unsynchronized_thread_safe != 1)
            status = 423;
        if (status == 0 && screen->caps.nir_atomics_as_deref != 0)
            status = 424;
        if (status == 0 && screen->caps.no_clip_on_copy_tex != 0)
            status = 425;
        if (status == 0 && screen->caps.shader_atomic_int64 != 1)
            status = 426;
        if (status == 0 && screen->caps.device_protected_surface != 1)
            status = 427;
        if (status == 0 && screen->caps.prefer_real_buffer_in_constbuf0 != 0)
            status = 428;
        if (status == 0 && screen->caps.gl_clamp != 0)
            status = 429;
        if (status == 0 && screen->caps.texrect != 1)
            status = 430;
        if (status == 0 && screen->caps.sampler_reduction_minmax != 1)
            status = 431;
        if (status == 0 && screen->caps.sampler_reduction_minmax_arb != 0)
            status = 432;
        if (status == 0 && screen->caps.allow_dynamic_vao_fastpath != 0)
            status = 433;
        if (status == 0 && screen->caps.emulate_nonfixed_primitive_restart != 1)
            status = 434;
        if (status == 0 && screen->caps.prefer_back_buffer_reuse != 1)
            status = 435;
        if (status == 0 && screen->caps.draw_vertex_state != 0)
            status = 436;
        if (status == 0 && screen->caps.prefer_pot_aligned_varyings != 0)
            status = 437;
        if (status == 0 && screen->caps.sparse_texture_full_array_cube_mipmaps != 1)
            status = 438;
        if (status == 0 && screen->caps.query_sparse_texture_residency != 1)
            status = 439;
        if (status == 0 && screen->caps.clamp_sparse_texture_lod != 0)
            status = 440;
        if (status == 0 && screen->caps.allow_draw_out_of_order != 0)
            status = 441;
        if (status == 0 && screen->caps.hardware_gl_select != 1)
            status = 442;
        if (status == 0 && screen->caps.dithering != 1)
            status = 443;
        if (status == 0 && screen->caps.fbfetch_zs != 0)
            status = 444;
        if (status == 0 && screen->caps.timeline_semaphore_import != 0)
            status = 445;
        if (status == 0 && screen->caps.device_protected_context != 1)
            status = 446;
        if (status == 0 && screen->caps.allow_glthread_buffer_subdata_opt != 1)
            status = 447;
        if (status == 0 && screen->caps.null_textures != 0)
            status = 448;
        if (status == 0 && screen->caps.astc_void_extents_need_denorm_flush != 0)
            status = 449;
        if (status == 0 && screen->caps.validate_all_dirty_states != 1)
            status = 450;
        if (status == 0 && screen->caps.has_const_bw != 1)
            status = 451;
        if (status == 0 && screen->caps.performance_monitor != 0)
            status = 452;
        if (status == 0 && screen->caps.texture_sampler_independent != 0)
            status = 453;
        if (status == 0 && screen->caps.astc_decode_mode != 1)
            status = 454;
        if (status == 0 && screen->caps.shader_subgroup_quad_all_stages != 1)
            status = 455;
        if (status == 0 && screen->caps.call_finalize_nir_in_linker != 0)
            status = 456;
        if (status == 0 && screen->caps.mesh_shader != 0)
            status = 457;
        if (status == 0 && screen->caps.representative_fragment_test != 1)
            status = 458;
        if (status == 0 && screen->caps.prefer_persp != 1)
            status = 459;
        if (status == 0 && screen->caps.blit_3d != 0)
            status = 460;
        if (status == 0 && screen->caps.glsl_bindless_handles_are_32bit != 0)
            status = 461;
        if (status == 0 && screen->caps.polygon_stipple != 1)
            status = 462;
        if (status == 0 && screen->caps.disable_custom_border_color_caching != 1)
            status = 463;
        if (status == 0 && screen->caps.needs_split_planar_samplers != 0)
            status = 464;
        if (status == 0 && screen->caps.accelerated != 1465)
            status = 465;
        if (status == 0 && screen->caps.min_texel_offset != 1466)
            status = 466;
        if (status == 0 && screen->caps.max_texel_offset != 1467)
            status = 467;
        if (status == 0 && screen->caps.min_texture_gather_offset != 1468)
            status = 468;
        if (status == 0 && screen->caps.max_texture_gather_offset != 1469)
            status = 469;
        if (status == 0 && screen->caps.max_dual_source_render_targets != 1470)
            status = 470;
        if (status == 0 && screen->caps.max_render_targets != 1471)
            status = 471;
        if (status == 0 && screen->caps.max_texture_2d_size != 1472)
            status = 472;
        if (status == 0 && screen->caps.max_texture_3d_levels != 1473)
            status = 473;
        if (status == 0 && screen->caps.max_texture_cube_levels != 1474)
            status = 474;
        if (status == 0 && screen->caps.max_stream_output_buffers != 1475)
            status = 475;
        if (status == 0 && screen->caps.max_texture_array_layers != 1476)
            status = 476;
        if (status == 0 && screen->caps.max_stream_output_separate_components != 1477)
            status = 477;
        if (status == 0 && screen->caps.max_stream_output_interleaved_components != 1478)
            status = 478;
        if (status == 0 && screen->caps.glsl_feature_level != 1479)
            status = 479;
        if (status == 0 && screen->caps.glsl_feature_level_compatibility != 1480)
            status = 480;
        if (status == 0 && screen->caps.essl_feature_level != 1481)
            status = 481;
        if (status == 0 && screen->caps.constant_buffer_offset_alignment != 1482)
            status = 482;
        if (status == 0 && screen->caps.timer_resolution != 1483)
            status = 483;
        if (status == 0 && screen->caps.min_map_buffer_alignment != 1484)
            status = 484;
        if (status == 0 && screen->caps.texture_buffer_offset_alignment != 1485)
            status = 485;
        if (status == 0 && screen->caps.linear_image_pitch_alignment != 1486)
            status = 486;
        if (status == 0 && screen->caps.linear_image_base_address_alignment != 1487)
            status = 487;
        if (status == 0 && screen->caps.texture_transfer_modes != 1488)
            status = 488;
        if (status == 0 && screen->caps.texture_border_color_quirk != 1489)
            status = 489;
        if (status == 0 && screen->caps.max_texel_buffer_elements != 1490)
            status = 490;
        if (status == 0 && screen->caps.max_viewports != 1491)
            status = 491;
        if (status == 0 && screen->caps.max_geometry_output_vertices != 1492)
            status = 492;
        if (status == 0 && screen->caps.max_geometry_total_output_components != 1493)
            status = 493;
        if (status == 0 && screen->caps.max_texture_gather_components != 1494)
            status = 494;
        if (status == 0 && screen->caps.max_vertex_streams != 1495)
            status = 495;
        if (status == 0 && screen->caps.vendor_id != 1496)
            status = 496;
        if (status == 0 && screen->caps.device_id != 1497)
            status = 497;
        if (status == 0 && screen->caps.video_memory != 1498)
            status = 498;
        if (status == 0 && screen->caps.max_vertex_attrib_stride != 1499)
            status = 499;
        if (status == 0 && screen->caps.max_shader_patch_varyings != 1500)
            status = 500;
        if (status == 0 && screen->caps.shader_buffer_offset_alignment != 1501)
            status = 501;
        if (status == 0 && screen->caps.pci_group != 1502)
            status = 502;
        if (status == 0 && screen->caps.pci_bus != 1503)
            status = 503;
        if (status == 0 && screen->caps.pci_device != 1504)
            status = 504;
        if (status == 0 && screen->caps.pci_function != 1505)
            status = 505;
        if (status == 0 && screen->caps.max_window_rectangles != 1506)
            status = 506;
        if (status == 0 && screen->caps.viewport_subpixel_bits != 1507)
            status = 507;
        if (status == 0 && screen->caps.rasterizer_subpixel_bits != 1508)
            status = 508;
        if (status == 0 && screen->caps.mixed_color_depth_bits != 1509)
            status = 509;
        if (status == 0 && screen->caps.fbfetch != 1510)
            status = 510;
        if (status == 0 && screen->caps.blend_equation_advanced != 1511)
            status = 511;
        if (status == 0 && screen->caps.sparse_buffer_page_size != 1512)
            status = 512;
        if (status == 0 && screen->caps.max_combined_shader_output_resources != 1513)
            status = 513;
        if (status == 0 && screen->caps.framebuffer_msaa_constraints != 1514)
            status = 514;
        if (status == 0 && screen->caps.context_priority_mask != 1515)
            status = 515;
        if (status == 0 && screen->caps.constbuf0_flags != 1516)
            status = 516;
        if (status == 0 && screen->caps.max_conservative_raster_subpixel_precision_bias != 1517)
            status = 517;
        if (status == 0 && screen->caps.max_gs_invocations != 1518)
            status = 518;
        if (status == 0 && screen->caps.max_shader_buffer_size != 1519)
            status = 519;
        if (status == 0 && screen->caps.max_combined_shader_buffers != 1520)
            status = 520;
        if (status == 0 && screen->caps.max_combined_image_uniforms != 1521)
            status = 521;
        if (status == 0 && screen->caps.max_combined_hw_atomic_counters != 1522)
            status = 522;
        if (status == 0 && screen->caps.max_combined_hw_atomic_counter_buffers != 1523)
            status = 523;
        if (status == 0 && screen->caps.max_texture_upload_memory_budget != 1524)
            status = 524;
        if (status == 0 && screen->caps.max_vertex_element_src_offset != 1525)
            status = 525;
        if (status == 0 && screen->caps.max_varyings != 1526)
            status = 526;
        if (status == 0 && screen->caps.ignored_output_varyings[0] != 1527)
            status = 527;
        if (status == 0 && screen->caps.ignored_output_varyings[1] != 1528)
            status = 528;
        if (status == 0 && screen->caps.ignored_output_varyings[2] != 1529)
            status = 529;
        if (status == 0 && screen->caps.ignored_output_varyings[3] != 1530)
            status = 530;
        if (status == 0 && screen->caps.ignored_output_varyings[4] != 1531)
            status = 531;
        if (status == 0 && screen->caps.ignored_output_varyings[5] != 1532)
            status = 532;
        if (status == 0 && screen->caps.ignored_output_varyings[6] != 1533)
            status = 533;
        if (status == 0 && screen->caps.ignored_output_varyings[7] != 1534)
            status = 534;
        if (status == 0 && screen->caps.dmabuf != 1535)
            status = 535;
        if (status == 0 && screen->caps.clip_planes != 1536)
            status = 536;
        if (status == 0 && screen->caps.max_vertex_buffers != 1537)
            status = 537;
        if (status == 0 && screen->caps.gl_begin_end_buffer_size != 1538)
            status = 538;
        if (status == 0 && screen->caps.glsl_zero_init != 1539)
            status = 539;
        if (status == 0 && screen->caps.max_texture_mb != 1540)
            status = 540;
        if (status == 0 && screen->caps.supported_prim_modes != 1541)
            status = 541;
        if (status == 0 && screen->caps.supported_prim_modes_with_restart != 1542)
            status = 542;
        if (status == 0 && screen->caps.max_sparse_texture_size != 1543)
            status = 543;
        if (status == 0 && screen->caps.max_sparse_3d_texture_size != 1544)
            status = 544;
        if (status == 0 && screen->caps.max_sparse_array_texture_layers != 1545)
            status = 545;
        if (status == 0 && screen->caps.max_constant_buffer_size != 1546)
            status = 546;
        if (status == 0 && screen->caps.query_timestamp_bits != 1547)
            status = 547;
        if (status == 0 && screen->caps.shader_subgroup_size != 1548)
            status = 548;
        if (status == 0 && screen->caps.shader_subgroup_supported_stages != 1549)
            status = 549;
        if (status == 0 && screen->caps.shader_subgroup_supported_features != 1550)
            status = 550;
        if (status == 0 && screen->caps.shader_pixel_local_storage_size != 1551)
            status = 551;
        if (status == 0 && screen->caps.shader_pixel_local_storage_fast_size != 1552)
            status = 552;
        if (status == 0 && screen->caps.multiview != 1553)
            status = 553;
        if (status == 0 && screen->caps.max_label_length != 1554)
            status = 554;
        if (status == 0 && screen->caps.max_timeline_semaphore_difference != 1555)
            status = 555;
        if (status == 0 && screen->caps.map_client_pointer != 1556)
            status = 556;
        if (status == 0 && screen->caps.min_vma != 1557)
            status = 557;
        if (status == 0 && screen->caps.max_vma != 1558)
            status = 558;
        if (status == 0 && screen->caps.hw_clear_buffer_sizes != 1559)
            status = 559;
        if (status == 0 && screen->caps.device_type != 1560)
            status = 560;
        if (status == 0 && screen->caps.vertex_input_alignment != 1561)
            status = 561;
        if (status == 0 && screen->caps.endianness != 1562)
            status = 562;
        if (status == 0 && screen->caps.point_size_fixed != 1563)
            status = 563;
        if (status == 0 && screen->caps.min_line_width != 1564.25f)
            status = 564;
        if (status == 0 && screen->caps.min_line_width_aa != 1565.25f)
            status = 565;
        if (status == 0 && screen->caps.max_line_width != 1566.25f)
            status = 566;
        if (status == 0 && screen->caps.max_line_width_aa != 1567.25f)
            status = 567;
        if (status == 0 && screen->caps.line_width_granularity != 1568.25f)
            status = 568;
        if (status == 0 && screen->caps.min_point_size != 1569.25f)
            status = 569;
        if (status == 0 && screen->caps.min_point_size_aa != 1570.25f)
            status = 570;
        if (status == 0 && screen->caps.max_point_size != 1571.25f)
            status = 571;
        if (status == 0 && screen->caps.max_point_size_aa != 1572.25f)
            status = 572;
        if (status == 0 && screen->caps.point_size_granularity != 1573.25f)
            status = 573;
        if (status == 0 && screen->caps.max_texture_anisotropy != 1574.25f)
            status = 574;
        if (status == 0 && screen->caps.max_texture_lod_bias != 1575.25f)
            status = 575;
        if (status == 0 && screen->caps.min_conservative_raster_dilate != 1576.25f)
            status = 576;
        if (status == 0 && screen->caps.max_conservative_raster_dilate != 1577.25f)
            status = 577;
        if (status == 0 && screen->caps.conservative_raster_dilate_granularity != 1578.25f)
            status = 578;
        if (status == 0 && screen->caps.mesh.max_task_work_group_total_count != 1579)
            status = 579;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_total_count != 1580)
            status = 580;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_invocations != 1581)
            status = 581;
        if (status == 0 && screen->caps.mesh.max_task_work_group_invocations != 1582)
            status = 582;
        if (status == 0 && screen->caps.mesh.max_task_payload_size != 1583)
            status = 583;
        if (status == 0 && screen->caps.mesh.max_task_shared_memory_size != 1584)
            status = 584;
        if (status == 0 && screen->caps.mesh.max_mesh_shared_memory_size != 1585)
            status = 585;
        if (status == 0 && screen->caps.mesh.max_task_payload_and_shared_memory_size != 1586)
            status = 586;
        if (status == 0 && screen->caps.mesh.max_mesh_payload_and_shared_memory_size != 1587)
            status = 587;
        if (status == 0 && screen->caps.mesh.max_mesh_output_memory_size != 1588)
            status = 588;
        if (status == 0 && screen->caps.mesh.max_mesh_payload_and_output_memory_size != 1589)
            status = 589;
        if (status == 0 && screen->caps.mesh.max_mesh_output_vertices != 1590)
            status = 590;
        if (status == 0 && screen->caps.mesh.max_mesh_output_primitives != 1591)
            status = 591;
        if (status == 0 && screen->caps.mesh.max_mesh_output_components != 1592)
            status = 592;
        if (status == 0 && screen->caps.mesh.max_mesh_output_layers != 1593)
            status = 593;
        if (status == 0 && screen->caps.mesh.max_mesh_multiview_view_count != 1594)
            status = 594;
        if (status == 0 && screen->caps.mesh.mesh_output_per_vertex_granularity != 1595)
            status = 595;
        if (status == 0 && screen->caps.mesh.mesh_output_per_primitive_granularity != 1596)
            status = 596;
        if (status == 0 && screen->caps.mesh.max_preferred_task_work_group_invocations != 1597)
            status = 597;
        if (status == 0 && screen->caps.mesh.max_preferred_mesh_work_group_invocations != 1598)
            status = 598;
        if (status == 0 && screen->caps.mesh.mesh_prefers_local_invocation_vertex_output != 1)
            status = 599;
        if (status == 0 && screen->caps.mesh.mesh_prefers_local_invocation_primitive_output != 0)
            status = 600;
        if (status == 0 && screen->caps.mesh.mesh_prefers_compact_vertex_output != 0)
            status = 601;
        if (status == 0 && screen->caps.mesh.mesh_prefers_compact_primitive_output != 1)
            status = 602;
        if (status == 0 && screen->caps.mesh.max_task_work_group_count[0] != 1603)
            status = 603;
        if (status == 0 && screen->caps.mesh.max_task_work_group_count[1] != 1604)
            status = 604;
        if (status == 0 && screen->caps.mesh.max_task_work_group_count[2] != 1605)
            status = 605;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_count[0] != 1606)
            status = 606;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_count[1] != 1607)
            status = 607;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_count[2] != 1608)
            status = 608;
        if (status == 0 && screen->caps.mesh.max_task_work_group_size[0] != 1609)
            status = 609;
        if (status == 0 && screen->caps.mesh.max_task_work_group_size[1] != 1610)
            status = 610;
        if (status == 0 && screen->caps.mesh.max_task_work_group_size[2] != 1611)
            status = 611;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_size[0] != 1612)
            status = 612;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_size[1] != 1613)
            status = 613;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_size[2] != 1614)
            status = 614;
        if (status == 0 && screen->caps.mesh.pipeline_statistic_queries != 1)
            status = 615;
        if (status == 0 && screen->compute_caps.address_bits != 1616)
            status = 616;
        if (status == 0 && screen->compute_caps.grid_dimension != 1617)
            status = 617;
        if (status == 0 && screen->compute_caps.max_grid_size[0] != 1618)
            status = 618;
        if (status == 0 && screen->compute_caps.max_grid_size[1] != 1619)
            status = 619;
        if (status == 0 && screen->compute_caps.max_grid_size[2] != 1620)
            status = 620;
        if (status == 0 && screen->compute_caps.max_block_size[0] != 1621)
            status = 621;
        if (status == 0 && screen->compute_caps.max_block_size[1] != 1622)
            status = 622;
        if (status == 0 && screen->compute_caps.max_block_size[2] != 1623)
            status = 623;
        if (status == 0 && screen->compute_caps.max_threads_per_block != 1624)
            status = 624;
        if (status == 0 && screen->compute_caps.max_local_size != 1625)
            status = 625;
        if (status == 0 && screen->compute_caps.max_clock_frequency != 1626)
            status = 626;
        if (status == 0 && screen->compute_caps.max_compute_units != 1627)
            status = 627;
        if (status == 0 && screen->compute_caps.max_subgroups != 1628)
            status = 628;
        if (status == 0 && screen->compute_caps.subgroup_sizes != 1629)
            status = 629;
        if (status == 0 && screen->compute_caps.max_variable_threads_per_block != 1630)
            status = 630;
        if (status == 0 && screen->compute_caps.max_mem_alloc_size != 1631)
            status = 631;
        if (status == 0 && screen->compute_caps.max_global_size != 1632)
            status = 632;
    }
    if (seed == 1) {
        if (status == 0 && screen->shader_caps[0].max_instructions != 1003)
            status = 633;
        if (status == 0 && screen->shader_caps[0].max_alu_instructions != 1004)
            status = 634;
        if (status == 0 && screen->shader_caps[0].max_tex_instructions != 1005)
            status = 635;
        if (status == 0 && screen->shader_caps[0].max_tex_indirections != 1006)
            status = 636;
        if (status == 0 && screen->shader_caps[0].max_control_flow_depth != 1007)
            status = 637;
        if (status == 0 && screen->shader_caps[0].max_inputs != 1008)
            status = 638;
        if (status == 0 && screen->shader_caps[0].max_outputs != 1009)
            status = 639;
        if (status == 0 && screen->shader_caps[0].max_const_buffer0_size != 1010)
            status = 640;
        if (status == 0 && screen->shader_caps[0].max_const_buffers != 1011)
            status = 641;
        if (status == 0 && screen->shader_caps[0].max_temps != 1012)
            status = 642;
        if (status == 0 && screen->shader_caps[0].max_texture_samplers != 1013)
            status = 643;
        if (status == 0 && screen->shader_caps[0].max_sampler_views != 1014)
            status = 644;
        if (status == 0 && screen->shader_caps[0].max_shader_buffers != 1015)
            status = 645;
        if (status == 0 && screen->shader_caps[0].max_shader_images != 1016)
            status = 646;
        if (status == 0 && screen->shader_caps[0].max_hw_atomic_counters != 1017)
            status = 647;
        if (status == 0 && screen->shader_caps[0].max_hw_atomic_counter_buffers != 1018)
            status = 648;
        if (status == 0 && screen->shader_caps[0].supported_irs != 1019)
            status = 649;
        if (status == 0 && screen->shader_caps[0].cont_supported != 0)
            status = 650;
        if (status == 0 && screen->shader_caps[0].indirect_temp_addr != 0)
            status = 651;
        if (status == 0 && screen->shader_caps[0].indirect_const_addr != 1)
            status = 652;
        if (status == 0 && screen->shader_caps[0].subroutines != 1)
            status = 653;
        if (status == 0 && screen->shader_caps[0].integers != 0)
            status = 654;
        if (status == 0 && screen->shader_caps[0].int64_atomics != 0)
            status = 655;
        if (status == 0 && screen->shader_caps[0].fp16 != 1)
            status = 656;
        if (status == 0 && screen->shader_caps[0].fp16_derivatives != 1)
            status = 657;
        if (status == 0 && screen->shader_caps[0].fp16_const_buffers != 0)
            status = 658;
        if (status == 0 && screen->shader_caps[0].fp16_no_denorms != 0)
            status = 659;
        if (status == 0 && screen->shader_caps[0].int16 != 1)
            status = 660;
        if (status == 0 && screen->shader_caps[0].glsl_16bit_consts != 1)
            status = 661;
        if (status == 0 && screen->shader_caps[0].glsl_16bit_load_dst != 0)
            status = 662;
        if (status == 0 && screen->shader_caps[0].tgsi_sqrt_supported != 0)
            status = 663;
        if (status == 0 && screen->shader_caps[0].tgsi_any_inout_decl_range != 1)
            status = 664;
        if (status == 0 && screen->shader_caps[1].max_instructions != 1035)
            status = 665;
        if (status == 0 && screen->shader_caps[1].max_alu_instructions != 1036)
            status = 666;
        if (status == 0 && screen->shader_caps[1].max_tex_instructions != 1037)
            status = 667;
        if (status == 0 && screen->shader_caps[1].max_tex_indirections != 1038)
            status = 668;
        if (status == 0 && screen->shader_caps[1].max_control_flow_depth != 1039)
            status = 669;
        if (status == 0 && screen->shader_caps[1].max_inputs != 1040)
            status = 670;
        if (status == 0 && screen->shader_caps[1].max_outputs != 1041)
            status = 671;
        if (status == 0 && screen->shader_caps[1].max_const_buffer0_size != 1042)
            status = 672;
        if (status == 0 && screen->shader_caps[1].max_const_buffers != 1043)
            status = 673;
        if (status == 0 && screen->shader_caps[1].max_temps != 1044)
            status = 674;
        if (status == 0 && screen->shader_caps[1].max_texture_samplers != 1045)
            status = 675;
        if (status == 0 && screen->shader_caps[1].max_sampler_views != 1046)
            status = 676;
        if (status == 0 && screen->shader_caps[1].max_shader_buffers != 1047)
            status = 677;
        if (status == 0 && screen->shader_caps[1].max_shader_images != 1048)
            status = 678;
        if (status == 0 && screen->shader_caps[1].max_hw_atomic_counters != 1049)
            status = 679;
        if (status == 0 && screen->shader_caps[1].max_hw_atomic_counter_buffers != 1050)
            status = 680;
        if (status == 0 && screen->shader_caps[1].supported_irs != 1051)
            status = 681;
        if (status == 0 && screen->shader_caps[1].cont_supported != 0)
            status = 682;
        if (status == 0 && screen->shader_caps[1].indirect_temp_addr != 0)
            status = 683;
        if (status == 0 && screen->shader_caps[1].indirect_const_addr != 1)
            status = 684;
        if (status == 0 && screen->shader_caps[1].subroutines != 1)
            status = 685;
        if (status == 0 && screen->shader_caps[1].integers != 0)
            status = 686;
        if (status == 0 && screen->shader_caps[1].int64_atomics != 0)
            status = 687;
        if (status == 0 && screen->shader_caps[1].fp16 != 1)
            status = 688;
        if (status == 0 && screen->shader_caps[1].fp16_derivatives != 1)
            status = 689;
        if (status == 0 && screen->shader_caps[1].fp16_const_buffers != 0)
            status = 690;
        if (status == 0 && screen->shader_caps[1].fp16_no_denorms != 0)
            status = 691;
        if (status == 0 && screen->shader_caps[1].int16 != 1)
            status = 692;
        if (status == 0 && screen->shader_caps[1].glsl_16bit_consts != 1)
            status = 693;
        if (status == 0 && screen->shader_caps[1].glsl_16bit_load_dst != 0)
            status = 694;
        if (status == 0 && screen->shader_caps[1].tgsi_sqrt_supported != 0)
            status = 695;
        if (status == 0 && screen->shader_caps[1].tgsi_any_inout_decl_range != 1)
            status = 696;
        if (status == 0 && screen->shader_caps[2].max_instructions != 1067)
            status = 697;
        if (status == 0 && screen->shader_caps[2].max_alu_instructions != 1068)
            status = 698;
        if (status == 0 && screen->shader_caps[2].max_tex_instructions != 1069)
            status = 699;
        if (status == 0 && screen->shader_caps[2].max_tex_indirections != 1070)
            status = 700;
        if (status == 0 && screen->shader_caps[2].max_control_flow_depth != 1071)
            status = 701;
        if (status == 0 && screen->shader_caps[2].max_inputs != 1072)
            status = 702;
        if (status == 0 && screen->shader_caps[2].max_outputs != 1073)
            status = 703;
        if (status == 0 && screen->shader_caps[2].max_const_buffer0_size != 1074)
            status = 704;
        if (status == 0 && screen->shader_caps[2].max_const_buffers != 1075)
            status = 705;
        if (status == 0 && screen->shader_caps[2].max_temps != 1076)
            status = 706;
        if (status == 0 && screen->shader_caps[2].max_texture_samplers != 1077)
            status = 707;
        if (status == 0 && screen->shader_caps[2].max_sampler_views != 1078)
            status = 708;
        if (status == 0 && screen->shader_caps[2].max_shader_buffers != 1079)
            status = 709;
        if (status == 0 && screen->shader_caps[2].max_shader_images != 1080)
            status = 710;
        if (status == 0 && screen->shader_caps[2].max_hw_atomic_counters != 1081)
            status = 711;
        if (status == 0 && screen->shader_caps[2].max_hw_atomic_counter_buffers != 1082)
            status = 712;
        if (status == 0 && screen->shader_caps[2].supported_irs != 1083)
            status = 713;
        if (status == 0 && screen->shader_caps[2].cont_supported != 0)
            status = 714;
        if (status == 0 && screen->shader_caps[2].indirect_temp_addr != 0)
            status = 715;
        if (status == 0 && screen->shader_caps[2].indirect_const_addr != 1)
            status = 716;
        if (status == 0 && screen->shader_caps[2].subroutines != 1)
            status = 717;
        if (status == 0 && screen->shader_caps[2].integers != 0)
            status = 718;
        if (status == 0 && screen->shader_caps[2].int64_atomics != 0)
            status = 719;
        if (status == 0 && screen->shader_caps[2].fp16 != 1)
            status = 720;
        if (status == 0 && screen->shader_caps[2].fp16_derivatives != 1)
            status = 721;
        if (status == 0 && screen->shader_caps[2].fp16_const_buffers != 0)
            status = 722;
        if (status == 0 && screen->shader_caps[2].fp16_no_denorms != 0)
            status = 723;
        if (status == 0 && screen->shader_caps[2].int16 != 1)
            status = 724;
        if (status == 0 && screen->shader_caps[2].glsl_16bit_consts != 1)
            status = 725;
        if (status == 0 && screen->shader_caps[2].glsl_16bit_load_dst != 0)
            status = 726;
        if (status == 0 && screen->shader_caps[2].tgsi_sqrt_supported != 0)
            status = 727;
        if (status == 0 && screen->shader_caps[2].tgsi_any_inout_decl_range != 1)
            status = 728;
        if (status == 0 && screen->shader_caps[3].max_instructions != 1099)
            status = 729;
        if (status == 0 && screen->shader_caps[3].max_alu_instructions != 1100)
            status = 730;
        if (status == 0 && screen->shader_caps[3].max_tex_instructions != 1101)
            status = 731;
        if (status == 0 && screen->shader_caps[3].max_tex_indirections != 1102)
            status = 732;
        if (status == 0 && screen->shader_caps[3].max_control_flow_depth != 1103)
            status = 733;
        if (status == 0 && screen->shader_caps[3].max_inputs != 1104)
            status = 734;
        if (status == 0 && screen->shader_caps[3].max_outputs != 1105)
            status = 735;
        if (status == 0 && screen->shader_caps[3].max_const_buffer0_size != 1106)
            status = 736;
        if (status == 0 && screen->shader_caps[3].max_const_buffers != 1107)
            status = 737;
        if (status == 0 && screen->shader_caps[3].max_temps != 1108)
            status = 738;
        if (status == 0 && screen->shader_caps[3].max_texture_samplers != 1109)
            status = 739;
        if (status == 0 && screen->shader_caps[3].max_sampler_views != 1110)
            status = 740;
        if (status == 0 && screen->shader_caps[3].max_shader_buffers != 1111)
            status = 741;
        if (status == 0 && screen->shader_caps[3].max_shader_images != 1112)
            status = 742;
        if (status == 0 && screen->shader_caps[3].max_hw_atomic_counters != 1113)
            status = 743;
        if (status == 0 && screen->shader_caps[3].max_hw_atomic_counter_buffers != 1114)
            status = 744;
        if (status == 0 && screen->shader_caps[3].supported_irs != 1115)
            status = 745;
        if (status == 0 && screen->shader_caps[3].cont_supported != 0)
            status = 746;
        if (status == 0 && screen->shader_caps[3].indirect_temp_addr != 0)
            status = 747;
        if (status == 0 && screen->shader_caps[3].indirect_const_addr != 1)
            status = 748;
        if (status == 0 && screen->shader_caps[3].subroutines != 1)
            status = 749;
        if (status == 0 && screen->shader_caps[3].integers != 0)
            status = 750;
        if (status == 0 && screen->shader_caps[3].int64_atomics != 0)
            status = 751;
        if (status == 0 && screen->shader_caps[3].fp16 != 1)
            status = 752;
        if (status == 0 && screen->shader_caps[3].fp16_derivatives != 1)
            status = 753;
        if (status == 0 && screen->shader_caps[3].fp16_const_buffers != 0)
            status = 754;
        if (status == 0 && screen->shader_caps[3].fp16_no_denorms != 0)
            status = 755;
        if (status == 0 && screen->shader_caps[3].int16 != 1)
            status = 756;
        if (status == 0 && screen->shader_caps[3].glsl_16bit_consts != 1)
            status = 757;
        if (status == 0 && screen->shader_caps[3].glsl_16bit_load_dst != 0)
            status = 758;
        if (status == 0 && screen->shader_caps[3].tgsi_sqrt_supported != 0)
            status = 759;
        if (status == 0 && screen->shader_caps[3].tgsi_any_inout_decl_range != 1)
            status = 760;
        if (status == 0 && screen->shader_caps[4].max_instructions != 1131)
            status = 761;
        if (status == 0 && screen->shader_caps[4].max_alu_instructions != 1132)
            status = 762;
        if (status == 0 && screen->shader_caps[4].max_tex_instructions != 1133)
            status = 763;
        if (status == 0 && screen->shader_caps[4].max_tex_indirections != 1134)
            status = 764;
        if (status == 0 && screen->shader_caps[4].max_control_flow_depth != 1135)
            status = 765;
        if (status == 0 && screen->shader_caps[4].max_inputs != 1136)
            status = 766;
        if (status == 0 && screen->shader_caps[4].max_outputs != 1137)
            status = 767;
        if (status == 0 && screen->shader_caps[4].max_const_buffer0_size != 1138)
            status = 768;
        if (status == 0 && screen->shader_caps[4].max_const_buffers != 1139)
            status = 769;
        if (status == 0 && screen->shader_caps[4].max_temps != 1140)
            status = 770;
        if (status == 0 && screen->shader_caps[4].max_texture_samplers != 1141)
            status = 771;
        if (status == 0 && screen->shader_caps[4].max_sampler_views != 1142)
            status = 772;
        if (status == 0 && screen->shader_caps[4].max_shader_buffers != 1143)
            status = 773;
        if (status == 0 && screen->shader_caps[4].max_shader_images != 1144)
            status = 774;
        if (status == 0 && screen->shader_caps[4].max_hw_atomic_counters != 1145)
            status = 775;
        if (status == 0 && screen->shader_caps[4].max_hw_atomic_counter_buffers != 1146)
            status = 776;
        if (status == 0 && screen->shader_caps[4].supported_irs != 1147)
            status = 777;
        if (status == 0 && screen->shader_caps[4].cont_supported != 0)
            status = 778;
        if (status == 0 && screen->shader_caps[4].indirect_temp_addr != 0)
            status = 779;
        if (status == 0 && screen->shader_caps[4].indirect_const_addr != 1)
            status = 780;
        if (status == 0 && screen->shader_caps[4].subroutines != 1)
            status = 781;
        if (status == 0 && screen->shader_caps[4].integers != 0)
            status = 782;
        if (status == 0 && screen->shader_caps[4].int64_atomics != 0)
            status = 783;
        if (status == 0 && screen->shader_caps[4].fp16 != 1)
            status = 784;
        if (status == 0 && screen->shader_caps[4].fp16_derivatives != 1)
            status = 785;
        if (status == 0 && screen->shader_caps[4].fp16_const_buffers != 0)
            status = 786;
        if (status == 0 && screen->shader_caps[4].fp16_no_denorms != 0)
            status = 787;
        if (status == 0 && screen->shader_caps[4].int16 != 1)
            status = 788;
        if (status == 0 && screen->shader_caps[4].glsl_16bit_consts != 1)
            status = 789;
        if (status == 0 && screen->shader_caps[4].glsl_16bit_load_dst != 0)
            status = 790;
        if (status == 0 && screen->shader_caps[4].tgsi_sqrt_supported != 0)
            status = 791;
        if (status == 0 && screen->shader_caps[4].tgsi_any_inout_decl_range != 1)
            status = 792;
        if (status == 0 && screen->shader_caps[5].max_instructions != 1163)
            status = 793;
        if (status == 0 && screen->shader_caps[5].max_alu_instructions != 1164)
            status = 794;
        if (status == 0 && screen->shader_caps[5].max_tex_instructions != 1165)
            status = 795;
        if (status == 0 && screen->shader_caps[5].max_tex_indirections != 1166)
            status = 796;
        if (status == 0 && screen->shader_caps[5].max_control_flow_depth != 1167)
            status = 797;
        if (status == 0 && screen->shader_caps[5].max_inputs != 1168)
            status = 798;
        if (status == 0 && screen->shader_caps[5].max_outputs != 1169)
            status = 799;
        if (status == 0 && screen->shader_caps[5].max_const_buffer0_size != 1170)
            status = 800;
        if (status == 0 && screen->shader_caps[5].max_const_buffers != 1171)
            status = 801;
        if (status == 0 && screen->shader_caps[5].max_temps != 1172)
            status = 802;
        if (status == 0 && screen->shader_caps[5].max_texture_samplers != 1173)
            status = 803;
        if (status == 0 && screen->shader_caps[5].max_sampler_views != 1174)
            status = 804;
        if (status == 0 && screen->shader_caps[5].max_shader_buffers != 1175)
            status = 805;
        if (status == 0 && screen->shader_caps[5].max_shader_images != 1176)
            status = 806;
        if (status == 0 && screen->shader_caps[5].max_hw_atomic_counters != 1177)
            status = 807;
        if (status == 0 && screen->shader_caps[5].max_hw_atomic_counter_buffers != 1178)
            status = 808;
        if (status == 0 && screen->shader_caps[5].supported_irs != 1179)
            status = 809;
        if (status == 0 && screen->shader_caps[5].cont_supported != 0)
            status = 810;
        if (status == 0 && screen->shader_caps[5].indirect_temp_addr != 0)
            status = 811;
        if (status == 0 && screen->shader_caps[5].indirect_const_addr != 1)
            status = 812;
        if (status == 0 && screen->shader_caps[5].subroutines != 1)
            status = 813;
        if (status == 0 && screen->shader_caps[5].integers != 0)
            status = 814;
        if (status == 0 && screen->shader_caps[5].int64_atomics != 0)
            status = 815;
        if (status == 0 && screen->shader_caps[5].fp16 != 1)
            status = 816;
        if (status == 0 && screen->shader_caps[5].fp16_derivatives != 1)
            status = 817;
        if (status == 0 && screen->shader_caps[5].fp16_const_buffers != 0)
            status = 818;
        if (status == 0 && screen->shader_caps[5].fp16_no_denorms != 0)
            status = 819;
        if (status == 0 && screen->shader_caps[5].int16 != 1)
            status = 820;
        if (status == 0 && screen->shader_caps[5].glsl_16bit_consts != 1)
            status = 821;
        if (status == 0 && screen->shader_caps[5].glsl_16bit_load_dst != 0)
            status = 822;
        if (status == 0 && screen->shader_caps[5].tgsi_sqrt_supported != 0)
            status = 823;
        if (status == 0 && screen->shader_caps[5].tgsi_any_inout_decl_range != 1)
            status = 824;
        if (status == 0 && screen->shader_caps[6].max_instructions != 1195)
            status = 825;
        if (status == 0 && screen->shader_caps[6].max_alu_instructions != 1196)
            status = 826;
        if (status == 0 && screen->shader_caps[6].max_tex_instructions != 1197)
            status = 827;
        if (status == 0 && screen->shader_caps[6].max_tex_indirections != 1198)
            status = 828;
        if (status == 0 && screen->shader_caps[6].max_control_flow_depth != 1199)
            status = 829;
        if (status == 0 && screen->shader_caps[6].max_inputs != 1200)
            status = 830;
        if (status == 0 && screen->shader_caps[6].max_outputs != 1201)
            status = 831;
        if (status == 0 && screen->shader_caps[6].max_const_buffer0_size != 1202)
            status = 832;
        if (status == 0 && screen->shader_caps[6].max_const_buffers != 1203)
            status = 833;
        if (status == 0 && screen->shader_caps[6].max_temps != 1204)
            status = 834;
        if (status == 0 && screen->shader_caps[6].max_texture_samplers != 1205)
            status = 835;
        if (status == 0 && screen->shader_caps[6].max_sampler_views != 1206)
            status = 836;
        if (status == 0 && screen->shader_caps[6].max_shader_buffers != 1207)
            status = 837;
        if (status == 0 && screen->shader_caps[6].max_shader_images != 1208)
            status = 838;
        if (status == 0 && screen->shader_caps[6].max_hw_atomic_counters != 1209)
            status = 839;
        if (status == 0 && screen->shader_caps[6].max_hw_atomic_counter_buffers != 1210)
            status = 840;
        if (status == 0 && screen->shader_caps[6].supported_irs != 1211)
            status = 841;
        if (status == 0 && screen->shader_caps[6].cont_supported != 0)
            status = 842;
        if (status == 0 && screen->shader_caps[6].indirect_temp_addr != 0)
            status = 843;
        if (status == 0 && screen->shader_caps[6].indirect_const_addr != 1)
            status = 844;
        if (status == 0 && screen->shader_caps[6].subroutines != 1)
            status = 845;
        if (status == 0 && screen->shader_caps[6].integers != 0)
            status = 846;
        if (status == 0 && screen->shader_caps[6].int64_atomics != 0)
            status = 847;
        if (status == 0 && screen->shader_caps[6].fp16 != 1)
            status = 848;
        if (status == 0 && screen->shader_caps[6].fp16_derivatives != 1)
            status = 849;
        if (status == 0 && screen->shader_caps[6].fp16_const_buffers != 0)
            status = 850;
        if (status == 0 && screen->shader_caps[6].fp16_no_denorms != 0)
            status = 851;
        if (status == 0 && screen->shader_caps[6].int16 != 1)
            status = 852;
        if (status == 0 && screen->shader_caps[6].glsl_16bit_consts != 1)
            status = 853;
        if (status == 0 && screen->shader_caps[6].glsl_16bit_load_dst != 0)
            status = 854;
        if (status == 0 && screen->shader_caps[6].tgsi_sqrt_supported != 0)
            status = 855;
        if (status == 0 && screen->shader_caps[6].tgsi_any_inout_decl_range != 1)
            status = 856;
        if (status == 0 && screen->shader_caps[7].max_instructions != 1227)
            status = 857;
        if (status == 0 && screen->shader_caps[7].max_alu_instructions != 1228)
            status = 858;
        if (status == 0 && screen->shader_caps[7].max_tex_instructions != 1229)
            status = 859;
        if (status == 0 && screen->shader_caps[7].max_tex_indirections != 1230)
            status = 860;
        if (status == 0 && screen->shader_caps[7].max_control_flow_depth != 1231)
            status = 861;
        if (status == 0 && screen->shader_caps[7].max_inputs != 1232)
            status = 862;
        if (status == 0 && screen->shader_caps[7].max_outputs != 1233)
            status = 863;
        if (status == 0 && screen->shader_caps[7].max_const_buffer0_size != 1234)
            status = 864;
        if (status == 0 && screen->shader_caps[7].max_const_buffers != 1235)
            status = 865;
        if (status == 0 && screen->shader_caps[7].max_temps != 1236)
            status = 866;
        if (status == 0 && screen->shader_caps[7].max_texture_samplers != 1237)
            status = 867;
        if (status == 0 && screen->shader_caps[7].max_sampler_views != 1238)
            status = 868;
        if (status == 0 && screen->shader_caps[7].max_shader_buffers != 1239)
            status = 869;
        if (status == 0 && screen->shader_caps[7].max_shader_images != 1240)
            status = 870;
        if (status == 0 && screen->shader_caps[7].max_hw_atomic_counters != 1241)
            status = 871;
        if (status == 0 && screen->shader_caps[7].max_hw_atomic_counter_buffers != 1242)
            status = 872;
        if (status == 0 && screen->shader_caps[7].supported_irs != 1243)
            status = 873;
        if (status == 0 && screen->shader_caps[7].cont_supported != 0)
            status = 874;
        if (status == 0 && screen->shader_caps[7].indirect_temp_addr != 0)
            status = 875;
        if (status == 0 && screen->shader_caps[7].indirect_const_addr != 1)
            status = 876;
        if (status == 0 && screen->shader_caps[7].subroutines != 1)
            status = 877;
        if (status == 0 && screen->shader_caps[7].integers != 0)
            status = 878;
        if (status == 0 && screen->shader_caps[7].int64_atomics != 0)
            status = 879;
        if (status == 0 && screen->shader_caps[7].fp16 != 1)
            status = 880;
        if (status == 0 && screen->shader_caps[7].fp16_derivatives != 1)
            status = 881;
        if (status == 0 && screen->shader_caps[7].fp16_const_buffers != 0)
            status = 882;
        if (status == 0 && screen->shader_caps[7].fp16_no_denorms != 0)
            status = 883;
        if (status == 0 && screen->shader_caps[7].int16 != 1)
            status = 884;
        if (status == 0 && screen->shader_caps[7].glsl_16bit_consts != 1)
            status = 885;
        if (status == 0 && screen->shader_caps[7].glsl_16bit_load_dst != 0)
            status = 886;
        if (status == 0 && screen->shader_caps[7].tgsi_sqrt_supported != 0)
            status = 887;
        if (status == 0 && screen->shader_caps[7].tgsi_any_inout_decl_range != 1)
            status = 888;
        if (status == 0 && screen->caps.graphics != 1)
            status = 889;
        if (status == 0 && screen->caps.npot_textures != 0)
            status = 890;
        if (status == 0 && screen->caps.anisotropic_filter != 0)
            status = 891;
        if (status == 0 && screen->caps.occlusion_query != 1)
            status = 892;
        if (status == 0 && screen->caps.query_time_elapsed != 1)
            status = 893;
        if (status == 0 && screen->caps.texture_shadow_map != 0)
            status = 894;
        if (status == 0 && screen->caps.texture_swizzle != 0)
            status = 895;
        if (status == 0 && screen->caps.texture_mirror_clamp != 1)
            status = 896;
        if (status == 0 && screen->caps.blend_equation_separate != 1)
            status = 897;
        if (status == 0 && screen->caps.primitive_restart != 0)
            status = 898;
        if (status == 0 && screen->caps.primitive_restart_fixed_index != 0)
            status = 899;
        if (status == 0 && screen->caps.indep_blend_enable != 1)
            status = 900;
        if (status == 0 && screen->caps.indep_blend_func != 1)
            status = 901;
        if (status == 0 && screen->caps.fs_coord_origin_upper_left != 0)
            status = 902;
        if (status == 0 && screen->caps.fs_coord_origin_lower_left != 0)
            status = 903;
        if (status == 0 && screen->caps.fs_coord_pixel_center_half_integer != 1)
            status = 904;
        if (status == 0 && screen->caps.fs_coord_pixel_center_integer != 1)
            status = 905;
        if (status == 0 && screen->caps.depth_clip_disable != 0)
            status = 906;
        if (status == 0 && screen->caps.depth_clip_disable_separate != 0)
            status = 907;
        if (status == 0 && screen->caps.depth_clamp_enable != 1)
            status = 908;
        if (status == 0 && screen->caps.shader_stencil_export != 1)
            status = 909;
        if (status == 0 && screen->caps.vs_instanceid != 0)
            status = 910;
        if (status == 0 && screen->caps.vertex_element_instance_divisor != 0)
            status = 911;
        if (status == 0 && screen->caps.fragment_color_clamped != 1)
            status = 912;
        if (status == 0 && screen->caps.mixed_colorbuffer_formats != 1)
            status = 913;
        if (status == 0 && screen->caps.seamless_cube_map != 0)
            status = 914;
        if (status == 0 && screen->caps.seamless_cube_map_per_texture != 0)
            status = 915;
        if (status == 0 && screen->caps.conditional_render != 1)
            status = 916;
        if (status == 0 && screen->caps.texture_barrier != 1)
            status = 917;
        if (status == 0 && screen->caps.stream_output_pause_resume != 0)
            status = 918;
        if (status == 0 && screen->caps.tgsi_can_compact_constants != 0)
            status = 919;
        if (status == 0 && screen->caps.vertex_color_unclamped != 1)
            status = 920;
        if (status == 0 && screen->caps.vertex_color_clamped != 1)
            status = 921;
        if (status == 0 && screen->caps.quads_follow_provoking_vertex_convention != 0)
            status = 922;
        if (status == 0 && screen->caps.user_vertex_buffers != 0)
            status = 923;
        if (status == 0 && screen->caps.compute != 1)
            status = 924;
        if (status == 0 && screen->caps.start_instance != 1)
            status = 925;
        if (status == 0 && screen->caps.query_timestamp != 0)
            status = 926;
        if (status == 0 && screen->caps.texture_multisample != 0)
            status = 927;
        if (status == 0 && screen->caps.cube_map_array != 1)
            status = 928;
        if (status == 0 && screen->caps.texture_buffer_objects != 1)
            status = 929;
        if (status == 0 && screen->caps.buffer_sampler_view_rgba_only != 0)
            status = 930;
        if (status == 0 && screen->caps.tgsi_texcoord != 0)
            status = 931;
        if (status == 0 && screen->caps.query_pipeline_statistics != 1)
            status = 932;
        if (status == 0 && screen->caps.mixed_framebuffer_sizes != 1)
            status = 933;
        if (status == 0 && screen->caps.vs_layer_viewport != 0)
            status = 934;
        if (status == 0 && screen->caps.texture_gather_sm5 != 0)
            status = 935;
        if (status == 0 && screen->caps.buffer_map_persistent_coherent != 1)
            status = 936;
        if (status == 0 && screen->caps.fake_sw_msaa != 1)
            status = 937;
        if (status == 0 && screen->caps.texture_query_lod != 0)
            status = 938;
        if (status == 0 && screen->caps.sample_shading != 0)
            status = 939;
        if (status == 0 && screen->caps.texture_gather_offsets != 1)
            status = 940;
        if (status == 0 && screen->caps.vs_window_space_position != 1)
            status = 941;
        if (status == 0 && screen->caps.draw_indirect != 0)
            status = 942;
        if (status == 0 && screen->caps.fs_fine_derivative != 0)
            status = 943;
        if (status == 0 && screen->caps.uma != 1)
            status = 944;
        if (status == 0 && screen->caps.conditional_render_inverted != 1)
            status = 945;
        if (status == 0 && screen->caps.sampler_view_target != 0)
            status = 946;
        if (status == 0 && screen->caps.clip_halfz != 0)
            status = 947;
        if (status == 0 && screen->caps.polygon_offset_clamp != 1)
            status = 948;
        if (status == 0 && screen->caps.multisample_z_resolve != 1)
            status = 949;
        if (status == 0 && screen->caps.resource_from_user_memory != 0)
            status = 950;
        if (status == 0 && screen->caps.resource_from_user_memory_compute_only != 0)
            status = 951;
        if (status == 0 && screen->caps.device_reset_status_query != 1)
            status = 952;
        if (status == 0 && screen->caps.texture_float_linear != 1)
            status = 953;
        if (status == 0 && screen->caps.texture_half_float_linear != 0)
            status = 954;
        if (status == 0 && screen->caps.depth_bounds_test != 0)
            status = 955;
        if (status == 0 && screen->caps.native_fp32_depth != 1)
            status = 956;
        if (status == 0 && screen->caps.texture_query_samples != 1)
            status = 957;
        if (status == 0 && screen->caps.force_persample_interp != 0)
            status = 958;
        if (status == 0 && screen->caps.shareable_shaders != 0)
            status = 959;
        if (status == 0 && screen->caps.copy_between_compressed_and_plain_formats != 1)
            status = 960;
        if (status == 0 && screen->caps.clear_scissored != 1)
            status = 961;
        if (status == 0 && screen->caps.clear_masked != 0)
            status = 962;
        if (status == 0 && screen->caps.draw_parameters != 0)
            status = 963;
        if (status == 0 && screen->caps.shader_pack_half_float != 1)
            status = 964;
        if (status == 0 && screen->caps.multi_draw_indirect != 1)
            status = 965;
        if (status == 0 && screen->caps.multi_draw_indirect_params != 0)
            status = 966;
        if (status == 0 && screen->caps.multi_draw_indirect_partial_stride != 0)
            status = 967;
        if (status == 0 && screen->caps.fs_position_is_sysval != 1)
            status = 968;
        if (status == 0 && screen->caps.fs_point_is_sysval != 1)
            status = 969;
        if (status == 0 && screen->caps.fs_face_is_integer_sysval != 0)
            status = 970;
        if (status == 0 && screen->caps.invalidate_buffer != 0)
            status = 971;
        if (status == 0 && screen->caps.generate_mipmap != 1)
            status = 972;
        if (status == 0 && screen->caps.string_marker != 1)
            status = 973;
        if (status == 0 && screen->caps.surface_no_compress != 0)
            status = 974;
        if (status == 0 && screen->caps.surface_reinterpret_blocks != 0)
            status = 975;
        if (status == 0 && screen->caps.compressed_surface_reinterpret_blocks_layered != 1)
            status = 976;
        if (status == 0 && screen->caps.query_buffer_object != 1)
            status = 977;
        if (status == 0 && screen->caps.query_memory_info != 0)
            status = 978;
        if (status == 0 && screen->caps.framebuffer_no_attachment != 0)
            status = 979;
        if (status == 0 && screen->caps.robust_buffer_access_behavior != 1)
            status = 980;
        if (status == 0 && screen->caps.cull_distance != 1)
            status = 981;
        if (status == 0 && screen->caps.shader_group_vote != 0)
            status = 982;
        if (status == 0 && screen->caps.shader_array_components != 0)
            status = 983;
        if (status == 0 && screen->caps.stream_output_interleave_buffers != 1)
            status = 984;
        if (status == 0 && screen->caps.native_fence_fd != 1)
            status = 985;
        if (status == 0 && screen->caps.glsl_tess_levels_as_inputs != 0)
            status = 986;
        if (status == 0 && screen->caps.legacy_math_rules != 0)
            status = 987;
        if (status == 0 && screen->caps.fp16 != 1)
            status = 988;
        if (status == 0 && screen->caps.doubles != 1)
            status = 989;
        if (status == 0 && screen->caps.int64 != 0)
            status = 990;
        if (status == 0 && screen->caps.shader_clock != 0)
            status = 991;
        if (status == 0 && screen->caps.shader_realtime_clock != 1)
            status = 992;
        if (status == 0 && screen->caps.polygon_mode_fill_rectangle != 1)
            status = 993;
        if (status == 0 && screen->caps.shader_ballot != 0)
            status = 994;
        if (status == 0 && screen->caps.tes_layer_viewport != 0)
            status = 995;
        if (status == 0 && screen->caps.can_bind_const_buffer_as_vertex != 1)
            status = 996;
        if (status == 0 && screen->caps.allow_mapped_buffers_during_execution != 1)
            status = 997;
        if (status == 0 && screen->caps.post_depth_coverage != 0)
            status = 998;
        if (status == 0 && screen->caps.bindless_texture != 0)
            status = 999;
        if (status == 0 && screen->caps.nir_samplers_as_deref != 1)
            status = 1000;
        if (status == 0 && screen->caps.query_so_overflow != 1)
            status = 1001;
        if (status == 0 && screen->caps.memobj != 0)
            status = 1002;
        if (status == 0 && screen->caps.load_constbuf != 0)
            status = 1003;
        if (status == 0 && screen->caps.tile_raster_order != 1)
            status = 1004;
        if (status == 0 && screen->caps.signed_vertex_buffer_offset != 1)
            status = 1005;
        if (status == 0 && screen->caps.fence_signal != 0)
            status = 1006;
        if (status == 0 && screen->caps.packed_uniforms != 0)
            status = 1007;
        if (status == 0 && screen->caps.conservative_raster_post_snap_triangles != 1)
            status = 1008;
        if (status == 0 && screen->caps.conservative_raster_post_snap_points_lines != 1)
            status = 1009;
        if (status == 0 && screen->caps.conservative_raster_pre_snap_triangles != 0)
            status = 1010;
        if (status == 0 && screen->caps.conservative_raster_pre_snap_points_lines != 0)
            status = 1011;
        if (status == 0 && screen->caps.conservative_raster_post_depth_coverage != 1)
            status = 1012;
        if (status == 0 && screen->caps.conservative_raster_inner_coverage != 1)
            status = 1013;
        if (status == 0 && screen->caps.programmable_sample_locations != 0)
            status = 1014;
        if (status == 0 && screen->caps.texture_mirror_clamp_to_edge != 0)
            status = 1015;
        if (status == 0 && screen->caps.surface_sample_count != 1)
            status = 1016;
        if (status == 0 && screen->caps.image_atomic_float_add != 1)
            status = 1017;
        if (status == 0 && screen->caps.query_pipeline_statistics_single != 0)
            status = 1018;
        if (status == 0 && screen->caps.dest_surface_srgb_control != 0)
            status = 1019;
        if (status == 0 && screen->caps.compute_grid_info_last_block != 1)
            status = 1020;
        if (status == 0 && screen->caps.compute_shader_derivatives != 1)
            status = 1021;
        if (status == 0 && screen->caps.image_load_formatted != 0)
            status = 1022;
        if (status == 0 && screen->caps.image_store_formatted != 0)
            status = 1023;
        if (status == 0 && screen->caps.throttle != 1)
            status = 1024;
        if (status == 0 && screen->caps.cl_gl_sharing != 1)
            status = 1025;
        if (status == 0 && screen->caps.prefer_compute_for_multimedia != 0)
            status = 1026;
        if (status == 0 && screen->caps.fragment_shader_interlock != 0)
            status = 1027;
        if (status == 0 && screen->caps.fbfetch_coherent != 1)
            status = 1028;
        if (status == 0 && screen->caps.atomic_float_minmax != 1)
            status = 1029;
        if (status == 0 && screen->caps.fragment_shader_texture_lod != 0)
            status = 1030;
        if (status == 0 && screen->caps.fragment_shader_derivatives != 0)
            status = 1031;
        if (status == 0 && screen->caps.fragment_shader_depth != 1)
            status = 1032;
        if (status == 0 && screen->caps.texture_shadow_lod != 1)
            status = 1033;
        if (status == 0 && screen->caps.shader_samples_identical != 0)
            status = 1034;
        if (status == 0 && screen->caps.image_atomic_inc_wrap != 0)
            status = 1035;
        if (status == 0 && screen->caps.prefer_imm_arrays_as_constbuf != 1)
            status = 1036;
        if (status == 0 && screen->caps.gl_spirv != 1)
            status = 1037;
        if (status == 0 && screen->caps.gl_spirv_variable_pointers != 0)
            status = 1038;
        if (status == 0 && screen->caps.demote_to_helper_invocation != 0)
            status = 1039;
        if (status == 0 && screen->caps.tgsi_tg4_component_in_swizzle != 1)
            status = 1040;
        if (status == 0 && screen->caps.flatshade != 1)
            status = 1041;
        if (status == 0 && screen->caps.alpha_test != 0)
            status = 1042;
        if (status == 0 && screen->caps.two_sided_color != 0)
            status = 1043;
        if (status == 0 && screen->caps.opencl_integer_functions != 1)
            status = 1044;
        if (status == 0 && screen->caps.integer_multiply_32x16 != 1)
            status = 1045;
        if (status == 0 && screen->caps.frontend_noop != 0)
            status = 1046;
        if (status == 0 && screen->caps.nir_images_as_deref != 0)
            status = 1047;
        if (status == 0 && screen->caps.packed_stream_output != 1)
            status = 1048;
        if (status == 0 && screen->caps.viewport_transform_lowered != 1)
            status = 1049;
        if (status == 0 && screen->caps.psiz_clamped != 0)
            status = 1050;
        if (status == 0 && screen->caps.viewport_swizzle != 0)
            status = 1051;
        if (status == 0 && screen->caps.system_svm != 1)
            status = 1052;
        if (status == 0 && screen->caps.viewport_mask != 1)
            status = 1053;
        if (status == 0 && screen->caps.alpha_to_coverage_dither_control != 0)
            status = 1054;
        if (status == 0 && screen->caps.map_unsynchronized_thread_safe != 0)
            status = 1055;
        if (status == 0 && screen->caps.nir_atomics_as_deref != 1)
            status = 1056;
        if (status == 0 && screen->caps.no_clip_on_copy_tex != 1)
            status = 1057;
        if (status == 0 && screen->caps.shader_atomic_int64 != 0)
            status = 1058;
        if (status == 0 && screen->caps.device_protected_surface != 0)
            status = 1059;
        if (status == 0 && screen->caps.prefer_real_buffer_in_constbuf0 != 1)
            status = 1060;
        if (status == 0 && screen->caps.gl_clamp != 1)
            status = 1061;
        if (status == 0 && screen->caps.texrect != 0)
            status = 1062;
        if (status == 0 && screen->caps.sampler_reduction_minmax != 0)
            status = 1063;
        if (status == 0 && screen->caps.sampler_reduction_minmax_arb != 1)
            status = 1064;
        if (status == 0 && screen->caps.allow_dynamic_vao_fastpath != 1)
            status = 1065;
        if (status == 0 && screen->caps.emulate_nonfixed_primitive_restart != 0)
            status = 1066;
        if (status == 0 && screen->caps.prefer_back_buffer_reuse != 0)
            status = 1067;
        if (status == 0 && screen->caps.draw_vertex_state != 1)
            status = 1068;
        if (status == 0 && screen->caps.prefer_pot_aligned_varyings != 1)
            status = 1069;
        if (status == 0 && screen->caps.sparse_texture_full_array_cube_mipmaps != 0)
            status = 1070;
        if (status == 0 && screen->caps.query_sparse_texture_residency != 0)
            status = 1071;
        if (status == 0 && screen->caps.clamp_sparse_texture_lod != 1)
            status = 1072;
        if (status == 0 && screen->caps.allow_draw_out_of_order != 1)
            status = 1073;
        if (status == 0 && screen->caps.hardware_gl_select != 0)
            status = 1074;
        if (status == 0 && screen->caps.dithering != 0)
            status = 1075;
        if (status == 0 && screen->caps.fbfetch_zs != 1)
            status = 1076;
        if (status == 0 && screen->caps.timeline_semaphore_import != 1)
            status = 1077;
        if (status == 0 && screen->caps.device_protected_context != 0)
            status = 1078;
        if (status == 0 && screen->caps.allow_glthread_buffer_subdata_opt != 0)
            status = 1079;
        if (status == 0 && screen->caps.null_textures != 1)
            status = 1080;
        if (status == 0 && screen->caps.astc_void_extents_need_denorm_flush != 1)
            status = 1081;
        if (status == 0 && screen->caps.validate_all_dirty_states != 0)
            status = 1082;
        if (status == 0 && screen->caps.has_const_bw != 0)
            status = 1083;
        if (status == 0 && screen->caps.performance_monitor != 1)
            status = 1084;
        if (status == 0 && screen->caps.texture_sampler_independent != 1)
            status = 1085;
        if (status == 0 && screen->caps.astc_decode_mode != 0)
            status = 1086;
        if (status == 0 && screen->caps.shader_subgroup_quad_all_stages != 0)
            status = 1087;
        if (status == 0 && screen->caps.call_finalize_nir_in_linker != 1)
            status = 1088;
        if (status == 0 && screen->caps.mesh_shader != 1)
            status = 1089;
        if (status == 0 && screen->caps.representative_fragment_test != 0)
            status = 1090;
        if (status == 0 && screen->caps.prefer_persp != 0)
            status = 1091;
        if (status == 0 && screen->caps.blit_3d != 1)
            status = 1092;
        if (status == 0 && screen->caps.glsl_bindless_handles_are_32bit != 1)
            status = 1093;
        if (status == 0 && screen->caps.polygon_stipple != 0)
            status = 1094;
        if (status == 0 && screen->caps.disable_custom_border_color_caching != 0)
            status = 1095;
        if (status == 0 && screen->caps.needs_split_planar_samplers != 1)
            status = 1096;
        if (status == 0 && screen->caps.accelerated != 1467)
            status = 1097;
        if (status == 0 && screen->caps.min_texel_offset != 1468)
            status = 1098;
        if (status == 0 && screen->caps.max_texel_offset != 1469)
            status = 1099;
        if (status == 0 && screen->caps.min_texture_gather_offset != 1470)
            status = 1100;
        if (status == 0 && screen->caps.max_texture_gather_offset != 1471)
            status = 1101;
        if (status == 0 && screen->caps.max_dual_source_render_targets != 1472)
            status = 1102;
        if (status == 0 && screen->caps.max_render_targets != 1473)
            status = 1103;
        if (status == 0 && screen->caps.max_texture_2d_size != 1474)
            status = 1104;
        if (status == 0 && screen->caps.max_texture_3d_levels != 1475)
            status = 1105;
        if (status == 0 && screen->caps.max_texture_cube_levels != 1476)
            status = 1106;
        if (status == 0 && screen->caps.max_stream_output_buffers != 1477)
            status = 1107;
        if (status == 0 && screen->caps.max_texture_array_layers != 1478)
            status = 1108;
        if (status == 0 && screen->caps.max_stream_output_separate_components != 1479)
            status = 1109;
        if (status == 0 && screen->caps.max_stream_output_interleaved_components != 1480)
            status = 1110;
        if (status == 0 && screen->caps.glsl_feature_level != 1481)
            status = 1111;
        if (status == 0 && screen->caps.glsl_feature_level_compatibility != 1482)
            status = 1112;
        if (status == 0 && screen->caps.essl_feature_level != 1483)
            status = 1113;
        if (status == 0 && screen->caps.constant_buffer_offset_alignment != 1484)
            status = 1114;
        if (status == 0 && screen->caps.timer_resolution != 1485)
            status = 1115;
        if (status == 0 && screen->caps.min_map_buffer_alignment != 1486)
            status = 1116;
        if (status == 0 && screen->caps.texture_buffer_offset_alignment != 1487)
            status = 1117;
        if (status == 0 && screen->caps.linear_image_pitch_alignment != 1488)
            status = 1118;
        if (status == 0 && screen->caps.linear_image_base_address_alignment != 1489)
            status = 1119;
        if (status == 0 && screen->caps.texture_transfer_modes != 1490)
            status = 1120;
        if (status == 0 && screen->caps.texture_border_color_quirk != 1491)
            status = 1121;
        if (status == 0 && screen->caps.max_texel_buffer_elements != 1492)
            status = 1122;
        if (status == 0 && screen->caps.max_viewports != 1493)
            status = 1123;
        if (status == 0 && screen->caps.max_geometry_output_vertices != 1494)
            status = 1124;
        if (status == 0 && screen->caps.max_geometry_total_output_components != 1495)
            status = 1125;
        if (status == 0 && screen->caps.max_texture_gather_components != 1496)
            status = 1126;
        if (status == 0 && screen->caps.max_vertex_streams != 1497)
            status = 1127;
        if (status == 0 && screen->caps.vendor_id != 1498)
            status = 1128;
        if (status == 0 && screen->caps.device_id != 1499)
            status = 1129;
        if (status == 0 && screen->caps.video_memory != 1500)
            status = 1130;
        if (status == 0 && screen->caps.max_vertex_attrib_stride != 1501)
            status = 1131;
        if (status == 0 && screen->caps.max_shader_patch_varyings != 1502)
            status = 1132;
        if (status == 0 && screen->caps.shader_buffer_offset_alignment != 1503)
            status = 1133;
        if (status == 0 && screen->caps.pci_group != 1504)
            status = 1134;
        if (status == 0 && screen->caps.pci_bus != 1505)
            status = 1135;
        if (status == 0 && screen->caps.pci_device != 1506)
            status = 1136;
        if (status == 0 && screen->caps.pci_function != 1507)
            status = 1137;
        if (status == 0 && screen->caps.max_window_rectangles != 1508)
            status = 1138;
        if (status == 0 && screen->caps.viewport_subpixel_bits != 1509)
            status = 1139;
        if (status == 0 && screen->caps.rasterizer_subpixel_bits != 1510)
            status = 1140;
        if (status == 0 && screen->caps.mixed_color_depth_bits != 1511)
            status = 1141;
        if (status == 0 && screen->caps.fbfetch != 1512)
            status = 1142;
        if (status == 0 && screen->caps.blend_equation_advanced != 1513)
            status = 1143;
        if (status == 0 && screen->caps.sparse_buffer_page_size != 1514)
            status = 1144;
        if (status == 0 && screen->caps.max_combined_shader_output_resources != 1515)
            status = 1145;
        if (status == 0 && screen->caps.framebuffer_msaa_constraints != 1516)
            status = 1146;
        if (status == 0 && screen->caps.context_priority_mask != 1517)
            status = 1147;
        if (status == 0 && screen->caps.constbuf0_flags != 1518)
            status = 1148;
        if (status == 0 && screen->caps.max_conservative_raster_subpixel_precision_bias != 1519)
            status = 1149;
        if (status == 0 && screen->caps.max_gs_invocations != 1520)
            status = 1150;
        if (status == 0 && screen->caps.max_shader_buffer_size != 1521)
            status = 1151;
        if (status == 0 && screen->caps.max_combined_shader_buffers != 1522)
            status = 1152;
        if (status == 0 && screen->caps.max_combined_image_uniforms != 1523)
            status = 1153;
        if (status == 0 && screen->caps.max_combined_hw_atomic_counters != 1524)
            status = 1154;
        if (status == 0 && screen->caps.max_combined_hw_atomic_counter_buffers != 1525)
            status = 1155;
        if (status == 0 && screen->caps.max_texture_upload_memory_budget != 1526)
            status = 1156;
        if (status == 0 && screen->caps.max_vertex_element_src_offset != 1527)
            status = 1157;
        if (status == 0 && screen->caps.max_varyings != 1528)
            status = 1158;
        if (status == 0 && screen->caps.ignored_output_varyings[0] != 1529)
            status = 1159;
        if (status == 0 && screen->caps.ignored_output_varyings[1] != 1530)
            status = 1160;
        if (status == 0 && screen->caps.ignored_output_varyings[2] != 1531)
            status = 1161;
        if (status == 0 && screen->caps.ignored_output_varyings[3] != 1532)
            status = 1162;
        if (status == 0 && screen->caps.ignored_output_varyings[4] != 1533)
            status = 1163;
        if (status == 0 && screen->caps.ignored_output_varyings[5] != 1534)
            status = 1164;
        if (status == 0 && screen->caps.ignored_output_varyings[6] != 1535)
            status = 1165;
        if (status == 0 && screen->caps.ignored_output_varyings[7] != 1536)
            status = 1166;
        if (status == 0 && screen->caps.dmabuf != 1537)
            status = 1167;
        if (status == 0 && screen->caps.clip_planes != 1538)
            status = 1168;
        if (status == 0 && screen->caps.max_vertex_buffers != 1539)
            status = 1169;
        if (status == 0 && screen->caps.gl_begin_end_buffer_size != 1540)
            status = 1170;
        if (status == 0 && screen->caps.glsl_zero_init != 1541)
            status = 1171;
        if (status == 0 && screen->caps.max_texture_mb != 1542)
            status = 1172;
        if (status == 0 && screen->caps.supported_prim_modes != 1543)
            status = 1173;
        if (status == 0 && screen->caps.supported_prim_modes_with_restart != 1544)
            status = 1174;
        if (status == 0 && screen->caps.max_sparse_texture_size != 1545)
            status = 1175;
        if (status == 0 && screen->caps.max_sparse_3d_texture_size != 1546)
            status = 1176;
        if (status == 0 && screen->caps.max_sparse_array_texture_layers != 1547)
            status = 1177;
        if (status == 0 && screen->caps.max_constant_buffer_size != 1548)
            status = 1178;
        if (status == 0 && screen->caps.query_timestamp_bits != 1549)
            status = 1179;
        if (status == 0 && screen->caps.shader_subgroup_size != 1550)
            status = 1180;
        if (status == 0 && screen->caps.shader_subgroup_supported_stages != 1551)
            status = 1181;
        if (status == 0 && screen->caps.shader_subgroup_supported_features != 1552)
            status = 1182;
        if (status == 0 && screen->caps.shader_pixel_local_storage_size != 1553)
            status = 1183;
        if (status == 0 && screen->caps.shader_pixel_local_storage_fast_size != 1554)
            status = 1184;
        if (status == 0 && screen->caps.multiview != 1555)
            status = 1185;
        if (status == 0 && screen->caps.max_label_length != 1556)
            status = 1186;
        if (status == 0 && screen->caps.max_timeline_semaphore_difference != 1557)
            status = 1187;
        if (status == 0 && screen->caps.map_client_pointer != 1558)
            status = 1188;
        if (status == 0 && screen->caps.min_vma != 1559)
            status = 1189;
        if (status == 0 && screen->caps.max_vma != 1560)
            status = 1190;
        if (status == 0 && screen->caps.hw_clear_buffer_sizes != 1561)
            status = 1191;
        if (status == 0 && screen->caps.device_type != 1562)
            status = 1192;
        if (status == 0 && screen->caps.vertex_input_alignment != 1563)
            status = 1193;
        if (status == 0 && screen->caps.endianness != 1564)
            status = 1194;
        if (status == 0 && screen->caps.point_size_fixed != 1565)
            status = 1195;
        if (status == 0 && screen->caps.min_line_width != 1566.25f)
            status = 1196;
        if (status == 0 && screen->caps.min_line_width_aa != 1567.25f)
            status = 1197;
        if (status == 0 && screen->caps.max_line_width != 1568.25f)
            status = 1198;
        if (status == 0 && screen->caps.max_line_width_aa != 1569.25f)
            status = 1199;
        if (status == 0 && screen->caps.line_width_granularity != 1570.25f)
            status = 1200;
        if (status == 0 && screen->caps.min_point_size != 1571.25f)
            status = 1201;
        if (status == 0 && screen->caps.min_point_size_aa != 1572.25f)
            status = 1202;
        if (status == 0 && screen->caps.max_point_size != 1573.25f)
            status = 1203;
        if (status == 0 && screen->caps.max_point_size_aa != 1574.25f)
            status = 1204;
        if (status == 0 && screen->caps.point_size_granularity != 1575.25f)
            status = 1205;
        if (status == 0 && screen->caps.max_texture_anisotropy != 1576.25f)
            status = 1206;
        if (status == 0 && screen->caps.max_texture_lod_bias != 1577.25f)
            status = 1207;
        if (status == 0 && screen->caps.min_conservative_raster_dilate != 1578.25f)
            status = 1208;
        if (status == 0 && screen->caps.max_conservative_raster_dilate != 1579.25f)
            status = 1209;
        if (status == 0 && screen->caps.conservative_raster_dilate_granularity != 1580.25f)
            status = 1210;
        if (status == 0 && screen->caps.mesh.max_task_work_group_total_count != 1581)
            status = 1211;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_total_count != 1582)
            status = 1212;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_invocations != 1583)
            status = 1213;
        if (status == 0 && screen->caps.mesh.max_task_work_group_invocations != 1584)
            status = 1214;
        if (status == 0 && screen->caps.mesh.max_task_payload_size != 1585)
            status = 1215;
        if (status == 0 && screen->caps.mesh.max_task_shared_memory_size != 1586)
            status = 1216;
        if (status == 0 && screen->caps.mesh.max_mesh_shared_memory_size != 1587)
            status = 1217;
        if (status == 0 && screen->caps.mesh.max_task_payload_and_shared_memory_size != 1588)
            status = 1218;
        if (status == 0 && screen->caps.mesh.max_mesh_payload_and_shared_memory_size != 1589)
            status = 1219;
        if (status == 0 && screen->caps.mesh.max_mesh_output_memory_size != 1590)
            status = 1220;
        if (status == 0 && screen->caps.mesh.max_mesh_payload_and_output_memory_size != 1591)
            status = 1221;
        if (status == 0 && screen->caps.mesh.max_mesh_output_vertices != 1592)
            status = 1222;
        if (status == 0 && screen->caps.mesh.max_mesh_output_primitives != 1593)
            status = 1223;
        if (status == 0 && screen->caps.mesh.max_mesh_output_components != 1594)
            status = 1224;
        if (status == 0 && screen->caps.mesh.max_mesh_output_layers != 1595)
            status = 1225;
        if (status == 0 && screen->caps.mesh.max_mesh_multiview_view_count != 1596)
            status = 1226;
        if (status == 0 && screen->caps.mesh.mesh_output_per_vertex_granularity != 1597)
            status = 1227;
        if (status == 0 && screen->caps.mesh.mesh_output_per_primitive_granularity != 1598)
            status = 1228;
        if (status == 0 && screen->caps.mesh.max_preferred_task_work_group_invocations != 1599)
            status = 1229;
        if (status == 0 && screen->caps.mesh.max_preferred_mesh_work_group_invocations != 1600)
            status = 1230;
        if (status == 0 && screen->caps.mesh.mesh_prefers_local_invocation_vertex_output != 0)
            status = 1231;
        if (status == 0 && screen->caps.mesh.mesh_prefers_local_invocation_primitive_output != 1)
            status = 1232;
        if (status == 0 && screen->caps.mesh.mesh_prefers_compact_vertex_output != 1)
            status = 1233;
        if (status == 0 && screen->caps.mesh.mesh_prefers_compact_primitive_output != 0)
            status = 1234;
        if (status == 0 && screen->caps.mesh.max_task_work_group_count[0] != 1605)
            status = 1235;
        if (status == 0 && screen->caps.mesh.max_task_work_group_count[1] != 1606)
            status = 1236;
        if (status == 0 && screen->caps.mesh.max_task_work_group_count[2] != 1607)
            status = 1237;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_count[0] != 1608)
            status = 1238;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_count[1] != 1609)
            status = 1239;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_count[2] != 1610)
            status = 1240;
        if (status == 0 && screen->caps.mesh.max_task_work_group_size[0] != 1611)
            status = 1241;
        if (status == 0 && screen->caps.mesh.max_task_work_group_size[1] != 1612)
            status = 1242;
        if (status == 0 && screen->caps.mesh.max_task_work_group_size[2] != 1613)
            status = 1243;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_size[0] != 1614)
            status = 1244;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_size[1] != 1615)
            status = 1245;
        if (status == 0 && screen->caps.mesh.max_mesh_work_group_size[2] != 1616)
            status = 1246;
        if (status == 0 && screen->caps.mesh.pipeline_statistic_queries != 0)
            status = 1247;
        if (status == 0 && screen->compute_caps.address_bits != 1618)
            status = 1248;
        if (status == 0 && screen->compute_caps.grid_dimension != 1619)
            status = 1249;
        if (status == 0 && screen->compute_caps.max_grid_size[0] != 1620)
            status = 1250;
        if (status == 0 && screen->compute_caps.max_grid_size[1] != 1621)
            status = 1251;
        if (status == 0 && screen->compute_caps.max_grid_size[2] != 1622)
            status = 1252;
        if (status == 0 && screen->compute_caps.max_block_size[0] != 1623)
            status = 1253;
        if (status == 0 && screen->compute_caps.max_block_size[1] != 1624)
            status = 1254;
        if (status == 0 && screen->compute_caps.max_block_size[2] != 1625)
            status = 1255;
        if (status == 0 && screen->compute_caps.max_threads_per_block != 1626)
            status = 1256;
        if (status == 0 && screen->compute_caps.max_local_size != 1627)
            status = 1257;
        if (status == 0 && screen->compute_caps.max_clock_frequency != 1628)
            status = 1258;
        if (status == 0 && screen->compute_caps.max_compute_units != 1629)
            status = 1259;
        if (status == 0 && screen->compute_caps.max_subgroups != 1630)
            status = 1260;
        if (status == 0 && screen->compute_caps.subgroup_sizes != 1631)
            status = 1261;
        if (status == 0 && screen->compute_caps.max_variable_threads_per_block != 1632)
            status = 1262;
        if (status == 0 && screen->compute_caps.max_mem_alloc_size != 1633)
            status = 1263;
        if (status == 0 && screen->compute_caps.max_global_size != 1634)
            status = 1264;
    }
    screen->destroy(screen);
    return status;
}

/* Two screens served at once: the second destroyed, the first still serves a context (each screen's key is its own).
 * 0 when it does; both screens are destroyed. */
int wc_caller_two_screens(uintptr_t first, uintptr_t second)
{
    struct pipe_screen *one = (struct pipe_screen *)first, *two = (struct pipe_screen *)second;
    two->destroy(two);
    struct pipe_context *ctx = one->context_create(one, NULL, 0);
    if (!ctx) {
        one->destroy(one);
        return 1;
    }
    ctx->destroy(ctx);
    one->destroy(one);
    return 0;
}
