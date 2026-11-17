//! The front's objects. Each holds the back end's handle in a [`Kept`], which hands the handle to the
//! device's graveyard when the last reference goes, so the device destroys it at its next call.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wcontainers_gpu::{Backend, Format, Object, Reflection, TextureDesc, VertexFormat};

use crate::desc::{
    BlendDesc, BufferDesc, DepthStencilDesc, RasterizerDesc, SamplerDesc, Texture2DDesc, ViewDesc,
};

/// Handles whose last front reference went, and the ids of the front objects that held them.
pub struct Graveyard<B: Backend> {
    pub objects: Vec<Object<B>>,
    pub ids: Vec<u64>,
}

pub type SharedGraveyard<B> = Rc<RefCell<Graveyard<B>>>;

/// A back-end handle owned by one front object.
pub struct Kept<B: Backend, H: Clone> {
    pub handle: H,
    pub id: u64,
    wrap: fn(H) -> Object<B>,
    graveyard: SharedGraveyard<B>,
}

impl<B: Backend, H: Clone> Kept<B, H> {
    pub fn new(
        handle: H,
        id: u64,
        wrap: fn(H) -> Object<B>,
        graveyard: &SharedGraveyard<B>,
    ) -> Self {
        Kept {
            handle,
            id,
            wrap,
            graveyard: graveyard.clone(),
        }
    }
}

impl<B: Backend, H: Clone> Drop for Kept<B, H> {
    fn drop(&mut self) {
        let mut g = self.graveyard.borrow_mut();
        g.objects.push((self.wrap)(self.handle.clone()));
        g.ids.push(self.id);
    }
}

pub struct Buffer<B: Backend> {
    pub desc: BufferDesc,
    /// The back end's buffer, replaced when a constant buffer grows to its shader's size.
    pub gpu: RefCell<Kept<B, B::Buffer>>,
    /// Bytes of the back end's buffer: `desc.byte_width` rounded up to the copy granule, or the size
    /// it grew to.
    pub size: Cell<u64>,
    /// It grew: its bytes past `desc.byte_width` are zero, and whole writes keep them so.
    pub grown: Cell<bool>,
    /// The latest contents of a dynamic or staging buffer, which a map hands out; for a grown dynamic
    /// buffer, all of its bytes.
    pub shadow: RefCell<Vec<u8>>,
}

impl<B: Backend> Buffer<B> {
    /// The back end's buffer.
    pub fn handle(&self) -> B::Buffer {
        self.gpu.borrow().handle.clone()
    }
}

pub struct Texture2D<B: Backend> {
    /// The description as created, with `mip_levels` resolved.
    pub desc: Texture2DDesc,
    pub format: Format,
    pub gpu_desc: TextureDesc,
    pub gpu: Kept<B, B::Texture>,
    /// Per subresource of a staging texture, tightly packed rows a map hands out.
    pub shadow: RefCell<Vec<Vec<u8>>>,
}

impl<B: Backend> Texture2D<B> {
    /// Width and height of mip `mip`.
    pub fn mip_size(&self, mip: u32) -> (u32, u32) {
        (
            (self.desc.width >> mip).max(1),
            (self.desc.height >> mip).max(1),
        )
    }
}

/// The resources a view or a copy may name.
pub enum Resource<B: Backend> {
    Buffer(Rc<Buffer<B>>),
    Texture2D(Rc<Texture2D<B>>),
}

impl<B: Backend> Clone for Resource<B> {
    fn clone(&self) -> Self {
        match self {
            Resource::Buffer(b) => Resource::Buffer(b.clone()),
            Resource::Texture2D(t) => Resource::Texture2D(t.clone()),
        }
    }
}

/// A render target, depth-stencil or shader resource view of a 2D texture.
pub struct View<B: Backend> {
    pub texture: Rc<Texture2D<B>>,
    /// The description, with defaults resolved.
    pub desc: ViewDesc,
    pub format: Format,
    pub gpu: Kept<B, B::TextureView>,
}

pub struct Sampler<B: Backend> {
    pub desc: SamplerDesc,
    pub gpu: Kept<B, B::Sampler>,
}

pub struct Blend<B: Backend> {
    pub desc: BlendDesc,
    pub gpu: Kept<B, B::Blend>,
}

pub struct DepthStencil<B: Backend> {
    pub desc: DepthStencilDesc,
    pub gpu: Kept<B, B::DepthStencil>,
}

pub struct Rasterizer<B: Backend> {
    pub desc: RasterizerDesc,
    pub gpu: Kept<B, B::Raster>,
}

/// One input element resolved against the vertex shader's signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutElement {
    pub location: u32,
    pub slot: u32,
    pub offset: u32,
    pub format: VertexFormat,
}

/// An input layout. The back end's vertex layout block is made per (layout, strides) at draws.
pub struct InputLayout {
    pub id: u64,
    pub elements: Vec<LayoutElement>,
    /// Per input slot: none (per vertex) or the instance step rate.
    pub instance_steps: [Option<u32>; 32],
}

pub struct Shader<B: Backend> {
    pub gpu: Kept<B, B::Shader>,
    pub reflection: Reflection,
    /// The bytes each constant buffer the program reads is declared with, by binding number.
    pub constant_sizes: Vec<(u32, u64)>,
}
