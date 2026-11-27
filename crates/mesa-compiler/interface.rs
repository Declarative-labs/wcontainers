// Written by tools/api-crate-generator from the Mesa compiler bridge's bridge.h and flat.h (crates/nir/bridge). Do not edit.
//! Mesa's shader compiler as our C bridge offers it (crates/nir/bridge: NIR made from SPIR-V or GLSL, changed for the
//! routes, read as a flat form, written as SPIR-V), as a safe trait. Every enumeration the bridge writes is a `u32` the
//! caller converts by a checked `TryFrom`.

/// The bridge's `struct wc_flat_type`, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatType {
    /// The bridge's `kind`.
    pub kind: u32,
    /// The bridge's `scalar`.
    pub scalar: u32,
    /// The bridge's `bits`.
    pub bits: u32,
    /// The bridge's `components`.
    pub components: u32,
    /// The bridge's `columns`.
    pub columns: u32,
    /// An array's length (0: sized at run time) or a struct's field count.
    pub length: u32,
    /// The first of the value's slots the name places: a vertex attribute read in part (some columns of a matrix) has a name a run of the slots read.
    pub element: u32,
    /// For a storage buffer, the stride of its runtime-sized array, else its block's size.
    pub stride: u32,
    /// The bridge's `row_major`.
    pub row_major: u32,
    /// The bridge's `dim`.
    pub dim: u32,
    /// The bridge's `arrayed`.
    pub arrayed: u32,
    /// The bridge's `shadow`.
    pub shadow: u32,
    /// The bridge's `multisampled`.
    pub multisampled: u32,
    /// The bridge's `sampled`.
    pub sampled: u32,
}

/// The bridge's `struct wc_flat_field`, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatField {
    /// DEREF: the type it points at. TEX: the result's scalar.
    pub type_: u32,
    /// The bridge's `offset`.
    pub offset: i32,
}

/// The bridge's `struct wc_flat_var`, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatVar {
    /// The bridge's `mode`.
    pub mode: u32,
    /// DEREF: the type it points at. TEX: the result's scalar.
    pub type_: u32,
    /// The bridge's `builtin`.
    pub builtin: u32,
    /// The bridge's `location`.
    pub location: u32,
    /// The bridge's `set`.
    pub set: u32,
    /// The bridge's `binding`.
    pub binding: u32,
    /// The bridge's `interpolation`.
    pub interpolation: u32,
    /// The bridge's `centroid`.
    pub centroid: u32,
    /// The bridge's `sample`.
    pub sample: u32,
    /// The bridge's `readable`.
    pub readable: u32,
    /// The bridge's `writable`.
    pub writable: u32,
    /// The bridge's `format`.
    pub format: u32,
    /// A constant initialiser's values (32-bit, flattened), or none.
    pub initialised: u32,
    /// A geometry program's output stream.
    pub stream: u32,
    /// A fragment output's blend source: 1 for dual-source blending's second.
    pub blend_source: u32,
    /// One value for the patch: a control program's patch output, an evaluation program's patch input.
    pub patch: u32,
    /// An output made exactly (GLSL's `invariant` or `precise`): every value stored to it comes from instructions the compiler may not reorder.
    pub invariant: u32,
}

/// The bridge's `struct wc_flat_src`, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatSrc {
    /// The bridge's `def`.
    pub def: u32,
    /// The bridge's `swizzle`.
    pub swizzle: [u8; 4],
}

/// The bridge's `struct wc_flat_instr`, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatInstr {
    /// The bridge's `kind`.
    pub kind: u32,
    /// enum wc_alu_op, wc_intrinsic, wc_deref_kind or wc_tex_op, by kind.
    pub op: u32,
    /// The bridge's `def`.
    pub def: u32,
    /// The bridge's `components`.
    pub components: u32,
    /// The bridge's `bits`.
    pub bits: u32,
    /// The bridge's `sources`.
    pub sources: u32,
    /// The bridge's `src`.
    pub src: [FlatSrc; 5],
    /// CONST: the values. DEREF: the variable, or the field. INTRINSIC: the set and binding of a resource index, or a stage value's variable; the write mask of a store; a barrier's workgroup execution, workgroup memory and buffer and image memory; an atomic's operation (enum wc_atomic_op) in the last.
    pub value: [u32; 4],
    /// DEREF: the type it points at. TEX: the result's scalar.
    pub type_: u32,
    /// The bridge's `tex`.
    pub tex: [u32; 10],
    /// The bridge's `dim`.
    pub dim: u32,
    /// The bridge's `arrayed`.
    pub arrayed: u32,
    /// The bridge's `shadow`.
    pub shadow: u32,
    /// TEX: a gather's component. LOAD_STAGE, STORE_STAGE: the first component (or clip distance) of the variable it reaches.
    pub component: u32,
    /// UNSUPPORTED, or an operation outside the enums: Mesa's name of it.
    pub name: Option<Vec<u8>>,
}

