/* Candidate B's flattening of NIR (flat.h). */
#include <stdbool.h>
#include "flat.h"

#include <stdlib.h>
#include <string.h>

#include "compiler/glsl_types.h"
#include "nir.h"
#include "nir_builder.h"
#include "util/format/u_formats.h"
#include "util/hash_table.h"

/* A growable array of one element type. */
#define GROW(array, count, capacity)                                                               \
   do {                                                                                            \
      if ((count) == (capacity)) {                                                                 \
         (capacity) = (capacity) ? (capacity) * 2 : 64;                                            \
         void *grown = realloc((array), (capacity) * sizeof(*(array)));                            \
         if (!grown)                                                                               \
            return false;                                                                          \
         (array) = grown;                                                                          \
      }                                                                                            \
   } while (0)

struct flattening {
   struct wc_flat *out;
   uint32_t type_capacity, field_capacity, var_capacity, instr_capacity, initialiser_capacity;
   struct hash_table *types;
   struct hash_table *vars;
};

static enum wc_scalar
scalar_of(enum glsl_base_type base)
{
   switch (base) {
   case GLSL_TYPE_FLOAT:
      return WC_SCALAR_FLOAT;
   case GLSL_TYPE_INT:
      return WC_SCALAR_SINT;
   case GLSL_TYPE_UINT:
      return WC_SCALAR_UINT;
   case GLSL_TYPE_BOOL:
      return WC_SCALAR_BOOL;
   default:
      return WC_SCALAR_OTHER;
   }
}

static enum wc_dim
dim_of(enum glsl_sampler_dim dim)
{
   switch (dim) {
   case GLSL_SAMPLER_DIM_1D:
      return WC_DIM_1D;
   case GLSL_SAMPLER_DIM_2D:
      return WC_DIM_2D;
   case GLSL_SAMPLER_DIM_3D:
      return WC_DIM_3D;
   case GLSL_SAMPLER_DIM_CUBE:
      return WC_DIM_CUBE;
   default:
      return WC_DIM_OTHER;
   }
}

/* Samplers, textures and images (which Mesa's type tables also count as scalars). */
static bool
is_resource(const struct glsl_type *type)
{
   return glsl_type_is_sampler(type) || glsl_type_is_texture(type) || glsl_type_is_image(type);
}

/* The bits of a scalar, vector or matrix's components; 0 for any other type. */
static uint32_t
bits_of(const struct glsl_type *type)
{
   if (is_resource(type))
      return 0;
   if (glsl_type_is_scalar(type) || glsl_type_is_vector(type) || glsl_type_is_matrix(type))
      return glsl_get_bit_size(type);
   return 0;
}

static bool type_index(struct flattening *f, const struct glsl_type *type, uint32_t *index);

