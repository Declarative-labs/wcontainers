/* What the bridge's own files share and Rust does not call. */
#ifndef WCONTAINERS_NIR_BRIDGE_PRIVATE_H
#define WCONTAINERS_NIR_BRIDGE_PRIVATE_H

#include "bridge.h"
#include "flat.h"
#include "nir.h"

#ifdef __cplusplus
extern "C" {
#endif

mesa_shader_stage wc_mesa_stage(enum wc_stage stage);

/* The built-in of a stage input's or output's slot, and its location in the common form where it
 * has none (Mesa's slot for one not taken). */
enum wc_builtin wc_builtin_at(const nir_shader *nir, bool output, unsigned slot, uint32_t *location);

/* The clean-up every route starts from: functions inlined, variables in SSA, constants folded. */
void wc_optimize(nir_shader *nir);

/* OpenGL's buffer accesses (a block index and a byte offset: load_ubo, load_ssbo, store_ssbo, the
 * storage atomics, get_ssbo_size) as derefs of one variable a block, an array of 32-bit words.
 * `uniform_bytes` gives each uniform block's size by its index; uniform block i binds at i and
 * storage block j at the number of uniform blocks plus j. Workgroup memory, which the linker
 * makes byte offsets too (load_shared, store_shared, the shared atomics), the same way: one
 * workgroup variable of words. */
void wc_nir_buffers_as_words(nir_shader *nir, const uint32_t *uniform_bytes, uint32_t uniforms);

#ifdef __cplusplus
}
#endif

#endif
