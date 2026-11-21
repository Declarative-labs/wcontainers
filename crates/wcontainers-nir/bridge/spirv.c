/* The bridge's SPIR-V steps (bridge.h), built where Mesa's SPIR-V reader and Zink's compiler are
 * linked: SPIR-V into NIR for the corpus, and candidate A's NIR to SPIR-V. */
#include "bridge_private.h"

#include <stdlib.h>
#include <string.h>

#include "compiler/spirv/nir_spirv.h"
#include "nir_to_spirv.h"

/* The SPIR-V version candidate A writes: 1.3, Vulkan 1.1's. */
#define WC_SPIRV_MAJOR 1
#define WC_SPIRV_MINOR 3

struct nir_shader *
wc_spirv_to_nir(const uint32_t *words, size_t count, enum wc_stage stage, const char *entry_point)
{
   const struct spirv_to_nir_options options = {
      .environment = NIR_SPIRV_VULKAN,
   };
   nir_shader *nir = spirv_to_nir(words, count, NULL, wc_mesa_stage(stage), entry_point, &options,
                                  wc_compiler_options());
   if (nir)
      wc_optimize(nir);
   return nir;
}

bool
wc_nir_to_spirv(struct nir_shader *nir, struct wc_words *out)
{
   const struct ntv_info info = {
      .is_native_vulkan = true,
      .has_demote_to_helper = true,
      .spirv_version = SPIRV_VERSION(WC_SPIRV_MAJOR, WC_SPIRV_MINOR),
   };
   ntv_shader_prepare(nir);
   struct spirv_shader *spirv = nir_to_spirv(nir, &info);
   if (!spirv)
      return false;
   out->count = spirv->num_words;
   out->data = malloc(spirv->num_words * sizeof(*spirv->words));
   if (out->data)
      memcpy(out->data, spirv->words, spirv->num_words * sizeof(*spirv->words));
   spirv_shader_delete(spirv);
   return out->data != NULL;
}
