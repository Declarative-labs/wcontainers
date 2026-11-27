/* The C side of wcontainers-nir: what it needs of the pinned Mesa, behind plain types. */
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

/* NIR of one entry point of a SPIR-V module for Vulkan, or NULL. */
struct nir_shader *wc_spirv_to_nir(const uint32_t *words, size_t count, enum wc_stage stage,
                                   const char *entry_point);

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

/* One GLSL program's stages as the OpenGL driver receives them, by enum wc_stage (NULL where the
 * program has none), each stage's names, and the compiler's and linker's messages; given back
 * with wc_glsl_program_free. */
struct wc_glsl_program {
   struct nir_shader *stages[WC_STAGES];
   struct wc_glsl_names names[WC_STAGES];
   char *log;
};

/* Compiles and links the sources (by enum wc_stage, NULL for none) with Mesa's GLSL compiler, then
 * takes st/mesa's steps for the OpenGL driver's capabilities; false when either step fails. */
bool wc_glsl_to_nir(const char *const sources[WC_STAGES], struct wc_glsl_program *out);
void wc_glsl_program_free(struct wc_glsl_program *program);

/* What every route does to the NIR to give it the common form's meaning and bindings: array layers of
 * sampling rounded to the nearest integer (Vulkan's rule; the common form's layer is an integer);
 * dispatches counted from workgroup 0 with no global offset;
 * each combined image sampler split into a texture and a sampler of the same shape; then every
 * resource renumbered into `set`, each kind from its range's first binding in the order of the
 * program's (set, binding). `out` receives one entry a resource. */
enum wc_bind_result wc_nir_to_common_form(struct nir_shader *nir, uint32_t set,
                                const struct wc_slot_range ranges[WC_SLOT_KINDS],
                                struct wc_bindings *out);

/* Stage values as lowered IO (load_input, load_interpolated_input, store_output), the one form
 * route B takes and the one the OpenGL linker hands over; a Vulkan program's blocks of them
 * (gl_PerVertex) split a member each first. Nothing where the IO is lowered already. */
void wc_nir_lower_io(struct nir_shader *nir);

/* Vulkan's alphaToOne in a fragment program (plan 15.4): Mesa's alpha to coverage, which demotes
 * the samples alpha does not cover, then alpha one on every colour output; IO lowered first. */
void wc_nir_alpha_to_one(struct nir_shader *nir);

/* NIR's own serialised form, the cache's key (plan 15.3). */
bool wc_nir_serialize(const struct nir_shader *nir, struct wc_bytes *out);

/* Candidate A's first step: a copy of the shader prepared and written as SPIR-V by Zink's compiler in its
 * native Vulkan mode (the shader itself is left as it was). False for a shader not in the form that mode takes. */
bool wc_nir_to_spirv(struct nir_shader *nir, struct wc_words *out);

struct nir_shader *wc_nir_clone(const struct nir_shader *nir);

/* Vertex inputs numbered as st/mesa hands the driver its vertex elements: each attribute, OpenGL's
 * own (gl_Vertex, gl_Color, ...) among them, at its rank among the attributes read. Returns the
 * slots read before. */
uint64_t wc_nir_number_vertex_elements(struct nir_shader *nir);
/* The most vertices a geometry program emits an invocation; zero for another stage. */
uint32_t wc_nir_geometry_vertices_out(const struct nir_shader *nir);
/* For a geometry program writing gl_Layer, the location after its generic outputs, where the
 * layer it names can be passed on; UINT32_MAX for one that writes none, or of another stage. */
uint32_t wc_nir_named_layer_location(const struct nir_shader *nir);
/* The control points of a tessellation control program's output patch; zero for another stage. */
uint32_t wc_nir_control_points_out(const struct nir_shader *nir);
/* An evaluation program in point mode, or a geometry program whose output is points. */
bool wc_nir_makes_points(const struct nir_shader *nir);

/* gl_FragColor written to each of the first `buffers` draw buffers, nir_lower_fragcolor's step
 * (the driver's own, as Zink's) on lowered stage values. */
