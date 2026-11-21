//! Blending: every colour source factor against every destination factor (dual-source factors and
//! the blend factor constant included), every alpha factor pair, every colour operation against
//! every alpha operation, and every write mask. Each cell first gets a gradient as destination.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::d3d11::{Gpu, cell, solid};
use crate::dxbc;
use crate::output::Output;

pub const SIZE: u32 = 256;
pub const CONSTANT: [f32; 4] = [0.3, 0.6, 0.9, 0.45];

const COLOR_FACTORS: [D3D11_BLEND; 17] = [
    D3D11_BLEND_ZERO,
    D3D11_BLEND_ONE,
    D3D11_BLEND_SRC_COLOR,
    D3D11_BLEND_INV_SRC_COLOR,
    D3D11_BLEND_SRC_ALPHA,
    D3D11_BLEND_INV_SRC_ALPHA,
    D3D11_BLEND_DEST_ALPHA,
    D3D11_BLEND_INV_DEST_ALPHA,
    D3D11_BLEND_DEST_COLOR,
    D3D11_BLEND_INV_DEST_COLOR,
    D3D11_BLEND_SRC_ALPHA_SAT,
    D3D11_BLEND_BLEND_FACTOR,
    D3D11_BLEND_INV_BLEND_FACTOR,
    D3D11_BLEND_SRC1_COLOR,
    D3D11_BLEND_INV_SRC1_COLOR,
    D3D11_BLEND_SRC1_ALPHA,
    D3D11_BLEND_INV_SRC1_ALPHA,
];

const ALPHA_FACTORS: [D3D11_BLEND; 11] = [
    D3D11_BLEND_ZERO,
    D3D11_BLEND_ONE,
    D3D11_BLEND_SRC_ALPHA,
    D3D11_BLEND_INV_SRC_ALPHA,
    D3D11_BLEND_DEST_ALPHA,
    D3D11_BLEND_INV_DEST_ALPHA,
    D3D11_BLEND_SRC_ALPHA_SAT,
    D3D11_BLEND_BLEND_FACTOR,
    D3D11_BLEND_INV_BLEND_FACTOR,
    D3D11_BLEND_SRC1_ALPHA,
    D3D11_BLEND_INV_SRC1_ALPHA,
];

const OPS: [D3D11_BLEND_OP; 5] = [
    D3D11_BLEND_OP_ADD,
    D3D11_BLEND_OP_SUBTRACT,
    D3D11_BLEND_OP_REV_SUBTRACT,
    D3D11_BLEND_OP_MIN,
    D3D11_BLEND_OP_MAX,
];

pub fn target(
    src: D3D11_BLEND,
    dst: D3D11_BLEND,
    op: D3D11_BLEND_OP,
    src_a: D3D11_BLEND,
    dst_a: D3D11_BLEND,
    op_a: D3D11_BLEND_OP,
) -> D3D11_RENDER_TARGET_BLEND_DESC {
    D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: true.into(),
        SrcBlend: src,
        DestBlend: dst,
        BlendOp: op,
        SrcBlendAlpha: src_a,
        DestBlendAlpha: dst_a,
        BlendOpAlpha: op_a,
        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
    }
}

