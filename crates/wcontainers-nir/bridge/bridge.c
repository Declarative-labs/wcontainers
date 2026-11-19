/* The C side of wcontainers-nir (bridge.h). */
#include "bridge.h"

#include <stdlib.h>
#include <string.h>

#include "compiler/glsl_types.h"
#include "compiler/spirv/nir_spirv.h"
#include "nir.h"
#include "nir_builder.h"
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

/* The same deref path as `deref`, on `var`. */
static nir_deref_instr *
deref_on(nir_builder *b, nir_deref_instr *deref, nir_variable *var)
{
   if (deref->deref_type == nir_deref_type_var)
      return nir_build_deref_var(b, var);
   nir_deref_instr *parent = deref_on(b, nir_deref_instr_parent(deref), var);
   return nir_build_deref_array(b, parent, deref->arr.index.ssa);
}

static bool
point_at_split_sampler(nir_builder *b, nir_instr *instr, void *samplers)
{
   if (instr->type != nir_instr_type_tex)
      return false;
   nir_tex_instr *tex = nir_instr_as_tex(instr);
   int at = nir_tex_instr_src_index(tex, nir_tex_src_sampler_deref);
   if (at < 0)
      return false;
   nir_deref_instr *deref = nir_src_as_deref(tex->src[at].src);
   struct hash_entry *split = _mesa_hash_table_search(samplers, nir_deref_instr_get_variable(deref));
   if (!split)
      return false;
   b->cursor = nir_before_instr(instr);
   nir_src_rewrite(&tex->src[at].src, &deref_on(b, deref, split->data)->def);
   return true;
}

/* Each combined image sampler becomes a texture, and a bare sampler of the same array shape at the
 * same set and binding takes its place as every sampling instruction's sampler. */
static void
split_combined_samplers(nir_shader *nir)
{
   struct hash_table *samplers = _mesa_pointer_hash_table_create(NULL);
   nir_foreach_variable_with_modes_safe(var, nir, nir_var_uniform) {
      const struct glsl_type *bare = glsl_without_array(var->type);
      if (!glsl_type_is_sampler(bare) || glsl_type_is_bare_sampler(bare))
         continue;
      nir_variable *sampler = nir_variable_clone(var, nir);
      sampler->type = glsl_type_wrap_in_arrays(glsl_sampler_type_is_shadow(bare)
                                                  ? glsl_bare_shadow_sampler_type()
                                                  : glsl_bare_sampler_type(),
                                               var->type);
      nir_shader_add_variable(nir, sampler);
      var->type = glsl_type_wrap_in_arrays(glsl_sampler_type_to_texture(bare), var->type);
      _mesa_hash_table_insert(samplers, var, sampler);
   }
   if (samplers->entries) {
      nir_fixup_deref_types(nir);
      nir_shader_instructions_pass(nir, point_at_split_sampler, nir_metadata_control_flow, samplers);
   }
   _mesa_hash_table_destroy(samplers, NULL);
}

/* The common form's kind of a resource variable, or WC_SLOT_KINDS for one that is not a resource. */
static enum wc_slot_kind
slot_kind(const nir_variable *var)
{
   const struct glsl_type *bare = glsl_without_array(var->type);
   switch (var->data.mode) {
   case nir_var_mem_ubo:
      return WC_SLOT_UNIFORM;
   case nir_var_mem_ssbo:
   case nir_var_image:
      return WC_SLOT_STORAGE;
   case nir_var_uniform:
      if (glsl_type_is_bare_sampler(bare))
         return WC_SLOT_SAMPLER;
      if (glsl_type_is_texture(bare) || glsl_type_is_sampler(bare))
         return WC_SLOT_TEXTURE;
      if (glsl_type_is_image(bare))
         return WC_SLOT_STORAGE;
      return WC_SLOT_KINDS;
   default:
      return WC_SLOT_KINDS;
   }
}

static int
by_set_and_binding(const void *a, const void *b)
{
   const nir_variable *x = *(nir_variable *const *)a, *y = *(nir_variable *const *)b;
   if (x->data.descriptor_set != y->data.descriptor_set)
      return x->data.descriptor_set < y->data.descriptor_set ? -1 : 1;
   if (x->data.binding != y->data.binding)
      return x->data.binding < y->data.binding ? -1 : 1;
   return (int)slot_kind(x) - (int)slot_kind(y);
}

