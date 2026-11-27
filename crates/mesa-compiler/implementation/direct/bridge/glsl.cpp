/* GLSL into the NIR the OpenGL driver receives: Mesa's GLSL compiler and linker, then the steps
 * st/mesa takes after a link (st_glsl_to_nir.cpp, MIT) for the capabilities that driver reports. */
#include "bridge.h"

#include <stdlib.h>
#include <string.h>

#include "nir.h"
#include "nir_builder.h"
#include "ir.h"
#include "glsl_parser_extras.h"
#include "builtin_functions.h"
#include "gl_nir.h"
#include "gl_nir_linker.h"
#include "linker_util.h"
#include "main/mtypes.h"
#include "pipe/p_screen.h"
#include "program/prog_parameter.h"
#include "program/prog_statevars.h"
#include "bridge_private.h"
#include "standalone_scaffolding.h"
#include "util/ralloc.h"
#include "util/u_math.h"

/* What the OpenGL driver reports (G9 reports the same): OpenGL 4.6 in the compatibility profile with
 * the ES compatibility extensions, so that one context takes desktop and ES sources alike. */
#define WC_GL_VERSION 46
#define WC_GLSL_VERSION 460
/* A source without #version is GLSL 1.10, as st/mesa sets for desktop OpenGL. */
#define WC_GLSL_DEFAULT_VERSION 110
#define WC_GL_DRAW_BUFFERS 8
#define WC_GL_CLIP_PLANES 8
#define WC_GL_TEXEL_OFFSET_MIN -8
#define WC_GL_TEXEL_OFFSET_MAX 7
#define WC_GL_VERTEX_ATTRIBUTES 16
#define WC_GL_TEXTURE_UNITS 16
#define WC_GL_UNIFORM_COMPONENTS 4096
#define WC_GL_STAGE_VALUE_COMPONENTS 128
#define WC_GL_BUFFER_BINDINGS 16
#define WC_GL_IMAGE_UNITS 8
#define WC_GL_UNIFORM_BLOCK_BYTES 65536
#define WC_GL_STORAGE_BLOCK_BYTES (128 << 20)
#define WC_GL_WORKGROUP_INVOCATIONS 256
#define WC_GL_WORKGROUP_DEPTH 64
#define WC_GL_WORKGROUPS 65535
#define WC_GL_WORKGROUP_BYTES 16384
/* Atomic counters live in storage buffers (no hardware counters): st/mesa's count for that case. */
#define WC_GL_ATOMIC_COUNTERS MAX_ATOMIC_COUNTERS
/* One vertex stream (no geometry programs); OpenGL's minimum of transform feedback buffers. */
#define WC_GL_VERTEX_STREAMS 1
#define WC_GL_TRANSFORM_FEEDBACK_BUFFERS 4
/* The common form's fragment coordinates: origin at the upper left, pixel centres at half. */
#define WC_GL_FRAG_COORD_UPPER_LEFT true
#define WC_GL_FRAG_COORD_HALF_INTEGER true

#define WC_GL_STATE_STRUCTURE "not taken: a built-in uniform structure of the compatibility profile\n"

static const gl_api WC_GL_API = API_OPENGL_COMPAT;

