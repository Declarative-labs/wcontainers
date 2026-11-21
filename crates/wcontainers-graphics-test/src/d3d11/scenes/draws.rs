//! Every kind of draw, one cell each: plain, instanced with and without a start instance, indexed
//! with positive and negative base vertices and 16 and 32-bit indices, both indirect draws, and a
//! vertex buffer bound at an offset. Instances read an offset per instance and a colour per two.
//! The indirect draws (feature level 11_0) go to a picture of their own, in the same cells.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

use super::hue;
use crate::d3d11::{Gpu, cell, element, solid};
use crate::dxbc;
use crate::output::Output;

pub const SIZE: u32 = 192;
const A: f32 = 0.3;
/// Where the list quad starts among the positions, and the indexed quad's base vertex.
pub const LIST_START: u32 = 3;
pub const INDEXED_BASE: i32 = 9;
/// Indices of the quad at `INDEXED_BASE`, met at several starts.
pub const INDICES: [u16; 14] = [7, 7, 0, 1, 2, 0, 2, 3, 5, 6, 7, 5, 7, 8];
/// Indirect arguments: a draw at byte 0, an indexed draw at byte 16.
pub const ARGS: [u32; 9] = [6, 4, LIST_START, 1, 6, 3, 2, INDEXED_BASE as u32, 0];

/// A two-component vector per vertex or instance.
type Pairs = Vec<[f32; 2]>;

/// Positions (three far ones first, so a start vertex of 0 draws off the cell), per-instance
/// offsets, and colours for every two instances.
pub fn vertices() -> (Pairs, Pairs, Vec<[f32; 4]>) {
    let far = [9.0f32, 9.0];
    let positions = [far, far, far]
        .into_iter()
        .chain([[-A, -A], [-A, A], [A, A], [-A, -A], [A, A], [A, -A]])
        .chain([[-A, -A], [-A, A], [A, A * 0.6], [A, -A]])
        .collect();
    let offsets = [[-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]]
        .into_iter()
        .chain([[-0.45, -0.6], [0.55, -0.45], [-0.6, 0.45], [0.45, 0.6]])
        .collect();
    let colors = (0..6)
        .map(|i| {
            let [r, g, b] = hue(i, 6);
            [r, g, b, 1.0]
        })
        .collect();
    (positions, offsets, colors)
}

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
    let (positions, offsets, colors) = vertices();
    let (list_start, indexed_base, indices) = (LIST_START, INDEXED_BASE, INDICES);
    let wide: Vec<u32> = indices.iter().map(|&i| i as u32).collect();
    let i16 = gpu.immutable(bytemuck::cast_slice(&indices), D3D11_BIND_INDEX_BUFFER)?;
    let i32 = gpu.immutable(bytemuck::cast_slice(&wide), D3D11_BIND_INDEX_BUFFER)?;
    let vertex = gpu.immutable(bytemuck::cast_slice(&positions), D3D11_BIND_VERTEX_BUFFER)?;
    let offset = gpu.immutable(bytemuck::cast_slice(&offsets), D3D11_BIND_VERTEX_BUFFER)?;
    let color = gpu.immutable(bytemuck::cast_slice(&colors), D3D11_BIND_VERTEX_BUFFER)?;
    let indirect = match gpu.lacks_level(D3D_FEATURE_LEVEL_11_0) {
        Some(why) => Err(why),
        None => {
            let args = gpu.buffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: 4 * ARGS.len() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    MiscFlags: D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS.0 as u32,
                    ..Default::default()
                },
                Some(bytemuck::cast_slice(&ARGS)),
            )?;
            Ok((args, gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?))
        }
    };
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
        if let Ok((args, (_, indirect_rtv))) = &indirect {
            gpu.begin(indirect_rtv, None, SIZE, SIZE);
            at(6);
            c.DrawInstancedIndirect(args, 0);
            c.IASetIndexBuffer(&i32, DXGI_FORMAT_R32_UINT, 4);
            at(7);
            // Starts at index 2 of the buffer bound 4 bytes in: index 3 of the whole buffer.
            c.DrawIndexedInstancedIndirect(args, 16);
            c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        }
        at(8);
        c.IASetVertexBuffers(0, 1, Some(&Some(vertex)), Some(&8), Some(&(8 * list_start)));
        c.DrawInstanced(6, 2, 0, 1);
    }
    Ok(vec![
        Output::picture("", gpu.read(&target, 0, 0)?),
        match indirect {
            Ok((_, (indirect_target, _))) => {
                Output::picture("indirect", gpu.read(&indirect_target, 0, 0)?)
            }
            Err(why) => Output::absent("indirect", why),
        },
    ])
}
