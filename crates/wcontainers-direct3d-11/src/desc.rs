//! Direct3D 11's descriptions as plain data, with the API's own numbers (as `d3d11.h` defines them), so
//! the Windows shell and the call buffer pass them through unchanged.

/// `D3D11_USAGE`.
pub mod usage {
    pub const DEFAULT: u32 = 0;
    pub const IMMUTABLE: u32 = 1;
    pub const DYNAMIC: u32 = 2;
    pub const STAGING: u32 = 3;
}

/// `D3D11_BIND_FLAG`.
pub mod bind {
    pub const VERTEX_BUFFER: u32 = 0x1;
    pub const INDEX_BUFFER: u32 = 0x2;
    pub const CONSTANT_BUFFER: u32 = 0x4;
    pub const SHADER_RESOURCE: u32 = 0x8;
    pub const STREAM_OUTPUT: u32 = 0x10;
    pub const RENDER_TARGET: u32 = 0x20;
    pub const DEPTH_STENCIL: u32 = 0x40;
    pub const UNORDERED_ACCESS: u32 = 0x80;
}

/// `D3D11_CPU_ACCESS_FLAG`.
pub mod cpu {
    pub const WRITE: u32 = 0x10000;
    pub const READ: u32 = 0x20000;
}

/// `D3D11_MAP`.
pub mod map {
    pub const READ: u32 = 1;
    pub const WRITE: u32 = 2;
    pub const READ_WRITE: u32 = 3;
    pub const WRITE_DISCARD: u32 = 4;
    pub const WRITE_NO_OVERWRITE: u32 = 5;
    /// `D3D11_MAP_FLAG_DO_NOT_WAIT`.
    pub const DO_NOT_WAIT: u32 = 0x100000;
}

/// `D3D11_CLEAR_FLAG`.
pub mod clear {
    pub const DEPTH: u32 = 1;
    pub const STENCIL: u32 = 2;
}

/// `D3D11_FORMAT_SUPPORT` bits.
pub mod support {
    pub use wcontainers_directx_graphics_infrastructure::support1::*;
    pub const CPU_LOCKABLE: u32 = 0x20000;
}

/// `D3D_FEATURE_LEVEL`.
pub mod level {
    pub const L9_1: u32 = 0x9100;
    pub const L9_2: u32 = 0x9200;
    pub const L9_3: u32 = 0x9300;
    pub const L10_0: u32 = 0xa000;
    pub const L10_1: u32 = 0xa100;
    pub const L11_0: u32 = 0xb000;
    pub const L11_1: u32 = 0xb100;
}

/// `D3D11_BUFFER_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BufferDesc {
    pub byte_width: u32,
    pub usage: u32,
    pub bind_flags: u32,
    pub cpu_access_flags: u32,
    pub misc_flags: u32,
    pub structure_byte_stride: u32,
}

/// A texture's dimension, as `D3D11_RESOURCE_DIMENSION` names it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TexDim {
    D1,
    #[default]
    D2,
    D3,
}

/// `D3D11_TEXTURE1D_DESC`, `D3D11_TEXTURE2D_DESC` and `D3D11_TEXTURE3D_DESC` as one: `height` is 1
/// for 1D, `depth` 1 for 1D and 2D, `array_size` 1 for 3D, and the sample description 1 and 0 for
/// 1D and 3D.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureDesc {
    pub dimension: TexDim,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_levels: u32,
    pub array_size: u32,
    pub format: u32,
    pub sample_count: u32,
    pub sample_quality: u32,
    pub usage: u32,
    pub bind_flags: u32,
    pub cpu_access_flags: u32,
    pub misc_flags: u32,
}

