//! Geometry shaders: points grown into quads, triangles with adjacency coloured by their
//! neighbours, three strips per input triangle with primitive ids, and instancing into the layers
//! of an array target.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use super::{Colored, colored_layout, hue};
use crate::d3d11::{Gpu, solid, tex2d};
use crate::dxbc;
use crate::output::Output;

const SIZE: u32 = 128;

fn draw(
    gpu: &Gpu,
    gs: &[u8],
    ps: &[u8],
    topology: D3D_PRIMITIVE_TOPOLOGY,
    vertices: &[Colored],
) -> Result<()> {
    gpu.program(dxbc!("geometry.vs"), ps, &colored_layout())?;
    gpu.vertices(0, bytemuck::cast_slice(vertices), 28)?;
    unsafe {
        gpu.context.RSSetState(&gpu.rasterizer(&solid())?);
        gpu.context.GSSetShader(&gpu.gs(gs)?, None);
        gpu.context.IASetPrimitiveTopology(topology);
        gpu.context.Draw(vertices.len() as u32, 0);
        gpu.context.GSSetShader(None, None);
    }
    Ok(())
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut outputs = Vec::new();
    let rgba = DXGI_FORMAT_R8G8B8A8_UNORM;
    let corner = |x: f32, y: f32, k: u32| {
        let [r, g, b] = hue(k, 6);
        [x, y, 0.5, r, g, b, 1.0]
    };

    let (target, rtv) = gpu.target(SIZE, SIZE, rgba)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    let points: Vec<Colored> = (0..24u32)
        .map(|k| {
            let [r, g, b] = hue(k, 24);
            [
                -0.8 + 0.32 * (k % 6) as f32,
                0.75 - 0.4 * (k / 6) as f32,
                0.5,
                r,
                g,
                b,
                1.0,
            ]
        })
        .collect();
    draw(
        gpu,
        dxbc!("geometry.gs_points"),
        dxbc!("geometry.ps"),
        D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
        &points,
    )?;
    outputs.push(Output::picture("points", gpu.read(&target, 0, 0)?));

    let (target, rtv) = gpu.target(SIZE, SIZE, rgba)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    // Two triangles, each corner followed by the far corner of its neighbour across the next edge.
    let adjacency = [
        corner(-0.9, 0.8, 0),
        corner(0.0, 0.95, 1),
        corner(-0.1, 0.1, 2),
        corner(-0.4, -0.2, 3),
        corner(-0.9, -0.5, 4),
        corner(-1.0, 0.2, 5),
        corner(0.1, 0.6, 1),
        corner(0.9, 0.95, 2),
        corner(0.9, -0.2, 3),
        corner(0.6, -0.9, 4),
        corner(0.0, -0.4, 5),
        corner(-0.2, 0.1, 0),
    ];
    draw(
        gpu,
        dxbc!("geometry.gs_adjacency"),
        dxbc!("geometry.ps"),
        D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ,
        &adjacency,
    )?;
    outputs.push(Output::picture("adjacency", gpu.read(&target, 0, 0)?));

    let (target, rtv) = gpu.target(SIZE, SIZE, rgba)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    let triangles = [
        corner(-0.6, 0.9, 0),
        corner(0.2, 0.2, 2),
        corner(-0.9, 0.1, 4),
        corner(0.3, 0.8, 1),
        corner(0.9, 0.1, 3),
        corner(0.1, -0.3, 5),
    ];
    draw(
        gpu,
        dxbc!("geometry.gs_strips"),
        dxbc!("geometry.ps_primitive"),
        D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        &triangles,
    )?;
    outputs.push(Output::picture("strips", gpu.read(&target, 0, 0)?));

    let (target, rtv) = gpu.target(SIZE, SIZE, rgba)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    gpu.program(
        dxbc!("geometry.vs"),
        dxbc!("geometry.ps_id"),
        &colored_layout(),
    )?;
    let mut fan = Vec::new();
    for k in 0..6u32 {
        let (a, b) = (k as f32 * 1.047, (k + 1) as f32 * 1.047);
        fan.extend([
            corner(0.0, 0.0, k),
            corner(0.9 * a.cos(), 0.9 * a.sin(), k),
            corner(0.9 * b.cos(), 0.9 * b.sin(), k),
        ]);
    }
    super::draw_colored(gpu, &fan)?;
    outputs.push(Output::picture("primitive_id", gpu.read(&target, 0, 0)?));

    let mut desc = tex2d(64, 64, rgba, D3D11_BIND_RENDER_TARGET);
    desc.ArraySize = 3;
    let layers = gpu.texture2d(&desc, &[])?;
    let rtv = gpu.rtv(&layers.cast()?, None)?;
    gpu.begin(&rtv, None, 64, 64);
    draw(
        gpu,
        dxbc!("geometry.gs_layers"),
        dxbc!("geometry.ps"),
        D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        &triangles[..3],
    )?;
    for layer in 0..3 {
        outputs.push(Output::picture(
            &format!("layer{layer}"),
            gpu.read(&layers, 0, layer)?,
        ));
    }
    Ok(outputs)
}
