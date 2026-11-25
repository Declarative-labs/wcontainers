/* Candidate B's flattening of NIR (flat.h). */
#include <stdbool.h>
#include "flat.h"
#include "bridge.h"
#include "bridge_private.h"

#include <stdlib.h>
#include <string.h>

#include "compiler/glsl_types.h"
#include "nir.h"
#include "nir_builder.h"
#include "util/format/u_formats.h"
#include "util/hash_table.h"

/* A growable array of one element type; on failure `cleanup` runs and the function fails. */
#define GROW_OR(array, count, capacity, cleanup)                                                   \
   do {                                                                                            \
      if ((count) == (capacity)) {                                                                 \
         (capacity) = (capacity) ? (capacity) * 2 : 64;                                            \
         void *grown = realloc((array), (capacity) * sizeof(*(array)));                            \
         if (!grown) {                                                                             \
            cleanup;                                                                               \
            return false;                                                                          \
         }                                                                                         \
         (array) = grown;                                                                          \
      }                                                                                            \
   } while (0)
#define GROW(array, count, capacity) GROW_OR(array, count, capacity, (void)0)

/* A stage value of lowered IO (load_input, load_interpolated_input, store_output): one variable a
 * slot, its type and interpolation gathered from the intrinsics that reach it. */
struct io_slot {
   bool output;
   /* One value a vertex of the input primitive (or a control point of a patch): an array of them. */
   bool per_vertex;
   /* One value for a patch. */
   bool patch;
   unsigned stream;
   unsigned location;
   /* A fragment output's blend source: 1 for dual-source blending's second. */
   unsigned blend_source;
   nir_alu_type base;
   unsigned components;
   /* A fragment input's load: load_input (flat) or the barycentric's op; nir_num_intrinsics before
    * the first. */
   nir_intrinsic_op load;
   enum glsl_interp_mode interp;
   /* An output every store to which is made exactly (`made_exactly`); false before the first. */
   bool invariant;
   unsigned stores;
   const char *refused;
};