static void
report_capabilities(struct gl_context *ctx)
{
   initialize_context_to_defaults(ctx, WC_GL_API);
   _mesa_glsl_builtin_functions_init_or_ref();
   ctx->Version = WC_GL_VERSION;
   ctx->Const.GLSLVersion = WC_GLSL_VERSION;
   ctx->Const.GLSLVersionCompat = WC_GLSL_VERSION;
   ctx->Const.DefaultGLSLVersion = WC_GLSL_DEFAULT_VERSION;
   ctx->Extensions.ARB_ES3_1_compatibility = true;
   ctx->Extensions.ARB_ES3_2_compatibility = true;
   ctx->Extensions.ARB_shader_storage_buffer_object = true;
   ctx->Extensions.ARB_shader_image_load_store = true;
   ctx->Extensions.ARB_shader_atomic_counters = true;
   ctx->Extensions.ARB_shader_atomic_counter_ops = true;
   ctx->Extensions.ARB_texture_buffer_object = true;
   ctx->Extensions.ARB_derivative_control = true;
   ctx->Extensions.ARB_shader_texture_image_samples = true;
   ctx->Extensions.ARB_explicit_uniform_location = true;
   ctx->Const.NativeIntegers = true;
   ctx->Const.UniformBooleanTrue = ~0u;
   ctx->Const.PackedDriverUniformStorage = true;
   /* Blocks without a layout are laid out as std430, as st/mesa does for a driver loading its
    * constants from a buffer (uniform buffer 0). */
   ctx->Const.UseSTD430AsDefaultPacking = true;
   ctx->Const.GLSLFragCoordIsSysVal = true;
   ctx->Const.GLSLFrontFacingIsSysVal = true;
   ctx->Const.GLSLPointCoordIsSysVal = true;
   /* Tessellation levels as plain inputs, as compact arrays (the routes' options) require. */
   ctx->Const.GLSLTessLevelsAsInputs = true;
   ctx->Const.MaxDrawBuffers = WC_GL_DRAW_BUFFERS;
   ctx->Const.MaxClipPlanes = WC_GL_CLIP_PLANES;
   ctx->Const.MaxVertexStreams = WC_GL_VERTEX_STREAMS;
   ctx->Const.MaxTransformFeedbackBuffers = WC_GL_TRANSFORM_FEEDBACK_BUFFERS;
   ctx->Const.MinProgramTexelOffset = WC_GL_TEXEL_OFFSET_MIN;
   ctx->Const.MaxProgramTexelOffset = WC_GL_TEXEL_OFFSET_MAX;
   ctx->Const.MaxTextureCoordUnits = WC_GL_DRAW_BUFFERS;
   ctx->Const.MaxTextureUnits = WC_GL_DRAW_BUFFERS;
   ctx->Const.MaxUniformBufferBindings = WC_GL_BUFFER_BINDINGS;
   ctx->Const.MaxShaderStorageBufferBindings = WC_GL_BUFFER_BINDINGS;
   ctx->Const.MaxAtomicBufferBindings = WC_GL_BUFFER_BINDINGS;
   ctx->Const.MaxAtomicBufferSize = ATOMIC_COUNTER_SIZE * WC_GL_ATOMIC_COUNTERS;
   ctx->Const.MaxCombinedAtomicBuffers = WC_GL_BUFFER_BINDINGS;
   ctx->Const.MaxCombinedAtomicCounters = WC_GL_ATOMIC_COUNTERS;
   ctx->Const.MaxCombinedShaderStorageBlocks = WC_GL_BUFFER_BINDINGS;
   ctx->Const.MaxCombinedUniformBlocks = WC_GL_BUFFER_BINDINGS;
   ctx->Const.MaxCombinedShaderOutputResources =
      WC_GL_BUFFER_BINDINGS + WC_GL_IMAGE_UNITS + WC_GL_DRAW_BUFFERS;
   ctx->Const.MaxUserAssignableUniformLocations = WC_GL_UNIFORM_COMPONENTS;
   ctx->Const.MaxUniformBlockSize = WC_GL_UNIFORM_BLOCK_BYTES;
   ctx->Const.MaxShaderStorageBlockSize = WC_GL_STORAGE_BLOCK_BYTES;
   ctx->Const.MaxImageUnits = WC_GL_IMAGE_UNITS;
   ctx->Const.MaxCombinedImageUniforms = WC_GL_IMAGE_UNITS;
   ctx->Const.MaxCombinedTextureImageUnits = WC_GL_TEXTURE_UNITS;
   ctx->Const.MaxVarying = WC_GL_STAGE_VALUE_COMPONENTS / 4;
   ctx->Const.MaxComputeSharedMemorySize = WC_GL_WORKGROUP_BYTES;
   ctx->Const.MaxComputeWorkGroupInvocations = WC_GL_WORKGROUP_INVOCATIONS;
   for (unsigned axis = 0; axis < ARRAY_SIZE(ctx->Const.MaxComputeWorkGroupCount); axis++) {
      ctx->Const.MaxComputeWorkGroupCount[axis] = WC_GL_WORKGROUPS;
      ctx->Const.MaxComputeWorkGroupSize[axis] = WC_GL_WORKGROUP_INVOCATIONS;
   }
   ctx->Const.MaxComputeWorkGroupSize[ARRAY_SIZE(ctx->Const.MaxComputeWorkGroupSize) - 1] =
      WC_GL_WORKGROUP_DEPTH;
   const mesa_shader_stage stages[] = {MESA_SHADER_VERTEX, MESA_SHADER_FRAGMENT, MESA_SHADER_COMPUTE};
   for (unsigned s = 0; s < ARRAY_SIZE(stages); s++) {
      struct gl_program_constants *p = &ctx->Const.Program[stages[s]];
      p->MaxAttribs = WC_GL_VERTEX_ATTRIBUTES;
      p->MaxTextureImageUnits = WC_GL_TEXTURE_UNITS;
      p->MaxUniformComponents = WC_GL_UNIFORM_COMPONENTS;
      p->MaxCombinedUniformComponents = WC_GL_UNIFORM_COMPONENTS;
      p->MaxInputComponents = WC_GL_STAGE_VALUE_COMPONENTS;
      p->MaxOutputComponents = WC_GL_STAGE_VALUE_COMPONENTS;
      p->MaxUniformBlocks = WC_GL_BUFFER_BINDINGS;
      p->MaxShaderStorageBlocks = WC_GL_BUFFER_BINDINGS;
      p->MaxAtomicBuffers = WC_GL_BUFFER_BINDINGS;
      p->MaxAtomicCounters = WC_GL_ATOMIC_COUNTERS;
      p->MaxImageUniforms = WC_GL_IMAGE_UNITS;
      ctx->screen->nir_options[stages[s]] = wc_compiler_options();
   }
}

