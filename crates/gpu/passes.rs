//! The pass tracker: the one place every front turns its targets, clears and draws into the
//! interface's passes (plan 4.7). Implicit fronts (Direct3D 11, OpenGL, Direct3D 9) set targets,
//! clear and draw, and the tracker opens a pass at the first draw, makes a clear that comes first the
//! pass's `Clear` load, and closes the pass when the front copies or changes targets. Explicit fronts
//! (Direct3D 12, Vulkan) begin and end their own passes through it. For both it fits attachments of
//! different sizes ([`SlowPath::AttachmentsFitted`]), splits a pass where a draw would break its usage
//! scope ([`SlowPath::UsageScopeSplit`]), and carries a running occlusion query across every split.

use crate::Gpu;
use crate::MAX_COLOR_TARGETS;
use crate::draw::{AspectOps, EmptyPass, LoadOp, ScissorRect, StoreOp};
use crate::format::Format;
use alloc::vec::Vec;

/// One attachment: the view a pass names and the subresource it is, as the front made them. A colour
/// attachment or resolve target whose size differs from the pass's is copied in and out of a
/// temporary, so its texture needs `COPY_SRC` and `COPY_DST`; a multisampled one is copied by a draw
/// reading it, so it needs `SAMPLE` too.
#[derive(Debug)]
pub struct Target<B: Gpu> {
    pub view: B::TextureView,
    pub texture: B::Texture,
    /// The view's format.
    pub format: Format,
    pub mip: u32,
    /// The array layer.
    pub layer: u32,
    /// The slice of a 3D level the view draws into (`ColorAttachment::depth_slice`); 0 otherwise.
    pub depth_slice: u32,
    /// The size of the view's level.
    pub width: u32,
    pub height: u32,
    pub samples: u32,
}

/// A colour target and the single-sampled target it resolves into, if any.
#[derive(Debug)]
pub struct ColorTarget<B: Gpu> {
    pub target: Target<B>,
    pub resolve: Option<Target<B>>,
}

/// The targets drawn into: up to eight colour targets and one depth-stencil target, or for a pass
/// with no attachment its size and sample count.
#[derive(Debug)]
pub struct PassTargets<B: Gpu> {
    pub colors: [Option<ColorTarget<B>>; MAX_COLOR_TARGETS],
    pub depth: Option<Target<B>>,
    pub empty: Option<EmptyPass>,
}

/// What a pass does with each attachment at its start and end; the depth and stencil operations are
/// used only for the aspects the format has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PassOps {
    pub colors: [(LoadOp<[u32; 4]>, StoreOp); MAX_COLOR_TARGETS],
    pub depth: AspectOps<u32>,
    pub stencil: AspectOps<u8>,
}

/// How a draw uses one resource, for the usage-scope rule: a whole buffer or one texture subresource,
/// named by a number the front gives it, used as writable storage or any other way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScopeUse {
    pub resource: u64,
    pub storage_write: bool,
}

/// A temporary attachment of a fitted pass, pooled by size, format and sample count.
#[derive(Debug)]
pub struct TemporaryTarget<B: Gpu> {
    pub key: (u32, u32, Format, u32),
    pub texture: B::Texture,
    pub view: B::TextureView,
    pub in_use: bool,
}

/// The pass that is open, and what closing it must do.
#[derive(Debug)]
pub struct OpenPass {
    /// For each colour target, the temporary standing for it and whether its common area goes back.
    pub colors: [Option<(usize, bool)>; MAX_COLOR_TARGETS],
    /// For each resolve target, the temporary it resolves into.
    pub resolves: [Option<usize>; MAX_COLOR_TARGETS],
    /// The area every attachment covers, where it is smaller than the pass.
    pub common: Option<ScissorRect>,
    /// Resources the pass's draws named as writable storage, and those they named any other way,
    /// each sorted.
    pub written: Vec<u64>,
    pub other: Vec<u64>,
    /// An occlusion query segment runs in this pass.
    pub query: bool,
}

/// The occlusion query set the tracker begins queries in.
#[derive(Debug)]
pub struct PassQueries<B: Gpu> {
    pub set: B::QuerySet,
    pub count: u32,
    pub next: u32,
    /// The first index of the guest query that runs, if one does.
    pub running: Option<u32>,
}

/// The pass tracker. A front holds one per device and calls the back end's pass, query and copy
/// methods only through it while it holds targets.
#[derive(Debug)]
pub struct PassTracker<B: Gpu> {
    pub targets: Option<PassTargets<B>>,
    pub ops: PassOps,
    pub open: Option<OpenPass>,
    pub pool: Vec<TemporaryTarget<B>>,
    pub queries: Option<PassQueries<B>>,
    pub generation: u64,
}