struct flattening {
   struct wc_flat *out;
   const nir_shader *nir;
   uint32_t type_capacity, field_capacity, var_capacity, instr_capacity, initialiser_capacity;
   struct hash_table *types;
   struct hash_table *vars;
   struct io_slot *slots;
   uint32_t slot_count, slot_capacity;
   /* The variable of the first slot; the others follow in order. */
   uint32_t first_slot_var;
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
   /* Rectangles' coordinates are normalized before (nir_lower_tex's lower_rect). */
   case GLSL_SAMPLER_DIM_RECT:
      return WC_DIM_2D;
   case GLSL_SAMPLER_DIM_3D:
      return WC_DIM_3D;
   case GLSL_SAMPLER_DIM_CUBE:
      return WC_DIM_CUBE;
   case GLSL_SAMPLER_DIM_BUF:
      return WC_DIM_BUFFER;
   case GLSL_SAMPLER_DIM_SUBPASS:
   case GLSL_SAMPLER_DIM_SUBPASS_MS:
      return WC_DIM_SUBPASS;
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
      /* The fields' types first, since flattening them adds fields of their own. */
      uint32_t *field_types = malloc((t.length ? t.length : 1) * sizeof(*field_types));
      if (!field_types)
         return false;
      for (uint32_t i = 0; i < t.length; i++) {
         if (!type_index(f, glsl_get_struct_field(type, i), &field_types[i])) {
            free(field_types);
            return false;
         }
      }
      t.element = f->out->field_count;
      for (uint32_t i = 0; i < t.length; i++) {
         GROW_OR(f->out->fields, f->out->field_count, f->field_capacity, free(field_types));
         f->out->fields[f->out->field_count++] = (struct wc_flat_field){
            .type = field_types[i],
            .offset = glsl_get_struct_field_offset(type, i),
         };
      }
      free(field_types);
   } else if (glsl_type_is_matrix(type)) {
      t.kind = WC_TYPE_MATRIX;
      /* Only a matrix laid out in memory has a layout; any other is column-major. */
      t.row_major = glsl_get_explicit_stride(type) && glsl_matrix_type_is_row_major(type);
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
   case PIPE_FORMAT_R8_UNORM:
      return WC_FORMAT_R8_UNORM;
   case PIPE_FORMAT_R8_SNORM:
      return WC_FORMAT_R8_SNORM;
   case PIPE_FORMAT_R8_UINT:
      return WC_FORMAT_R8_UINT;
   case PIPE_FORMAT_R8_SINT:
      return WC_FORMAT_R8_SINT;
   case PIPE_FORMAT_R16_UINT:
      return WC_FORMAT_R16_UINT;
   case PIPE_FORMAT_R16_SINT:
      return WC_FORMAT_R16_SINT;
   case PIPE_FORMAT_R16_FLOAT:
      return WC_FORMAT_R16_FLOAT;
   case PIPE_FORMAT_R16_UNORM:
      return WC_FORMAT_R16_UNORM;
   case PIPE_FORMAT_R16_SNORM:
      return WC_FORMAT_R16_SNORM;
   case PIPE_FORMAT_R8G8_UNORM:
      return WC_FORMAT_RG8_UNORM;
   case PIPE_FORMAT_R8G8_SNORM:
      return WC_FORMAT_RG8_SNORM;
   case PIPE_FORMAT_R8G8_UINT:
      return WC_FORMAT_RG8_UINT;
   case PIPE_FORMAT_R8G8_SINT:
      return WC_FORMAT_RG8_SINT;
   case PIPE_FORMAT_R32_UINT:
      return WC_FORMAT_R32_UINT;
   case PIPE_FORMAT_R32_SINT:
      return WC_FORMAT_R32_SINT;
   case PIPE_FORMAT_R32_FLOAT:
      return WC_FORMAT_R32_FLOAT;
   case PIPE_FORMAT_R16G16_UINT:
      return WC_FORMAT_RG16_UINT;
   case PIPE_FORMAT_R16G16_SINT:
      return WC_FORMAT_RG16_SINT;
   case PIPE_FORMAT_R16G16_FLOAT:
      return WC_FORMAT_RG16_FLOAT;
   case PIPE_FORMAT_R16G16_UNORM:
      return WC_FORMAT_RG16_UNORM;
   case PIPE_FORMAT_R16G16_SNORM:
      return WC_FORMAT_RG16_SNORM;
   case PIPE_FORMAT_R8G8B8A8_UNORM:
      return WC_FORMAT_RGBA8_UNORM;
   case PIPE_FORMAT_R8G8B8A8_SNORM:
      return WC_FORMAT_RGBA8_SNORM;
   case PIPE_FORMAT_R8G8B8A8_UINT:
      return WC_FORMAT_RGBA8_UINT;
   case PIPE_FORMAT_R8G8B8A8_SINT:
      return WC_FORMAT_RGBA8_SINT;
   case PIPE_FORMAT_R10G10B10A2_UINT:
      return WC_FORMAT_RGB10A2_UINT;
   case PIPE_FORMAT_R10G10B10A2_UNORM:
      return WC_FORMAT_RGB10A2_UNORM;
   case PIPE_FORMAT_R11G11B10_FLOAT:
      return WC_FORMAT_RG11B10_UFLOAT;
   case PIPE_FORMAT_R32G32_UINT:
      return WC_FORMAT_RG32_UINT;
   case PIPE_FORMAT_R32G32_SINT:
      return WC_FORMAT_RG32_SINT;
   case PIPE_FORMAT_R32G32_FLOAT:
      return WC_FORMAT_RG32_FLOAT;
   case PIPE_FORMAT_R16G16B16A16_UINT:
      return WC_FORMAT_RGBA16_UINT;
   case PIPE_FORMAT_R16G16B16A16_SINT:
      return WC_FORMAT_RGBA16_SINT;
   case PIPE_FORMAT_R16G16B16A16_FLOAT:
      return WC_FORMAT_RGBA16_FLOAT;
   case PIPE_FORMAT_R16G16B16A16_UNORM:
      return WC_FORMAT_RGBA16_UNORM;
   case PIPE_FORMAT_R16G16B16A16_SNORM:
      return WC_FORMAT_RGBA16_SNORM;
   case PIPE_FORMAT_R32G32B32A32_UINT:
      return WC_FORMAT_RGBA32_UINT;
   case PIPE_FORMAT_R32G32B32A32_SINT:
      return WC_FORMAT_RGBA32_SINT;
   case PIPE_FORMAT_R32G32B32A32_FLOAT:
      return WC_FORMAT_RGBA32_FLOAT;
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

enum wc_builtin
wc_builtin_at(const nir_shader *nir, bool output, unsigned slot, uint32_t *location)
{
   *location = 0;
   if (!output && nir->info.stage == MESA_SHADER_VERTEX) {
      if (slot < VERT_ATTRIB_GENERIC0) {
         *location = slot;
         return WC_BUILTIN_OTHER;
      }
      *location = slot - VERT_ATTRIB_GENERIC0;
      return WC_BUILTIN_NONE;
   }
   if (output && nir->info.stage == MESA_SHADER_FRAGMENT) {
      if (slot == FRAG_RESULT_DEPTH)
         return WC_BUILTIN_FRAG_DEPTH;
      if (slot == FRAG_RESULT_SAMPLE_MASK)
         return WC_BUILTIN_SAMPLE_MASK;
      if (slot < FRAG_RESULT_DATA0) {
         *location = slot;
         return WC_BUILTIN_OTHER;
      }
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
   case VARYING_SLOT_LAYER:
      return WC_BUILTIN_LAYER;
   case VARYING_SLOT_VIEWPORT:
      return WC_BUILTIN_VIEWPORT_INDEX;
   case VARYING_SLOT_PRIMITIVE_ID:
      return WC_BUILTIN_PRIMITIVE_ID;
   case VARYING_SLOT_TESS_LEVEL_OUTER:
      return WC_BUILTIN_TESS_LEVEL_OUTER;
   case VARYING_SLOT_TESS_LEVEL_INNER:
      return WC_BUILTIN_TESS_LEVEL_INNER;
   case VARYING_SLOT_PNTC:
      return WC_BUILTIN_POINT_COORD;
   case VARYING_SLOT_COL0:
   case VARYING_SLOT_COL1:
   case VARYING_SLOT_BFC0:
   case VARYING_SLOT_BFC1:
   case VARYING_SLOT_FOGC:
      /* OpenGL's own stage values: after the generic ones, by their slot. */
      *location = MAX_VARYING + slot;
      return WC_BUILTIN_NONE;
   default:
      if (slot >= VARYING_SLOT_TEX0 && slot <= VARYING_SLOT_TEX7) {
         *location = MAX_VARYING + slot;
         return WC_BUILTIN_NONE;
      }
      if (slot < VARYING_SLOT_VAR0) {
         *location = slot;
         return WC_BUILTIN_OTHER;
      }
      /* A patch's own values have locations of their own. */
      if (slot >= VARYING_SLOT_PATCH0) {
         *location = slot - VARYING_SLOT_PATCH0;
         return WC_BUILTIN_NONE;
      }
      *location = slot - VARYING_SLOT_VAR0;
      return WC_BUILTIN_NONE;
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
add_var(struct flattening *f, nir_variable *var)
{
   struct wc_flat_var v = {
      .mode = mode_of(var->data.mode),
      .set = var->data.descriptor_set,
      .binding = var->data.binding,
      .readable = !(var->data.access & ACCESS_NON_READABLE),
      .writable = !(var->data.access & ACCESS_NON_WRITEABLE),
      .format = format_of(var->data.image.format),
      .initialised = WC_NONE,
   };
   if (!type_index(f, var->type, &v.type))
      return false;
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

/* The bits of one component's stream in nir_io_semantics.gs_streams. */
#define WC_STREAM_BITS 2
#define WC_STREAM_MASK ((1u << WC_STREAM_BITS) - 1)

/* What an IO intrinsic reaches: its slot (the clip distances folded into one array) and its first
 * component, or the reason it is refused. */
struct io_access {
   /* An output slot, and whether the intrinsic writes it (a control program also reads them). */
   bool output;
   bool store;
   /* One value a vertex of the input primitive or a control point, its vertex the intrinsic's
    * vertex source. */
   bool per_vertex;
   /* One value for a patch. */
   bool patch;
   /* A geometry program's output stream. */
   unsigned stream;
   unsigned location;
   unsigned blend_source;
   unsigned first;
   unsigned count;
   nir_alu_type base;
   const char *refused;
};

static bool
io_access(const nir_shader *nir, nir_intrinsic_instr *intr, struct io_access *a)
{
   bool output;
   switch (intr->intrinsic) {
   case nir_intrinsic_load_input:
   case nir_intrinsic_load_interpolated_input:
   case nir_intrinsic_load_per_vertex_input:
      output = false;
      break;
   case nir_intrinsic_store_output:
      output = true;
      break;
   case nir_intrinsic_store_per_vertex_output:
   case nir_intrinsic_load_output:
   case nir_intrinsic_load_per_vertex_output:
      /* A control program writes one control point's values and reads any of its outputs. */
      if (nir->info.stage != MESA_SHADER_TESS_CTRL)
         return false;
      output = true;
      break;
   default:
      return false;
   }
   bool store = intr->intrinsic == nir_intrinsic_store_output ||
                intr->intrinsic == nir_intrinsic_store_per_vertex_output;
   nir_io_semantics semantics = nir_intrinsic_io_semantics(intr);
   nir_src *offset = nir_get_io_offset_src(intr);
   const nir_def *value = store ? intr->src[0].ssa : &intr->def;
   *a = (struct io_access){
      .output = output,
      .store = store,
      .per_vertex = intr->intrinsic == nir_intrinsic_load_per_vertex_input ||
                    intr->intrinsic == nir_intrinsic_store_per_vertex_output ||
                    intr->intrinsic == nir_intrinsic_load_per_vertex_output,
      .patch = semantics.location >= VARYING_SLOT_PATCH0 ||
               semantics.location == VARYING_SLOT_TESS_LEVEL_OUTER ||
               semantics.location == VARYING_SLOT_TESS_LEVEL_INNER,
      .location = semantics.location,
      .first = nir_intrinsic_component(intr),
      .count = value->num_components,
      .base = nir_alu_type_get_base_type(store ? nir_intrinsic_src_type(intr)
                                               : nir_intrinsic_dest_type(intr)),
   };
   if (output && nir->info.stage == MESA_SHADER_GEOMETRY) {
      unsigned streams = semantics.gs_streams;
      a->stream = streams & WC_STREAM_MASK;
      for (unsigned c = 1; c < a->count; c++) {
         if (((streams >> (c * WC_STREAM_BITS)) & WC_STREAM_MASK) != a->stream)
            a->refused = "a stage value whose components go to different streams";
      }
   }
   if (nir_src_is_const(*offset))
      a->location += nir_src_as_uint(*offset);
   else
      a->refused = "a stage value indexed at run time";
   if (value->bit_size != WC_VALUE_BITS)
      a->refused = "a stage value of other than 32 bits";
   a->blend_source = semantics.dual_source_blend_index;
   bool varying = (nir->info.stage == MESA_SHADER_FRAGMENT) != output;
   if (varying &&
       (a->location == VARYING_SLOT_CLIP_DIST0 || a->location == VARYING_SLOT_CLIP_DIST1)) {
      a->first += (a->location - VARYING_SLOT_CLIP_DIST0) * WC_COMPONENTS;
      a->location = VARYING_SLOT_CLIP_DIST0;
   }
   return true;
}

static struct io_slot *
find_slot(struct flattening *f, bool output, unsigned location, unsigned blend_source)
{
   for (uint32_t n = 0; n < f->slot_count; n++) {
      if (f->slots[n].output == output && f->slots[n].location == location &&
          f->slots[n].blend_source == blend_source)
         return &f->slots[n];
   }
   return NULL;
}

static bool
is_barycentric(nir_intrinsic_op op)
{
   switch (op) {
   case nir_intrinsic_load_barycentric_pixel:
   case nir_intrinsic_load_barycentric_centroid:
   case nir_intrinsic_load_barycentric_sample:
   case nir_intrinsic_load_barycentric_at_sample:
   case nir_intrinsic_load_barycentric_at_offset:
      return true;
   default:
      return false;
   }
}

/* Whether a value is made exactly: GLSL's linker marks every instruction an `invariant` or
 * `precise` output is made from exact, and moves and vectors gather such values. */
static bool
made_exactly(const nir_def *def)
{
   nir_instr *instr = nir_def_instr(def);
   if (instr->type != nir_instr_type_alu)
      return false;
   nir_alu_instr *alu = nir_instr_as_alu(instr);
   if (nir_alu_instr_is_exact(alu))
      return true;
   if (!nir_op_is_vec_or_mov(alu->op))
      return false;
   for (unsigned i = 0; i < nir_op_infos[alu->op].num_inputs; i++) {
      if (made_exactly(alu->src[i].src.ssa))
         return true;
   }
   return false;
}

/* Every slot the IO intrinsics reach, with the widest access, one kind (u32 where they disagree)
 * and a fragment input's one way of interpolation. */
static bool
gather_slots(struct flattening *f, nir_function_impl *impl)
{
   nir_foreach_block(block, impl) {
      nir_foreach_instr(instr, block) {
         if (instr->type != nir_instr_type_intrinsic)
            continue;
         nir_intrinsic_instr *intr = nir_instr_as_intrinsic(instr);
         struct io_access a;
         if (!io_access(f->nir, intr, &a) || a.refused)
            continue;
         struct io_slot *s = find_slot(f, a.output, a.location, a.blend_source);
         if (!s) {
            GROW(f->slots, f->slot_count, f->slot_capacity);
            s = &f->slots[f->slot_count++];
            *s = (struct io_slot){
               .output = a.output,
               .per_vertex = a.per_vertex,
               .patch = a.patch,
               .stream = a.stream,
               .location = a.location,
               .blend_source = a.blend_source,
               .base = a.base,
               .load = nir_num_intrinsics,
            };
         }
         if (s->base != a.base)
            s->base = nir_type_uint;
         if (s->per_vertex != a.per_vertex)
            s->refused = "a stage value read both for one vertex and for the primitive";
         if (s->stream != a.stream)
            s->refused = "a stage value written to two streams";
         s->components = MAX2(s->components, a.first + a.count);
         if (a.store) {
            bool exact = made_exactly(intr->src[0].ssa);
            s->invariant = s->stores == 0 ? exact : s->invariant && exact;
            s->stores++;
         }
         if (a.output || f->nir->info.stage != MESA_SHADER_FRAGMENT)
            continue;
         nir_intrinsic_op load = nir_intrinsic_load_input;
         enum glsl_interp_mode interp = INTERP_MODE_FLAT;
         if (intr->intrinsic == nir_intrinsic_load_interpolated_input) {
            nir_intrinsic_instr *barycentric = nir_src_as_intrinsic(intr->src[0]);
            load = barycentric->intrinsic;
            interp = nir_intrinsic_interp_mode(barycentric);
            if (load == nir_intrinsic_load_barycentric_at_sample ||
                load == nir_intrinsic_load_barycentric_at_offset)
               s->refused = "interpolation at a sample or an offset";
         }
         if (s->load != nir_num_intrinsics && (s->load != load || s->interp != interp))
            s->refused = "a stage value interpolated two ways";
         s->load = load;
         s->interp = interp;
      }
   }
   return true;
}

static enum glsl_base_type
base_type_of(nir_alu_type base)
{
   switch (base) {
   case nir_type_float:
      return GLSL_TYPE_FLOAT;
   case nir_type_int:
      return GLSL_TYPE_INT;
   default:
      return GLSL_TYPE_UINT;
   }
}

/* The vertices or control points a per-vertex value is held for. */
static uint32_t
per_vertex_count(const struct flattening *f, bool output)
{
   switch (f->nir->info.stage) {
   case MESA_SHADER_TESS_CTRL:
      return output ? f->out->tessellation.points_out : f->out->tessellation.points_in;
   case MESA_SHADER_TESS_EVAL:
      return f->out->tessellation.points_in;
   default:
      return f->out->geometry.vertices_in;
   }
}

/* One variable a slot, after the shader's own. */
static bool
add_slots(struct flattening *f)
{
   f->first_slot_var = f->out->var_count;
   for (uint32_t n = 0; n < f->slot_count; n++) {
      const struct io_slot *s = &f->slots[n];
      struct wc_flat_var v = {
         .mode = s->output ? WC_VAR_OUTPUT : WC_VAR_INPUT,
         .readable = true,
         .writable = true,
         .format = WC_FORMAT_NONE,
         .initialised = WC_NONE,
      };
      v.builtin = wc_builtin_at(f->nir, s->output, s->location, &v.location);
      /* A built-in has its own type whatever part of it the program reaches. */
      const struct glsl_type *type =
         v.builtin == WC_BUILTIN_CLIP_DISTANCE ? glsl_array_type(glsl_float_type(), s->components, 0)
         : v.builtin == WC_BUILTIN_TESS_LEVEL_OUTER
            ? glsl_array_type(glsl_float_type(), WC_TESS_OUTER_LEVELS, 0)
         : v.builtin == WC_BUILTIN_TESS_LEVEL_INNER
            ? glsl_array_type(glsl_float_type(), WC_TESS_INNER_LEVELS, 0)
         : v.builtin == WC_BUILTIN_POSITION ? glsl_vec4_type()
         : v.builtin == WC_BUILTIN_SAMPLE_MASK ? glsl_uint_type()
         : v.builtin == WC_BUILTIN_FRAG_DEPTH || v.builtin == WC_BUILTIN_POINT_SIZE
            ? glsl_float_type()
            : v.builtin == WC_BUILTIN_LAYER || v.builtin == WC_BUILTIN_VIEWPORT_INDEX ||
                  v.builtin == WC_BUILTIN_PRIMITIVE_ID
               ? glsl_uint_type()
               : glsl_vector_type(base_type_of(s->base), s->components);
      if (s->per_vertex)
         type = glsl_array_type(type, per_vertex_count(f, s->output), 0);
      if (!type_index(f, type, &v.type))
         return false;
      /* Lowered IO keeps the interpolation only where a fragment input is loaded; an output takes
       * the default of its kind. */
      v.interpolation = s->base == nir_type_float ? WC_INTERP_PERSPECTIVE : WC_INTERP_FLAT;
      if (s->load == nir_intrinsic_load_input)
         v.interpolation = WC_INTERP_FLAT;
      else if (s->load != nir_num_intrinsics)
         v.interpolation = s->interp == INTERP_MODE_NOPERSPECTIVE ? WC_INTERP_LINEAR
                           : s->interp == INTERP_MODE_FLAT        ? WC_INTERP_FLAT
                                                                  : WC_INTERP_PERSPECTIVE;
      v.stream = s->stream;
      v.blend_source = s->blend_source;
      v.patch = s->patch;
      v.invariant = s->invariant;
      v.centroid = s->load == nir_intrinsic_load_barycentric_centroid;
      v.sample = s->load == nir_intrinsic_load_barycentric_sample;
      GROW(f->out->vars, f->out->var_count, f->var_capacity);
      f->out->vars[f->out->var_count++] = v;
   }
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
   /* The value itself: the host keeps denormals and has no other canonical form, as Mesa's own
    * algebraic step reads it where denormals are not flushed. */
   case nir_op_fcanonicalize:
      return WC_ALU_mov;
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
   case nir_intrinsic_load_point_coord:
      return WC_INTRINSIC_LOAD_POINT_COORD;
   case nir_intrinsic_image_deref_load:
      return WC_INTRINSIC_IMAGE_DEREF_LOAD;
   case nir_intrinsic_image_deref_store:
      return WC_INTRINSIC_IMAGE_DEREF_STORE;
   case nir_intrinsic_image_deref_size:
      return WC_INTRINSIC_IMAGE_DEREF_SIZE;
   case nir_intrinsic_deref_atomic:
      return WC_INTRINSIC_DEREF_ATOMIC;
   case nir_intrinsic_deref_atomic_swap:
      return WC_INTRINSIC_DEREF_ATOMIC_SWAP;
   case nir_intrinsic_demote_samples:
      return WC_INTRINSIC_DEMOTE_SAMPLES;
   case nir_intrinsic_load_sample_mask_in:
      return WC_INTRINSIC_LOAD_SAMPLE_MASK_IN;
   case nir_intrinsic_load_primitive_id:
      return WC_INTRINSIC_LOAD_PRIMITIVE_ID;
   case nir_intrinsic_load_invocation_id:
      return WC_INTRINSIC_LOAD_INVOCATION_ID;
   case nir_intrinsic_emit_vertex:
   case nir_intrinsic_emit_vertex_with_counter:
      return WC_INTRINSIC_EMIT_VERTEX;
   case nir_intrinsic_end_primitive:
   case nir_intrinsic_end_primitive_with_counter:
      return WC_INTRINSIC_END_PRIMITIVE;
   case nir_intrinsic_load_tess_coord:
      return WC_INTRINSIC_LOAD_TESS_COORD;
   case nir_intrinsic_load_patch_vertices_in:
      return WC_INTRINSIC_LOAD_PATCH_VERTICES_IN;
   case nir_intrinsic_image_deref_atomic:
      return WC_INTRINSIC_IMAGE_DEREF_ATOMIC;
   case nir_intrinsic_image_deref_atomic_swap:
      return WC_INTRINSIC_IMAGE_DEREF_ATOMIC_SWAP;
   case nir_intrinsic_load_helper_invocation:
      return WC_INTRINSIC_LOAD_HELPER_INVOCATION;
   default:
      return WC_INTRINSIC_UNSUPPORTED;
   }
}

static enum wc_atomic_op
atomic_of(nir_atomic_op op)
{
   switch (op) {
   case nir_atomic_op_iadd:
      return WC_ATOMIC_ADD;
   case nir_atomic_op_imin:
      return WC_ATOMIC_IMIN;
   case nir_atomic_op_umin:
      return WC_ATOMIC_UMIN;
   case nir_atomic_op_imax:
      return WC_ATOMIC_IMAX;
   case nir_atomic_op_umax:
      return WC_ATOMIC_UMAX;
   case nir_atomic_op_iand:
      return WC_ATOMIC_AND;
   case nir_atomic_op_ior:
      return WC_ATOMIC_OR;
   case nir_atomic_op_ixor:
      return WC_ATOMIC_XOR;
   case nir_atomic_op_xchg:
      return WC_ATOMIC_EXCHANGE;
   case nir_atomic_op_cmpxchg:
      return WC_ATOMIC_COMPARE_EXCHANGE;
   default:
      return WC_ATOMIC_OTHER;
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
   /* A multisampled texture is two-dimensional; the texture's type says it is multisampled. */
   i->dim = tex->sampler_dim == GLSL_SAMPLER_DIM_MS ? WC_DIM_2D : dim_of(tex->sampler_dim);
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
   /* A barycentric is read only as the interpolation of its loads (gather_slots). */
   if (is_barycentric(intr->intrinsic))
      return true;
   /* The counts a geometry program's lowering keeps are the stage helper's own. */
   if (intr->intrinsic == nir_intrinsic_set_vertex_and_primitive_count)
      return true;
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
   if (nir_intrinsic_has_atomic_op(intr))
      i->value[WC_COMPONENTS - 1] = atomic_of(nir_intrinsic_atomic_op(intr));
   if (nir_intrinsic_has_stream_id(intr))
      i->value[0] = nir_intrinsic_stream_id(intr);
   if (intr->intrinsic == nir_intrinsic_barrier) {
      i->value[0] = nir_intrinsic_execution_scope(intr) == SCOPE_WORKGROUP;
      i->value[1] = (nir_intrinsic_memory_modes(intr) & nir_var_mem_shared) != 0;
      i->value[2] = (nir_intrinsic_memory_modes(intr) & (nir_var_mem_ssbo | nir_var_image)) != 0;
   }
   struct io_access a;
   if (io_access(f->nir, intr, &a)) {
      const struct io_slot *s =
         a.refused ? NULL : find_slot(f, a.output, a.location, a.blend_source);
      const char *refused = s ? s->refused : a.refused;
      if (refused) {
         i->kind = WC_FLAT_UNSUPPORTED;
         i->name = refused;
         return true;
      }
      i->op = a.store ? WC_INTRINSIC_STORE_STAGE : WC_INTRINSIC_LOAD_STAGE;
      i->value[0] = f->first_slot_var + (uint32_t)(s - f->slots);
      i->component = a.first;
      i->arrayed = a.per_vertex;
   }
   return true;
}

/* The 16-bit operations of Mesa's alpha-to-coverage pass (nir_lower_alpha.c), which route B carries
 * in 32-bit words holding the zero-extended value; no other 16-bit operation is taken. */
static bool
is_narrow_op(nir_op op)
{
   switch (op) {
   case nir_op_mov:
   case nir_op_iand:
   case nir_op_ior:
   case nir_op_imul:
   case nir_op_ishl:
   case nir_op_ushr:
   case nir_op_u2u16:
   case nir_op_u2u32:
      return true;
   default:
      return false;
   }
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
      bool narrow = alu->def.bit_size == WC_NARROW_BITS;
      for (unsigned s = 0; s < nir_op_infos[alu->op].num_inputs; s++) {
         set_src(i, s, &alu->src[s].src);
         for (unsigned c = 0; c < WC_COMPONENTS; c++)
            i->src[s].swizzle[c] = alu->src[s].swizzle[c];
         narrow = narrow || nir_src_bit_size(alu->src[s].src) == WC_NARROW_BITS;
      }
      if (narrow && !is_narrow_op(alu->op)) {
         i->kind = WC_FLAT_UNSUPPORTED;
         i->name = "a 16-bit operation other than the alpha-to-coverage pass's";
      }
      return true;
   }
   case nir_instr_type_load_const: {
      nir_load_const_instr *lc = nir_instr_as_load_const(instr);
      if (!(i = next_instr(f, WC_FLAT_CONST)))
         return false;
      set_def(i, &lc->def);
      for (unsigned c = 0; c < lc->def.num_components; c++)
         i->value[c] = lc->def.bit_size == 1                ? lc->value[c].b
                       : lc->def.bit_size == WC_NARROW_BITS ? lc->value[c].u16
                                                            : lc->value[c].u32;
      if (lc->def.bit_size != 1 && lc->def.bit_size != WC_NARROW_BITS &&
          lc->def.bit_size != WC_VALUE_BITS) {
         i->kind = WC_FLAT_UNSUPPORTED;
         i->name = "load_const of other than 16 or 32 bits";
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

/* What is made again in each block that uses it, as derefs are: a buffer's descriptor and a
 * barycentric, which never need a register. */
static bool
made_in_each_block(nir_intrinsic_op op)
{
   return op == nir_intrinsic_vulkan_resource_index ||
          op == nir_intrinsic_load_vulkan_descriptor || is_barycentric(op);
}

/* Walked backwards so that a copy's own descriptor is copied in turn. */
static void
rematerialize_in_use_blocks(nir_function_impl *impl)
{
   nir_foreach_block_reverse(block, impl) {
      nir_foreach_instr_reverse_safe(instr, block) {
         if (instr->type != nir_instr_type_intrinsic)
            continue;
         nir_intrinsic_instr *intr = nir_instr_as_intrinsic(instr);
         if (!made_in_each_block(intr->intrinsic))
            continue;
         nir_foreach_use_including_if_safe(use, &intr->def) {
            if (nir_src_is_if(use) || nir_src_use_instr(use)->block == block)
               continue;
            nir_instr *copy = nir_instr_clone(impl->function->shader, instr);
            nir_instr_insert_before(nir_src_use_instr(use), copy);
            nir_src_rewrite(use, &nir_instr_as_intrinsic(copy)->def);
         }
      }
   }
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

static struct wc_flat_geometry
geometry_of(const nir_shader *nir)
{
   struct wc_flat_geometry g = {
      .input = WC_GEOMETRY_IN_OTHER,
      .output = WC_GEOMETRY_OUT_OTHER,
      .vertices_in = nir->info.gs.vertices_in,
      .max_vertices = nir->info.gs.vertices_out,
      .invocations = MAX2(nir->info.gs.invocations, 1),
      .streams = util_last_bit(nir->info.gs.active_stream_mask),
   };
   switch (nir->info.gs.input_primitive) {
   case MESA_PRIM_POINTS:
      g.input = WC_GEOMETRY_IN_POINTS;
      break;
   case MESA_PRIM_LINES:
      g.input = WC_GEOMETRY_IN_LINES;
      break;
   case MESA_PRIM_LINES_ADJACENCY:
      g.input = WC_GEOMETRY_IN_LINES_ADJACENCY;
      break;
   case MESA_PRIM_TRIANGLES:
      g.input = WC_GEOMETRY_IN_TRIANGLES;
      break;
   case MESA_PRIM_TRIANGLES_ADJACENCY:
      g.input = WC_GEOMETRY_IN_TRIANGLES_ADJACENCY;
      break;
   default:
      break;
   }
   switch (nir->info.gs.output_primitive) {
   case MESA_PRIM_POINTS:
      g.output = WC_GEOMETRY_OUT_POINTS;
      break;
   case MESA_PRIM_LINE_STRIP:
      g.output = WC_GEOMETRY_OUT_LINE_STRIP;
      break;
   case MESA_PRIM_TRIANGLE_STRIP:
      g.output = WC_GEOMETRY_OUT_TRIANGLE_STRIP;
      break;
   default:
      break;
   }
   return g;
}

static struct wc_flat_tessellation
tessellation_of(const nir_shader *nir, uint32_t points_in)
{
   struct wc_flat_tessellation t = {
      .points_in = points_in,
      .points_out = nir->info.tess.tcs_vertices_out,
      .domain = WC_TESS_DOMAIN_OTHER,
      .spacing = WC_TESS_SPACING_OTHER,
      .counter_clockwise = nir->info.tess.ccw,
      .point_mode = nir->info.tess.point_mode,
   };
   switch (nir->info.tess._primitive_mode) {
   case TESS_PRIMITIVE_TRIANGLES:
      t.domain = WC_TESS_TRIANGLES;
      break;
   case TESS_PRIMITIVE_QUADS:
      t.domain = WC_TESS_QUADS;
      break;
   case TESS_PRIMITIVE_ISOLINES:
      t.domain = WC_TESS_ISOLINES;
      break;
   default:
      break;
   }
   switch (nir->info.tess.spacing) {
   case TESS_SPACING_EQUAL:
      t.spacing = WC_TESS_EQUAL;
      break;
   case TESS_SPACING_FRACTIONAL_ODD:
      t.spacing = WC_TESS_FRACTIONAL_ODD;
      break;
   case TESS_SPACING_FRACTIONAL_EVEN:
      t.spacing = WC_TESS_FRACTIONAL_EVEN;
      break;
   default:
      break;
   }
   return t;
}

/* A read of a control point at a constant index past the input patch's points, which OpenGL
 * leaves undefined (the program's arrays are as long as the most points a patch may have): zero,
 * as the stage helper holds only the patch's own points. */
static bool
missing_point_reads_zero(nir_builder *b, nir_intrinsic_instr *intr, void *data)
{
   const uint32_t *points = data;
   if (intr->intrinsic != nir_intrinsic_load_per_vertex_input)
      return false;
   nir_src *point = nir_get_io_arrayed_index_src(intr);
   if (!nir_src_is_const(*point) || nir_src_as_uint(*point) < *points)
      return false;
   b->cursor = nir_before_instr(&intr->instr);
   nir_def_replace(&intr->def, nir_imm_zero(b, intr->def.num_components, intr->def.bit_size));
   return true;
}

bool
wc_nir_flatten(struct nir_shader *nir, uint32_t points_in, struct wc_flat *out)
{
   memset(out, 0, sizeof(*out));
   nir_function_impl *impl = nir_shader_get_entrypoint(nir);
   /* Constants, and every variable with an initialiser, as variables stored on entry (the
    * linker makes an evaluation program's `gl_PatchVerticesIn` a constant), made values below. */
   nir_lower_constant_to_temp(nir);
   NIR_PASS(_, nir, nir_lower_variable_initializers, nir_var_function_temp | nir_var_shader_temp);
   wc_nir_lower_io(nir);
   if (nir->info.stage == MESA_SHADER_TESS_CTRL || nir->info.stage == MESA_SHADER_TESS_EVAL)
      nir_shader_intrinsics_pass(nir, missing_point_reads_zero, nir_metadata_control_flow, &points_in);
   NIR_PASS(_, nir, nir_lower_vars_to_ssa);
   NIR_PASS(_, nir, nir_convert_from_ssa, true, false);
   nir_rematerialize_derefs_in_use_blocks_impl(impl);
   rematerialize_in_use_blocks(impl);
   nir_builder b = nir_builder_create(impl);
   nir_foreach_block(block, impl) {
      nir_foreach_instr_safe(instr, block) {
         nir_foreach_def(instr, value_to_register, &b);
      }
   }
   nir_index_ssa_defs(impl);

   out->stage = nir->info.stage == MESA_SHADER_VERTEX     ? WC_FLAT_VERTEX
                : nir->info.stage == MESA_SHADER_FRAGMENT ? WC_FLAT_FRAGMENT
                : nir->info.stage == MESA_SHADER_GEOMETRY ? WC_FLAT_GEOMETRY
                : nir->info.stage == MESA_SHADER_TESS_CTRL ? WC_FLAT_TESS_CONTROL
                : nir->info.stage == MESA_SHADER_TESS_EVAL ? WC_FLAT_TESS_EVALUATION
                                                           : WC_FLAT_COMPUTE;
   if (nir->info.stage == MESA_SHADER_GEOMETRY)
      out->geometry = geometry_of(nir);
   if (nir->info.stage == MESA_SHADER_TESS_CTRL || nir->info.stage == MESA_SHADER_TESS_EVAL)
      out->tessellation = tessellation_of(nir, points_in);
   for (unsigned d = 0; d < ARRAY_SIZE(out->workgroup_size); d++)
      out->workgroup_size[d] = nir->info.workgroup_size[d];
   out->early_fragment_tests =
      nir->info.stage == MESA_SHADER_FRAGMENT && nir->info.fs.early_fragment_tests;
   out->values = impl->ssa_alloc;

   struct flattening f = {
      .out = out,
      .nir = nir,
      .types = _mesa_pointer_hash_table_create(NULL),
      .vars = _mesa_pointer_hash_table_create(NULL),
   };
   bool ok = true;
   /* The stage values' own variables are left behind by the lowering; their slots stand for them. */
   nir_foreach_variable_in_shader(var, nir) {
      if (!(var->data.mode & (nir_var_shader_in | nir_var_shader_out)))
         ok = ok && add_var(&f, var);
   }
   nir_foreach_function_temp_variable(var, impl) {
      ok = ok && add_var(&f, var);
   }
   ok = ok && gather_slots(&f, impl) && add_slots(&f) && flatten_list(&f, &impl->body);
   _mesa_hash_table_destroy(f.types, NULL);
   _mesa_hash_table_destroy(f.vars, NULL);
   free(f.slots);
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
