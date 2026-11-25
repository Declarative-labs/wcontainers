//! Named slow paths. Every place a front, a back end or a shared helper does something the slow way, or
//! refuses a valid request for want of an exact route, names its reason here, bumps its counter and
//! raises a warning the first time.

use alloc::vec::Vec;

/// The reason [`SlowPath::StorageOffsetInShader`] shows.
const STORAGE_OFFSET_IN_SHADER: &str = "storage view's offset added in the shader";

/// The reason [`SlowPath::FloatBitsClearByDraw`] shows.
const FLOAT_BITS_CLEAR_BY_DRAW: &str = "float colour clear made by a draw keeping its bits";

/// The reason [`SlowPath::StreamOutAfterReplay`] shows.
const STREAM_OUT_AFTER_REPLAY: &str = "stream output of the replay route written on the host";

/// The reason [`SlowPath::FrameStretched`] shows.
const FRAME_STRETCHED: &str = "frame stretched to its window against the swap chain's scaling";

/// The reason [`SlowPath::SharedThroughHost`] shows.
const SHARED_THROUGH_HOST: &str = "shared resource copied between devices through the host";

/// The reason [`SlowPath::QuerySetSplitPass`] shows.
const QUERY_SET_SPLIT_PASS: &str = "query set out of indices split a pass";

/// The reason [`SlowPath::PackedTargetQuantised`] shows.
const PACKED_TARGET_QUANTISED: &str = "packed target drawn wider and rounded to its bits by a draw";

/// The reason [`SlowPath::PackedTargetReadByBlend`] shows.
const PACKED_TARGET_READ_BY_BLEND: &str =
    "packed target read by a blend split a pass to round it first";

/// The reason [`SlowPath::StorageReadWriteThroughBuffer`] shows.
const STORAGE_READ_WRITE_THROUGH_BUFFER: &str =
    "storage texture loaded and stored by one program through a buffer of its texels";

/// The reason [`SlowPath::AliasedLinesAsQuads`] shows.
const ALIASED_LINES_AS_QUADS: &str =
    "aliased lines drawn as rectangles keeping the pixels of their rule";

