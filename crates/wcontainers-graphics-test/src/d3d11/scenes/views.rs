//! Views of part of a resource and of other formats of its typeless group: a mip range, a layer
//! range, unsigned normalised against sRGB reads of one typeless texture, a float texture's bits as
//! integers, the depth and the stencil of a depth buffer, and render target views of one mip and of
//! one layer.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use super::{Colored, colored_program, depth_test, draw_colored, quad};
use crate::d3d11::{Gpu, cell, checker, data, solid, tex2d};
use crate::dxbc;
use crate::output::Output;

fn srv_2d(format: DXGI_FORMAT, first_mip: u32, mips: u32) -> D3D11_SHADER_RESOURCE_VIEW_DESC {
    let mut desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: format,
        ViewDimension: D3D_SRV_DIMENSION_TEXTURE2D,
        ..Default::default()
    };
    desc.Anonymous.Texture2D = D3D11_TEX2D_SRV {
        MostDetailedMip: first_mip,
        MipLevels: mips,
    };
    desc
}

/// Six mips of 32 by 32, each its own colour.
pub fn mip_levels() -> Vec<Vec<u8>> {
    (0..6).map(|m| checker(32 >> m, 32 >> m, m, 6)).collect()
}

/// Four layers of 16 by 16.
pub fn layers() -> Vec<Vec<u8>> {
    (0..4).map(|l| checker(16, 16, l, 4)).collect()
}

/// A 4 by 4 gradient of RGBA bytes.
pub fn gradient() -> Vec<u8> {
    (0..16u8)
        .flat_map(|i| [i * 16, 255 - i * 16, (i % 4) * 80, 255])
        .collect()
}

/// 16 by 16 floats, to be seen as their bits.
pub fn floats() -> Vec<f32> {
    (0..256).map(|i| i as f32 * 0.37 - 20.0).collect()
}

/// Five overlapping sloped quads drawn into the depth buffer, each adding to the stencil.
pub fn steps() -> Vec<Colored> {
    (0..5)
        .flat_map(|k| {
            let x = -1.0 + 0.3 * k as f32;
            quad(x, -0.9, 0.9, 0.9 - 0.2 * k as f32, 0.1, 0.9, [1.0; 4])
        })
        .collect()
}

