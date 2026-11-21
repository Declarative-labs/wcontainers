//! Samplers, one per cell: every minification, magnification and mip filter combination,
//! anisotropy, level bias and clamps; minimum and maximum reduction filters (where the device says
//! it has them); every address mode for u against v with a border colour; and comparison sampling
//! of a depth texture with each function under each comparison filter, and gathering.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use super::{colored_program, depth_test, draw_colored, quad};
use crate::d3d11::{Gpu, cell, checker, data, solid, tex2d};
use crate::dxbc;
use crate::output::Output;

pub fn sampler(
    filter: D3D11_FILTER,
    address: [D3D11_TEXTURE_ADDRESS_MODE; 2],
) -> D3D11_SAMPLER_DESC {
    D3D11_SAMPLER_DESC {
        Filter: filter,
        AddressU: address[0],
        AddressV: address[1],
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MipLODBias: 0.0,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D11_COMPARISON_NEVER,
        BorderColor: [0.2, 0.9, 0.4, 0.6],
        MinLOD: 0.0,
        MaxLOD: f32::MAX,
    }
}

/// Draws one cell per sampler with `ps`, the cell's rectangle in constant buffer 0, and reads it.
fn cells(
    gpu: &Gpu,
    ps: &[u8],
    columns: u32,
    size: u32,
    samplers: &[D3D11_SAMPLER_DESC],
    slot: u32,
) -> Result<image::DynamicImage> {
    let rows = (samplers.len() as u32).div_ceil(columns);
    let (target, rtv) = gpu.target(columns * size, rows * size, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, columns * size, rows * size);
    gpu.program(dxbc!("common.vs_full"), ps, &[])?;
    let rect = gpu.buffer(
        &D3D11_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            ..Default::default()
        },
        None,
    )?;
    for (i, desc) in samplers.iter().enumerate() {
        let viewport = cell(i as u32, columns, size as f32);
        let r = [
            viewport.TopLeftX,
            viewport.TopLeftY,
            viewport.Width,
            viewport.Height,
        ];
        unsafe {
            let c = &gpu.context;
            c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            c.UpdateSubresource(&rect, 0, None, r.as_ptr().cast(), 0, 0);
            c.PSSetConstantBuffers(0, Some(&[Some(rect.clone())]));
            c.PSSetSamplers(slot, Some(&[Some(gpu.sampler(desc)?)]));
            c.RSSetViewports(Some(&[viewport]));
            c.Draw(3, 0);
        }
    }
    gpu.read(&target, 0, 0)
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut outputs = Vec::new();
    let c = &gpu.context;
    unsafe { c.RSSetState(&gpu.rasterizer(&solid())?) };

    // A 32 by 32 texture whose six mips are each a checker of their own colour.
    let levels: Vec<Vec<u8>> = (0..6).map(|m| checker(32 >> m, 32 >> m, m, 6)).collect();
    let mut desc = tex2d(
        32,
        32,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D11_BIND_SHADER_RESOURCE,
    );
    desc.MipLevels = 6;
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let init: Vec<_> = levels
        .iter()
        .enumerate()
        .map(|(m, b)| data(b, (32 >> m) * 4, 0))
        .collect();
    let mips = gpu.texture2d(&desc, &init)?;
    let view = gpu.srv(&mips.cast()?, None)?;
    unsafe { c.PSSetShaderResources(0, Some(&[Some(view)])) };
    let filters = filters();
    outputs.push(Output::picture(
        "filters",
        cells(gpu, dxbc!("samplers.ps_sample"), 4, 64, &filters, 0)?,
    ));
    if gpu.options()?.1.MinMaxFiltering.as_bool() {
        outputs.push(Output::picture(
            "reduction",
            cells(gpu, dxbc!("samplers.ps_sample"), 4, 64, &reduction(), 0)?,
        ));
    } else {
        outputs.push(Output::absent(
            "reduction",
            "minimum and maximum filtering (D3D11_FEATURE_D3D11_OPTIONS1 MinMaxFiltering)"
                .to_owned(),
        ));
    }

    // Every address mode for u against v, linear filtering of an 8 by 8 checker.
    let texels = checker(8, 8, 1, 3);
    let mut desc = tex2d(8, 8, DXGI_FORMAT_R8G8B8A8_UNORM, D3D11_BIND_SHADER_RESOURCE);
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let small = gpu.texture2d(&desc, &[data(&texels, 32, 0)])?;
    unsafe { c.PSSetShaderResources(0, Some(&[Some(gpu.srv(&small.cast()?, None)?)])) };
    let address = address();
    outputs.push(Output::picture(
        "address",
        cells(gpu, dxbc!("samplers.ps_sample"), 5, 48, &address, 0)?,
    ));
    match gpu.lacks_level(D3D_FEATURE_LEVEL_10_1) {
        Some(why) => outputs.push(Output::absent("gather", why)),
        None => outputs.push(Output::picture(
            "gather",
            cells(gpu, dxbc!("samplers.ps_gather"), 5, 48, &address, 0)?,
        )),
    }

    // A depth texture holding a slope, compared with each function, point and linear filtered.
    let mut desc = tex2d(
        32,
        32,
        DXGI_FORMAT_R32_TYPELESS,
        D3D11_BIND_FLAG(D3D11_BIND_DEPTH_STENCIL.0 | D3D11_BIND_SHADER_RESOURCE.0),
    );
    desc.MipLevels = 1;
    let depth = gpu.texture2d(&desc, &[])?;
    let dsv = gpu.dsv(
        &depth.cast()?,
        Some(&D3D11_DEPTH_STENCIL_VIEW_DESC {
            Format: DXGI_FORMAT_D32_FLOAT,
            ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
            ..Default::default()
        }),
    )?;
    unsafe {
        c.ClearDepthStencilView(&dsv, D3D11_CLEAR_DEPTH.0, 1.0, 0);
        c.OMSetRenderTargets(None, &dsv);
        c.OMSetDepthStencilState(
            &gpu.depth_stencil(&depth_test(D3D11_COMPARISON_ALWAYS, true))?,
            0,
        );
    }
    gpu.viewport(32, 32);
    colored_program(gpu)?;
    draw_colored(gpu, &quad(-0.8, -1.0, 1.0, 0.7, 0.05, 0.95, [1.0; 4]))?;
    unsafe {
        c.OMSetRenderTargets(None, None);
        c.OMSetDepthStencilState(None, 0);
    }
    let mut desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT_R32_FLOAT,
        ViewDimension: D3D_SRV_DIMENSION_TEXTURE2D,
        ..Default::default()
    };
    desc.Anonymous.Texture2D.MipLevels = 1;
    let depth_view = gpu.srv(&depth.cast()?, Some(&desc))?;
    unsafe { c.PSSetShaderResources(0, Some(&[Some(depth_view)])) };
    let compare = compare();
    outputs.push(Output::picture(
        "compare",
        cells(gpu, dxbc!("samplers.ps_compare"), 4, 48, &compare, 1)?,
    ));
    outputs.push(Output::picture(
        "compare_zero",
        cells(gpu, dxbc!("samplers.ps_compare_zero"), 4, 48, &compare, 1)?,
    ));
    Ok(outputs)
}

