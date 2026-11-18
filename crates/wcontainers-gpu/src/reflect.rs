//! What a converted shader binds, reads and writes, and which variant of it a front asks for.
//!
//! This is the contract between `wcontainers-shaders`, which converts an API's shader into the common
//! form and fills a [`Reflection`], and the fronts, which bind resources and write helper constants by
//! it. Back ends never read it: they see the program's module and the binding sets only.

use crate::format::Format;
use crate::shader::ShaderStage;
use crate::state::VertexFormat;
use alloc::string::String;
use alloc::vec::Vec;
use bitflags::bitflags;

/// The kind of API slot a resource comes from. Each kind owns a fixed range of binding numbers in every
/// binding set, so a slot's binding number never depends on what else the program uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SlotKind {
    /// Constant buffers, uniform blocks.
    Uniform,
    Sampler,
    /// Sampled textures and typed buffers read through a view.
    Texture,
    /// Storage buffers and textures (Direct3D's unordered access views).
    Storage,
    /// The hidden counter of a storage slot (Direct3D's append, consume and counter views): a storage
    /// buffer range of one u32, numbered by the storage slot it belongs to.
    Counter,
}

impl SlotKind {
    /// The first binding number of the kind's range.
    pub const fn first_binding(self) -> u32 {
        match self {
            SlotKind::Uniform => 0,
            SlotKind::Sampler => 16,
            SlotKind::Texture => 32,
            SlotKind::Storage => 160,
            SlotKind::Counter => 224,
        }
    }

    /// How many slots the range holds.
    pub const fn slots(self) -> u32 {
        match self {
            SlotKind::Uniform | SlotKind::Sampler => 16,
            SlotKind::Texture => 128,
            SlotKind::Storage | SlotKind::Counter => 64,
        }
    }

    /// The binding number of slot `slot`, if the range holds it.
    pub const fn binding(self, slot: u32) -> Option<u32> {
        if slot < self.slots() {
            Some(self.first_binding() + slot)
        } else {
            None
        }
    }
}

/// The binding set that holds a converted program's helper constants (one uniform buffer per stage, at
/// [`helper_binding`]) and the other bindings a variant adds.
pub const HELPER_SET: u32 = 3;

/// The binding in [`HELPER_SET`] of the uniform buffer holding one stage's helper constants. Each stage
/// of a program has its own block, laid out by that stage's own [`Reflection::helpers`], so the vertex
/// and fragment stages of one draw never read each other's values; the front writes both blocks.
pub const fn helper_binding(stage: ShaderStage) -> u32 {
    match stage {
        ShaderStage::Vertex | ShaderStage::Compute => 0,
        ShaderStage::Fragment => 12,
    }
}
/// The binding in [`HELPER_SET`] of the copy of color target 0 a program reads to blend or apply a
/// logic operation itself ([`TargetInShader::combine`]); target n is at this plus n.
pub const TARGET_COPY_BINDING: u32 = 1;
/// The binding in [`HELPER_SET`] of the depth aspect of the pass's read-only depth-stencil attachment,
/// read by a program that runs its early depth test itself ([`VariantFlags::EARLY_TESTS`]).
pub const EARLY_DEPTH_BINDING: u32 = 9;
/// The binding in [`HELPER_SET`] of the stencil aspect of that attachment.
pub const EARLY_STENCIL_BINDING: u32 = 10;
/// The bindings in [`HELPER_SET`] of the merged samplers of a clamped program: pair n of
/// [`Reflection::sampled_pairs`], when its texture slot is in [`VariantKey::min_lod_clamped`], samples
/// through the sampler at this plus n.
pub const CLAMPED_SAMPLER_BINDING: u32 = 16;
/// The binding in [`HELPER_SET`] of an indirect draw's argument buffer, bound read-only so the program
/// reads the draw's bases from it ([`DrawBaseSource::Arguments`]).
pub const DRAW_ARGUMENTS_BINDING: u32 = 11;
/// The binding in [`HELPER_SET`] of the hull stage's patches a domain program reads, a read-only
/// storage buffer the shaders crate's stage helper owns and names with the draw it asks for.
pub const DOMAIN_PATCHES_BINDING: u32 = 13;
/// The binding in [`HELPER_SET`] of the tessellator's points a domain program reads, likewise.
pub const DOMAIN_POINTS_BINDING: u32 = 14;
/// The width in texels of the texture a constant buffer is read through
/// ([`AddedBinding::ConstantsAsTexture`]): the buffer's 16-byte row r is texel (r % width, r / width).
pub const CONSTANTS_TEXTURE_WIDTH: u32 = 256;
/// The width in texels of the 2D texture a typed buffer is read through ([`TexelBufferRoute::Texture`]):
/// element e of the buffer, counted in the view's format from the buffer's start, is texel
/// (e % width, e / width). WebGL 2's smallest texture size limit.
pub const TEXEL_BUFFER_TEXTURE_WIDTH: u32 = 2048;

