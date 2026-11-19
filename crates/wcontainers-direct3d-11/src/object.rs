//! The front's objects. Each holds the back end's handle in a [`Kept`], which hands the handle to the
//! device's graveyard when the last reference goes, so the device destroys it at its next call.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wcontainers_gpu::{
    Backend, Extent, Format, FormatKind, Object, Origin, Reflection, SamplerDesc as GpuSampler,
    TextureDesc as GpuTexture, TextureRegion, VertexFormat, ViewDimension,
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
    /// Counts the writes into it, for the copies of its windows.
    pub writes: Cell<u64>,
    /// Copies of its windows read by blocks longer than them.
    pub windows: RefCell<Vec<Window<B>>>,
    /// Bytes stream output has filled it to (one u32 on the GPU), made at its first binding as a
    /// stream output target.
    pub filled: RefCell<Option<Kept<B, B::Buffer>>>,
}

/// A copy of a constant buffer window of `length` bytes at `offset`, read by a block of `size` bytes:
/// its bytes past the window are zero.
pub struct Window<B: Backend> {
    pub offset: u64,
    pub length: u64,
    pub size: u64,
    /// The buffer's write count and back-end buffer at the last copy.
    pub copied: Cell<Option<(u64, u64)>>,
    pub copy: WindowCopy<B>,
}

/// Where a window is copied to, for the binding its program reads it through.
pub enum WindowCopy<B: Backend> {
    /// A buffer of the block's size.
    Buffer(Kept<B, B::Buffer>),
    /// An RGBA32 unsigned texture of its 16-byte rows, `CONSTANTS_TEXTURE_WIDTH` rows to a texture
    /// row, as many as the block has.
    Texture {
        texture: Kept<B, B::Texture>,
        view: Kept<B, B::TextureView>,
    },
}

/// A staging subresource's shadow against the GPU's copy of it.
pub enum Staged<B: Backend> {
    /// The shadow holds the GPU's bytes.
    Current,
    /// The GPU wrote it since the shadow was filled; no read back started.
    Written,
    /// The GPU wrote it, and the read back started after that write: of the whole, or for a format
    /// with depth and stencil of the depth, then of the stencil.
    Reading(B::Readback, Option<B::Readback>),
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
    /// A swap chain's buffer, whose target views may also take the sRGB twin of its format.
    pub chain: bool,
    /// The minimum level-of-detail clamp `SetResourceMinLOD` gives, in the texture's levels.
    pub min_lod: Cell<f32>,
    /// NV12's chroma plane (this texture holds its luma): half the width and height, two
    /// channels, viewed through `R8G8_*`.
    pub chroma: Option<Rc<Texture<B>>>,
}

/// What a texture's second read back or write holds besides the first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Second {
    /// The stencil of a format with depth and stencil, whose texels have this many bytes.
    Stencil(usize),
    /// NV12's chroma plane, whose rows follow the luma's.
    Chroma,
}