/// `D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS`, `_BUFFER_ALLOW_RAW_VIEWS` and `_BUFFER_STRUCTURED`.
pub const MISC_DRAWINDIRECT_ARGS: u32 = 0x10;
pub const MISC_ALLOW_RAW_VIEWS: u32 = 0x20;
pub const MISC_STRUCTURED: u32 = 0x40;
/// `D3D11_BUFFER_UAV_FLAG`.
pub mod buffer_uav {
    pub const RAW: u32 = 0x1;
    pub const APPEND: u32 = 0x2;
    pub const COUNTER_FLAG: u32 = 0x4;
}
/// `D3D11_RESOURCE_MISC_GENERATE_MIPS`.
pub const MISC_GENERATE_MIPS: u32 = 0x1;
/// `D3D11_RESOURCE_MISC_TEXTURECUBE`.
pub const MISC_TEXTURECUBE: u32 = 0x4;
/// Shareable with other devices; the sharing itself (`GetSharedHandle`) is not served.
pub const MISC_SHARED: u32 = 0x2;
/// `D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX`: shared with a keyed mutex (not served).
pub const MISC_SHARED_KEYEDMUTEX: u32 = 0x100;
/// `D3D11_RESOURCE_MISC_SHARED_NTHANDLE`: shared through an NT handle (not served).
pub const MISC_SHARED_NTHANDLE: u32 = 0x800;
/// `D3D11_RESOURCE_MISC_GDI_COMPATIBLE`: GDI draws into it through `IDXGISurface1::GetDC`.
pub const MISC_GDI_COMPATIBLE: u32 = 0x200;
/// `D3D11_RESOURCE_MISC_RESOURCE_CLAMP`: the texture takes `SetResourceMinLOD`.
pub const MISC_RESOURCE_CLAMP: u32 = 0x80;
/// `D3D11_STANDARD_MULTISAMPLE_PATTERN` and `D3D11_CENTER_MULTISAMPLE_PATTERN`, sample qualities.
pub const STANDARD_MULTISAMPLE_PATTERN: u32 = 0xffff_ffff;
pub const CENTER_MULTISAMPLE_PATTERN: u32 = 0xffff_fffe;

/// One subresource of initial data (`D3D11_SUBRESOURCE_DATA`), its bytes already bounded.
#[derive(Clone, Copy, Debug)]
pub struct SubresourceData<'a> {
    pub bytes: &'a [u8],
    pub row_pitch: u32,
    pub slice_pitch: u32,
}

/// The part of a texture a view sees: `D3D11_*_VIEW_DESC` for the texture dimensions. Layers are
/// a 3D view's depth slices (a render target's `FirstWSlice` and `WSize`), and a cube array's
/// layers count faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewDesc {
    pub format: u32,
    pub dimension: ViewDim,
    pub first_mip: u32,
    pub mip_count: u32,
    pub first_layer: u32,
    pub layer_count: u32,
    /// A depth-stencil view's `D3D11_DSV_FLAG`s: the aspects it only reads, so shader views may
    /// read them while it is bound. Zero for other views.
    pub read_only: u32,
}

/// `D3D11_DSV_READ_ONLY_DEPTH` and `D3D11_DSV_READ_ONLY_STENCIL`.
pub const DSV_READ_ONLY_DEPTH: u32 = 0x1;
pub const DSV_READ_ONLY_STENCIL: u32 = 0x2;

/// The view dimensions this front serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewDim {
    Texture1D,
    Texture1DArray,
    Texture2D,
    Texture2DArray,
    Texture3D,
    TextureCube,
    TextureCubeArray,
    Texture2DMS,
    Texture2DMSArray,
}

/// `D3D11_SAMPLER_DESC`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplerDesc {
    pub filter: u32,
    pub address: [u32; 3],
    pub mip_lod_bias: f32,
    pub max_anisotropy: u32,
    pub comparison: u32,
    pub border: [f32; 4],
    pub min_lod: f32,
    pub max_lod: f32,
}

impl SamplerDesc {
    /// What an empty sampler slot samples with: linear filtering, clamped addressing.
    pub fn default_state() -> Self {
        SamplerDesc {
            filter: 0x15,
            address: [3; 3],
            mip_lod_bias: 0.0,
            max_anisotropy: 1,
            comparison: 1,
            border: [1.0; 4],
            min_lod: -f32::MAX,
            max_lod: f32::MAX,
        }
    }
}

/// `D3D11_RENDER_TARGET_BLEND_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TargetBlend {
    pub enable: bool,
    pub src: u32,
    pub dst: u32,
    pub op: u32,
    pub src_alpha: u32,
    pub dst_alpha: u32,
    pub op_alpha: u32,
    pub write_mask: u8,
}

