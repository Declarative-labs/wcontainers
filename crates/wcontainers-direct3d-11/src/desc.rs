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

/// `D3D_FEATURE_LEVEL`.
/// `D3D11_FORMAT_SUPPORT` bits.
pub mod support {
    pub const IA_VERTEX_BUFFER: u32 = 0x2;
    pub const IA_INDEX_BUFFER: u32 = 0x4;
    pub const TEXTURE2D: u32 = 0x20;
    pub const TEXTURECUBE: u32 = 0x80;
    pub const SHADER_LOAD: u32 = 0x100;
    pub const SHADER_SAMPLE: u32 = 0x200;
    pub const MIP: u32 = 0x1000;
    pub const RENDER_TARGET: u32 = 0x4000;
    pub const BLENDABLE: u32 = 0x8000;
    pub const DEPTH_STENCIL: u32 = 0x10000;
    pub const CPU_LOCKABLE: u32 = 0x20000;
}

pub mod level {
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

/// `D3D11_TEXTURE2D_DESC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Texture2DDesc {
    pub width: u32,
    pub height: u32,
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

/// `D3D11_RESOURCE_MISC_TEXTURECUBE`.
pub const MISC_TEXTURECUBE: u32 = 0x4;

/// One subresource of initial data (`D3D11_SUBRESOURCE_DATA`), its bytes already bounded.
#[derive(Clone, Copy, Debug)]
pub struct SubresourceData<'a> {
    pub bytes: &'a [u8],
    pub row_pitch: u32,
    pub slice_pitch: u32,
}

/// The part of a texture a view sees: `D3D11_*_VIEW_DESC` for the 2D dimensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewDesc {
    pub format: u32,
    pub dimension: ViewDim,
    pub first_mip: u32,
    pub mip_count: u32,
    pub first_layer: u32,
    pub layer_count: u32,
}

/// The view dimensions this front serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewDim {
    Texture2D,
    Texture2DArray,
    TextureCube,
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
}