void wc_nir_fragcolor_to_buffers(struct nir_shader *nir, unsigned buffers);

/* Arrays of textures and samplers read only at constant indices (OpenGL ES's constant index
 * expressions) as one variable an element, at the element's binding: the host may have no arrays
 * of bindings. */
void wc_nir_split_resource_arrays(struct nir_shader *nir);

/* Every 1D texture the program samples as a 2D texture of one row (nir_lower_tex's lower_1d, its
 * variables retyped): the driver makes 1D textures 2D, as the host's 1D textures have neither
 * levels nor layers. */
void wc_nir_textures_1d_as_2d(struct nir_shader *nir);

/* The program's name and label dropped (st/mesa names each by the API program's number), so that
 * programs of equal content serialise alike. */
void wc_nir_drop_name(struct nir_shader *nir);

/* Every uniform block access st/mesa hands over (load_ubo, the default uniforms as block 0) as
 * derefs of one array of words a block, sized by the block variables' explicit types; the default
 * uniforms' own variables, which nothing reads after it, removed. Storage block j is bound at
 * PIPE_MAX_SHADER_IMAGES plus j, past the image units. */
void wc_nir_uniform_blocks_as_words(struct nir_shader *nir);

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

void wc_nir_transform_position(struct nir_shader *nir, struct wc_position_transform transform);

/* Whether a fragment program reads where its samples are (gl_SamplePosition, interpolateAtSample,
 * interpolateAtOffset of a value read per sample), which depends on the targets' sample count. */
bool wc_nir_reads_sample_positions(struct nir_shader *nir);

/* A fragment program's reads of inputs at an offset or a sample and of the sample position made
 * from the values the common form interpolates, for targets of `samples` samples; false where
 * that count's positions are needed and not known (one, and the standard patterns of two, four,
 * eight and sixteen, are). */
bool wc_nir_interpolate_elsewhere(struct nir_shader *nir, unsigned samples);

/* A fragment program's sample masks as OpenGL defines them: gl_SampleMaskIn of a program run per
 * sample holds only that sample's bit (a host may give every covered sample's), and with targets
 * of one sample its writes of gl_SampleMask are dropped (OpenGL applies the mask only as a
 * multisample operation; a host applies its first bit). */
void wc_nir_fit_sample_masks(struct nir_shader *nir, bool single_sampled);

/* Whether a fragment program writes gl_SampleMask, which depends on the targets' sample count. */
bool wc_nir_writes_sample_mask(struct nir_shader *nir);

/* Whether a fragment program reads its first target's texels (framebuffer fetch, which Mesa's
 * lowering of KHR_blend_equation_advanced writes). */
bool wc_nir_fetches_target(struct nir_shader *nir);

/* The image units a program declares images at without a format (stores converted to the bound
 * image's format), a bit each. */
uint64_t wc_nir_formatless_images(struct nir_shader *nir);

/* A fragment program's reads of its first target's texels made loads of the texture at `unit`,
 * a copy of the target the driver binds there (multisampled where the target is, each sample
 * reading its own). */
void wc_nir_fetch_from_texture(struct nir_shader *nir, unsigned unit, bool multisampled);

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

/* Every result read from the textures at the units listed rearranged by their swizzle (Gallium's
 * sampler views carry one; the host's texture views do not). */
void wc_nir_swizzle_textures(struct nir_shader *nir, const struct wc_texture_swizzle *swizzles,
                             size_t count);

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

/* Every sampling of an integer texture at the units listed made a load of its nearest texel, as
 * OpenGL samples them (the host only loads and gathers them); a cube map or cube map array among
 * them becomes a 2D array of its faces, six a cube, which the driver binds such a view of. False
 * where the program also gathers such a cube: its gathers then read their own face's edge texels
 * past the edge, not the next face's, which is not built. */
bool wc_nir_nearest_integer_textures(struct nir_shader *nir,
                                     const struct wc_nearest_sampler *samplers, size_t count);

