//! Every kind of draw, one cell each: plain, instanced with and without a start instance, indexed
//! with positive and negative base vertices and 16 and 32-bit indices, both indirect draws, and a
//! vertex buffer bound at an offset. Instances read an offset per instance and a colour per two.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

use super::hue;
use crate::d3d11::{Gpu, cell, element, solid};
use crate::dxbc;
use crate::output::Output;

const SIZE: u32 = 192;
const A: f32 = 0.3;

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    gpu.program(
        dxbc!("draws.vs"),
        dxbc!("draws.ps"),
        &[
            element(s!("POSITION"), 0, DXGI_FORMAT_R32G32_FLOAT, 0, 0, 0),
            element(s!("OFFSET"), 0, DXGI_FORMAT_R32G32_FLOAT, 1, 0, 1),
            element(s!("COLOR"), 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 2, 0, 2),
        ],
    )?;
    // Three far vertices first, so a start vertex of 0 draws off the cell; the list quad at 3, the
    // indexed quad at 9.
    let far = [9.0f32, 9.0];
    let positions: Vec<[f32; 2]> = [far, far, far]
        .into_iter()
        .chain([[-A, -A], [-A, A], [A, A], [-A, -A], [A, A], [A, -A]])
        .chain([[-A, -A], [-A, A], [A, A * 0.6], [A, -A]])
        .collect();
    let (list_start, indexed_base) = (3u32, 9i32);
    let offsets: Vec<[f32; 2]> = [[-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]]
        .into_iter()
        .chain([[-0.45, -0.6], [0.55, -0.45], [-0.6, 0.45], [0.45, 0.6]])
        .collect();
    let colors: Vec<[f32; 4]> = (0..6)
        .map(|i| {
            let [r, g, b] = hue(i, 6);
            [r, g, b, 1.0]
        })
        .collect();
    let indices: [u16; 14] = [7, 7, 0, 1, 2, 0, 2, 3, 5, 6, 7, 5, 7, 8];
    let wide: Vec<u32> = indices.iter().map(|&i| i as u32).collect();
    let i16 = gpu.immutable(bytemuck::cast_slice(&indices), D3D11_BIND_INDEX_BUFFER)?;
    let i32 = gpu.immutable(bytemuck::cast_slice(&wide), D3D11_BIND_INDEX_BUFFER)?;
    let vertex = gpu.immutable(bytemuck::cast_slice(&positions), D3D11_BIND_VERTEX_BUFFER)?;
    let offset = gpu.immutable(bytemuck::cast_slice(&offsets), D3D11_BIND_VERTEX_BUFFER)?;
    let color = gpu.immutable(bytemuck::cast_slice(&colors), D3D11_BIND_VERTEX_BUFFER)?;
    let args: [u32; 9] = [6, 4, list_start, 1, 6, 3, 2, indexed_base as u32, 0];
    let args = gpu.buffer(
        &D3D11_BUFFER_DESC {
            ByteWidth: 4 * args.len() as u32,
            Usage: D3D11_USAGE_DEFAULT,
            MiscFlags: D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS.0 as u32,
            ..Default::default()
        },
        Some(bytemuck::cast_slice(&args)),
    )?;
    unsafe {
        let c = &gpu.context;
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        let buffers = [Some(vertex.clone()), Some(offset), Some(color)];
        c.IASetVertexBuffers(
            0,
            3,
            Some(buffers.as_ptr()),
            Some([8, 8, 16].as_ptr()),
            Some([0, 0, 0].as_ptr()),
        );
        let at = |i: u32| c.RSSetViewports(Some(&[cell(i, 3, 64.0)]));
        at(0);
        c.Draw(6, list_start);
        at(1);
        c.DrawInstanced(6, 4, list_start, 0);
        at(2);
        c.DrawInstanced(6, 4, list_start, 3);
        c.IASetIndexBuffer(&i16, DXGI_FORMAT_R16_UINT, 0);
        at(3);
        c.DrawIndexed(6, 2, indexed_base);
        at(4);
        c.DrawIndexedInstanced(6, 4, 8, indexed_base - 5, 1);
        c.IASetIndexBuffer(&i32, DXGI_FORMAT_R32_UINT, 0);
        at(5);
        c.DrawIndexedInstanced(6, 4, 2, indexed_base, 2);
        at(6);
        c.DrawInstancedIndirect(&args, 0);
        c.IASetIndexBuffer(&i32, DXGI_FORMAT_R32_UINT, 4);
        at(7);
        // Starts at index 2 of the buffer bound 4 bytes in: index 3 of the whole buffer.
        c.DrawIndexedInstancedIndirect(&args, 16);
        at(8);
        c.IASetVertexBuffers(0, 1, Some(&Some(vertex)), Some(&8), Some(&(8 * list_start)));
        c.DrawInstanced(6, 2, 0, 1);
    }
    Ok(vec![Output::picture("", gpu.read(&target, 0, 0)?)])
}
