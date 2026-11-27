// Written by tools/api-crate-generator from the Mesa compiler bridge's bridge.h and flat.h (crates/mesa-compiler/implementation/direct/bridge). Do not edit.
//! Mesa's shader compiler as our C bridge offers it (crates/mesa-compiler/implementation/direct/bridge: NIR made from SPIR-V or GLSL, changed for the
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
    /// The common form's set and each kind's range.
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
    /// The common form's set and each kind's range.
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

/// A program read from SPIR-V in the common form, with its bindings; the shader NULL when it is refused.
#[derive(Debug)]
pub struct SpirvCommonForm<SpirvCommon> {
    /// The bridge's `shader`.
    pub shader: Option<SpirvCommon>,
    /// The bridge's `bindings`.
    pub bindings: Vec<Binding>,
    /// The bridge's `result`.
    pub result: u32,
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
pub struct GlslProgram<Received> {
    /// The bridge's `stages`.
    pub stages: [Option<Received>; 3],
    /// The bridge's `names`.
    pub names: [Vec<GlslName>; 3],
    /// The bridge's `log`.
    pub log: Option<Vec<u8>>,
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

/// A stage after the driver's steps at its creation, and what the driver reads of it then and at its draws.
#[derive(Debug)]
pub struct PreparedForm<Prepared, Made> {
    /// NULL when the cache's key could not be made (out of memory).
    pub shader: Option<Prepared>,
    /// The cache's key: NIR's serialised form (a prepared stage's after every step, state.rs:1178 to 1180; a variant's as the settings say, draw.rs:807 to 815).
    pub key: Vec<u8>,
    /// The outputs it streams and each buffer's bytes a vertex (state.rs:1157 to 1162); none when it streams a value the route places neither at a location nor as the position (`streams_elsewhere`).
    pub streamed: Vec<StreamOutput>,
    /// The bridge's `strides`.
    pub strides: [u32; 4],
    /// The bridge's `streams_elsewhere`.
    pub streams_elsewhere: bool,
    /// state.rs:1164 to 1169.
    pub reads_sample_positions: bool,
    /// The bridge's `writes_sample_mask`.
    pub writes_sample_mask: bool,
    /// The bridge's `fetches_target`.
    pub fetches_target: bool,
    /// The bridge's `formatless_images`.
    pub formatless_images: u64,
    /// The bridge's `integer_sampled_units`.
    pub integer_sampled_units: u64,
    /// UINT32_MAX for a program naming no layer.
    pub named_layer: u32,
    /// A vertex or evaluation program using storage: the pass-through geometry programs by the primitive taken (state.rs:1174 to 1176); `passes_through` false for any other.
    pub passes_through: bool,
    /// The bridge's `pass_through`.
    pub pass_through: [Option<Made>; 5],
    /// What the driver reads at its draws (draw.rs:2346, 2393, 2414, 2576, 3422 and 3509).
    pub generic_end: u32,
    /// The bridge's `geometry_vertices_out`.
    pub geometry_vertices_out: u32,
    /// The bridge's `control_points_out`.
    pub control_points_out: u32,
    /// The bridge's `makes_points`.
    pub makes_points: bool,
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

/// What a draw makes a variant of a program for (draw.rs:779 to 818).
#[derive(Clone, Debug, PartialEq)]
pub struct VariantSettings {
    /// The position's transform (none when every field is false).
    pub transform: PositionTransform,
    /// The transform the cache's key is taken with: where it is not `transform`, the key is of a copy with it applied after `transform` (a geometry program's records, draw.rs:807 to 815).
    pub key_transform: PositionTransform,
    /// A program reading its first target's texels reads them from the texture at this unit, multisampled where the target is.
    pub fetch_unit: u32,
    /// The bridge's `fetch_multisampled`.
    pub fetch_multisampled: bool,
    /// Targets of one sample, and their sample count.
    pub single_sampled: bool,
    /// The bridge's `samples`.
    pub samples: u32,
    /// The common form's set and each kind's range.
    pub set: u32,
    /// The bridge's `ranges`.
    pub ranges: [SlotRange; 4],
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

/// A draw's variant, its key and its bindings; the shader NULL when `result` is not WC_VARIANT_MADE.
#[derive(Debug)]
pub struct VariantForm<Variant> {
    /// The bridge's `shader`.
    pub shader: Option<Variant>,
    /// The bridge's `result`.
    pub result: u32,
    /// The cache's key: NIR's serialised form (a prepared stage's after every step, state.rs:1178 to 1180; a variant's as the settings say, draw.rs:807 to 815).
    pub key: Vec<u8>,
    /// The bridge's `bindings`.
    pub bindings: Vec<Binding>,
}

/// The bridge's `struct wc_squares`, owned.
#[derive(Debug)]
pub struct Squares<Made> {
    /// The bridge's `kind`.
    pub kind: u32,
    /// Whether the program writes a point size (then also at `at`).
    pub sized: bool,
    /// The vertex program drawn before the squares where it writes a point size: a copy also writing it at `at`.
    pub before: Option<Made>,
    /// The bridge's `geometry`.
    pub geometry: Option<Made>,
}

pub mod constants;

/// Mesa's compiler through the bridge: one method a bridge function. A shader is a value of one of the
/// implementation's types, one a place of the bridge's typed chain (bridge.h), each owning one NIR shader and giving it
/// back when dropped; a method takes only the type of its place, so a shader goes only through the orders the bridge's
/// callers take.
pub trait MesaCompiler: Sized {
    /// A program read from SPIR-V for Vulkan: the corpus's first step (run.rs:493 to 496).
    type SpirvShader;