static bool
type_index(struct flattening *f, const struct glsl_type *type, uint32_t *index)
{
   struct hash_entry *found = _mesa_hash_table_search(f->types, type);
   if (found) {
      *index = (uint32_t)(uintptr_t)found->data;
      return true;
   }
   struct wc_flat_type t = {
      .kind = WC_TYPE_OTHER,
      .scalar = scalar_of(glsl_get_base_type(glsl_without_array(type))),
      .bits = bits_of(glsl_without_array(type)),
      .components = glsl_get_vector_elements(type),
      .columns = glsl_get_matrix_columns(type),
      .element = WC_NONE,
      .stride = glsl_get_explicit_stride(type),
      .dim = WC_DIM_OTHER,
      .sampled = WC_SCALAR_OTHER,
   };
   if (glsl_type_is_array(type)) {
      t.kind = WC_TYPE_ARRAY;
      t.length = glsl_get_length(type);
      if (!type_index(f, glsl_get_array_element(type), &t.element))
         return false;
   } else if (glsl_type_is_struct_or_ifc(type)) {
      t.kind = WC_TYPE_STRUCT;
      t.length = glsl_get_length(type);
      uint32_t field_types[t.length ? t.length : 1];
      for (uint32_t i = 0; i < t.length; i++) {
         if (!type_index(f, glsl_get_struct_field(type, i), &field_types[i]))
            return false;
      }
      t.element = f->out->field_count;
      for (uint32_t i = 0; i < t.length; i++) {
         GROW(f->out->fields, f->out->field_count, f->field_capacity);
         f->out->fields[f->out->field_count++] = (struct wc_flat_field){
            .type = field_types[i],
            .offset = glsl_get_struct_field_offset(type, i),
         };
      }
   } else if (glsl_type_is_matrix(type)) {
      t.kind = WC_TYPE_MATRIX;
      t.row_major = glsl_matrix_type_is_row_major(type);
      if (!type_index(f, glsl_get_column_type(type), &t.element))
         return false;
   } else if (is_resource(type)) {
      t.kind = glsl_type_is_bare_sampler(type) ? WC_TYPE_SAMPLER
               : glsl_type_is_image(type)      ? WC_TYPE_IMAGE
                                               : WC_TYPE_TEXTURE;
      t.dim = dim_of(glsl_get_sampler_dim(type));
      t.multisampled = glsl_get_sampler_dim(type) == GLSL_SAMPLER_DIM_MS;
      if (t.multisampled)
         t.dim = WC_DIM_2D;
      t.arrayed = glsl_sampler_type_is_array(type);
      t.shadow = glsl_type_is_sampler(type) && glsl_sampler_type_is_shadow(type);
      if (t.kind != WC_TYPE_SAMPLER)
         t.sampled = scalar_of(glsl_get_sampler_result_type(type));
   } else if (glsl_type_is_vector(type)) {
      t.kind = WC_TYPE_VECTOR;
   } else if (glsl_type_is_scalar(type)) {
      t.kind = WC_TYPE_SCALAR;
   }
   GROW(f->out->types, f->out->type_count, f->type_capacity);
   *index = f->out->type_count;
   f->out->types[f->out->type_count++] = t;
   _mesa_hash_table_insert(f->types, type, (void *)(uintptr_t)*index);
   return true;
}

static enum wc_storage_format
format_of(enum pipe_format format)
{
   switch (format) {
   case PIPE_FORMAT_NONE:
      return WC_FORMAT_NONE;
   case PIPE_FORMAT_R32_FLOAT:
      return WC_FORMAT_R32_FLOAT;
   case PIPE_FORMAT_R32_UINT:
      return WC_FORMAT_R32_UINT;
   case PIPE_FORMAT_R32_SINT:
      return WC_FORMAT_R32_SINT;
   case PIPE_FORMAT_R32G32_FLOAT:
      return WC_FORMAT_RG32_FLOAT;
   case PIPE_FORMAT_R32G32B32A32_FLOAT:
      return WC_FORMAT_RGBA32_FLOAT;
   case PIPE_FORMAT_R32G32B32A32_UINT:
      return WC_FORMAT_RGBA32_UINT;
   case PIPE_FORMAT_R32G32B32A32_SINT:
      return WC_FORMAT_RGBA32_SINT;
   case PIPE_FORMAT_R16G16B16A16_FLOAT:
      return WC_FORMAT_RGBA16_FLOAT;
   case PIPE_FORMAT_R8G8B8A8_UNORM:
      return WC_FORMAT_RGBA8_UNORM;
   case PIPE_FORMAT_R8G8B8A8_SNORM:
      return WC_FORMAT_RGBA8_SNORM;
   case PIPE_FORMAT_R8_UNORM:
      return WC_FORMAT_R8_UNORM;
   case PIPE_FORMAT_R8G8_UNORM:
      return WC_FORMAT_RG8_UNORM;
   default:
      return WC_FORMAT_OTHER;
   }
}

