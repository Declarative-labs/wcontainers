//! Queries: occlusion counts of rectangles partly behind an occluder, occlusion predicates and
//! predicated drawing, pipeline statistics of a draw, and timestamps (whose values vary, so only
//! facts about them are recorded).

use std::fmt::Write;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use super::{colored_program, depth_test, draw_colored, quad};
use crate::d3d11::{Gpu, made, solid};
use crate::output::Output;

const SIZE: u32 = 64;

/// A rectangle covering pixels x0..x1 and y0..y1 exactly, at depth `z`.
fn pixels(x0: u32, y0: u32, x1: u32, y1: u32, z: f32, color: [f32; 4]) -> [super::Colored; 6] {
    let x = |p: u32| p as f32 / (SIZE / 2) as f32 - 1.0;
    let y = |p: u32| 1.0 - p as f32 / (SIZE / 2) as f32;
    quad(x(x0), y(y1), x(x1), y(y0), z, z, color)
}

/// Waits for a query's result.
pub fn result<T: Default>(gpu: &Gpu, query: &ID3D11Asynchronous) -> Result<T> {
    let mut data = T::default();
    loop {
        // The wrapper treats S_FALSE (not ready) as success, so the call goes through the vtable.
        let hr = unsafe {
            (Interface::vtable(&gpu.context).GetData)(
                Interface::as_raw(&gpu.context),
                Interface::as_raw(query),
                (&mut data as *mut T).cast(),
                std::mem::size_of::<T>() as u32,
                0,
            )
        };
        hr.ok()?;
        if hr.0 == 0 {
            return Ok(data);
        }
        std::thread::yield_now();
    }
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

fn predicate(gpu: &Gpu) -> Result<ID3D11Predicate> {
    made(|o| unsafe {
        gpu.device.CreatePredicate(
            &D3D11_QUERY_DESC {
                Query: D3D11_QUERY_OCCLUSION_PREDICATE,
                MiscFlags: 0,
            },
            Some(o),
        )
    })
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let (target, rtv) = gpu.target(SIZE, SIZE, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let (_, dsv) = gpu.depth(SIZE, SIZE, DXGI_FORMAT_D32_FLOAT)?;
    gpu.begin(&rtv, Some(&dsv), SIZE, SIZE);
    colored_program(gpu)?;
    unsafe {
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.OMSetDepthStencilState(
            &gpu.depth_stencil(&depth_test(D3D11_COMPARISON_LESS, true))?,
            0,
        );
    }
    draw_colored(gpu, &pixels(0, 0, 32, 64, 0.2, [0.4, 0.4, 0.4, 1.0]))?;

    let mut text = String::new();
    let rects = [
        (4, 4, 12, 9),
        (28, 10, 40, 20),
        (0, 30, 64, 31),
        (10, 40, 30, 60),
        (40, 40, 63, 63),
        (33, 0, 34, 64),
    ];
    for (k, &(x0, y0, x1, y1)) in rects.iter().enumerate() {
        let occlusion = query(gpu, D3D11_QUERY_OCCLUSION)?;
        unsafe { c.Begin(&occlusion) };
        draw_colored(gpu, &pixels(x0, y0, x1, y1, 0.5, [0.9, 0.5, 0.1, 1.0]))?;
        unsafe { c.End(&occlusion) };
        let samples: u64 = result(gpu, &occlusion.cast()?)?;
        writeln!(
            text,
            "rectangle {k} ({x0},{y0})-({x1},{y1}) behind the left half: {samples} samples passed"
        )?;
    }

    // A hidden rectangle (behind the left half) and a visible one each make a predicate; a marker drawn under each predicate
    // appears only where its rectangle was visible.
    let mut markers = Vec::new();
    for (k, (rect, marker)) in [
        ((4, 50, 20, 60), (4, 20, 12, 28)),
        ((44, 50, 60, 60), (48, 20, 56, 28)),
    ]
    .into_iter()
    .enumerate()
    {
        let p = predicate(gpu)?;
        unsafe { c.Begin(&p) };
        let z = if k == 0 { 0.6 } else { 0.3 };
        draw_colored(
            gpu,
            &pixels(rect.0, rect.1, rect.2, rect.3, z, [0.2, 0.3, 0.9, 1.0]),
        )?;
        unsafe { c.End(&p) };
        let visible: windows::core::BOOL = result(gpu, &p.cast()?)?;
        writeln!(
            text,
            "predicate {k}: {}",
            if visible.as_bool() {
                "visible"
            } else {
                "hidden"
            }
        )?;
        markers.push((p, marker));
    }
    for (p, (x0, y0, x1, y1)) in &markers {
        unsafe { c.SetPredication(p, false) };
        draw_colored(gpu, &pixels(*x0, *y0, *x1, *y1, 0.1, [0.1, 0.9, 0.2, 1.0]))?;
    }
    unsafe { c.SetPredication(None, false) };

    let statistics = query(gpu, D3D11_QUERY_PIPELINE_STATISTICS)?;
    unsafe { c.Begin(&statistics) };
    draw_colored(gpu, &pixels(36, 2, 62, 18, 0.05, [0.9, 0.9, 0.2, 1.0]))?;
    unsafe { c.End(&statistics) };
    let s: D3D11_QUERY_DATA_PIPELINE_STATISTICS = result(gpu, &statistics.cast()?)?;
    // Pixel shader invocations may include helper pixels, so only their lower bound is a fact.
    writeln!(
        text,
        "statistics: input vertices {} primitives {}, vertex shader {}, geometry shader {} primitives {}, clipper {} primitives {}, pixel shader at least the 416 covered pixels {}, hull {} domain {} compute {}",
        s.IAVertices,
        s.IAPrimitives,
        s.VSInvocations,
        s.GSInvocations,
        s.GSPrimitives,
        s.CInvocations,
        s.CPrimitives,
        s.PSInvocations >= 26 * 16,
        s.HSInvocations,
        s.DSInvocations,
        s.CSInvocations
    )?;

    let disjoint = query(gpu, D3D11_QUERY_TIMESTAMP_DISJOINT)?;
    let (start, end) = (
        query(gpu, D3D11_QUERY_TIMESTAMP)?,
        query(gpu, D3D11_QUERY_TIMESTAMP)?,
    );
    unsafe {
        c.Begin(&disjoint);
        c.End(&start);
    }
    draw_colored(gpu, &pixels(0, 0, 64, 64, 0.9, [0.0, 0.0, 0.0, 1.0]))?;
    unsafe {
        c.End(&end);
        c.End(&disjoint);
    }
    let d: D3D11_QUERY_DATA_TIMESTAMP_DISJOINT = result(gpu, &disjoint.cast()?)?;
    let (t0, t1): (u64, u64) = (result(gpu, &start.cast()?)?, result(gpu, &end.cast()?)?);
    // A disjoint interval (the clock changed) makes the timestamps meaningless, which Direct3D
    // allows at any time; then only the frequency is a fact.
    writeln!(
        text,
        "timestamps: frequency above zero {}, in order or disjoint {}",
        d.Frequency > 0,
        d.Disjoint.as_bool() || t1 >= t0
    )?;
    Ok(vec![
        Output::text("results", text),
        Output::picture("", gpu.read(&target, 0, 0)?),
    ])
}