    /// A program read from SPIR-V in the common form: what the corpus's candidates convert (run.rs:570 and 671).
    type SpirvCommon;

    /// A stage as the OpenGL driver receives it from st/mesa (state.rs:1124 to 1138): here from Mesa's GLSL compiler and st/mesa's steps (glsl.cpp).
    type Received;

    /// A stage after the driver's steps at its creation (state.rs:1139 to 1180), which nothing changes after.
    type Prepared;

    /// A program the driver's steps make: a pass-through geometry program (state.rs:1175), a vertex program with its point size also at a location, or a geometry program drawing points as squares (draw.rs:3426 to 3441 and 3514 to 3522).
    type Made;

    /// A draw's variant of a prepared or made program, in the common form (draw.rs:779 to 818).
    type Variant;

    /// The compiler, ready (Mesa's type tables taken for the process).
    fn new() -> Self;

    /// Reads one entry point of a SPIR-V module for Vulkan, and, with `alpha_to_one`, makes Vulkan's alphaToOne in a fragment program: the corpus reads each program, then makes alphaToOne in those of its folder for it (run.rs:493 to 496). NULL for words that are no valid module for Vulkan, or that Mesa's reader refuses.
    ///
    /// Calls the bridge's `wc_spirv_read`.
    fn spirv_read(&self, words: &[u32], stage: u32, entry_point: &core::ffi::CStr, alpha_to_one: bool) -> Option<Self::SpirvShader>;

    /// The cache's key of a program read: NIR's serialised form, taken before the common form (run.rs:567).
    ///
    /// Calls the bridge's `wc_spirv_shader_key`.
    fn spirv_shader_key(&self, shader: &Self::SpirvShader) -> Option<Vec<u8>>;

    /// Gives the program the common form's meaning and bindings in `set` (each kind from its range's first binding), the step both candidates take first (run.rs:570 and 671).
    ///
    /// Calls the bridge's `wc_spirv_to_common_form`.
    fn spirv_to_common_form(&self, taken: Self::SpirvShader, set: u32, ranges: &[SlotRange; 4]) -> SpirvCommonForm<Self::SpirvCommon>;

    /// Candidate A: the program written as SPIR-V by Zink's compiler in its native Vulkan mode (run.rs:672). False for a program not in the form that mode takes.
    ///
    /// Calls the bridge's `wc_spirv_common_to_spirv`.
    fn spirv_common_to_spirv(&self, taken: Self::SpirvCommon) -> Option<Vec<u32>>;

