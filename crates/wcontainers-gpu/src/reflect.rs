//! What a converted shader binds, reads and writes, and which variant of it a front asks for.
//!
//! This is the contract between `wcontainers-shaders`, which converts an API's shader into the common
//! form and fills a [`Reflection`], and the fronts, which bind resources and write helper constants by
//! it. Back ends never read it: they see the program's module and the binding sets only.

use crate::format::Format;
use crate::state::VertexFormat;
use alloc::string::String;
use alloc::vec::Vec;
use bitflags::bitflags;

/// The kind of API slot a resource comes from. Each kind owns a fixed range of binding numbers in every
/// binding set, so a slot's binding number never depends on what else the program uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// The binding set that holds a converted program's helper constants, as one uniform buffer at
/// [`HELPER_BINDING`].
pub const HELPER_SET: u32 = 3;
/// The binding of the helper constants' uniform buffer in [`HELPER_SET`].
pub const HELPER_BINDING: u32 = 0;
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

/// The API stage a shader was written for. Geometry, hull and domain shaders become compute and vertex
/// programs in the shaders crate's stage helper; the others become one program stage each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceStage {
    Vertex,
    Hull,
    Domain,
    Geometry,
    Pixel,
    Compute,
}

/// One resource the shader uses, where the API names it and where the converted program binds it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
}

/// A value the API's pipeline gives a stage, or takes from it, by meaning rather than by location.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SystemValue {
    Position,
    VertexId,
    InstanceId,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScalarKind {
    Float,
    Sint,
    Uint,
}

/// How a value is interpolated between vertices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    },
    Domain {
        /// Triangle (0), quad (1) or isoline (2) domain.
        domain: u8,
        input_control_points: u8,
    },
}

/// A value the front writes into the program's helper constants before a draw or dispatch, at the byte
/// offset the reflection gives. Each exists only in programs whose variant needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HelperValue {
    /// The part of a storage binding's offset below [`crate::Limits::min_storage_offset_alignment`],
    /// in bytes (u32), which the program adds to its addresses.
    StorageOffset { set: u32, binding: u32 },
    /// A sampler's level-of-detail bias (f32), added to every sampling through it.
    LodBias { sampler: u32 },
    /// A sampler's border color (four f32, or four u32 for integer textures).
    BorderColor { sampler: u32 },
    /// A sampler's state, for programs that filter or address in the program: min, mag and mip filter
    /// and the three address modes as u32 codes, anisotropy (u32), then the level range (two f32).
    SamplerState { sampler: u32 },
    /// The bound view's mip level count (u32), in programs made with [`VariantFlags::TEXTURE_INFO`].
    TextureLevels { texture: u32 },
    /// The bound texture's sample count (u32, one for a texture that is not multisampled), in programs
    /// made with [`VariantFlags::TEXTURE_INFO`].
    TextureSamples { texture: u32 },
    /// Depth bias added in the program: constant units, slope scale, clamp, and the depth format's
    /// smallest step (four f32).
    DepthBias,
    /// A vertex stream fetched in the program: byte offset of the first element, stride, and instance
    /// step rate (zero for per-vertex), three u32.
    PulledStream { stream: u32 },
    /// The sample mask the program writes (u32).
    SampleMask,
    /// Where a packed storage slot's range starts in the packed buffer, in bytes (u32).
    PackedStorage { slot: u32 },
    /// The texel layout of a storage view read through a buffer: row pitch in texels and rows per
    /// layer, two u32.
    TexelLayout { slot: u32 },
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
    /// The fractional viewport within the whole-pixel one the back end draws with: scale and offset of
    /// clip x and y (four f32), applied to the last vertex stage's position.
    ViewportAdjust,
}

/// Where one helper value sits in the helper constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HelperSlot {
    pub value: HelperValue,
    pub offset: u32,
}

/// Everything a front needs to know of a converted shader.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Reflection {
    pub stage: SourceStage,
    pub inputs: Vec<StageValue>,
    pub outputs: Vec<StageValue>,
    pub bindings: Vec<ReflectedBinding>,
    /// The helper constants the program reads, and the size of their block in bytes (zero for none).
    pub helpers: Vec<HelperSlot>,
    pub helper_size: u32,
    /// The (texture slot, sampler slot) pairs the shader samples with, in a fixed order.
    pub sampled_pairs: Vec<(u32, u32)>,
    pub extras: StageExtras,
}

bitflags! {
    /// Whole-program rewrites a variant applies.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
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
    }
}

/// How a program reads typed buffers (Direct3D's `Buffer` and `RWBuffer`, OpenGL's texture buffers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TexelBufferRoute {
    /// As texel buffers, where the back end's language has them.
    #[default]
    Native,
    /// From storage buffers, the format decoded in the program
    /// ([`crate::SlowPath::TexelBufferAsStorage`]).
    Storage,
    /// From a 2D texture the front copies the buffer into ([`crate::SlowPath::TexelBufferAsTexture`]).
    Texture,
}

/// A color target the program writes itself: encoded into the guest's format where the texture is
/// stored in another, and combined with its old value where the back end cannot blend it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// Which variant of a converted shader a front asks for. It is part of the converted shader's cache key.
/// Each per-slot field has a bit per API slot of that kind.
///
/// Every rewrite that depends on what is bound or on the back end's abilities is a field here, and the
/// conversion reads nothing else: a converted shader is a function of its source bytes, the converter's
/// version and this key, so equal cache keys name equal modules on every back end.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
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
    /// Storage slots packed into one buffer, at offsets from the helper constants
    /// ([`crate::SlowPath::StoragePacked`]).
    pub storage_packed: u64,
    /// Storage texture slots read and written through a buffer of their texels, for atomics
    /// ([`crate::SlowPath::AtomicsThroughBuffer`]).
    pub atomics_through_buffer: u64,
    /// Texture slots bound to textures stored as the integer format of the same bits, with the guest's
    /// format, which the program decodes and filters from loads ([`crate::SlowPath::NormAsInteger`]).
    pub decoded: Vec<(u32, Format)>,
    /// Color targets written in another stored format, or combined in the program.
    pub targets_in_shader: Vec<TargetInShader>,
    /// Texture slots under a per-resource minimum level-of-detail clamp: sampled through the samplers at
    /// [`CLAMPED_SAMPLER_BINDING`], loaded and gathered by [`HelperValue::MinLod`]
    /// ([`crate::SlowPath::MinLodClamp`]).
    pub min_lod_clamped: u128,
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
}
