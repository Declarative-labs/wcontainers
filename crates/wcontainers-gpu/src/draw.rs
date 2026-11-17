//! Passes, draws and dispatches.
//!
//! Recording follows one rule for every front and back end: draws and queries happen inside a pass;
//! copies, whole-texture clears, dispatches, vertex capture and read backs happen outside. Writes may come
//! anywhere, but an [`crate::WriteMode::Ordered`] write inside a pass is a slow path on a back end that has
//! to split the pass for it. Explicit fronts (Direct3D 12, Vulkan) map their own passes onto this;
//! implicit ones (Direct3D 11, OpenGL) open and close passes as their targets change and before copies.

use crate::MAX_COLOR_TARGETS;
use crate::backend::Backend;
use crate::resource::BufferRange;
use crate::state::{IndexFormat, Topology};

/// What happens to an attachment when a pass begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoadOp<V> {
    /// Keep what is there.
    Load,
    /// Clear to this value first.
    Clear(V),
    /// The contents are not needed; the back end may start from anything.
    DontCare,
}

/// What happens to an attachment when a pass ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StoreOp {
    Store,
    /// The contents are not needed after the pass.
    Discard,
}

/// One color target of a pass.
#[derive(Debug)]
pub struct ColorAttachment<'a, B: Backend> {
    pub view: &'a B::TextureView,
    /// Where a multisampled target is resolved to when the pass ends.
    pub resolve: Option<&'a B::TextureView>,
    /// Clear color as raw bits: f32 for float formats, the integer for integer formats.
    pub load: LoadOp<[u32; 4]>,
    pub store: StoreOp,
}

/// The depth and stencil target of a pass.
#[derive(Debug)]
pub struct DepthAttachment<'a, B: Backend> {
    pub view: &'a B::TextureView,
    /// Clear depth as f32 bits.
    pub depth_load: LoadOp<u32>,
    pub depth_store: StoreOp,
    pub stencil_load: LoadOp<u8>,
    pub stencil_store: StoreOp,
    /// The pass only reads depth and stencil, so they may also be sampled during it.
    pub read_only: bool,
}

/// A pass to begin.
#[derive(Debug)]
pub struct PassDesc<'a, B: Backend> {
    pub colors: [Option<ColorAttachment<'a, B>>; MAX_COLOR_TARGETS],
    pub depth: Option<DepthAttachment<'a, B>>,
    /// Occlusion queries recorded in this pass write into this set.
    pub occlusion: Option<&'a B::QuerySet>,
}

/// A viewport. Floats as bits so draws can be compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub min_depth: u32,
    pub max_depth: u32,
}

/// A scissor rectangle, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScissorRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// One bound vertex stream.
#[derive(Debug)]
pub struct VertexBinding<'a, B: Backend> {
    pub buffer: &'a B::Buffer,
    pub offset: u64,
}

/// The bound index buffer.
#[derive(Debug)]
pub struct IndexBinding<'a, B: Backend> {
    pub buffer: &'a B::Buffer,
    pub offset: u64,
    pub format: IndexFormat,
}

/// Which vertices a draw covers.
#[derive(Debug)]
pub enum DrawRange<'a, B: Backend> {
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
    /// Arguments read from a buffer, where [`crate::Features::INDIRECT`] is present.
    Indirect {
        args: &'a B::Buffer,
        offset: u64,
    },
    IndexedIndirect {
        args: &'a B::Buffer,
        offset: u64,
    },
}

/// Everything one draw names. Implicit fronts keep one of these as their current state and pass it on
/// each draw; back ends compare it with the previous draw's by handle and value, and change only what
/// differs.
#[derive(Debug)]
pub struct DrawCall<'a, B: Backend> {
    pub program: &'a B::Program,
    pub blend: &'a B::Blend,
    pub depth_stencil: &'a B::DepthStencil,
    pub raster: &'a B::Raster,
    pub vertex_layout: &'a B::VertexLayout,
    pub bindings: [Option<&'a B::BindingSet>; crate::MAX_BINDING_SETS],
    /// Indexed by stream number of the vertex layout.
    pub vertex_buffers: &'a [Option<VertexBinding<'a, B>>],
    pub index: Option<IndexBinding<'a, B>>,
    pub topology: Topology,
    /// Strips restart at the largest index value. Where [`crate::Features::RESTART_SWITCH`] is absent and
    /// a front's API allows turning it off, the front rewrites indices (slow path
    /// [`crate::SlowPath::RestartRewrite`]).
    pub restart: bool,
    pub viewports: &'a [Viewport],
    pub scissors: &'a [ScissorRect],
    /// Blend constant as f32 bits.
    pub blend_constant: [u32; 4],
    pub stencil_reference: u8,
    /// Small constants the program reads directly (root constants, push constants, plain uniforms),
    /// up to [`crate::Limits::immediate_bytes`].
    pub immediates: &'a [u8],
    pub range: DrawRange<'a, B>,
}

/// One compute dispatch.
#[derive(Debug)]
pub struct DispatchCall<'a, B: Backend> {
    pub program: &'a B::Program,
    pub bindings: [Option<&'a B::BindingSet>; crate::MAX_BINDING_SETS],
    pub immediates: &'a [u8],
    pub groups: DispatchGroups<'a, B>,
}

/// How many workgroups a dispatch runs.
#[derive(Debug)]
pub enum DispatchGroups<'a, B: Backend> {
    Direct([u32; 3]),
    Indirect { args: &'a B::Buffer, offset: u64 },
}

/// Where captured vertices go, for [`crate::Backend::capture_vertices`].
pub type CaptureTargets<'a, B> = &'a [BufferRange<&'a <B as Backend>::Buffer>];
