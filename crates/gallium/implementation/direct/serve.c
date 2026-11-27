/* Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit. */
/* The serving glue's first slice: one trampoline per pointer, flattening what Mesa passes and calling Rust's table
 * by value; C frees what it allocated only when Rust answers that it was live. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "pipe/p_context.h"
#include "pipe/p_defines.h"
#include "pipe/p_screen.h"
#include "pipe/p_state.h"

#include "flat.h"

struct wc_keys {
    uint32_t count;
    uintptr_t keys[PIPE_MAX_SAMPLERS];
};

struct wc_vertex_elements {
    uint32_t count;
    struct wc_flat_pipe_vertex_element elements[PIPE_MAX_ATTRIBS];
};

/* Rust's table (serve.rs `Table`, the same order). */
struct wc_table {
    uintptr_t (*context_create)(uintptr_t screen, uintptr_t context, uintptr_t priv, uint32_t flags);
    uint8_t (*screen_destroy)(uintptr_t screen, uintptr_t context);
    uint8_t (*context_destroy)(uintptr_t screen, uintptr_t context);
    uintptr_t (*create_blend_state)(uintptr_t screen, uintptr_t context, struct wc_flat_pipe_blend_state state);
    uint8_t (*bind_blend_state)(uintptr_t screen, uintptr_t context, uintptr_t state);
    uint8_t (*delete_blend_state)(uintptr_t screen, uintptr_t context, uintptr_t state);
    uintptr_t (*create_depth_stencil_alpha_state)(uintptr_t screen, uintptr_t context, struct wc_flat_pipe_depth_stencil_alpha_state state);
    uint8_t (*bind_depth_stencil_alpha_state)(uintptr_t screen, uintptr_t context, uintptr_t state);
    uint8_t (*delete_depth_stencil_alpha_state)(uintptr_t screen, uintptr_t context, uintptr_t state);
    uintptr_t (*create_rasterizer_state)(uintptr_t screen, uintptr_t context, struct wc_flat_pipe_rasterizer_state state);
    uint8_t (*bind_rasterizer_state)(uintptr_t screen, uintptr_t context, uintptr_t state);
    uint8_t (*delete_rasterizer_state)(uintptr_t screen, uintptr_t context, uintptr_t state);
    uintptr_t (*create_sampler_state)(uintptr_t screen, uintptr_t context, struct wc_flat_pipe_sampler_state state);
    uint8_t (*bind_sampler_states)(uintptr_t screen, uintptr_t context, int32_t shader, uint32_t start_slot, struct wc_keys slots);
    uint8_t (*delete_sampler_state)(uintptr_t screen, uintptr_t context, uintptr_t state);
    uintptr_t (*create_vertex_elements_state)(uintptr_t screen, uintptr_t context, struct wc_vertex_elements list);
    uint8_t (*bind_vertex_elements_state)(uintptr_t screen, uintptr_t context, uintptr_t state);
    uint8_t (*delete_vertex_elements_state)(uintptr_t screen, uintptr_t context, uintptr_t state);
    struct wc_flat_pipe_shader_caps (*shader_caps)(uintptr_t screen, uintptr_t context, int32_t stage);
    struct wc_flat_pipe_caps (*caps)(uintptr_t screen, uintptr_t context, uint32_t constant_buffer0_size, uint8_t hardware_select);
    struct wc_flat_pipe_compute_caps (*compute_caps)(uintptr_t screen, uintptr_t context);
};

struct wc_screen {
    struct pipe_screen base;
    uintptr_t key;
    const struct wc_table *table;
};

struct wc_context {
    struct pipe_context base;
    uintptr_t key;
    struct wc_screen *screen;
};

static void wc_context_pointers(struct pipe_context *pipe);
uintptr_t wc_screen_create(uintptr_t key, const struct wc_table *table);
size_t wc_table_size(void);