/// The API stage a shader was written for. Geometry, hull and domain shaders become compute and vertex
/// programs in the shaders crate's stage helper; the others become one program stage each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SourceStage {
    Vertex,
    Hull,
    Domain,
    Geometry,
    Pixel,
    Compute,
}

/// One resource the shader uses, where the API names it and where the converted program binds it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ReflectedBinding {
    pub kind: SlotKind,
    /// The API's register space (Direct3D 12) or zero.
    pub space: u32,
    /// The API's slot: Direct3D's register number, OpenGL's binding point.
    pub register: u32,
    /// How many consecutive slots (an array of resources); one for a single resource.
    pub count: u32,
    /// Where the converted program binds it: set and binding number.
    pub set: u32,
    pub binding: u32,
    /// For a binding the variant adds rather than one of the API's slots, what it holds; `space`,
    /// `register` and `count` then say nothing.
    pub added: Option<AddedBinding>,
}

/// A binding a variant adds to a program. It lives in the set of the one stage that reads it (set 0 for
/// vertex and compute programs, set 1 for fragment programs; plan section 6.4), numbered in its kind's
/// range from the first binding past the highest one of that kind the stage's program already uses, so
/// the stages of one draw, converted apart, never collide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AddedBinding {
    /// The vertex stream feeding the program's input at location n, fetched in the program (a
    /// read-only storage buffer of u32 words, bound from the stream's offset rounded down to the
    /// storage offset alignment). Each pulled input has its own, so a stream feeding two inputs is
    /// bound twice.
    PulledStream(u32),
    /// The constant buffer in the stage's slot n (Direct3D 12: the root signature's constant buffer
    /// range n), read as a read-only storage buffer.
    ConstantsAsStorage(u32),
    /// The same, read through a 2D RGBA32 unsigned integer texture the front copies the buffer into,
    /// one 16-byte row per texel laid out as [`CONSTANTS_TEXTURE_WIDTH`] says.
    ConstantsAsTexture(u32),
    /// Root constants past the back end's immediate bytes, as a uniform buffer (Direct3D 12).
    SpilledImmediates,
}

/// A value the API's pipeline gives a stage, or takes from it, by meaning rather than by location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SystemValue {
    Position,
    /// The API's vertex number. Direct3D's `SV_VertexID` leaves out the draw's first or base vertex;
    /// OpenGL's and Vulkan's include it, as the common form's `vertex_index` does.
    VertexId,
    /// The API's instance number. Direct3D's `SV_InstanceID` and OpenGL's `gl_InstanceID` leave out
    /// the first instance; Vulkan's `InstanceIndex` includes it, as the common form's
    /// `instance_index` does.
    InstanceId,
    /// The draw's first or base vertex (OpenGL's `gl_BaseVertex`, Vulkan's `BaseVertex`).
    BaseVertex,
    /// The draw's first instance (OpenGL's `gl_BaseInstance`, Vulkan's `BaseInstance`).
    BaseInstance,
    /// The draw's number within a multi-draw (OpenGL's `gl_DrawID`, Vulkan's `DrawIndex`).
    DrawIndex,
    PrimitiveId,
    FrontFacing,
    SampleIndex,
    Coverage,
    Depth,
    DepthGreaterEqual,
    DepthLessEqual,
    StencilRef,
    ClipDistance,
    CullDistance,
    RenderTargetArrayIndex,
    ViewportArrayIndex,
    /// Color target n (Direct3D's `SV_Target`n).
    Target(u8),
}

/// The scalar type of a stage value's components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ScalarKind {
    Float,
    Sint,
    Uint,
}

