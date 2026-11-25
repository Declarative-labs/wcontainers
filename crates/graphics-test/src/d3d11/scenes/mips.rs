//! GenerateMips on 8-bit, sRGB and half-float textures of 64 by 48 texels (odd sizes further down),
//! every generated level read back.

use anyhow::Result;
use rand::Rng;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use crate::d3d11::{Gpu, seeded, tex2d};
use crate::output::Output;

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;
const LEVELS: u32 = 7;

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut outputs = Vec::new();
    let mut random = seeded(19);
    // Smooth gradients with seeded noise on top, as 0..1 values.
    let values: Vec<f32> = (0..WIDTH * HEIGHT * 4)
        .map(|i| {
            let (x, y, c) = ((i / 4) % WIDTH, (i / 4) / WIDTH, i % 4);
            let base = match c {
                0 => x as f32 / WIDTH as f32,
                1 => y as f32 / HEIGHT as f32,
                2 => ((x / 8 + y / 8) % 2) as f32,
                _ => 1.0 - (x + y) as f32 / (WIDTH + HEIGHT) as f32,
            };
            (base * 0.8 + random.random_range(0.0..0.2)).min(1.0)
        })
        .collect();
    for (name, format) in [
        ("rgba8", DXGI_FORMAT_R8G8B8A8_UNORM),
        ("srgb", DXGI_FORMAT_R8G8B8A8_UNORM_SRGB),
        ("rgba16f", DXGI_FORMAT_R16G16B16A16_FLOAT),
    ] {
        let (bytes, pitch): (Vec<u8>, u32) = if format == DXGI_FORMAT_R16G16B16A16_FLOAT {
            let halves: Vec<u16> = values
                .iter()
                .map(|&v| half::f16::from_f32(v * 4.0 - 1.0).to_bits())
                .collect();
            (bytemuck::cast_slice(&halves).to_vec(), WIDTH * 8)
        } else {
            (
                values.iter().map(|&v| (v * 255.0).round() as u8).collect(),
                WIDTH * 4,
            )
        };
        let mut desc = tex2d(
            WIDTH,
            HEIGHT,
            format,
            D3D11_BIND_FLAG(D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0),
        );
        desc.MipLevels = LEVELS;
        desc.MiscFlags = D3D11_RESOURCE_MISC_GENERATE_MIPS.0 as u32;
        let texture = gpu.texture2d(&desc, &[])?;
        let view = gpu.srv(&texture.cast()?, None)?;
        unsafe {
            gpu.context
                .UpdateSubresource(&texture, 0, None, bytes.as_ptr().cast(), pitch, 0);
            gpu.context.GenerateMips(&view);
        }
        for level in 1..LEVELS {
            outputs.push(Output::picture(
                &format!("{name}_{level}"),
                gpu.read(&texture, level, 0)?,
            ));
        }
    }
    Ok(outputs)
}
