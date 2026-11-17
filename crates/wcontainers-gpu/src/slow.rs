//! Named slow paths. Every place a front, a back end or a shared helper does something the slow way, or
//! refuses a valid request for want of an exact route, names its reason here, bumps its counter and
//! raises a warning the first time.

use alloc::vec::Vec;

/// Why something took the slow way. A variant whose description starts "Refused" names a valid request
/// that failed because no exact route exists; it is counted and warned the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum SlowPath {
    /// A triangle fan was drawn as a generated triangle list.
    FanConverted,
    /// A line loop was drawn as a generated line list.
    LoopConverted,
    /// A read back that the guest needed at once paused the device thread until the GPU finished.
    ReadBackWait,
    /// A state combination was first seen at a draw and built before drawing, pausing the frame.
    PipelineMiss,
    /// An ordered write landed inside a pass, which the back end had to split.
    WriteSplitPass,
    /// A view needed a format, dimension or part the texture cannot be viewed as, so a second texture of
    /// the view's shape was kept and copied into.
    ReinterpretCopy,
    /// Geometry shader run as compute work before the draw.
    GeometryAsCompute,
    /// Geometry shader run by replaying captured vertices once per output vertex (no compute).
    GeometryAsReplay,
    /// Hull and domain shaders and the tessellator run as compute work before the draw.
    TessellationAsCompute,
    /// Stream output written by a compute copy of the vertex work.
    StreamOutAsCompute,
    /// A vertex-stage shader writing storage ran as compute work.
    VertexStorageAsCompute,
    /// Indices rewritten so strips do not restart, or restart at another value.
    RestartRewrite,
    /// The provoking vertex was changed by rewriting the program or the indices.
    ProvokingRewrite,
    /// Depth bias applied in the program: on lines and points (WebGPU allows none), or with a clamp the
    /// back end lacks.
    DepthBiasInShader,
    /// Sampler border color applied in the program.
    BorderInShader,
    /// Wide lines drawn as triangles.
    WideLinesAsTriangles,
    /// Logic operation applied in the program, reading a copy of the target.
    LogicOpInShader,
    /// Polygon fill mode drawn as generated lines or points.
    FillModeConverted,
    /// A typed buffer view read through a storage buffer with the format decoded in the program.
    TexelBufferAsStorage,
    /// More storage bindings than the back end allows, packed into one buffer copied in and out.
    StoragePacked,
    /// A shader of more texture-sampler pairs than WebGL 2 has units: pairs expanded.
    SamplerPairsExpanded,
    /// Clip-space depth from minus one rewritten in the program.
    ClipDepthRewrite,
    /// A vertex format the back end lacks, fetched from a storage buffer or converted on upload.
    VertexFormatConverted,
    /// A texture format the back end lacks, stored in a wider one and converted on upload and read.
    FormatConverted,
    /// A typed buffer view read as a 2D texture (no storage buffers).
    TexelBufferAsTexture,
    /// Base vertex or first instance applied by binding vertex streams at an offset.
    BaseVertexRebind,
    /// Indirect draw arguments read back to the device thread before drawing.
    IndirectReadBack,
    /// A draw count read from a buffer turned into the largest count of draws, unused ones zeroed.
    IndirectCountExpanded,
    /// A persistently mapped range uploaded whole because the written bytes were not known.
    MappedUploadWhole,
    /// Small constants beyond the back end's immediate bytes passed in a uniform buffer.
    ImmediatesSpilled,
    /// Each view of a multiview pass drawn as its own pass.
    MultiviewSerial,
    /// A draw to several viewports split into one draw per viewport.
    MultiViewportSplit,
    /// Points larger than one pixel drawn as quads.
    PointsAsQuads,
    /// 64-bit float arithmetic emulated with 32-bit operations.
    F64Emulated,
    /// A predicated draw waited for its query's answer on the device thread.
    PredicationWait,
    /// Depth or stencil read back by drawing it into a color texture first.
    DepthReadByDraw,
    /// A draw with different stencil values per face drawn once per face.
    StencilFacesSplit,
    /// Mirror-once addressing applied in the program: absolute coordinate, clamped, original gradients.
    MirrorOnceInShader,
    /// A vertex stream stepping every n instances fetched in the program from the instance index.
    InstanceRateInShader,
    /// An exact occlusion count made by drawing the query's draws again into a counting target.
    OcclusionCountReplay,
    /// A timestamp asked for between draws split the pass, since timestamps are written between passes.
    TimestampSplitPass,
    /// The unaligned edge bytes of a buffer write, copy or clear merged on the GPU by a small program.
    UnalignedBufferEdge,
    /// A draw run once per group of targets: its targets hold more bytes per sample than the back end
    /// allows, or differ in blend or write mask where the back end has one blend for all.
    TargetsSplit,
    /// A texture the back end cannot filter, sampled with a filtering sampler, filtered in the program
    /// from loads.
    FilterInShader,
    /// Stream output with gaps or layouts transform feedback cannot write, captured whole and repacked
    /// by a second capture.
    StreamOutRepacked,
    /// A draw that reads a copy of its target in the program (logic operations, blending into a format
    /// stored as integers), drawn one primitive at a time because its primitives may overlap.
    TargetReadPerPrimitive,
    /// A layered draw (layer chosen per primitive) run once per layer used, into that layer.
    LayerSplit,
    /// The part of a draw past the end of a vertex buffer drawn with a zero buffer bound.
    VertexFetchZeroed,
    /// Refused, not slow: a program needs more bindings of one kind than the back end has, and no exact
    /// route exists, so making it failed.
    BindingsBeyondLimit,
    /// Vertex streams or attributes beyond the back end's limits fetched by the program from storage
    /// buffers.
    VertexPulledInShader,
    /// Constant buffers beyond the back end's uniform buffer slots read as read-only storage buffers.
    ConstantsAsStorage,
    /// Constant buffers beyond the back end's uniform blocks or block size, copied into a texture and
    /// read with texel fetches (no storage buffers).
    ConstantsAsTexture,
    /// A block-compressed texture the back end cannot sample, stored decoded, with its blocks kept on the
    /// host for copies and read backs.
    BlockDecoded,
    /// A copy into a texture whose bytes the back end keeps on the host (decoded block-compressed
    /// textures, formats its API cannot read back) from one it does not, read back to the host first.
    HostCopyReadBack,
    /// A sampler's level-of-detail bias added in the program.
    LodBiasInShader,
    /// A viewport whose near depth exceeds its far depth drawn swapped, with clip depth flipped in the
    /// program.
    ViewportDepthFlip,
    /// Refused, not slow: a program passes more values between stages than the back end has, after
    /// packing, so making it failed.
    VaryingsBeyondLimit,
    /// A storage view used with atomics, on a format whose storage views have none, read and written in
    /// the program through a buffer of its texels, copied from and to the texture around other uses.
    AtomicsThroughBuffer,
    /// A 16-bit normalised format, or a rendered 8-bit signed normalised one, the back end lacks, stored
    /// as the integer format of the same bits: decoded and filtered in the program, encoded on writes.
    NormAsInteger,
    /// Blending done in the program, reading a copy of the target: into a target stored as integers, or
    /// of a format the back end cannot blend.
    BlendInShader,
    /// A sample mask other than all ones written by the fragment program.
    SampleMaskInShader,
    /// The stencil aspect sampled through an 8-bit texture filled from it by draws.
    StencilCopiedByDraw,
    /// A copy between two formats the API copies only through a buffer: two copies.
    CopyThroughBuffer,
    /// Depth copied into a texture whose depth cannot be written by a copy, by a draw writing depth.
    DepthWrittenByDraw,
    /// A texture whose format the back end cannot read back or copy on the GPU, kept on the host too.
    HostCopyKept,
    /// Overlapping guest queries split into one back-end query per stretch between their begins and
    /// ends, the results added.
    QueriesSegmented,
}

