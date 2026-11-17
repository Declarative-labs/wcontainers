//! Stencil: separate front and back operations on overlapping triangles of both windings, then
//! replace, saturating and wrapping steps, invert, a write mask, the depth-fail operation and a read
//! mask, shown as colours by equality tests and read back exactly.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{Colored, blend::no_blend, colored_program, depth_test, draw_colored, hue, quad};
use crate::d3d11::{Gpu, solid};
use crate::output::Output;

const SIZE: u32 = 192;

fn op(pass: D3D11_STENCIL_OP, func: D3D11_COMPARISON_FUNC) -> D3D11_DEPTH_STENCILOP_DESC {
    D3D11_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D11_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
        StencilPassOp: pass,
        StencilFunc: func,
    }
}

/// Stencil on, depth always passing and not written; the same operation for both faces.
fn stencil(
    front: D3D11_DEPTH_STENCILOP_DESC,
    back: D3D11_DEPTH_STENCILOP_DESC,
    read: u8,
    write: u8,
) -> D3D11_DEPTH_STENCIL_DESC {
    D3D11_DEPTH_STENCIL_DESC {
        StencilEnable: true.into(),
        StencilReadMask: read,
        StencilWriteMask: write,
        FrontFace: front,
        BackFace: back,
        ..depth_test(D3D11_COMPARISON_ALWAYS, false)
    }
}

/// A triangle, clockwise (front-facing) or not.
fn triangle(cx: f32, cy: f32, r: f32, clockwise: bool) -> [Colored; 3] {
    let white = |x: f32, y: f32| [x, y, 0.5, 1.0, 1.0, 1.0, 1.0];
    let (a, b, c) = (
        white(cx, cy + r),
        white(cx + r, cy - r),
        white(cx - r, cy - r),
    );
    if clockwise { [a, b, c] } else { [a, c, b] }
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (depth, dsv) = gpu.depth(SIZE, SIZE, DXGI_FORMAT_D24_UNORM_S8_UINT)?;
    gpu.begin(&rtv, Some(&dsv), SIZE, SIZE);
    colored_program(gpu)?;
    let c = &gpu.context;
    let mut masked = no_blend();
    masked.RenderTarget[0].RenderTargetWriteMask = 0;
    unsafe {
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.OMSetBlendState(&gpu.blend(&masked)?, None, u32::MAX);
    }
    let set = |desc: &D3D11_DEPTH_STENCIL_DESC, reference: u32| -> Result<()> {
        unsafe { c.OMSetDepthStencilState(&gpu.depth_stencil(desc)?, reference) };
        Ok(())
    };

    // Front faces count up, back faces down, wrapping.
    set(
        &stencil(
            op(D3D11_STENCIL_OP_INCR, D3D11_COMPARISON_ALWAYS),
            op(D3D11_STENCIL_OP_DECR, D3D11_COMPARISON_ALWAYS),
            0xff,
            0xff,
        ),
        0,
    )?;
    let mut fan = Vec::new();
    for k in 0..6 {
        let angle = k as f32 * std::f32::consts::TAU / 6.0;
        fan.extend(triangle(
            0.25 * angle.cos(),
            0.25 * angle.sin(),
            0.45,
            k % 2 == 0,
        ));
    }
    draw_colored(gpu, &fan)?;

    let always = |pass| op(pass, D3D11_COMPARISON_ALWAYS);
    let rect = |x: f32, y: f32| quad(x, y, x + 0.5, y + 0.5, 0.5, 0.5, [1.0; 4]);
    set(
        &stencil(
            always(D3D11_STENCIL_OP_REPLACE),
            always(D3D11_STENCIL_OP_KEEP),
            0xff,
            0xff,
        ),
        0x80,
    )?;
    draw_colored(gpu, &rect(-0.95, 0.45))?;
    set(
        &stencil(
            always(D3D11_STENCIL_OP_INCR_SAT),
            always(D3D11_STENCIL_OP_KEEP),
            0xff,
            0xff,
        ),
        0,
    )?;
    for _ in 0..3 {
        draw_colored(gpu, &rect(0.45, 0.45))?;
    }
    set(
        &stencil(
            always(D3D11_STENCIL_OP_INVERT),
            always(D3D11_STENCIL_OP_KEEP),
            0xff,
            0xff,
        ),
        0,
    )?;
    draw_colored(gpu, &rect(-0.95, -0.95))?;
    set(
        &stencil(
            always(D3D11_STENCIL_OP_DECR_SAT),
            always(D3D11_STENCIL_OP_KEEP),
            0xff,
            0xff,
        ),
        0,
    )?;
    draw_colored(gpu, &rect(0.45, -0.95))?;
    set(
        &stencil(
            always(D3D11_STENCIL_OP_REPLACE),
            always(D3D11_STENCIL_OP_KEEP),
            0xff,
            0x0f,
        ),
        0xf7,
    )?;
    draw_colored(gpu, &quad(-0.2, -0.95, 0.2, 0.95, 0.5, 0.5, [1.0; 4]))?;
    // Where a near quad is in front, the far one's depth test fails and the stencil takes 0x40;
    // elsewhere it counts up.
    set(&depth_test(D3D11_COMPARISON_LESS, true), 0)?;
    draw_colored(gpu, &quad(-0.9, -0.15, -0.3, 0.15, 0.2, 0.2, [1.0; 4]))?;
    let mut depth_fail = stencil(
        always(D3D11_STENCIL_OP_INCR),
        always(D3D11_STENCIL_OP_INCR),
        0xff,
        0xff,
    );
    depth_fail.DepthFunc = D3D11_COMPARISON_LESS;
    depth_fail.FrontFace.StencilDepthFailOp = D3D11_STENCIL_OP_REPLACE;
    set(&depth_fail, 0x40)?;
    draw_colored(gpu, &quad(-0.95, -0.25, -0.05, 0.25, 0.8, 0.8, [1.0; 4]))?;

    // Colours where the stencil equals each value seen, then where its low four bits are 7 (by
    // the read mask) in grey.
    unsafe { c.OMSetBlendState(&gpu.blend(&no_blend())?, None, u32::MAX) };
    let values: [u32; 12] = [
        1, 2, 3, 0xff, 0xfe, 0xfd, 0x80, 0x7f, 0x40, 0x41, 0x07, 0xf8,
    ];
    for (i, &value) in values.iter().enumerate() {
        let [r, g, b] = hue(i as u32, values.len() as u32);
        set(
            &stencil(
                op(D3D11_STENCIL_OP_KEEP, D3D11_COMPARISON_EQUAL),
                op(D3D11_STENCIL_OP_KEEP, D3D11_COMPARISON_EQUAL),
                0xff,
                0,
            ),
            value,
        )?;
        draw_colored(gpu, &quad(-1.0, -1.0, 1.0, 1.0, 0.5, 0.5, [r, g, b, 1.0]))?;
    }
    set(
        &stencil(
            op(D3D11_STENCIL_OP_KEEP, D3D11_COMPARISON_EQUAL),
            op(D3D11_STENCIL_OP_KEEP, D3D11_COMPARISON_EQUAL),
            0x0f,
            0,
        ),
        0x07,
    )?;
    draw_colored(
        gpu,
        &quad(-1.0, -1.0, 1.0, -0.6, 0.5, 0.5, [0.5, 0.5, 0.5, 1.0]),
    )?;
    Ok(vec![
        Output::picture("", gpu.read(&target, 0, 0)?),
        Output::picture("values", gpu.read(&depth, 0, 0)?),
    ])
}
