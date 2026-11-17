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
}

impl SlotKind {
    /// The first binding number of the kind's range.
    pub const fn first_binding(self) -> u32 {
        match self {
            SlotKind::Uniform => 0,
            SlotKind::Sampler => 16,
            SlotKind::Texture => 32,
            SlotKind::Storage => 160,
        }
    }

    /// How many slots the range holds.
    pub const fn slots(self) -> u32 {
        match self {
            SlotKind::Uniform | SlotKind::Sampler => 16,
            SlotKind::Texture => 128,
            SlotKind::Storage => 64,
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
    /// A texture's mip level count (u32), where the back end's language cannot ask.
    TextureLevels { texture: u32 },
    /// A multisampled texture's sample count (u32), where the back end's language cannot ask.
    TextureSamples { texture: u32 },
    /// Depth bias added in the program: constant units, slope scale, clamp, and the depth format's
    /// smallest step (four f32).
    DepthBias,
    /// A vertex stream fetched in the program: byte offset of the first element, stride, and instance
    /// step rate (zero for per-vertex), three u32.
    PulledStream { stream: u32 },
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
    }
}

/// Which variant of a converted shader a front asks for. It is part of the converted shader's cache key.
/// Each per-slot field has a bit per API slot of that kind.
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
    }
}