static enum wc_var_mode
mode_of(nir_variable_mode mode)
{
   switch (mode) {
   case nir_var_shader_in:
      return WC_VAR_INPUT;
   case nir_var_shader_out:
      return WC_VAR_OUTPUT;
   case nir_var_uniform:
   case nir_var_image:
      return WC_VAR_HANDLE;
   case nir_var_mem_ubo:
      return WC_VAR_UNIFORM_BUFFER;
   case nir_var_mem_ssbo:
      return WC_VAR_STORAGE_BUFFER;
   case nir_var_mem_push_const:
      return WC_VAR_IMMEDIATE;
   case nir_var_mem_shared:
      return WC_VAR_WORKGROUP;
   case nir_var_shader_temp:
      return WC_VAR_PRIVATE;
   case nir_var_function_temp:
      return WC_VAR_FUNCTION;
   default:
      return WC_VAR_OTHER;
   }
}

/* A stage input's or output's built-in, and its generic location where it has none. */
static enum wc_builtin
builtin_of(const nir_shader *nir, const nir_variable *var, uint32_t *location)
{
   int slot = var->data.location;
   *location = 0;
   if (var->data.mode == nir_var_shader_in && nir->info.stage == MESA_SHADER_VERTEX) {
      *location = slot - VERT_ATTRIB_GENERIC0;
      return WC_BUILTIN_NONE;
   }
   if (var->data.mode == nir_var_shader_out && nir->info.stage == MESA_SHADER_FRAGMENT) {
      if (slot == FRAG_RESULT_DEPTH)
         return WC_BUILTIN_FRAG_DEPTH;
      if (slot == FRAG_RESULT_SAMPLE_MASK)
         return WC_BUILTIN_SAMPLE_MASK;
      if (slot < FRAG_RESULT_DATA0)
         return WC_BUILTIN_OTHER;
      *location = slot - FRAG_RESULT_DATA0;
      return WC_BUILTIN_NONE;
   }
   switch (slot) {
   case VARYING_SLOT_POS:
      return WC_BUILTIN_POSITION;
   case VARYING_SLOT_PSIZ:
      return WC_BUILTIN_POINT_SIZE;
   case VARYING_SLOT_CLIP_DIST0:
      return WC_BUILTIN_CLIP_DISTANCE;
   default:
      if (slot < VARYING_SLOT_VAR0)
         return WC_BUILTIN_OTHER;
      *location = slot - VARYING_SLOT_VAR0;
      return WC_BUILTIN_NONE;
   }
}

static enum wc_interpolation
interpolation_of(const nir_variable *var)
{
   switch (var->data.interpolation) {
   case INTERP_MODE_FLAT:
      return WC_INTERP_FLAT;
   case INTERP_MODE_NOPERSPECTIVE:
      return WC_INTERP_LINEAR;
   default:
      return glsl_base_type_is_integer(glsl_get_base_type(glsl_without_array(var->type)))
                ? WC_INTERP_FLAT
                : WC_INTERP_PERSPECTIVE;
   }
}

static bool
add_constant(struct flattening *f, const nir_constant *c, const struct glsl_type *type)
{
   if (glsl_type_is_array(type) || glsl_type_is_struct_or_ifc(type) || glsl_type_is_matrix(type)) {
      bool matrix = glsl_type_is_matrix(type);
      uint32_t n = matrix ? glsl_get_matrix_columns(type) : glsl_get_length(type);
      for (uint32_t i = 0; i < n; i++) {
         const struct glsl_type *element = matrix ? glsl_get_column_type(type)
                                           : glsl_type_is_array(type)
                                              ? glsl_get_array_element(type)
                                              : glsl_get_struct_field(type, i);
         if (!add_constant(f, c->elements[i], element))
            return false;
      }
      return true;
   }
   for (uint32_t i = 0; i < glsl_get_vector_elements(type); i++) {
      GROW(f->out->initialisers, f->out->initialiser_count, f->initialiser_capacity);
      f->out->initialisers[f->out->initialiser_count++] =
         glsl_type_is_boolean(type) ? c->values[i].b : c->values[i].u32;
   }
   return true;
}