impl SlowPath {
    /// Every reason, for reading all counters.
    pub const ALL: [SlowPath; 66] = [
        SlowPath::FanConverted,
        SlowPath::LoopConverted,
        SlowPath::ReadBackWait,
        SlowPath::PipelineMiss,
        SlowPath::WriteSplitPass,
        SlowPath::ReinterpretCopy,
        SlowPath::GeometryAsCompute,
        SlowPath::GeometryAsReplay,
        SlowPath::TessellationAsCompute,
        SlowPath::StreamOutAsCompute,
        SlowPath::VertexStorageAsCompute,
        SlowPath::RestartRewrite,
        SlowPath::ProvokingRewrite,
        SlowPath::DepthBiasInShader,
        SlowPath::BorderInShader,
        SlowPath::WideLinesAsTriangles,
        SlowPath::LogicOpInShader,
        SlowPath::FillModeConverted,
        SlowPath::TexelBufferAsStorage,
        SlowPath::StoragePacked,
        SlowPath::SamplerPairsExpanded,
        SlowPath::ClipDepthRewrite,
        SlowPath::VertexFormatConverted,
        SlowPath::FormatConverted,
        SlowPath::TexelBufferAsTexture,
        SlowPath::BaseVertexRebind,
        SlowPath::IndirectReadBack,
        SlowPath::IndirectCountExpanded,
        SlowPath::MappedUploadWhole,
        SlowPath::ImmediatesSpilled,
        SlowPath::MultiviewSerial,
        SlowPath::MultiViewportSplit,
        SlowPath::PointsAsQuads,
        SlowPath::F64Emulated,
        SlowPath::PredicationWait,
        SlowPath::DepthReadByDraw,
        SlowPath::StencilFacesSplit,
        SlowPath::MirrorOnceInShader,
        SlowPath::InstanceRateInShader,
        SlowPath::OcclusionCountReplay,
        SlowPath::TimestampSplitPass,
        SlowPath::UnalignedBufferEdge,
        SlowPath::TargetsSplit,
        SlowPath::FilterInShader,
        SlowPath::StreamOutRepacked,
        SlowPath::TargetReadPerPrimitive,
        SlowPath::LayerSplit,
        SlowPath::VertexFetchZeroed,
        SlowPath::BindingsBeyondLimit,
        SlowPath::VertexPulledInShader,
        SlowPath::ConstantsAsStorage,
        SlowPath::ConstantsAsTexture,
        SlowPath::BlockDecoded,
        SlowPath::HostCopyReadBack,
        SlowPath::LodBiasInShader,
        SlowPath::ViewportDepthFlip,
        SlowPath::VaryingsBeyondLimit,
        SlowPath::AtomicsThroughBuffer,
        SlowPath::NormAsInteger,
        SlowPath::BlendInShader,
        SlowPath::SampleMaskInShader,
        SlowPath::StencilCopiedByDraw,
        SlowPath::CopyThroughBuffer,
        SlowPath::DepthWrittenByDraw,
        SlowPath::HostCopyKept,
        SlowPath::QueriesSegmented,
    ];

