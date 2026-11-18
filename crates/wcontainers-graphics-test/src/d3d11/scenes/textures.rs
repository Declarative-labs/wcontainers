//! Texture kinds sampled over a target: 1D with mips, 1D arrays, 2D arrays, 3D, cubes, cube arrays,
//! a 2D mip chain by explicit level, and every block-compressed format (BC1 to BC7, signed and sRGB
//! kinds) filled with seeded random blocks and read texel by texel into a float target.

use anyhow::Result;
use rand::RngCore;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use crate::d3d11::{Gpu, cell, checker, data, made, seeded, solid, tex2d};
use crate::dxbc;
use crate::output::Output;

pub const SIZE: u32 = 64;

/// The block-compressed formats drawn, with the bytes of a block of each; 32 by 32 texels each.
pub const BC: [(DXGI_FORMAT, usize); 16] = [
    (DXGI_FORMAT_BC1_UNORM, 8),
    (DXGI_FORMAT_BC1_UNORM_SRGB, 8),
    (DXGI_FORMAT_BC2_UNORM, 16),
    (DXGI_FORMAT_BC3_UNORM, 16),
    (DXGI_FORMAT_BC3_UNORM_SRGB, 16),
    (DXGI_FORMAT_BC4_UNORM, 8),
    (DXGI_FORMAT_BC4_SNORM, 8),
    (DXGI_FORMAT_BC5_UNORM, 16),
    (DXGI_FORMAT_BC5_SNORM, 16),
    (DXGI_FORMAT_BC6H_UF16, 16),
    (DXGI_FORMAT_BC6H_SF16, 16),
    (DXGI_FORMAT_BC7_UNORM, 16),
    (DXGI_FORMAT_BC7_UNORM_SRGB, 16),
    (DXGI_FORMAT_BC2_UNORM_SRGB, 16),
    (DXGI_FORMAT_BC7_UNORM, 16),
    (DXGI_FORMAT_BC6H_UF16, 16),
];

/// The parts the block-compressed formats are drawn in: BC1 to BC5 from feature level 10_0, BC6H
/// and BC7 from 11_0 (true).
pub const BC_PARTS: [(&str, bool); 2] = [("bc", false), ("bc67", true)];

/// Whether a block-compressed format is BC6H or BC7, which need feature level 11_0.
pub fn bc67(format: DXGI_FORMAT) -> bool {
    matches!(
        format,
        DXGI_FORMAT_BC6H_UF16
            | DXGI_FORMAT_BC6H_SF16
            | DXGI_FORMAT_BC7_UNORM
            | DXGI_FORMAT_BC7_UNORM_SRGB
    )
}

/// The random blocks of each block-compressed format, in the order of `BC`.
pub fn bc_blocks() -> Vec<Vec<u8>> {
    let mut random = seeded(11);
    BC.iter()
        .map(|&(_, block)| {
            let mut blocks = vec![0u8; 8 * 8 * block];
            random.fill_bytes(&mut blocks);
            blocks
        })
        .collect()
}

/// The initial data of every subresource, layer by layer and mip by mip, `index` naming the colour
/// of each.
pub fn subresources(
    width: u32,
    height: u32,
    depth: u32,
    mips: u32,
    layers: u32,
) -> (Vec<Vec<u8>>, Vec<(u32, u32)>) {
    let mut bytes = Vec::new();
    let mut pitches = Vec::new();
    for layer in 0..layers {
        for mip in 0..mips {
            let (w, h, d) = (
                (width >> mip).max(1),
                (height >> mip).max(1),
                (depth >> mip).max(1),
            );
            let mut texels = Vec::new();
            for slice in 0..d {
                texels.extend(checker(
                    w,
                    h,
                    layer * mips + mip + slice * 3,
                    layers * mips + d * 3,
                ));
            }
            bytes.push(texels);
            pitches.push((w * 4, w * h * 4));
        }
    }
    (bytes, pitches)
}

fn init(bytes: &[Vec<u8>], pitches: &[(u32, u32)]) -> Vec<D3D11_SUBRESOURCE_DATA> {
    bytes
        .iter()
        .zip(pitches)
        .map(|(b, &(p, s))| data(b, p, s))
        .collect()
}

