//! The interface between graphics fronts and back ends.
//!
//! A **front** implements an API a guest program calls (Direct3D 11 and 12 now; OpenGL, Vulkan and
//! Direct3D 9 later). A **back end** drives a GPU API the host offers (WebGPU and WebGL 2 now). Fronts and
//! back ends never meet: both depend only on this crate.
//!
//! The shape, in brief (the plan, `docs/phase-2-graphics.md`, has the reasons):
//!
//! - [`Backend`] holds the **base**: what every back end provides. Create and destroy buffers, textures,
//!   views and samplers; write bytes into them straight from guest memory; take a shader in the common form
//!   ([`ShaderModule`]); record passes of draws, each draw naming immutable state blocks, a binding set and
//!   a range; copy and clear; read back later; present a frame.
//! - It also holds the **fast paths**: methods with a default made from the base, which a back end
//!   overrides only where it has something better (read back now, triangle fans and line loops, preparing
//!   a state combination ahead of time, clearing a texture or rectangles of it, generating mipmaps, a ring
//!   for per-draw data). Their defaults keep what they make in [`Helpers`].
//! - Things with no cheap default (compute, transform feedback, big resource tables, timestamps) are
//!   methods whose default reports [`GpuError::Absent`]; a back end that has them overrides them and
//!   declares them in its [`Caps`]. Fronts read [`Caps`] once and choose their route.
//! - Drawing state is a handful of immutable blocks ([`BlendDesc`], [`DepthStencilDesc`], [`RasterDesc`],
//!   [`VertexLayoutDesc`], a [`Program`]) plus binding sets
//!   ([`Backend::create_binding_set`]). Every block becomes a back-end handle once; a draw names one of
//!   each. A back end consumes them its own way (WebGPU: a cached pipeline per combination; WebGL 2: apply
//!   only the blocks that changed).
//! - Everything is generic: a front is written as `Front<B: Backend>` and each call is one ordinary
//!   function call, resolved when the front is compiled for a back end.
//! - One thread owns a back end. Nothing here is `Send`; guest threads reach it through the call channel.

#![no_std]

extern crate alloc;

mod backend;
mod caps;
mod draw;
mod error;
mod format;
mod helpers;
mod occlusion;
mod passes;
mod reflect;
mod resource;
mod shader;
mod slow;
mod state;

pub use backend::{
    Backend, ClearValue, FanOrLoop, IndexSource, Object, PrepareDesc, RingKind,
    generate_mips_by_draws,
};
pub use caps::{AdapterInfo, Caps, Features, FormatCaps, FormatUse, Limits};
pub use draw::{
    AspectOps, CaptureTargets, ColorAttachment, DepthAttachment, DispatchCall, DispatchGroups,
    DrawCall, DrawRange, EmptyPass, IndexBinding, LoadOp, PassDesc, ScissorRect, StoreOp,
    VertexBinding, Viewport,
};
pub use error::{DeviceStatus, GpuError, LossReason};
pub use format::{FORMAT_COUNT, FORMAT_FACTS, Format, FormatFacts, FormatKind, color_target_bytes};
pub use helpers::Helpers;
pub use occlusion::{CountedDraw, OcclusionCounter};
pub use passes::{ColorTarget, PassOps, PassTargets, PassTracker, ScopeUse, Target};
pub use reflect::{
    AddedBinding, BLEND_CONSTANT_COMPONENTS, BLEND_HALF_CODES, CLAMPED_SAMPLER_BINDING,
    CONSTANTS_TEXTURE_WIDTH, COUNTER_BINDING, DOMAIN_PATCHES_BINDING, DOMAIN_POINTS_BINDING,
    DRAW_ARGUMENTS_BINDING, DrawBaseSource, EARLY_DEPTH_BINDING, EARLY_STENCIL_BINDING, HELPER_SET,
    HelperSlot, HelperValue, Interpolation, LinkedValues, ReflectedBinding, Reflection, SLOT_KINDS,
    ScalarKind, SlotKind, SlotRange, SourceStage, StageExtras, StageValue, SystemValue,
    TARGET_COPY_BINDING, TEXEL_BUFFER_TEXTURE_WIDTH, TargetCombineValue, TargetInShader,
    TexelBufferRoute, VariantFlags, VariantKey, ViewComponent, helper_binding,
};
pub use resource::{AddressMode, BorderColor, CompareFn, Filter};
pub use resource::{
    BindingEntry, BindingLayoutKey, BindingResource, BufferDesc, BufferRange, BufferUsage,
    CUBE_FACES, CopyBufferTexture, Extent, MapIntent, Origin, QueryKind, ReadSource, ReadState,
    SamplerDesc, TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc,
    ViewDimension, WriteMode,
};
pub use shader::{Program, ShaderModule, ShaderStage, StageSet};
pub use slow::{SlowPath, SlowPaths};
pub use state::{
    BlendComponent, BlendDesc, BlendFactor, BlendOp, ColorWrites, CullMode, DepthStencilDesc,
    FillMode, FrontFace, INDEX_FORMATS, IndexFormat, IndexFormatFacts, LogicOp, ProvokingVertex,
    RasterDesc, RenderTargetBlend, StencilFace, StencilOp, StepMode, TargetLayout, Topology,
    VertexAttribute, VertexFormat, VertexLayoutDesc, VertexStream,
};

/// The most color targets any front can bind at once (Direct3D 10 and later, Vulkan's common limit).
pub const MAX_COLOR_TARGETS: usize = 8;
/// The most vertex streams a draw can read (Direct3D 11's input slots). Streams beyond the back end's
/// [`Limits::vertex_streams`] are fetched by the program ([`SlowPath::VertexPulledInShader`]).
pub const MAX_VERTEX_STREAMS: usize = 32;
/// The most binding sets a draw names, by the binding rule (plan section 6.4): what the vertex (or
/// compute) stage reads alone, what the fragment stage reads alone, what every stage reads (Direct3D 12
/// ranges visible to all stages), and the helper constants ([`HELPER_SET`]).
pub const MAX_BINDING_SETS: usize = 4;
/// The bytes of one word of a storage buffer: a storage binding's size is a whole number of them
/// (WebGPU's rule), and so is the size of a buffer such a binding reaches the end of.
pub const STORAGE_WORD: u64 = size_of::<u32>() as u64;