/// How a value is interpolated between vertices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Interpolation {
    Perspective,
    Linear,
    Flat,
    PerspectiveCentroid,
    LinearCentroid,
    PerspectiveSample,
    LinearSample,
}

/// One input or output of a stage, as the API's signature names it (Direct3D's semantic name and index,
/// register and component mask), and the location the converted program gives it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StageValue {
    pub semantic: String,
    pub semantic_index: u32,
    pub register: u32,
    /// Components used, a bit per component (x = 1).
    pub mask: u8,
    pub scalar: ScalarKind,
    pub interpolation: Interpolation,
    pub system: Option<SystemValue>,
    /// The converted program's location (inputs and outputs between stages are packed by component
    /// where their interpolation agrees, so several values may share one location).
    pub location: u32,
    /// The first component the value takes in its location.
    pub component: u8,
}

/// What a stage needs beyond its bindings and values.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StageExtras {
    None,
    Pixel {
        /// Depth and stencil tests run before the shader (Direct3D's `earlydepthstencil`).
        early_tests: bool,
        /// The shader writes storage views or counters.
        side_effects: bool,
        /// The shader may discard.
        discards: bool,
        /// The shader runs once per sample.
        per_sample: bool,
    },
    Compute {
        workgroup_size: [u32; 3],
    },
    Geometry {
        /// Vertices per input primitive (1, 2, 3, or 4 and 6 with adjacency).
        input_vertices: u8,
        max_output_vertices: u32,
        /// Output primitives are points (1), line strips (2) or triangle strips (3).
        output_vertices: u8,
        instances: u32,
        /// Streams written, a bit per stream.
        streams: u8,
    },
    Hull {
        input_control_points: u8,
        output_control_points: u8,
        /// Triangle (0), quad (1) or isoline (2) domain.
        domain: u8,
        /// Integer (0), pow2 (1), fractional odd (2) or fractional even (3) partitioning.
        partitioning: u8,
        /// Point (0), line (1), clockwise (2) or counter-clockwise (3) triangle output.
        output_primitive: u8,
        max_tess_factor: u32,
        /// Locations written per output control point, and locations of patch constants: what the
        /// stage helper's record of a patch holds.
        point_locations: u8,
        constant_locations: u8,
    },
    Domain {
        /// Triangle (0), quad (1) or isoline (2) domain.
        domain: u8,
        input_control_points: u8,
    },
}

