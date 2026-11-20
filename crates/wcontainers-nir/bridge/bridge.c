/* The C side of wcontainers-nir (bridge.h). */
#include "bridge_private.h"

#include <limits.h>
#include <stdlib.h>
#include <string.h>

#include "compiler/glsl_types.h"
#include "compiler/spirv/nir_spirv.h"
#include "nir.h"
#include "nir_builder.h"
#include "nir_serialize.h"
#include "nir_to_spirv.h"
#include "pipe/p_state.h"
#include "util/bitset.h"
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
   /* The common form's instance index counts from the draw's first instance, as Vulkan's does. */
   .instance_id_includes_base_index = true,
   .has_fsub = true,
   .has_isub = true,
   .compact_arrays = true,
   .float_mul_add32 = nir_float_muladd_support_keep_weak_ffma,
};

const struct nir_shader_compiler_options *
wc_compiler_options(void)
{
   return &compiler_options;
}

mesa_shader_stage
wc_mesa_stage(enum wc_stage stage)
{
   switch (stage) {
   case WC_STAGE_VERTEX:
      return MESA_SHADER_VERTEX;
   case WC_STAGE_FRAGMENT:
      return MESA_SHADER_FRAGMENT;
   case WC_STAGE_COMPUTE:
      return MESA_SHADER_COMPUTE;
   case WC_STAGES:
      break;
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

void
wc_optimize(nir_shader *nir)
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
   NIR_PASS(_, nir, nir_lower_system_values);
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
      spirv_to_nir(words, count, NULL, wc_mesa_stage(stage), entry_point, &options, &compiler_options);
   if (nir)
      wc_optimize(nir);
   return nir;
}

void
wc_nir_lower_io(struct nir_shader *nir)
{
   if (nir->info.io_lowered)
      return;
   NIR_PASS(_, nir, nir_split_per_member_structs);
   NIR_PASS(_, nir, nir_remove_dead_variables, nir_var_shader_in | nir_var_shader_out, NULL);
   nir_lower_io_passes(nir, false);
}

void
wc_nir_alpha_to_one(struct nir_shader *nir)
{
   if (nir->info.stage != MESA_SHADER_FRAGMENT)
      return;
   wc_nir_lower_io(nir);
   NIR_PASS(_, nir, nir_lower_alpha_to_coverage, false, NULL);
   NIR_PASS(_, nir, nir_lower_alpha_to_one);
   /* The pass's own operations in the form the routes take (its fsat lowered, as the options say). */
   bool progress;
   do {
      progress = false;
      NIR_PASS(progress, nir, nir_opt_algebraic);
      NIR_PASS(progress, nir, nir_opt_constant_folding);
      NIR_PASS(progress, nir, nir_opt_dce);
   } while (progress);
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

/* A sampled array layer is the float coordinate rounded to the nearest even integer (Vulkan's rule);
 * the common form takes an integer layer, so the rounding is made explicit before any conversion. */
static bool
round_array_layer(nir_builder *b, nir_instr *instr, void *data)
{
   if (instr->type != nir_instr_type_tex)
      return false;
   nir_tex_instr *tex = nir_instr_as_tex(instr);
   int at = nir_tex_instr_src_index(tex, nir_tex_src_coord);
   if (!tex->is_array || at < 0 || nir_tex_instr_src_type(tex, at) != nir_type_float)
      return false;
   nir_def *coord = tex->src[at].src.ssa;
   unsigned layer = coord->num_components - 1;
   b->cursor = nir_before_instr(instr);
   nir_def *rounded =
      nir_vector_insert_imm(b, coord, nir_fround_even(b, nir_channel(b, coord, layer)), layer);
   nir_src_rewrite(&tex->src[at].src, rounded);
   return true;
}

/* A vertex program that writes no position (one feeding transform feedback) gets the position
 * (0, 0, 0, -1), outside every clip volume, as the shaders crate gives a Direct3D one: the common
 * form's vertex stage must write a position, and drawn it rasterises nothing. */
static void
clip_unpositioned(nir_shader *nir)
{
   if (nir->info.stage != MESA_SHADER_VERTEX)
      return;
   nir_shader_gather_info(nir, nir_shader_get_entrypoint(nir));
   if (nir->info.outputs_written & VARYING_BIT_POS)
      return;
   nir_function_impl *impl = nir_shader_get_entrypoint(nir);
   nir_builder b = nir_builder_at(nir_after_impl(impl));
   nir_def *outside = nir_vec4(&b, nir_imm_float(&b, 0), nir_imm_float(&b, 0), nir_imm_float(&b, 0),
                               nir_imm_float(&b, -1));
   if (nir->info.io_lowered) {
      nir_store_output(&b, outside, nir_imm_int(&b, 0), .base = nir->num_outputs++,
                       .write_mask = BITFIELD_MASK(outside->num_components),
                       .src_type = nir_type_float32,
                       .io_semantics = (nir_io_semantics){.location = VARYING_SLOT_POS, .num_slots = 1});
   } else {
      nir_variable *position =
         nir_variable_create(nir, nir_var_shader_out, glsl_vec4_type(), "gl_Position");
      position->data.location = VARYING_SLOT_POS;
      nir_store_var(&b, position, outside, BITFIELD_MASK(outside->num_components));
   }
   nir_progress(true, impl, nir_metadata_control_flow);
   nir_shader_gather_info(nir, impl);
}

enum wc_bind_result
wc_nir_to_common_form(struct nir_shader *nir, uint32_t set,
                      const struct wc_slot_range ranges[WC_SLOT_KINDS], struct wc_bindings *bindings)
{
   clip_unpositioned(nir);
   nir_shader_instructions_pass(nir, round_array_layer, nir_metadata_control_flow, NULL);
   /* The common form's dispatches start at workgroup 0, and Vulkan has no global offset. */
   const nir_lower_compute_system_values_options from_zero = {0};
   NIR_PASS(_, nir, nir_lower_compute_system_values, &from_zero);
   NIR_PASS(_, nir, nir_opt_constant_folding);
   NIR_PASS(_, nir, nir_opt_algebraic);
   NIR_PASS(_, nir, nir_opt_dce);
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

/* The words of a uniform block are read as vectors of this many, the step of a uniform array. */
#define WC_UNIFORM_VECTOR_WORDS 4
#define WC_WORD_BYTES sizeof(uint32_t)

struct word_blocks {
   nir_variable **uniform;
   uint32_t uniforms;
   nir_variable **storage;
   uint32_t storages;
};

static nir_variable *
word_block(nir_shader *nir, nir_variable_mode mode, uint32_t binding, uint32_t bytes, bool writable)
{
   const struct glsl_type *words;
   if (mode == nir_var_mem_ubo) {
      const uint32_t vector_bytes = WC_UNIFORM_VECTOR_WORDS * WC_WORD_BYTES;
      words = glsl_array_type(glsl_vector_type(GLSL_TYPE_UINT, WC_UNIFORM_VECTOR_WORDS),
                              MAX2(DIV_ROUND_UP(bytes, vector_bytes), 1), vector_bytes);
   } else {
      words = glsl_array_type(glsl_uint_type(), 0, WC_WORD_BYTES);
   }
   const struct glsl_struct_field field = {.type = words, .name = "words", .location = -1};
   const struct glsl_type *block =
      glsl_interface_type(&field, 1, GLSL_INTERFACE_PACKING_STD430, false, field.name);
   nir_variable *var = nir_variable_create(nir, mode, block, NULL);
   var->interface_type = block;
   var->data.binding = binding;
   var->data.explicit_binding = true;
   var->data.access = writable ? 0 : ACCESS_NON_WRITEABLE;
   return var;
}

/* The 32-bit word `word` of a block. */
static nir_deref_instr *
word_at(nir_builder *b, nir_variable *var, nir_def *word)
{
   nir_deref_instr *words = nir_build_deref_struct(b, nir_build_deref_var(b, var), 0);
   if (var->data.mode != nir_var_mem_ubo)
      return nir_build_deref_array(b, words, word);
   nir_deref_instr *vector =
      nir_build_deref_array(b, words, nir_ushr_imm(b, word, util_logbase2(WC_UNIFORM_VECTOR_WORDS)));
   return nir_build_deref_array(b, vector, nir_iand_imm(b, word, WC_UNIFORM_VECTOR_WORDS - 1));
}

/* Where an access names its block and its byte offset, and which blocks it reaches. */
static bool
buffer_access(const nir_intrinsic_instr *intr, int *block, int *offset, bool *uniform, bool *writes)
{
   *offset = -1;
   *uniform = false;
   *writes = false;
   switch (intr->intrinsic) {
   case nir_intrinsic_load_ubo:
      *uniform = true;
      FALLTHROUGH;
   case nir_intrinsic_load_ssbo:
      *block = 0;
      *offset = 1;
      return true;
   case nir_intrinsic_store_ssbo:
      *block = 1;
      *offset = 2;
      *writes = true;
      return true;
   case nir_intrinsic_ssbo_atomic:
   case nir_intrinsic_ssbo_atomic_swap:
      *block = 0;
      *offset = 1;
      *writes = true;
      return true;
   case nir_intrinsic_get_ssbo_size:
      *block = 0;
      return true;
   default:
      return false;
   }
}

/* What the accesses show of the storage blocks: how many there are, and which are written. */
struct storage_use {
   uint32_t blocks;
   BITSET_DECLARE(written, PIPE_MAX_SHADER_BUFFERS);
};

static bool
note_storage(nir_builder *b, nir_intrinsic_instr *intr, void *data)
{
   struct storage_use *use = data;
   int block, offset;
   bool uniform, writes;
   if (!buffer_access(intr, &block, &offset, &uniform, &writes) || uniform ||
       !nir_src_is_const(intr->src[block]) || nir_src_as_uint(intr->src[block]) >= PIPE_MAX_SHADER_BUFFERS)
      return false;
   uint32_t index = nir_src_as_uint(intr->src[block]);
   use->blocks = MAX2(use->blocks, index + 1);
   if (writes)
      BITSET_SET(use->written, index);
   return false;
}

static bool
access_words(nir_builder *b, nir_intrinsic_instr *intr, void *data)
{
   const struct word_blocks *w = data;
   int block, offset;
   bool uniform, writes;
   if (!buffer_access(intr, &block, &offset, &uniform, &writes) || !nir_src_is_const(intr->src[block]))
      return false;
   uint32_t index = nir_src_as_uint(intr->src[block]);
   if (index >= (uniform ? w->uniforms : w->storages))
      return false;
   nir_variable *var = uniform ? w->uniform[index] : w->storage[index];
   b->cursor = nir_before_instr(&intr->instr);
   if (offset < 0) {
      nir_deref_instr *words = nir_build_deref_struct(b, nir_build_deref_var(b, var), 0);
      nir_def *length = nir_deref_buffer_array_length(b, intr->def.bit_size, &words->def);
      nir_def_replace(&intr->def, nir_imul_imm(b, length, WC_WORD_BYTES));
      return true;
   }
   bool stores = intr->intrinsic == nir_intrinsic_store_ssbo;
   unsigned bits = stores ? nir_src_bit_size(intr->src[0]) : intr->def.bit_size;
   if (bits != WC_WORD_BYTES * CHAR_BIT)
      return false;
   nir_def *first = nir_ushr_imm(b, intr->src[offset].ssa, util_logbase2(WC_WORD_BYTES));
   if (stores) {
      nir_def *value = intr->src[0].ssa;
      u_foreach_bit(c, nir_intrinsic_write_mask(intr))
         nir_store_deref(b, word_at(b, var, nir_iadd_imm(b, first, c)), nir_channel(b, value, c), 1);
      nir_instr_remove(&intr->instr);
      return true;
   }
   nir_def *result;
   if (intr->intrinsic == nir_intrinsic_ssbo_atomic) {
      result = nir_deref_atomic(b, bits, &word_at(b, var, first)->def, intr->src[2].ssa,
                                .atomic_op = nir_intrinsic_atomic_op(intr));
   } else if (intr->intrinsic == nir_intrinsic_ssbo_atomic_swap) {
      result = nir_deref_atomic_swap(b, bits, &word_at(b, var, first)->def, intr->src[2].ssa,
                                     intr->src[3].ssa, .atomic_op = nir_intrinsic_atomic_op(intr));
   } else {
      nir_def *words[NIR_MAX_VEC_COMPONENTS];
      for (unsigned c = 0; c < intr->def.num_components; c++)
         words[c] = nir_load_deref(b, word_at(b, var, nir_iadd_imm(b, first, c)));
      result = nir_vec(b, words, intr->def.num_components);
   }
   nir_def_replace(&intr->def, result);
   return true;
}

void
wc_nir_buffers_as_words(nir_shader *nir, const uint32_t *uniform_bytes, uint32_t uniforms)
{
   /* Block indices that are constants show as constants. */
   wc_optimize(nir);
   struct storage_use use = {0};
   nir_shader_intrinsics_pass(nir, note_storage, nir_metadata_all, &use);
   nir_variable **vars = calloc(uniforms + use.blocks + 1, sizeof(*vars));
   if (!vars)
      return;
   nir_foreach_variable_with_modes_safe(var, nir, nir_var_mem_ubo | nir_var_mem_ssbo)
      exec_node_remove(&var->node);
   for (uint32_t i = 0; i < uniforms; i++)
      vars[i] = word_block(nir, nir_var_mem_ubo, i, uniform_bytes[i], false);
   for (uint32_t j = 0; j < use.blocks; j++)
      vars[uniforms + j] =
         word_block(nir, nir_var_mem_ssbo, uniforms + j, 0, BITSET_TEST(use.written, j));
   const struct word_blocks blocks = {
      .uniform = vars,
      .uniforms = uniforms,
      .storage = vars + uniforms,
      .storages = use.blocks,
   };
   nir_shader_intrinsics_pass(nir, access_words, nir_metadata_control_flow, (void *)&blocks);
   free(vars);
}