/// Blending off, every channel written.
pub fn no_blend() -> D3D11_BLEND_DESC {
    let mut desc = D3D11_BLEND_DESC::default();
    let mut t = target(
        D3D11_BLEND_ONE,
        D3D11_BLEND_ZERO,
        D3D11_BLEND_OP_ADD,
        D3D11_BLEND_ONE,
        D3D11_BLEND_ZERO,
        D3D11_BLEND_OP_ADD,
    );
    t.BlendEnable = false.into();
    desc.RenderTarget = [t; 8];
    desc
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let vs = gpu.vs(dxbc!("blend.vs"))?;
    let dual = gpu.ps(dxbc!("blend.ps_dual"))?;
    let dest = gpu.ps(dxbc!("blend.ps_dest"))?;
    let rect = gpu.buffer(
        &D3D11_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            ..Default::default()
        },
        None,
    )?;
    unsafe {
        let c = &gpu.context;
        c.VSSetShader(&vs, None);
        c.IASetInputLayout(None);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.PSSetConstantBuffers(0, Some(&[Some(rect.clone())]));
    }
    // Draws each cell's destination gradient, then the source with that cell's blend.
    let picture = |columns: u32, size: u32, cells: Vec<D3D11_BLEND_DESC>| -> Result<Output> {
        let (texture, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
        gpu.begin(&rtv, None, SIZE, SIZE);
        let plain = gpu.blend(&no_blend())?;
        for (i, desc) in cells.iter().enumerate() {
            let viewport = cell(i as u32, columns, size as f32);
            let r = [
                viewport.TopLeftX,
                viewport.TopLeftY,
                viewport.Width,
                viewport.Height,
            ];
            let state = gpu.blend(desc)?;
            unsafe {
                let c = &gpu.context;
                c.UpdateSubresource(&rect, 0, None, r.as_ptr().cast(), 0, 0);
                c.RSSetViewports(Some(&[viewport]));
                c.OMSetBlendState(&plain, None, u32::MAX);
                c.PSSetShader(&dest, None);
                c.Draw(3, 0);
                c.OMSetBlendState(&state, Some(&CONSTANT), u32::MAX);
                c.PSSetShader(&dual, None);
                c.Draw(3, 0);
            }
        }
        Ok(Output::picture("", gpu.read(&texture, 0, 0)?))
    };
    let mut outputs = Vec::new();
    for (part, columns, size, cells) in parts() {
        let mut output = picture(columns, size, cells)?;
        output.part = part.to_owned();
        outputs.push(output);
    }
    Ok(outputs)
}

/// The pictures: their names, columns, cell sizes and each cell's blend.
pub fn parts() -> Vec<(&'static str, u32, u32, Vec<D3D11_BLEND_DESC>)> {
    let one_target = |t: D3D11_RENDER_TARGET_BLEND_DESC| {
        let mut desc = D3D11_BLEND_DESC::default();
        desc.RenderTarget[0] = t;
        desc
    };

    let mut factors = Vec::new();
    for &src in &COLOR_FACTORS {
        for &dst in &COLOR_FACTORS {
            factors.push(one_target(target(
                src,
                dst,
                D3D11_BLEND_OP_ADD,
                D3D11_BLEND_ONE,
                D3D11_BLEND_ZERO,
                D3D11_BLEND_OP_ADD,
            )));
        }
    }
    let mut alpha = Vec::new();
    for &src in &ALPHA_FACTORS {
        for &dst in &ALPHA_FACTORS {
            alpha.push(one_target(target(
                D3D11_BLEND_ONE,
                D3D11_BLEND_ZERO,
                D3D11_BLEND_OP_ADD,
                src,
                dst,
                D3D11_BLEND_OP_ADD,
            )));
        }
    }
    let mut ops = Vec::new();
    for &op in &OPS {
        for &op_a in &OPS {
            ops.push(one_target(target(
                D3D11_BLEND_SRC_ALPHA,
                D3D11_BLEND_ONE,
                op,
                D3D11_BLEND_ONE,
                D3D11_BLEND_ONE,
                op_a,
            )));
        }
    }
    let masks = (0..16u8)
        .map(|m| {
            let mut t = target(
                D3D11_BLEND_ONE,
                D3D11_BLEND_ZERO,
                D3D11_BLEND_OP_ADD,
                D3D11_BLEND_ONE,
                D3D11_BLEND_ZERO,
                D3D11_BLEND_OP_ADD,
            );
            t.BlendEnable = false.into();
            t.RenderTargetWriteMask = m;
            one_target(t)
        })
        .collect();

    vec![
        ("factors", 17, 15, factors),
        ("alpha", 11, 23, alpha),
        ("ops", 5, 51, ops),
        ("masks", 4, 64, masks),
    ]
}