/// A geometry program's declarations: its primitives, the vertices of its input primitive, the most it emits, its instances, and one past its last stream.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatGeometry {
    /// The bridge's `input`.
    pub input: u32,
    /// The bridge's `output`.
    pub output: u32,
    /// The bridge's `vertices_in`.
    pub vertices_in: u32,
    /// The bridge's `max_vertices`.
    pub max_vertices: u32,
    /// The bridge's `invocations`.
    pub invocations: u32,
    /// The bridge's `streams`.
    pub streams: u32,
}

/// A tessellation program's declarations: the control points of its input patch (given to wc_nir_flatten: the draw's patch size for a control program, its control program's output for an evaluation program), a control program's output control points, and an evaluation program's domain, spacing, winding and point mode.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatTessellation {
    /// The bridge's `points_in`.
    pub points_in: u32,
    /// The bridge's `points_out`.
    pub points_out: u32,
    /// The bridge's `domain`.
    pub domain: u32,
    /// The bridge's `spacing`.
    pub spacing: u32,
    /// The bridge's `counter_clockwise`.
    pub counter_clockwise: u32,
    /// The bridge's `point_mode`.
    pub point_mode: u32,
}

/// The bridge's `struct wc_flat`, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct Flat {
    /// The bridge's `stage`.
    pub stage: u32,
    /// The bridge's `workgroup_size`.
    pub workgroup_size: [u32; 3],
    /// The bridge's `early_fragment_tests`.
    pub early_fragment_tests: u32,
    /// The bridge's `values`.
    pub values: u32,
    /// The bridge's `types`.
    pub types: Vec<FlatType>,
    /// The bridge's `fields`.
    pub fields: Vec<FlatField>,
    /// The bridge's `vars`.
    pub vars: Vec<FlatVar>,
    /// The bridge's `initialisers`.
    pub initialisers: Vec<u32>,
    /// The bridge's `instrs`.
    pub instrs: Vec<FlatInstr>,
    /// The bridge's `geometry`.
    pub geometry: FlatGeometry,
    /// The bridge's `tessellation`.
    pub tessellation: FlatTessellation,
}

/// The bridge's `struct wc_glsl_name`, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct GlslName {
    /// The bridge's `kind`.
    pub kind: u32,
    /// The bridge's `name`.
    pub name: Option<Vec<u8>>,
    /// A stage value's GLSL type (NULL for other names) and interpolation.
    pub type_: Option<Vec<u8>>,
    /// The bridge's `interpolation`.
    pub interpolation: u32,
    /// The bridge's `at`.
    pub at: u32,
    /// The bridge's `count`.
    pub count: u32,
    /// The first of the value's slots the name places: a vertex attribute read in part (some columns of a matrix) has a name a run of the slots read.
    pub element: u32,
}

/// One GLSL program's stages as the OpenGL driver receives them, by enum wc_stage (NULL where the program has none), each stage's names, and the compiler's and linker's messages; given back with wc_glsl_program_free.
#[derive(Debug)]
pub struct GlslProgram<S> {
    /// The bridge's `stages`.
    pub stages: [Option<S>; 3],
    /// The bridge's `names`.
    pub names: [Vec<GlslName>; 3],
    /// The bridge's `log`.
    pub log: Option<Vec<u8>>,
}

/// A kind's range of binding numbers in a set.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotRange {
    /// The bridge's `first`.
    pub first: u32,
    /// The bridge's `slots`.
    pub slots: u32,
}

/// One resource as the program declared it (set, binding, count) and where it now binds.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    /// The bridge's `kind`.
    pub kind: u32,
    /// The bridge's `set`.
    pub set: u32,
    /// The bridge's `binding`.
    pub binding: u32,
    /// The bridge's `count`.
    pub count: u32,
    /// The bridge's `new_binding`.
    pub new_binding: u32,
    /// For a storage buffer, the stride of its runtime-sized array, else its block's size.
    pub stride: u32,
}

