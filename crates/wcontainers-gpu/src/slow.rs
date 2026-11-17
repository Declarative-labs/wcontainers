//! Named slow paths. Every place a front, a back end or a shared helper does something the slow way
//! names its reason here, bumps its counter and raises a warning the first time.

use alloc::vec::Vec;

/// Why something took the slow way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum SlowPath {
    /// A triangle fan was drawn as a generated triangle list.
    FanConverted,
    /// A line loop was drawn as a generated line strip.
    LoopConverted,
    /// A read back that the guest needed at once paused the device thread until the GPU finished.
    ReadBackWait,
    /// A state combination was first seen at a draw and built before drawing, pausing the frame.
    PipelineMiss,
    /// An ordered write landed inside a pass, which the back end had to split.
    WriteSplitPass,
    /// A view needed a format the texture cannot be viewed as, so the texture was copied.
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
    /// Depth bias on lines or points applied in the program.
    LineBiasInShader,
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
    /// A multisampled texture read in a program by first resolving or copying it.
    MultisampleReadCopy,
}

impl SlowPath {
    /// Every reason, for reading all counters.
    pub const ALL: [SlowPath; 25] = [
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
        SlowPath::LineBiasInShader,
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
        SlowPath::MultisampleReadCopy,
    ];

    /// The fixed name the host's warning and counter hooks show.
    pub fn reason(self) -> &'static str {
        match self {
            SlowPath::FanConverted => "triangle fan converted to a list",
            SlowPath::LoopConverted => "line loop converted to a strip",
            SlowPath::ReadBackWait => "read back waited for the gpu",
            SlowPath::PipelineMiss => "state combination built at first draw",
            SlowPath::WriteSplitPass => "ordered write split a pass",
            SlowPath::ReinterpretCopy => "texture copied to view it as another format",
            SlowPath::GeometryAsCompute => "geometry shader run as compute",
            SlowPath::GeometryAsReplay => "geometry shader run by vertex replay",
            SlowPath::TessellationAsCompute => "tessellation run as compute",
            SlowPath::StreamOutAsCompute => "stream output run as compute",
            SlowPath::VertexStorageAsCompute => "vertex stage storage writes run as compute",
            SlowPath::RestartRewrite => "indices rewritten for primitive restart",
            SlowPath::ProvokingRewrite => "provoking vertex rewritten",
            SlowPath::LineBiasInShader => "depth bias on lines or points applied in the shader",
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
            SlowPath::MultisampleReadCopy => "multisampled texture copied to be read",
        }
    }
}

/// Counters for every slow path, and the warnings not yet handed to the host.
#[derive(Clone, Debug, Default)]
pub struct SlowPaths {
    counts: [u64; SlowPath::ALL.len()],
    unwarned: Vec<SlowPath>,
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
