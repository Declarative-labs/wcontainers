/* The bridge's SPIR-V step (bridge.h), built where Mesa's SPIR-V reader is linked: SPIR-V into NIR for the
 * corpus. */
#include "bridge_private.h"

#include <stdlib.h>
#include <string.h>

#include "compiler/spirv/nir_spirv.h"
#include "spirv-tools/libspirv.h"

/* Whether the words are a valid SPIR-V module for Vulkan (SPIRV-Tools' validator, Vulkan 1.3's rules, which take
 * SPIR-V 1.0 to 1.6). Mesa's reader takes only valid modules, as Vulkan does (an invalid one is undefined
 * behaviour): changed words make it read past its arrays (found by the hostile caller's fuzzing). */
/* The most ids a module may bound: SPIRV-Tools' default universal limit (0x3FFFFF). Mesa allocates a value for
 * every id below the bound before it reads an instruction. */
#define WC_MOST_IDS 0x3FFFFFu
/* The first word of every SPIR-V module (the SPIR-V specification §2.3). */
#define WC_SPIRV_MAGIC 0x07230203u

static bool
valid_module(const uint32_t *words, size_t count)
{
   /* The header as Mesa's reader checks it, checked here first: its own refusals of a header reach a log through
    * options it has not set yet (vtn_create_builder), and SPIRV-Tools does not refuse a schema word that is not 0. */
   if (count <= 5 || words[0] != WC_SPIRV_MAGIC || words[1] < 0x10000u || words[3] > WC_MOST_IDS || words[4] != 0)
      return false;
   spv_context context = spvContextCreate(SPV_ENV_VULKAN_1_3);
   if (!context)
      return false;
   spv_diagnostic diagnostic = NULL;
   spv_result_t result = spvValidateBinary(context, words, count, &diagnostic);
   spvDiagnosticDestroy(diagnostic);
   spvContextDestroy(context);
   return result == SPV_SUCCESS;
}

struct nir_shader *
wc_spirv_to_nir(const uint32_t *words, size_t count, enum wc_stage stage, const char *entry_point)
{
   if (!valid_module(words, count))
      return NULL;
   const struct spirv_to_nir_options options = {
      .environment = NIR_SPIRV_VULKAN,
      /* A module the reader refuses is a refusal (NULL), not a break into the debugger: a build with assertions
       * on otherwise stops the process at every refused module (os_break, SIGTRAP). */
      .skip_os_break_in_debug_build = true,
   };
   nir_shader *nir = spirv_to_nir(words, count, NULL, wc_mesa_stage(stage), entry_point, &options,
                                  wc_compiler_options());
   if (nir)
      wc_optimize(nir);
   return nir;
}

struct wc_spirv_shader *
wc_spirv_read(const uint32_t *words, size_t count, enum wc_stage stage, const char *entry_point, bool alpha_to_one)
{
   nir_shader *nir = wc_spirv_to_nir(words, count, stage, entry_point);
   if (!nir)
      return NULL;
   /* Both candidates lower its stage values (alphaToOne here, candidate B's flat form, Zink's preparation): a module
    * whose stage values the lowering does not take is refused here, before any of them. */
   if (!wc_io_fits(nir)) {
      ralloc_free(nir);
      return NULL;
   }
   if (alpha_to_one)
      wc_nir_alpha_to_one(nir);
   return (struct wc_spirv_shader *)nir;
}
