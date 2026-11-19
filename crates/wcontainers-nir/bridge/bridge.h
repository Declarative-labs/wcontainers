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

/* The binding kinds of the common form, in the order of wcontainers_gpu::SlotKind. */
enum wc_slot_kind {
   WC_SLOT_UNIFORM,
   WC_SLOT_SAMPLER,
   WC_SLOT_TEXTURE,
   WC_SLOT_STORAGE,
   WC_SLOT_KINDS,
};

/* A kind's range of binding numbers in a set. */
struct wc_slot_range {
   uint32_t first;
   uint32_t slots;
};

/* One resource as the program declared it (set, binding, count) and where it now binds. */
struct wc_binding {
   enum wc_slot_kind kind;
   uint32_t set;
   uint32_t binding;
   uint32_t count;
   uint32_t new_binding;
   /* For a storage buffer, the stride of its runtime-sized array, else its block's size. */
   uint32_t stride;
};

enum wc_bind_result {
   WC_BIND_OK,
   /* More resources of a kind than its range holds. */
   WC_BIND_BEYOND_LIMIT,
   /* An array of resources without a size (it needs a resource table). */
   WC_BIND_UNBOUNDED,
   WC_BIND_OUT_OF_MEMORY,
};

/* The bindings of one program, owned by the bridge; given back with wc_bindings_free. */
struct wc_bindings {
   struct wc_binding *data;
   size_t count;
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

/* The binding rewrite of every route: each combined image sampler split into a texture and a sampler
 * of the same shape, then every resource renumbered into `set`, each kind from its range's first
 * binding in the order of the program's (set, binding); `out` receives one entry a resource. */
enum wc_bind_result wc_nir_bind(struct nir_shader *nir, uint32_t set,
                                const struct wc_slot_range ranges[WC_SLOT_KINDS],
                                struct wc_bindings *out);

/* NIR's own serialised form, the cache's key (plan 15.3). */
bool wc_nir_serialize(const struct nir_shader *nir, struct wc_bytes *out);

/* Candidate A's first step: the shader prepared and written as SPIR-V by Zink's compiler in its
 * native Vulkan mode. The shader is changed by the preparation. */
bool wc_nir_to_spirv(struct nir_shader *nir, struct wc_words *out);

void wc_nir_free(struct nir_shader *nir);
void wc_bytes_free(struct wc_bytes *bytes);
void wc_words_free(struct wc_words *words);
void wc_bindings_free(struct wc_bindings *bindings);

#endif