static bool
add_var(struct flattening *f, nir_shader *nir, nir_variable *var)
{
   struct wc_flat_var v = {
      .mode = mode_of(var->data.mode),
      .set = var->data.descriptor_set,
      .binding = var->data.binding,
      .interpolation = interpolation_of(var),
      .centroid = var->data.centroid,
      .sample = var->data.sample,
      .readable = !(var->data.access & ACCESS_NON_READABLE),
      .writable = !(var->data.access & ACCESS_NON_WRITEABLE),
      .format = format_of(var->data.image.format),
      .initialised = WC_NONE,
   };
   if (!type_index(f, var->type, &v.type))
      return false;
   if (var->data.mode & (nir_var_shader_in | nir_var_shader_out))
      v.builtin = builtin_of(nir, var, &v.location);
   if (var->constant_initializer) {
      v.initialised = f->out->initialiser_count;
      if (!add_constant(f, var->constant_initializer, var->type))
         return false;
   }
   GROW(f->out->vars, f->out->var_count, f->var_capacity);
   _mesa_hash_table_insert(f->vars, var, (void *)(uintptr_t)f->out->var_count);
   f->out->vars[f->out->var_count++] = v;
   return true;
}

static struct wc_flat_instr *
next_instr(struct flattening *f, enum wc_flat_kind kind)
{
   struct wc_flat *out = f->out;
   if (out->instr_count == f->instr_capacity) {
      f->instr_capacity = f->instr_capacity ? f->instr_capacity * 2 : 256;
      void *grown = realloc(out->instrs, f->instr_capacity * sizeof(*out->instrs));
      if (!grown)
         return NULL;
      out->instrs = grown;
   }
   struct wc_flat_instr *i = &out->instrs[out->instr_count++];
   memset(i, 0, sizeof(*i));
   i->kind = kind;
   i->def = WC_NONE;
   for (uint32_t s = 0; s < WC_TEX_SRCS; s++)
      i->tex[s] = WC_NONE;
   for (uint32_t s = 0; s < WC_SOURCES; s++) {
      i->src[s].def = WC_NONE;
      for (uint32_t c = 0; c < WC_COMPONENTS; c++)
         i->src[s].swizzle[c] = c;
   }
   return i;
}

static void
set_def(struct wc_flat_instr *i, const nir_def *def)
{
   i->def = def->index;
   i->components = def->num_components;
   i->bits = def->bit_size;
}

static enum wc_alu_op
alu_op(nir_op op)
{
   switch (op) {
#define WC_CASE(name)                                                                              \
   case nir_op_##name:                                                                             \
      return WC_ALU_##name;
      WC_ALU_OPS(WC_CASE)
#undef WC_CASE
   default:
      return WC_ALU_UNSUPPORTED;
   }
}