static GLenum
gl_stage(enum wc_stage stage)
{
   switch (stage) {
   case WC_STAGE_VERTEX:
      return GL_VERTEX_SHADER;
   case WC_STAGE_FRAGMENT:
      return GL_FRAGMENT_SHADER;
   case WC_STAGE_COMPUTE:
      return GL_COMPUTE_SHADER;
   case WC_STAGES:
      break;
   }
   return GL_NONE;
}

/* The parameter a uniform's storage starts at (st_nir_lookup_parameter_index). */
static int
parameter_index(struct gl_program *prog, nir_variable *var)
{
   struct gl_program_parameter_list *params = prog->Parameters;
   for (unsigned i = 0; i < params->NumParameters; i++) {
      if ((int)params->Parameters[i].MainUniformStorageIndex == var->data.location)
         return i;
   }
   size_t length = strlen(var->name);
   for (unsigned i = 0; i < params->NumParameters; i++) {
      const char *name = params->Parameters[i].Name;
      if (strncmp(name, var->name, length) == 0 && (name[length] == '.' || name[length] == '['))
         return i;
   }
   return -1;
}

/* Each uniform's place in the default block, in words (packed storage), and each sampler's and
 * image's unit (st_nir_assign_uniform_locations and st_update_state_param_locations). */
static void
assign_uniform_locations(struct gl_program *prog, nir_shader *nir)
{
   int samplers = 0, images = 0;
   nir_foreach_variable_with_modes(var, nir, nir_var_uniform | nir_var_image) {
      const struct glsl_type *bare = glsl_without_array(var->type);
      int at;
      if (!var->data.bindless && glsl_type_is_sampler(bare)) {
         at = samplers;
         samplers += glsl_count_attribute_slots(var->type, false);
      } else if (!var->data.bindless && glsl_type_is_image(bare)) {
         at = images;
         images += glsl_count_attribute_slots(var->type, false);
      } else if (var->state_slots) {
         at = _mesa_lookup_state_param_idx(prog->Parameters, var->state_slots[0].tokens);
         at = at >= 0 ? (int)prog->Parameters->Parameters[at].ValueOffset : at;
      } else {
         at = parameter_index(prog, var);
         at = at >= 0 ? (int)prog->Parameters->Parameters[at].ValueOffset : at;
      }
      var->data.driver_location = at;
   }
}