/// Draws a full-target triangle with `ps` reading `view` into a fresh target and reads it back.
fn show(
    gpu: &Gpu,
    ps: &[u8],
    view: &ID3D11ShaderResourceView,
    format: DXGI_FORMAT,
    width: u32,
    height: u32,
) -> Result<image::DynamicImage> {
    let (target, rtv) = gpu.target(width, height, format)?;
    gpu.begin(&rtv, None, width, height);
    gpu.program(dxbc!("common.vs_full"), ps, &[])?;
    unsafe {
        gpu.context
            .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        gpu.context
            .PSSetShaderResources(0, Some(&[Some(view.clone())]));
        gpu.context.Draw(3, 0);
    }
    gpu.read(&target, 0, 0)
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut outputs = Vec::new();
    let c = &gpu.context;
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
    unsafe {
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.PSSetSamplers(0, Some(&[Some(sampler)]));
    }
    let rgba = DXGI_FORMAT_R8G8B8A8_UNORM;
    let srv_bind = D3D11_BIND_SHADER_RESOURCE.0 as u32;

    // 1D: 64 texels with all their mips, and 4 layers of 64 without.
    for (part, mips, layers, ps) in [
        ("1d", 7, 1, dxbc!("textures.ps_1d")),
        ("1d_array", 1, 4, dxbc!("textures.ps_1d_array")),
    ] {
        let (bytes, pitches) = subresources(64, 1, 1, mips, layers);
        let desc = D3D11_TEXTURE1D_DESC {
            Width: 64,
            MipLevels: mips,
            ArraySize: layers,
            Format: rgba,
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: srv_bind,
            ..Default::default()
        };
        let init = init(&bytes, &pitches);
        let texture: ID3D11Texture1D = made(|o| unsafe {
            gpu.device
                .CreateTexture1D(&desc, Some(init.as_ptr()), Some(o))
        })?;
        let view = gpu.srv(&texture.cast()?, None)?;
        outputs.push(Output::picture(
            part,
            show(gpu, ps, &view, rgba, SIZE, SIZE)?,
        ));
    }

    // 2D arrays, cubes and cube arrays: 4, 6 and 12 layers of 16 by 16.
    for (part, layers, cube, ps, level) in [
        (
            "2d_array",
            4,
            false,
            dxbc!("textures.ps_2d_array"),
            D3D_FEATURE_LEVEL_10_0,
        ),
        (
            "cube",
            6,
            true,
            dxbc!("textures.ps_cube"),
            D3D_FEATURE_LEVEL_10_0,
        ),
        (
            "cube_array",
            12,
            true,
            dxbc!("textures.ps_cube_array"),
            D3D_FEATURE_LEVEL_10_1,
        ),
    ] {
        if let Some(why) = gpu.lacks_level(level) {
            outputs.push(Output::absent(part, why));
            continue;
        }
        let (bytes, pitches) = subresources(16, 16, 1, 1, layers);
        let mut desc = tex2d(16, 16, rgba, D3D11_BIND_SHADER_RESOURCE);
        desc.ArraySize = layers;
        desc.Usage = D3D11_USAGE_IMMUTABLE;
        if cube {
            desc.MiscFlags = D3D11_RESOURCE_MISC_TEXTURECUBE.0 as u32;
        }
        let texture = gpu.texture2d(&desc, &init(&bytes, &pitches))?;
        let view = gpu.srv(&texture.cast()?, None)?;
        outputs.push(Output::picture(
            part,
            show(gpu, ps, &view, rgba, SIZE * 2, SIZE)?,
        ));
    }

    // 3D: 16 by 16 by 8.
    let (bytes, pitches) = subresources(16, 16, 8, 1, 1);
    let desc = D3D11_TEXTURE3D_DESC {
        Width: 16,
        Height: 16,
        Depth: 8,
        MipLevels: 1,
        Format: rgba,
        Usage: D3D11_USAGE_IMMUTABLE,
        BindFlags: srv_bind,
        ..Default::default()
    };
    let init3 = init(&bytes, &pitches);
    let texture: ID3D11Texture3D = made(|o| unsafe {
        gpu.device
            .CreateTexture3D(&desc, Some(init3.as_ptr()), Some(o))
    })?;
    let view = gpu.srv(&texture.cast()?, None)?;
    outputs.push(Output::picture(
        "3d",
        show(gpu, dxbc!("textures.ps_3d"), &view, rgba, SIZE, SIZE)?,
    ));

    // A 2D mip chain read at levels 0 to 6 down the target.
    let (bytes, pitches) = subresources(64, 64, 1, 7, 1);
    let mut desc = tex2d(64, 64, rgba, D3D11_BIND_SHADER_RESOURCE);
    desc.MipLevels = 7;
    desc.Usage = D3D11_USAGE_IMMUTABLE;
    let texture = gpu.texture2d(&desc, &init(&bytes, &pitches))?;
    let view = gpu.srv(&texture.cast()?, None)?;
    outputs.push(Output::picture(
        "mips",
        show(gpu, dxbc!("textures.ps_mips"), &view, rgba, SIZE, SIZE * 2)?,
    ));

    // Block-compressed: 32 by 32 texels of random blocks per format, read exactly, in the parts
    // of `BC_PARTS`.
    let all = bc_blocks();
    for (part, newer) in BC_PARTS {
        if newer && let Some(why) = gpu.lacks_level(D3D_FEATURE_LEVEL_11_0) {
            outputs.push(Output::absent(part, why));
            continue;
        }
        let (target, rtv) = gpu.target(128, 128, DXGI_FORMAT_R32G32B32A32_FLOAT)?;
        gpu.begin(&rtv, None, 128, 128);
        gpu.program(dxbc!("common.vs_full"), dxbc!("textures.ps_load"), &[])?;
        let formats = BC
            .into_iter()
            .zip(&all)
            .filter(|((f, _), _)| bc67(*f) == newer);
        for (i, ((format, block), blocks)) in formats.enumerate() {
            let mut desc = tex2d(32, 32, format, D3D11_BIND_SHADER_RESOURCE);
            desc.Usage = D3D11_USAGE_IMMUTABLE;
            let texture = gpu.texture2d(&desc, &[data(blocks, 8 * block as u32, 0)])?;
            let view = gpu.srv(&texture.cast()?, None)?;
            unsafe {
                c.RSSetViewports(Some(&[cell(i as u32, 4, 32.0)]));
                c.PSSetShaderResources(0, Some(&[Some(view)]));
                c.Draw(3, 0);
            }
        }
        outputs.push(Output::picture(part, gpu.read(&target, 0, 0)?));
    }
    Ok(outputs)
}