/// How a vertex program's position (and point size) is changed to meet the host's clip space, viewport and points.
#[derive(Clone, Debug, PartialEq)]
pub struct PositionTransform {
    /// y negated before the route's own negation: a Gallium viewport whose y scale is negative.
    pub flip_y: bool,
    /// Depth from OpenGL's -w..w to the host's 0..w.
    pub half_z: bool,
    /// Depth as w - z, for a depth range whose near value is the larger.
    pub reverse_z: bool,
    /// The point size output removed: the host draws points of one size, to which OpenGL clamps every size a program writes.
    pub fixed_point_size: bool,
}

/// The swizzle of the texture bound at one texture unit.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureSwizzle {
    /// The bridge's `unit`.
    pub unit: u32,
    /// The bridge's `swizzle`.
    pub swizzle: [u8; 4],
}

/// The bridge's `struct wc_nearest_sampler`, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct NearestSampler {
    /// The bridge's `unit`.
    pub unit: u32,
    /// The bridge's `wrap`.
    pub wrap: [u8; 3],
    /// The bridge's `mipmapped`.
    pub mipmapped: bool,
    /// The bridge's `min_lod`.
    pub min_lod: f32,
    /// The bridge's `max_lod`.
    pub max_lod: f32,
    /// The bridge's `border`.
    pub border: [u32; 4],
}

/// One output a vertex program streams: the buffer, its byte offset in each vertex there, where the route places the output (a generic location, or the position), and its components.
#[derive(Clone, Debug, PartialEq)]
pub struct StreamOutput {
    /// The bridge's `buffer`.
    pub buffer: u32,
    /// The bridge's `offset`.
    pub offset: u32,
    /// The bridge's `location`.
    pub location: u32,
    /// The bridge's `position`.
    pub position: bool,
    /// The bridge's `first_component`.
    pub first_component: u8,
    /// The bridge's `components`.
    pub components: u8,
}

pub mod constants;

/// Mesa's compiler through the bridge: one method a bridge function, each shader a value of the implementation's
/// own type that owns one NIR shader and gives it back when dropped.
pub trait MesaCompiler: Sized {
    /// One NIR shader, owned.
    type Shader;

    /// The compiler, ready (Mesa's type tables taken for the process).
    fn new() -> Self;

    /// Calls the bridge's `wc_nir_flatten`.
    fn flatten(&self, nir: &mut Self::Shader, points_in: u32) -> Option<Flat>;

    /// Calls the bridge's `wc_spirv_to_nir`.
    fn spirv_to_nir(&self, words: &[u32], stage: u32, entry_point: &core::ffi::CStr) -> Option<Self::Shader>;

    /// Calls the bridge's `wc_glsl_to_nir`.
    fn glsl_to_nir(&self, sources: [Option<&core::ffi::CStr>; 3]) -> Result<GlslProgram<Self::Shader>, Vec<u8>>;

    /// Calls the bridge's `wc_nir_to_common_form`.
    fn to_common_form(&self, nir: &mut Self::Shader, set: u32, ranges: &[SlotRange; 4]) -> Result<Vec<Binding>, u32>;

    /// Calls the bridge's `wc_nir_lower_io`.
    fn lower_io(&self, nir: &mut Self::Shader);

    /// Calls the bridge's `wc_nir_alpha_to_one`.
    fn alpha_to_one(&self, nir: &mut Self::Shader);

    /// Calls the bridge's `wc_nir_serialize`.
    fn serialize(&self, nir: &Self::Shader) -> Option<Vec<u8>>;

    /// Calls the bridge's `wc_nir_to_spirv`.
    fn to_spirv(&self, nir: &mut Self::Shader) -> Option<Vec<u32>>;

    /// Calls the bridge's `wc_nir_clone`.
    fn cloned(&self, nir: &Self::Shader) -> Option<Self::Shader>;

    /// Calls the bridge's `wc_nir_number_vertex_elements`.
    fn number_vertex_elements(&self, nir: &mut Self::Shader) -> u64;

    /// Calls the bridge's `wc_nir_geometry_vertices_out`.
    fn geometry_vertices_out(&self, nir: &Self::Shader) -> u32;