static enum wc_intrinsic
intrinsic_of(nir_intrinsic_op op)
{
   switch (op) {
   case nir_intrinsic_load_deref:
      return WC_INTRINSIC_LOAD_DEREF;
   case nir_intrinsic_store_deref:
      return WC_INTRINSIC_STORE_DEREF;
   case nir_intrinsic_decl_reg:
      return WC_INTRINSIC_DECL_REG;
   case nir_intrinsic_load_reg:
      return WC_INTRINSIC_LOAD_REG;
   case nir_intrinsic_store_reg:
      return WC_INTRINSIC_STORE_REG;
   case nir_intrinsic_vulkan_resource_index:
      return WC_INTRINSIC_VULKAN_RESOURCE_INDEX;
   case nir_intrinsic_load_vulkan_descriptor:
      return WC_INTRINSIC_LOAD_VULKAN_DESCRIPTOR;
   case nir_intrinsic_load_vertex_id:
      return WC_INTRINSIC_LOAD_VERTEX_ID;
   case nir_intrinsic_load_instance_id:
      return WC_INTRINSIC_LOAD_INSTANCE_ID;
   case nir_intrinsic_load_front_face:
      return WC_INTRINSIC_LOAD_FRONT_FACE;
   case nir_intrinsic_load_frag_coord:
      return WC_INTRINSIC_LOAD_FRAG_COORD;
   case nir_intrinsic_load_sample_id:
      return WC_INTRINSIC_LOAD_SAMPLE_ID;
   case nir_intrinsic_load_local_invocation_id:
      return WC_INTRINSIC_LOAD_LOCAL_INVOCATION_ID;
   case nir_intrinsic_load_global_invocation_id:
      return WC_INTRINSIC_LOAD_GLOBAL_INVOCATION_ID;
   case nir_intrinsic_load_workgroup_id:
      return WC_INTRINSIC_LOAD_WORKGROUP_ID;
   case nir_intrinsic_load_num_workgroups:
      return WC_INTRINSIC_LOAD_NUM_WORKGROUPS;
   case nir_intrinsic_load_local_invocation_index:
      return WC_INTRINSIC_LOAD_LOCAL_INVOCATION_INDEX;
   case nir_intrinsic_demote:
      return WC_INTRINSIC_DEMOTE;
   case nir_intrinsic_terminate:
      return WC_INTRINSIC_TERMINATE;
   case nir_intrinsic_demote_if:
      return WC_INTRINSIC_DEMOTE_IF;
   case nir_intrinsic_terminate_if:
      return WC_INTRINSIC_TERMINATE_IF;
   case nir_intrinsic_barrier:
      return WC_INTRINSIC_BARRIER;
   case nir_intrinsic_deref_buffer_array_length:
      return WC_INTRINSIC_DEREF_BUFFER_ARRAY_LENGTH;
   case nir_intrinsic_ddx:
      return WC_INTRINSIC_DDX;
   case nir_intrinsic_ddy:
      return WC_INTRINSIC_DDY;
   case nir_intrinsic_ddx_fine:
      return WC_INTRINSIC_DDX_FINE;
   case nir_intrinsic_ddy_fine:
      return WC_INTRINSIC_DDY_FINE;
   case nir_intrinsic_ddx_coarse:
      return WC_INTRINSIC_DDX_COARSE;
   case nir_intrinsic_ddy_coarse:
      return WC_INTRINSIC_DDY_COARSE;
   default:
      return WC_INTRINSIC_UNSUPPORTED;
   }
}

static void
set_src(struct wc_flat_instr *i, uint32_t at, const nir_src *src)
{
   i->src[at].def = src->ssa->index;
   if (at + 1 > i->sources)
      i->sources = at + 1;
}

static bool
flatten_tex(struct flattening *f, nir_tex_instr *tex)
{
   struct wc_flat_instr *i = next_instr(f, WC_FLAT_TEX);
   if (!i)
      return false;
   set_def(i, &tex->def);
   i->dim = dim_of(tex->sampler_dim);
   i->arrayed = tex->is_array;
   i->shadow = tex->is_shadow;
   i->component = tex->component;
   i->type = scalar_of(nir_get_glsl_base_type_for_nir_type(tex->dest_type));
   switch (tex->op) {
   case nir_texop_tex:
      i->op = WC_TEX_SAMPLE;
      break;
   case nir_texop_txb:
      i->op = WC_TEX_BIAS;
      break;
   case nir_texop_txl:
      i->op = WC_TEX_LEVEL;
      break;
   case nir_texop_txd:
      i->op = WC_TEX_GRADIENT;
      break;
   case nir_texop_txf:
   case nir_texop_txf_ms:
      i->op = WC_TEX_LOAD;
      break;
   case nir_texop_txs:
      i->op = WC_TEX_SIZE;
      break;
   case nir_texop_query_levels:
      i->op = WC_TEX_LEVELS;
      break;
   case nir_texop_tg4:
      i->op = WC_TEX_GATHER;
      break;
   default:
      i->kind = WC_FLAT_UNSUPPORTED;
      i->name = "a texture operation of another kind";
      return true;
   }
   for (unsigned s = 0; s < tex->num_srcs; s++) {
      enum wc_tex_src at;
      switch (tex->src[s].src_type) {
      case nir_tex_src_coord:
         at = WC_TEX_SRC_COORD;
         break;
      case nir_tex_src_texture_deref:
         at = WC_TEX_SRC_TEXTURE;
         break;
      case nir_tex_src_sampler_deref:
         at = WC_TEX_SRC_SAMPLER;
         break;
      case nir_tex_src_bias:
         at = WC_TEX_SRC_BIAS;
         break;
      case nir_tex_src_lod:
         at = WC_TEX_SRC_LOD;
         break;
      case nir_tex_src_comparator:
         at = WC_TEX_SRC_COMPARATOR;
         break;
      case nir_tex_src_offset:
         at = WC_TEX_SRC_OFFSET;
         break;
      case nir_tex_src_ddx:
         at = WC_TEX_SRC_DDX;
         break;
      case nir_tex_src_ddy:
         at = WC_TEX_SRC_DDY;
         break;
      case nir_tex_src_ms_index:
         at = WC_TEX_SRC_SAMPLE;
         break;
      default:
         i->kind = WC_FLAT_UNSUPPORTED;
         i->name = "a texture operand of another kind";
         return true;
      }
      i->tex[at] = tex->src[s].src.ssa->index;
   }
   return true;
}

