//! Direct3D 11's structures as the front's descriptions, and back.

use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};
use windows::core::Result;

use wcontainers_direct3d_11::desc::*;

pub fn buffer(d: &D3D11_BUFFER_DESC) -> BufferDesc {
    BufferDesc {
        byte_width: d.ByteWidth,
        usage: d.Usage.0 as u32,
        bind_flags: d.BindFlags,
        cpu_access_flags: d.CPUAccessFlags,
        misc_flags: d.MiscFlags,
        structure_byte_stride: d.StructureByteStride,
    }
}

pub fn buffer_back(d: &BufferDesc) -> D3D11_BUFFER_DESC {
    D3D11_BUFFER_DESC {
        ByteWidth: d.byte_width,
        Usage: D3D11_USAGE(d.usage as i32),
        BindFlags: d.bind_flags,
        CPUAccessFlags: d.cpu_access_flags,
        MiscFlags: d.misc_flags,
        StructureByteStride: d.structure_byte_stride,
    }
}

pub fn texture1d(d: &D3D11_TEXTURE1D_DESC) -> TextureDesc {
    TextureDesc {
        dimension: TexDim::D1,
        width: d.Width,
        height: 1,
        depth: 1,
        mip_levels: d.MipLevels,
        array_size: d.ArraySize,
        format: d.Format.0 as u32,
        sample_count: 1,
        sample_quality: 0,
        usage: d.Usage.0 as u32,
        bind_flags: d.BindFlags,
        cpu_access_flags: d.CPUAccessFlags,
        misc_flags: d.MiscFlags,
    }
}

pub fn texture1d_back(d: &TextureDesc) -> D3D11_TEXTURE1D_DESC {
    D3D11_TEXTURE1D_DESC {
        Width: d.width,
        MipLevels: d.mip_levels,
        ArraySize: d.array_size,
        Format: DXGI_FORMAT(d.format as i32),
        Usage: D3D11_USAGE(d.usage as i32),
        BindFlags: d.bind_flags,
        CPUAccessFlags: d.cpu_access_flags,
        MiscFlags: d.misc_flags,
    }
}

pub fn texture2d(d: &D3D11_TEXTURE2D_DESC) -> TextureDesc {
    TextureDesc {
        dimension: TexDim::D2,
        width: d.Width,
        height: d.Height,
        depth: 1,
        mip_levels: d.MipLevels,
        array_size: d.ArraySize,
        format: d.Format.0 as u32,
        sample_count: d.SampleDesc.Count,
        sample_quality: d.SampleDesc.Quality,
        usage: d.Usage.0 as u32,
        bind_flags: d.BindFlags,
        cpu_access_flags: d.CPUAccessFlags,
        misc_flags: d.MiscFlags,
    }
}

pub fn texture2d_back(d: &TextureDesc) -> D3D11_TEXTURE2D_DESC {
    D3D11_TEXTURE2D_DESC {
        Width: d.width,
        Height: d.height,
        MipLevels: d.mip_levels,
        ArraySize: d.array_size,
        Format: DXGI_FORMAT(d.format as i32),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: d.sample_count,
            Quality: d.sample_quality,
        },
        Usage: D3D11_USAGE(d.usage as i32),
        BindFlags: d.bind_flags,
        CPUAccessFlags: d.cpu_access_flags,
        MiscFlags: d.misc_flags,
    }
}

pub fn texture3d(d: &D3D11_TEXTURE3D_DESC) -> TextureDesc {
    TextureDesc {
        dimension: TexDim::D3,
        width: d.Width,
        height: d.Height,
        depth: d.Depth,
        mip_levels: d.MipLevels,
        array_size: 1,
        format: d.Format.0 as u32,
        sample_count: 1,
        sample_quality: 0,
        usage: d.Usage.0 as u32,
        bind_flags: d.BindFlags,
        cpu_access_flags: d.CPUAccessFlags,
        misc_flags: d.MiscFlags,
    }
}

pub fn texture3d_back(d: &TextureDesc) -> D3D11_TEXTURE3D_DESC {
    D3D11_TEXTURE3D_DESC {
        Width: d.width,
        Height: d.height,
        Depth: d.depth,
        MipLevels: d.mip_levels,
        Format: DXGI_FORMAT(d.format as i32),
        Usage: D3D11_USAGE(d.usage as i32),
        BindFlags: d.bind_flags,
        CPUAccessFlags: d.cpu_access_flags,
        MiscFlags: d.misc_flags,
    }
}