    /// Calls the bridge's `wc_nir_named_layer_location`.
    fn named_layer_location(&self, nir: &Self::Shader) -> u32;

    /// Calls the bridge's `wc_nir_control_points_out`.
    fn control_points_out(&self, nir: &Self::Shader) -> u32;

    /// Calls the bridge's `wc_nir_makes_points`.
    fn makes_points(&self, nir: &Self::Shader) -> bool;

    /// Calls the bridge's `wc_nir_fragcolor_to_buffers`.
    fn fragcolor_to_buffers(&self, nir: &mut Self::Shader, buffers: u32);

    /// Calls the bridge's `wc_nir_split_resource_arrays`.
    fn split_resource_arrays(&self, nir: &mut Self::Shader);

    /// Calls the bridge's `wc_nir_textures_1d_as_2d`.
    fn textures_1d_as_2d(&self, nir: &mut Self::Shader);

    /// Calls the bridge's `wc_nir_drop_name`.
    fn drop_name(&self, nir: &mut Self::Shader);

    /// Calls the bridge's `wc_nir_uniform_blocks_as_words`.
    fn uniform_blocks_as_words(&self, nir: &mut Self::Shader);

    /// Calls the bridge's `wc_nir_transform_position`.
    fn transform_position(&self, nir: &mut Self::Shader, transform: PositionTransform);

    /// Calls the bridge's `wc_nir_reads_sample_positions`.
    fn reads_sample_positions(&self, nir: &mut Self::Shader) -> bool;

    /// Calls the bridge's `wc_nir_interpolate_elsewhere`.
    fn interpolate_elsewhere(&self, nir: &mut Self::Shader, samples: u32) -> bool;

    /// Calls the bridge's `wc_nir_fit_sample_masks`.
    fn fit_sample_masks(&self, nir: &mut Self::Shader, single_sampled: bool);

    /// Calls the bridge's `wc_nir_writes_sample_mask`.
    fn writes_sample_mask(&self, nir: &mut Self::Shader) -> bool;

    /// Calls the bridge's `wc_nir_fetches_target`.
    fn fetches_target(&self, nir: &mut Self::Shader) -> bool;

    /// Calls the bridge's `wc_nir_formatless_images`.
    fn formatless_images(&self, nir: &mut Self::Shader) -> u64;

    /// Calls the bridge's `wc_nir_fetch_from_texture`.
    fn fetch_from_texture(&self, nir: &mut Self::Shader, unit: u32, multisampled: bool);

    /// Calls the bridge's `wc_nir_swizzle_textures`.
    fn swizzle_textures(&self, nir: &mut Self::Shader, swizzles: &[TextureSwizzle]);

    /// Calls the bridge's `wc_nir_nearest_integer_textures`.
    fn nearest_integer_textures(&self, nir: &mut Self::Shader, samplers: &[NearestSampler]) -> bool;

    /// Calls the bridge's `wc_nir_integer_sampled_units`.
    fn integer_sampled_units(&self, nir: &mut Self::Shader) -> u64;

    /// Calls the bridge's `wc_nir_stream_point_size_at_a_location`.
    fn stream_point_size_at_a_location(&self, nir: &mut Self::Shader);

    /// Calls the bridge's `wc_nir_generic_end`.
    fn generic_end(&self, nir: &Self::Shader) -> u32;

    /// Calls the bridge's `wc_nir_point_size_also_at`.
    fn point_size_also_at(&self, nir: &mut Self::Shader, at: u32) -> bool;

    /// Calls the bridge's `wc_nir_point_size_at_most`.
    fn point_size_at_most(&self, nir: &Self::Shader, size: f32) -> bool;

    /// Calls the bridge's `wc_nir_point_square`.
    fn point_square(&self, vs: &Self::Shader, at: u32, largest: f32) -> Option<Self::Shader>;

    /// Calls the bridge's `wc_nir_geometry_squares`.
    fn geometry_squares(&self, gs: &Self::Shader, at: u32, largest: f32, block: u32) -> Option<Self::Shader>;

    /// Calls the bridge's `wc_nir_storage_pass_through`.
    fn storage_pass_through(&self, last: &mut Self::Shader) -> Option<[Option<Self::Shader>; 5]>;

    /// Calls the bridge's `wc_nir_stream_outputs`.
    fn stream_outputs(&self, nir: &Self::Shader, outputs: &mut [StreamOutput], strides: &mut [u32]) -> u32;

}
