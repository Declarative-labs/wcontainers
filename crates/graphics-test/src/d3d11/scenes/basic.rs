//! The first three scenes, shared with the other renderers through scene.rs.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Interface, s};

use crate::d3d11::{Gpu, data, element, solid, tex2d};
use crate::dxbc;
use crate::output::Output;
use crate::scene::{self, HEIGHT, WIDTH};

pub fn triangle(gpu: &Gpu) -> Result<Vec<Output>> {
    let (target, rtv) = gpu.target(WIDTH, HEIGHT, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, WIDTH, HEIGHT);
    gpu.program(
        dxbc!("triangle.vs"),
        dxbc!("triangle.ps"),
        &[
            element(s!("POSITION"), 0, DXGI_FORMAT_R32G32_FLOAT, 0, 0, 0),
            element(s!("COLOR"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 8, 0),
        ],
    )?;
    gpu.vertices(0, bytemuck::cast_slice(&scene::TRIANGLE), 20)?;
    unsafe {
        gpu.context.RSSetState(&gpu.rasterizer(&solid())?);
        gpu.context
            .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        gpu.context.Draw(3, 0);
    }
    Ok(vec![Output::picture("", gpu.read(&target, 0, 0)?)])
}

pub fn quads(gpu: &Gpu) -> Result<Vec<Output>> {
    let (target, rtv) = gpu.target(WIDTH, HEIGHT, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (_, dsv) = gpu.depth(WIDTH, HEIGHT, DXGI_FORMAT_D32_FLOAT)?;
    gpu.begin(&rtv, Some(&dsv), WIDTH, HEIGHT);
    gpu.program(
        dxbc!("quads.vs"),
        dxbc!("quads.ps"),
        &[
            element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
            element(s!("TEXCOORD"), 0, DXGI_FORMAT_R32G32_FLOAT, 0, 12, 0),
            element(s!("COLOR"), 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 20, 0),
        ],
    )?;
    gpu.vertices(0, bytemuck::cast_slice(&scene::QUADS), 36)?;
    let indices = gpu.immutable(
        bytemuck::cast_slice(&scene::QUAD_INDICES),
        D3D11_BIND_INDEX_BUFFER,
    )?;
    let texels = scene::texture();
    let mut desc = tex2d(
        scene::TEXTURE_SIZE,
        scene::TEXTURE_SIZE,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D11_BIND_SHADER_RESOURCE,
    );
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let texture = gpu.texture2d(&desc, &[data(&texels, scene::TEXTURE_SIZE * 4, 0)])?;
    let view = gpu.srv(&texture.cast()?, None)?;
    let sampler = gpu.sampler(&D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
        AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
        AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D11_COMPARISON_NEVER,
        MaxLOD: f32::MAX,
        ..Default::default()
    })?;
    let mut blend = D3D11_BLEND_DESC::default();
    blend.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: true.into(),
        SrcBlend: D3D11_BLEND_SRC_ALPHA,
        DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_ONE,
        DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    let depth = gpu.depth_stencil(&D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: true.into(),
        DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
        DepthFunc: D3D11_COMPARISON_LESS,
        ..Default::default()
    })?;
    unsafe {
        let c = &gpu.context;
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.IASetIndexBuffer(&indices, DXGI_FORMAT_R16_UINT, 0);
        c.PSSetShaderResources(0, Some(&[Some(view)]));
        c.PSSetSamplers(0, Some(&[Some(sampler)]));
        c.OMSetBlendState(&gpu.blend(&blend)?, None, u32::MAX);
        c.OMSetDepthStencilState(&depth, 0);
        c.DrawIndexed(scene::QUAD_INDICES.len() as u32, 0, 0);
    }
    Ok(vec![Output::picture("", gpu.read(&target, 0, 0)?)])
}

pub fn instanced(gpu: &Gpu) -> Result<Vec<Output>> {
    let (target, rtv) = gpu.target(WIDTH, HEIGHT, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, WIDTH, HEIGHT);
    gpu.program(
        dxbc!("instanced.vs"),
        dxbc!("instanced.ps"),
        &[element(
            s!("POSITION"),
            0,
            DXGI_FORMAT_R32G32_FLOAT,
            0,
            0,
            0,
        )],
    )?;
    gpu.vertices(0, bytemuck::cast_slice(&scene::INSTANCE_SHAPE), 8)?;
    let constants = gpu.immutable(
        bytemuck::cast_slice(&scene::instance_constants()),
        D3D11_BIND_CONSTANT_BUFFER,
    )?;
    unsafe {
        let c = &gpu.context;
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.VSSetConstantBuffers(0, Some(&[Some(constants)]));
        c.DrawInstanced(3, scene::INSTANCES, 0, 0);
    }
    Ok(vec![Output::picture("", gpu.read(&target, 0, 0)?)])
}