/// A view's part of a texture from (dimension kind, the union's fields).
fn view(format: DXGI_FORMAT, dimension: ViewDim, mips: (u32, u32), layers: (u32, u32)) -> ViewDesc {
    ViewDesc {
        format: format.0 as u32,
        dimension,
        first_mip: mips.0,
        mip_count: mips.1,
        first_layer: layers.0,
        layer_count: layers.1,
    }
}

/// A view dimension this front does not serve: unknown is invalid, the others not served yet.
fn other_view(unknown: bool, what: &'static str) -> windows::core::Error {
    if unknown {
        E_INVALIDARG.into()
    } else {
        super::missing(what)
    }
}

pub fn rtv(d: &D3D11_RENDER_TARGET_VIEW_DESC) -> Result<ViewDesc> {
    let f = d.Format;
    unsafe {
        let u = &d.Anonymous;
        Ok(match d.ViewDimension {
            D3D11_RTV_DIMENSION_TEXTURE1D => {
                view(f, ViewDim::Texture1D, (u.Texture1D.MipSlice, 1), (0, 1))
            }
            D3D11_RTV_DIMENSION_TEXTURE1DARRAY => {
                let a = u.Texture1DArray;
                let layers = (a.FirstArraySlice, a.ArraySize);
                view(f, ViewDim::Texture1DArray, (a.MipSlice, 1), layers)
            }
            D3D11_RTV_DIMENSION_TEXTURE2D => {
                view(f, ViewDim::Texture2D, (u.Texture2D.MipSlice, 1), (0, 1))
            }
            D3D11_RTV_DIMENSION_TEXTURE2DARRAY => {
                let a = u.Texture2DArray;
                let layers = (a.FirstArraySlice, a.ArraySize);
                view(f, ViewDim::Texture2DArray, (a.MipSlice, 1), layers)
            }
            D3D11_RTV_DIMENSION_TEXTURE3D => {
                let a = u.Texture3D;
                view(
                    f,
                    ViewDim::Texture3D,
                    (a.MipSlice, 1),
                    (a.FirstWSlice, a.WSize),
                )
            }
            other => {
                return Err(other_view(
                    other == D3D11_RTV_DIMENSION_UNKNOWN,
                    "render target views of buffers and multisampled textures",
                ));
            }
        })
    }
}

pub fn rtv_back(v: &ViewDesc) -> D3D11_RENDER_TARGET_VIEW_DESC {
    let mut d = D3D11_RENDER_TARGET_VIEW_DESC {
        Format: DXGI_FORMAT(v.format as i32),
        ..Default::default()
    };
    let (mip, first, size) = (v.first_mip, v.first_layer, v.layer_count);
    match v.dimension {
        ViewDim::Texture1D => {
            d.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE1D;
            d.Anonymous.Texture1D = D3D11_TEX1D_RTV { MipSlice: mip };
        }
        ViewDim::Texture1DArray => {
            d.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE1DARRAY;
            d.Anonymous.Texture1DArray = D3D11_TEX1D_ARRAY_RTV {
                MipSlice: mip,
                FirstArraySlice: first,
                ArraySize: size,
            };
        }
        ViewDim::Texture2D => {
            d.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE2D;
            d.Anonymous.Texture2D = D3D11_TEX2D_RTV { MipSlice: mip };
        }
        ViewDim::Texture3D => {
            d.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE3D;
            d.Anonymous.Texture3D = D3D11_TEX3D_RTV {
                MipSlice: mip,
                FirstWSlice: first,
                WSize: size,
            };
        }
        _ => {
            d.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE2DARRAY;
            d.Anonymous.Texture2DArray = D3D11_TEX2D_ARRAY_RTV {
                MipSlice: mip,
                FirstArraySlice: first,
                ArraySize: size,
            };
        }
    }
    d
}

