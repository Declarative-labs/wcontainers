//! Writing resources while drawing: a dynamic vertex buffer filled with no-overwrite maps and
//! renamed by a discard map every fourth draw, a dynamic constant buffer discarded for every draw,
//! and UpdateSubresource into part of a vertex buffer and part of a texture.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use super::{Colored, colored_program, hue, quad};
use crate::d3d11::{Gpu, checker, data, solid, tex2d};
use crate::dxbc;
use crate::output::Output;

const SIZE: u32 = 128;

/// Writes `bytes` at `offset` of a dynamic buffer through a map of kind `map`.
fn write(
    gpu: &Gpu,
    buffer: &ID3D11Buffer,
    map: D3D11_MAP,
    offset: usize,
    bytes: &[u8],
) -> Result<()> {
    unsafe {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        gpu.context.Map(buffer, 0, map, 0, Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            mapped.pData.cast::<u8>().add(offset),
            bytes.len(),
        );
        gpu.context.Unmap(buffer, 0);
    }
    Ok(())
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    colored_program(gpu)?;
    let dynamic = |bytes: u32, bind: D3D11_BIND_FLAG| {
        gpu.buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: bytes,
                Usage: D3D11_USAGE_DYNAMIC,
                BindFlags: bind.0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                ..Default::default()
            },
            None,
        )
    };
    let vertices = dynamic(4 * 6 * 28, D3D11_BIND_VERTEX_BUFFER)?;
    let tint = dynamic(16, D3D11_BIND_CONSTANT_BUFFER)?;
    unsafe {
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.PSSetShader(&gpu.ps(dxbc!("dynamic.ps_tint"))?, None);
        c.PSSetConstantBuffers(0, Some(&[Some(tint.clone())]));
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.IASetVertexBuffers(0, 1, Some(&Some(vertices.clone())), Some(&28), Some(&0));
    }
    for k in 0..16u32 {
        let (x, y) = (-1.0 + 0.5 * (k % 4) as f32, 0.5 - 0.5 * (k / 4) as f32);
        let [r, g, b] = hue(k, 16);
        let shape = quad(
            x + 0.05,
            y + 0.05,
            x + 0.45,
            y + 0.45,
            0.5,
            0.5,
            [r, g, b, 1.0],
        );
        let slot = (k % 4) as usize;
        let map = if slot == 0 {
            D3D11_MAP_WRITE_DISCARD
        } else {
            D3D11_MAP_WRITE_NO_OVERWRITE
        };
        write(
            gpu,
            &vertices,
            map,
            slot * 6 * 28,
            bytemuck::cast_slice(&shape),
        )?;
        let shade = 0.5 + 0.03 * k as f32;
        write(
            gpu,
            &tint,
            D3D11_MAP_WRITE_DISCARD,
            0,
            bytemuck::cast_slice(&[shade, shade, 1.0, 1.0]),
        )?;
        unsafe { c.Draw(6, slot as u32 * 6) };
    }

    // A default vertex buffer of two quads whose second is replaced by UpdateSubresource.
    colored_program(gpu)?;
    let mut two: Vec<Colored> =
        quad(-0.95, -0.95, -0.6, -0.6, 0.5, 0.5, [1.0, 1.0, 1.0, 1.0]).to_vec();
    two.extend(quad(
        -0.55,
        -0.95,
        -0.2,
        -0.6,
        0.5,
        0.5,
        [1.0, 0.0, 0.0, 1.0],
    ));
    let fixed = gpu.buffer(
        &D3D11_BUFFER_DESC {
            ByteWidth: (two.len() * 28) as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
            ..Default::default()
        },
        Some(bytemuck::cast_slice(&two)),
    )?;
    let replacement = quad(-0.5, -0.9, -0.1, -0.5, 0.5, 0.5, [0.2, 0.9, 0.9, 1.0]);
    let region = D3D11_BOX {
        left: 6 * 28,
        right: 12 * 28,
        top: 0,
        bottom: 1,
        front: 0,
        back: 1,
    };
    unsafe {
        c.UpdateSubresource(&fixed, 0, Some(&region), replacement.as_ptr().cast(), 0, 0);
        c.IASetVertexBuffers(0, 1, Some(&Some(fixed)), Some(&28), Some(&0));
        c.Draw(12, 0);
    }

    // A 16 by 16 texture with an 8 by 8 block replaced, drawn into a corner.
    let mut desc = tex2d(
        16,
        16,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D11_BIND_SHADER_RESOURCE,
    );
    desc.Usage = D3D11_USAGE_DEFAULT;
    let texels = checker(16, 16, 0, 3);
    let texture = gpu.texture2d(&desc, &[data(&texels, 64, 0)])?;
    let block = checker(8, 8, 2, 3);
    let region = D3D11_BOX {
        left: 5,
        right: 13,
        top: 3,
        bottom: 11,
        front: 0,
        back: 1,
    };
    let sampler = gpu.sampler(&D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MaxAnisotropy: 1,
        MaxLOD: f32::MAX,
        ..Default::default()
    })?;
    gpu.program(dxbc!("common.vs_full"), dxbc!("views.ps_level0"), &[])?;
    unsafe {
        c.UpdateSubresource(&texture, 0, Some(&region), block.as_ptr().cast(), 32, 0);
        c.PSSetShaderResources(0, Some(&[Some(gpu.srv(&texture.cast()?, None)?)]));
        c.PSSetSamplers(0, Some(&[Some(sampler)]));
        c.RSSetViewports(Some(&[D3D11_VIEWPORT {
            TopLeftX: 80.0,
            TopLeftY: 80.0,
            Width: 48.0,
            Height: 48.0,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]));
        c.Draw(3, 0);
    }
    Ok(vec![Output::picture("", gpu.read(&target, 0, 0)?)])
}