/// The quad drawn through each render target view.
pub fn patch() -> [Colored; 6] {
    quad(-0.7, -0.5, 0.6, 0.8, 0.5, 0.5, [0.9, 0.6, 0.2, 1.0])
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let mut views: Vec<(ID3D11ShaderResourceView, &[u8])> = Vec::new();
    let rgba = DXGI_FORMAT_R8G8B8A8_UNORM;

    // Six mips, each its own colour: all of them, and from mip 2 on.
    let levels = mip_levels();
    let mut desc = tex2d(32, 32, rgba, D3D11_BIND_SHADER_RESOURCE);
    desc.MipLevels = 6;
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let init: Vec<_> = levels
        .iter()
        .enumerate()
        .map(|(m, b)| data(b, (32 >> m) * 4, 0))
        .collect();
    let mips = gpu.texture2d(&desc, &init)?;
    views.push((gpu.srv(&mips.cast()?, None)?, dxbc!("views.ps_level0")));
    views.push((
        gpu.srv(&mips.cast()?, Some(&srv_2d(rgba, 2, 3)))?,
        dxbc!("views.ps_level0"),
    ));

    // Four layers, viewed from layer 1 for three.
    let layers = layers();
    let mut desc = tex2d(16, 16, rgba, D3D11_BIND_SHADER_RESOURCE);
    desc.ArraySize = 4;
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let init: Vec<_> = layers.iter().map(|b| data(b, 64, 0)).collect();
    let array = gpu.texture2d(&desc, &init)?;
    let mut range = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: rgba,
        ViewDimension: D3D_SRV_DIMENSION_TEXTURE2DARRAY,
        ..Default::default()
    };
    range.Anonymous.Texture2DArray = D3D11_TEX2D_ARRAY_SRV {
        MostDetailedMip: 0,
        MipLevels: 1,
        FirstArraySlice: 1,
        ArraySize: 3,
    };
    views.push((
        gpu.srv(&array.cast()?, Some(&range))?,
        dxbc!("views.ps_array"),
    ));

    // A typeless 4 by 4 gradient read as unsigned normalised and as sRGB, linearly filtered.
    let gradient = gradient();
    let mut desc = tex2d(
        4,
        4,
        DXGI_FORMAT_R8G8B8A8_TYPELESS,
        D3D11_BIND_SHADER_RESOURCE,
    );
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let typeless = gpu.texture2d(&desc, &[data(&gradient, 16, 0)])?;
    views.push((
        gpu.srv(&typeless.cast()?, Some(&srv_2d(rgba, 0, 1)))?,
        dxbc!("views.ps_level0"),
    ));
    views.push((
        gpu.srv(
            &typeless.cast()?,
            Some(&srv_2d(DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, 0, 1)),
        )?,
        dxbc!("views.ps_level0"),
    ));

    // Floats viewed as their bits.
    let floats = floats();
    let mut desc = tex2d(16, 16, DXGI_FORMAT_R32_TYPELESS, D3D11_BIND_SHADER_RESOURCE);
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let bits = gpu.texture2d(&desc, &[data(bytemuck::cast_slice(&floats), 64, 0)])?;
    views.push((
        gpu.srv(&bits.cast()?, Some(&srv_2d(DXGI_FORMAT_R32_UINT, 0, 1)))?,
        dxbc!("views.ps_uint"),
    ));

    // A depth buffer drawn with a slope and stencil values, read as depth and as stencil.
    let desc = tex2d(
        32,
        32,
        DXGI_FORMAT_R24G8_TYPELESS,
        D3D11_BIND_FLAG(D3D11_BIND_DEPTH_STENCIL.0 | D3D11_BIND_SHADER_RESOURCE.0),
    );
    let depth = gpu.texture2d(&desc, &[])?;
    let dsv = gpu.dsv(
        &depth.cast()?,
        Some(&D3D11_DEPTH_STENCIL_VIEW_DESC {
            Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
            ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
            ..Default::default()
        }),
    )?;
    let mut write_stencil = depth_test(D3D11_COMPARISON_ALWAYS, true);
    write_stencil.StencilEnable = true.into();
    write_stencil.FrontFace.StencilPassOp = D3D11_STENCIL_OP_INCR_SAT;
    write_stencil.BackFace.StencilPassOp = D3D11_STENCIL_OP_INCR_SAT;
    unsafe {
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.ClearDepthStencilView(&dsv, D3D11_CLEAR_DEPTH.0 | D3D11_CLEAR_STENCIL.0, 1.0, 3);
        c.OMSetRenderTargets(None, &dsv);
        c.OMSetDepthStencilState(&gpu.depth_stencil(&write_stencil)?, 0);
    }
    gpu.viewport(32, 32);
    colored_program(gpu)?;
    draw_colored(gpu, &steps())?;
    unsafe { c.OMSetRenderTargets(None, None) };
    views.push((
        gpu.srv(
            &depth.cast()?,
            Some(&srv_2d(DXGI_FORMAT_R24_UNORM_X8_TYPELESS, 0, 1)),
        )?,
        dxbc!("views.ps_depth"),
    ));
    views.push((
        gpu.srv(
            &depth.cast()?,
            Some(&srv_2d(DXGI_FORMAT_X24_TYPELESS_G8_UINT, 0, 1)),
        )?,
        dxbc!("views.ps_stencil"),
    ));

    let (target, rtv) = gpu.target(256, 128, rgba)?;
    gpu.begin(&rtv, None, 256, 128);
    let filter = gpu.sampler(&D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MaxAnisotropy: 1,
        MaxLOD: f32::MAX,
        ..Default::default()
    })?;
    for (i, (view, ps)) in views.into_iter().enumerate() {
        gpu.program(dxbc!("common.vs_full"), ps, &[])?;
        unsafe {
            c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            c.PSSetSamplers(0, Some(&[Some(filter.clone())]));
            c.PSSetShaderResources(0, Some(&[Some(view)]));
            c.RSSetViewports(Some(&[cell(i as u32, 4, 64.0)]));
            c.Draw(3, 0);
        }
    }
    let mut outputs = vec![Output::picture("", gpu.read(&target, 0, 0)?)];

    // Render target views of mip 1 of a mip chain and of layer 2 of an array.
    let mut desc = tex2d(64, 64, rgba, D3D11_BIND_RENDER_TARGET);
    desc.MipLevels = 3;
    let chain = gpu.texture2d(&desc, &[])?;
    let mut mip_view = D3D11_RENDER_TARGET_VIEW_DESC {
        Format: rgba,
        ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
        ..Default::default()
    };
    mip_view.Anonymous.Texture2D.MipSlice = 1;
    let mut desc = tex2d(32, 32, rgba, D3D11_BIND_RENDER_TARGET);
    desc.ArraySize = 4;
    let layered = gpu.texture2d(&desc, &[])?;
    let mut layer_view = D3D11_RENDER_TARGET_VIEW_DESC {
        Format: rgba,
        ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2DARRAY,
        ..Default::default()
    };
    layer_view.Anonymous.Texture2DArray = D3D11_TEX2D_ARRAY_RTV {
        MipSlice: 0,
        FirstArraySlice: 2,
        ArraySize: 1,
    };
    colored_program(gpu)?;
    for (part, texture, view, mip, layer) in [
        ("rtv_mip", &chain, mip_view, 1, 0),
        ("rtv_layer", &layered, layer_view, 0, 2),
    ] {
        let rtv = gpu.rtv(&texture.cast()?, Some(&view))?;
        gpu.begin(&rtv, None, 32, 32);
        draw_colored(gpu, &patch())?;
        outputs.push(Output::picture(part, gpu.read(texture, mip, layer)?));
    }
    Ok(outputs)
}