pub fn dsv(d: &D3D11_DEPTH_STENCIL_VIEW_DESC) -> Result<ViewDesc> {
    if d.Flags != 0 {
        return Err(super::missing("read-only depth-stencil views"));
    }
    let f = d.Format;
    unsafe {
        let u = &d.Anonymous;
        Ok(match d.ViewDimension {
            D3D11_DSV_DIMENSION_TEXTURE1D => {
                view(f, ViewDim::Texture1D, (u.Texture1D.MipSlice, 1), (0, 1))
            }
            D3D11_DSV_DIMENSION_TEXTURE1DARRAY => {
                let a = u.Texture1DArray;
                let layers = (a.FirstArraySlice, a.ArraySize);
                view(f, ViewDim::Texture1DArray, (a.MipSlice, 1), layers)
            }
            D3D11_DSV_DIMENSION_TEXTURE2D => {
                view(f, ViewDim::Texture2D, (u.Texture2D.MipSlice, 1), (0, 1))
            }
            D3D11_DSV_DIMENSION_TEXTURE2DARRAY => {
                let a = u.Texture2DArray;
                let layers = (a.FirstArraySlice, a.ArraySize);
                view(f, ViewDim::Texture2DArray, (a.MipSlice, 1), layers)
            }
            other => {
                return Err(other_view(
                    other == D3D11_DSV_DIMENSION_UNKNOWN,
                    "depth-stencil views of multisampled textures",
                ));
            }
        })
    }
}

pub fn dsv_back(v: &ViewDesc) -> D3D11_DEPTH_STENCIL_VIEW_DESC {
    let mut d = D3D11_DEPTH_STENCIL_VIEW_DESC {
        Format: DXGI_FORMAT(v.format as i32),
        ..Default::default()
    };
    let (mip, first, size) = (v.first_mip, v.first_layer, v.layer_count);
    match v.dimension {
        ViewDim::Texture1D => {
            d.ViewDimension = D3D11_DSV_DIMENSION_TEXTURE1D;
            d.Anonymous.Texture1D = D3D11_TEX1D_DSV { MipSlice: mip };
        }
        ViewDim::Texture1DArray => {
            d.ViewDimension = D3D11_DSV_DIMENSION_TEXTURE1DARRAY;
            d.Anonymous.Texture1DArray = D3D11_TEX1D_ARRAY_DSV {
                MipSlice: mip,
                FirstArraySlice: first,
                ArraySize: size,
            };
        }
        ViewDim::Texture2D => {
            d.ViewDimension = D3D11_DSV_DIMENSION_TEXTURE2D;
            d.Anonymous.Texture2D = D3D11_TEX2D_DSV { MipSlice: mip };
        }
        _ => {
            d.ViewDimension = D3D11_DSV_DIMENSION_TEXTURE2DARRAY;
            d.Anonymous.Texture2DArray = D3D11_TEX2D_ARRAY_DSV {
                MipSlice: mip,
                FirstArraySlice: first,
                ArraySize: size,
            };
        }
    }
    d
}

pub fn srv(d: &D3D11_SHADER_RESOURCE_VIEW_DESC) -> Result<ViewDesc> {
    use windows::Win32::Graphics::Direct3D::*;
    let f = d.Format;
    unsafe {
        let u = &d.Anonymous;
        Ok(match d.ViewDimension {
            D3D_SRV_DIMENSION_TEXTURE1D => {
                let t = u.Texture1D;
                view(
                    f,
                    ViewDim::Texture1D,
                    (t.MostDetailedMip, t.MipLevels),
                    (0, 1),
                )
            }
            D3D_SRV_DIMENSION_TEXTURE1DARRAY => {
                let t = u.Texture1DArray;
                let layers = (t.FirstArraySlice, t.ArraySize);
                view(
                    f,
                    ViewDim::Texture1DArray,
                    (t.MostDetailedMip, t.MipLevels),
                    layers,
                )
            }
            D3D_SRV_DIMENSION_TEXTURE2D => {
                let t = u.Texture2D;
                view(
                    f,
                    ViewDim::Texture2D,
                    (t.MostDetailedMip, t.MipLevels),
                    (0, 1),
                )
            }
            D3D_SRV_DIMENSION_TEXTURE2DARRAY => {
                let t = u.Texture2DArray;
                let layers = (t.FirstArraySlice, t.ArraySize);
                view(
                    f,
                    ViewDim::Texture2DArray,
                    (t.MostDetailedMip, t.MipLevels),
                    layers,
                )
            }
            D3D_SRV_DIMENSION_TEXTURE3D => {
                let t = u.Texture3D;
                view(
                    f,
                    ViewDim::Texture3D,
                    (t.MostDetailedMip, t.MipLevels),
                    (0, 1),
                )
            }
            D3D_SRV_DIMENSION_TEXTURECUBE => {
                let t = u.TextureCube;
                view(
                    f,
                    ViewDim::TextureCube,
                    (t.MostDetailedMip, t.MipLevels),
                    (0, 6),
                )
            }
            D3D_SRV_DIMENSION_TEXTURECUBEARRAY => {
                let t = u.TextureCubeArray;
                let faces = if t.NumCubes == u32::MAX {
                    u32::MAX
                } else {
                    t.NumCubes.saturating_mul(6)
                };
                let mips = (t.MostDetailedMip, t.MipLevels);
                view(
                    f,
                    ViewDim::TextureCubeArray,
                    mips,
                    (t.First2DArrayFace, faces),
                )
            }
            other => {
                return Err(other_view(
                    other == D3D_SRV_DIMENSION_UNKNOWN,
                    "shader resource views of buffers and multisampled textures",
                ));
            }
        })
    }
}

