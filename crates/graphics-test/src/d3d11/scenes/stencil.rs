//! Stencil: separate front and back operations on overlapping triangles of both windings, then
//! replace, saturating and wrapping steps, invert, a write mask, the depth-fail operation and a read
//! mask, shown as colours by equality tests and read back exactly.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{Colored, blend::no_blend, colored_program, depth_test, draw_colored, hue, quad};
use crate::d3d11::{Gpu, solid};
use crate::output::Output;

pub const SIZE: u32 = 192;

pub fn op(pass: D3D11_STENCIL_OP, func: D3D11_COMPARISON_FUNC) -> D3D11_DEPTH_STENCILOP_DESC {
    D3D11_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D11_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
        StencilPassOp: pass,
        StencilFunc: func,
    }
}

/// Stencil on, depth always passing and not written; the same operation for both faces.
pub fn stencil(
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
pub fn triangle(cx: f32, cy: f32, r: f32, clockwise: bool) -> [Colored; 3] {
    let white = |x: f32, y: f32| [x, y, 0.5, 1.0, 1.0, 1.0, 1.0];
    let (a, b, c) = (
        white(cx, cy + r),
        white(cx + r, cy - r),
        white(cx - r, cy - r),
    );
    if clockwise { [a, b, c] } else { [a, c, b] }
}

/// One draw of the scene: its depth and stencil state, stencil reference, whether it writes
/// colour, and its vertices.
pub struct Step {
    pub desc: D3D11_DEPTH_STENCIL_DESC,
    pub reference: u32,
    pub colour: bool,
    pub vertices: Vec<Colored>,
}

/// The scene's draws, in order; both APIs draw these.
pub fn steps() -> Vec<Step> {
    let mut steps = Vec::new();
    let mut add = |desc, reference, colour, vertices: &[Colored]| {
        steps.push(Step {
            desc,
            reference,
            colour,
            vertices: vertices.to_vec(),
        })
    };
    // Front faces count up, back faces down, wrapping.
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
    add(
        stencil(
            op(D3D11_STENCIL_OP_INCR, D3D11_COMPARISON_ALWAYS),
            op(D3D11_STENCIL_OP_DECR, D3D11_COMPARISON_ALWAYS),
            0xff,
            0xff,
        ),
        0,
        false,
        &fan,
    );

    let always = |pass| op(pass, D3D11_COMPARISON_ALWAYS);
    let keep = always(D3D11_STENCIL_OP_KEEP);
    let rect = |x: f32, y: f32| quad(x, y, x + 0.5, y + 0.5, 0.5, 0.5, [1.0; 4]);
    add(
        stencil(always(D3D11_STENCIL_OP_REPLACE), keep, 0xff, 0xff),
        0x80,
        false,
        &rect(-0.95, 0.45),
    );
    for _ in 0..3 {
        add(
            stencil(always(D3D11_STENCIL_OP_INCR_SAT), keep, 0xff, 0xff),
            0,
            false,
            &rect(0.45, 0.45),
        );
    }
    add(
        stencil(always(D3D11_STENCIL_OP_INVERT), keep, 0xff, 0xff),
        0,
        false,
        &rect(-0.95, -0.95),
    );
    add(
        stencil(always(D3D11_STENCIL_OP_DECR_SAT), keep, 0xff, 0xff),
        0,
        false,
        &rect(0.45, -0.95),
    );
    add(
        stencil(always(D3D11_STENCIL_OP_REPLACE), keep, 0xff, 0x0f),
        0xf7,
        false,
        &quad(-0.2, -0.95, 0.2, 0.95, 0.5, 0.5, [1.0; 4]),
    );
    // Where a near quad is in front, the far one's depth test fails and the stencil takes 0x40;
    // elsewhere it counts up.
    add(
        depth_test(D3D11_COMPARISON_LESS, true),
        0,
        false,
        &quad(-0.9, -0.15, -0.3, 0.15, 0.2, 0.2, [1.0; 4]),
    );
    let mut depth_fail = stencil(
        always(D3D11_STENCIL_OP_INCR),
        always(D3D11_STENCIL_OP_INCR),
        0xff,
        0xff,
    );
    depth_fail.DepthFunc = D3D11_COMPARISON_LESS;
    depth_fail.FrontFace.StencilDepthFailOp = D3D11_STENCIL_OP_REPLACE;
    add(
        depth_fail,
        0x40,
        false,
        &quad(-0.95, -0.25, -0.05, 0.25, 0.8, 0.8, [1.0; 4]),
    );

    // Colours where the stencil equals each value seen, then where its low four bits are 7 (by
    // the read mask) in grey.
    let values: [u32; 12] = [
        1, 2, 3, 0xff, 0xfe, 0xfd, 0x80, 0x7f, 0x40, 0x41, 0x07, 0xf8,
    ];
    let equal = op(D3D11_STENCIL_OP_KEEP, D3D11_COMPARISON_EQUAL);
    for (i, &value) in values.iter().enumerate() {
        let [r, g, b] = hue(i as u32, values.len() as u32);
        add(
            stencil(equal, equal, 0xff, 0),
            value,
            true,
            &quad(-1.0, -1.0, 1.0, 1.0, 0.5, 0.5, [r, g, b, 1.0]),
        );
    }
    add(
        stencil(equal, equal, 0x0f, 0),
        0x07,
        true,
        &quad(-1.0, -1.0, 1.0, -0.6, 0.5, 0.5, [0.5, 0.5, 0.5, 1.0]),
    );
    steps
}

/// The blend state of a step: colour written or not.
pub fn colour_blend(colour: bool) -> D3D11_BLEND_DESC {
    let mut blend = no_blend();
    if !colour {
        blend.RenderTarget[0].RenderTargetWriteMask = 0;
    }
    blend
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (depth, dsv) = gpu.depth(SIZE, SIZE, DXGI_FORMAT_D24_UNORM_S8_UINT)?;
    gpu.begin(&rtv, Some(&dsv), SIZE, SIZE);
    colored_program(gpu)?;
    let c = &gpu.context;
    unsafe { c.RSSetState(&gpu.rasterizer(&solid())?) };
    for step in steps() {
        unsafe {
            c.OMSetBlendState(&gpu.blend(&colour_blend(step.colour))?, None, u32::MAX);
            c.OMSetDepthStencilState(&gpu.depth_stencil(&step.desc)?, step.reference);
        }
        draw_colored(gpu, &step.vertices)?;
    }
    Ok(vec![
        Output::picture("", gpu.read(&target, 0, 0)?),
        Output::picture("values", gpu.read(&depth, 0, 0)?),
    ])
}
