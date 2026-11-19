/* The C side of wcontainers-nir: what it needs of the pinned Mesa, behind plain types. */
#ifndef WCONTAINERS_NIR_BRIDGE_H
#define WCONTAINERS_NIR_BRIDGE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

struct nir_shader;

/* The program stages the routes take, apart from Mesa's own numbering. */
enum wc_stage {
   WC_STAGE_VERTEX,
   WC_STAGE_FRAGMENT,
   WC_STAGE_COMPUTE,
};

/* Bytes or words owned by the bridge; given back with wc_bytes_free and wc_words_free. */
struct wc_bytes {
   uint8_t *data;
   size_t size;
};

struct wc_words {
   uint32_t *data;
   size_t count;
};

/* Takes a reference on Mesa's type tables; wc_finish gives it back. */
void wc_init(void);
void wc_finish(void);

/* NIR of one entry point of a SPIR-V module for Vulkan, or NULL. */
struct nir_shader *wc_spirv_to_nir(const uint32_t *words, size_t count, enum wc_stage stage,
                                   const char *entry_point);

/* NIR's own serialised form, the cache's key (plan 15.3). */
bool wc_nir_serialize(const struct nir_shader *nir, struct wc_bytes *out);

/* Candidate A's first step: the shader prepared and written as SPIR-V by Zink's compiler in its
 * native Vulkan mode. The shader is changed by the preparation. */
bool wc_nir_to_spirv(struct nir_shader *nir, struct wc_words *out);

void wc_nir_free(struct nir_shader *nir);
void wc_bytes_free(struct wc_bytes *bytes);
void wc_words_free(struct wc_words *words);

#endif