static bool
flatten_intrinsic(struct flattening *f, nir_intrinsic_instr *intr)
{
   struct wc_flat_instr *i = next_instr(f, WC_FLAT_INTRINSIC);
   if (!i)
      return false;
   const nir_intrinsic_info *info = &nir_intrinsic_infos[intr->intrinsic];
   i->op = intrinsic_of(intr->intrinsic);
   i->name = info->name;
   if (info->has_dest)
      set_def(i, &intr->def);
   for (unsigned s = 0; s < info->num_srcs && s < WC_SOURCES; s++)
      set_src(i, s, &intr->src[s]);
   if (nir_intrinsic_has_desc_set(intr))
      i->value[0] = nir_intrinsic_desc_set(intr);
   if (nir_intrinsic_has_binding(intr))
      i->value[1] = nir_intrinsic_binding(intr);
   if (nir_intrinsic_has_write_mask(intr))
      i->value[2] = nir_intrinsic_write_mask(intr);
   if (intr->intrinsic == nir_intrinsic_decl_reg) {
      i->components = nir_intrinsic_num_components(intr);
      i->bits = nir_intrinsic_bit_size(intr);
      if (nir_intrinsic_num_array_elems(intr)) {
         i->kind = WC_FLAT_UNSUPPORTED;
         i->name = info->name;
      }
   }
   if (intr->intrinsic == nir_intrinsic_barrier) {
      i->value[0] = nir_intrinsic_execution_scope(intr) == SCOPE_WORKGROUP;
      i->value[1] = (nir_intrinsic_memory_modes(intr) & nir_var_mem_shared) != 0;
      i->value[2] = (nir_intrinsic_memory_modes(intr) & (nir_var_mem_ssbo | nir_var_image)) != 0;
   }
   return true;
}