    /// The fixed name the host's warning and counter hooks show.
    pub fn reason(self) -> &'static str {
        match self {
            SlowPath::FanConverted => "triangle fan converted to a list",
            SlowPath::LoopConverted => "line loop converted to a list",
            SlowPath::ReadBackWait => "read back waited for the gpu",
            SlowPath::PipelineMiss => "state combination built at first draw",
            SlowPath::WriteSplitPass => "ordered write split a pass",
            SlowPath::ReinterpretCopy => "texture copied to view it another way",
            SlowPath::GeometryAsCompute => "geometry shader run as compute",
            SlowPath::GeometryAsReplay => "geometry shader run by vertex replay",
            SlowPath::TessellationAsCompute => "tessellation run as compute",
            SlowPath::StreamOutAsCompute => "stream output run as compute",
            SlowPath::VertexStorageAsCompute => "vertex stage storage writes run as compute",
            SlowPath::RestartRewrite => "indices rewritten for primitive restart",
            SlowPath::ProvokingRewrite => "provoking vertex rewritten",
            SlowPath::DepthBiasInShader => "depth bias applied in the shader",
            SlowPath::BorderInShader => "sampler border color applied in the shader",
            SlowPath::WideLinesAsTriangles => "wide lines drawn as triangles",
            SlowPath::LogicOpInShader => "logic operation applied in the shader",
            SlowPath::FillModeConverted => "fill mode drawn as generated lines or points",
            SlowPath::TexelBufferAsStorage => "typed buffer read through a storage buffer",
            SlowPath::StoragePacked => "storage bindings packed into one buffer",
            SlowPath::SamplerPairsExpanded => "texture and sampler pairs expanded",
            SlowPath::ClipDepthRewrite => "clip space depth rewritten in the shader",
            SlowPath::VertexFormatConverted => "vertex format converted",
            SlowPath::FormatConverted => "texture format stored in a wider one",
            SlowPath::TexelBufferAsTexture => "typed buffer read as a texture",
            SlowPath::BaseVertexRebind => "vertex streams rebound for base vertex or instance",
            SlowPath::IndirectReadBack => "indirect arguments read back",
            SlowPath::IndirectCountExpanded => "indirect draw count expanded",
            SlowPath::MappedUploadWhole => "mapped range uploaded whole",
            SlowPath::ImmediatesSpilled => "immediate constants spilled to a buffer",
            SlowPath::MultiviewSerial => "multiview drawn one view at a time",
            SlowPath::MultiViewportSplit => "draw split per viewport",
            SlowPath::PointsAsQuads => "large points drawn as quads",
            SlowPath::F64Emulated => "64-bit floats emulated",
            SlowPath::PredicationWait => "predicated draw waited for its query",
            SlowPath::DepthReadByDraw => "depth read back through a draw",
            SlowPath::StencilFacesSplit => "draw split per stencil face",
            SlowPath::MirrorOnceInShader => "mirror once addressing applied in the shader",
            SlowPath::InstanceRateInShader => "instance step rate applied in the shader",
            SlowPath::OcclusionCountReplay => "occlusion count made by replaying draws",
            SlowPath::TimestampSplitPass => "timestamp split a pass",
            SlowPath::UnalignedBufferEdge => "unaligned buffer edge merged by a program",
            SlowPath::TargetsSplit => "draw run once per group of targets",
            SlowPath::FilterInShader => "texture filtered in the shader",
            SlowPath::StreamOutRepacked => "stream output repacked",
            SlowPath::TargetReadPerPrimitive => {
                "draw reading its target run one primitive at a time"
            }
            SlowPath::LayerSplit => "layered draw split per layer",
            SlowPath::VertexFetchZeroed => "vertex fetch past the end zeroed",
            SlowPath::BindingsBeyondLimit => "program refused: more bindings than the back end has",
            SlowPath::VertexPulledInShader => "vertex streams fetched in the shader",
            SlowPath::ConstantsAsStorage => "constant buffers read as storage buffers",
            SlowPath::ConstantsAsTexture => "constant buffers read through a texture",
            SlowPath::BlockDecoded => "block-compressed texture stored decoded",
            SlowPath::HostCopyReadBack => "copy source read back to the host copy",
            SlowPath::LodBiasInShader => "sampler lod bias applied in the shader",
            SlowPath::ViewportDepthFlip => "viewport depth range flipped in the shader",
            SlowPath::VaryingsBeyondLimit => {
                "program refused: more stage values than the back end has"
            }
            SlowPath::AtomicsThroughBuffer => "storage texture atomics run through a buffer",
            SlowPath::NormAsInteger => "normalised format stored as integers",
            SlowPath::BlendInShader => "blending done in the shader",
            SlowPath::SampleMaskInShader => "sample mask written by the shader",
            SlowPath::StencilCopiedByDraw => "stencil copied by draws to be sampled",
            SlowPath::CopyThroughBuffer => "texture copy made through a buffer",
            SlowPath::DepthWrittenByDraw => "depth copy made by a draw",
            SlowPath::HostCopyKept => "texture kept on the host as well",
            SlowPath::QueriesSegmented => "overlapping queries segmented",
        }
    }
}

