//! The front's objects. Each holds the back end's handle in a [`Kept`], which hands the handle to the
//! device's graveyard when the last reference goes, so the device destroys it at its next call.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wcontainers_gpu::{
    Backend, Extent, Format, FormatKind, Object, Origin, Reflection, TextureDesc as GpuTexture,
    TextureRegion, VertexFormat, ViewDimension,
};

use crate::desc::{
    BlendDesc, BufferDesc, DepthStencilDesc, RasterizerDesc, SamplerDesc, TextureDesc, ViewDesc,
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
    /// How the handle is destroyed; `None` for a handle another owner destroys (a swap chain's
    /// back buffers).
    wrap: Option<fn(H) -> Object<B>>,
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
            wrap: Some(wrap),
            graveyard: graveyard.clone(),
        }
    }

    /// A handle this object uses but another owner destroys.
    pub fn borrowed(handle: H, id: u64, graveyard: &SharedGraveyard<B>) -> Self {
        Kept {
            handle,
            id,
            wrap: None,
            graveyard: graveyard.clone(),
        }
    }
}

impl<B: Backend, H: Clone> Drop for Kept<B, H> {
    fn drop(&mut self) {
        let mut g = self.graveyard.borrow_mut();
        if let Some(wrap) = self.wrap {
            g.objects.push(wrap(self.handle.clone()));
        }
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
    /// A staging buffer's shadow against the GPU's bytes.
    pub staged: RefCell<Staged<B>>,
}

/// A staging subresource's shadow against the GPU's copy of it.
pub enum Staged<B: Backend> {
    /// The shadow holds the GPU's bytes.
    Current,
    /// The GPU wrote it since the shadow was filled; no read back started.
    Written,
    /// The GPU wrote it, and the read back started after that write.
    Reading(B::Readback),
}

impl<B: Backend> Buffer<B> {
    /// The back end's buffer.
    pub fn handle(&self) -> B::Buffer {
        self.gpu.borrow().handle.clone()
    }
}

/// A 1D, 2D or 3D texture. A 1D texture is a 2D one of height 1 on the back end, as converted DXBC
/// reads it.
pub struct Texture<B: Backend> {
    /// The description as created, with `mip_levels` resolved.
    pub desc: TextureDesc,
    pub format: Format,
    pub gpu_desc: GpuTexture,
    pub gpu: Kept<B, B::Texture>,
    /// Per subresource of a staging texture, tightly packed rows and slices a map hands out.
    pub shadow: RefCell<Vec<Vec<u8>>>,
    /// Per subresource of a staging texture, its shadow against the GPU's copy.
    pub staged: RefCell<Vec<Staged<B>>>,
}

impl<B: Backend> Texture<B> {
    /// Width, height and depth of mip `mip`.
    pub fn mip_extent(&self, mip: u32) -> Extent {
        Extent {
            width: (self.desc.width >> mip).max(1),
            height: (self.desc.height >> mip).max(1),
            depth_or_layers: (self.desc.depth >> mip).max(1),
        }
    }

    /// The mip and layer of subresource `sub`, or none past the texture.
    pub fn subresource(&self, sub: u32) -> Option<(u32, u32)> {
        let mips = self.desc.mip_levels;
        (sub < mips * self.desc.array_size).then_some((sub % mips, sub / mips))
    }

    /// The back end's region of `mip` and `layer` at `origin`.
    pub fn region(&self, mip: u32, layer: u32, origin: Origin) -> TextureRegion<'_, B> {
        TextureRegion {
            texture: &self.gpu.handle,
            mip,
            layer,
            origin,
            stencil: false,
        }
    }

    /// Subresource `sub` whole: its region and extent.
    pub fn whole(&self, sub: u32) -> Option<(TextureRegion<'_, B>, Extent)> {
        let (mip, layer) = self.subresource(sub)?;
        Some((
            self.region(mip, layer, Origin { x: 0, y: 0, z: 0 }),
            self.mip_extent(mip),
        ))
    }

    /// Bytes of a row, rows of a slice, and slices of mip `mip`, tightly packed.
    pub fn packed(&self, mip: u32) -> (u32, u32, u32) {
        let e = self.mip_extent(mip);
        let (row, rows) = crate::format::row_bytes(self.format, e.width, e.height);
        (row, rows, e.depth_or_layers)
    }
}

/// The resources a view or a copy may name.
pub enum Resource<B: Backend> {
    Buffer(Rc<Buffer<B>>),
    Texture(Rc<Texture<B>>),
}

impl<B: Backend> Clone for Resource<B> {
    fn clone(&self) -> Self {
        match self {
            Resource::Buffer(b) => Resource::Buffer(b.clone()),
            Resource::Texture(t) => Resource::Texture(t.clone()),
        }
    }
}

/// A render target, depth-stencil or shader resource view of a 2D texture.
pub struct View<B: Backend> {
    pub texture: Rc<Texture<B>>,
    /// The description, with defaults resolved.
    pub desc: ViewDesc,
    pub format: Format,
    pub gpu: Kept<B, B::TextureView>,
}

impl<B: Backend> View<B> {
    /// Whether the two views share a subresource.
    pub fn overlaps(&self, other: &View<B>) -> bool {
        fn meet(a: u32, n: u32, b: u32, m: u32) -> bool {
            a < b + m && b < a + n
        }
        Rc::ptr_eq(&self.texture, &other.texture)
            && meet(
                self.desc.first_mip,
                self.desc.mip_count,
                other.desc.first_mip,
                other.desc.mip_count,
            )
            && meet(
                self.desc.first_layer,
                self.desc.layer_count,
                other.desc.first_layer,
                other.desc.layer_count,
            )
    }
}

/// A view a shader reads (a shader resource view) or writes (an unordered access view): of a
/// texture, or of a range of a buffer.
pub enum ShaderView<B: Backend> {
    Texture(Rc<View<B>>),
    Buffer(BufferSpan<B>),
}

impl<B: Backend> ShaderView<B> {
    /// The texture view, for a view of a texture.
    pub fn texture(&self) -> Option<&Rc<View<B>>> {
        match self {
            ShaderView::Texture(v) => Some(v),
            ShaderView::Buffer(_) => None,
        }
    }

    /// Whether the two views share a subresource of a texture, or any of one buffer.
    pub fn overlaps(&self, other: &ShaderView<B>) -> bool {
        match (self, other) {
            (ShaderView::Texture(a), ShaderView::Texture(b)) => a.overlaps(b),
            (ShaderView::Buffer(a), ShaderView::Buffer(b)) => Rc::ptr_eq(&a.buffer, &b.buffer),
            _ => false,
        }
    }
}

/// A range of a buffer a view reads or writes, with the hidden counter of an append, consume or
/// counter view.
pub struct BufferSpan<B: Backend> {
    pub buffer: Rc<Buffer<B>>,
    pub offset: u64,
    pub size: u64,
    pub counter: Option<Kept<B, B::Buffer>>,
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
    /// What stands in for an empty slot of each texture and sampler it reads, by binding number
    /// (`None` where nothing does yet).
    pub stand_ins: Vec<(u32, Option<StandIn>)>,
    /// A vertex shader's bytecode, converted again for pixel shaders that interpolate its outputs
    /// other than by perspective; empty for the other stages.
    pub bytecode: Vec<u8>,
}

/// What a draw binds for an empty texture or sampler slot its shader reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandIn {
    /// A view of zeros of this dimension and kind.
    Zeros(ViewDimension, FormatKind),
    /// Direct3D's default sampler state.
    DefaultSampler,
    /// A buffer of zeros read as a structured or raw buffer.
    ZeroBuffer,
}
