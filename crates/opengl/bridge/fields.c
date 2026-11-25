/* The fields of fields.h. */
#include "fields.h"

enum pipe_texture_target
wc_sampler_view_target(const struct pipe_sampler_view *view)
{
   return view->target;
}

void
wc_sampler_view_swizzle(const struct pipe_sampler_view *view, unsigned swizzle[4])
{
   const unsigned of_view[] = {view->swizzle_r, view->swizzle_g, view->swizzle_b, view->swizzle_a};
   for (unsigned i = 0; i < ARRAY_SIZE(of_view); i++)
      swizzle[i] = of_view[i];
}
