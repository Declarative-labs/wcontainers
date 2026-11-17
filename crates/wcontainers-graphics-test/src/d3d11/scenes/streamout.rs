//! Stream output: a geometry shader writing each triangle and a moved copy both to the target and to
//! a buffer, the buffer drawn back mirrored with DrawAuto, its values, the stream output statistics,
//! and a buffer too small for the output overflowing.

use std::fmt::Write;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{Interface, s};

use super::{colored_layout, hue, queries::result};
use crate::d3d11::{Gpu, element, made, solid};
use crate::dxbc;
use crate::output::Output;

const SIZE: u32 = 128;

fn so_buffer(gpu: &Gpu, bytes: u32) -> Result<ID3D11Buffer> {
    gpu.buffer(
        &D3D11_BUFFER_DESC {
            ByteWidth: bytes,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_STREAM_OUTPUT.0 | D3D11_BIND_VERTEX_BUFFER.0) as u32,
            ..Default::default()
        },
        None,
    )
}

fn query(gpu: &Gpu, kind: D3D11_QUERY) -> Result<ID3D11Query> {
    made(|o| unsafe {
        gpu.device.CreateQuery(
            &D3D11_QUERY_DESC {
                Query: kind,
                MiscFlags: 0,
            },
            Some(o),
        )
    })
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let mut text = String::new();
    let mut written = None;
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, SIZE, SIZE);
    gpu.program(
        dxbc!("common.vs_color"),
        dxbc!("common.ps_color"),
        &colored_layout(),
    )?;
    let declaration = [
        D3D11_SO_DECLARATION_ENTRY {
            Stream: 0,
            SemanticName: s!("SV_Position"),
            SemanticIndex: 0,
            StartComponent: 0,
            ComponentCount: 4,
            OutputSlot: 0,
        },
        D3D11_SO_DECLARATION_ENTRY {
            Stream: 0,
            SemanticName: s!("COLOR"),
            SemanticIndex: 0,
            StartComponent: 0,
            ComponentCount: 4,
            OutputSlot: 0,
        },
    ];
    let gs: ID3D11GeometryShader = made(|o| unsafe {
        gpu.device.CreateGeometryShaderWithStreamOutput(
            dxbc!("streamout.gs"),
            Some(&declaration),
            Some(&[32]),
            0,
            None::<&ID3D11ClassLinkage>,
            Some(o),
        )
    })?;
    let triangles: Vec<[f32; 7]> = (0..6u32)
        .map(|k| {
            let [r, g, b] = hue(k, 6);
            let (x, y) = if k < 3 { (-0.5, 0.5) } else { (0.3, 0.2) };
            let a = k as f32 * 2.0944;
            [x + 0.35 * a.sin(), y + 0.35 * a.cos(), 0.5, r, g, b, 1.0]
        })
        .collect();
    gpu.vertices(0, bytemuck::cast_slice(&triangles), 28)?;
    unsafe {
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.GSSetShader(&gs, None);
    }

    // Room for all 12 vertices, then for only 5.
    for (name, bytes) in [("enough", 12 * 32), ("small", 5 * 32)] {
        let buffer = so_buffer(gpu, bytes)?;
        let statistics = query(gpu, D3D11_QUERY_SO_STATISTICS)?;
        let overflow = query(gpu, D3D11_QUERY_SO_OVERFLOW_PREDICATE)?;
        unsafe {
            c.SOSetTargets(1, Some(&Some(buffer.clone())), Some(&0));
            c.Begin(&statistics);
            c.Begin(&overflow);
            c.Draw(6, 0);
            c.End(&overflow);
            c.End(&statistics);
            c.SOSetTargets(0, None, None);
        }
        let s: D3D11_QUERY_DATA_SO_STATISTICS = result(gpu, &statistics.cast()?)?;
        let over: windows::core::BOOL = result(gpu, &overflow.cast()?)?;
        writeln!(
            text,
            "{name}: {} primitives written, {} needed, overflow {}",
            s.NumPrimitivesWritten,
            s.PrimitivesStorageNeeded,
            over.as_bool()
        )?;
        if name == "enough" {
            // The written vertices as a float picture, a row per vertex: position, then colour.
            let floats: Vec<f32> = bytemuck::pod_collect_to_vec(&gpu.read_buffer(&buffer)?);
            written = Some(
                image::Rgba32FImage::from_raw(2, 12, floats).expect("12 vertices of 8 floats"),
            );
            // Drawn back mirrored, as many vertices as were written.
            gpu.program(
                dxbc!("streamout.vs_back"),
                dxbc!("common.ps_color"),
                &[
                    element(s!("POSITION"), 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 0, 0),
                    element(s!("COLOR"), 0, DXGI_FORMAT_R32G32B32A32_FLOAT, 0, 16, 0),
                ],
            )?;
            unsafe {
                c.GSSetShader(None, None);
                c.IASetVertexBuffers(0, 1, Some(&Some(buffer)), Some(&32), Some(&0));
                c.DrawAuto();
                c.GSSetShader(&gs, None);
            }
            gpu.program(
                dxbc!("common.vs_color"),
                dxbc!("common.ps_color"),
                &colored_layout(),
            )?;
            gpu.vertices(0, bytemuck::cast_slice(&triangles), 28)?;
        }
    }
    unsafe { c.GSSetShader(None, None) };
    let written = written.expect("the first buffer had room");
    Ok(vec![
        Output::text("results", text),
        Output::picture("buffer", written),
        Output::picture("", gpu.read(&target, 0, 0)?),
    ])
}
