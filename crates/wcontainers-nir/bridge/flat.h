/* Candidate B's view of a shader: NIR out of SSA (every value used outside its block in a register),
 * flattened into plain arrays the Rust side reads (src/direct.rs). Mesa's numbering stays here: every
 * kind, operation and type is given in the enums below. */
#ifndef WCONTAINERS_NIR_FLAT_H
#define WCONTAINERS_NIR_FLAT_H

#include <stdbool.h>
#include <stdint.h>

struct nir_shader;

/* No value, variable or type. */
#define WC_NONE UINT32_MAX
#define WC_COMPONENTS 4
#define WC_SOURCES 5
/* The bit size of the values the route takes (booleans apart). */
#define WC_VALUE_BITS 32
/* The bit size of the integers of Mesa's alpha-to-coverage pass, carried in 32-bit words. */
#define WC_NARROW_BITS 16

enum wc_scalar { WC_SCALAR_FLOAT, WC_SCALAR_SINT, WC_SCALAR_UINT, WC_SCALAR_BOOL, WC_SCALAR_OTHER };

enum wc_type_kind {
   WC_TYPE_SCALAR,
   WC_TYPE_VECTOR,
   WC_TYPE_MATRIX,
   WC_TYPE_ARRAY,
   WC_TYPE_STRUCT,
   WC_TYPE_SAMPLER,
   WC_TYPE_TEXTURE,
   WC_TYPE_IMAGE,
   WC_TYPE_OTHER,
};

enum wc_dim { WC_DIM_1D, WC_DIM_2D, WC_DIM_3D, WC_DIM_CUBE, WC_DIM_BUFFER, WC_DIM_SUBPASS, WC_DIM_OTHER };

struct wc_flat_type {
   enum wc_type_kind kind;
   enum wc_scalar scalar;
   uint32_t bits;
   uint32_t components;
   uint32_t columns;
   /* An array's length (0: sized at run time) or a struct's field count. */
   uint32_t length;
   /* An array's element type, a matrix's column type, or a struct's first field (in `fields`). */
   uint32_t element;
   /* An array's or a matrix's explicit stride, 0 where the type has none. */
   uint32_t stride;
   uint32_t row_major;
   enum wc_dim dim;
   uint32_t arrayed;
   uint32_t shadow;
   uint32_t multisampled;
   enum wc_scalar sampled;
};

struct wc_flat_field {
   uint32_t type;
   int32_t offset;
};

enum wc_var_mode {
   WC_VAR_INPUT,
   WC_VAR_OUTPUT,
   WC_VAR_HANDLE,
   WC_VAR_UNIFORM_BUFFER,
   WC_VAR_STORAGE_BUFFER,
   WC_VAR_IMMEDIATE,
   WC_VAR_WORKGROUP,
   WC_VAR_PRIVATE,
   WC_VAR_FUNCTION,
   WC_VAR_OTHER,
};

/* Where a stage input or output meets its neighbour: a generic location or a built-in. */
enum wc_builtin {
   WC_BUILTIN_NONE,
   WC_BUILTIN_POSITION,
   WC_BUILTIN_POINT_SIZE,
   WC_BUILTIN_CLIP_DISTANCE,
   WC_BUILTIN_FRAG_DEPTH,
   WC_BUILTIN_SAMPLE_MASK,
   WC_BUILTIN_OTHER,
};

enum wc_interpolation { WC_INTERP_PERSPECTIVE, WC_INTERP_LINEAR, WC_INTERP_FLAT };

enum wc_storage_format {
   WC_FORMAT_NONE,
   WC_FORMAT_R32_FLOAT,
   WC_FORMAT_R32_UINT,
   WC_FORMAT_R32_SINT,
   WC_FORMAT_RG32_FLOAT,
   WC_FORMAT_RGBA32_FLOAT,
   WC_FORMAT_RGBA32_UINT,
   WC_FORMAT_RGBA32_SINT,
   WC_FORMAT_RGBA16_FLOAT,
   WC_FORMAT_RGBA8_UNORM,
   WC_FORMAT_RGBA8_SNORM,
   WC_FORMAT_R8_UNORM,
   WC_FORMAT_RG8_UNORM,
   WC_FORMAT_OTHER,
};

struct wc_flat_var {
   enum wc_var_mode mode;
   uint32_t type;
   enum wc_builtin builtin;
   uint32_t location;
   uint32_t set;
   uint32_t binding;
   enum wc_interpolation interpolation;
   uint32_t centroid;
   uint32_t sample;
   uint32_t readable;
   uint32_t writable;
   enum wc_storage_format format;
   /* A constant initialiser's values (32-bit, flattened), or none. */
   uint32_t initialised;
};

