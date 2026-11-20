/* The flat interface between the Gallium bridge (C, on Mesa's headers) and the driver (Rust). */
#ifndef WCONTAINERS_OPENGL_SCREEN_H
#define WCONTAINERS_OPENGL_SCREEN_H

#include <stdint.h>

struct wc_gl_device;

/* What the device can do, in the terms Gallium's caps take; filled by wc_gl_device_create. */
struct wc_gl_limits {
   uint32_t max_texture_2d;
   uint32_t max_texture_cube;
   uint32_t max_texture_3d;
   uint32_t max_texture_layers;
   uint32_t color_targets;
   uint32_t vertex_streams;
   uint32_t max_vertex_stride;
   float max_point_size;
};

/* The device on the GPU the back end finds, or NULL without one. */
struct wc_gl_device *wc_gl_device_create(struct wc_gl_limits *limits);
void wc_gl_device_destroy(struct wc_gl_device *device);
/* The GPU's name, alive as long as the device. */
const char *wc_gl_device_name(const struct wc_gl_device *device);

#endif
