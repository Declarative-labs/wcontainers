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
    /// The slice drawn into when `view` is a 3D view (Direct3D's render target of a volume slice); zero
    /// otherwise. Layered drawing through a program's layer output is plan section 7's stage work.
    pub depth_slice: u32,
    /// Where a multisampled target is resolved to when the pass ends.
    pub resolve: Option<&'a B::TextureView>,
    /// Clear color as raw bits: f32 for float formats, the integer for integer formats.
    pub load: LoadOp<[u32; 4]>,
    pub store: StoreOp,
}

/// What a pass does with one aspect (depth or stencil) of its depth-stencil target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AspectOps<V> {
    pub load: LoadOp<V>,
    pub store: StoreOp,
    /// The pass only reads this aspect, so it may also be sampled during the pass (Direct3D's read-only
    /// depth and stencil views). No draw of the pass writes it: depth writes off, or stencil operations
    /// that keep or a write mask of zero, on each face.
    pub read_only: bool,
}

/// The depth and stencil target of a pass. Each aspect is present exactly when the view's format has it,
/// so back ends never see operations on an aspect that does not exist.
#[derive(Debug)]
pub struct DepthAttachment<'a, B: Backend> {
    pub view: &'a B::TextureView,
    /// Clear depth as f32 bits.
    pub depth: Option<AspectOps<u32>>,
    pub stencil: Option<AspectOps<u8>>,
}

/// The size of a pass that has no attachments (Direct3D's rendering with only storage writes bound).
/// A back end whose API needs an attachment adds one of its own, never stored, of this size and sample
/// count; its pipelines are built for it. Width and height are 1 to [`crate::Limits::max_texture_2d`],
/// and `samples` a count [`crate::FormatCaps::takes_samples`] gives for [`crate::Format::R8Unorm`], the
/// format of that attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EmptyPass {
    pub width: u32,
    pub height: u32,
    pub samples: u32,
}

/// A pass to begin. Either at least one attachment is given, or `empty` is. Every attachment and resolve
/// target has the same width and height, as both browser APIs require (WebGPU's render extent, WebGL 2's
/// `FRAMEBUFFER_INCOMPLETE_DIMENSIONS`). A front whose API draws into attachments of different sizes,
/// limited to their common area (Direct3D 9 to 11 with a larger depth buffer, OpenGL, Vulkan), passes
/// them through the shared pass tracker, which puts temporaries of one size in their place (slow path
/// [`crate::SlowPath::AttachmentsFitted`], plan section 4.7). Every attachment has the same sample
/// count; a resolve target is a single-sampled view of its attachment's format. No subresource (or 3D
/// slice) is attached or resolved into twice, and colour attachments stay within
/// `Limits::color_targets` and `Limits::color_bytes_per_sample`.
///
/// One pass is one usage scope, as WebGPU defines it: a buffer, or a texture subresource, that any draw
/// of the pass binds as writable storage is used no other way by any draw of the pass (not as vertices,
/// indices, indirect arguments, a uniform, sampled or read-only storage binding). The shared pass tracker
/// keeps this for every front: it ends the pass and begins the next with `Load` before a draw that would
/// break it ([`crate::SlowPath::UsageScopeSplit`]), and a draw that itself uses one buffer both ways
/// (OpenGL and Vulkan allow it) reads the other use from a copy ([`crate::SlowPath::UsageScopeCopied`]).
#[derive(Debug)]
pub struct PassDesc<'a, B: Backend> {
    pub colors: [Option<ColorAttachment<'a, B>>; MAX_COLOR_TARGETS],
    pub depth: Option<DepthAttachment<'a, B>>,
    pub empty: Option<EmptyPass>,
    /// Occlusion queries recorded in this pass write into this set. A guest query that spans several
    /// passes is several interface queries, whose results the front adds (counts) or combines (any).
    pub occlusion: Option<&'a B::QuerySet>,
}

/// A viewport. Floats as bits so draws can be compared; x, y, width and height may be fractional only on a
/// back end with [`crate::Features::FRACTIONAL_VIEWPORT`]. `min_depth` is at most `max_depth`, as WebGPU
/// and WebGL 2 require; a front whose API allows the reverse (Vulkan, OpenGL's `glDepthRange`) passes the
/// range swapped and binds the program variant that writes `w - z` as clip depth, which gives the same
/// depth values (slow path [`crate::SlowPath::ViewportDepthFlip`]).
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

/// Which vertices a draw covers. Every back end gives the program's `vertex_index` and `instance_index`
/// as WebGPU and Vulkan count them: `first` plus the vertex's place for a draw without indices, the
/// index plus `base_vertex` for an indexed one, and `first_instance` plus the instance's place. A front
/// whose API counts otherwise binds a program variant that takes the bases out
/// ([`crate::DrawBaseSource`]); the bases it writes are those of the API's draw, also when a route
/// splits the draw into several.
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
    /// One viewport: a front whose API draws through several splits the draw into one per viewport
    /// ([`crate::SlowPath::MultiViewportSplit`]).
    pub viewports: &'a [Viewport],
    /// At most one scissor, and one where the raster block scissors. It may reach past the pass's
    /// attachments and is clipped to them; a back end whose API refuses such a scissor cuts it.
    pub scissors: &'a [ScissorRect],
    /// Blend constant as f32 bits.
    pub blend_constant: [u32; 4],
    /// Stencil reference for front and back faces. They differ only with
    /// [`crate::Features::STENCIL_PER_FACE`]; otherwise a front draws twice, culling one face each time
    /// (slow path [`crate::SlowPath::StencilFacesSplit`]).
    pub stencil_reference: [u8; 2],
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
