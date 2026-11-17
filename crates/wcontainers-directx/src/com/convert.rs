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

pub fn texture2d(d: &D3D11_TEXTURE2D_DESC) -> Texture2DDesc {
    Texture2DDesc {
        width: d.Width,
        height: d.Height,
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

pub fn texture2d_back(d: &Texture2DDesc) -> D3D11_TEXTURE2D_DESC {
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

/// A view's part of a 2D texture from (dimension kind, the union's fields).
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

pub fn rtv(d: &D3D11_RENDER_TARGET_VIEW_DESC) -> Result<ViewDesc> {
    unsafe {
        Ok(match d.ViewDimension {
            D3D11_RTV_DIMENSION_TEXTURE2D => view(
                d.Format,
                ViewDim::Texture2D,
                (d.Anonymous.Texture2D.MipSlice, 1),
                (0, 1),
            ),
            D3D11_RTV_DIMENSION_TEXTURE2DARRAY => {
                let a = d.Anonymous.Texture2DArray;
                view(
                    d.Format,
                    ViewDim::Texture2DArray,
                    (a.MipSlice, 1),
                    (a.FirstArraySlice, a.ArraySize),
                )
            }
            _ => return Err(super::missing("render target views of other dimensions")),
        })
    }
}

pub fn rtv_back(v: &ViewDesc) -> D3D11_RENDER_TARGET_VIEW_DESC {
    let mut d = D3D11_RENDER_TARGET_VIEW_DESC {
        Format: DXGI_FORMAT(v.format as i32),
        ..Default::default()
    };
    if v.dimension == ViewDim::Texture2D {
        d.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE2D;
        d.Anonymous.Texture2D = D3D11_TEX2D_RTV {
            MipSlice: v.first_mip,
        };
    } else {
        d.ViewDimension = D3D11_RTV_DIMENSION_TEXTURE2DARRAY;
        d.Anonymous.Texture2DArray = D3D11_TEX2D_ARRAY_RTV {
            MipSlice: v.first_mip,
            FirstArraySlice: v.first_layer,
            ArraySize: v.layer_count,
        };
    }
    d
}

pub fn dsv(d: &D3D11_DEPTH_STENCIL_VIEW_DESC) -> Result<ViewDesc> {
    if d.Flags != 0 {
        return Err(super::missing("read-only depth-stencil views"));
    }
    unsafe {
        Ok(match d.ViewDimension {
            D3D11_DSV_DIMENSION_TEXTURE2D => view(
                d.Format,
                ViewDim::Texture2D,
                (d.Anonymous.Texture2D.MipSlice, 1),
                (0, 1),
            ),
            D3D11_DSV_DIMENSION_TEXTURE2DARRAY => {
                let a = d.Anonymous.Texture2DArray;
                view(
                    d.Format,
                    ViewDim::Texture2DArray,
                    (a.MipSlice, 1),
                    (a.FirstArraySlice, a.ArraySize),
                )
            }
            _ => return Err(super::missing("depth-stencil views of other dimensions")),
        })
    }
}

pub fn dsv_back(v: &ViewDesc) -> D3D11_DEPTH_STENCIL_VIEW_DESC {
    let mut d = D3D11_DEPTH_STENCIL_VIEW_DESC {
        Format: DXGI_FORMAT(v.format as i32),
        ..Default::default()
    };
    if v.dimension == ViewDim::Texture2D {
        d.ViewDimension = D3D11_DSV_DIMENSION_TEXTURE2D;
        d.Anonymous.Texture2D = D3D11_TEX2D_DSV {
            MipSlice: v.first_mip,
        };
    } else {
        d.ViewDimension = D3D11_DSV_DIMENSION_TEXTURE2DARRAY;
        d.Anonymous.Texture2DArray = D3D11_TEX2D_ARRAY_DSV {
            MipSlice: v.first_mip,
            FirstArraySlice: v.first_layer,
            ArraySize: v.layer_count,
        };
    }
    d
}

pub fn srv(d: &D3D11_SHADER_RESOURCE_VIEW_DESC) -> Result<ViewDesc> {
    use windows::Win32::Graphics::Direct3D::*;
    unsafe {
        Ok(match d.ViewDimension {
            D3D_SRV_DIMENSION_TEXTURE2D => {
                let t = d.Anonymous.Texture2D;
                view(
                    d.Format,
                    ViewDim::Texture2D,
                    (t.MostDetailedMip, t.MipLevels),
                    (0, 1),
                )
            }
            D3D_SRV_DIMENSION_TEXTURE2DARRAY => {
                let t = d.Anonymous.Texture2DArray;
                view(
                    d.Format,
                    ViewDim::Texture2DArray,
                    (t.MostDetailedMip, t.MipLevels),
                    (t.FirstArraySlice, t.ArraySize),
                )
            }
            D3D_SRV_DIMENSION_TEXTURECUBE => {
                let t = d.Anonymous.TextureCube;
                view(
                    d.Format,
                    ViewDim::TextureCube,
                    (t.MostDetailedMip, t.MipLevels),
                    (0, 6),
                )
            }
            _ => return Err(super::missing("shader resource views of other dimensions")),
        })
    }
}

pub fn srv_back(v: &ViewDesc) -> D3D11_SHADER_RESOURCE_VIEW_DESC {
    use windows::Win32::Graphics::Direct3D::*;
    let mut d = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT(v.format as i32),
        ..Default::default()
    };
    match v.dimension {
        ViewDim::Texture2D => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURE2D;
            d.Anonymous.Texture2D = D3D11_TEX2D_SRV {
                MostDetailedMip: v.first_mip,
                MipLevels: v.mip_count,
            };
        }
        ViewDim::Texture2DArray => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURE2DARRAY;
            d.Anonymous.Texture2DArray = D3D11_TEX2D_ARRAY_SRV {
                MostDetailedMip: v.first_mip,
                MipLevels: v.mip_count,
                FirstArraySlice: v.first_layer,
                ArraySize: v.layer_count,
            };
        }
        ViewDim::TextureCube => {
            d.ViewDimension = D3D_SRV_DIMENSION_TEXTURECUBE;
            d.Anonymous.TextureCube = D3D11_TEXCUBE_SRV {
                MostDetailedMip: v.first_mip,
                MipLevels: v.mip_count,
            };
        }
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
