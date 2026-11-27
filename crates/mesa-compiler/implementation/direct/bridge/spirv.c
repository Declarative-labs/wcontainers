/* The bridge's SPIR-V steps (bridge.h), built where Mesa's SPIR-V reader and Zink's compiler are
 * linked: SPIR-V into NIR for the corpus, and candidate A's NIR to SPIR-V. */
#include "bridge_private.h"

#include <limits.h>
#include <stdlib.h>
#include <string.h>

#include "compiler/spirv/nir_spirv.h"
#include "nir_to_spirv.h"
#include "pipe/p_state.h"
#include "spirv-tools/libspirv.h"

/* The SPIR-V version candidate A writes: 1.3, Vulkan 1.1's. */
#define WC_SPIRV_MAJOR 1
#define WC_SPIRV_MINOR 3

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

/* Whether the shader is in the form Zink's writer takes in its native Vulkan mode (stage values and uniforms as
 * variables, read through derefs): not with its IO lowered, nor with the OpenGL driver's lowered loads, which the
 * writer stops the process on (an assertion in emit_intrinsic). */
static bool
native_vulkan_form(nir_shader *nir)
{
   if (nir->info.io_lowered)
      return false;
   /* The writer files each image and sampler variable in an array of its own by its driver location
    * (ntv_context's image_var and sampler_var, on the stack): a location past them would be written past them. */
   nir_foreach_variable_with_modes(var, nir, nir_var_image) {
      if (var->data.driver_location >= PIPE_MAX_SHADER_IMAGES)
         return false;
   }
   nir_foreach_variable_with_modes(var, nir, nir_var_uniform) {
      if (var->data.driver_location >= PIPE_MAX_SHADER_SAMPLER_VIEWS)
         return false;
   }
   nir_foreach_function_impl(impl, nir) {
      nir_foreach_block(block, impl) {
         nir_foreach_instr(instr, block) {
            /* Every deref chain starts at a variable: the writer takes each one's storage class from it (a cast
             * has none, and the writer reads it through a null pointer). */
            if (instr->type == nir_instr_type_deref) {
               if (!nir_deref_instr_get_variable(nir_instr_as_deref(instr)))
                  return false;
               continue;
            }
            if (instr->type != nir_instr_type_intrinsic)
               continue;
            switch (nir_instr_as_intrinsic(instr)->intrinsic) {
            case nir_intrinsic_load_input:
            case nir_intrinsic_load_interpolated_input:
            case nir_intrinsic_load_per_vertex_input:
            case nir_intrinsic_load_output:
            case nir_intrinsic_load_per_vertex_output:
            case nir_intrinsic_store_output:
            case nir_intrinsic_store_per_vertex_output:
            case nir_intrinsic_load_ubo:
            case nir_intrinsic_load_uniform:
            case nir_intrinsic_load_barycentric_pixel:
            case nir_intrinsic_load_barycentric_centroid:
            case nir_intrinsic_load_barycentric_sample:
            case nir_intrinsic_load_barycentric_at_sample:
            case nir_intrinsic_load_barycentric_at_offset:
               return false;
            default:
               break;
            }
         }
      }
   }
   return true;
}

bool
wc_nir_to_spirv(struct nir_shader *nir, struct wc_words *out)
{
   if (!native_vulkan_form(nir))
      return false;
   const struct ntv_info info = {
      .is_native_vulkan = true,
      .has_demote_to_helper = true,
      .spirv_version = SPIRV_VERSION(WC_SPIRV_MAJOR, WC_SPIRV_MINOR),
      /* No set is bindless: the writer files a sampler of the bindless set in an array of two by its location, and
       * left at 0 that set would be every program's set 0 (a third sampler written past the array). */
      .bindless_set_idx = UINT_MAX,
   };
   /* Written from a copy: the preparation turns the shader into Zink's own form, which the bridge's other steps
    * do not take (a later step met outputs at per-patch slots and stopped the process); the caller's shader is
    * left as it was. */
   nir_shader *copy = nir_shader_clone(NULL, nir);
   if (!copy)
      return false;
   ntv_shader_prepare(copy);
   struct spirv_shader *spirv = nir_to_spirv(copy, &info);
   ralloc_free(copy);
   if (!spirv)
      return false;
   out->count = spirv->num_words;
   out->data = malloc(spirv->num_words * sizeof(*spirv->words));
   if (out->data)
      memcpy(out->data, spirv->words, spirv->num_words * sizeof(*spirv->words));
   spirv_shader_delete(spirv);
   return out->data != NULL;
}
