//! Multisampling with the standard sample patterns: edges at 2, 4 and 8 samples resolved, the
//! sample mask, alpha to coverage, shading per sample, coverage written by the pixel shader, and
//! single samples read from a multisampled texture.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use super::{Colored, blend::no_blend, colored_program, draw_colored, hue, quad};
use crate::d3d11::{Gpu, solid, tex2d};
use crate::dxbc;
use crate::output::Output;

pub const SIZE: u32 = 128;
pub const FORMAT: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

/// Thin wedges, whose edges cross pixels at every angle.
pub fn wedges() -> Vec<Colored> {
    let mut v = Vec::new();
    for k in 0..5u32 {
        let a0 = k as f32 * 0.31 + 0.05;
        let a1 = a0 + 0.12;
        let [r, g, b] = hue(k, 5);
        let p = |a: f32, d: f32| [d * a.cos() - 0.9, d * a.sin() - 0.9, 0.5, r, g, b, 1.0];
        v.extend([p(a0, 0.1), p(a1, 1.9), p(a0, 1.9)]);
    }
    v
}

/// A square whose alpha runs from 0 on the left to 1 on the right.
pub fn fade() -> [Colored; 6] {
    let mut fade = quad(-0.9, -0.9, 0.9, 0.9, 0.5, 0.5, [0.95, 0.8, 0.3, 0.0]);
    for v in fade.iter_mut().filter(|v| v[0] > 0.0) {
        v[6] = 1.0;
    }
    fade
}

/// The square the pixel shader's coverage is written over.
pub fn square() -> [Colored; 6] {
    quad(-0.9, -0.9, 0.9, 0.9, 0.5, 0.5, [0.3, 0.8, 0.95, 1.0])
}

/// A multisampled target with the standard pattern, bindable as a texture when `read`.
fn target(
    gpu: &Gpu,
    samples: u32,
    read: bool,
) -> Result<(ID3D11Texture2D, ID3D11RenderTargetView)> {
    let mut bind = D3D11_BIND_RENDER_TARGET.0;
    if read {
        bind |= D3D11_BIND_SHADER_RESOURCE.0;
    }
    let mut desc = tex2d(SIZE, SIZE, FORMAT, D3D11_BIND_FLAG(bind));
    desc.SampleDesc = DXGI_SAMPLE_DESC {
        Count: samples,
        Quality: D3D11_STANDARD_MULTISAMPLE_PATTERN.0 as u32,
    };
    let texture = gpu.texture2d(&desc, &[])?;
    let view = gpu.rtv(&texture.cast()?, None)?;
    Ok((texture, view))
}

/// Resolves a multisampled target and reads it back.
fn resolve(gpu: &Gpu, texture: &ID3D11Texture2D) -> Result<image::DynamicImage> {
    let single = gpu.texture2d(&tex2d(SIZE, SIZE, FORMAT, D3D11_BIND_FLAG(0)), &[])?;
    unsafe {
        gpu.context
            .ResolveSubresource(&single, 0, texture, 0, FORMAT)
    };
    gpu.read(&single, 0, 0)
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut outputs = Vec::new();
    let c = &gpu.context;
    let multisample = gpu.rasterizer(&D3D11_RASTERIZER_DESC {
        MultisampleEnable: true.into(),
        ..solid()
    })?;
    let plain = gpu.blend(&no_blend())?;
    let fresh = |samples: u32| -> Result<(ID3D11Texture2D, ID3D11RenderTargetView)> {
        let (texture, rtv) = target(gpu, samples, samples == 4)?;
        gpu.begin(&rtv, None, SIZE, SIZE);
        colored_program(gpu)?;
        unsafe {
            c.RSSetState(&multisample);
            c.OMSetBlendState(&plain, None, u32::MAX);
        }
        Ok((texture, rtv))
    };

    for samples in [2, 4, 8] {
        let (texture, _rtv) = fresh(samples)?;
        draw_colored(gpu, &wedges())?;
        outputs.push(Output::picture(
            &format!("edges{samples}"),
            resolve(gpu, &texture)?,
        ));
    }

    let (texture, _rtv) = fresh(4)?;
    unsafe { c.OMSetBlendState(&plain, None, 0b0101) };
    draw_colored(gpu, &wedges())?;
    outputs.push(Output::picture("sample_mask", resolve(gpu, &texture)?));

    let (texture, _rtv) = fresh(4)?;
    let mut coverage = no_blend();
    coverage.AlphaToCoverageEnable = true.into();
    unsafe { c.OMSetBlendState(&gpu.blend(&coverage)?, None, u32::MAX) };
    draw_colored(gpu, &fade())?;
    outputs.push(Output::picture(
        "alpha_to_coverage",
        resolve(gpu, &texture)?,
    ));

    let (texture, _rtv) = fresh(4)?;
    unsafe { c.PSSetShader(&gpu.ps(dxbc!("msaa.ps_sample"))?, None) };
    draw_colored(gpu, &wedges())?;
    outputs.push(Output::picture("per_sample", resolve(gpu, &texture)?));

    let (texture, _rtv) = fresh(4)?;
    unsafe { c.PSSetShader(&gpu.ps(dxbc!("msaa.ps_coverage"))?, None) };
    draw_colored(gpu, &square())?;
    outputs.push(Output::picture("shader_coverage", resolve(gpu, &texture)?));

    // The wedges at 4 samples, then each quarter of a single-sampled picture shows one sample.
    let (texture, _rtv) = fresh(4)?;
    draw_colored(gpu, &wedges())?;
    let samples = gpu.srv(&texture.cast()?, None)?;
    let (single, rtv) = gpu.target(SIZE, SIZE, FORMAT)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    gpu.program(dxbc!("msaa.vs_full"), dxbc!("msaa.ps_load"), &[])?;
    unsafe {
        c.PSSetShaderResources(0, Some(&[Some(samples)]));
        c.Draw(3, 0);
    }
    outputs.push(Output::picture("load", gpu.read(&single, 0, 0)?));
    Ok(outputs)
}