/// A value the front writes into the program's helper constants before a draw or dispatch, at the byte
/// offset the reflection gives. Each exists only in programs whose variant needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HelperValue {
    /// The part of a storage binding's offset below [`crate::Limits::min_storage_offset_alignment`],
    /// in bytes (u32), which the program adds to its addresses.
    StorageOffset { set: u32, binding: u32 },
    /// A sampler's level-of-detail bias (f32), added to every sampling through it.
    LodBias { sampler: u32 },
    /// A sampler's border color (four f32, or four u32 for integer textures).
    BorderColor { sampler: u32 },
    /// The components the bound view's format has ([`crate::Format::components`], u32), for
    /// programs that apply the border in the program: a border colour is read as a texel of the
    /// view's format, so a component the format lacks reads its default (zero, one for alpha).
    TextureComponents { texture: u32 },
    /// A sampler's state, for programs that filter or address in the program: min, mag and mip filter
    /// and the three address modes as u32 codes (the order of [`crate::Filter`]'s and
    /// [`crate::AddressMode`]'s variants, from zero), anisotropy (u32), then the level range (two f32).
    SamplerState { sampler: u32 },
    /// The bound view's mip level count (u32), in programs made with [`VariantFlags::TEXTURE_INFO`].
    TextureLevels { texture: u32 },
    /// The bound texture's sample count (u32, one for a texture that is not multisampled), in programs
    /// made with [`VariantFlags::TEXTURE_INFO`].
    TextureSamples { texture: u32 },
    /// Depth bias added in the program: constant units, slope scale, clamp, and the depth format's
    /// smallest step (four f32).
    DepthBias,
    /// How the program fetches its input at location `input` ([`VariantKey::pulled`]), four u32: the
    /// byte offset of the input's first element in its binding (the stream's offset remainder plus the
    /// attribute's offset), the stride, a divisor and a bias. The element read is the vertex index
    /// when the divisor is zero, otherwise (instance index + bias) / divisor, the instance index
    /// counting from zero as the common form's does from the first instance: a step rate n takes
    /// divisor n and bias (n - 1) times the first instance, and a step rate of zero takes divisor one,
    /// bias zero, stride zero and the first instance's element in the offset.
    PulledStream { input: u32 },
    /// The sample mask the program writes (u32).
    SampleMask,
    /// Where a packed storage slot's range starts in the packed buffer, in bytes (u32).
    PackedStorage { slot: u32 },
    /// The texel layout of a storage view read through a buffer: row pitch in texels, rows per layer
    /// (or depth slice), then the view's width, height, and layers (or depth), five u32 in two
    /// 16-byte rows. A texel outside the view reads zero and takes no writes; the view's first texel is
    /// the buffer's first.
    TexelLayout { slot: u32 },
    /// A typed buffer view read through a storage buffer or a texture ([`TexelBufferRoute`]) at this
    /// binding: the byte offset of the view's first element in the bound range (in the buffer the
    /// texture copies) and the view's element count, two u32. Elements at or past the count read zero in the format's components
    /// and the defaults in the others, as an element of a vertex stream does, and take no writes.
    TexelBufferView { set: u32, binding: u32 },
    /// How the program combines target n with its old value: color and alpha source and destination
    /// factors and operations, the write mask and the logic operation as u32 codes (eight u32), then the
    /// draw's blend constant (four f32).
    TargetCombine { target: u8 },
    /// The early tests the program runs itself: depth compare, front and back stencil compare (u32
    /// codes, zero for a test that is off), stencil reference and read mask (u32), eight u32.
    EarlyTests,
    /// A texture's per-resource minimum level-of-detail clamp, in the view's level numbering (f32), and
    /// the view's level count (u32): loads and gathers of a level below the clamp's whole part, and every
    /// access when the clamp passes the last level, read zero.
    MinLod { texture: u32 },
    /// The viewport a front's API gives (fractional, or past [`crate::Viewport`]'s bounds) within the
    /// whole-pixel one the back end draws with: scale and offset of
    /// clip x and y (four f32), applied to the last vertex stage's position.
    ViewportAdjust,
    /// The draw's bases as the API counts them, for [`DrawBaseSource::Helpers`]: base vertex (i32: the
    /// first vertex of a draw without indices, the base vertex of an indexed one), first instance (u32)
    /// and draw number (u32).
    DrawBase,
    /// Where an indirect draw's arguments are, for [`DrawBaseSource::Arguments`]: the word offset of
    /// the draw's arguments in the buffer at [`DRAW_ARGUMENTS_BINDING`], whether they are indexed (u32,
    /// zero or one) and the draw number (u32).
    DrawArguments,
    /// The viewport's depth range (two f32, near then far), which [`VariantFlags::DEPTH_CLAMP`] clamps
    /// the fragment's depth to.
    DepthRange,
}

/// Where one helper value sits in the helper constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct HelperSlot {
    pub value: HelperValue,
    pub offset: u32,
}

/// Everything a front needs to know of a converted shader.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Reflection {
    pub stage: SourceStage,
    pub inputs: Vec<StageValue>,
    pub outputs: Vec<StageValue>,
    pub bindings: Vec<ReflectedBinding>,
    /// The helper constants the program reads, at offsets in its own stage's block (bound at
    /// [`helper_binding`] of that stage), and the size of that block in bytes (zero for none). The stage
    /// helper fills the blocks of the programs it makes from geometry, hull and domain shaders itself.
    pub helpers: Vec<HelperSlot>,
    pub helper_size: u32,
    /// The (texture slot, sampler slot) pairs the shader samples with, in a fixed order.
    pub sampled_pairs: Vec<(u32, u32)>,
    pub extras: StageExtras,
}