    /// Candidate B: the program out of SSA and flattened (run.rs:571, through crates/nir/src/direct.rs:974 to 984).
    ///
    /// Calls the bridge's `wc_spirv_common_flatten`.
    fn spirv_common_flatten(&self, taken: Self::SpirvCommon) -> Option<Flat>;

    /// Compiles and links the sources (by enum wc_stage, NULL for none) with Mesa's GLSL compiler, then takes st/mesa's steps for the OpenGL driver's capabilities; false when either step fails.
    ///
    /// Calls the bridge's `wc_glsl_to_nir`.
    fn glsl_to_nir(&self, sources: [Option<&core::ffi::CStr>; 3]) -> Result<GlslProgram<Self::Received>, Vec<u8>>;

    /// The driver's steps at a stage's creation, in its order (state.rs:1139 to 1180): the name dropped, uniform blocks as words, resource arrays split, 1D textures as 2D; a vertex program's inputs numbered as vertex elements, a fragment program's gl_FragColor written to every draw buffer; the last stage before the rasterizer's point size streamed at a location; the facts read; a vertex or evaluation program's pass-through programs made.
    ///
    /// Calls the bridge's `wc_received_prepare`.
    fn received_prepare(&self, taken: Self::Received) -> PreparedForm<Self::Prepared, Self::Made>;

    /// The driver's steps at a draw, in its order (draw.rs:779 to 818), on a copy of the program: the position transformed, integer textures' samplings made nearest loads at the units of `samplers`, the textures at the units of `swizzles` swizzled, the first target's texels read from `fetch_unit` (a program that reads them), the sample masks fitted, reads at offsets and samples made from the interpolated values; the key taken; the common form.
    ///
    /// Calls the bridge's `wc_prepared_variant`.
    fn prepared_variant(&self, shader: &Self::Prepared, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]) -> VariantForm<Self::Variant>;

    /// A vertex program's points drawn as squares where the back end draws points of one size (draw.rs:3426 to 3441): a copy writing its point size also at the generic slot `at`; nothing where every point is a pixel; the geometry program drawing the squares (sized to at most `largest`), its uniform blocks as words.
    ///
    /// Calls the bridge's `wc_prepared_points_as_squares`.
    fn prepared_points_as_squares(&self, vs: &Self::Prepared, at: u32, largest: f32) -> Squares<Self::Made>;

    /// A geometry program's points drawn as squares (draw.rs:3514 to 3522): nothing where every point is a pixel; the program rewritten to draw squares, of at most `largest`, its size else read from uniform block `block`.
    ///
    /// Calls the bridge's `wc_prepared_geometry_points_as_squares`.
    fn prepared_geometry_points_as_squares(&self, gs: &Self::Prepared, at: u32, largest: f32, block: u32) -> Squares<Self::Made>;

    /// The cache's key of a made program (state.rs:1186, draw.rs:3445 and 3525).
    ///
    /// Calls the bridge's `wc_made_key`.
    fn made_key(&self, shader: &Self::Made) -> Option<Vec<u8>>;

    /// For a made geometry program writing gl_Layer, the location after its generic outputs (state.rs:1195); UINT32_MAX for one naming no layer.
    ///
    /// Calls the bridge's `wc_made_named_layer_location`.
    fn made_named_layer_location(&self, shader: &Self::Made) -> u32;

    /// The driver's steps at a draw on a made program, as on a prepared one (draw.rs:779 to 818).
    ///
    /// Calls the bridge's `wc_made_variant`.
    fn made_variant(&self, shader: &Self::Made, settings: VariantSettings, swizzles: &[TextureSwizzle], samplers: &[NearestSampler]) -> VariantForm<Self::Variant>;

    /// A variant out of SSA and flattened: the route the driver converts it by (draw.rs:840, 978, 1034 and 1081, through crates/nir/src/direct.rs:974 to 984), `points_in` a tessellation program's input control points.
    ///
    /// Calls the bridge's `wc_variant_flatten`.
    fn variant_flatten(&self, taken: Self::Variant, points_in: u32) -> Option<Flat>;

}