/// Counters for every slow path, and the warnings not yet handed to the host.
#[derive(Clone, Debug)]
pub struct SlowPaths {
    counts: [u64; SlowPath::ALL.len()],
    unwarned: Vec<SlowPath>,
}

impl Default for SlowPaths {
    fn default() -> Self {
        SlowPaths {
            counts: [0; SlowPath::ALL.len()],
            unwarned: Vec::new(),
        }
    }
}

impl SlowPaths {
    /// Records one use of `path`. The first use queues a warning.
    pub fn hit(&mut self, path: SlowPath) {
        let c = &mut self.counts[path as usize];
        if *c == 0 {
            self.unwarned.push(path);
        }
        *c += 1;
    }

    /// How many times `path` has been taken.
    pub fn count(&self, path: SlowPath) -> u64 {
        self.counts[path as usize]
    }

    /// Takes the warnings raised since the last call, for the host's warning hook.
    pub fn take_warnings(&mut self) -> Vec<SlowPath> {
        core::mem::take(&mut self.unwarned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lists_every_reason_in_order() {
        for (i, p) in SlowPath::ALL.iter().enumerate() {
            assert_eq!(*p as usize, i);
        }
        let mut names: Vec<_> = SlowPath::ALL.iter().map(|p| p.reason()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), SlowPath::ALL.len());
    }
}