static unsigned
words_of(const struct glsl_type *type, bool bindless)
{
   return glsl_count_dword_slots(type, bindless);
}

static enum wc_glsl_interpolation
interpolation_of(enum glsl_interp_mode mode)
{
   switch (mode) {
   case INTERP_MODE_NONE:
      return WC_GLSL_INTERPOLATION_UNQUALIFIED;
   case INTERP_MODE_SMOOTH:
      return WC_GLSL_INTERPOLATION_SMOOTH;
   case INTERP_MODE_FLAT:
      return WC_GLSL_INTERPOLATION_FLAT;
   case INTERP_MODE_NOPERSPECTIVE:
      return WC_GLSL_INTERPOLATION_NOPERSPECTIVE;
   default:
      return WC_GLSL_INTERPOLATION_OTHER;
   }
}

static bool
add_name(struct wc_glsl_names *names, enum wc_glsl_name_kind kind, const char *name,
         const char *type, enum wc_glsl_interpolation interpolation, uint32_t at, uint32_t count)
{
   struct wc_glsl_name *grown =
      (struct wc_glsl_name *)realloc(names->data, (names->count + 1) * sizeof(*grown));
   if (!grown)
      return false;
   names->data = grown;
   struct wc_glsl_name *added = &grown[names->count++];
   added->kind = kind;
   added->name = strdup(name ? name : "");
   added->type = type ? strdup(type) : NULL;
   added->interpolation = interpolation;
   added->at = at;
   added->count = count;
   added->element = 0;
   return added->name && (!type || added->type);
}

static void
free_names(struct wc_glsl_names *names)
{
   for (size_t i = 0; i < names->count; i++) {
      free(names->data[i].name);
      free(names->data[i].type);
   }
   free(names->data);
   names->data = NULL;
   names->count = 0;
}

/* The program's own stage inputs and outputs, by their location in the common form; a vertex
 * program's attributes, OpenGL's own among them, by their slot until name_vertex_elements. */
static bool
name_stage_values(nir_shader *nir, struct wc_glsl_names *names)
{
   bool added = true;
   nir_foreach_variable_with_modes(var, nir, nir_var_shader_in | nir_var_shader_out) {
      bool output = var->data.mode == nir_var_shader_out;
      bool attribute = !output && nir->info.stage == MESA_SHADER_VERTEX;
      uint32_t location = var->data.location;
      if (!attribute && wc_builtin_at(nir, output, var->data.location, &location) != WC_BUILTIN_NONE)
         continue;
      added &= add_name(names, output ? WC_GLSL_OUTPUT : WC_GLSL_INPUT, var->name,
                        glsl_get_type_name(var->type),
                        interpolation_of((enum glsl_interp_mode)var->data.interpolation), location,
                        glsl_count_attribute_slots(var->type, attribute));
   }
   return added;
}

/* The length of Mesa's name prefix of a subroutine uniform (MESA_SUBROUTINE_PREFIX_LEN in
 * main/shaderobj.h, which the standalone scaffolding cannot include): "__subu_", the stage's
 * letter, "_". */
#define WC_SUBROUTINE_PREFIX_LEN 9

/* Whether a uniform parameter is a subroutine uniform (Mesa's name for it carries the stage's
 * subroutine prefix). */
static bool
subroutine_uniform(struct gl_program *prog, const struct gl_program_parameter *p)
{
   if (!prog->sh.data || p->UniformStorageIndex >= prog->sh.data->NumUniformStorage)
      return false;
   const struct gl_uniform_storage *u = &prog->sh.data->UniformStorage[p->UniformStorageIndex];
   return glsl_type_is_subroutine(glsl_without_array(u->type)) && p->Name &&
          strlen(p->Name) > WC_SUBROUTINE_PREFIX_LEN;
}