/* The step of a storage buffer's runtime-sized array, else the size of its block. */
static uint32_t
storage_stride(const nir_variable *var)
{
   const struct glsl_type *block = glsl_without_array(var->type);
   unsigned fields = glsl_get_length(block);
   if (glsl_type_is_struct_or_ifc(block) && fields) {
      const struct glsl_type *last = glsl_get_struct_field(block, fields - 1);
      if (glsl_type_is_unsized_array(last))
         return glsl_get_explicit_stride(last);
   }
   return glsl_get_explicit_size(block, false);
}

/* Buffer accesses name their buffer by set and binding in `vulkan_resource_index`, not through the
 * variable: each is pointed at its buffer's new binding. */
struct renumbered {
   const struct wc_binding *bindings;
   size_t count;
   uint32_t set;
};

static bool
renumber_resource_index(nir_builder *b, nir_intrinsic_instr *intr, void *data)
{
   if (intr->intrinsic != nir_intrinsic_vulkan_resource_index)
      return false;
   const struct renumbered *r = data;
   for (size_t i = 0; i < r->count; i++) {
      const struct wc_binding *at = &r->bindings[i];
      bool buffer = at->kind == WC_SLOT_UNIFORM || at->kind == WC_SLOT_STORAGE;
      if (buffer && at->set == nir_intrinsic_desc_set(intr) &&
          at->binding == nir_intrinsic_binding(intr)) {
         nir_intrinsic_set_desc_set(intr, r->set);
         nir_intrinsic_set_binding(intr, at->new_binding);
         return true;
      }
   }
   return false;
}

enum wc_bind_result
wc_nir_bind(struct nir_shader *nir, uint32_t set, const struct wc_slot_range ranges[WC_SLOT_KINDS],
            struct wc_bindings *bindings)
{
   split_combined_samplers(nir);
   const nir_variable_mode modes = nir_var_uniform | nir_var_image | nir_var_mem_ubo | nir_var_mem_ssbo;
   size_t found = 0;
   nir_foreach_variable_with_modes(var, nir, modes) {
      found += slot_kind(var) != WC_SLOT_KINDS;
   }
   nir_variable **vars = malloc(found * sizeof(*vars));
   struct wc_binding *out = malloc(found * sizeof(*out));
   bindings->data = out;
   bindings->count = found;
   if (found && (!vars || !out)) {
      free(vars);
      return WC_BIND_OUT_OF_MEMORY;
   }
   size_t n = 0;
   nir_foreach_variable_with_modes(var, nir, modes) {
      if (slot_kind(var) != WC_SLOT_KINDS)
         vars[n++] = var;
   }
   qsort(vars, n, sizeof(*vars), by_set_and_binding);
   uint32_t used[WC_SLOT_KINDS] = {0};
   enum wc_bind_result result = WC_BIND_OK;
   size_t i = 0;
   for (; i < n && result == WC_BIND_OK; i++) {
      nir_variable *var = vars[i];
      enum wc_slot_kind kind = slot_kind(var);
      uint32_t slots = glsl_type_is_array(var->type) ? glsl_get_aoa_size(var->type) : 1;
      if (!slots) {
         result = WC_BIND_UNBOUNDED;
         break;
      }
      if (used[kind] + slots > ranges[kind].slots) {
         result = WC_BIND_BEYOND_LIMIT;
         break;
      }
      out[i] = (struct wc_binding){
         .kind = kind,
         .set = var->data.descriptor_set,
         .binding = var->data.binding,
         .count = slots,
         .new_binding = ranges[kind].first + used[kind],
         .stride = var->data.mode == nir_var_mem_ssbo ? storage_stride(var) : 0,
      };
      used[kind] += slots;
      var->data.descriptor_set = set;
      var->data.binding = out[i].new_binding;
   }
   bindings->count = i;
   free(vars);
   if (result == WC_BIND_OK) {
      struct renumbered r = {.bindings = out, .count = i, .set = set};
      nir_shader_intrinsics_pass(nir, renumber_resource_index, nir_metadata_all, &r);
   }
   return result;
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

void
wc_bindings_free(struct wc_bindings *bindings)
{
   free(bindings->data);
   bindings->data = NULL;
}