pub fn srv_back(v: &ViewDesc) -> D3D11_SHADER_RESOURCE_VIEW_DESC {
    use windows::Win32::Graphics::Direct3D::*;
    let mut d = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT(v.format as i32),
        ..Default::default()
    };
    let (top, mips, first, size) = (v.first_mip, v.mip_count, v.first_layer, v.layer_count);
    match v.dimension {
        ViewDim::Texture1D => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURE1D;
            d.Anonymous.Texture1D = D3D11_TEX1D_SRV {
                MostDetailedMip: top,
                MipLevels: mips,
            };
        }
        ViewDim::Texture1DArray => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURE1DARRAY;
            d.Anonymous.Texture1DArray = D3D11_TEX1D_ARRAY_SRV {
                MostDetailedMip: top,
                MipLevels: mips,
                FirstArraySlice: first,
                ArraySize: size,
            };
        }
        ViewDim::Texture2D => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURE2D;
            d.Anonymous.Texture2D = D3D11_TEX2D_SRV {
                MostDetailedMip: top,
                MipLevels: mips,
            };
        }
        ViewDim::Texture2DArray => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURE2DARRAY;
            d.Anonymous.Texture2DArray = D3D11_TEX2D_ARRAY_SRV {
                MostDetailedMip: top,
                MipLevels: mips,
                FirstArraySlice: first,
                ArraySize: size,
            };
        }
        ViewDim::Texture3D => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURE3D;
            d.Anonymous.Texture3D = D3D11_TEX3D_SRV {
                MostDetailedMip: top,
                MipLevels: mips,
            };
        }
        ViewDim::TextureCube => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURECUBE;
            d.Anonymous.TextureCube = D3D11_TEXCUBE_SRV {
                MostDetailedMip: top,
                MipLevels: mips,
            };
        }
        ViewDim::TextureCubeArray => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURECUBEARRAY;
            d.Anonymous.TextureCubeArray = D3D11_TEXCUBE_ARRAY_SRV {
                MostDetailedMip: top,
                MipLevels: mips,
                First2DArrayFace: first,
                NumCubes: size / 6,
            };
        }
    }
    d
}

/// The description Direct3D keeps: no anisotropy unless the filter is anisotropic, `NEVER` unless
/// it compares, and no border colour unless an address mode reads it.
pub fn sampler_stored(d: &D3D11_SAMPLER_DESC) -> D3D11_SAMPLER_DESC {
    let mut d = *d;
    let filter = d.Filter.0 as u32;
    if filter & 0x40 == 0 {
        d.MaxAnisotropy = 0;
    }
    if (filter >> 7) & 3 != 1 {
        d.ComparisonFunc = D3D11_COMPARISON_NEVER;
    }
    if ![d.AddressU, d.AddressV, d.AddressW].contains(&D3D11_TEXTURE_ADDRESS_BORDER) {
        d.BorderColor = [0.0; 4];
    }
    d
}

