//! Tessellation, drawn as wireframe so every generated vertex shows: a triangle patch, a quad patch
//! with each partitioning (fractional odd and even, powers of two, integer) and an isoline patch,
//! with fractional factors from a constant buffer; then the quad again with solid filling.

use anyhow::Result;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{Colored, colored_layout, hue};
use crate::d3d11::{Gpu, cell, solid};
use crate::dxbc;
use crate::output::Output;

/// Hull and domain shader, patch topology and control points.
type Patch<'a> = (&'a [u8], &'a [u8], D3D_PRIMITIVE_TOPOLOGY, &'a [Colored]);

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let (target, rtv) = gpu.target(256, 128, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    gpu.begin(&rtv, None, 256, 128);
    gpu.program(
        dxbc!("tessellation.vs"),
        dxbc!("tessellation.ps"),
        &colored_layout(),
    )?;
    let point = |x: f32, y: f32, k: u32| {
        let [r, g, b] = hue(k, 4);
        [x, y, 0.5, r, g, b, 1.0]
    };
    let tri = [
        point(0.0, 0.85, 0),
        point(0.85, -0.75, 1),
        point(-0.85, -0.75, 2),
    ];
    let quad = [
        point(-0.85, 0.85, 0),
        point(0.85, 0.85, 1),
        point(0.85, -0.85, 2),
        point(-0.85, -0.85, 3),
    ];
    let line = [point(-0.9, 0.0, 0), point(0.9, 0.0, 2)];
    let factors: [f32; 8] = [3.3, 4.7, 2.2, 5.9, 3.6, 2.4, 0.0, 0.0];
    let constants = gpu.immutable(bytemuck::cast_slice(&factors), D3D11_BIND_CONSTANT_BUFFER)?;
    let wire = gpu.rasterizer(&D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_WIREFRAME,
        ..solid()
    })?;
    let patches: [Patch; 7] = [
        (
            dxbc!("tessellation.hs_tri"),
            dxbc!("tessellation.ds_tri"),
            D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST,
            &tri,
        ),
        (
            dxbc!("tessellation.hs_quad_odd"),
            dxbc!("tessellation.ds_quad"),
            D3D_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST,
            &quad,
        ),
        (
            dxbc!("tessellation.hs_quad_even"),
            dxbc!("tessellation.ds_quad"),
            D3D_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST,
            &quad,
        ),
        (
            dxbc!("tessellation.hs_quad_pow2"),
            dxbc!("tessellation.ds_quad"),
            D3D_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST,
            &quad,
        ),
        (
            dxbc!("tessellation.hs_quad_integer"),
            dxbc!("tessellation.ds_quad"),
            D3D_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST,
            &quad,
        ),
        (
            dxbc!("tessellation.hs_line"),
            dxbc!("tessellation.ds_line"),
            D3D_PRIMITIVE_TOPOLOGY_2_CONTROL_POINT_PATCHLIST,
            &line,
        ),
        (
            dxbc!("tessellation.hs_tri"),
            dxbc!("tessellation.ds_tri"),
            D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST,
            &tri,
        ),
    ];
    unsafe {
        c.HSSetConstantBuffers(0, Some(&[Some(constants.clone())]));
        c.DSSetConstantBuffers(0, Some(&[Some(constants)]));
    }
    // The last patch is drawn solid.
    let last = patches.len() - 1;
    for (i, (hs, ds, topology, vertices)) in patches.into_iter().enumerate() {
        gpu.vertices(0, bytemuck::cast_slice(vertices), 28)?;
        let fill = if i == last {
            gpu.rasterizer(&solid())?
        } else {
            wire.clone()
        };
        unsafe {
            c.RSSetState(&fill);
            c.RSSetViewports(Some(&[cell(i as u32, 4, 64.0)]));
            c.HSSetShader(&gpu.hs(hs)?, None);
            c.DSSetShader(&gpu.ds(ds)?, None);
            c.IASetPrimitiveTopology(topology);
            c.Draw(vertices.len() as u32, 0);
        }
    }
    unsafe {
        c.HSSetShader(None, None);
        c.DSSetShader(None, None);
    }
    Ok(vec![Output::picture("", gpu.read(&target, 0, 0)?)])
}
