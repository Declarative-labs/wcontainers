/* What the bridge's own files share and Rust does not call. */
#ifndef WCONTAINERS_NIR_BRIDGE_PRIVATE_H
#define WCONTAINERS_NIR_BRIDGE_PRIVATE_H

#include "bridge.h"
#include "nir.h"

#ifdef __cplusplus
extern "C" {
#endif

/* The NIR options every route takes (the drivers report the same). */
const struct nir_shader_compiler_options *wc_compiler_options(void);

mesa_shader_stage wc_mesa_stage(enum wc_stage stage);

/* The clean-up every route starts from: functions inlined, variables in SSA, constants folded. */
void wc_optimize(nir_shader *nir);

/* OpenGL's buffer accesses (a block index and a byte offset: load_ubo, load_ssbo, store_ssbo, the
 * storage atomics, get_ssbo_size) as derefs of one variable a block, an array of 32-bit words.
 * `uniform_bytes` gives each uniform block's size by its index; uniform block i binds at i and
 * storage block j at the number of uniform blocks plus j. */
void wc_nir_buffers_as_words(nir_shader *nir, const uint32_t *uniform_bytes, uint32_t uniforms);

#ifdef __cplusplus
}
#endif

#endif
