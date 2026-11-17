//! Every Direct3D 11 scene by name. Each is deterministic: fixed data, no clocks, seeded when random.

mod basic;
pub(crate) mod blend;
mod compute;
mod constants;
mod copies;
mod deferred;
pub(crate) mod depth;
mod draws;
mod dynamic;
pub(crate) mod formats;
mod geometry;
mod levels;
mod mips;
mod msaa;
mod queries;
mod raster;
pub(crate) mod samplers;
pub(crate) mod stencil;
mod streamout;
mod swapchain;
mod targets;
mod tessellation;
mod textures;
mod topology;
mod views;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::D3D11_INPUT_ELEMENT_DESC;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R32G32B32_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT,
};
use windows::core::s;

use super::{Gpu, element};
use crate::output::Output;

pub type Scene = fn(&Gpu) -> Result<Vec<Output>>;

pub const SCENES: &[(&str, Scene)] = &[
    ("triangle", basic::triangle),
    ("quads", basic::quads),
    ("instanced", basic::instanced),
    ("topology", topology::run),
    ("draws", draws::run),
    ("blend", blend::run),
    ("depth", depth::run),
    ("stencil", stencil::run),
    ("raster", raster::run),
    ("msaa", msaa::run),
    ("targets", targets::run),
    ("formats", formats::run),
    ("textures", textures::run),
    ("samplers", samplers::run),
    ("views", views::run),
    ("constants", constants::run),
    ("dynamic", dynamic::run),
    ("copies", copies::run),
    ("queries", queries::run),
    ("mips", mips::run),
    ("compute", compute::run),
    ("geometry", geometry::run),
    ("tessellation", tessellation::run),
    ("streamout", streamout::run),
    ("deferred", deferred::run),
    ("levels", levels::run),
    ("swapchain", swapchain::run),
];

pub fn find(name: &str) -> Option<Scene> {
    SCENES.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// A vertex of common.hlsl's `vs_color`: position x, y, z and colour r, g, b, a.
pub type Colored = [f32; 7];

pub fn colored_layout() -> [D3D11_INPUT_ELEMENT_DESC; 2] {
    [
        element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0, 0),
        element(s!("COLOR"), 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 12, 0),
    ]
}

/// A colour of a hue wheel, for telling vertices and cells apart.
pub fn hue(i: u32, n: u32) -> [f32; 3] {
    let h = i as f32 / n as f32 * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}

/// Two triangles covering x0..x1 and y0..y1, at depth z0 on the left to z1 on the right.
pub fn quad(x0: f32, y0: f32, x1: f32, y1: f32, z0: f32, z1: f32, color: [f32; 4]) -> [Colored; 6] {
    let [r, g, b, a] = color;
    let v = |x: f32, y: f32| {
        let z = if x == x0 { z0 } else { z1 };
        [x, y, z, r, g, b, a]
    };
    [
        v(x0, y0),
        v(x0, y1),
        v(x1, y1),
        v(x0, y0),
        v(x1, y1),
        v(x1, y0),
    ]
}

/// Binds common.hlsl's coloured-vertex program.
pub fn colored_program(gpu: &Gpu) -> Result<()> {
    gpu.program(
        crate::dxbc!("common.vs_color"),
        crate::dxbc!("common.ps_color"),
        &colored_layout(),
    )
}

/// Draws coloured vertices as a triangle list with whatever program and state are bound.
pub fn draw_colored(gpu: &Gpu, vertices: &[Colored]) -> Result<()> {
    gpu.vertices(0, bytemuck::cast_slice(vertices), 28)?;
    unsafe {
        gpu.context.IASetPrimitiveTopology(
            windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        );
        gpu.context.Draw(vertices.len() as u32, 0);
    }
    Ok(())
}

/// A depth test with `func`, writing depth when `write`, stencil off.
pub fn depth_test(
    func: windows::Win32::Graphics::Direct3D11::D3D11_COMPARISON_FUNC,
    write: bool,
) -> windows::Win32::Graphics::Direct3D11::D3D11_DEPTH_STENCIL_DESC {
    use windows::Win32::Graphics::Direct3D11::*;
    let keep = D3D11_DEPTH_STENCILOP_DESC {
        StencilFailOp: D3D11_STENCIL_OP_KEEP,
        StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
        StencilPassOp: D3D11_STENCIL_OP_KEEP,
        StencilFunc: D3D11_COMPARISON_ALWAYS,
    };
    D3D11_DEPTH_STENCIL_DESC {
        DepthEnable: true.into(),
        DepthWriteMask: if write {
            D3D11_DEPTH_WRITE_MASK_ALL
        } else {
            D3D11_DEPTH_WRITE_MASK_ZERO
        },
        DepthFunc: func,
        StencilEnable: false.into(),
        StencilReadMask: 0xff,
        StencilWriteMask: 0xff,
        FrontFace: keep,
        BackFace: keep,
    }
}