/* Each default-block uniform's words and each state value's, from the stage's parameters. */
/* The matrices a state value can name, in Mesa's order: each with its inverse, transpose and
 * inverse transpose after it. */
static const struct {
   gl_state_index16 first;
   const char *name;
} state_matrices[] = {
   {STATE_MODELVIEW_MATRIX, "modelview"},
   {STATE_PROJECTION_MATRIX, "projection"},
   {STATE_MVP_MATRIX, "mvp"},
   {STATE_TEXTURE_MATRIX, "texture"},
   {STATE_PROGRAM_MATRIX, "program"},
};
static const char *const state_matrix_forms[] = {"plain", "inverse", "transpose", "invtrans"};

/* A state value's name, which the standalone compiler's state strings leave empty: a matrix's as
 * "matrix.<matrix>.<form>.<index>.<first row>.<last row>" (Mesa's tokens: the rows of the matrix,
 * or of its transpose), any other as "state.<tokens>". Freed by the caller. */
static char *
state_name(const gl_state_index16 state[STATE_LENGTH])
{
   char *name = NULL;
   for (unsigned m = 0; m < ARRAY_SIZE(state_matrices); m++) {
      int form = state[0] - state_matrices[m].first;
      if (form >= 0 && form < (int)ARRAY_SIZE(state_matrix_forms))
         return asprintf(&name, "matrix.%s.%s.%d.%d.%d", state_matrices[m].name,
                         state_matrix_forms[form], state[1], state[2], state[3]) < 0 ? NULL : name;
   }
   return asprintf(&name, "state.%d.%d.%d.%d.%d", state[0], state[1], state[2], state[3],
                   state[4]) < 0 ? NULL : name;
}

static bool
name_parameters(struct gl_program *prog, struct wc_glsl_names *names)
{
   struct gl_program_parameter_list *params = prog->Parameters;
   bool added = true;
   for (unsigned i = 0; i < params->NumParameters; i++) {
      struct gl_program_parameter *p = &params->Parameters[i];
      enum wc_glsl_name_kind kind;
      const char *name = p->Name;
      if (p->Type == PROGRAM_UNIFORM && subroutine_uniform(prog, p)) {
         kind = WC_GLSL_SUBROUTINE_UNIFORM;
         name += WC_SUBROUTINE_PREFIX_LEN;
      } else if (p->Type == PROGRAM_UNIFORM)
         kind = WC_GLSL_UNIFORM;
      else if (p->Type != PROGRAM_STATE_VAR)
         continue;
      else if (p->StateIndexes[0] == STATE_FB_WPOS_Y_TRANSFORM)
         kind = WC_GLSL_FRAG_COORD_Y_TRANSFORM;
      else if (p->StateIndexes[0] == STATE_FB_PNTC_Y_TRANSFORM)
         kind = WC_GLSL_POINT_COORD_Y_TRANSFORM;
      else
         kind = WC_GLSL_OTHER_STATE;
      char *state = kind == WC_GLSL_OTHER_STATE ? state_name(p->StateIndexes) : NULL;
      if (kind == WC_GLSL_OTHER_STATE && !state)
         return false;
      if (state)
         name = state;
      /* A matrix or an array has a parameter a column or element, one after another: one name. */
      struct wc_glsl_name *last = names->count ? &names->data[names->count - 1] : NULL;
      bool uniform = kind == WC_GLSL_UNIFORM || kind == WC_GLSL_SUBROUTINE_UNIFORM;
      if (uniform && last && last->kind == kind && name && strcmp(last->name, name) == 0 &&
          last->at + last->count == p->ValueOffset) {
         last->count += p->Size;
         continue;
      }
      added &= add_name(names, kind, name, NULL, WC_GLSL_INTERPOLATION_UNQUALIFIED,
                        p->ValueOffset, p->Size);
      free(state);
   }
   return added;
}

/* Each block's, sampler's, image's and atomic counter buffer's program binding; the atomic counter
 * buffers' storage blocks start at `atomic_base`. */
