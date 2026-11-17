//! Deferred contexts: two command lists recorded on deferred contexts (one writing a dynamic buffer
//! with a discard map), executed on the immediate context with its state kept and then cleared,
//! with the immediate context drawing between them on the state each leaves.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{colored_layout, quad};
use crate::d3d11::{Gpu, made, solid};
use crate::dxbc;
use crate::output::Output;

const SIZE: u32 = 128;

/// Sets everything a draw of coloured vertices needs on `context`.
fn prepare(
    gpu: &Gpu,
    context: &ID3D11DeviceContext,
    rtv: &ID3D11RenderTargetView,
    viewport: D3D11_VIEWPORT,
) -> Result<()> {
    let layout = gpu.layout(&colored_layout(), dxbc!("common.vs_color"))?;
    unsafe {
        context.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        context.RSSetViewports(Some(&[viewport]));
        context.RSSetState(&gpu.rasterizer(&solid())?);
        context.VSSetShader(&gpu.vs(dxbc!("common.vs_color"))?, None);
        context.PSSetShader(&gpu.ps(dxbc!("common.ps_color"))?, None);
        context.IASetInputLayout(&layout);
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    }
    Ok(())
}

fn draw(gpu: &Gpu, context: &ID3D11DeviceContext, vertices: &[[f32; 7]]) -> Result<()> {
    let buffer = gpu.immutable(bytemuck::cast_slice(vertices), D3D11_BIND_VERTEX_BUFFER)?;
    unsafe {
        context.IASetVertexBuffers(0, 1, Some(&Some(buffer)), Some(&28), Some(&0));
        context.Draw(vertices.len() as u32, 0);
    }
    Ok(())
}

fn half(left: bool) -> D3D11_VIEWPORT {
    D3D11_VIEWPORT {
        TopLeftX: if left { 0.0 } else { SIZE as f32 / 2.0 },
        TopLeftY: 0.0,
        Width: SIZE as f32 / 2.0,
        Height: SIZE as f32,
        MinDepth: 0.0,
        MaxDepth: 1.0,
    }
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    let deferred = || made(|o| unsafe { gpu.device.CreateDeferredContext(0, Some(o)) });

    // The first list draws a quad and one from a dynamic buffer written on the deferred context.
    let first: ID3D11DeviceContext = deferred()?;
    prepare(gpu, &first, &rtv, half(true))?;
    draw(
        gpu,
        &first,
        &quad(-0.8, 0.4, 0.8, 0.9, 0.5, 0.5, [0.9, 0.3, 0.2, 1.0]),
    )?;
    let dynamic = gpu.buffer(
        &D3D11_BUFFER_DESC {
            ByteWidth: 6 * 28,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            ..Default::default()
        },
        None,
    )?;
    let shape = quad(-0.8, -0.2, 0.8, 0.3, 0.5, 0.5, [0.2, 0.8, 0.3, 1.0]);
    unsafe {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        first.Map(&dynamic, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(shape.as_ptr().cast::<u8>(), mapped.pData.cast(), 6 * 28);
        first.Unmap(&dynamic, 0);
        first.IASetVertexBuffers(0, 1, Some(&Some(dynamic)), Some(&28), Some(&0));
        first.Draw(6, 0);
    }
    let first_list = made(|o| unsafe { first.FinishCommandList(false, Some(o)) })?;

    // The second list draws on the right half.
    let second: ID3D11DeviceContext = deferred()?;
    prepare(gpu, &second, &rtv, half(false))?;
    draw(
        gpu,
        &second,
        &quad(-0.8, 0.4, 0.8, 0.9, 0.5, 0.5, [0.3, 0.4, 0.95, 1.0]),
    )?;
    let second_list = made(|o| unsafe { second.FinishCommandList(true, Some(o)) })?;

    // The immediate context, set for the right half, keeps that state across the first list and
    // draws there; after the second list, executed without keeping it, it sets its state again.
    let c = &gpu.context;
    prepare(gpu, c, &rtv, half(false))?;
    unsafe { c.ExecuteCommandList(&first_list, true) };
    draw(
        gpu,
        c,
        &quad(-0.8, -0.2, 0.8, 0.3, 0.5, 0.5, [0.95, 0.85, 0.2, 1.0]),
    )?;
    unsafe { c.ExecuteCommandList(&second_list, false) };
    prepare(gpu, c, &rtv, half(true))?;
    draw(
        gpu,
        c,
        &quad(-0.8, -0.9, 0.8, -0.4, 0.5, 0.5, [0.8, 0.3, 0.9, 1.0]),
    )?;
    // The second deferred context kept its state: it draws again on its own half.
    draw(
        gpu,
        &second,
        &quad(-0.8, -0.9, 0.8, -0.4, 0.5, 0.5, [0.2, 0.9, 0.9, 1.0]),
    )?;
    let third_list = made(|o| unsafe { second.FinishCommandList(false, Some(o)) })?;
    unsafe { c.ExecuteCommandList(&third_list, false) };
    Ok(vec![Output::picture("", gpu.read(&target, 0, 0)?)])
}
