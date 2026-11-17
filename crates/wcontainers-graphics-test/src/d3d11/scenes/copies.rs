//! Copies: a whole texture, regions between mips at offsets, between formats of one typeless group,
//! block-compressed blocks into 64-bit integers, buffer ranges and partial updates, and a staging
//! texture written by the CPU into a default one; read back exactly.

use anyhow::Result;
use rand::RngCore;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::d3d11::{Gpu, checker, data, hex, seeded, tex2d};
use crate::output::Output;

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let mut outputs = Vec::new();
    let rgba = DXGI_FORMAT_R8G8B8A8_UNORM;

    // A 32 by 32 source with three mips, each its own colour.
    let levels: Vec<Vec<u8>> = (0..3).map(|m| checker(32 >> m, 32 >> m, m, 3)).collect();
    let mut desc = tex2d(32, 32, rgba, D3D11_BIND_SHADER_RESOURCE);
    desc.MipLevels = 3;
    let init: Vec<_> = levels
        .iter()
        .enumerate()
        .map(|(m, b)| data(b, (32 >> m) * 4, 0))
        .collect();
    let source = gpu.texture2d(&desc, &init)?;
    let whole = gpu.texture2d(&desc, &[])?;
    unsafe { c.CopyResource(&whole, &source) };
    outputs.push(Output::picture("resource_mip1", gpu.read(&whole, 1, 0)?));

    // Regions of each mip into a 64 by 64 texture of grey.
    let grey = vec![90u8; 64 * 64 * 4];
    let destination = gpu.texture2d(
        &tex2d(64, 64, rgba, D3D11_BIND_SHADER_RESOURCE),
        &[data(&grey, 256, 0)],
    )?;
    let region = |left, top, right, bottom| D3D11_BOX {
        left,
        top,
        front: 0,
        right,
        bottom,
        back: 1,
    };
    unsafe {
        c.CopySubresourceRegion(
            &destination,
            0,
            10,
            20,
            0,
            &source,
            0,
            Some(&region(4, 2, 28, 18)),
        );
        c.CopySubresourceRegion(&destination, 0, 40, 40, 0, &source, 1, None);
        c.CopySubresourceRegion(
            &destination,
            0,
            1,
            50,
            0,
            &source,
            2,
            Some(&region(1, 1, 7, 7)),
        );
        c.CopySubresourceRegion(
            &destination,
            0,
            50,
            2,
            0,
            &source,
            0,
            Some(&region(30, 30, 32, 32)),
        );
    }
    outputs.push(Output::picture("region", gpu.read(&destination, 0, 0)?));

    // Unsigned normalised texels copied into an unsigned integer texture of the same group.
    let integers = gpu.texture2d(
        &tex2d(
            32,
            32,
            DXGI_FORMAT_R8G8B8A8_UINT,
            D3D11_BIND_SHADER_RESOURCE,
        ),
        &[],
    )?;
    unsafe { c.CopySubresourceRegion(&integers, 0, 0, 0, 0, &source, 0, None) };
    outputs.push(Output::picture("uint", gpu.read(&integers, 0, 0)?));

    // BC1 blocks copied into a 64-bit integer texture, one texel per block.
    let mut blocks = vec![0u8; 8 * 8 * 8];
    seeded(17).fill_bytes(&mut blocks);
    let bc = gpu.texture2d(
        &tex2d(32, 32, DXGI_FORMAT_BC1_UNORM, D3D11_BIND_SHADER_RESOURCE),
        &[data(&blocks, 64, 0)],
    )?;
    let wide = gpu.texture2d(
        &tex2d(8, 8, DXGI_FORMAT_R32G32_UINT, D3D11_BIND_SHADER_RESOURCE),
        &[],
    )?;
    unsafe { c.CopySubresourceRegion(&wide, 0, 0, 0, 0, &bc, 0, None) };
    let staging = gpu.texture2d(
        &D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..tex2d(8, 8, DXGI_FORMAT_R32G32_UINT, D3D11_BIND_FLAG(0))
        },
        &[],
    )?;
    let mut copied = Vec::new();
    unsafe {
        c.CopyResource(&staging, &wide);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        c.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        for row in 0..8 {
            let start = mapped
                .pData
                .cast::<u8>()
                .add(row * mapped.RowPitch as usize);
            copied.extend_from_slice(std::slice::from_raw_parts(start, 64));
        }
        c.Unmap(&staging, 0);
    }
    let same = if copied == blocks {
        "the blocks arrived unchanged\n"
    } else {
        "the blocks changed\n"
    };
    outputs.push(Output::text(
        "bc_as_uint",
        format!("{same}{}", hex(&copied)),
    ));

    // Buffers: a range copied to an offset, then a few bytes replaced.
    let pattern: Vec<u8> = (0..=255u8).collect();
    let buffer = |bytes: &[u8]| {
        gpu.buffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: 256,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                ..Default::default()
            },
            Some(bytes),
        )
    };
    let from = buffer(&pattern)?;
    let to = buffer(&[0xee; 256])?;
    let update = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    unsafe {
        c.CopySubresourceRegion(&to, 0, 16, 0, 0, &from, 0, Some(&region(64, 0, 128, 1)));
        c.UpdateSubresource(
            &to,
            0,
            Some(&region(200, 0, 216, 1)),
            update.as_ptr().cast(),
            0,
            0,
        );
    }
    outputs.push(Output::text("buffer", hex(&gpu.read_buffer(&to)?)));

    // A staging texture written by the CPU, copied into a default one.
    let writable = gpu.texture2d(
        &D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            ..tex2d(16, 16, rgba, D3D11_BIND_FLAG(0))
        },
        &[],
    )?;
    let texels = checker(16, 16, 1, 4);
    unsafe {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        c.Map(&writable, 0, D3D11_MAP_WRITE, 0, Some(&mut mapped))?;
        for row in 0..16 {
            let to = mapped
                .pData
                .cast::<u8>()
                .add(row * mapped.RowPitch as usize);
            std::ptr::copy_nonoverlapping(texels[row * 64..].as_ptr(), to, 64);
        }
        c.Unmap(&writable, 0);
    }
    let fixed = gpu.texture2d(&tex2d(16, 16, rgba, D3D11_BIND_SHADER_RESOURCE), &[])?;
    unsafe { c.CopyResource(&fixed, &writable) };
    outputs.push(Output::picture("staging", gpu.read(&fixed, 0, 0)?));
    Ok(outputs)
}
