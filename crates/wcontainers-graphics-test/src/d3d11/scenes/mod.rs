//! Every Direct3D 11 scene by name. Each is deterministic: fixed data, no clocks, seeded when random.

mod basic;
pub(crate) mod blend;
mod compute;
pub(crate) mod constants;
mod copies;
mod deferred;
pub(crate) mod depth;
mod device;
pub(crate) mod draws;
mod dynamic;
pub(crate) mod formats;
pub(crate) mod geometry;
mod levels;
mod mips;
pub(crate) mod msaa;
mod queries;
pub(crate) mod raster;
pub(crate) mod samplers;
pub(crate) mod stencil;
pub(crate) mod streamout;
mod swapchain;
pub(crate) mod targets;
pub(crate) mod tessellation;
pub(crate) mod textures;
pub(crate) mod topology;
pub(crate) mod views;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::{
    D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_9_1, D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1,
    D3D_FEATURE_LEVEL_11_0,
};
use windows::Win32::Graphics::Direct3D11::D3D11_INPUT_ELEMENT_DESC;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R32G32B32_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT,
};
use windows::core::s;

use super::{Gpu, element};
use crate::output::Output;

pub type Scene = fn(&Gpu) -> Result<Vec<Output>>;

/// Every scene with the lowest feature level it draws at, below which it is absent: 10_0 where its
/// shaders are shader model 4.0 (parts needing more say so themselves); 10_1 for standard sample
/// patterns (msaa) and blending set per target (targets); 11_0 for indirect draws, compute shaders,
/// tessellation and the device scene's answers about 11_0 devices; 9_1 for the levels scene, which
/// makes its own devices.
pub const SCENES: &[(&str, D3D_FEATURE_LEVEL, Scene)] = &[
    ("triangle", D3D_FEATURE_LEVEL_10_0, basic::triangle),
    ("quads", D3D_FEATURE_LEVEL_10_0, basic::quads),
    ("instanced", D3D_FEATURE_LEVEL_10_0, basic::instanced),
    ("topology", D3D_FEATURE_LEVEL_10_0, topology::run),
    ("draws", D3D_FEATURE_LEVEL_11_0, draws::run),
    ("blend", D3D_FEATURE_LEVEL_10_0, blend::run),
    ("depth", D3D_FEATURE_LEVEL_10_0, depth::run),
    ("stencil", D3D_FEATURE_LEVEL_10_0, stencil::run),
    ("raster", D3D_FEATURE_LEVEL_10_0, raster::run),
    ("msaa", D3D_FEATURE_LEVEL_10_1, msaa::run),
    ("targets", D3D_FEATURE_LEVEL_10_1, targets::run),
    ("formats", D3D_FEATURE_LEVEL_10_0, formats::run),
    ("textures", D3D_FEATURE_LEVEL_10_0, textures::run),
    ("samplers", D3D_FEATURE_LEVEL_10_0, samplers::run),
    ("views", D3D_FEATURE_LEVEL_10_0, views::run),
    ("constants", D3D_FEATURE_LEVEL_10_0, constants::run),
    ("dynamic", D3D_FEATURE_LEVEL_10_0, dynamic::run),
    ("copies", D3D_FEATURE_LEVEL_10_0, copies::run),
    ("queries", D3D_FEATURE_LEVEL_10_0, queries::run),
    ("mips", D3D_FEATURE_LEVEL_10_0, mips::run),
    ("compute", D3D_FEATURE_LEVEL_11_0, compute::run),
    ("geometry", D3D_FEATURE_LEVEL_10_0, geometry::run),
    ("tessellation", D3D_FEATURE_LEVEL_11_0, tessellation::run),
    ("streamout", D3D_FEATURE_LEVEL_10_0, streamout::run),
    ("deferred", D3D_FEATURE_LEVEL_10_0, deferred::run),
    ("levels", D3D_FEATURE_LEVEL_9_1, levels::run),
    ("device", D3D_FEATURE_LEVEL_11_0, device::run),
    ("swapchain", D3D_FEATURE_LEVEL_10_0, swapchain::run),
];

pub fn find(name: &str) -> Option<(D3D_FEATURE_LEVEL, Scene)> {
    SCENES
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|(_, level, scene)| (*level, *scene))
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