/// `D3D11_BLEND_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlendDesc {
    pub alpha_to_coverage: bool,
    pub independent: bool,
    pub targets: [TargetBlend; 8],
}

impl BlendDesc {
    /// Whether a target that blends reads the pixel shader's second output (`D3D11_BLEND_SRC1_*`).
    pub fn dual_source(&self) -> bool {
        self.targets.iter().filter(|t| t.enable).any(|t| {
            [t.src, t.dst, t.src_alpha, t.dst_alpha]
                .iter()
                .any(|f| (16..=19).contains(f))
        })
    }

    /// What a blend state of `None` means: no blending, every channel written.
    pub fn default_state() -> Self {
        let target = TargetBlend {
            enable: false,
            src: 2,
            dst: 1,
            op: 1,
            src_alpha: 2,
            dst_alpha: 1,
            op_alpha: 1,
            write_mask: 0xf,
        };
        BlendDesc {
            alpha_to_coverage: false,
            independent: false,
            targets: [target; 8],
        }
    }
}

/// `D3D11_DEPTH_STENCILOP_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StencilFaceDesc {
    pub fail: u32,
    pub depth_fail: u32,
    pub pass: u32,
    pub func: u32,
}

/// `D3D11_DEPTH_STENCIL_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DepthStencilDesc {
    pub depth_enable: bool,
    pub depth_write: bool,
    pub depth_func: u32,
    pub stencil_enable: bool,
    pub stencil_read_mask: u8,
    pub stencil_write_mask: u8,
    pub front: StencilFaceDesc,
    pub back: StencilFaceDesc,
}

impl DepthStencilDesc {
    /// What a depth-stencil state of `None` means.
    pub fn default_state() -> Self {
        let face = StencilFaceDesc {
            fail: 1,
            depth_fail: 1,
            pass: 1,
            func: 8,
        };
        DepthStencilDesc {
            depth_enable: true,
            depth_write: true,
            depth_func: 2,
            stencil_enable: false,
            stencil_read_mask: 0xff,
            stencil_write_mask: 0xff,
            front: face,
            back: face,
        }
    }
}

/// `D3D11_RASTERIZER_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RasterizerDesc {
    pub fill: u32,
    pub cull: u32,
    pub front_counter_clockwise: bool,
    pub depth_bias: i32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub depth_clip: bool,
    pub scissor: bool,
    pub multisample: bool,
    pub antialiased_lines: bool,
}

impl RasterizerDesc {
    /// What a rasterizer state of `None` means.
    pub fn default_state() -> Self {
        RasterizerDesc {
            fill: 3,
            cull: 3,
            depth_clip: true,
            ..Default::default()
        }
    }
}

/// `D3D11_APPEND_ALIGNED_ELEMENT`.
pub const APPEND_ALIGNED: u32 = u32::MAX;

/// `D3D11_INPUT_ELEMENT_DESC`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputElement {
    pub semantic: String,
    pub semantic_index: u32,
    pub format: u32,
    pub slot: u32,
    pub offset: u32,
    pub per_instance: bool,
    pub step_rate: u32,
}

/// `D3D11_VIEWPORT`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

/// `D3D11_BOX`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Box3 {
    pub left: u32,
    pub top: u32,
    pub front: u32,
    pub right: u32,
    pub bottom: u32,
    pub back: u32,
}

impl Box3 {
    /// Whether the box holds nothing in some dimension, which makes Direct3D drop the call.
    pub fn empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top || self.back <= self.front
    }
}

/// `D3D11_PRIMITIVE_TOPOLOGY`.
pub mod topology {
    pub const UNDEFINED: u32 = 0;
    pub const POINTLIST: u32 = 1;
    pub const LINELIST: u32 = 2;
    pub const LINESTRIP: u32 = 3;
    pub const TRIANGLELIST: u32 = 4;
    pub const TRIANGLESTRIP: u32 = 5;
    pub const LINELIST_ADJ: u32 = 10;
    pub const LINESTRIP_ADJ: u32 = 11;
    pub const TRIANGLELIST_ADJ: u32 = 12;
    pub const TRIANGLESTRIP_ADJ: u32 = 13;
    /// The first of the 32 patch list topologies, of one control point.
    pub const PATCHLIST_1: u32 = 33;
}