static bool
name_bindings(struct gl_shader_program *whole, struct gl_program *prog, nir_shader *nir,
              uint32_t atomic_base, struct wc_glsl_names *names)
{
   bool added = true;
   uint32_t uniform_blocks = prog->info.num_ubos + nir->info.first_ubo_is_default_ubo;
   for (unsigned i = 0; i < prog->info.num_ubos; i++)
      added &= add_name(names, WC_GLSL_UNIFORM_BLOCK, prog->sh.UniformBlocks[i]->name.string,
                        NULL, WC_GLSL_INTERPOLATION_UNQUALIFIED, i + nir->info.first_ubo_is_default_ubo, 1);
   for (unsigned i = 0; i < prog->info.num_ssbos; i++)
      added &= add_name(names, WC_GLSL_STORAGE_BLOCK, prog->sh.ShaderStorageBlocks[i]->name.string,
                        NULL, WC_GLSL_INTERPOLATION_UNQUALIFIED, uniform_blocks + i, 1);
   for (unsigned i = 0; i < whole->data->NumAtomicBuffers; i++) {
      const struct gl_active_atomic_buffer *buffer = &whole->data->AtomicBuffers[i];
      if (!buffer->StageReferences[nir->info.stage])
         continue;
      added &= add_name(names, WC_GLSL_ATOMIC_COUNTER_BUFFER, "", NULL,
                        WC_GLSL_INTERPOLATION_UNQUALIFIED,
                        uniform_blocks + atomic_base + buffer->Binding, 1);
      if (added)
         names->data[names->count - 1].element = buffer->Binding;
   }
   nir_foreach_variable_with_modes(var, nir, nir_var_uniform | nir_var_image) {
      const struct glsl_type *bare = glsl_without_array(var->type);
      if (!glsl_type_is_sampler(bare) && !glsl_type_is_image(bare))
         continue;
      added &= add_name(names, glsl_type_is_sampler(bare) ? WC_GLSL_SAMPLER : WC_GLSL_IMAGE,
                        var->name, NULL, WC_GLSL_INTERPOLATION_UNQUALIFIED, var->data.binding,
                        glsl_type_is_array(var->type) ? glsl_get_aoa_size(var->type) : 1);
   }
   return added;
}

/* Each attribute's name at its vertex elements, from the slots read before they were numbered: a
 * name a run of slots read (an attribute read in part has several); one not read has none. */
static bool
name_vertex_elements(struct wc_glsl_names *names, uint64_t read)
{
   struct wc_glsl_names placed = {NULL, 0};
   bool added = true;
   for (size_t i = 0; i < names->count; i++) {
      struct wc_glsl_name name = names->data[i];
      if (name.kind != WC_GLSL_INPUT) {
         added &= add_name(&placed, name.kind, name.name, name.type, name.interpolation, name.at,
                           name.count);
         continue;
      }
      for (uint32_t s = 0; s < name.count;) {
         uint32_t slot = name.at + s;
         if (!(read & BITFIELD64_BIT(slot))) {
            s++;
            continue;
         }
         uint32_t run = 0;
         while (s + run < name.count && (read & BITFIELD64_BIT(slot + run)))
            run++;
         added &= add_name(&placed, name.kind, name.name, name.type, name.interpolation,
                           util_bitcount64(read & BITFIELD64_MASK(slot)), run);
         if (added)
            placed.data[placed.count - 1].element = s;
         s += run;
      }
   }
   free_names(names);
   *names = placed;
   return added;
}

/* st/mesa's steps from the linked NIR to what the driver receives, for this driver's capabilities:
 * fragment coordinates as the common form has them, no hardware atomic counters, rectangle
 * textures and gathers with offsets lowered, uniforms packed in words into buffer 0, samplers and
 * images as derefs; then gl_FragColor written to every draw buffer, as the driver does. False for
 * a built-in uniform structure (the compatibility profile's lighting state), not taken yet. */