pub fn sampler(d: &D3D11_SAMPLER_DESC) -> SamplerDesc {
    SamplerDesc {
        filter: d.Filter.0 as u32,
        address: [
            d.AddressU.0 as u32,
            d.AddressV.0 as u32,
            d.AddressW.0 as u32,
        ],
        mip_lod_bias: d.MipLODBias,
        max_anisotropy: d.MaxAnisotropy,
        comparison: d.ComparisonFunc.0 as u32,
        border: d.BorderColor,
        min_lod: d.MinLOD,
        max_lod: d.MaxLOD,
    }
}

/// The description Direct3D keeps: a target that does not blend holds the default factors and
/// operations, and without independent blending every target is the first.
/// Each target's logic operation enable and operation (Direct3D 11.1).
pub type LogicOps = [(windows::core::BOOL, D3D11_LOGIC_OP); 8];

/// A Direct3D 11.1 blend description as Direct3D 11's and its logic operations.
pub fn blend_split(d: &D3D11_BLEND_DESC1) -> (D3D11_BLEND_DESC, LogicOps) {
    let mut out = D3D11_BLEND_DESC {
        AlphaToCoverageEnable: d.AlphaToCoverageEnable,
        IndependentBlendEnable: d.IndependentBlendEnable,
        ..Default::default()
    };
    let mut logic = [(false.into(), D3D11_LOGIC_OP_NOOP); 8];
    for ((o, l), t) in out
        .RenderTarget
        .iter_mut()
        .zip(&mut logic)
        .zip(&d.RenderTarget)
    {
        *o = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: t.BlendEnable,
            SrcBlend: t.SrcBlend,
            DestBlend: t.DestBlend,
            BlendOp: t.BlendOp,
            SrcBlendAlpha: t.SrcBlendAlpha,
            DestBlendAlpha: t.DestBlendAlpha,
            BlendOpAlpha: t.BlendOpAlpha,
            RenderTargetWriteMask: t.RenderTargetWriteMask,
        };
        *l = (t.LogicOpEnable, t.LogicOp);
    }
    if !d.IndependentBlendEnable.as_bool() {
        logic = [logic[0]; 8];
    }
    (out, logic)
}

/// A Direct3D 11.1 blend description from Direct3D 11's and its logic operations.
pub fn blend_joined(d: &D3D11_BLEND_DESC, logic: &LogicOps) -> D3D11_BLEND_DESC1 {
    let mut out = D3D11_BLEND_DESC1 {
        AlphaToCoverageEnable: d.AlphaToCoverageEnable,
        IndependentBlendEnable: d.IndependentBlendEnable,
        ..Default::default()
    };
    for ((o, l), t) in out.RenderTarget.iter_mut().zip(logic).zip(&d.RenderTarget) {
        *o = D3D11_RENDER_TARGET_BLEND_DESC1 {
            BlendEnable: t.BlendEnable,
            LogicOpEnable: l.0,
            SrcBlend: t.SrcBlend,
            DestBlend: t.DestBlend,
            BlendOp: t.BlendOp,
            SrcBlendAlpha: t.SrcBlendAlpha,
            DestBlendAlpha: t.DestBlendAlpha,
            BlendOpAlpha: t.BlendOpAlpha,
            LogicOp: l.1,
            RenderTargetWriteMask: t.RenderTargetWriteMask,
        };
    }
    out
}

/// A Direct3D 11.1 rasterizer description without a forced sample count, as Direct3D 11's.
pub fn rasterizer_of1(d: &D3D11_RASTERIZER_DESC1) -> D3D11_RASTERIZER_DESC {
    D3D11_RASTERIZER_DESC {
        FillMode: d.FillMode,
        CullMode: d.CullMode,
        FrontCounterClockwise: d.FrontCounterClockwise,
        DepthBias: d.DepthBias,
        DepthBiasClamp: d.DepthBiasClamp,
        SlopeScaledDepthBias: d.SlopeScaledDepthBias,
        DepthClipEnable: d.DepthClipEnable,
        ScissorEnable: d.ScissorEnable,
        MultisampleEnable: d.MultisampleEnable,
        AntialiasedLineEnable: d.AntialiasedLineEnable,
    }
}

