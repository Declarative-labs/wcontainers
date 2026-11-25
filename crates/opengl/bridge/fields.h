/* Fields of Gallium's structs whose bit fields mix widths, read by the compiler that lays them out:
 * bindgen lays them out as the Itanium ABI does, while Visual C++ starts a new unit wherever the
 * width of the declared type changes (pipe_sampler_view's bool between its enums). */
#ifndef WCONTAINERS_OPENGL_FIELDS_H
#define WCONTAINERS_OPENGL_FIELDS_H

#include "pipe/p_state.h"

enum pipe_texture_target wc_sampler_view_target(const struct pipe_sampler_view *view);

/* The view's swizzle of red, green, blue and alpha. */
void wc_sampler_view_swizzle(const struct pipe_sampler_view *view, unsigned swizzle[4]);

#endif
