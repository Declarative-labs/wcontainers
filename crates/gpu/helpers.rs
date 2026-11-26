//! What the fast-path defaults keep between calls: built-in programs and blocks made at first use, and
//! the ring for per-draw data. Each built-in program is its own module, holding only the bindings and
//! types its entry points use, and is made only when a default first draws with it, so a back end never
//! meets one it cannot run (WebGL 2 never meets a cube-array texture). Each back end holds one
//! [`Helpers`] and hands it out through [`crate::Gpu::helpers`]; a back end that overrides every fast
//! path never fills it.

use crate::Gpu;
use crate::format::FormatKind;
use crate::resource::ViewDimension;
use alloc::vec::Vec;

/// How a mip program reads the level above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MipRead {
    /// One sample through the linear sampler: formats with [`crate::caps::FormatUse::FILTER`].
    Filtered,
    /// Four (eight in 3D) samples at texel centres through the nearest sampler, blended in the program
    /// with the weights a linear filter uses: float formats the back end cannot filter.
    Unfiltered,
    /// Integer loads decoded as unsigned or signed normalised values (scale in `params.value.x`),
    /// blended in the program, encoded back: textures a front stores by
    /// [`crate::slow::SlowPath::NormAsInteger`].
    Unorm,
    Snorm,
}

/// The built-in programs, by use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    /// Fills a level from the level above, read through a view of this dimension.
    Mip(ViewDimension, MipRead),
    ClearFloat,
    ClearUint,
    ClearSint,
    ClearDepth,
    /// No fragment shader: stencil-only clears.
    ClearStencil,
    /// Writes each texel of the texture at binding 32 (of this kind: float, unsigned or signed)
    /// into the same place of the target: with `true`, from a multisampled texture, run per sample
    /// and reading that sample; with `false`, from a single-sampled one into every sample.
    SampleCopy(FormatKind, bool),
}

/// Bytes of the built-in shaders' uniform block.
pub const PARAMS_SIZE: usize = 32;

/// Blocks the defaults draw with, made once.
#[derive(Debug)]
pub struct Builtins<B: Gpu> {
    pub blend: B::Blend,
    pub raster: B::Raster,
    pub vertex_layout: B::VertexLayout,
    pub no_depth: B::DepthStencil,
    /// Depth and stencil written unconditionally: depth only, stencil only, both.
    pub write_depth_stencil: [B::DepthStencil; 3],
    pub linear: B::Sampler,
    pub nearest: B::Sampler,
}

/// One ring [`crate::Gpu::write_buffer_ring`] writes into.
#[derive(Debug)]
pub struct HelperRing<B: Gpu> {
    pub buffer: B::Buffer,
    pub size: u64,
    pub head: u64,
}

/// State the fast-path defaults keep. Back ends make it with `Helpers::default()` and never look inside.
#[derive(Debug)]
pub struct Helpers<B: Gpu> {
    pub builtins: Option<Builtins<B>>,
    /// Built-in programs made so far.
    pub programs: Vec<(Builtin, B::Program)>,
    /// The rings, by [`crate::RingKind`].
    pub rings: [Option<HelperRing<B>>; 2],
    /// Bytes a default builds before one ring write (a converted fan's indices), kept so no draw
    /// allocates once it has grown.
    pub scratch: Vec<u8>,
}