static struct pipe_context *wc_context_create(struct pipe_screen *screen, void *priv, unsigned flags)
{
    struct wc_screen *s = (struct wc_screen *)screen;
    const uintptr_t key = s->table->context_create(s->key, 0, (uintptr_t)priv, flags);
    if (!key)
        return NULL;
    struct wc_context *c = calloc(1, sizeof *c);
    if (!c) {
        s->table->context_destroy(s->key, key);
        return NULL;
    }
    c->key = key;
    c->screen = s;
    c->base.screen = screen;
    c->base.priv = priv;
    wc_context_pointers(&c->base);
    return &c->base;
}

static void wc_screen_destroy(struct pipe_screen *screen)
{
    struct wc_screen *s = (struct wc_screen *)screen;
    /* The table is Rust's and ends with the screen: nothing of it is read after the call. */
    if (s->table->screen_destroy(s->key, 0))
        free(s);
}

static void wc_context_destroy(struct pipe_context *pipe)
{
    struct wc_context *c = (struct wc_context *)pipe;
    if (c->screen->table->context_destroy(c->screen->key, c->key))
        free(c);
}

static void *wc_create_blend_state(struct pipe_context *pipe, const struct pipe_blend_state *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    if (!state)
        return NULL;
    struct wc_flat_pipe_blend_state flat;
    wc_flat_pipe_blend_state_from(state, &flat);
    return (void *)c->screen->table->create_blend_state(c->screen->key, c->key, flat);
}

static void wc_bind_blend_state(struct pipe_context *pipe, void *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    c->screen->table->bind_blend_state(c->screen->key, c->key, (uintptr_t)state);
}

static void wc_delete_blend_state(struct pipe_context *pipe, void *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    c->screen->table->delete_blend_state(c->screen->key, c->key, (uintptr_t)state);
}

static void *wc_create_depth_stencil_alpha_state(struct pipe_context *pipe, const struct pipe_depth_stencil_alpha_state *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    if (!state)
        return NULL;
    struct wc_flat_pipe_depth_stencil_alpha_state flat;
    wc_flat_pipe_depth_stencil_alpha_state_from(state, &flat);
    return (void *)c->screen->table->create_depth_stencil_alpha_state(c->screen->key, c->key, flat);
}

static void wc_bind_depth_stencil_alpha_state(struct pipe_context *pipe, void *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    c->screen->table->bind_depth_stencil_alpha_state(c->screen->key, c->key, (uintptr_t)state);
}

static void wc_delete_depth_stencil_alpha_state(struct pipe_context *pipe, void *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    c->screen->table->delete_depth_stencil_alpha_state(c->screen->key, c->key, (uintptr_t)state);
}

static void *wc_create_rasterizer_state(struct pipe_context *pipe, const struct pipe_rasterizer_state *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    if (!state)
        return NULL;
    struct wc_flat_pipe_rasterizer_state flat;
    wc_flat_pipe_rasterizer_state_from(state, &flat);
    return (void *)c->screen->table->create_rasterizer_state(c->screen->key, c->key, flat);
}

static void wc_bind_rasterizer_state(struct pipe_context *pipe, void *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    c->screen->table->bind_rasterizer_state(c->screen->key, c->key, (uintptr_t)state);
}

static void wc_delete_rasterizer_state(struct pipe_context *pipe, void *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    c->screen->table->delete_rasterizer_state(c->screen->key, c->key, (uintptr_t)state);
}

static void *wc_create_sampler_state(struct pipe_context *pipe, const struct pipe_sampler_state *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    if (!state)
        return NULL;
    struct wc_flat_pipe_sampler_state flat;
    wc_flat_pipe_sampler_state_from(state, &flat);
    return (void *)c->screen->table->create_sampler_state(c->screen->key, c->key, flat);
}

static void wc_bind_sampler_states(struct pipe_context *pipe, mesa_shader_stage shader, unsigned start_slot, unsigned count, void **states)
{
    struct wc_context *c = (struct wc_context *)pipe;
    if (count > PIPE_MAX_SAMPLERS)
        return;
    struct wc_keys slots;
    memset(&slots, 0, sizeof slots);
    slots.count = count;
    for (unsigned i = 0; i < count; i++)
        slots.keys[i] = states ? (uintptr_t)states[i] : 0;
    c->screen->table->bind_sampler_states(c->screen->key, c->key, (int32_t)shader, start_slot, slots);
}

