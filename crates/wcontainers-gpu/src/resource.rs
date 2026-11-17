//! Buffers, textures, views, samplers, binding sets, queries, and how bytes move in and out of them.

use crate::backend::Backend;
use crate::format::Format;
use crate::shader::StageSet;
use bitflags::bitflags;

bitflags! {
    /// How a buffer will be used. Every combination is valid; a back end whose API cannot hold one in one
    /// object builds it from several. WebGL 2, where one buffer never holds both indices and other data,
    /// keeps a buffer that is both `INDEX` and anything but `COPY_SRC`, `COPY_DST` and `READ_BACK` as two
    /// GL buffers, each made when first used its way, every write from the host made to both and every
    /// GPU write to the one it can reach (slow paths [`crate::SlowPath::IndexBufferTwin`],
    /// [`crate::SlowPath::IndexTwinReadBack`]; plan section 4.6).
    ///
    /// What each use needs: `COPY_DST` to be written ([`crate::Backend::write_buffer`], a copy's
    /// destination, [`crate::Backend::clear_buffer`], [`crate::Backend::resolve_queries`]); `COPY_SRC` to
    /// be read (a copy's source, [`crate::Backend::read_back`]); `VERTEX`, `INDEX` and `INDIRECT` for a
    /// draw's streams, indices and arguments; `UNIFORM` and `STORAGE` for those bindings; `CAPTURE` for
    /// [`crate::Backend::capture_vertices`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
    pub struct BufferUsage: u16 {
        const VERTEX = 1 << 0;
        const INDEX = 1 << 1;
        const UNIFORM = 1 << 2;
        const STORAGE = 1 << 3;
        const INDIRECT = 1 << 4;
        const COPY_SRC = 1 << 5;
        const COPY_DST = 1 << 6;
        /// The buffer is a target of [`crate::Backend::capture_vertices`].
        const CAPTURE = 1 << 7;
        /// Results are read back to guest memory.
        const READ_BACK = 1 << 8;
    }
}

/// How the guest will change a buffer's contents. Back ends choose where the buffer lives from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapIntent {
    /// Written once or rarely (static vertex data, textures' uploads).
    Rarely,
    /// Rewritten often, usually whole, usually every frame (Direct3D's dynamic buffers).
    Often,
}

/// A buffer to create.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferDesc {
    pub size: u64,
    pub usage: BufferUsage,
    pub intent: MapIntent,
}

/// What a write promises about the bytes already in the buffer. This is how the mapping rules of
/// Direct3D, OpenGL and Vulkan reach a back end without any buffer being mapped across the boundary.
/// A texture write ([`crate::Backend::write_texture`]) takes the same modes: the whole texture for the
/// whole buffer (every level and layer), its texels for bytes (both aspects of a depth and stencil
/// format).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WriteMode {
    /// The write takes effect between the commands recorded before it and those after it, exactly as
    /// if the GPU had stopped to take it. The general case, and the slowest where a back end must order
    /// it inside a pass.
    Ordered,
    /// The whole buffer's old contents are dropped; commands recorded earlier still see the old bytes.
    /// A back end may give the buffer fresh storage (renaming), so nothing waits. The write may cover
    /// only part of the buffer: the buffer keeps its size and the bytes outside the part are undefined
    /// until written (WebGL 2 orphans with `bufferData(size, null)`, then writes the part).
    Discard,
    /// The caller promises that no command recorded earlier and not yet finished, and no `Ordered`
    /// write made since the caller last waited, reads or writes the bytes written. A back end may
    /// write in place at once, ahead of the commands recorded before it; it keeps the order of these
    /// writes among themselves and with `Discard` writes.
    NoOverwrite,
}

bitflags! {
    /// How a texture will be used. What each use needs: `COPY_DST` to be written
    /// ([`crate::Backend::write_texture`], a copy's destination); `COPY_SRC` to be read (a copy's
    /// source, [`crate::Backend::read_back`]); `SAMPLE` and `STORAGE` for those bindings; `RENDER` for
    /// an attachment or resolve target, and so for both sides of [`crate::Backend::resolve`],
    /// [`crate::Backend::clear_texture`],
    /// [`crate::Backend::clear_rects`] and [`crate::Backend::generate_mips`] (which also samples);
    /// `PRESENT` for a surface's texture.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
    pub struct TextureUsage: u16 {
        const SAMPLE = 1 << 0;
        const RENDER = 1 << 1;
        const STORAGE = 1 << 2;
        const COPY_SRC = 1 << 3;
        const COPY_DST = 1 << 4;
        /// Shown on screen through [`crate::Backend::present`].
        const PRESENT = 1 << 5;
    }
}