/// Every filter combination, anisotropy, level bias and clamps, sampling the six-level texture.
pub fn filters() -> Vec<D3D11_SAMPLER_DESC> {
    let wrap = [D3D11_TEXTURE_ADDRESS_WRAP; 2];
    let mut filters: Vec<D3D11_SAMPLER_DESC> = [
        D3D11_FILTER_MIN_MAG_MIP_POINT,
        D3D11_FILTER_MIN_MAG_POINT_MIP_LINEAR,
        D3D11_FILTER_MIN_POINT_MAG_LINEAR_MIP_POINT,
        D3D11_FILTER_MIN_POINT_MAG_MIP_LINEAR,
        D3D11_FILTER_MIN_LINEAR_MAG_MIP_POINT,
        D3D11_FILTER_MIN_LINEAR_MAG_POINT_MIP_LINEAR,
        D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT,
        D3D11_FILTER_MIN_MAG_MIP_LINEAR,
    ]
    .into_iter()
    .map(|f| sampler(f, wrap))
    .collect();
    for anisotropy in [2, 16] {
        filters.push(D3D11_SAMPLER_DESC {
            MaxAnisotropy: anisotropy,
            ..sampler(D3D11_FILTER_ANISOTROPIC, wrap)
        });
    }
    let linear = sampler(D3D11_FILTER_MIN_MAG_MIP_LINEAR, wrap);
    filters.extend([
        D3D11_SAMPLER_DESC {
            MipLODBias: 1.5,
            ..linear
        },
        D3D11_SAMPLER_DESC {
            MipLODBias: -2.0,
            ..linear
        },
        D3D11_SAMPLER_DESC {
            MinLOD: 2.0,
            ..linear
        },
        D3D11_SAMPLER_DESC {
            MaxLOD: 1.0,
            ..linear
        },
        D3D11_SAMPLER_DESC {
            MinLOD: 1.5,
            MaxLOD: 1.5,
            ..linear
        },
        D3D11_SAMPLER_DESC {
            MaxLOD: 0.0,
            ..sampler(D3D11_FILTER_MIN_LINEAR_MAG_MIP_POINT, wrap)
        },
    ]);
    filters
}

