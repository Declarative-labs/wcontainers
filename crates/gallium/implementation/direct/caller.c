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
    elements[1].dual_slot = 1;
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