bitflags! {
    /// Whole-program rewrites a variant applies.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
    pub struct VariantFlags: u32 {
        /// Clip depth rewritten as `2z - w` ([`crate::SlowPath::ClipDepthRewrite`]).
        const CLIP_DEPTH_REWRITE = 1 << 0;
        /// Clip depth rewritten as `w - z` for a swapped depth range
        /// ([`crate::SlowPath::ViewportDepthFlip`]).
        const VIEWPORT_DEPTH_FLIP = 1 << 1;
        /// Flat values taken from the last vertex ([`crate::SlowPath::ProvokingRewrite`]).
        const PROVOKING_LAST = 1 << 2;
        /// Depth bias added to the fragment's depth ([`crate::SlowPath::DepthBiasInShader`]).
        const DEPTH_BIAS = 1 << 3;
        /// The fragment stage writes 1.0 to target 0 (alpha kept for alpha to coverage) and nothing
        /// else, for the occlusion counting replay ([`crate::SlowPath::OcclusionCountReplay`]).
        const COUNTING = 1 << 4;
        /// The pixel shader's second output feeds dual-source blending.
        const DUAL_SOURCE = 1 << 5;
        /// The fragment stage writes the sample mask from the helper constants
        /// ([`crate::SlowPath::SampleMaskInShader`]).
        const SAMPLE_MASK = 1 << 6;
        /// The pixel shader's early depth and stencil tests run in the program before its side effects,
        /// against the read-only attachment at [`EARLY_DEPTH_BINDING`] and [`EARLY_STENCIL_BINDING`],
        /// with the fragment's depth biased as the raster block says; a failing fragment is discarded
        /// ([`crate::SlowPath::EarlyTestsInShader`], [`crate::SlowPath::EarlyTestsPerPrimitive`]).
        const EARLY_TESTS = 1 << 7;
        /// The last vertex stage maps clip x and y by [`HelperValue::ViewportAdjust`]
        /// ([`crate::SlowPath::ViewportInShader`]).
        const VIEWPORT_ADJUST = 1 << 8;
        /// Texture level and sample counts (Direct3D's `resinfo` and `sampleinfo`, `GetDimensions`)
        /// read from [`HelperValue::TextureLevels`] and [`HelperValue::TextureSamples`], which the front
        /// writes from the bound views, instead of asked of the texture: for back ends without
        /// [`crate::Features::TEXTURE_LEVEL_QUERY`] ([`crate::SlowPath::TextureInfoInHelpers`]).
        const TEXTURE_INFO = 1 << 9;
        /// The pixel shader's target 0 alpha multiplied by the line coverage the stage helper's
        /// rectangle passes in ([`crate::SlowPath::AntialiasedLinesAsQuads`]).
        const LINE_COVERAGE = 1 << 10;
        /// Clip distances made ordinary perspective-correct values between the stages (at
        /// [`VariantKey::linked`]), and the fragment stage discards where one is negative, or on
        /// multisampled targets ([`VariantKey::samples`]) clears the samples where one is, from each
        /// distance times the fragment position's w (affine on the screen; no `noperspective`, which
        /// GLSL ES 3.00 lacks) ([`crate::SlowPath::ClipDistanceInShader`]).
        const CLIP_DISTANCE = 1 << 11;
        /// Depth clamped instead of clipped: the last vertex stage keeps clip z inside the clip volume
        /// and passes the unclamped z as a perspective-correct value (at [`VariantKey::linked`]), and
        /// the fragment stage writes it times the fragment position's w (z over w) as the depth,
        /// biased as the raster block says (with [`VariantFlags::DEPTH_BIAS`]) and clamped to
        /// [`HelperValue::DepthRange`] ([`crate::SlowPath::DepthClampInShader`]).
        const DEPTH_CLAMP = 1 << 12;
        /// The pixel shader reads its primitive index from a flat value the stage helper's draw passes,
        /// not from the built-in ([`crate::SlowPath::PrimitiveIdAsGeometry`]).
        const PRIMITIVE_ID_FROM_STAGE = 1 << 13;
    }
}

/// Where a program that needs the draw's bases reads them. The common form's `vertex_index` and
/// `instance_index` include the bases (as WebGPU's and Vulkan's do; [`crate::DrawRange`]); an API value
/// that leaves them out ([`SystemValue::VertexId`], [`SystemValue::InstanceId`]) or names them
/// ([`SystemValue::BaseVertex`], [`SystemValue::BaseInstance`], [`SystemValue::DrawIndex`]) is made from
/// them by the shaders crate.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum DrawBaseSource {
    /// The program reads no base.
    #[default]
    None,
    /// From [`HelperValue::DrawBase`], which the front writes for each direct draw whose bases differ
    /// from the last ([`crate::SlowPath::DrawBaseInHelpers`]).
    Helpers,
    /// From the indirect arguments at [`DRAW_ARGUMENTS_BINDING`], located by
    /// [`HelperValue::DrawArguments`] ([`crate::SlowPath::DrawBaseFromArguments`]).
    Arguments,
}