/// Why something took the slow way. A variant whose description starts "Refused" names a valid request
/// that failed because no exact route exists; it is counted and warned the same way. Serialised as its
/// number (the shaders crate's cache), so a new reason goes at the end and none moves.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
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
    /// Depth bias applied in the program: on lines and points (WebGPU allows none), with a clamp the
    /// back end lacks, or into 24-bit depth the back end stores as float.
    DepthBiasInShader,
    /// Sampler border color applied in the program.
    BorderInShader,
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
    /// A predicated operation waited for its predicate's value on the device thread.
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
    /// Blending done in the program, reading a copy of the target: into a target stored as integers, of
    /// a format the back end cannot blend, or with dual-source factors the back end lacks.
    BlendInShader,
    /// A sample mask other than all ones written by the fragment program.
    SampleMaskInShader,
    /// The stencil aspect sampled through an 8-bit texture filled from it by draws.
    StencilCopiedByDraw,
    /// A copy between two formats the API copies only through a buffer: two copies.
    CopyThroughBuffer,
    /// Depth copied into a texture whose depth cannot be written by a copy (from a texture, or from a
    /// buffer through an `r32float` or `r32uint` texture), by a draw writing depth.
    DepthWrittenByDraw,
    /// A texture whose format the back end cannot read back or copy on the GPU, kept on the host too.
    HostCopyKept,
    /// Overlapping guest queries split into one back-end query per stretch between their begins and
    /// ends, the results added.
    QueriesSegmented,
    /// A copy between non-overlapping ranges of one buffer made through a temporary buffer.
    CopyWithinBuffer,
    /// A pixel shader declaring early depth and stencil tests, with side effects, drawn with the depth
    /// and stencil read in the program from the read-only attachment and tested before the side effects.
    EarlyTestsInShader,
    /// As [`SlowPath::EarlyTestsInShader`] for a draw that also writes depth or stencil: each primitive
    /// drawn with the test in the program, then drawn again writing only depth and stencil.
    EarlyTestsPerPrimitive,
    /// A per-resource minimum level-of-detail clamp applied by merged samplers and by the program.
    MinLodClamp,
    /// A fractional viewport, or one past [`crate::Viewport`]'s bounds, drawn with a whole-pixel
    /// viewport within them that covers what it draws, adjusted in the program.
    ViewportInShader,
    /// A staging resource the GPU wrote, mapped for writing only, read back first so the bytes the
    /// program leaves alone keep the GPU's values.
    StagingRefreshed,
    /// Attachments of different sizes drawn through temporaries of one size, copied in and out, with
    /// every draw scissored to their common area.
    AttachmentsFitted,
    /// A buffer used both for indices and for other data kept as two buffers, since WebGL 2 lets no
    /// buffer hold both, every host write made to both.
    IndexBufferTwin,
    /// The index twin of a buffer the GPU wrote refreshed through a read back before drawing with it.
    IndexTwinReadBack,
    /// Query results written into a buffer through a read back, pausing the device thread.
    QueryResolveWait,
    /// Predicated draws and dispatches recorded as indirect ones, whose arguments a small compute
    /// program zeroes where the predicate says skip.
    PredicationIndirect,
    /// 8-bit indices widened to 16 bits.
    IndexWidened,
    /// A draw with an adjacency topology and no geometry shader drawn from indices without the
    /// adjacent vertices.
    AdjacencyDropped,
    /// Texture level and sample counts passed to the program in the helper constants.
    TextureInfoInHelpers,
    /// A program reading the API's vertex or instance number, or the draw's bases, given the draw's
    /// base vertex, first instance and draw number in the helper constants.
    DrawBaseInHelpers,
    /// As [`SlowPath::DrawBaseInHelpers`] for an indirect draw: the program reads the bases from the
    /// argument buffer, bound as read-only storage.
    DrawBaseFromArguments,
    /// A storage view of a format the back end cannot store into, read and written in the program
    /// through a buffer of its texels, copied from and to the texture around other uses.
    StorageFormatThroughBuffer,
    /// A copy between multisampled textures the back end cannot copy, made by drawing the source's
    /// resolved value into every sample of the destination (a listed loss).
    MultisampleCopyResolved,
    /// Bytes a placed resource shares with another one written since, copied into it before its use.
    AliasCopied,
    /// Alpha-antialiased lines drawn as rectangles whose coverage the program multiplies into alpha.
    AntialiasedLinesAsQuads,
    /// A pass ended and resumed with `Load` because a draw used a resource another way than an earlier
    /// draw of the pass wrote it as storage (WebGPU makes one pass one usage scope).
    UsageScopeSplit,
    /// A buffer one draw uses both as writable storage and another way: the other use reads a copy made
    /// before the pass.
    UsageScopeCopied,
    /// Clip distances applied by the program: interpolated as ordinary perspective-correct values, the
    /// fragment discarded or its samples masked where one is negative.
    ClipDistanceInShader,
    /// Depth clamped instead of clipped by the program, which writes the fragment's depth.
    DepthClampInShader,
    /// Cull distances applied by the stage helper, which drops each primitive whose vertices are all
    /// negative on one plane.
    CullDistanceAsGeometry,
    /// The pixel shader's primitive index made by the stage helper, as for a geometry shader.
    PrimitiveIdAsGeometry,
    /// A copy between a buffer and a texture whose row pitch the back end's API does not take, made
    /// through a buffer at a pitch it takes.
    RowsRepacked,
    /// A resolve asked in the sRGB twin of the stored format averaged in the stored format, because the
    /// back end cannot read its multisampled samples another way (a listed loss).
    ResolveInStoredFormat,
    /// A resolve made by a draw loading and averaging every sample, where the API's own resolve does
    /// not average in the format named (Chrome's WebGPU averages the stored values of an sRGB twin).
    ResolveByDraw,
    /// A buffer range filled with a non-zero pattern by copies from a block of the pattern, where the
    /// API clears buffers only to zero.
    PatternClearCopied,
    /// A constant buffer smaller than the size its shader declares (Direct3D reads past its end as
    /// zero), moved into a buffer of the declared size whose bytes past its own are zero.
    ConstantBufferGrown,
    /// An integer colour clear made by a draw writing the value, where the API clears colour targets
    /// through a float that cannot hold it.
    IntegerClearByDraw,
    /// Stencil copied into a texture whose stencil the API copies only whole (part of it, or from a
    /// buffer through an `r8uint` texture), by one draw per bit writing that bit.
    StencilWrittenByDraw,
    /// A surface format the window lacks (`rgba8unorm` on a Metal layer) drawn into a texture of
    /// that format, then into the window's own format by one draw at each present.
    PresentConverted,
    /// A captured draw the API's transform feedback cannot take (strips, fans, loops, indices) captured
    /// as points in list order, one draw per run of consecutive vertices, with indices read back first.
    CaptureExpanded,
    /// A copy between regions of one subresource (as Vulkan and OpenGL allow) made through a
    /// temporary buffer (or, for depth and stencil, a temporary texture), where the API copies only
    /// between different subresources.
    CopyWithinTexture,
    /// Pixel shader invocations counted by drawing each draw again, with the depth and stencil tests
    /// off, through a program adding to a counter, where the back end has no statistics queries.
    PixelInvocationsCounted,
    /// A view's component mapping applied by the program to what it reads, where the back end's views
    /// have none.
    SwizzleInShader,
    /// An atomic minimum or maximum of the other signedness than the memory's other operations, made
    /// by a compare-exchange loop.
    AtomicExchangeLoop,
    /// A blit swap chain's buffers past the first, which DXGI moves one place at each present,
    /// moved by copies.
    SequentialBuffersCopied,
    /// An indirect draw without indices fetching per-vertex streams, whose vertex count only the GPU
    /// knows: one reading past a stream's end is skipped whole by the back end, where the API reads
    /// zeros (a listed loss; counted at each such draw, as any of them may).
    IndirectFetchPastEnd,
    /// A storage buffer bound from below its view's start, with the rest added to every address in
    /// the program (`VariantFlags::STORAGE_OFFSET`).
    StorageOffsetInShader,
    /// A 32-bit float colour clear whose bits the API's clear value would change (denormals flushed,
    /// NaN payloads quieted), made by a draw writing the bits.
    FloatBitsClearByDraw,
    /// Stream output on the replay route, written on the host from a read back of what was
    /// captured: after a geometry shader run by replay, which captures every output slot, cut ones
    /// included, and only a count across invocations, which a vertex program cannot make, drops
    /// them; or from a vertex shader alone, whose declared layout, overflow and counts transform
    /// feedback does not give.
    StreamOutAfterReplay,
    /// A frame shown stretched to a window of another size than its buffers, where the swap chain
    /// asked for no stretching (a listed loss: the host's window scales whatever it shows).
    FrameStretched,
    /// A resource shared between devices that hold no common GPU memory, kept as one copy per
    /// device and copied through a read back at the API's synchronisation points.
    SharedThroughHost,
    /// A pass whose query set had no index left for the next stretch, or that began without a set,
    /// ended, and the next opened with a fresh set.
    QuerySetSplitPass,
    /// A target of a 16-bit packed format (`B5G6R5`, `B5G5R5A1`) held in a wider one,
    /// drawn, blended and resolved at the wider precision, and each channel rounded to its own bits
    /// by a draw when the pass ends.
    PackedTargetQuantised,
    /// A blend reading a packed target ([`SlowPath::PackedTargetQuantised`]) that earlier draws
    /// in the pass wrote: the pass ended there so the blend reads values the format holds.
    PackedTargetReadByBlend,
    /// A storage texture a program loads from and stores into, of a format the back end does not
    /// load and store in one program (WebGPU: all but `r32float`, `r32uint` and `r32sint`), read
    /// and written through a buffer of its texels copied around each use.
    StorageReadWriteThroughBuffer,
    /// Aliased lines into a multisampled target drawn as rectangles whose program keeps the pixels
    /// Direct3D's line rule picks (every sample of each), where the back end draws lines there by
    /// its own multisampled rule.
    AliasedLinesAsQuads,
}

