/* Our C bridge over the pinned Mesa's compiler, as Rust calls it (crates/mesa-compiler's trait is generated from this
 * header and flat.h): plain types, and the typed chain at the end. The single steps are in steps.h. */
#ifndef WCONTAINERS_NIR_BRIDGE_H
#define WCONTAINERS_NIR_BRIDGE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#include "flat.h"

struct nir_shader;
struct nir_shader_compiler_options;

/* The program stages the routes take, apart from Mesa's own numbering. */
enum wc_stage {
   WC_STAGE_VERTEX,
   WC_STAGE_FRAGMENT,
   WC_STAGE_COMPUTE,
   WC_STAGES,
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

/* The NIR options every route takes; a Mesa driver reports them so that the NIR it is handed has
 * the routes' shape. */
const struct nir_shader_compiler_options *wc_compiler_options(void);

/* What a name of a GLSL program, or a state value, is in one stage's NIR. */
enum wc_glsl_name_kind {
   /* A default-block uniform: `at` its first word in uniform buffer 0, `count` its words. */
   WC_GLSL_UNIFORM,
   /* A subroutine uniform by its API name, as WC_GLSL_UNIFORM: each element's word holds the
    * index of the subroutine chosen for it. */
   WC_GLSL_SUBROUTINE_UNIFORM,
   /* The state values st/mesa fills (`at` the first of their four words), and any other state
    * value, named by Mesa's state string. */
   WC_GLSL_FRAG_COORD_Y_TRANSFORM,
   WC_GLSL_POINT_COORD_Y_TRANSFORM,
   WC_GLSL_OTHER_STATE,
   /* A block, a sampler or an image: `at` its program binding, `count` its bindings. */
   WC_GLSL_UNIFORM_BLOCK,
   WC_GLSL_STORAGE_BLOCK,
   WC_GLSL_SAMPLER,
   WC_GLSL_IMAGE,
   /* An atomic counter buffer, which the driver gives the program as a storage block: `at` its
    * program binding, `element` its OpenGL binding point. */
   WC_GLSL_ATOMIC_COUNTER_BUFFER,
   /* A stage input or output: `at` its location in the common form, `count` its locations. */
   WC_GLSL_INPUT,
   WC_GLSL_OUTPUT,
};

/* A stage value's interpolation qualifier. */
enum wc_glsl_interpolation {
   WC_GLSL_INTERPOLATION_UNQUALIFIED,
   WC_GLSL_INTERPOLATION_SMOOTH,
   WC_GLSL_INTERPOLATION_FLAT,
   WC_GLSL_INTERPOLATION_NOPERSPECTIVE,
   WC_GLSL_INTERPOLATION_OTHER,
};

struct wc_glsl_name {
   enum wc_glsl_name_kind kind;
   char *name;
   /* A stage value's GLSL type (NULL for other names) and interpolation. */
   char *type;
   enum wc_glsl_interpolation interpolation;
   uint32_t at;
   uint32_t count;
   /* The first of the value's slots the name places: a vertex attribute read in part (some
    * columns of a matrix) has a name a run of the slots read. */
   uint32_t element;
};

struct wc_glsl_names {
   struct wc_glsl_name *data;
   size_t count;
};

/* How a vertex program's position (and point size) is changed to meet the host's clip space,
 * viewport and points. */
struct wc_position_transform {
   /* y negated before the route's own negation: a Gallium viewport whose y scale is negative. */
   bool flip_y;
   /* Depth from OpenGL's -w..w to the host's 0..w. */
   bool half_z;
   /* Depth as w - z, for a depth range whose near value is the larger. */
   bool reverse_z;
   /* The point size output removed: the host draws points of one size, to which OpenGL clamps
    * every size a program writes. */
   bool fixed_point_size;
};

/* What one component of a texture's result is made of: one of its components, zero or one. */
enum wc_swizzle {
   WC_SWIZZLE_X,
   WC_SWIZZLE_Y,
   WC_SWIZZLE_Z,
   WC_SWIZZLE_W,
   WC_SWIZZLE_ZERO,
   WC_SWIZZLE_ONE,
};

/* The swizzle of the texture bound at one texture unit. */
struct wc_texture_swizzle {
   uint32_t unit;
   uint8_t swizzle[WC_COMPONENTS];
};

/* How a texture coordinate outside the texture wraps, for a nearest texel. */
enum wc_wrap {
   WC_WRAP_REPEAT,
   WC_WRAP_CLAMP_TO_EDGE,
   WC_WRAP_MIRRORED_REPEAT,
   /* Outside the texture, the border colour. */
   WC_WRAP_CLAMP_TO_BORDER,
};

/* The sampler at one texture unit whose texture holds integers: its wraps, whether it selects a
 * level by the level of detail, that level's bounds, and the border colour's integers. */
#define WC_TEXTURE_AXES 3

struct wc_nearest_sampler {
   uint32_t unit;
   uint8_t wrap[WC_TEXTURE_AXES];
   bool mipmapped;
   float min_lod;
   float max_lod;
   uint32_t border[WC_COMPONENTS];
};

/* One output a vertex program streams: the buffer, its byte offset in each vertex there, where the
 * route places the output (a generic location, or the position), and its components. */
struct wc_stream_output {
   uint32_t buffer;
   uint32_t offset;
   uint32_t location;
   bool position;
   uint8_t first_component;
   uint8_t components;
};

/* The primitives a pass-through geometry program takes, one program each: points, lines,
 * triangles, lines with adjacency and triangles with adjacency. */
#define WC_PASS_THROUGH_INPUTS 5

/* The buffers a program streams its outputs to: Gallium's PIPE_MAX_SO_BUFFERS. */
#define WC_STREAM_BUFFERS 4

/* ----- The typed chain. Each shader below is a nir_shader at one place in the chains our callers take, and a
 * function takes only the shader of its place: the orders below are the only ones the bridge offers. The callers
 * named are graphics-implementations' at 6a001c3ed: the corpus's two candidates on SPIR-V (crates/nir/tests/corpus/
 * run.rs) and the OpenGL driver (crates/opengl/src/state.rs at a shader's creation, draw.rs at a draw). A function
 * taking a shader named `taken` takes it whole: the bridge gives it back itself, whatever it returns. ----- */

/* A program read from SPIR-V for Vulkan: the corpus's first step (run.rs:493 to 496). */
struct wc_spirv_shader;
/* A program read from SPIR-V in the common form: what the corpus's candidates convert (run.rs:570 and 671). */
struct wc_spirv_common;
/* A stage as the OpenGL driver receives it from st/mesa (state.rs:1124 to 1138): here from Mesa's GLSL compiler
 * and st/mesa's steps (glsl.cpp). */
struct wc_received;
/* A stage after the driver's steps at its creation (state.rs:1139 to 1180), which nothing changes after. */
struct wc_prepared;
/* A program the driver's steps make: a pass-through geometry program (state.rs:1175), a vertex program with its
 * point size also at a location, or a geometry program drawing points as squares (draw.rs:3426 to 3441 and 3514 to
 * 3522). */
struct wc_made;
/* A draw's variant of a prepared or made program, in the common form (draw.rs:779 to 818). */
struct wc_variant;

/* Reads one entry point of a SPIR-V module for Vulkan, and, with `alpha_to_one`, makes Vulkan's alphaToOne in a
 * fragment program: the corpus reads each program, then makes alphaToOne in those of its folder for it (run.rs:493 to
 * 496). NULL for words that are no valid module for Vulkan, or that Mesa's reader refuses. */
struct wc_spirv_shader *wc_spirv_read(const uint32_t *words, size_t count, enum wc_stage stage, const char *entry_point,
                                      bool alpha_to_one);
/* The cache's key of a program read: NIR's serialised form, taken before the common form (run.rs:567). */
bool wc_spirv_shader_key(const struct wc_spirv_shader *shader, struct wc_bytes *out);

/* A program read from SPIR-V in the common form, with its bindings; the shader NULL when it is refused. */
struct wc_spirv_common_form {
   struct wc_spirv_common *shader;
   struct wc_bindings bindings;
   enum wc_bind_result result;
};
/* Gives the program the common form's meaning and bindings in `set` (each kind from its range's first binding), the
 * step both candidates take first (run.rs:570 and 671). */
void wc_spirv_to_common_form(struct wc_spirv_shader *taken, uint32_t set,
                             const struct wc_slot_range ranges[WC_SLOT_KINDS], struct wc_spirv_common_form *out);
void wc_spirv_common_form_free(struct wc_spirv_common_form *form);

/* Candidate A: the program written as SPIR-V by Zink's compiler in its native Vulkan mode (run.rs:672). False for a
 * program not in the form that mode takes. */
bool wc_spirv_common_to_spirv(struct wc_spirv_common *taken, struct wc_words *out);
/* Candidate B: the program out of SSA and flattened (run.rs:571, through crates/nir/src/direct.rs:974 to 984). */
bool wc_spirv_common_flatten(struct wc_spirv_common *taken, struct wc_flat *out);

/* One GLSL program's stages as the OpenGL driver receives them, by enum wc_stage (NULL where the
 * program has none), each stage's names, and the compiler's and linker's messages; given back
 * with wc_glsl_program_free. */
struct wc_glsl_program {
   struct wc_received *stages[WC_STAGES];
   struct wc_glsl_names names[WC_STAGES];
   char *log;
};

/* Compiles and links the sources (by enum wc_stage, NULL for none) with Mesa's GLSL compiler, then
 * takes st/mesa's steps for the OpenGL driver's capabilities; false when either step fails. */
bool wc_glsl_to_nir(const char *const sources[WC_STAGES], struct wc_glsl_program *out);
void wc_glsl_program_free(struct wc_glsl_program *program);

/* A stage after the driver's steps at its creation, and what the driver reads of it then and at its draws. */
struct wc_prepared_form {
   /* NULL when the cache's key could not be made (out of memory). */
   struct wc_prepared *shader;
   /* The cache's key: NIR's serialised form (a prepared stage's after every step, state.rs:1178 to 1180; a
    * variant's as the settings say, draw.rs:807 to 815). */
   struct wc_bytes key;
   /* The outputs it streams and each buffer's bytes a vertex (state.rs:1157 to 1162); none when it streams a value
    * the route places neither at a location nor as the position (`streams_elsewhere`). */
   struct wc_stream_output *streamed;
   uint32_t streamed_count;
   uint32_t strides[WC_STREAM_BUFFERS];
   bool streams_elsewhere;
   /* state.rs:1164 to 1169. */
   bool reads_sample_positions;
   bool writes_sample_mask;
   bool fetches_target;
   uint64_t formatless_images;
   uint64_t integer_sampled_units;
   /* UINT32_MAX for a program naming no layer. */
   uint32_t named_layer;
   /* A vertex or evaluation program using storage: the pass-through geometry programs by the primitive taken
    * (state.rs:1174 to 1176); `passes_through` false for any other. */
   bool passes_through;
   struct wc_made *pass_through[WC_PASS_THROUGH_INPUTS];
   /* What the driver reads at its draws (draw.rs:2346, 2393, 2414, 2576, 3422 and 3509). */
   uint32_t generic_end;
   uint32_t geometry_vertices_out;
   uint32_t control_points_out;
   bool makes_points;
};

/* The driver's steps at a stage's creation, in its order (state.rs:1139 to 1180): the name dropped, uniform blocks as
 * words, resource arrays split, 1D textures as 2D; a vertex program's inputs numbered as vertex elements, a fragment
 * program's gl_FragColor written to every draw buffer; the last stage before the rasterizer's point size streamed at
 * a location; the facts read; a vertex or evaluation program's pass-through programs made. */
void wc_received_prepare(struct wc_received *taken, struct wc_prepared_form *out);
void wc_prepared_form_free(struct wc_prepared_form *form);

/* What a draw makes a variant of a program for (draw.rs:779 to 818). */
struct wc_variant_settings {
   /* The position's transform (none when every field is false). */
   struct wc_position_transform transform;
   /* The transform the cache's key is taken with: where it is not `transform`, the key is of a copy with it applied
    * after `transform` (a geometry program's records, draw.rs:807 to 815). */
   struct wc_position_transform key_transform;
   /* A program reading its first target's texels reads them from the texture at this unit, multisampled where the
    * target is. */
   uint32_t fetch_unit;
   bool fetch_multisampled;
   /* Targets of one sample, and their sample count. */
   bool single_sampled;
   uint32_t samples;
   /* The common form's set and each kind's range. */
   uint32_t set;
   struct wc_slot_range ranges[WC_SLOT_KINDS];
};

/* Why a variant was not made. */
enum wc_variant_result {
   WC_VARIANT_MADE,
   /* A gather of an integer cube map the program also samples (draw.rs:788 to 793). */
   WC_VARIANT_GATHERS_INTEGER_CUBE,
   /* Where the samples of the targets' sample count are is not known (draw.rs:801 to 806). */
   WC_VARIANT_SAMPLES_UNKNOWN,
   /* The common form's refusals (wc_bind_result's). */
   WC_VARIANT_BEYOND_LIMIT,
   WC_VARIANT_UNBOUNDED,
   WC_VARIANT_OUT_OF_MEMORY,
};

/* A draw's variant, its key and its bindings; the shader NULL when `result` is not WC_VARIANT_MADE. */
struct wc_variant_form {
   struct wc_variant *shader;
   enum wc_variant_result result;
   struct wc_bytes key;
   struct wc_bindings bindings;
};

/* The driver's steps at a draw, in its order (draw.rs:779 to 818), on a copy of the program: the position transformed,
 * integer textures' samplings made nearest loads at the units of `samplers`, the textures at the units of `swizzles`
 * swizzled, the first target's texels read from `fetch_unit` (a program that reads them), the sample masks fitted,
 * reads at offsets and samples made from the interpolated values; the key taken; the common form. */
void wc_prepared_variant(const struct wc_prepared *shader, struct wc_variant_settings settings,
                         const struct wc_texture_swizzle *swizzles, size_t swizzle_count,
                         const struct wc_nearest_sampler *samplers, size_t sampler_count, struct wc_variant_form *out);
void wc_variant_form_free(struct wc_variant_form *form);

/* What drawing points as squares makes (draw.rs:3416 to 3550). */
enum wc_squares_kind {
   /* Every point is a pixel: the back end's own points (draw.rs:3428 to 3432 and 3516 to 3520). */
   WC_SQUARES_ONE_PIXEL,
   WC_SQUARES_MADE,
   /* None made: a program writing clip distances or outputs at offsets known only when run, or out of memory. */
   WC_SQUARES_REFUSED,
};

struct wc_squares {
   enum wc_squares_kind kind;
   /* Whether the program writes a point size (then also at `at`). */
   bool sized;
   /* The vertex program drawn before the squares where it writes a point size: a copy also writing it at `at`. */
   struct wc_made *before;
   struct wc_made *geometry;
};

/* A vertex program's points drawn as squares where the back end draws points of one size (draw.rs:3426 to 3441): a
 * copy writing its point size also at the generic slot `at`; nothing where every point is a pixel; the geometry
 * program drawing the squares (sized to at most `largest`), its uniform blocks as words. */
void wc_prepared_points_as_squares(const struct wc_prepared *vs, uint32_t at, float largest, struct wc_squares *out);
/* A geometry program's points drawn as squares (draw.rs:3514 to 3522): nothing where every point is a pixel; the
 * program rewritten to draw squares, of at most `largest`, its size else read from uniform block `block`. */
void wc_prepared_geometry_points_as_squares(const struct wc_prepared *gs, uint32_t at, float largest, uint32_t block,
                                            struct wc_squares *out);
void wc_squares_free(struct wc_squares *squares);

/* The cache's key of a made program (state.rs:1186, draw.rs:3445 and 3525). */
bool wc_made_key(const struct wc_made *shader, struct wc_bytes *out);
/* For a made geometry program writing gl_Layer, the location after its generic outputs (state.rs:1195); UINT32_MAX
 * for one naming no layer. */
uint32_t wc_made_named_layer_location(const struct wc_made *shader);
/* The driver's steps at a draw on a made program, as on a prepared one (draw.rs:779 to 818). */
void wc_made_variant(const struct wc_made *shader, struct wc_variant_settings settings,
                     const struct wc_texture_swizzle *swizzles, size_t swizzle_count,
                     const struct wc_nearest_sampler *samplers, size_t sampler_count, struct wc_variant_form *out);

/* A variant out of SSA and flattened: the route the driver converts it by (draw.rs:840, 978, 1034 and 1081, through
 * crates/nir/src/direct.rs:974 to 984), `points_in` a tessellation program's input control points. */
bool wc_variant_flatten(struct wc_variant *taken, uint32_t points_in, struct wc_flat *out);

/* Each shader given back. */
void wc_spirv_shader_free(struct wc_spirv_shader *shader);
void wc_spirv_common_free(struct wc_spirv_common *shader);
void wc_received_free(struct wc_received *shader);
void wc_prepared_free(struct wc_prepared *shader);
void wc_made_free(struct wc_made *shader);
void wc_variant_free(struct wc_variant *shader);
void wc_bytes_free(struct wc_bytes *bytes);
void wc_words_free(struct wc_words *words);

#ifdef __cplusplus
}
#endif

#endif