enum wc_flat_kind {
   WC_FLAT_ALU,
   WC_FLAT_CONST,
   WC_FLAT_UNDEF,
   WC_FLAT_DEREF,
   WC_FLAT_INTRINSIC,
   WC_FLAT_TEX,
   WC_FLAT_BREAK,
   WC_FLAT_CONTINUE,
   WC_FLAT_RETURN,
   WC_FLAT_IF,
   WC_FLAT_ELSE,
   WC_FLAT_END_IF,
   WC_FLAT_LOOP,
   WC_FLAT_END_LOOP,
   /* What the flattening does not take; `name` says what. */
   WC_FLAT_UNSUPPORTED,
};

/* The ALU operations candidate B takes, by NIR's names (each nir_op_<name>). */
#define WC_ALU_OPS(X) \
   X(mov) X(vec2) X(vec3) X(vec4) X(fadd) X(fsub) X(fmul) X(ffma) X(ffma_weak) X(flrp) X(fdiv) X(fneg) X(fabs) \
   X(fsign) X(fmin) X(fmax) X(ffloor) X(fceil) X(ffract) X(ftrunc) X(fround_even) X(fsqrt) \
   X(frsq) X(frcp) X(fexp2) X(flog2) X(fpow) X(fsin) X(fcos) X(fdot2) X(fdot3) X(fdot4) X(flt) \
   X(fge) X(feq) X(fneu) \
   X(iadd) X(isub) X(imul) X(ineg) X(iabs) X(imin) X(imax) X(umin) X(umax) X(idiv) X(udiv) \
   X(irem) X(umod) X(ishl) X(ishr) X(ushr) X(iand) X(ior) X(ixor) X(inot) X(ilt) X(ige) X(ieq) \
   X(ine) X(ult) X(uge) X(bcsel) X(i2f32) X(u2f32) X(f2i32) X(f2u32) X(b2f32) X(b2i32) \
   X(bit_count) X(ufind_msb) X(ifind_msb) X(find_lsb) X(bitfield_reverse) X(pack_half_2x16) \
   X(unpack_half_2x16) X(pack_unorm_4x8) X(unpack_unorm_4x8) X(pack_snorm_4x8) \
   X(unpack_snorm_4x8) X(pack_unorm_2x16) X(unpack_unorm_2x16) X(ball_fequal2) X(ball_fequal3) \
   X(ball_fequal4) X(bany_fnequal2) X(bany_fnequal3) X(bany_fnequal4) X(ball_iequal2) \
   X(ball_iequal3) X(ball_iequal4) X(bany_inequal2) X(bany_inequal3) X(bany_inequal4) X(fmod) \
   X(frem) X(u2u16) X(u2u32) X(b2b1) X(isign) X(ubitfield_extract) X(ibitfield_extract) \
   X(bitfield_insert) X(frexp_exp) X(frexp_sig)

#define WC_INTRINSICS(X)                                                                           \
   X(LOAD_DEREF) X(STORE_DEREF) X(DECL_REG) X(LOAD_REG) X(STORE_REG) X(VULKAN_RESOURCE_INDEX)    \
   X(LOAD_VULKAN_DESCRIPTOR) X(LOAD_VERTEX_ID) X(LOAD_INSTANCE_ID) X(LOAD_FRONT_FACE)             \
   X(LOAD_FRAG_COORD) X(LOAD_SAMPLE_ID) X(LOAD_LOCAL_INVOCATION_ID) X(LOAD_GLOBAL_INVOCATION_ID) \
   X(LOAD_WORKGROUP_ID) X(LOAD_NUM_WORKGROUPS) X(LOAD_LOCAL_INVOCATION_INDEX) X(DEMOTE)           \
   X(TERMINATE) X(DEMOTE_IF) X(TERMINATE_IF) X(BARRIER) X(DEREF_BUFFER_ARRAY_LENGTH) \
   X(DDX) X(DDY) X(DDX_FINE) X(DDY_FINE) X(DDX_COARSE) X(DDY_COARSE) X(LOAD_POINT_COORD)        \
   X(IMAGE_DEREF_LOAD) X(IMAGE_DEREF_STORE) X(IMAGE_DEREF_SIZE) X(DEREF_ATOMIC)                 \
   X(DEREF_ATOMIC_SWAP) X(LOAD_STAGE) X(STORE_STAGE) X(DEMOTE_SAMPLES) X(LOAD_SAMPLE_MASK_IN)