/// How many reasons there are.
pub const SLOW_PATH_COUNT: usize = SlowPath::AliasedLinesAsQuads as usize + 1;

/// Every reason, in declaration order, for reading all counters: `SlowPath as usize` indexes it.
pub const SLOW_PATHS: [SlowPath; SLOW_PATH_COUNT] = [
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
    SlowPath::CopyWithinBuffer,
    SlowPath::EarlyTestsInShader,
    SlowPath::EarlyTestsPerPrimitive,
    SlowPath::MinLodClamp,
    SlowPath::ViewportInShader,
    SlowPath::StagingRefreshed,
    SlowPath::AttachmentsFitted,
    SlowPath::IndexBufferTwin,
    SlowPath::IndexTwinReadBack,
    SlowPath::QueryResolveWait,
    SlowPath::PredicationIndirect,
    SlowPath::IndexWidened,
    SlowPath::AdjacencyDropped,
    SlowPath::TextureInfoInHelpers,
    SlowPath::DrawBaseInHelpers,
    SlowPath::DrawBaseFromArguments,
    SlowPath::StorageFormatThroughBuffer,
    SlowPath::MultisampleCopyResolved,
    SlowPath::AliasCopied,
    SlowPath::AntialiasedLinesAsQuads,
    SlowPath::UsageScopeSplit,
    SlowPath::UsageScopeCopied,
    SlowPath::ClipDistanceInShader,
    SlowPath::DepthClampInShader,
    SlowPath::CullDistanceAsGeometry,
    SlowPath::PrimitiveIdAsGeometry,
    SlowPath::RowsRepacked,
    SlowPath::ResolveInStoredFormat,
    SlowPath::ResolveByDraw,
    SlowPath::PatternClearCopied,
    SlowPath::ConstantBufferGrown,
    SlowPath::IntegerClearByDraw,
    SlowPath::StencilWrittenByDraw,
    SlowPath::PresentConverted,
    SlowPath::CaptureExpanded,
    SlowPath::CopyWithinTexture,
    SlowPath::PixelInvocationsCounted,
    SlowPath::SwizzleInShader,
    SlowPath::AtomicExchangeLoop,
    SlowPath::SequentialBuffersCopied,
    SlowPath::IndirectFetchPastEnd,
    SlowPath::StorageOffsetInShader,
    SlowPath::FloatBitsClearByDraw,
    SlowPath::StreamOutAfterReplay,
    SlowPath::FrameStretched,
    SlowPath::SharedThroughHost,
    SlowPath::QuerySetSplitPass,
    SlowPath::PackedTargetQuantised,
    SlowPath::PackedTargetReadByBlend,
    SlowPath::StorageReadWriteThroughBuffer,
    SlowPath::AliasedLinesAsQuads,
];