impl<B: Backend> Texture<B> {
    /// Width, height and depth of mip `mip`, in the back end's texels (a YUY2 texel is a pixel
    /// pair).
    pub fn mip_extent(&self, mip: u32) -> Extent {
        let width = match self.desc.format {
            crate::format::YUY2 => self.desc.width / 2,
            _ => self.desc.width,
        };
        Extent {
            width: (width >> mip).max(1),
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

    /// What of subresource `sub` the back end reads and writes apart from [`Self::whole`]: the
    /// stencil aspect of a format with depth and stencil, or NV12's chroma plane.
    pub fn second_region(&self, sub: u32) -> Option<(TextureRegion<'_, B>, Extent, Second)> {
        if let Some(chroma) = &self.chroma {
            let (region, extent) = chroma.whole(sub)?;
            return Some((region, extent, Second::Chroma));
        }
        if self.format.kind() != FormatKind::DepthStencil {
            return None;
        }
        let (mut region, extent) = self.whole(sub)?;
        region.stencil = true;
        Some((
            region,
            extent,
            Second::Stencil(self.format.block().2 as usize),
        ))
    }

    /// Bytes of a row, rows of a slice, and slices of mip `mip`, tightly packed as the program
    /// sees them (NV12's chroma rows after its luma rows).
    pub fn packed(&self, mip: u32) -> (u32, u32, u32) {
        let e = self.mip_extent(mip);
        let (row, rows) =
            crate::format::subresource_rows(self.desc.format, self.format, e.width, e.height);
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
    /// The texture of the view's format the view is made on, where it reinterprets the texture's
    /// bits; its levels and layers are copied in before each use and out after each write
    /// (`ReinterpretCopy`).
    pub twin: Option<Kept<B, B::Texture>>,
    /// The twin holds only the view's slices of its 3D level, as the twin's one level from its
    /// first slice (a storage view of some slices, which no back end's view sees).
    pub window: bool,
    /// A target view of a buffer: the buffer and the byte its first element is at, whose
    /// elements the view's one-row texture holds, copied in at a pass's start and out at its end
    /// (`ReinterpretCopy`).
    pub buffer: Option<(Rc<Buffer<B>>, u64)>,
    /// A depth-stencil view's stencil aspect, read by pixel programs running their forced early
    /// tests themselves; made at the first such draw.
    pub stencil_aspect: RefCell<Option<Kept<B, B::TextureView>>>,
    /// A drawn view's layers as views of one layer each, which a geometry shader choosing a layer
    /// draws into one run at a time (`LayerSplit`); made at the first such draw.
    pub layers: RefCell<Vec<Rc<View<B>>>>,
}

impl<B: Backend> View<B> {
    /// Whether the view reads stencil (`X24_TYPELESS_G8_UINT`, `X32_TYPELESS_G8X24_UINT`).
    pub fn reads_stencil(&self) -> bool {
        use wcontainers_directx_graphics_infrastructure::dxgi;
        matches!(
            self.desc.format,
            dxgi::X32_TYPELESS_G8X24_UINT | dxgi::X24_TYPELESS_G8_UINT
        )
    }

    /// The kind of values programs read through the view: unsigned integers for a stencil view.
    pub fn read_kind(&self) -> FormatKind {
        if self.reads_stencil() {
            FormatKind::Uint
        } else {
            self.format.kind()
        }
    }

    /// Whether what is drawn into the view is copied out of the texture the pass draws into.
    pub fn copied(&self) -> bool {
        self.twin.is_some() || self.buffer.is_some()
    }

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
    pub texel: Option<TexelPlace>,
}

/// A typed buffer view's format, the byte of its first element in its bound range and its element
/// count.
pub type TexelPlace = (Format, u32, u32);

pub struct Sampler<B: Backend> {
    pub desc: SamplerDesc,
    pub gpu: Kept<B, B::Sampler>,
    /// What `gpu` was made from.
    pub made: GpuSampler,
    /// The same sampler comparing where `gpu` does not, or not where it does, for a program that reads
    /// it the other way; made at the first such draw.
    pub twin: RefCell<Option<Kept<B, B::Sampler>>>,
    /// The same sampler with its minimum level raised, by that minimum's bits, for textures under a
    /// minimum level-of-detail clamp; made at the first draw through each.
    pub clamped: RefCell<Vec<ClampedSampler<B>>>,
}

pub struct Blend<B: Backend> {
    pub desc: BlendDesc,
    pub gpu: Kept<B, B::Blend>,
    /// Without the back end's per-target blends: each group of targets that agree (a mask of
    /// target slots) and the state all of them are drawn with, in a run of their own.
    pub groups: Vec<(u8, Rc<Blend<B>>)>,
}

pub struct DepthStencil<B: Backend> {
    pub desc: DepthStencilDesc,
    pub gpu: Kept<B, B::DepthStencil>,
    /// The state writing nothing in the aspects of a read-only depth-stencil view, by the view's
    /// flags; made at the first draw through each.
    pub read_only: RefCell<Vec<(u32, Rc<DepthStencil<B>>)>>,
}

/// A sampler made with its minimum level raised, by that minimum's bits.
pub type ClampedSampler<B> = (u32, Kept<B, <B as Backend>::Sampler>);

/// A raster block made as one twin key (`context::RasterKey`).
pub type MaskedRaster<B> = ((u32, bool, bool), Kept<B, <B as Backend>::Raster>);

pub struct Rasterizer<B: Backend> {
    pub desc: RasterizerDesc,
    pub gpu: Kept<B, B::Raster>,
    /// The block made with each sample mask leaving out samples, which Direct3D sets with the
    /// blend state and the back ends keep in the raster block; made at a mask's first draw.
    pub masked: RefCell<Vec<MaskedRaster<B>>>,
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
    /// The elements the back end fetches.
    pub elements: Vec<LayoutElement>,
    /// The elements the vertex program fetches: of slots stepping by instances at a rate the back
    /// end cannot step by, and of 8- and 16-bit integers the shader reads with the other
    /// signedness, whose bits the back end's formats would not keep.
    pub pulled: Vec<LayoutElement>,
    /// The slow paths a draw fetching `pulled` takes.
    pub pulled_paths: Vec<wcontainers_gpu::SlowPath>,
    /// Every element, as the vertex stage fetches them behind a geometry shader or stream output.
    pub all: Vec<LayoutElement>,
    /// Per input slot: none (per vertex) or the instance step rate.
    pub instance_steps: [Option<u32>; 32],
}

pub struct Shader<B: Backend> {
    pub gpu: Kept<B, B::Shader>,
    /// The content key of the module `gpu` was made from (the cache's).
    pub key: [u8; 32],
    pub reflection: Reflection,
    /// The bytes each constant buffer the program reads is declared with, by binding number.
    pub constant_sizes: Vec<(u32, u64)>,
    /// What stands in for an empty slot of each texture and sampler it reads, by binding number
    /// (`None` where nothing does yet).
    pub stand_ins: Vec<(u32, Option<StandIn>)>,
    /// The bytecode, converted again for each variant a draw asks for.
    pub bytecode: Vec<u8>,
    /// The sampler slots the program reads, one bit each.
    pub samplers: u16,
    /// The texture slots the program reads, one bit each.
    pub textures: u128,
    /// The sampler slots it gathers, compares or offsets through, which it cannot mirror once
    /// around: through a mirror-once sampler the back end lacks it filters itself.
    pub mirror_filtered: u16,
    /// The program reads the targets' sample count, so each draw converts it for theirs.
    pub reads_samples: bool,
    /// Where the plain program reads its helper constants.
    pub helpers: Rc<crate::shader::Helpers>,
    /// The typed views it declares, whose formats each draw converts it for.
    pub typed: Vec<crate::shader::TypedView>,
    /// The texture slots it declares as multisampled arrays, a bit each.
    pub layered: u128,
    /// The constant buffer slots it indexes at run time, a bit each.
    pub indexed_constants: u16,
    /// The clip distances it writes where the back end has none, which its programs apply
    /// themselves (`ClipDistanceInShader`): the location of the values it adds and their count.
    pub clip: Option<(u32, u8)>,
}

/// A geometry shader object: the geometry stage and the stream output of its declaration. Made
/// from a vertex shader's bytecode it has no stage of its own, and streams the vertex stage's
/// output.
pub struct Geometry<B: Backend> {
    pub id: u64,
    /// The stage's slots and its pass-through vertex program (`gpu`), with its conversion.
    pub stage: Option<(Rc<Shader<B>>, wcontainers_shaders::Converted)>,
    pub stream: Option<wcontainers_shaders::stage::StreamOutput>,
    /// Stream 0 is drawn.
    pub drawn: bool,
}

/// What a draw binds for an empty texture or sampler slot its shader reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandIn {
    /// A view of zeros of this dimension and kind, multisampled or not.
    Zeros(ViewDimension, FormatKind, bool),
    /// Direct3D's default sampler state.
    DefaultSampler,
    /// A buffer of zeros read as a structured or raw buffer.
    ZeroBuffer,
}