pub fn blend_stored(d: &D3D11_BLEND_DESC) -> D3D11_BLEND_DESC {
    let mut d = *d;
    if !d.IndependentBlendEnable.as_bool() {
        d.RenderTarget = [d.RenderTarget[0]; 8];
    }
    for t in &mut d.RenderTarget {
        if !t.BlendEnable.as_bool() {
            *t = D3D11_RENDER_TARGET_BLEND_DESC {
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_ZERO,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_ZERO,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                ..*t
            };
        }
    }
    d
}

/// The description Direct3D keeps: with depth off its depth fields are the defaults, and with
/// stencil off its stencil fields.
pub fn depth_stencil_stored(d: &D3D11_DEPTH_STENCIL_DESC) -> D3D11_DEPTH_STENCIL_DESC {
    let mut d = *d;
    if !d.DepthEnable.as_bool() {
        d.DepthWriteMask = D3D11_DEPTH_WRITE_MASK_ALL;
        d.DepthFunc = D3D11_COMPARISON_LESS;
    }
    if !d.StencilEnable.as_bool() {
        let face = D3D11_DEPTH_STENCILOP_DESC {
            StencilFailOp: D3D11_STENCIL_OP_KEEP,
            StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
            StencilPassOp: D3D11_STENCIL_OP_KEEP,
            StencilFunc: D3D11_COMPARISON_ALWAYS,
        };
        d.StencilReadMask = 0xff;
        d.StencilWriteMask = 0xff;
        d.FrontFace = face;
        d.BackFace = face;
    }
    d
}

pub fn blend(d: &D3D11_BLEND_DESC) -> BlendDesc {
    let mut out = BlendDesc {
        alpha_to_coverage: d.AlphaToCoverageEnable.as_bool(),
        independent: d.IndependentBlendEnable.as_bool(),
        ..Default::default()
    };
    for (o, t) in out.targets.iter_mut().zip(&d.RenderTarget) {
        *o = TargetBlend {
            enable: t.BlendEnable.as_bool(),
            src: t.SrcBlend.0 as u32,
            dst: t.DestBlend.0 as u32,
            op: t.BlendOp.0 as u32,
            src_alpha: t.SrcBlendAlpha.0 as u32,
            dst_alpha: t.DestBlendAlpha.0 as u32,
            op_alpha: t.BlendOpAlpha.0 as u32,
            write_mask: t.RenderTargetWriteMask,
        };
    }
    out
}

fn face(f: &D3D11_DEPTH_STENCILOP_DESC) -> StencilFaceDesc {
    StencilFaceDesc {
        fail: f.StencilFailOp.0 as u32,
        depth_fail: f.StencilDepthFailOp.0 as u32,
        pass: f.StencilPassOp.0 as u32,
        func: f.StencilFunc.0 as u32,
    }
}

pub fn depth_stencil(d: &D3D11_DEPTH_STENCIL_DESC) -> Result<DepthStencilDesc> {
    if d.DepthWriteMask.0 > 1 {
        return Err(E_INVALIDARG.into());
    }
    Ok(DepthStencilDesc {
        depth_enable: d.DepthEnable.as_bool(),
        depth_write: d.DepthWriteMask == D3D11_DEPTH_WRITE_MASK_ALL,
        depth_func: d.DepthFunc.0 as u32,
        stencil_enable: d.StencilEnable.as_bool(),
        stencil_read_mask: d.StencilReadMask,
        stencil_write_mask: d.StencilWriteMask,
        front: face(&d.FrontFace),
        back: face(&d.BackFace),
    })
}

pub fn rasterizer(d: &D3D11_RASTERIZER_DESC) -> RasterizerDesc {
    RasterizerDesc {
        fill: d.FillMode.0 as u32,
        cull: d.CullMode.0 as u32,
        front_counter_clockwise: d.FrontCounterClockwise.as_bool(),
        depth_bias: d.DepthBias,
        depth_bias_clamp: d.DepthBiasClamp,
        slope_scaled_depth_bias: d.SlopeScaledDepthBias,
        depth_clip: d.DepthClipEnable.as_bool(),
        scissor: d.ScissorEnable.as_bool(),
        multisample: d.MultisampleEnable.as_bool(),
        antialiased_lines: d.AntialiasedLineEnable.as_bool(),
    }
}
