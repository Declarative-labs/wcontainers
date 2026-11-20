/* The driver's pipe_screen: Gallium's view of the device the Rust side makes. */
#include <stdio.h>

#include "pipe/p_defines.h"
#include "pipe/p_screen.h"
#include "util/u_math.h"
#include "util/u_memory.h"
#include "util/u_screen.h"
#include "wcontainers/wcontainers_public.h"

#include "screen.h"

#define WC_DRIVER_NAME "wcontainers"

struct wc_screen {
   struct pipe_screen base;
   struct wc_gl_device *device;
};

static struct wc_screen *
wc_screen(struct pipe_screen *screen)
{
   return (struct wc_screen *)screen;
}

static const char *
wc_get_name(struct pipe_screen *screen)
{
   return WC_DRIVER_NAME;
}

static const char *
wc_get_device_vendor(struct pipe_screen *screen)
{
   return wc_gl_device_name(wc_screen(screen)->device);
}

static void
wc_destroy(struct pipe_screen *screen)
{
   wc_gl_device_destroy(wc_screen(screen)->device);
   FREE(screen);
}

/* No format is reported until resources of it can be made. */
static bool
wc_is_format_supported(struct pipe_screen *screen, enum pipe_format format,
                       enum pipe_texture_target target, unsigned sample_count,
                       unsigned storage_sample_count, unsigned bindings)
{
   return false;
}

static struct pipe_context *
wc_context_create(struct pipe_screen *screen, void *priv, unsigned flags)
{
   fprintf(stderr, WC_DRIVER_NAME ": refused: an OpenGL context (the driver's context is not built yet)\n");
   return NULL;
}

/* Levels of a full chain on a side of `size` texels. */
static unsigned
wc_levels(uint32_t size)
{
   return util_logbase2(size) + 1;
}

struct pipe_screen *
wcontainers_screen_create(void)
{
   struct wc_gl_limits limits;
   struct wc_gl_device *device = wc_gl_device_create(&limits);
   if (!device)
      return NULL;
   struct wc_screen *screen = CALLOC_STRUCT(wc_screen);
   if (!screen) {
      wc_gl_device_destroy(device);
      return NULL;
   }
   screen->device = device;
   struct pipe_screen *base = &screen->base;
   base->destroy = wc_destroy;
   base->get_name = wc_get_name;
   base->get_vendor = wc_get_name;
   base->get_device_vendor = wc_get_device_vendor;
   base->is_format_supported = wc_is_format_supported;
   base->context_create = wc_context_create;

   u_init_pipe_screen_caps(base, true);
   struct pipe_caps *caps = (struct pipe_caps *)&base->caps;
   caps->max_texture_2d_size = limits.max_texture_2d;
   caps->max_texture_cube_levels = wc_levels(limits.max_texture_cube);
   caps->max_texture_3d_levels = wc_levels(limits.max_texture_3d);
   caps->max_texture_array_layers = limits.max_texture_layers;
   caps->max_render_targets = limits.color_targets;
   caps->max_vertex_buffers = limits.vertex_streams;
   caps->max_vertex_attrib_stride = limits.max_vertex_stride;
   caps->max_point_size = limits.max_point_size;
   return base;
}