static bool
after_link(struct gl_shader_program *whole, struct gl_linked_shader *linked, uint64_t *attributes,
           uint32_t *atomic_base)
{
   struct gl_program *prog = linked->Program;
   nir_shader *nir = prog->nir;
   NIR_PASS(_, nir, nir_remove_dead_variables, nir_var_shader_in | nir_var_shader_out, NULL);
   NIR_PASS(_, nir, gl_nir_lower_buffers, whole);
   if (nir->info.stage == MESA_SHADER_FRAGMENT) {
      nir_lower_wpos_ytransform_options wpos = {};
      wpos.state_tokens[0] = STATE_FB_WPOS_Y_TRANSFORM;
      wpos.fs_coord_origin_upper_left = WC_GL_FRAG_COORD_UPPER_LEFT;
      wpos.fs_coord_pixel_center_half_integer = WC_GL_FRAG_COORD_HALF_INTEGER;
      if (nir_lower_wpos_ytransform(nir, &wpos))
         _mesa_add_state_reference(prog->Parameters, wpos.state_tokens);
      const gl_state_index16 point[STATE_LENGTH] = {STATE_FB_PNTC_Y_TRANSFORM};
      if (nir_lower_pntc_ytransform(nir, &point))
         _mesa_add_state_reference(prog->Parameters, point);
   }
   const nir_lower_compute_system_values_options from_zero = {};
   NIR_PASS(_, nir, nir_lower_system_values);
   NIR_PASS(_, nir, nir_lower_compute_system_values, &from_zero);

   nir_foreach_uniform_variable(var, nir) {
      const struct glsl_type *bare = glsl_without_array(var->type);
      if (var->state_slots && glsl_type_is_struct_or_ifc(bare))
         return false;
      for (unsigned s = 0; var->state_slots && s < var->num_state_slots; s++)
         _mesa_add_sized_state_reference(prog->Parameters, var->state_slots[s].tokens,
                                         glsl_get_vector_elements(bare), false);
   }
   NIR_PASS(_, nir, gl_nir_lower_atomics, whole, true);
   NIR_PASS(_, nir, nir_lower_int64);
   NIR_PASS(_, nir, nir_remove_dead_variables, nir_var_function_temp, NULL);
   *atomic_base = nir->info.num_ssbos;
   NIR_PASS(_, nir, nir_lower_atomics_to_ssbo, 0);
   NIR_PASS(_, nir, nir_opt_intrinsics);
   nir_shader_gather_info(nir, nir_shader_get_entrypoint(nir));
   if (nir->info.stage == MESA_SHADER_VERTEX)
      *attributes = wc_nir_number_vertex_elements(nir);

   nir_lower_tex_options tex = {};
   tex.lower_rect = true;
   tex.lower_txp = ~0u;
   tex.lower_txp_array = true;
   tex.lower_tg4_offsets = true;
   NIR_PASS(_, nir, nir_lower_tex, &tex);
   assign_uniform_locations(prog, nir);
   nir->num_uniforms = DIV_ROUND_UP(prog->Parameters->NumParameterValues, 4);
   NIR_PASS(_, nir, nir_lower_io, nir_var_uniform, words_of, (nir_lower_io_options)0);
   NIR_PASS(_, nir, nir_lower_uniforms_to_ubo, true, false);
   NIR_PASS(_, nir, nir_opt_dce);
   NIR_PASS(_, nir, nir_remove_dead_variables, nir_var_uniform, NULL);
   NIR_PASS(_, nir, gl_nir_lower_samplers_as_deref, whole);
   if (nir->info.stage == MESA_SHADER_FRAGMENT)
      wc_nir_fragcolor_to_buffers(nir, WC_GL_DRAW_BUFFERS);
   return true;
}