/// How a program reads typed buffers (Direct3D's `Buffer` and `RWBuffer`, OpenGL's texture buffers).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum TexelBufferRoute {
    /// As texel buffers, where [`crate::Features::TEXEL_BUFFERS`] is present.
    #[default]
    Native,
    /// From storage buffers, the format decoded in the program
    /// ([`crate::SlowPath::TexelBufferAsStorage`]): each typed buffer is a storage buffer of u32
    /// words at its slot's binding (read-only for a texture slot), the view's place in it and its
    /// length in [`HelperValue::TexelBufferView`], its format from [`VariantKey::texel_formats`]
    /// (texture slots) or [`VariantKey::storage_formats`] (storage slots).
    Storage,
    /// From a 2D texture of the view's format the front copies the buffer into, laid out as
    /// [`TEXEL_BUFFER_TEXTURE_WIDTH`] says, at the slot's binding, the view's place and length in
    /// [`HelperValue::TexelBufferView`] and its format from [`VariantKey::texel_formats`]
    /// ([`crate::SlowPath::TexelBufferAsTexture`]). Typed buffers at texture slots only.
    Texture,
}

/// A color target the program writes itself: encoded into the guest's format where the texture is
/// stored in another, and combined with its old value where the back end cannot blend it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TargetInShader {
    pub target: u8,
    /// The guest's format, which the program encodes its output into
    /// ([`crate::SlowPath::NormAsInteger`]).
    pub format: Format,
    /// The program combines its output with the target's old value, read from a copy at
    /// [`TARGET_COPY_BINDING`], by [`HelperValue::TargetCombine`] ([`crate::SlowPath::BlendInShader`],
    /// [`crate::SlowPath::LogicOpInShader`]).
    pub combine: bool,
}

/// The values a vertex program adds for its pixel program, which is converted apart: four-component
/// f32 values from `location` holding the vertex program's `clip_distances` clip distances (with
/// [`VariantFlags::CLIP_DISTANCE`], zero without), then its unclamped clip z (with
/// [`VariantFlags::DEPTH_CLAMP`]). `location` is one past every location either program's reflection
/// lists.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct LinkedValues {
    pub location: u32,
    pub clip_distances: u8,
}