/// The shape of a texture. There is no 1D shape: fronts make an API's 1D textures as 2D textures one
/// texel high, and the shaders crate reads them as such, so every back end serves them with mipmaps,
/// arrays and rendering (WebGPU's 1D textures have none of these).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureDimension {
    D2,
    D3,
}

/// A texture to create.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureDesc {
    pub dimension: TextureDimension,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    /// Depth for 3D textures, array layers otherwise (6 per cube).
    pub depth_or_layers: u32,
    pub mip_levels: u32,
    /// One, or a count [`crate::FormatCaps::takes_samples`] gives the format. A multisampled texture
    /// is 2D with one level and one layer, never a storage texture, and its bytes move only by a draw,
    /// a resolve, or a copy of the whole subresource into another of the same sample count: never to
    /// or from a buffer, by a write or by a read back (WebGPU's rules).
    pub samples: u32,
    pub usage: TextureUsage,
    /// The layers are cube faces, six per cube (Direct3D's `TEXTURECUBE` flag, Vulkan's cube-compatible
    /// images, OpenGL's cube map targets). With [`TextureDesc::own_view`] it fixes what a back end that
    /// must choose a texture's kind at creation (WebGL 2) makes.
    pub cube: bool,
    /// Views may use other colour formats of the same block size (Direct3D's typeless textures). Views
    /// as the sRGB twin are always allowed. Without [`crate::Features::FORMAT_REINTERPRET`], the front copies
    /// into a texture of the other format instead (slow path [`crate::SlowPath::ReinterpretCopy`]).
    pub reinterpret: bool,
}

impl TextureDesc {
    /// The dimension a view of the whole texture has: `Cube` for a cube texture of six layers,
    /// `CubeArray` for more, `D2Array` for a 2D texture of several layers, otherwise `D2` or `D3`.
    pub fn own_view(&self) -> ViewDimension {
        match (self.dimension, self.cube, self.depth_or_layers) {
            (TextureDimension::D3, _, _) => ViewDimension::D3,
            (TextureDimension::D2, true, 6) => ViewDimension::Cube,
            (TextureDimension::D2, true, _) => ViewDimension::CubeArray,
            (TextureDimension::D2, false, 1) => ViewDimension::D2,
            (TextureDimension::D2, false, _) => ViewDimension::D2Array,
        }
    }
}

/// How a view sees a texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewDimension {
    D2,
    D2Array,
    Cube,
    CubeArray,
    D3,
}

/// A view of part of a texture, as a front's shader resource, render target, depth or storage view.
///
/// Every back end makes, without further features: views to draw into of one mip level and one layer
/// (or one slice of a 3D level, or all layers); and views to sample or store into of a mip range of all
/// layers in the texture's own dimension ([`TextureDesc::own_view`]) and own format or its sRGB twin.
/// Anything else (another dimension, such as one layer of an array or a cube seen as a 2D array; a layer
/// range; another format of the same size) needs [`crate::Features::TEXTURE_VIEWS`] or
/// [`crate::Features::FORMAT_REINTERPRET`]. Without them the front keeps a second texture made in the
/// view's own shape and format, copied from the first whenever that changed since the last copy (slow
/// path [`crate::SlowPath::ReinterpretCopy`]). Without [`crate::Features::TEXTURE_VIEWS`] a draw also
/// binds each texture through one mip range only (WebGL 2 sets the range on the texture itself); a second
/// range in the same draw comes from the second texture too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureViewDesc {
    pub format: Format,
    pub dimension: ViewDimension,
    pub base_mip: u32,
    pub mip_count: u32,
    pub base_layer: u32,
    pub layer_count: u32,
    /// Which of depth and stencil a depth-stencil view reads: true for stencil, read as unsigned
    /// integers. Every back end serves both aspects; one whose API cannot sample stencil fills an 8-bit
    /// texture from the stencil by draws when the stencil changed (slow path
    /// [`crate::SlowPath::StencilCopiedByDraw`]). A view of a format with both aspects that a pass
    /// draws into ([`crate::DepthAttachment`]) holds both whatever this says.
    pub stencil: bool,
}