/// The fixed name the host's warning and counter hooks show for each reason, indexed by
/// `SlowPath as usize`.
pub const SLOW_PATH_REASONS: [&str; SLOW_PATH_COUNT] = [
    // FanConverted
    "triangle fan converted to a list",
    // LoopConverted
    "line loop converted to a list",
    // ReadBackWait
    "read back waited for the gpu",
    // PipelineMiss
    "state combination built at first draw",
    // WriteSplitPass
    "ordered write split a pass",
    // ReinterpretCopy
    "texture copied to view it another way",
    // GeometryAsCompute
    "geometry shader run as compute",
    // GeometryAsReplay
    "geometry shader run by vertex replay",
    // TessellationAsCompute
    "tessellation run as compute",
    // StreamOutAsCompute
    "stream output run as compute",
    // VertexStorageAsCompute
    "vertex stage storage writes run as compute",
    // RestartRewrite
    "indices rewritten for primitive restart",
    // ProvokingRewrite
    "provoking vertex rewritten",
    // DepthBiasInShader
    "depth bias applied in the shader",
    // BorderInShader
    "sampler border color applied in the shader",
    // LogicOpInShader
    "logic operation applied in the shader",
    // FillModeConverted
    "fill mode drawn as generated lines or points",
    // TexelBufferAsStorage
    "typed buffer read through a storage buffer",
    // StoragePacked
    "storage bindings packed into one buffer",
    // SamplerPairsExpanded
    "texture and sampler pairs expanded",
    // ClipDepthRewrite
    "clip space depth rewritten in the shader",
    // VertexFormatConverted
    "vertex format converted",
    // FormatConverted
    "texture format stored in a wider one",
    // TexelBufferAsTexture
    "typed buffer read as a texture",
    // BaseVertexRebind
    "vertex streams rebound for base vertex or instance",
    // IndirectReadBack
    "indirect arguments read back",
    // IndirectCountExpanded
    "indirect draw count expanded",
    // MappedUploadWhole
    "mapped range uploaded whole",
    // ImmediatesSpilled
    "immediate constants spilled to a buffer",
    // MultiviewSerial
    "multiview drawn one view at a time",
    // MultiViewportSplit
    "draw split per viewport",
    // PointsAsQuads
    "large points drawn as quads",
    // F64Emulated
    "64-bit floats emulated",
    // PredicationWait
    "predicated operation waited for its predicate",
    // DepthReadByDraw
    "depth read back through a draw",
    // StencilFacesSplit
    "draw split per stencil face",
    // MirrorOnceInShader
    "mirror once addressing applied in the shader",
    // InstanceRateInShader
    "instance step rate applied in the shader",
    // OcclusionCountReplay
    "occlusion count made by replaying draws",
    // TimestampSplitPass
    "timestamp split a pass",
    // UnalignedBufferEdge
    "unaligned buffer edge merged by a program",
    // TargetsSplit
    "draw run once per group of targets",
    // FilterInShader
    "texture filtered in the shader",
    // StreamOutRepacked
    "stream output repacked",
    // TargetReadPerPrimitive
    { "draw reading its target run one primitive at a time" },
    // LayerSplit
    "layered draw split per layer",
    // VertexFetchZeroed
    "vertex fetch past the end zeroed",
    // BindingsBeyondLimit
    "program refused: more bindings than the back end has",
    // VertexPulledInShader
    "vertex streams fetched in the shader",
    // ConstantsAsStorage
    "constant buffers read as storage buffers",
    // ConstantsAsTexture
    "constant buffers read through a texture",
    // BlockDecoded
    "block-compressed texture stored decoded",
    // HostCopyReadBack
    "copy source read back to the host copy",
    // LodBiasInShader
    "sampler lod bias applied in the shader",
    // ViewportDepthFlip
    "viewport depth range flipped in the shader",
    // VaryingsBeyondLimit
    { "program refused: more stage values than the back end has" },
    // AtomicsThroughBuffer
    "storage texture atomics run through a buffer",
    // NormAsInteger
    "normalised format stored as integers",
    // BlendInShader
    "blending done in the shader",
    // SampleMaskInShader
    "sample mask written by the shader",
    // StencilCopiedByDraw
    "stencil copied by draws to be sampled",
    // CopyThroughBuffer
    "texture copy made through a buffer",
    // DepthWrittenByDraw
    "depth copy made by a draw",
    // HostCopyKept
    "texture kept on the host as well",
    // QueriesSegmented
    "overlapping queries segmented",
    // CopyWithinBuffer
    "copy within one buffer made through another",
    // EarlyTestsInShader
    "early depth and stencil tests run in the shader",
    // EarlyTestsPerPrimitive
    { "early depth and stencil tests run one primitive at a time" },
    // MinLodClamp
    { "resource minimum lod clamp applied by samplers and the shader" },
    // ViewportInShader
    "fractional or oversized viewport applied in the shader",
    // StagingRefreshed
    "staging resource read back before a write-only map",
    // AttachmentsFitted
    { "attachments of different sizes fitted through temporaries" },
    // IndexBufferTwin
    "buffer kept twice for indices and other data",
    // IndexTwinReadBack
    "index twin refreshed through a read back",
    // QueryResolveWait
    "query results resolved through a read back",
    // PredicationIndirect
    "predicated draws recorded as indirect draws",
    // IndexWidened
    "8-bit indices widened",
    // AdjacencyDropped
    "adjacent vertices dropped from a draw",
    // TextureInfoInHelpers
    "texture level counts passed in helper constants",
    // DrawBaseInHelpers
    "draw bases passed in helper constants",
    // DrawBaseFromArguments
    "draw bases read from indirect arguments",
    // StorageFormatThroughBuffer
    { "storage texture of an unstorable format run through a buffer" },
    // MultisampleCopyResolved
    "multisampled copy made from the resolved source",
    // AliasCopied
    "aliased resource bytes copied before use",
    // AntialiasedLinesAsQuads
    "antialiased lines drawn as covered rectangles",
    // UsageScopeSplit
    "pass split between a storage write and another use",
    // UsageScopeCopied
    "buffer used two ways in one draw read from a copy",
    // ClipDistanceInShader
    "clip distances applied in the shader",
    // DepthClampInShader
    "depth clamped in the shader",
    // CullDistanceAsGeometry
    "cull distances applied by the stage helper",
    // PrimitiveIdAsGeometry
    "primitive index made by the stage helper",
    // RowsRepacked
    "copy rows repacked to the api's pitch",
    // ResolveInStoredFormat
    "resolve averaged in the stored format",
    // ResolveByDraw
    "resolve averaged by a draw",
    // PatternClearCopied
    "buffer filled with a pattern by copies",
    // ConstantBufferGrown
    "constant buffer grown to its shader's size",
    // IntegerClearByDraw
    "integer colour clear made by a draw",
    // StencilWrittenByDraw
    "stencil written by a draw per bit",
    // PresentConverted
    "surface format converted by a draw at present",
    // CaptureExpanded
    "captured draw expanded into points in list order",
    // CopyWithinTexture
    { "copy within one subresource made through a temporary buffer or texture" },
    // PixelInvocationsCounted
    { "pixel shader invocations counted by drawing again" },
    // SwizzleInShader
    "view component mapping applied in the program",
    // AtomicExchangeLoop
    { "atomic minimum or maximum made by a compare-exchange loop" },
    // SequentialBuffersCopied
    "swap chain buffers moved by copies at present",
    // IndirectFetchPastEnd
    { "indirect draw skipped whole if it fetches past a vertex buffer's end" },
    // StorageOffsetInShader
    STORAGE_OFFSET_IN_SHADER,
    // FloatBitsClearByDraw
    FLOAT_BITS_CLEAR_BY_DRAW,
    // StreamOutAfterReplay
    STREAM_OUT_AFTER_REPLAY,
    // FrameStretched
    FRAME_STRETCHED,
    // SharedThroughHost
    SHARED_THROUGH_HOST,
    // QuerySetSplitPass
    QUERY_SET_SPLIT_PASS,
    // PackedTargetQuantised
    PACKED_TARGET_QUANTISED,
    // PackedTargetReadByBlend
    PACKED_TARGET_READ_BY_BLEND,
    // StorageReadWriteThroughBuffer
    STORAGE_READ_WRITE_THROUGH_BUFFER,
    // AliasedLinesAsQuads
    ALIASED_LINES_AS_QUADS,
];