/* The byte size of each uniform buffer the stage reads, by its index: 0 the default block. */
static uint32_t *
uniform_sizes(struct gl_program *prog, nir_shader *nir, uint32_t *count)
{
   *count = prog->info.num_ubos + nir->info.first_ubo_is_default_ubo;
   uint32_t *sizes = (uint32_t *)calloc(*count ? *count : 1, sizeof(*sizes));
   if (!sizes)
      return NULL;
   for (uint32_t i = 0; i < *count; i++) {
      if (i == 0 && nir->info.first_ubo_is_default_ubo)
         sizes[i] = prog->Parameters->NumParameterValues * sizeof(gl_constant_value);
      else
         sizes[i] = prog->sh.UniformBlocks[i - nir->info.first_ubo_is_default_ubo]->UniformBufferSize;
   }
   return sizes;
}

static char *
append_log(char *log, const char *more)
{
   if (!more || !*more)
      return log;
   size_t had = log ? strlen(log) : 0, add = strlen(more);
   char *grown = (char *)realloc(log, had + add + 1);
   if (!grown)
      return log;
   memcpy(grown + had, more, add + 1);
   return grown;
}

extern "C" bool
wc_glsl_to_nir(const char *const sources[WC_STAGES], struct wc_glsl_program *out)
{
   memset(out, 0, sizeof(*out));
   struct gl_context *ctx = (struct gl_context *)calloc(1, sizeof(*ctx));
   if (!ctx)
      return false;
   report_capabilities(ctx);
   struct gl_shader_program *whole = standalone_create_shader_program();
   bool compiled = true;
   for (int s = 0; s < WC_STAGES; s++) {
      if (!sources[s])
         continue;
      struct gl_shader *shader =
         standalone_add_shader_source(ctx, whole, gl_stage((enum wc_stage)s), sources[s]);
      _mesa_glsl_compile_shader(ctx, shader, NULL, false, false, true);
      out->log = append_log(out->log, shader->InfoLog);
      compiled &= shader->CompileStatus == COMPILE_SUCCESS;
   }
   bool linked = false;
   if (compiled) {
      _mesa_clear_shader_program_data(ctx, whole);
      whole->data->LinkStatus = LINKING_SUCCESS;
      link_shaders_init(ctx, whole);
      linked = gl_nir_link_glsl(ctx, whole) && whole->data->LinkStatus;
      out->log = append_log(out->log, whole->data->InfoLog);
   }
   for (int s = 0; linked && s < WC_STAGES; s++) {
      struct gl_linked_shader *shader = whole->_LinkedShaders[wc_mesa_stage((enum wc_stage)s)];
      if (!sources[s] || !shader)
         continue;
      struct wc_glsl_names *names = &out->names[s];
      if (!name_stage_values(shader->Program->nir, names)) {
         linked = false;
         break;
      }
      uint64_t attributes = 0;
      uint32_t atomic_base = 0;
      if (!after_link(whole, shader, &attributes, &atomic_base)) {
         out->log = append_log(out->log, WC_GL_STATE_STRUCTURE);
         linked = false;
         break;
      }
      if (shader->Stage == MESA_SHADER_VERTEX && !name_vertex_elements(names, attributes)) {
         linked = false;
         break;
      }
      nir_shader *nir = shader->Program->nir;
      if (!name_parameters(shader->Program, names) ||
          !name_bindings(whole, shader->Program, nir, atomic_base, names)) {
         linked = false;
         break;
      }
      uint32_t count;
      uint32_t *sizes = uniform_sizes(shader->Program, nir, &count);
      if (!sizes) {
         linked = false;
         break;
      }
      wc_nir_buffers_as_words(nir, sizes, count, count);
      free(sizes);
      out->stages[s] = nir_shader_clone(NULL, nir);
      wc_optimize(out->stages[s]);
   }
   standalone_destroy_shader_program(whole);
   free(ctx->screen);
   free(ctx);
   _mesa_glsl_builtin_functions_decref();
   if (!linked) {
      char *log = out->log;
      out->log = NULL;
      wc_glsl_program_free(out);
      out->log = log;
   }
   return linked;
}

extern "C" void
wc_glsl_program_free(struct wc_glsl_program *program)
{
   for (int s = 0; s < WC_STAGES; s++) {
      ralloc_free(program->stages[s]);
      free_names(&program->names[s]);
   }
   free(program->log);
   memset(program, 0, sizeof(*program));
}