/// Texture filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Filter {
    Nearest,
    Linear,
}

/// What a sampler does outside the texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddressMode {
    Repeat,
    Mirror,
    Clamp,
    /// Needs [`crate::Features::BORDER_COLOR`] for colors other than transparent black. Otherwise the
    /// front binds a clamping sampler and a program variant applies the border (slow path
    /// [`crate::SlowPath::BorderInShader`], [`crate::VariantKey::border_in_shader`]).
    Border,
    /// Mirrored once about zero, then clamped. Needs [`crate::Features::MIRROR_ONCE`]; otherwise the
    /// front binds a clamping sampler and a program variant samples the absolute coordinate with the
    /// original gradients, which is exact (slow path [`crate::SlowPath::MirrorOnceInShader`]).
    MirrorOnce,
}

/// The color outside the texture for [`AddressMode::Border`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BorderColor {
    TransparentBlack,
    OpaqueBlack,
    OpaqueWhite,
    /// Any other color, as raw f32 bits.
    Custom([u32; 4]),
}

/// A comparison, for depth and stencil tests and comparison samplers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CompareFn {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

/// A sampler to create.
///
/// There is no level-of-detail bias: neither WebGPU nor WebGL 2 samplers have one. A front whose API
/// sets one (Direct3D's `MipLODBias`, OpenGL's `TEXTURE_LOD_BIAS`, Vulkan's `mipLodBias`) passes it in the
/// program's helper constants and binds the variant that adds it to every sampling of that sampler (slow
/// path [`crate::SlowPath::LodBiasInShader`], [`crate::VariantKey::lod_bias`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    pub min: Filter,
    pub mag: Filter,
    pub mip: Filter,
    pub address: [AddressMode; 3],
    /// Smallest and largest level of detail, as f32 bits.
    pub lod_range: [u32; 2],
    pub anisotropy: u16,
    pub compare: Option<CompareFn>,
    pub border: BorderColor,
}

/// Part of a buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferRange<H> {
    pub buffer: H,
    pub offset: u64,
    pub size: u64,
}

/// One resource in a binding set.
#[derive(Debug)]
pub enum BindingResource<'a, B: Backend> {
    /// The offset is a multiple of [`crate::Limits::min_uniform_offset_alignment`] and the size at most
    /// [`crate::Limits::max_uniform_buffer_size`]; a buffer range in a binding lies inside its buffer.
    /// A draw binds at least the size of the block the program declares at that binding (WebGPU's
    /// `minBindingSize`, WebGL 2's `UNIFORM_BLOCK_DATA_SIZE`); a front whose API binds less grows the
    /// buffer ([`crate::SlowPath::ConstantBufferGrown`]).
    UniformBuffer(BufferRange<&'a B::Buffer>),
    /// The offset is a multiple of [`crate::Limits::min_storage_offset_alignment`] and the size at most
    /// [`crate::Limits::max_storage_buffer_size`], and at least the size of the program's block, a
    /// runtime-sized array counted as one element. A front whose API
    /// binds at any offset binds from the aligned offset below it and writes the rest into the program's
    /// helper constants ([`crate::HelperValue::StorageOffset`]), which the program adds to its addresses.
    StorageBuffer {
        range: BufferRange<&'a B::Buffer>,
        writable: bool,
    },
    Texture(&'a B::TextureView),
    StorageTexture {
        view: &'a B::TextureView,
        writable: bool,
    },
    /// A sampler that compares ([`crate::SamplerDesc::compare`] set) exactly where the program compares
    /// through that binding. One any of whose filters is linear is never paired in the program with a
    /// texture whose view's format lacks [`crate::FormatUse::FILTER`]; the front binds the program
    /// variant that filters that texture itself ([`crate::SlowPath::FilterInShader`]).
    Sampler(&'a B::Sampler),
    /// A buffer read as typed texels (Direct3D's typed buffer views, OpenGL's buffer textures), where
    /// [`crate::Features::TEXEL_BUFFERS`] is present.
    TexelBuffer {
        range: BufferRange<&'a B::Buffer>,
        format: Format,
        writable: bool,
    },
    /// A big resource table, where [`crate::Features::RESOURCE_TABLES`] is present.
    Table(&'a B::ResourceTable),
}

/// What a pipeline must know of a binding set beyond its program: which sampled textures (bit = binding
/// number minus 32) cannot be filtered on this back end. The zero key means every texture can be.
///
/// WebGPU builds layouts from it with the program (plan section 4.5): a float texture the program only
/// loads from, never pairing it with a sampler, is `"unfilterable-float"` whatever its view (which
/// every float view binds to, so such textures need no bit, wherever their binding number lies); a
/// texture whose bit is set is `"unfilterable-float"`, and a sampler is `"comparison"` where the program
/// compares with it,
/// `"non-filtering"` where the program pairs it with such a texture, and `"filtering"` otherwise (which
/// takes every sampler that does not compare). Back ends without layouts give zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BindingLayoutKey {
    pub unfilterable_textures: u128,
}

/// One slot of a binding set: which program binding it fills, and with what.
#[derive(Debug)]
pub struct BindingEntry<'a, B: Backend> {
    /// The binding number the shader converter gave this slot (see the plan, section 6).
    pub binding: u32,
    /// Stages that read it; back ends that bind per stage use this.
    pub stages: StageSet,
    pub resource: BindingResource<'a, B>,
}

/// Where in a texture a copy starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Origin {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

/// How much of a texture a copy covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
}