/* The texture units a program samples integers at (not loads, gathers or queries), a bit each. */
uint64_t wc_nir_integer_sampled_units(struct nir_shader *nir);

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

/* A streamed point size (which the common form keeps as a built-in the stage helper does not
 * stream) also written at a location no output takes, which the stream names instead. */
void wc_nir_stream_point_size_at_a_location(struct nir_shader *nir);

/* The generic slots a fragment program reads, or another stage's program writes, end here. */
uint32_t wc_nir_generic_end(const struct nir_shader *nir);
/* A vertex program's point size also written at the generic slot `at`; false where it writes
 * none. */
bool wc_nir_point_size_also_at(struct nir_shader *nir, uint32_t at);
/* Every point size a program with lowered outputs writes is a constant no larger than `size`;
 * false where it writes none. */
bool wc_nir_point_size_at_most(const struct nir_shader *nir, float size);
/* A geometry program drawing each point of `vs` as a square of two triangles (the slow path
 * PointsAsQuads), each corner with every output of `vs` and gl_PointCoord at the generic slot
 * `at`. Its size is the point size `vs` also writes at `at` (`wc_nir_point_size_also_at`),
 * clamped to 1..`largest`, or else uniform block 0's first word; the block's next words are the
 * clip-space step of half a pixel over w in x and y and gl_PointCoord's t direction (1 where t
 * grows with the rows, -1 where it falls). NULL for a program writing clip distances, or outputs
 * at offsets known only when run. */
struct nir_shader *wc_nir_point_square(const struct nir_shader *vs, uint32_t at, float largest);
/* A geometry program whose output is points, its uniform blocks as words
 * (`wc_nir_uniform_blocks_as_words`), rewritten to draw each as a square of two triangles
 * (PointsAsQuads), as `wc_nir_point_square` draws them: its size the point size it writes,
 * clamped to 1..`largest`, or else the first word of uniform block `block` (made in words too),
 * whose words are `wc_nir_point_square`'s; gl_PointCoord at the generic slot `at`. NULL for a program of another
 * output, writing clip distances, outputs at offsets known only when run or at `at`, or to more
 * than one stream. */
struct nir_shader *wc_nir_geometry_squares(const struct nir_shader *gs, uint32_t at, float largest,
                                           uint32_t block);
/* The primitives a pass-through geometry program takes, one program each: points, lines,
 * triangles, lines with adjacency and triangles with adjacency. */
#define WC_PASS_THROUGH_INPUTS 5

/* For a vertex or evaluation program `last` that reads or writes storage buffers or images, which
 * runs before a geometry program passing each of its primitives on: its point size also written at
 * the generic slot after its outputs (`wc_nir_point_size_also_at`), and in `made` those geometry
 * programs, by the primitive taken (`WC_PASS_THROUGH_INPUTS`'s order), each passing the
 * primitive's own vertices on unchanged with every output as written, its point size read from
 * that slot and not passed on. An evaluation program has one, for the primitive it makes. Returns
 * false, with `made` all NULL, for a program of another stage or using no storage; an entry is NULL
 * for a program writing outputs at offsets known only when run, or a point size the geometry
 * program cannot read. */
bool wc_nir_storage_pass_through(struct nir_shader *last,
                                 struct nir_shader *made[WC_PASS_THROUGH_INPUTS]);

/* The outputs `nir` streams (its transform feedback), at most `capacity` written to `outputs`, with
 * each of the first `buffers` buffers' bytes a vertex in `strides`; returns how many it streams,
 * or UINT32_MAX when one is a value the route places neither at a location nor as the position. */
uint32_t wc_nir_stream_outputs(const struct nir_shader *nir, struct wc_stream_output *outputs,
                               uint32_t capacity, uint32_t *strides, uint32_t buffers);

void wc_nir_free(struct nir_shader *nir);
void wc_bytes_free(struct wc_bytes *bytes);
void wc_words_free(struct wc_words *words);
void wc_bindings_free(struct wc_bindings *bindings);

#ifdef __cplusplus
}
#endif

#endif
