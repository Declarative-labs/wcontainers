//! Render target formats: a gradient drawn into each (floats past 0 and 1, so normalised formats
//! clamp them), integer gradients into integer formats, blending in an sRGB target, and what the
//! device says each format supports.

use std::fmt::Write;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{blend::no_blend, blend::target, colored_program, draw_colored, quad};
use crate::d3d11::{Gpu, solid};
use crate::dxbc;
use crate::output::Output;

const SIZE: u32 = 64;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Float,
    Uint,
    Sint,
}

pub const FORMATS: [(&str, DXGI_FORMAT, Kind); 22] = [
    ("rgba8_unorm", DXGI_FORMAT_R8G8B8A8_UNORM, Kind::Float),
    ("rgba8_srgb", DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, Kind::Float),
    ("rgba8_snorm", DXGI_FORMAT_R8G8B8A8_SNORM, Kind::Float),
    ("bgra8_unorm", DXGI_FORMAT_B8G8R8A8_UNORM, Kind::Float),
    ("bgra8_srgb", DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, Kind::Float),
    ("bgrx8_unorm", DXGI_FORMAT_B8G8R8X8_UNORM, Kind::Float),
    ("b5g6r5_unorm", DXGI_FORMAT_B5G6R5_UNORM, Kind::Float),
    ("rgb10a2_unorm", DXGI_FORMAT_R10G10B10A2_UNORM, Kind::Float),
    ("rg11b10_float", DXGI_FORMAT_R11G11B10_FLOAT, Kind::Float),
    ("rgba16_float", DXGI_FORMAT_R16G16B16A16_FLOAT, Kind::Float),
    ("rgba16_unorm", DXGI_FORMAT_R16G16B16A16_UNORM, Kind::Float),
    ("rgba16_snorm", DXGI_FORMAT_R16G16B16A16_SNORM, Kind::Float),
    ("rgba32_float", DXGI_FORMAT_R32G32B32A32_FLOAT, Kind::Float),
    ("rg16_float", DXGI_FORMAT_R16G16_FLOAT, Kind::Float),
    ("r32_float", DXGI_FORMAT_R32_FLOAT, Kind::Float),
    ("rg8_unorm", DXGI_FORMAT_R8G8_UNORM, Kind::Float),
    ("r8_unorm", DXGI_FORMAT_R8_UNORM, Kind::Float),
    ("r16_unorm", DXGI_FORMAT_R16_UNORM, Kind::Float),
    ("rgba8_uint", DXGI_FORMAT_R8G8B8A8_UINT, Kind::Uint),
    ("rgba16_sint", DXGI_FORMAT_R16G16B16A16_SINT, Kind::Sint),
    ("r32_uint", DXGI_FORMAT_R32_UINT, Kind::Uint),
    ("rgba32_sint", DXGI_FORMAT_R32G32B32A32_SINT, Kind::Sint),
];

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let mut outputs = Vec::new();
    let mut support = String::new();
    let vs = gpu.vs(dxbc!("common.vs_full"))?;
    let shaders = [
        (Kind::Float, gpu.ps(dxbc!("formats.ps_float"))?),
        (Kind::Uint, gpu.ps(dxbc!("formats.ps_uint"))?),
        (Kind::Sint, gpu.ps(dxbc!("formats.ps_sint"))?),
    ];
    for (name, format, kind) in FORMATS {
        let flags = unsafe { gpu.device.CheckFormatSupport(format)? } as i32;
        let has = |flag: D3D11_FORMAT_SUPPORT| u8::from(flags & flag.0 != 0);
        writeln!(
            support,
            "{name} render target {} blendable {} multisample render target {} shader sample {}",
            has(D3D11_FORMAT_SUPPORT_RENDER_TARGET),
            has(D3D11_FORMAT_SUPPORT_BLENDABLE),
            has(D3D11_FORMAT_SUPPORT_MULTISAMPLE_RENDERTARGET),
            has(D3D11_FORMAT_SUPPORT_SHADER_SAMPLE),
        )?;
        let (texture, rtv) = gpu.target(SIZE, SIZE, format)?;
        gpu.begin(&rtv, None, SIZE, SIZE);
        let ps = &shaders.iter().find(|(k, _)| *k == kind).unwrap().1;
        unsafe {
            c.RSSetState(&gpu.rasterizer(&solid())?);
            c.IASetInputLayout(None);
            c.IASetPrimitiveTopology(
                windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            );
            c.VSSetShader(&vs, None);
            c.PSSetShader(ps, None);
            c.Draw(3, 0);
        }
        outputs.push(Output::picture(name, gpu.read(&texture, 0, 0)?));
    }
    outputs.push(Output::text("support", support));

    // Blending in an sRGB target happens on linear values.
    let (texture, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    unsafe {
        c.VSSetShader(&vs, None);
        c.PSSetShader(&shaders[0].1, None);
        c.IASetInputLayout(None);
        c.Draw(3, 0);
    }
    let mut blend = no_blend();
    blend.RenderTarget[0] = target(
        D3D11_BLEND_SRC_ALPHA,
        D3D11_BLEND_INV_SRC_ALPHA,
        D3D11_BLEND_OP_ADD,
        D3D11_BLEND_ONE,
        D3D11_BLEND_ZERO,
        D3D11_BLEND_OP_ADD,
    );
    colored_program(gpu)?;
    unsafe { c.OMSetBlendState(&gpu.blend(&blend)?, None, u32::MAX) };
    draw_colored(
        gpu,
        &quad(-0.8, -0.8, 0.8, 0.8, 0.5, 0.5, [0.2, 0.9, 0.5, 0.5]),
    )?;
    outputs.push(Output::picture("srgb_blend", gpu.read(&texture, 0, 0)?));
    Ok(outputs)
}