/// The minimum and maximum reduction of point, linear and anisotropic filtering.
pub fn reduction() -> Vec<D3D11_SAMPLER_DESC> {
    let wrap = [D3D11_TEXTURE_ADDRESS_WRAP; 2];
    [
        D3D11_FILTER_MINIMUM_MIN_MAG_MIP_POINT,
        D3D11_FILTER_MINIMUM_MIN_MAG_LINEAR_MIP_POINT,
        D3D11_FILTER_MINIMUM_MIN_MAG_MIP_LINEAR,
        D3D11_FILTER_MINIMUM_ANISOTROPIC,
        D3D11_FILTER_MAXIMUM_MIN_MAG_MIP_POINT,
        D3D11_FILTER_MAXIMUM_MIN_MAG_LINEAR_MIP_POINT,
        D3D11_FILTER_MAXIMUM_MIN_MAG_MIP_LINEAR,
        D3D11_FILTER_MAXIMUM_ANISOTROPIC,
    ]
    .into_iter()
    .map(|f| D3D11_SAMPLER_DESC {
        MaxAnisotropy: 8,
        ..sampler(f, wrap)
    })
    .collect()
}

/// Every address mode for u against v, linear filtered.
pub fn address() -> Vec<D3D11_SAMPLER_DESC> {
    let modes = [
        D3D11_TEXTURE_ADDRESS_WRAP,
        D3D11_TEXTURE_ADDRESS_MIRROR,
        D3D11_TEXTURE_ADDRESS_CLAMP,
        D3D11_TEXTURE_ADDRESS_BORDER,
        D3D11_TEXTURE_ADDRESS_MIRROR_ONCE,
    ];
    modes
        .iter()
        .flat_map(|&v| modes.map(|u| sampler(D3D11_FILTER_MIN_MAG_MIP_LINEAR, [u, v])))
        .collect()
}

/// Each comparison function under each comparison filter: point, linear with point mips, linear
/// throughout, and anisotropic.
pub fn compare() -> Vec<D3D11_SAMPLER_DESC> {
    let funcs = [
        D3D11_COMPARISON_NEVER,
        D3D11_COMPARISON_LESS,
        D3D11_COMPARISON_EQUAL,
        D3D11_COMPARISON_LESS_EQUAL,
        D3D11_COMPARISON_GREATER,
        D3D11_COMPARISON_NOT_EQUAL,
        D3D11_COMPARISON_GREATER_EQUAL,
        D3D11_COMPARISON_ALWAYS,
    ];
    [
        D3D11_FILTER_COMPARISON_MIN_MAG_MIP_POINT,
        D3D11_FILTER_COMPARISON_MIN_MAG_LINEAR_MIP_POINT,
        D3D11_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR,
        D3D11_FILTER_COMPARISON_ANISOTROPIC,
    ]
    .into_iter()
    .flat_map(|filter| {
        funcs.map(|func| D3D11_SAMPLER_DESC {
            ComparisonFunc: func,
            MaxAnisotropy: 4,
            ..sampler(filter, [D3D11_TEXTURE_ADDRESS_CLAMP; 2])
        })
    })
    .collect()
}