#define WC_ENUM_ENTRY(name) WC_ALU_##name,
enum wc_alu_op { WC_ALU_OPS(WC_ENUM_ENTRY) WC_ALU_UNSUPPORTED };
#undef WC_ENUM_ENTRY
#define WC_ENUM_ENTRY(name) WC_INTRINSIC_##name,
enum wc_intrinsic { WC_INTRINSICS(WC_ENUM_ENTRY) WC_INTRINSIC_UNSUPPORTED };
#undef WC_ENUM_ENTRY

/* An atomic operation of DEREF_ATOMIC, by meaning (Mesa's nir_atomic_op). */
enum wc_atomic_op {
   WC_ATOMIC_ADD,
   WC_ATOMIC_IMIN,
   WC_ATOMIC_UMIN,
   WC_ATOMIC_IMAX,
   WC_ATOMIC_UMAX,
   WC_ATOMIC_AND,
   WC_ATOMIC_OR,
   WC_ATOMIC_XOR,
   WC_ATOMIC_EXCHANGE,
   WC_ATOMIC_COMPARE_EXCHANGE,
   WC_ATOMIC_OTHER,
};

enum wc_deref_kind { WC_DEREF_VAR, WC_DEREF_ARRAY, WC_DEREF_STRUCT, WC_DEREF_CAST };

enum wc_tex_op {
   WC_TEX_SAMPLE,
   WC_TEX_BIAS,
   WC_TEX_LEVEL,
   WC_TEX_GRADIENT,
   WC_TEX_LOAD,
   WC_TEX_SIZE,
   WC_TEX_LEVELS,
   WC_TEX_GATHER,
};

/* A texture instruction's operands, by meaning. */
enum wc_tex_src {
   WC_TEX_SRC_COORD,
   WC_TEX_SRC_TEXTURE,
   WC_TEX_SRC_SAMPLER,
   WC_TEX_SRC_BIAS,
   WC_TEX_SRC_LOD,
   WC_TEX_SRC_COMPARATOR,
   WC_TEX_SRC_OFFSET,
   WC_TEX_SRC_DDX,
   WC_TEX_SRC_DDY,
   WC_TEX_SRC_SAMPLE,
   WC_TEX_SRCS,
};

struct wc_flat_src {
   uint32_t def;
   uint8_t swizzle[WC_COMPONENTS];
};

struct wc_flat_instr {
   enum wc_flat_kind kind;
   /* enum wc_alu_op, wc_intrinsic, wc_deref_kind or wc_tex_op, by kind. */
   uint32_t op;
   uint32_t def;
   uint32_t components;
   uint32_t bits;
   uint32_t sources;
   struct wc_flat_src src[WC_SOURCES];
   /* CONST: the values. DEREF: the variable, or the field. INTRINSIC: the set and binding of a
    * resource index, or a stage value's variable; the write mask of a store; a barrier's workgroup execution, workgroup memory and buffer
    * and image memory; an atomic's operation (enum wc_atomic_op) in the last. */
   uint32_t value[WC_COMPONENTS];
   /* DEREF: the type it points at. TEX: the result's scalar. */
   uint32_t type;
   uint32_t tex[WC_TEX_SRCS];
   enum wc_dim dim;
   uint32_t arrayed;
   uint32_t shadow;
   /* TEX: a gather's component. LOAD_STAGE, STORE_STAGE: the first component (or clip distance)
    * of the variable it reaches. */
   uint32_t component;
   /* UNSUPPORTED, or an operation outside the enums: Mesa's name of it. */
   const char *name;
};

enum wc_stage_kind { WC_FLAT_VERTEX, WC_FLAT_FRAGMENT, WC_FLAT_COMPUTE };

struct wc_flat {
   enum wc_stage_kind stage;
   uint32_t workgroup_size[3];
   uint32_t early_fragment_tests;
   uint32_t values;
   struct wc_flat_type *types;
   uint32_t type_count;
   struct wc_flat_field *fields;
   uint32_t field_count;
   struct wc_flat_var *vars;
   uint32_t var_count;
   uint32_t *initialisers;
   uint32_t initialiser_count;
   struct wc_flat_instr *instrs;
   uint32_t instr_count;
};

/* Candidate B's preparation (out of SSA, values used outside their block in registers) and the
 * shader flattened; the arrays are the bridge's until wc_flat_free. False when out of memory. */
bool wc_nir_flatten(struct nir_shader *nir, struct wc_flat *out);
void wc_flat_free(struct wc_flat *flat);

#endif
