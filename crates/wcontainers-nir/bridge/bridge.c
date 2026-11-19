/* The C side of wcontainers-nir (bridge.h). */
#include "bridge.h"

#include <stdlib.h>
#include <string.h>

#include "compiler/glsl_types.h"
#include "compiler/spirv/nir_spirv.h"
#include "nir.h"
#include "nir_serialize.h"
#include "nir_to_spirv.h"
#include "util/blob.h"
#include "util/ralloc.h"

/* The SPIR-V version candidate A writes: 1.3, Vulkan 1.1's. */
#define WC_SPIRV_MAJOR 1
#define WC_SPIRV_MINOR 3

/* What the routes take: NIR with the operations the common form lacks lowered by NIR itself (the
 * drivers report the same, so the NIR they hand over has this shape). */
static const struct nir_shader_compiler_options compiler_options = {
   .io_options = nir_io_has_intrinsics | nir_io_mediump_is_32bit,
   .lower_scmp = true,
   .lower_fdph = true,
   .lower_flrp32 = true,
   .lower_fsat = true,
   .lower_hadd = true,
   .lower_iadd_sat = true,
   .lower_fisnormal = true,
   .lower_extract_byte = true,
   .lower_extract_word = true,
   .lower_insert_byte = true,
   .lower_insert_word = true,
   .lower_mul_high = true,
   .lower_uadd_carry = true,
   .lower_usub_borrow = true,
   .lower_uadd_sat = true,
   .lower_usub_sat = true,
   .lower_mul_2x32_64 = true,
   .lower_int64_options = (nir_lower_int64_options)~0,
   .lower_uniforms_to_ubo = true,
   .has_fsub = true,
   .has_isub = true,
   .compact_arrays = true,
   .float_mul_add32 = nir_float_muladd_support_keep_weak_ffma,
};

static mesa_shader_stage
mesa_stage(enum wc_stage stage)
{
   switch (stage) {
   case WC_STAGE_VERTEX:
      return MESA_SHADER_VERTEX;
   case WC_STAGE_FRAGMENT:
      return MESA_SHADER_FRAGMENT;
   case WC_STAGE_COMPUTE:
      return MESA_SHADER_COMPUTE;
   }
   return MESA_SHADER_NONE;
}

void
wc_init(void)
{
   glsl_type_singleton_init_or_ref();
}

void
wc_finish(void)
{
   glsl_type_singleton_decref();
}

/* The clean-up every route starts from: functions inlined, variables in SSA, constants folded. */
static void
optimize(nir_shader *nir)
{
   NIR_PASS(_, nir, nir_lower_returns);
   NIR_PASS(_, nir, nir_inline_functions);
   nir_remove_non_entrypoints(nir);
   bool progress;
   do {
      progress = false;
      NIR_PASS(progress, nir, nir_lower_vars_to_ssa);
      NIR_PASS(progress, nir, nir_opt_copy_prop);
      NIR_PASS(progress, nir, nir_opt_deref);
      NIR_PASS(progress, nir, nir_opt_dce);
      NIR_PASS(progress, nir, nir_opt_cse);
      NIR_PASS(progress, nir, nir_opt_dead_cf);
      NIR_PASS(progress, nir, nir_opt_constant_folding);
      NIR_PASS(progress, nir, nir_opt_algebraic);
   } while (progress);
   NIR_PASS(_, nir, nir_lower_int64);
   NIR_PASS(_, nir, nir_remove_dead_variables,
            nir_var_function_temp | nir_var_shader_in | nir_var_shader_out | nir_var_system_value,
            NULL);
}

struct nir_shader *
wc_spirv_to_nir(const uint32_t *words, size_t count, enum wc_stage stage, const char *entry_point)
{
   const struct spirv_to_nir_options options = {
      .environment = NIR_SPIRV_VULKAN,
   };
   nir_shader *nir =
      spirv_to_nir(words, count, NULL, mesa_stage(stage), entry_point, &options, &compiler_options);
   if (nir)
      optimize(nir);
   return nir;
}

bool
wc_nir_serialize(const struct nir_shader *nir, struct wc_bytes *out)
{
   struct blob blob;
   blob_init(&blob);
   nir_serialize(&blob, nir, false);
   if (blob.out_of_memory) {
      blob_finish(&blob);
      return false;
   }
   void *data;
   blob_finish_get_buffer(&blob, &data, &out->size);
   out->data = data;
   return true;
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

void
wc_nir_free(struct nir_shader *nir)
{
   ralloc_free(nir);
}

void
wc_bytes_free(struct wc_bytes *bytes)
{
   free(bytes->data);
   bytes->data = NULL;
}

void
wc_words_free(struct wc_words *words)
{
   free(words->data);
   words->data = NULL;
}