/// Counters for every slow path, and the warnings not yet handed to the host.
#[derive(Clone, Debug)]
pub struct SlowPaths {
    counts: [u64; SLOW_PATH_COUNT],
    unwarned: Vec<SlowPath>,
}

impl Default for SlowPaths {
    fn default() -> Self {
        SlowPaths {
            counts: [0; SLOW_PATH_COUNT],
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
        for (i, p) in SLOW_PATHS.iter().enumerate() {
            assert_eq!(*p as usize, i);
        }
        let mut names: Vec<_> = SLOW_PATH_REASONS.to_vec();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), SLOW_PATH_COUNT);
    }

    /// Every reason keeps the number it had when it was added, the place serialised caches read.
    #[test]
    fn reasons_keep_their_numbers() {
        const HELD: &str = "\
            FanConverted LoopConverted ReadBackWait PipelineMiss WriteSplitPass ReinterpretCopy \
            GeometryAsCompute GeometryAsReplay TessellationAsCompute StreamOutAsCompute \
            VertexStorageAsCompute RestartRewrite ProvokingRewrite DepthBiasInShader \
            BorderInShader LogicOpInShader FillModeConverted TexelBufferAsStorage StoragePacked \
            SamplerPairsExpanded ClipDepthRewrite VertexFormatConverted FormatConverted \
            TexelBufferAsTexture BaseVertexRebind IndirectReadBack IndirectCountExpanded \
            MappedUploadWhole ImmediatesSpilled MultiviewSerial MultiViewportSplit PointsAsQuads \
            F64Emulated PredicationWait DepthReadByDraw StencilFacesSplit MirrorOnceInShader \
            InstanceRateInShader OcclusionCountReplay TimestampSplitPass UnalignedBufferEdge \
            TargetsSplit FilterInShader StreamOutRepacked TargetReadPerPrimitive LayerSplit \
            VertexFetchZeroed BindingsBeyondLimit VertexPulledInShader ConstantsAsStorage \
            ConstantsAsTexture BlockDecoded HostCopyReadBack LodBiasInShader ViewportDepthFlip \
            VaryingsBeyondLimit AtomicsThroughBuffer NormAsInteger BlendInShader \
            SampleMaskInShader StencilCopiedByDraw CopyThroughBuffer DepthWrittenByDraw \
            HostCopyKept QueriesSegmented CopyWithinBuffer EarlyTestsInShader \
            EarlyTestsPerPrimitive MinLodClamp ViewportInShader StagingRefreshed \
            AttachmentsFitted IndexBufferTwin IndexTwinReadBack QueryResolveWait \
            PredicationIndirect IndexWidened AdjacencyDropped TextureInfoInHelpers \
            DrawBaseInHelpers DrawBaseFromArguments StorageFormatThroughBuffer \
            MultisampleCopyResolved AliasCopied AntialiasedLinesAsQuads UsageScopeSplit \
            UsageScopeCopied ClipDistanceInShader DepthClampInShader CullDistanceAsGeometry \
            PrimitiveIdAsGeometry RowsRepacked ResolveInStoredFormat ResolveByDraw \
            PatternClearCopied ConstantBufferGrown IntegerClearByDraw StencilWrittenByDraw \
            PresentConverted CaptureExpanded CopyWithinTexture PixelInvocationsCounted \
            SwizzleInShader AtomicExchangeLoop SequentialBuffersCopied IndirectFetchPastEnd \
            StorageOffsetInShader FloatBitsClearByDraw StreamOutAfterReplay FrameStretched \
            SharedThroughHost QuerySetSplitPass PackedTargetQuantised PackedTargetReadByBlend \
            StorageReadWriteThroughBuffer AliasedLinesAsQuads";
        let held: Vec<_> = HELD.split_whitespace().collect();
        assert!(held.len() <= SLOW_PATH_COUNT);
        for (i, name) in held.iter().enumerate() {
            assert_eq!(alloc::format!("{:?}", SLOW_PATHS[i]), *name, "reason {i}");
        }
    }
}