/// Which variant of a converted shader a front asks for. It is part of the converted shader's cache key.
/// Each per-slot field has a bit per API slot of that kind.
///
/// Every rewrite that depends on what is bound or on the back end's abilities is a field here, and the
/// conversion reads nothing else: a converted shader is a function of its source bytes, the converter's
/// version and this key, so equal cache keys name equal modules on every back end.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VariantKey {
    pub flags: VariantFlags,
    /// Textures filtered in the program from loads ([`crate::SlowPath::FilterInShader`]).
    pub filter_in_shader: u128,
    /// Samplers whose border color the program applies ([`crate::SlowPath::BorderInShader`]).
    pub border_in_shader: u16,
    /// Samplers whose mirror-once addressing the program applies
    /// ([`crate::SlowPath::MirrorOnceInShader`]).
    pub mirror_once_in_shader: u16,
    /// Samplers whose level-of-detail bias the program adds ([`crate::SlowPath::LodBiasInShader`]).
    pub lod_bias: u16,
    /// Constant buffers read as storage buffers ([`crate::SlowPath::ConstantsAsStorage`]).
    pub constants_as_storage: u16,
    /// Constant buffers read through a texture ([`crate::SlowPath::ConstantsAsTexture`]).
    pub constants_as_texture: u16,
    /// Vertex inputs fetched in the program, by input register, with the format each is stored in
    /// ([`crate::SlowPath::VertexPulledInShader`], [`crate::SlowPath::InstanceRateInShader`]).
    pub pulled: Vec<(u32, VertexFormat)>,
    /// Formats of storage views the shader does not declare, by storage slot, from the bound view.
    pub storage_formats: Vec<(u32, Format)>,
    /// How typed buffers are read.
    pub texel_buffers: TexelBufferRoute,
    /// Formats of the typed buffer views bound at texture slots, by slot, from the bound view, for the
    /// routes other than [`TexelBufferRoute::Native`].
    pub texel_formats: Vec<(u32, Format)>,
    /// Storage slots packed into one buffer, at offsets from the helper constants
    /// ([`crate::SlowPath::StoragePacked`]).
    pub storage_packed: u64,
    /// Storage texture slots read and written through a buffer of their texels, in the layout
    /// [`HelperValue::TexelLayout`] gives and the bound view's format from `storage_formats`, encoded
    /// and decoded in the program; texels narrower than 32 bits are written by compare-exchange on their
    /// word, so neighbours written at the same time are kept. For atomics
    /// ([`crate::SlowPath::AtomicsThroughBuffer`]) and for formats the back end cannot store into
    /// ([`crate::SlowPath::StorageFormatThroughBuffer`]).
    pub texels_through_buffer: u64,
    /// Texture slots bound to textures stored as the integer format of the same bits, with the guest's
    /// format, which the program decodes and filters from loads ([`crate::SlowPath::NormAsInteger`]).
    pub decoded: Vec<(u32, Format)>,
    /// Color targets written in another stored format, or combined in the program.
    pub targets_in_shader: Vec<TargetInShader>,
    /// Texture slots under a per-resource minimum level-of-detail clamp: sampled through the samplers at
    /// [`CLAMPED_SAMPLER_BINDING`], loaded and gathered by [`HelperValue::MinLod`]
    /// ([`crate::SlowPath::MinLodClamp`]).
    pub min_lod_clamped: u128,
    /// Where the program reads the draw's bases.
    pub draw_base: DrawBaseSource,
    /// How the next stage interpolates the vertex program's float outputs, by location, where not
    /// `Perspective`: the program writes each so, since WebGPU and GLSL ES require the two stages to
    /// agree. The front takes it from the pixel program's [`Reflection::inputs`].
    pub output_interpolation: Vec<(u32, Interpolation)>,
    /// Where the values [`VariantFlags::CLIP_DISTANCE`] and [`VariantFlags::DEPTH_CLAMP`] add pass
    /// from the vertex to the pixel program. The front sets the same value in both programs' keys.
    pub linked: LinkedValues,
    /// The sample count of the targets, where the program depends on it.
    pub samples: u8,
    /// A hash of the binding layout that numbers the slots, for APIs whose layout the program does not
    /// fix alone (a Direct3D 12 root signature); zero otherwise.
    pub layout: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_ranges_follow_each_other_without_overlap() {
        let kinds = [
            SlotKind::Uniform,
            SlotKind::Sampler,
            SlotKind::Texture,
            SlotKind::Storage,
            SlotKind::Counter,
        ];
        for pair in kinds.windows(2) {
            assert_eq!(
                pair[0].first_binding() + pair[0].slots(),
                pair[1].first_binding()
            );
        }
        assert_eq!(SlotKind::Texture.binding(127), Some(159));
        assert_eq!(SlotKind::Texture.binding(128), None);
        assert_eq!(SlotKind::Storage.binding(63), Some(223));
        assert_eq!(SlotKind::Counter.binding(0), Some(224));
    }

    #[test]
    fn sampler_state_codes_keep_their_numbers() {
        use crate::{AddressMode, Filter};
        assert_eq!([Filter::Nearest, Filter::Linear].map(|f| f as u32), [0, 1]);
        let modes = [
            AddressMode::Repeat,
            AddressMode::Mirror,
            AddressMode::Clamp,
            AddressMode::Border,
            AddressMode::MirrorOnce,
        ];
        assert_eq!(modes.map(|m| m as u32), [0, 1, 2, 3, 4]);
    }

    #[test]
    fn helper_blocks_of_the_two_draw_stages_never_share_a_binding() {
        let vertex = helper_binding(ShaderStage::Vertex);
        let fragment = helper_binding(ShaderStage::Fragment);
        assert_ne!(vertex, fragment);
        let targets = TARGET_COPY_BINDING..TARGET_COPY_BINDING + crate::MAX_COLOR_TARGETS as u32;
        let others = [
            EARLY_DEPTH_BINDING,
            EARLY_STENCIL_BINDING,
            DRAW_ARGUMENTS_BINDING,
        ];
        for b in [vertex, fragment] {
            assert!(!targets.contains(&b));
            assert!(!others.contains(&b));
            assert!(b < CLAMPED_SAMPLER_BINDING);
        }
    }
}