static bool
flatten_instr(struct flattening *f, nir_instr *instr)
{
   struct wc_flat_instr *i;
   switch (instr->type) {
   case nir_instr_type_alu: {
      nir_alu_instr *alu = nir_instr_as_alu(instr);
      if (!(i = next_instr(f, WC_FLAT_ALU)))
         return false;
      i->op = alu_op(alu->op);
      i->name = nir_op_infos[alu->op].name;
      set_def(i, &alu->def);
      for (unsigned s = 0; s < nir_op_infos[alu->op].num_inputs; s++) {
         set_src(i, s, &alu->src[s].src);
         for (unsigned c = 0; c < WC_COMPONENTS; c++)
            i->src[s].swizzle[c] = alu->src[s].swizzle[c];
      }
      return true;
   }
   case nir_instr_type_load_const: {
      nir_load_const_instr *lc = nir_instr_as_load_const(instr);
      if (!(i = next_instr(f, WC_FLAT_CONST)))
         return false;
      set_def(i, &lc->def);
      for (unsigned c = 0; c < lc->def.num_components; c++)
         i->value[c] = lc->def.bit_size == 1 ? lc->value[c].b : lc->value[c].u32;
      if (lc->def.bit_size != 1 && lc->def.bit_size != 32) {
         i->kind = WC_FLAT_UNSUPPORTED;
         i->name = "load_const of other than 32 bits";
      }
      return true;
   }
   case nir_instr_type_undef: {
      nir_undef_instr *u = nir_instr_as_undef(instr);
      if (!(i = next_instr(f, WC_FLAT_UNDEF)))
         return false;
      set_def(i, &u->def);
      return true;
   }
   case nir_instr_type_deref: {
      nir_deref_instr *d = nir_instr_as_deref(instr);
      if (!(i = next_instr(f, WC_FLAT_DEREF)))
         return false;
      set_def(i, &d->def);
      if (!type_index(f, d->type, &i->type))
         return false;
      switch (d->deref_type) {
      case nir_deref_type_var: {
         i->op = WC_DEREF_VAR;
         struct hash_entry *v = _mesa_hash_table_search(f->vars, d->var);
         i->value[0] = v ? (uint32_t)(uintptr_t)v->data : WC_NONE;
         break;
      }
      case nir_deref_type_array:
         i->op = WC_DEREF_ARRAY;
         set_src(i, 0, &d->parent);
         set_src(i, 1, &d->arr.index);
         break;
      case nir_deref_type_struct:
         i->op = WC_DEREF_STRUCT;
         set_src(i, 0, &d->parent);
         i->value[0] = d->strct.index;
         break;
      case nir_deref_type_cast:
         i->op = WC_DEREF_CAST;
         set_src(i, 0, &d->parent);
         i->value[0] = mode_of(d->modes);
         break;
      default:
         i->kind = WC_FLAT_UNSUPPORTED;
         i->name = "a deref of another kind";
      }
      return true;
   }
   case nir_instr_type_intrinsic:
      return flatten_intrinsic(f, nir_instr_as_intrinsic(instr));
   case nir_instr_type_tex:
      return flatten_tex(f, nir_instr_as_tex(instr));
   case nir_instr_type_jump: {
      nir_jump_instr *j = nir_instr_as_jump(instr);
      enum wc_flat_kind kind = j->type == nir_jump_break      ? WC_FLAT_BREAK
                               : j->type == nir_jump_continue ? WC_FLAT_CONTINUE
                               : j->type == nir_jump_return   ? WC_FLAT_RETURN
                                                              : WC_FLAT_UNSUPPORTED;
      if (!(i = next_instr(f, kind)))
         return false;
      i->name = "a jump of another kind";
      return true;
   }
   default:
      if (!(i = next_instr(f, WC_FLAT_UNSUPPORTED)))
         return false;
      i->name = "an instruction of another kind";
      return true;
   }
}

static bool
flatten_list(struct flattening *f, struct exec_list *list)
{
   foreach_list_typed(nir_cf_node, node, node, list) {
      switch (node->type) {
      case nir_cf_node_block:
         nir_foreach_instr(instr, nir_cf_node_as_block(node)) {
            if (!flatten_instr(f, instr))
               return false;
         }
         break;
      case nir_cf_node_if: {
         nir_if *nif = nir_cf_node_as_if(node);
         struct wc_flat_instr *i = next_instr(f, WC_FLAT_IF);
         if (!i)
            return false;
         set_src(i, 0, &nif->condition);
         if (!flatten_list(f, &nif->then_list) || !next_instr(f, WC_FLAT_ELSE) ||
             !flatten_list(f, &nif->else_list) || !next_instr(f, WC_FLAT_END_IF))
            return false;
         break;
      }
      case nir_cf_node_loop: {
         nir_loop *loop = nir_cf_node_as_loop(node);
         struct wc_flat_instr *i = next_instr(f, WC_FLAT_LOOP);
         if (!i)
            return false;
         if (nir_loop_has_continue_construct(loop)) {
            i->kind = WC_FLAT_UNSUPPORTED;
            i->name = "a loop with a continue construct";
         }
         if (!flatten_list(f, &loop->body) || !next_instr(f, WC_FLAT_END_LOOP))
            return false;
         break;
      }
      default:
         break;
      }
   }
   return true;
}

