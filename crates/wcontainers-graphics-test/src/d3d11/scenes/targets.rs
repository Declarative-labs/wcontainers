//! Four render targets at once (8-bit, half float, 10-bit and 32-bit integer), each cleared to its
//! own value, written without blending (from feature level 10_0), then again each blended its own
//! way (from 10_1): alpha blending, adding, the minimum, and no blending with only red written.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_10_1;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{Colored, blend::no_blend, blend::target, colored_program, draw_colored, hue};
use crate::d3d11::{Gpu, solid};
use crate::dxbc;
use crate::output::Output;

pub const SIZE: u32 = 128;

/// The targets' formats, their clear values and their outputs' names.
pub const FORMATS: [(DXGI_FORMAT, [f32; 4], &str); 4] = [
    (DXGI_FORMAT_R8G8B8A8_UNORM, [0.1, 0.2, 0.3, 1.0], "rgba8"),
    (
        DXGI_FORMAT_R16G16B16A16_FLOAT,
        [-0.5, 0.25, 2.0, 1.0],
        "rgba16f",
    ),
    (
        DXGI_FORMAT_R10G10B10A2_UNORM,
        [0.6, 0.4, 0.2, 1.0],
        "rgb10a2",
    ),
    (DXGI_FORMAT_R32_UINT, [12345.0, 0.0, 0.0, 0.0], "r32ui"),
];

/// Makes a part's blend description.
pub type Blend = fn() -> D3D11_BLEND_DESC;

/// The parts: their outputs' prefix, their blending, and whether it is set per target (10_1).
pub const PARTS: [(&str, Blend, bool); 2] = [("plain_", no_blend, false), ("", blend, true)];

/// Each target's blending: alpha blending, adding, the minimum, and only red written.
pub fn blend() -> D3D11_BLEND_DESC {
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
    blend
}

/// Three overlapping translucent triangles.
pub fn triangles() -> Vec<Colored> {
    let mut v = Vec::new();
    for k in 0..3u32 {
        let [r, g, b] = hue(k, 3);
        let angle = k as f32 * 2.1;
        let (x, y) = (0.3 * angle.cos(), 0.3 * angle.sin());
        for (dx, dy) in [(0.0, 0.7), (0.6, -0.5), (-0.6, -0.5)] {
            v.push([x + dx, y + dy, 0.5, r, g, b, 0.6]);
        }
    }
    v
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let mut outputs = Vec::new();
    for (prefix, blend, per_target) in PARTS {
        if per_target && let Some(why) = gpu.lacks_level(D3D_FEATURE_LEVEL_10_1) {
            for (_, _, name) in &FORMATS {
                outputs.push(Output::absent(&format!("{prefix}{name}"), why.clone()));
            }
            continue;
        }
        let mut textures = Vec::new();
        let mut views = Vec::new();
        for (format, clear, _) in &FORMATS {
            let (texture, rtv) = gpu.target(SIZE, SIZE, *format)?;
            unsafe { c.ClearRenderTargetView(&rtv, clear) };
            textures.push(texture);
            views.push(Some(rtv));
        }
        colored_program(gpu)?;
        unsafe {
            c.OMSetRenderTargets(Some(&views), None);
            c.OMSetBlendState(&gpu.blend(&blend())?, None, u32::MAX);
            c.RSSetState(&gpu.rasterizer(&solid())?);
            c.PSSetShader(&gpu.ps(dxbc!("targets.ps"))?, None);
        }
        gpu.viewport(SIZE, SIZE);
        draw_colored(gpu, &triangles())?;
        for ((_, _, name), texture) in FORMATS.iter().zip(&textures) {
            outputs.push(Output::picture(
                &format!("{prefix}{name}"),
                gpu.read(texture, 0, 0)?,
            ));
        }
    }
    Ok(outputs)
}
