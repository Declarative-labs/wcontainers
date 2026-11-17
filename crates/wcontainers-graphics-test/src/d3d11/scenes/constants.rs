//! Constant buffers: fourteen slots of sizes from 16 bytes to 64 KiB read at once, and windows into
//! one 64 KiB buffer at several offsets and lengths (Direct3D 11.1's constant buffer offsets), where
//! reads past a window's end give zero.

use anyhow::Result;
use rand::Rng;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use crate::d3d11::{Gpu, seeded, solid};
use crate::dxbc;
use crate::output::Output;

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let mut random = seeded(13);
    let mut constants = |count: usize| -> Vec<f32> {
        (0..count * 4)
            .map(|_| random.random_range(0.0..0.25))
            .collect()
    };
    let sizes = [1, 16, 256, 4096, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1];
    let buffers: Vec<Option<ID3D11Buffer>> = sizes
        .iter()
        .map(|&n| {
            Ok(Some(gpu.immutable(
                bytemuck::cast_slice(&constants(n)),
                D3D11_BIND_CONSTANT_BUFFER,
            )?))
        })
        .collect::<Result<_>>()?;
    let mut outputs = Vec::new();
    let (target, rtv) = gpu.target(128, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, 128, 64);
    gpu.program(dxbc!("common.vs_full"), dxbc!("constants.ps_slots"), &[])?;
    unsafe {
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.PSSetConstantBuffers(0, Some(&buffers));
        c.Draw(3, 0);
    }
    outputs.push(Output::picture("slots", gpu.read(&target, 0, 0)?));

    let big = gpu.immutable(
        bytemuck::cast_slice(&constants(4096)),
        D3D11_BIND_CONSTANT_BUFFER,
    )?;
    let context1: ID3D11DeviceContext1 = c.cast()?;
    let (target, rtv) = gpu.target(64, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, 64, 64);
    gpu.program(dxbc!("common.vs_full"), dxbc!("constants.ps_window"), &[])?;
    for (i, (first, count)) in [(0u32, 64u32), (16, 16), (256, 64), (4080, 16)]
        .into_iter()
        .enumerate()
    {
        unsafe {
            c.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 16.0 * i as f32,
                Width: 64.0,
                Height: 16.0,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            context1.PSSetConstantBuffers1(
                0,
                1,
                Some(&Some(big.clone())),
                Some(&first),
                Some(&count),
            );
            c.Draw(3, 0);
        }
    }
    outputs.push(Output::picture("window", gpu.read(&target, 0, 0)?));
    Ok(outputs)
}
