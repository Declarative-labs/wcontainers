//! The objects this implementation hands out, each a cheap handle to what it made through Metal.

use core::cell::{Cell, RefCell};
use core::fmt;
use std::collections::BTreeMap;
use std::rc::Rc;

use wcontainers_gpu::{
    BlendDesc, DepthStencilDesc, Format, RasterDesc, ShaderStage, TextureDesc, VertexLayoutDesc,
};
use wcontainers_metal::{Metal, PixelFormat};

/// A handle whose clones name the same object.
pub struct Shared<T>(pub(crate) Rc<T>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T> fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Shared({:p})", Rc::as_ptr(&self.0))
    }
}

impl<T> core::ops::Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

/// A buffer, renamed by writes that drop its contents while GPU work still uses the old storage.
pub struct BufferObj<M: Metal> {
    pub(crate) size: u64,
    pub(crate) raw: RefCell<M::Buffer>,
    /// The serial of the last command buffer that uses the current storage.
    pub(crate) last_use: Cell<u64>,
}

/// A texture and what it was made as.
pub struct TextureObj<M: Metal> {
    pub(crate) desc: TextureDesc,
    pub(crate) raw: M::Texture,
}

/// A view: the Metal texture that sees part of a texture, or a surface's texture.
pub struct ViewObj<M: Metal> {
    pub(crate) raw: M::Texture,
    pub(crate) format: Format,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) samples: u32,
}

/// A sampler state.
pub struct SamplerObj<M: Metal> {
    pub(crate) raw: M::SamplerState,
}

/// What kind of slot a resource binding of a shader is read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotKind {
    Buffer,
    Texture,
    Sampler,
}

/// A shader in the common form, with the Metal slot of every resource binding its entry point reads.
pub struct ShaderObj {
    pub(crate) id: u64,
    pub(crate) module: naga::Module,
    pub(crate) info: naga::valid::ModuleInfo,
    pub(crate) entry_point: String,
    pub(crate) stage: ShaderStage,
    /// The slot of each `(group, binding)` the entry point reads.
    pub(crate) slots: Slots,
}

/// The slot kind and index of each `(group, binding)` of a shader.
pub(crate) type Slots = BTreeMap<(u32, u32), (SlotKind, u32)>;

/// A program: its shaders, compiled when a draw or dispatch first needs them.
pub enum ProgramObj {
    Render {
        id: u64,
        vertex: Shared<ShaderObj>,
        fragment: Option<Shared<ShaderObj>>,
    },
    Compute {
        id: u64,
        shader: Shared<ShaderObj>,
    },
}

/// A state block and its number, which keys the pipelines built with it.
pub struct Block<D> {
    pub(crate) id: u64,
    pub(crate) desc: D,
}

/// The blend block.
pub type BlendBlock = Block<BlendDesc>;
/// The depth and stencil block.
pub type DepthStencilBlock = Block<DepthStencilDesc>;
/// The rasterising block.
pub type RasterBlock = Block<RasterDesc>;
/// The vertex layout block.
pub type VertexLayoutBlock = Block<VertexLayoutDesc>;

/// One resource of a binding set.
pub(crate) enum SetResource<M: Metal> {
    Buffer {
        buffer: Shared<BufferObj<M>>,
        offset: u64,
        size: u64,
    },
    Texture(Shared<ViewObj<M>>),
    Sampler(Shared<SamplerObj<M>>),
}

/// A binding set: each binding number and its resource.
pub struct SetObj<M: Metal> {
    pub(crate) entries: Vec<(u32, SetResource<M>)>,
}

/// A query set: one eight-byte result each, in a buffer the GPU writes.
pub struct QueryObj<M: Metal> {
    pub(crate) count: u32,
    pub(crate) buffer: M::Buffer,
}

/// How the bytes of a read back become what the caller reads.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ReadLayout {
    /// As they are.
    Bytes,
    /// Depth held as 32-bit float, read as 24-bit unorm in the low bits of each word.
    Depth24,
}

/// A read back: the buffer a copy lands in, and the command buffer it was recorded into.
pub struct ReadbackObj<M: Metal> {
    pub(crate) serial: u64,
    pub(crate) buffer: M::Buffer,
    pub(crate) length: u64,
    pub(crate) layout: ReadLayout,
}

/// A point in the GPU's work: the serial of a submitted command buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetalFence(pub(crate) u64);

/// A window's layer and the frame drawn for it.
pub struct SurfaceObj<M: Metal> {
    pub(crate) layer: M::Layer,
    pub(crate) frame: RefCell<Option<Frame<M>>>,
}

/// The texture a surface's frames are drawn into, in the surface's format, and its view read when presenting.
pub(crate) struct Frame<M: Metal> {
    pub(crate) format: Format,
    pub(crate) drawn: Shared<ViewObj<M>>,
    /// A view of the same texels as linear values, so presenting moves their bytes.
    pub(crate) read: Shared<ViewObj<M>>,
}

/// No object: this implementation makes no resource tables yet.
#[derive(Clone, Copy, Debug)]
pub enum Never {}

/// Returns the Metal pixel format of a view of a format, for an aspect.
pub(crate) fn choose_view_format(format: Format, stencil: bool) -> Option<PixelFormat> {
    match (format, stencil) {
        (Format::Depth24PlusStencil8 | Format::Depth32FloatStencil8, true) => {
            Some(PixelFormat::X32Stencil8)
        }
        _ => crate::convert::pixel_format(format),
    }
}