/// One subresource region of a texture.
#[derive(Debug)]
pub struct TextureRegion<'a, B: Backend> {
    pub texture: &'a B::Texture,
    pub mip: u32,
    /// First layer for array textures, ignored for 3D (use `origin.z`).
    pub layer: u32,
    pub origin: Origin,
    /// For a format with both depth and stencil, the aspect a buffer copy or read back moves: the
    /// stencil byte, or depth in four bytes (32-bit float, or 24-bit unorm in the low bits of a word).
    /// A copy between textures moves both aspects and names neither. Only such formats name stencil.
    pub stencil: bool,
}

/// The buffer side of a copy between a buffer and a texture. A block's size here is that of the
/// aspect moved for a format with depth and stencil (one byte of stencil, four of depth).
#[derive(Debug)]
pub struct CopyBufferTexture<'a, B: Backend> {
    pub buffer: &'a B::Buffer,
    /// Where the first block is, any multiple of the block size (as Vulkan and WebGPU require). A
    /// back end whose API needs more (WebGPU: multiples of 4 for depth and stencil) copies the rows
    /// through a buffer ([`crate::SlowPath::RowsRepacked`]).
    pub offset: u64,
    /// Bytes from one row of blocks to the next, any multiple of the block size. A back end whose API
    /// needs larger pitches (WebGPU: multiples of 256) copies the rows through a buffer at its own pitch
    /// ([`crate::SlowPath::RowsRepacked`]).
    pub row_pitch: u32,
    /// Rows of blocks from one layer or slice to the next.
    pub rows_per_layer: u32,
}

/// What a read back reads.
#[derive(Debug)]
pub enum ReadSource<'a, B: Backend> {
    Buffer {
        buffer: &'a B::Buffer,
        offset: u64,
        size: u64,
    },
    /// Rows land tightly packed in guest memory, whatever alignment the back end copies with.
    Texture {
        region: TextureRegion<'a, B>,
        extent: Extent,
    },
}

/// Whether a read back has finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadState {
    Pending,
    Ready,
}

/// Kinds of query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueryKind {
    /// Zero when no sample passed every per-fragment test, otherwise non-zero. Every back end has it.
    OcclusionAny,
    /// The exact number of samples that passed every per-fragment test. Needs
    /// [`crate::Features::OCCLUSION_COUNT`]; without it fronts count by replaying the query's draws into
    /// a counting target (plan section 4.12, slow path [`crate::SlowPath::OcclusionCountReplay`]).
    Occlusion,
    /// GPU time in nanoseconds, where [`crate::Features::TIMESTAMPS`] is present;
    /// [`crate::Backend::timestamps_disjoint`] says when a stretch of them is meaningless.
    Timestamp,
}
