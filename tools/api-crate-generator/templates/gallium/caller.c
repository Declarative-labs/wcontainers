/* The hostile C caller of the serving glue's first slice: it calls a served screen's pointers as Mesa would, and as
 * Mesa never would (a key never handed out, a key of another kind, a state given back twice, no template, lists past
 * their bound). Each case returns 0 when what C sees holds; the Rust test (tests/served.rs) checks what reached the
 * implementation. Built with the direct crate's `hostile` feature only. */
#include <stdint.h>
#include <string.h>

#include "pipe/p_context.h"
#include "pipe/p_defines.h"
#include "pipe/p_screen.h"
#include "pipe/p_state.h"
#include "util/blend.h"

int wc_caller_cases(void);
int wc_caller_case(uintptr_t address, int which);

/* FILLED */
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
