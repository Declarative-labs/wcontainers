//! The occlusion counter (plan 4.12): exact sample counts for the APIs that ask for them, on back ends
//! whose queries answer only "any" ([`SlowPath::OcclusionCountReplay`]). The query's draws run
//! normally and are kept; before anything changes what they read or drew on, and at the query's end,
//! they are drawn again into a counting target cleared to zero, each with its own depth-stencil and
//! raster blocks, blending One plus One and a counting program from the front (the draw's fragment
//! shader writing 1.0 at location 0, its storage writes removed), so each sample holds how many
//! fragments passed every test there. The targets' sums add up to the count: summed on the GPU into
//! a buffer of the counter's where the back end computes and no pixel can reach the counting format's
//! exact bound, otherwise read back and summed here.

use crate::Gpu;
use crate::MAX_BINDING_SETS;
use crate::draw::{ScissorRect, Viewport};
use crate::format::Format;
use crate::passes::PassTargets;
use crate::state::{IndexFormat, Topology};
use alloc::vec::Vec;

/// What a kept draw covers.
#[derive(Debug)]
pub enum KeptRange<B: Gpu> {
    Vertices {
        first: u32,
        count: u32,
        first_instance: u32,
        instances: u32,
    },
    Indexed {
        first: u32,
        count: u32,
        base_vertex: i32,
        first_instance: u32,
        instances: u32,
    },
    Indirect(B::Buffer, u64, bool),
}

/// One draw of a counted query, kept to be drawn again with its counting program.
#[derive(Debug)]
pub struct KeptDraw<B: Gpu> {
    pub program: B::Program,
    pub alpha_to_coverage: bool,
    pub depth_stencil: B::DepthStencil,
    pub raster: B::Raster,
    pub vertex_layout: B::VertexLayout,
    pub bindings: [Option<B::BindingSet>; MAX_BINDING_SETS],
    pub vertex_buffers: Vec<Option<(B::Buffer, u64)>>,
    pub index: Option<(B::Buffer, u64, IndexFormat)>,
    pub topology: Topology,
    pub restart: bool,
    pub viewports: Vec<Viewport>,
    pub scissors: Vec<ScissorRect>,
    pub stencil_reference: [u8; 2],
    pub immediates: Vec<u8>,
    pub range: KeptRange<B>,
    pub instances_step_by_one: bool,
}

/// What the occlusion counter needs of a counted draw that its call does not say; the front, which
/// made the draw's blocks, knows it.
#[derive(Debug)]
pub struct CountedDraw<'a, B: Gpu> {
    /// The draw's counting program.
    pub counting: &'a B::Program,
    /// The blend block's alpha to coverage.
    pub alpha_to_coverage: bool,
    /// The depth-stencil block writes depth or stencil. The first such draw since the last count
    /// copies the depth-stencil target first, so the target must have [`TextureUsage::COPY_SRC`].
    pub writes_depth_stencil: bool,
    /// Every per-instance stream of the vertex layout steps by one instance
    /// ([`crate::state::StepMode::Instance`] of 1), so the draw's instances can be counted in parts by
    /// raising the first instance.
    pub instances_step_by_one: bool,
}

/// A pooled texture and its view, by size, format and sample count.
#[derive(Debug)]
pub struct PooledTexture<B: Gpu> {
    pub key: (u32, u32, Format, u32),
    pub texture: B::Texture,
    pub view: B::TextureView,
}

#[derive(Debug)]
pub struct OcclusionCounter<B: Gpu> {
    /// The draws kept since the last count.
    pub draws: Vec<KeptDraw<B>>,
    pub running: bool,
    /// What the running query has counted so far.
    pub sum: u64,
    /// The targets the kept draws were drawn on, and the pass tracker's generation then.
    pub segment: Option<(PassTargets<B>, u64)>,
    /// The depth-stencil target as it was before the first kept draw that writes it; the count draws
    /// on it in its place.
    pub depth_copy: Option<PooledTexture<B>>,
    pub copied: bool,
    /// The copy as it was before a count that may have to be done again in halves.
    pub saved: Option<PooledTexture<B>>,
    pub targets: Vec<PooledTexture<B>>,
    /// The One plus One blocks, without and with alpha to coverage.
    pub blends: [Option<B::Blend>; 2],
    /// The summing programs for single-sampled and multisampled counting targets, and the buffer
    /// they add the running query's count to (a u64), made at the first sum on the GPU.
    pub summing: [Option<B::Program>; 2],
    pub sum_buffer: Option<B::Buffer>,
    /// Some of the running query's count is in the sum buffer.
    pub on_gpu: bool,
}