/* A value used outside its block (or by an if) is kept in a register of its own, except registers'
 * declarations, constants (made again at each use) and derefs (made again in each block). */
static bool
value_to_register(nir_def *def, void *data)
{
   nir_builder *b = data;
   nir_instr *instr = nir_def_instr(def);
   if (instr->type == nir_instr_type_load_const || instr->type == nir_instr_type_deref ||
       (instr->type == nir_instr_type_intrinsic &&
        nir_instr_as_intrinsic(instr)->intrinsic == nir_intrinsic_decl_reg))
      return true;
   bool local = true;
   nir_foreach_use_including_if(use, def) {
      if (nir_src_is_if(use) || nir_src_use_instr(use)->block != instr->block)
         local = false;
   }
   if (local)
      return true;
   b->cursor = nir_before_impl(b->impl);
   nir_def *reg = nir_decl_reg(b, def->num_components, def->bit_size, 0);
   nir_rewrite_uses_to_load_reg(b, def, reg);
   b->cursor = nir_after_instr(instr);
   nir_store_reg(b, def, reg);
   return true;
}

bool
wc_nir_flatten(struct nir_shader *nir, struct wc_flat *out)
{
   memset(out, 0, sizeof(*out));
   nir_function_impl *impl = nir_shader_get_entrypoint(nir);
   NIR_PASS(_, nir, nir_lower_vars_to_ssa);
   NIR_PASS(_, nir, nir_convert_from_ssa, true, false);
   nir_rematerialize_derefs_in_use_blocks_impl(impl);
   nir_builder b = nir_builder_create(impl);
   nir_foreach_block(block, impl) {
      nir_foreach_instr_safe(instr, block) {
         nir_foreach_def(instr, value_to_register, &b);
      }
   }
   nir_index_ssa_defs(impl);

   out->stage = nir->info.stage == MESA_SHADER_VERTEX     ? WC_FLAT_VERTEX
                : nir->info.stage == MESA_SHADER_FRAGMENT ? WC_FLAT_FRAGMENT
                                                          : WC_FLAT_COMPUTE;
   for (unsigned d = 0; d < ARRAY_SIZE(out->workgroup_size); d++)
      out->workgroup_size[d] = nir->info.workgroup_size[d];
   out->early_fragment_tests =
      nir->info.stage == MESA_SHADER_FRAGMENT && nir->info.fs.early_fragment_tests;
   out->values = impl->ssa_alloc;

   struct flattening f = {
      .out = out,
      .types = _mesa_pointer_hash_table_create(NULL),
      .vars = _mesa_pointer_hash_table_create(NULL),
   };
   bool ok = true;
   nir_foreach_variable_in_shader(var, nir) {
      ok = ok && add_var(&f, nir, var);
   }
   nir_foreach_function_temp_variable(var, impl) {
      ok = ok && add_var(&f, nir, var);
   }
   ok = ok && flatten_list(&f, &impl->body);
   _mesa_hash_table_destroy(f.types, NULL);
   _mesa_hash_table_destroy(f.vars, NULL);
   if (!ok)
      wc_flat_free(out);
   return ok;
}

void
wc_flat_free(struct wc_flat *flat)
{
   free(flat->types);
   free(flat->fields);
   free(flat->vars);
   free(flat->initialisers);
   free(flat->instrs);
   memset(flat, 0, sizeof(*flat));
}
