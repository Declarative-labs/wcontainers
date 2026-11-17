//! Four render targets at once (8-bit, half float, 10-bit and 32-bit integer), each cleared to its
//! own value and blended its own way: alpha blending, adding, the minimum, and no blending with
//! only red written.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{blend::no_blend, blend::target, colored_program, draw_colored, hue};
use crate::d3d11::{Gpu, solid};
use crate::dxbc;
use crate::output::Output;

const SIZE: u32 = 128;

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let formats = [
        DXGI_FORMAT_R8G8B8A8_UNORM,
        DXGI_FORMAT_R16G16B16A16_FLOAT,
        DXGI_FORMAT_R10G10B10A2_UNORM,
        DXGI_FORMAT_R32_UINT,
    ];
    let clears = [
        [0.1, 0.2, 0.3, 1.0],
        [-0.5, 0.25, 2.0, 1.0],
        [0.6, 0.4, 0.2, 1.0],
        [12345.0, 0.0, 0.0, 0.0],
    ];
    let mut textures = Vec::new();
    let mut views = Vec::new();
    for (format, clear) in formats.into_iter().zip(&clears) {
        let (texture, rtv) = gpu.target(SIZE, SIZE, format)?;
        unsafe { c.ClearRenderTargetView(&rtv, clear) };
        textures.push(texture);
        views.push(Some(rtv));
    }
    let mut blend = no_blend();
    blend.IndependentBlendEnable = true.into();
    blend.RenderTarget[0] = target(
        D3D11_BLEND_SRC_ALPHA,
        D3D11_BLEND_INV_SRC_ALPHA,
        D3D11_BLEND_OP_ADD,
        D3D11_BLEND_ONE,
        D3D11_BLEND_INV_SRC_ALPHA,
        D3D11_BLEND_OP_ADD,
    );
    blend.RenderTarget[1] = target(
        D3D11_BLEND_ONE,
        D3D11_BLEND_ONE,
        D3D11_BLEND_OP_ADD,
        D3D11_BLEND_ONE,
        D3D11_BLEND_ONE,
        D3D11_BLEND_OP_ADD,
    );
    blend.RenderTarget[2] = target(
        D3D11_BLEND_ONE,
        D3D11_BLEND_ONE,
        D3D11_BLEND_OP_MIN,
        D3D11_BLEND_ONE,
        D3D11_BLEND_ONE,
        D3D11_BLEND_OP_MIN,
    );
    blend.RenderTarget[3].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_RED.0 as u8;
    colored_program(gpu)?;
    unsafe {
        c.OMSetRenderTargets(Some(&views), None);
        c.OMSetBlendState(&gpu.blend(&blend)?, None, u32::MAX);
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.PSSetShader(&gpu.ps(dxbc!("targets.ps"))?, None);
    }
    gpu.viewport(SIZE, SIZE);
    let mut v = Vec::new();
    for k in 0..3u32 {
        let [r, g, b] = hue(k, 3);
        let angle = k as f32 * 2.1;
        let (x, y) = (0.3 * angle.cos(), 0.3 * angle.sin());
        for (dx, dy) in [(0.0, 0.7), (0.6, -0.5), (-0.6, -0.5)] {
            v.push([x + dx, y + dy, 0.5, r, g, b, 0.6]);
        }
    }
    draw_colored(gpu, &v)?;
    let names = ["rgba8", "rgba16f", "rgb10a2", "r32ui"];
    names
        .iter()
        .zip(&textures)
        .map(|(name, texture)| Ok(Output::picture(name, gpu.read(texture, 0, 0)?)))
        .collect()
}
