//! Rasterizer state: each cull mode with either winding as front, wireframe filling, the scissor
//! test on and off, and eight viewports with a scissor rectangle each, chosen by a geometry shader.

use anyhow::Result;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{Colored, colored_layout, colored_program, draw_colored, hue, quad};
use crate::d3d11::{Gpu, cell, solid};
use crate::dxbc;
use crate::output::Output;

/// A clockwise red triangle on the left and an anticlockwise green one on the right.
fn windings() -> Vec<Colored> {
    let red = |x: f32, y: f32| [x, y, 0.5, 0.9, 0.2, 0.2, 1.0];
    let green = |x: f32, y: f32| [x, y, 0.5, 0.2, 0.9, 0.3, 1.0];
    vec![
        red(-0.5, 0.8),
        red(-0.1, -0.8),
        red(-0.9, -0.8),
        green(0.5, 0.8),
        green(0.1, -0.8),
        green(0.9, -0.8),
    ]
}

/// Three triangles of a fan, coloured by corner.
fn fan() -> Vec<Colored> {
    let points = [
        [0.0f32, 0.0],
        [0.8, -0.6],
        [0.7, 0.6],
        [-0.3, 0.85],
        [-0.85, -0.2],
    ];
    let mut v = Vec::new();
    for k in 1..4 {
        for p in [points[0], points[k], points[k + 1]] {
            let [r, g, b] = hue(v.len() as u32, 9);
            v.push([p[0], p[1], 0.5, r, g, b, 1.0]);
        }
    }
    v
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut outputs = Vec::new();
    let c = &gpu.context;
    colored_program(gpu)?;

    let (target, rtv) = gpu.target(192, 128, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, 192, 128);
    let culls = [D3D11_CULL_NONE, D3D11_CULL_FRONT, D3D11_CULL_BACK];
    for (i, (ccw, cull)) in [false, true]
        .into_iter()
        .flat_map(|ccw| culls.map(|c| (ccw, c)))
        .enumerate()
    {
        let state = gpu.rasterizer(&D3D11_RASTERIZER_DESC {
            CullMode: cull,
            FrontCounterClockwise: ccw.into(),
            ..solid()
        })?;
        unsafe {
            c.RSSetState(&state);
            c.RSSetViewports(Some(&[cell(i as u32, 3, 64.0)]));
        }
        draw_colored(gpu, &windings())?;
    }
    outputs.push(Output::picture("cull", gpu.read(&target, 0, 0)?));

    let (target, rtv) = gpu.target(256, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, 256, 64);
    let fills = [
        (D3D11_FILL_WIREFRAME, D3D11_CULL_NONE),
        (D3D11_FILL_WIREFRAME, D3D11_CULL_BACK),
        (D3D11_FILL_WIREFRAME, D3D11_CULL_FRONT),
        (D3D11_FILL_SOLID, D3D11_CULL_NONE),
    ];
    for (i, (fill, cull)) in fills.into_iter().enumerate() {
        let state = gpu.rasterizer(&D3D11_RASTERIZER_DESC {
            FillMode: fill,
            CullMode: cull,
            ..solid()
        })?;
        unsafe {
            c.RSSetState(&state);
            c.RSSetViewports(Some(&[cell(i as u32, 4, 64.0)]));
        }
        let mut v = fan();
        v.extend(windings());
        draw_colored(gpu, &v)?;
    }
    outputs.push(Output::picture("fill", gpu.read(&target, 0, 0)?));

    // The same rectangle set with the scissor test on, then off.
    let (target, rtv) = gpu.target(128, 64, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, 128, 64);
    for (i, on) in [true, false].into_iter().enumerate() {
        let state = gpu.rasterizer(&D3D11_RASTERIZER_DESC {
            ScissorEnable: on.into(),
            ..solid()
        })?;
        let left = 64 * i as i32;
        unsafe {
            c.RSSetState(&state);
            c.RSSetViewports(Some(&[cell(i as u32, 2, 64.0)]));
            c.RSSetScissorRects(Some(&[RECT {
                left: left + 13,
                top: 9,
                right: left + 47,
                bottom: 51,
            }]));
        }
        draw_colored(gpu, &fan())?;
    }
    outputs.push(Output::picture("scissor", gpu.read(&target, 0, 0)?));

    // Eight viewports of different sizes and depth ranges, each with its own scissor rectangle.
    let (target, rtv) = gpu.target(256, 128, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, 256, 128);
    gpu.program(dxbc!("raster.vs"), dxbc!("raster.ps"), &colored_layout())?;
    let viewports: Vec<D3D11_VIEWPORT> = (0..8)
        .map(|i| {
            let mut v = cell(i, 4, 64.0);
            let shrink = 3.0 * i as f32;
            v.TopLeftX += shrink;
            v.TopLeftY += shrink / 2.0;
            v.Width -= 2.0 * shrink;
            v.Height -= shrink;
            v
        })
        .collect();
    let scissors: Vec<RECT> = (0..8)
        .map(|i| {
            let (x, y) = (64 * (i % 4), 64 * (i / 4));
            RECT {
                left: x + 2 * i,
                top: y + i,
                right: x + 64 - i,
                bottom: y + 60 - 3 * i,
            }
        })
        .collect();
    unsafe {
        c.GSSetShader(&gpu.gs(dxbc!("raster.gs"))?, None);
        c.RSSetState(&gpu.rasterizer(&D3D11_RASTERIZER_DESC {
            ScissorEnable: true.into(),
            ..solid()
        })?);
        c.RSSetViewports(Some(&viewports));
        c.RSSetScissorRects(Some(&scissors));
    }
    let mut v = fan();
    v.extend(quad(
        -0.95,
        -0.95,
        -0.5,
        -0.5,
        0.5,
        0.5,
        [1.0, 1.0, 1.0, 1.0],
    ));
    gpu.vertices(0, bytemuck::cast_slice(&v), 28)?;
    unsafe {
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.Draw(v.len() as u32, 0);
        c.GSSetShader(None, None);
    }
    outputs.push(Output::picture("viewports", gpu.read(&target, 0, 0)?));
    Ok(outputs)
}
