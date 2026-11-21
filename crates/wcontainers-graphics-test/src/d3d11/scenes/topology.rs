//! Every primitive topology but patches, one cell each: lists, strips, the adjacency kinds (drawn
//! without a geometry shader, so the adjacent vertices are skipped), and indexed strips cut by the
//! restart index of 16 and 32-bit indices.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{Colored, colored_layout, hue};
use crate::d3d11::{Gpu, cell, solid};
use crate::dxbc;
use crate::output::Output;

const SIZE: u32 = 256;

/// `n` vertices zigzagging across a cell, each its own colour.
pub fn zigzag(n: u32) -> Vec<Colored> {
    (0..n)
        .map(|k| {
            let x = -0.85 + 1.7 * k as f32 / (n - 1) as f32;
            let y = if k % 2 == 0 { -0.6 } else { 0.6 } + 0.03 * k as f32;
            let [r, g, b] = hue(k, n);
            [x, y, 0.5, r, g, b, 1.0]
        })
        .collect()
}

/// `n` vertices for a triangle adjacency topology: the even ones, which make the triangles, zigzag;
/// each odd one, an adjacent vertex the rasterizer skips, sits off to the side.
fn with_adjacency(n: u32) -> Vec<Colored> {
    zigzag(n / 2)
        .into_iter()
        .flat_map(|v| [v, [v[0] + 0.1, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]])
        .collect()
}

/// Each plain topology with its vertices, one cell each.
pub fn plain() -> [(D3D_PRIMITIVE_TOPOLOGY, Vec<Colored>); 9] {
    [
        (D3D_PRIMITIVE_TOPOLOGY_POINTLIST, zigzag(8)),
        (D3D_PRIMITIVE_TOPOLOGY_LINELIST, zigzag(8)),
        (D3D_PRIMITIVE_TOPOLOGY_LINESTRIP, zigzag(8)),
        (D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, zigzag(6)),
        (D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP, zigzag(8)),
        (D3D_PRIMITIVE_TOPOLOGY_LINELIST_ADJ, zigzag(8)),
        (D3D_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ, zigzag(8)),
        (D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ, with_adjacency(12)),
        (D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ, with_adjacency(12)),
    ]
}

/// Indices of two strips of `zigzag(8)` cut by the 16-bit restart index.
pub const CUT16: [u16; 9] = [0, 1, 2, 3, u16::MAX, 4, 5, 6, 7];

/// `CUT16` with the 32-bit restart index.
pub fn cut32() -> Vec<u32> {
    CUT16
        .iter()
        .map(|&i| if i == u16::MAX { u32::MAX } else { i as u32 })
        .collect()
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    gpu.program(
        dxbc!("common.vs_color"),
        dxbc!("common.ps_color"),
        &colored_layout(),
    )?;
    let plain = plain();
    let mut vertices = Vec::new();
    let mut draws = Vec::new();
    for (topology, v) in &plain {
        draws.push((*topology, v.len() as u32, vertices.len() as u32));
        vertices.extend(v);
    }
    let restart_base = vertices.len() as u32;
    vertices.extend(zigzag(8));
    gpu.vertices(0, bytemuck::cast_slice(&vertices), 28)?;

    let indices16 = gpu.immutable(bytemuck::cast_slice(&CUT16), D3D11_BIND_INDEX_BUFFER)?;
    let indices32 = gpu.immutable(bytemuck::cast_slice(&cut32()), D3D11_BIND_INDEX_BUFFER)?;
    unsafe {
        let c = &gpu.context;
        c.RSSetState(&gpu.rasterizer(&solid())?);
        for (i, &(topology, n, start)) in draws.iter().enumerate() {
            c.RSSetViewports(Some(&[cell(i as u32, 4, 64.0)]));
            c.IASetPrimitiveTopology(topology);
            c.Draw(n, start);
        }
        let indexed = [
            (
                D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
                &indices16,
                DXGI_FORMAT_R16_UINT,
            ),
            (
                D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
                &indices32,
                DXGI_FORMAT_R32_UINT,
            ),
            (
                D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
                &indices16,
                DXGI_FORMAT_R16_UINT,
            ),
            (
                D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
                &indices32,
                DXGI_FORMAT_R32_UINT,
            ),
        ];
        for (i, (topology, indices, format)) in indexed.into_iter().enumerate() {
            c.RSSetViewports(Some(&[cell(plain.len() as u32 + i as u32, 4, 64.0)]));
            c.IASetPrimitiveTopology(topology);
            c.IASetIndexBuffer(indices, format, 0);
            c.DrawIndexed(CUT16.len() as u32, 0, restart_base as i32);
        }
    }
    Ok(vec![Output::picture("", gpu.read(&target, 0, 0)?)])
}
