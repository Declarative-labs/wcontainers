// Written by tools/api-crate-generator from the Mesa compiler bridge's bridge.h and flat.h (crates/nir/bridge). Do not edit.
//! The bridge's numbers: its defines and every enumeration's values.

/// `WC_COMPONENTS` of the bridge's headers.
pub const COMPONENTS: u32 = 4;
/// `WC_NARROW_BITS` of the bridge's headers.
pub const NARROW_BITS: u32 = 16;
/// `WC_PASS_THROUGH_INPUTS` of the bridge's headers.
pub const PASS_THROUGH_INPUTS: u32 = 5;
/// `WC_SOURCES` of the bridge's headers.
pub const SOURCES: u32 = 5;
/// `WC_TESS_INNER_LEVELS` of the bridge's headers.
pub const TESS_INNER_LEVELS: u32 = 2;
/// `WC_TESS_OUTER_LEVELS` of the bridge's headers.
pub const TESS_OUTER_LEVELS: u32 = 4;
/// `WC_TEXTURE_AXES` of the bridge's headers.
pub const TEXTURE_AXES: u32 = 3;
/// `WC_VALUE_BITS` of the bridge's headers.
pub const VALUE_BITS: u32 = 32;
/// `WC_ATOMIC_ADD` of `enum wc_atomic_op`.
pub const ATOMIC_ADD: u32 = 0;
/// `WC_ATOMIC_IMIN` of `enum wc_atomic_op`.
pub const ATOMIC_IMIN: u32 = 1;
/// `WC_ATOMIC_UMIN` of `enum wc_atomic_op`.
pub const ATOMIC_UMIN: u32 = 2;
/// `WC_ATOMIC_IMAX` of `enum wc_atomic_op`.
pub const ATOMIC_IMAX: u32 = 3;
/// `WC_ATOMIC_UMAX` of `enum wc_atomic_op`.
pub const ATOMIC_UMAX: u32 = 4;
/// `WC_ATOMIC_AND` of `enum wc_atomic_op`.
pub const ATOMIC_AND: u32 = 5;
/// `WC_ATOMIC_OR` of `enum wc_atomic_op`.
pub const ATOMIC_OR: u32 = 6;
/// `WC_ATOMIC_XOR` of `enum wc_atomic_op`.
pub const ATOMIC_XOR: u32 = 7;
/// `WC_ATOMIC_EXCHANGE` of `enum wc_atomic_op`.
pub const ATOMIC_EXCHANGE: u32 = 8;
/// `WC_ATOMIC_COMPARE_EXCHANGE` of `enum wc_atomic_op`.
pub const ATOMIC_COMPARE_EXCHANGE: u32 = 9;
/// `WC_ATOMIC_OTHER` of `enum wc_atomic_op`.
pub const ATOMIC_OTHER: u32 = 10;
/// `WC_BIND_OK` of `enum wc_bind_result`.
pub const BIND_OK: u32 = 0;
/// `WC_BIND_BEYOND_LIMIT` of `enum wc_bind_result`.
pub const BIND_BEYOND_LIMIT: u32 = 1;
/// `WC_BIND_UNBOUNDED` of `enum wc_bind_result`.
pub const BIND_UNBOUNDED: u32 = 2;
/// `WC_BIND_OUT_OF_MEMORY` of `enum wc_bind_result`.
pub const BIND_OUT_OF_MEMORY: u32 = 3;
/// `WC_BUILTIN_NONE` of `enum wc_builtin`.
pub const BUILTIN_NONE: u32 = 0;
/// `WC_BUILTIN_POSITION` of `enum wc_builtin`.
pub const BUILTIN_POSITION: u32 = 1;
/// `WC_BUILTIN_POINT_SIZE` of `enum wc_builtin`.
pub const BUILTIN_POINT_SIZE: u32 = 2;
/// `WC_BUILTIN_CLIP_DISTANCE` of `enum wc_builtin`.
pub const BUILTIN_CLIP_DISTANCE: u32 = 3;
/// `WC_BUILTIN_FRAG_DEPTH` of `enum wc_builtin`.
pub const BUILTIN_FRAG_DEPTH: u32 = 4;
/// `WC_BUILTIN_SAMPLE_MASK` of `enum wc_builtin`.
pub const BUILTIN_SAMPLE_MASK: u32 = 5;
/// `WC_BUILTIN_LAYER` of `enum wc_builtin`.
pub const BUILTIN_LAYER: u32 = 6;
/// `WC_BUILTIN_VIEWPORT_INDEX` of `enum wc_builtin`.
pub const BUILTIN_VIEWPORT_INDEX: u32 = 7;
/// `WC_BUILTIN_PRIMITIVE_ID` of `enum wc_builtin`.
pub const BUILTIN_PRIMITIVE_ID: u32 = 8;
/// `WC_BUILTIN_TESS_LEVEL_OUTER` of `enum wc_builtin`.
pub const BUILTIN_TESS_LEVEL_OUTER: u32 = 9;
/// `WC_BUILTIN_TESS_LEVEL_INNER` of `enum wc_builtin`.
pub const BUILTIN_TESS_LEVEL_INNER: u32 = 10;
/// `WC_BUILTIN_POINT_COORD` of `enum wc_builtin`.
pub const BUILTIN_POINT_COORD: u32 = 11;
/// `WC_BUILTIN_OTHER` of `enum wc_builtin`.
pub const BUILTIN_OTHER: u32 = 12;
/// `WC_DEREF_VAR` of `enum wc_deref_kind`.
pub const DEREF_VAR: u32 = 0;
/// `WC_DEREF_ARRAY` of `enum wc_deref_kind`.
pub const DEREF_ARRAY: u32 = 1;
/// `WC_DEREF_STRUCT` of `enum wc_deref_kind`.
pub const DEREF_STRUCT: u32 = 2;
/// `WC_DEREF_CAST` of `enum wc_deref_kind`.
pub const DEREF_CAST: u32 = 3;
/// `WC_DIM_1D` of `enum wc_dim`.
pub const DIM_1D: u32 = 0;
/// `WC_DIM_2D` of `enum wc_dim`.
pub const DIM_2D: u32 = 1;
/// `WC_DIM_3D` of `enum wc_dim`.
pub const DIM_3D: u32 = 2;
/// `WC_DIM_CUBE` of `enum wc_dim`.
pub const DIM_CUBE: u32 = 3;
/// `WC_DIM_BUFFER` of `enum wc_dim`.
pub const DIM_BUFFER: u32 = 4;
/// `WC_DIM_SUBPASS` of `enum wc_dim`.
pub const DIM_SUBPASS: u32 = 5;
/// `WC_DIM_OTHER` of `enum wc_dim`.
pub const DIM_OTHER: u32 = 6;
/// `WC_FLAT_ALU` of `enum wc_flat_kind`.
pub const FLAT_ALU: u32 = 0;
/// `WC_FLAT_CONST` of `enum wc_flat_kind`.
pub const FLAT_CONST: u32 = 1;
/// `WC_FLAT_UNDEF` of `enum wc_flat_kind`.
pub const FLAT_UNDEF: u32 = 2;
/// `WC_FLAT_DEREF` of `enum wc_flat_kind`.
pub const FLAT_DEREF: u32 = 3;
/// `WC_FLAT_INTRINSIC` of `enum wc_flat_kind`.
pub const FLAT_INTRINSIC: u32 = 4;
/// `WC_FLAT_TEX` of `enum wc_flat_kind`.
pub const FLAT_TEX: u32 = 5;
/// `WC_FLAT_BREAK` of `enum wc_flat_kind`.
pub const FLAT_BREAK: u32 = 6;
/// `WC_FLAT_CONTINUE` of `enum wc_flat_kind`.
pub const FLAT_CONTINUE: u32 = 7;
/// `WC_FLAT_RETURN` of `enum wc_flat_kind`.
pub const FLAT_RETURN: u32 = 8;
/// `WC_FLAT_IF` of `enum wc_flat_kind`.
pub const FLAT_IF: u32 = 9;
/// `WC_FLAT_ELSE` of `enum wc_flat_kind`.
pub const FLAT_ELSE: u32 = 10;
/// `WC_FLAT_END_IF` of `enum wc_flat_kind`.
pub const FLAT_END_IF: u32 = 11;
/// `WC_FLAT_LOOP` of `enum wc_flat_kind`.
pub const FLAT_LOOP: u32 = 12;
/// `WC_FLAT_END_LOOP` of `enum wc_flat_kind`.
pub const FLAT_END_LOOP: u32 = 13;
/// `WC_FLAT_UNSUPPORTED` of `enum wc_flat_kind`.
pub const FLAT_UNSUPPORTED: u32 = 14;
/// `WC_GEOMETRY_IN_POINTS` of `enum wc_geometry_input`.
pub const GEOMETRY_IN_POINTS: u32 = 0;
/// `WC_GEOMETRY_IN_LINES` of `enum wc_geometry_input`.
pub const GEOMETRY_IN_LINES: u32 = 1;
/// `WC_GEOMETRY_IN_LINES_ADJACENCY` of `enum wc_geometry_input`.
pub const GEOMETRY_IN_LINES_ADJACENCY: u32 = 2;
/// `WC_GEOMETRY_IN_TRIANGLES` of `enum wc_geometry_input`.
pub const GEOMETRY_IN_TRIANGLES: u32 = 3;
/// `WC_GEOMETRY_IN_TRIANGLES_ADJACENCY` of `enum wc_geometry_input`.
pub const GEOMETRY_IN_TRIANGLES_ADJACENCY: u32 = 4;
/// `WC_GEOMETRY_IN_OTHER` of `enum wc_geometry_input`.
pub const GEOMETRY_IN_OTHER: u32 = 5;
/// `WC_GEOMETRY_OUT_POINTS` of `enum wc_geometry_output`.
pub const GEOMETRY_OUT_POINTS: u32 = 0;
/// `WC_GEOMETRY_OUT_LINE_STRIP` of `enum wc_geometry_output`.
pub const GEOMETRY_OUT_LINE_STRIP: u32 = 1;
/// `WC_GEOMETRY_OUT_TRIANGLE_STRIP` of `enum wc_geometry_output`.
pub const GEOMETRY_OUT_TRIANGLE_STRIP: u32 = 2;
/// `WC_GEOMETRY_OUT_OTHER` of `enum wc_geometry_output`.
pub const GEOMETRY_OUT_OTHER: u32 = 3;
/// `WC_GLSL_INTERPOLATION_UNQUALIFIED` of `enum wc_glsl_interpolation`.
pub const GLSL_INTERPOLATION_UNQUALIFIED: u32 = 0;
/// `WC_GLSL_INTERPOLATION_SMOOTH` of `enum wc_glsl_interpolation`.
pub const GLSL_INTERPOLATION_SMOOTH: u32 = 1;
/// `WC_GLSL_INTERPOLATION_FLAT` of `enum wc_glsl_interpolation`.
pub const GLSL_INTERPOLATION_FLAT: u32 = 2;
/// `WC_GLSL_INTERPOLATION_NOPERSPECTIVE` of `enum wc_glsl_interpolation`.
pub const GLSL_INTERPOLATION_NOPERSPECTIVE: u32 = 3;
/// `WC_GLSL_INTERPOLATION_OTHER` of `enum wc_glsl_interpolation`.
pub const GLSL_INTERPOLATION_OTHER: u32 = 4;
/// `WC_GLSL_UNIFORM` of `enum wc_glsl_name_kind`.
pub const GLSL_UNIFORM: u32 = 0;
/// `WC_GLSL_SUBROUTINE_UNIFORM` of `enum wc_glsl_name_kind`.
pub const GLSL_SUBROUTINE_UNIFORM: u32 = 1;
/// `WC_GLSL_FRAG_COORD_Y_TRANSFORM` of `enum wc_glsl_name_kind`.
pub const GLSL_FRAG_COORD_Y_TRANSFORM: u32 = 2;
/// `WC_GLSL_POINT_COORD_Y_TRANSFORM` of `enum wc_glsl_name_kind`.
pub const GLSL_POINT_COORD_Y_TRANSFORM: u32 = 3;
/// `WC_GLSL_OTHER_STATE` of `enum wc_glsl_name_kind`.
pub const GLSL_OTHER_STATE: u32 = 4;
/// `WC_GLSL_UNIFORM_BLOCK` of `enum wc_glsl_name_kind`.
pub const GLSL_UNIFORM_BLOCK: u32 = 5;
/// `WC_GLSL_STORAGE_BLOCK` of `enum wc_glsl_name_kind`.
pub const GLSL_STORAGE_BLOCK: u32 = 6;
/// `WC_GLSL_SAMPLER` of `enum wc_glsl_name_kind`.
pub const GLSL_SAMPLER: u32 = 7;
/// `WC_GLSL_IMAGE` of `enum wc_glsl_name_kind`.
pub const GLSL_IMAGE: u32 = 8;
/// `WC_GLSL_ATOMIC_COUNTER_BUFFER` of `enum wc_glsl_name_kind`.
pub const GLSL_ATOMIC_COUNTER_BUFFER: u32 = 9;
/// `WC_GLSL_INPUT` of `enum wc_glsl_name_kind`.
pub const GLSL_INPUT: u32 = 10;
/// `WC_GLSL_OUTPUT` of `enum wc_glsl_name_kind`.
pub const GLSL_OUTPUT: u32 = 11;
/// `WC_INTERP_PERSPECTIVE` of `enum wc_interpolation`.
pub const INTERP_PERSPECTIVE: u32 = 0;
/// `WC_INTERP_LINEAR` of `enum wc_interpolation`.
pub const INTERP_LINEAR: u32 = 1;
/// `WC_INTERP_FLAT` of `enum wc_interpolation`.
pub const INTERP_FLAT: u32 = 2;
/// `WC_SCALAR_FLOAT` of `enum wc_scalar`.
pub const SCALAR_FLOAT: u32 = 0;
/// `WC_SCALAR_SINT` of `enum wc_scalar`.
pub const SCALAR_SINT: u32 = 1;
/// `WC_SCALAR_UINT` of `enum wc_scalar`.
pub const SCALAR_UINT: u32 = 2;
/// `WC_SCALAR_BOOL` of `enum wc_scalar`.
pub const SCALAR_BOOL: u32 = 3;
/// `WC_SCALAR_OTHER` of `enum wc_scalar`.
pub const SCALAR_OTHER: u32 = 4;
/// `WC_SLOT_UNIFORM` of `enum wc_slot_kind`.
pub const SLOT_UNIFORM: u32 = 0;
/// `WC_SLOT_SAMPLER` of `enum wc_slot_kind`.
pub const SLOT_SAMPLER: u32 = 1;
/// `WC_SLOT_TEXTURE` of `enum wc_slot_kind`.
pub const SLOT_TEXTURE: u32 = 2;
/// `WC_SLOT_STORAGE` of `enum wc_slot_kind`.
pub const SLOT_STORAGE: u32 = 3;
/// `WC_SLOT_KINDS` of `enum wc_slot_kind`.
pub const SLOT_KINDS: u32 = 4;
/// `WC_STAGE_VERTEX` of `enum wc_stage`.
pub const STAGE_VERTEX: u32 = 0;
/// `WC_STAGE_FRAGMENT` of `enum wc_stage`.
pub const STAGE_FRAGMENT: u32 = 1;
/// `WC_STAGE_COMPUTE` of `enum wc_stage`.
pub const STAGE_COMPUTE: u32 = 2;
/// `WC_STAGES` of `enum wc_stage`.
pub const STAGES: u32 = 3;
/// `WC_FLAT_VERTEX` of `enum wc_stage_kind`.
pub const FLAT_VERTEX: u32 = 0;
/// `WC_FLAT_FRAGMENT` of `enum wc_stage_kind`.
pub const FLAT_FRAGMENT: u32 = 1;
/// `WC_FLAT_COMPUTE` of `enum wc_stage_kind`.
pub const FLAT_COMPUTE: u32 = 2;
/// `WC_FLAT_GEOMETRY` of `enum wc_stage_kind`.
pub const FLAT_GEOMETRY: u32 = 3;
/// `WC_FLAT_TESS_CONTROL` of `enum wc_stage_kind`.
pub const FLAT_TESS_CONTROL: u32 = 4;
/// `WC_FLAT_TESS_EVALUATION` of `enum wc_stage_kind`.
pub const FLAT_TESS_EVALUATION: u32 = 5;
/// `WC_FORMAT_NONE` of `enum wc_storage_format`.
pub const FORMAT_NONE: u32 = 0;
/// `WC_FORMAT_R8_UNORM` of `enum wc_storage_format`.
pub const FORMAT_R8_UNORM: u32 = 1;
/// `WC_FORMAT_R8_SNORM` of `enum wc_storage_format`.
pub const FORMAT_R8_SNORM: u32 = 2;
/// `WC_FORMAT_R8_UINT` of `enum wc_storage_format`.
pub const FORMAT_R8_UINT: u32 = 3;
/// `WC_FORMAT_R8_SINT` of `enum wc_storage_format`.
pub const FORMAT_R8_SINT: u32 = 4;
/// `WC_FORMAT_R16_UINT` of `enum wc_storage_format`.
pub const FORMAT_R16_UINT: u32 = 5;
/// `WC_FORMAT_R16_SINT` of `enum wc_storage_format`.
pub const FORMAT_R16_SINT: u32 = 6;
/// `WC_FORMAT_R16_FLOAT` of `enum wc_storage_format`.
pub const FORMAT_R16_FLOAT: u32 = 7;
/// `WC_FORMAT_R16_UNORM` of `enum wc_storage_format`.
pub const FORMAT_R16_UNORM: u32 = 8;
/// `WC_FORMAT_R16_SNORM` of `enum wc_storage_format`.
pub const FORMAT_R16_SNORM: u32 = 9;
/// `WC_FORMAT_RG8_UNORM` of `enum wc_storage_format`.
pub const FORMAT_RG8_UNORM: u32 = 10;
/// `WC_FORMAT_RG8_SNORM` of `enum wc_storage_format`.
pub const FORMAT_RG8_SNORM: u32 = 11;
/// `WC_FORMAT_RG8_UINT` of `enum wc_storage_format`.
pub const FORMAT_RG8_UINT: u32 = 12;
/// `WC_FORMAT_RG8_SINT` of `enum wc_storage_format`.
pub const FORMAT_RG8_SINT: u32 = 13;
/// `WC_FORMAT_R32_UINT` of `enum wc_storage_format`.
pub const FORMAT_R32_UINT: u32 = 14;
/// `WC_FORMAT_R32_SINT` of `enum wc_storage_format`.
pub const FORMAT_R32_SINT: u32 = 15;
/// `WC_FORMAT_R32_FLOAT` of `enum wc_storage_format`.
pub const FORMAT_R32_FLOAT: u32 = 16;
/// `WC_FORMAT_RG16_UINT` of `enum wc_storage_format`.
pub const FORMAT_RG16_UINT: u32 = 17;
/// `WC_FORMAT_RG16_SINT` of `enum wc_storage_format`.
pub const FORMAT_RG16_SINT: u32 = 18;
/// `WC_FORMAT_RG16_FLOAT` of `enum wc_storage_format`.
pub const FORMAT_RG16_FLOAT: u32 = 19;
/// `WC_FORMAT_RG16_UNORM` of `enum wc_storage_format`.
pub const FORMAT_RG16_UNORM: u32 = 20;
/// `WC_FORMAT_RG16_SNORM` of `enum wc_storage_format`.
pub const FORMAT_RG16_SNORM: u32 = 21;
/// `WC_FORMAT_RGBA8_UNORM` of `enum wc_storage_format`.
pub const FORMAT_RGBA8_UNORM: u32 = 22;
/// `WC_FORMAT_RGBA8_SNORM` of `enum wc_storage_format`.
pub const FORMAT_RGBA8_SNORM: u32 = 23;
/// `WC_FORMAT_RGBA8_UINT` of `enum wc_storage_format`.
pub const FORMAT_RGBA8_UINT: u32 = 24;
/// `WC_FORMAT_RGBA8_SINT` of `enum wc_storage_format`.
pub const FORMAT_RGBA8_SINT: u32 = 25;
/// `WC_FORMAT_RGB10A2_UINT` of `enum wc_storage_format`.
pub const FORMAT_RGB10A2_UINT: u32 = 26;
/// `WC_FORMAT_RGB10A2_UNORM` of `enum wc_storage_format`.
pub const FORMAT_RGB10A2_UNORM: u32 = 27;
/// `WC_FORMAT_RG11B10_UFLOAT` of `enum wc_storage_format`.
pub const FORMAT_RG11B10_UFLOAT: u32 = 28;
/// `WC_FORMAT_RG32_UINT` of `enum wc_storage_format`.
pub const FORMAT_RG32_UINT: u32 = 29;
/// `WC_FORMAT_RG32_SINT` of `enum wc_storage_format`.
pub const FORMAT_RG32_SINT: u32 = 30;
/// `WC_FORMAT_RG32_FLOAT` of `enum wc_storage_format`.
pub const FORMAT_RG32_FLOAT: u32 = 31;
/// `WC_FORMAT_RGBA16_UINT` of `enum wc_storage_format`.
pub const FORMAT_RGBA16_UINT: u32 = 32;
/// `WC_FORMAT_RGBA16_SINT` of `enum wc_storage_format`.
pub const FORMAT_RGBA16_SINT: u32 = 33;
/// `WC_FORMAT_RGBA16_FLOAT` of `enum wc_storage_format`.
pub const FORMAT_RGBA16_FLOAT: u32 = 34;
/// `WC_FORMAT_RGBA16_UNORM` of `enum wc_storage_format`.
pub const FORMAT_RGBA16_UNORM: u32 = 35;
/// `WC_FORMAT_RGBA16_SNORM` of `enum wc_storage_format`.
pub const FORMAT_RGBA16_SNORM: u32 = 36;
/// `WC_FORMAT_RGBA32_UINT` of `enum wc_storage_format`.
pub const FORMAT_RGBA32_UINT: u32 = 37;
/// `WC_FORMAT_RGBA32_SINT` of `enum wc_storage_format`.
pub const FORMAT_RGBA32_SINT: u32 = 38;
/// `WC_FORMAT_RGBA32_FLOAT` of `enum wc_storage_format`.
pub const FORMAT_RGBA32_FLOAT: u32 = 39;
/// `WC_FORMAT_OTHER` of `enum wc_storage_format`.
pub const FORMAT_OTHER: u32 = 40;
/// `WC_SWIZZLE_X` of `enum wc_swizzle`.
pub const SWIZZLE_X: u32 = 0;
/// `WC_SWIZZLE_Y` of `enum wc_swizzle`.
pub const SWIZZLE_Y: u32 = 1;
/// `WC_SWIZZLE_Z` of `enum wc_swizzle`.
pub const SWIZZLE_Z: u32 = 2;
/// `WC_SWIZZLE_W` of `enum wc_swizzle`.
pub const SWIZZLE_W: u32 = 3;
/// `WC_SWIZZLE_ZERO` of `enum wc_swizzle`.
pub const SWIZZLE_ZERO: u32 = 4;
/// `WC_SWIZZLE_ONE` of `enum wc_swizzle`.
pub const SWIZZLE_ONE: u32 = 5;
/// `WC_TESS_TRIANGLES` of `enum wc_tess_domain`.
pub const TESS_TRIANGLES: u32 = 0;
/// `WC_TESS_QUADS` of `enum wc_tess_domain`.
pub const TESS_QUADS: u32 = 1;
/// `WC_TESS_ISOLINES` of `enum wc_tess_domain`.
pub const TESS_ISOLINES: u32 = 2;
/// `WC_TESS_DOMAIN_OTHER` of `enum wc_tess_domain`.
pub const TESS_DOMAIN_OTHER: u32 = 3;
/// `WC_TESS_EQUAL` of `enum wc_tess_spacing`.
pub const TESS_EQUAL: u32 = 0;
/// `WC_TESS_FRACTIONAL_ODD` of `enum wc_tess_spacing`.
pub const TESS_FRACTIONAL_ODD: u32 = 1;
/// `WC_TESS_FRACTIONAL_EVEN` of `enum wc_tess_spacing`.
pub const TESS_FRACTIONAL_EVEN: u32 = 2;
/// `WC_TESS_SPACING_OTHER` of `enum wc_tess_spacing`.
pub const TESS_SPACING_OTHER: u32 = 3;
/// `WC_TEX_SAMPLE` of `enum wc_tex_op`.
pub const TEX_SAMPLE: u32 = 0;
/// `WC_TEX_BIAS` of `enum wc_tex_op`.
pub const TEX_BIAS: u32 = 1;
/// `WC_TEX_LEVEL` of `enum wc_tex_op`.
pub const TEX_LEVEL: u32 = 2;
/// `WC_TEX_GRADIENT` of `enum wc_tex_op`.
pub const TEX_GRADIENT: u32 = 3;
/// `WC_TEX_LOAD` of `enum wc_tex_op`.
pub const TEX_LOAD: u32 = 4;
/// `WC_TEX_SIZE` of `enum wc_tex_op`.
pub const TEX_SIZE: u32 = 5;
/// `WC_TEX_LEVELS` of `enum wc_tex_op`.
pub const TEX_LEVELS: u32 = 6;
/// `WC_TEX_GATHER` of `enum wc_tex_op`.
pub const TEX_GATHER: u32 = 7;
/// `WC_TEX_SRC_COORD` of `enum wc_tex_src`.
pub const TEX_SRC_COORD: u32 = 0;
/// `WC_TEX_SRC_TEXTURE` of `enum wc_tex_src`.
pub const TEX_SRC_TEXTURE: u32 = 1;
/// `WC_TEX_SRC_SAMPLER` of `enum wc_tex_src`.
pub const TEX_SRC_SAMPLER: u32 = 2;
/// `WC_TEX_SRC_BIAS` of `enum wc_tex_src`.
pub const TEX_SRC_BIAS: u32 = 3;
/// `WC_TEX_SRC_LOD` of `enum wc_tex_src`.
pub const TEX_SRC_LOD: u32 = 4;
/// `WC_TEX_SRC_COMPARATOR` of `enum wc_tex_src`.
pub const TEX_SRC_COMPARATOR: u32 = 5;
/// `WC_TEX_SRC_OFFSET` of `enum wc_tex_src`.
pub const TEX_SRC_OFFSET: u32 = 6;
/// `WC_TEX_SRC_DDX` of `enum wc_tex_src`.
pub const TEX_SRC_DDX: u32 = 7;
/// `WC_TEX_SRC_DDY` of `enum wc_tex_src`.
pub const TEX_SRC_DDY: u32 = 8;
/// `WC_TEX_SRC_SAMPLE` of `enum wc_tex_src`.
pub const TEX_SRC_SAMPLE: u32 = 9;
/// `WC_TEX_SRCS` of `enum wc_tex_src`.
pub const TEX_SRCS: u32 = 10;
/// `WC_TYPE_SCALAR` of `enum wc_type_kind`.
pub const TYPE_SCALAR: u32 = 0;
/// `WC_TYPE_VECTOR` of `enum wc_type_kind`.
pub const TYPE_VECTOR: u32 = 1;
/// `WC_TYPE_MATRIX` of `enum wc_type_kind`.
pub const TYPE_MATRIX: u32 = 2;
/// `WC_TYPE_ARRAY` of `enum wc_type_kind`.
pub const TYPE_ARRAY: u32 = 3;
/// `WC_TYPE_STRUCT` of `enum wc_type_kind`.
pub const TYPE_STRUCT: u32 = 4;
/// `WC_TYPE_SAMPLER` of `enum wc_type_kind`.
pub const TYPE_SAMPLER: u32 = 5;
/// `WC_TYPE_TEXTURE` of `enum wc_type_kind`.
pub const TYPE_TEXTURE: u32 = 6;
/// `WC_TYPE_IMAGE` of `enum wc_type_kind`.
pub const TYPE_IMAGE: u32 = 7;
/// `WC_TYPE_OTHER` of `enum wc_type_kind`.
pub const TYPE_OTHER: u32 = 8;
/// `WC_VAR_INPUT` of `enum wc_var_mode`.
pub const VAR_INPUT: u32 = 0;
/// `WC_VAR_OUTPUT` of `enum wc_var_mode`.
pub const VAR_OUTPUT: u32 = 1;
/// `WC_VAR_HANDLE` of `enum wc_var_mode`.
pub const VAR_HANDLE: u32 = 2;
/// `WC_VAR_UNIFORM_BUFFER` of `enum wc_var_mode`.
pub const VAR_UNIFORM_BUFFER: u32 = 3;
/// `WC_VAR_STORAGE_BUFFER` of `enum wc_var_mode`.
pub const VAR_STORAGE_BUFFER: u32 = 4;
/// `WC_VAR_IMMEDIATE` of `enum wc_var_mode`.
pub const VAR_IMMEDIATE: u32 = 5;
/// `WC_VAR_WORKGROUP` of `enum wc_var_mode`.
pub const VAR_WORKGROUP: u32 = 6;
/// `WC_VAR_PRIVATE` of `enum wc_var_mode`.
pub const VAR_PRIVATE: u32 = 7;
/// `WC_VAR_FUNCTION` of `enum wc_var_mode`.
pub const VAR_FUNCTION: u32 = 8;
/// `WC_VAR_OTHER` of `enum wc_var_mode`.
pub const VAR_OTHER: u32 = 9;
/// `WC_WRAP_REPEAT` of `enum wc_wrap`.
pub const WRAP_REPEAT: u32 = 0;
/// `WC_WRAP_CLAMP_TO_EDGE` of `enum wc_wrap`.
pub const WRAP_CLAMP_TO_EDGE: u32 = 1;
/// `WC_WRAP_MIRRORED_REPEAT` of `enum wc_wrap`.
pub const WRAP_MIRRORED_REPEAT: u32 = 2;
/// `WC_WRAP_CLAMP_TO_BORDER` of `enum wc_wrap`.
pub const WRAP_CLAMP_TO_BORDER: u32 = 3;