static void wc_delete_sampler_state(struct pipe_context *pipe, void *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    c->screen->table->delete_sampler_state(c->screen->key, c->key, (uintptr_t)state);
}

static void *wc_create_vertex_elements_state(struct pipe_context *pipe, unsigned count, const struct pipe_vertex_element *elements)
{
    struct wc_context *c = (struct wc_context *)pipe;
    if (count > PIPE_MAX_ATTRIBS || (count && !elements))
        return NULL;
    struct wc_vertex_elements list;
    memset(&list, 0, sizeof list);
    list.count = count;
    for (unsigned i = 0; i < count; i++)
        wc_flat_pipe_vertex_element_from(&elements[i], &list.elements[i]);
    return (void *)c->screen->table->create_vertex_elements_state(c->screen->key, c->key, list);
}

static void wc_bind_vertex_elements_state(struct pipe_context *pipe, void *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    c->screen->table->bind_vertex_elements_state(c->screen->key, c->key, (uintptr_t)state);
}

static void wc_delete_vertex_elements_state(struct pipe_context *pipe, void *state)
{
    struct wc_context *c = (struct wc_context *)pipe;
    c->screen->table->delete_vertex_elements_state(c->screen->key, c->key, (uintptr_t)state);
}

static void wc_context_pointers(struct pipe_context *pipe)
{
    pipe->destroy = wc_context_destroy;
    pipe->create_blend_state = wc_create_blend_state;
    pipe->bind_blend_state = wc_bind_blend_state;
    pipe->delete_blend_state = wc_delete_blend_state;
    pipe->create_depth_stencil_alpha_state = wc_create_depth_stencil_alpha_state;
    pipe->bind_depth_stencil_alpha_state = wc_bind_depth_stencil_alpha_state;
    pipe->delete_depth_stencil_alpha_state = wc_delete_depth_stencil_alpha_state;
    pipe->create_rasterizer_state = wc_create_rasterizer_state;
    pipe->bind_rasterizer_state = wc_bind_rasterizer_state;
    pipe->delete_rasterizer_state = wc_delete_rasterizer_state;
    pipe->create_sampler_state = wc_create_sampler_state;
    pipe->bind_sampler_states = wc_bind_sampler_states;
    pipe->delete_sampler_state = wc_delete_sampler_state;
    pipe->create_vertex_elements_state = wc_create_vertex_elements_state;
    pipe->bind_vertex_elements_state = wc_bind_vertex_elements_state;
    pipe->delete_vertex_elements_state = wc_delete_vertex_elements_state;
}

uintptr_t wc_screen_create(uintptr_t key, const struct wc_table *table)
{
    struct wc_screen *s = calloc(1, sizeof *s);
    if (!s || !table)
        return 0;
    s->key = key;
    s->table = table;
    struct pipe_screen *screen = &s->base;
    screen->context_create = wc_context_create;
    screen->destroy = wc_screen_destroy;
    for (int32_t stage = 0; stage < MESA_SHADER_MESH_STAGES; stage++) {
        const struct wc_flat_pipe_shader_caps flat = table->shader_caps(key, 0, stage);
        wc_flat_pipe_shader_caps_to(&flat, (struct pipe_shader_caps *)&screen->shader_caps[stage]);
    }
    const struct pipe_shader_caps *geometry = &screen->shader_caps[MESA_SHADER_GEOMETRY];
    const struct wc_flat_pipe_caps caps = table->caps(key, 0, screen->shader_caps[MESA_SHADER_FRAGMENT].max_const_buffer0_size, geometry->indirect_temp_addr && geometry->max_shader_buffers);
    wc_flat_pipe_caps_to(&caps, (struct pipe_caps *)&screen->caps);
    const struct wc_flat_pipe_compute_caps compute = table->compute_caps(key, 0);
    wc_flat_pipe_compute_caps_to(&compute, (struct pipe_compute_caps *)&screen->compute_caps);
    return (uintptr_t)screen;
}

/* The table's size, which the Rust side checks against its own. */
size_t wc_table_size(void)
{
    return sizeof(struct wc_table);
}
