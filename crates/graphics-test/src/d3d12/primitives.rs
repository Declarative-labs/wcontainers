//! Direct3D 11's topology, geometry shader, tessellation and stream output scenes drawn through
//! Direct3D 12: the same vertices, with the kind of primitive, the strip cut value, the hull,
//! domain and geometry shaders and the stream output layout fixed in each pipeline state object.

use std::fmt::Write;

use anyhow::{Context, Result};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use windows::core::s;

use super::states::{Immediate, RT, blend12, constants_root, raster12};
use super::{Gpu, area, buffer_desc, bytecode, element, held, release, tex2d_desc, transition};
use crate::d3d11::scenes::blend::no_blend;
use crate::d3d11::scenes::streamout as streamout11;
use crate::d3d11::scenes::{
    geometry as geometry11, tessellation as tessellation11, topology as topology11,
};
use crate::d3d11::{cell, solid};
use crate::dxil;
use crate::output::Output;

const RGBA: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

/// Direct3D 11's topology scene: indexed strips restart only where the pipeline's cut value says.
pub fn topology(gpu: &Gpu) -> Result<Vec<Output>> {
    const SIZE: u32 = 256;
    let mut im = Immediate::new(gpu)?;
    let (target, rtv) = gpu.target(SIZE, SIZE, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, SIZE, SIZE);
    let plain = topology11::plain();
    for (i, (topology, vertices)) in plain.iter().enumerate() {
        im.viewport(cell(i as u32, 4, 64.0));
        im.topology = *topology;
        im.draw_colored(vertices)?;
    }
    let strip = topology11::zigzag(8);
    let cut16 = bytemuck::cast_slice(&topology11::CUT16);
    let cut32 = topology11::cut32();
    let indexed = [
        (
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            cut16,
            DXGI_FORMAT_R16_UINT,
        ),
        (
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
            bytemuck::cast_slice(&cut32),
            DXGI_FORMAT_R32_UINT,
        ),
        (
            D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
            cut16,
            DXGI_FORMAT_R16_UINT,
        ),
        (
            D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
            bytemuck::cast_slice(&cut32),
            DXGI_FORMAT_R32_UINT,
        ),
    ];
    for (i, (topology, indices, format)) in indexed.into_iter().enumerate() {
        im.viewport(cell((plain.len() + i) as u32, 4, 64.0));
        im.topology = topology;
        im.cut = if format == DXGI_FORMAT_R16_UINT {
            D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0xFFFF
        } else {
            D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_0xFFFFFFFF
        };
        im.draw_indexed(&strip, indices, format)?;
    }
    Ok(vec![Output::picture("", gpu.read(&target, 0, RT)?)])
}

/// Direct3D 11's geometry shader scene, the layers of its array target bound as one view.
pub fn geometry(gpu: &Gpu) -> Result<Vec<Output>> {
    const SIZE: u32 = 128;
    let mut im = Immediate::new(gpu)?;
    im.vs = dxil!("geometry.vs");
    let mut outputs = Vec::new();
    let triangles = geometry11::triangles();
    let parts: [(&str, _, _, _, &[[f32; 7]]); 3] = [
        (
            "points",
            dxil!("geometry.gs_points"),
            dxil!("geometry.ps"),
            D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
            &geometry11::points(),
        ),
        (
            "adjacency",
            dxil!("geometry.gs_adjacency"),
            dxil!("geometry.ps"),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ,
            &geometry11::adjacency(),
        ),
        (
            "strips",
            dxil!("geometry.gs_strips"),
            dxil!("geometry.ps_primitive"),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            &triangles,
        ),
    ];
    for (part, gs, ps, topology, vertices) in parts {
        let (target, rtv) = gpu.target(SIZE, SIZE, RGBA)?;
        im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, SIZE, SIZE);
        im.gs = Some(gs);
        im.ps = ps;
        im.topology = topology;
        im.draw_colored(vertices)?;
        outputs.push(Output::picture(part, gpu.read(&target, 0, RT)?));
    }

    let (target, rtv) = gpu.target(SIZE, SIZE, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, SIZE, SIZE);
    im.gs = None;
    im.ps = dxil!("geometry.ps_id");
    im.topology = D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
    im.draw_colored(&geometry11::fan())?;
    outputs.push(Output::picture("primitive_id", gpu.read(&target, 0, RT)?));

    let mut desc = tex2d_desc(64, 64, RGBA, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET);
    desc.DepthOrArraySize = 3;
    let clear = D3D12_CLEAR_VALUE {
        Format: RGBA,
        Anonymous: D3D12_CLEAR_VALUE_0 {
            Color: crate::scene::CLEAR,
        },
    };
    let layers = gpu.committed(D3D12_HEAP_TYPE_DEFAULT, &desc, RT, Some(&clear))?;
    let heap = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, false)?;
    unsafe {
        gpu.device
            .CreateRenderTargetView(&layers, None, gpu.cpu(&heap, 0))
    };
    im.begin(Some(&heap), None, DXGI_FORMAT_UNKNOWN, 64, 64);
    im.gs = Some(dxil!("geometry.gs_layers"));
    im.ps = dxil!("geometry.ps");
    im.draw_colored(&triangles[..3])?;
    for layer in 0..3 {
        outputs.push(Output::picture(
            &format!("layer{layer}"),
            gpu.read(&layers, layer, RT)?,
        ));
    }
    Ok(outputs)
}

/// Direct3D 11's tessellation scene, its factors given as root constants.
pub fn tessellation(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut im = Immediate::new(gpu)?;
    im.root = constants_root(gpu, tessellation11::FACTORS.len() as u32)?;
    im.constants = tessellation11::FACTORS.to_vec();
    im.vs = dxil!("tessellation.vs");
    im.ps = dxil!("tessellation.ps");
    let (target, rtv) = gpu.target(256, 128, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 256, 128);
    let (tri, quad, line) = tessellation11::control_points();
    let tri_list = D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST;
    let quad_list = D3D_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST;
    let patches: [(_, _, _, &[[f32; 7]]); 7] = [
        (
            dxil!("tessellation.hs_tri"),
            dxil!("tessellation.ds_tri"),
            tri_list,
            &tri,
        ),
        (
            dxil!("tessellation.hs_quad_odd"),
            dxil!("tessellation.ds_quad"),
            quad_list,
            &quad,
        ),
        (
            dxil!("tessellation.hs_quad_even"),
            dxil!("tessellation.ds_quad"),
            quad_list,
            &quad,
        ),
        (
            dxil!("tessellation.hs_quad_pow2"),
            dxil!("tessellation.ds_quad"),
            quad_list,
            &quad,
        ),
        (
            dxil!("tessellation.hs_quad_integer"),
            dxil!("tessellation.ds_quad"),
            quad_list,
            &quad,
        ),
        (
            dxil!("tessellation.hs_line"),
            dxil!("tessellation.ds_line"),
            D3D_PRIMITIVE_TOPOLOGY_2_CONTROL_POINT_PATCHLIST,
            &line,
        ),
        (
            dxil!("tessellation.hs_tri"),
            dxil!("tessellation.ds_tri"),
            tri_list,
            &tri,
        ),
    ];
    // The last patch is drawn solid.
    let last = patches.len() - 1;
    for (i, (hs, ds, topology, vertices)) in patches.into_iter().enumerate() {
        im.raster = if i == last {
            solid()
        } else {
            D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_WIREFRAME,
                ..solid()
            }
        };
        im.viewport(cell(i as u32, 4, 64.0));
        im.hs = Some(hs);
        im.ds = Some(ds);
        im.topology = topology;
        im.draw_colored(vertices)?;
    }
    Ok(vec![Output::picture("", gpu.read(&target, 0, RT)?)])
}

/// Direct3D 11's stream output scene. Direct3D 12 has no DrawAuto: the vertices written are drawn
/// back by the count the buffer's filled size gives, read back first; and no overflow predicate:
/// the statistics say it.
pub fn streamout(gpu: &Gpu) -> Result<Vec<Output>> {
    const SIZE: u32 = 128;
    const STRIDE: u32 = 32;
    let root = gpu.root_signature(&D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
        Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
            Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT
                    | D3D12_ROOT_SIGNATURE_FLAG_ALLOW_STREAM_OUTPUT,
                ..Default::default()
            },
        },
    })?;
    let entry = |name| D3D12_SO_DECLARATION_ENTRY {
        Stream: 0,
        SemanticName: name,
        SemanticIndex: 0,
        StartComponent: 0,
        ComponentCount: 4,
        OutputSlot: 0,
    };
    let declaration = [entry(s!("SV_Position")), entry(s!("COLOR"))];
    let colored = [
        element(s!("POSITION"), DXGI_FORMAT_R32G32B32_FLOAT, 0),
        element(s!("COLOR"), DXGI_FORMAT_R32G32B32A32_FLOAT, 12),
    ];
    let written_layout = [
        element(s!("POSITION"), DXGI_FORMAT_R32G32B32A32_FLOAT, 0),
        element(s!("COLOR"), DXGI_FORMAT_R32G32B32A32_FLOAT, 16),
    ];
    let pipeline = |vs: &[u8],
                    gs: Option<&[u8]>,
                    layout: &[D3D12_INPUT_ELEMENT_DESC]|
     -> Result<ID3D12PipelineState> {
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: held(&root),
            VS: bytecode(vs),
            GS: gs.map(bytecode).unwrap_or_default(),
            PS: bytecode(dxil!("common.ps_color")),
            BlendState: blend12(&no_blend()),
            SampleMask: u32::MAX,
            RasterizerState: raster12(&solid()),
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: layout.as_ptr(),
                NumElements: layout.len() as u32,
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        if gs.is_some() {
            desc.StreamOutput = D3D12_STREAM_OUTPUT_DESC {
                pSODeclaration: declaration.as_ptr(),
                NumEntries: declaration.len() as u32,
                pBufferStrides: &STRIDE,
                NumStrides: 1,
                RasterizedStream: 0,
            };
        }
        desc.RTVFormats[0] = RGBA;
        let made = unsafe { gpu.device.CreateGraphicsPipelineState(&desc) };
        release(&mut desc.pRootSignature);
        Ok(made?)
    };
    let out = pipeline(
        dxil!("common.vs_color"),
        Some(dxil!("streamout.gs")),
        &colored,
    )?;
    let back = pipeline(dxil!("streamout.vs_back"), None, &written_layout)?;
    let triangles = streamout11::triangles();
    let mut statistics: Option<ID3D12QueryHeap> = None;
    unsafe {
        gpu.device.CreateQueryHeap(
            &D3D12_QUERY_HEAP_DESC {
                Type: D3D12_QUERY_HEAP_TYPE_SO_STATISTICS,
                Count: 1,
                NodeMask: 0,
            },
            &mut statistics,
        )?
    };
    let statistics = statistics.context("no query heap")?;

    let (target, rtv) = gpu.target(SIZE, SIZE, RGBA)?;
    gpu.begin(&rtv, SIZE, SIZE);
    let mut text = String::new();
    let mut written = None;
    // Room for all 12 vertices, then for only 5.
    for (name, bytes) in [("enough", 12 * STRIDE), ("small", 5 * STRIDE)] {
        let stream = |size: u32| {
            gpu.committed(
                D3D12_HEAP_TYPE_DEFAULT,
                &buffer_desc(size as u64),
                D3D12_RESOURCE_STATE_STREAM_OUT,
                None,
            )
        };
        // The buffer, and its 64-bit filled size, which a committed resource starts with at zero.
        let (buffer, filled) = (stream(bytes)?, stream(8)?);
        let results = gpu.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer_desc(16),
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
        )?;
        let vertices = gpu.upload(bytemuck::cast_slice(&triangles))?;
        let list = &gpu.list;
        unsafe {
            list.OMSetRenderTargets(1, Some(&gpu.cpu(&rtv, 0)), false, None);
            area(list, 0, 0, SIZE, SIZE);
            list.SetGraphicsRootSignature(&root);
            list.SetPipelineState(&out);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            list.IASetVertexBuffers(
                0,
                Some(&[D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: vertices.GetGPUVirtualAddress(),
                    SizeInBytes: (triangles.len() * 28) as u32,
                    StrideInBytes: 28,
                }]),
            );
            list.SOSetTargets(
                0,
                Some(&[D3D12_STREAM_OUTPUT_BUFFER_VIEW {
                    BufferLocation: buffer.GetGPUVirtualAddress(),
                    SizeInBytes: bytes as u64,
                    BufferFilledSizeLocation: filled.GetGPUVirtualAddress(),
                }]),
            );
            list.BeginQuery(&statistics, D3D12_QUERY_TYPE_SO_STATISTICS_STREAM0, 0);
            list.DrawInstanced(triangles.len() as u32, 1, 0, 0);
            list.EndQuery(&statistics, D3D12_QUERY_TYPE_SO_STATISTICS_STREAM0, 0);
            list.SOSetTargets(0, None);
            list.ResolveQueryData(
                &statistics,
                D3D12_QUERY_TYPE_SO_STATISTICS_STREAM0,
                0,
                1,
                &results,
                0,
            );
        }
        gpu.keep(&vertices);
        let stats: [u64; 2] = bytemuck::pod_read_unaligned(&gpu.read_buffer(
            &results,
            16,
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?);
        let size: u64 = bytemuck::pod_read_unaligned(&gpu.read_buffer(
            &filled,
            8,
            D3D12_RESOURCE_STATE_STREAM_OUT,
        )?);
        writeln!(
            text,
            "{name}: {} primitives written, {} needed, {size} bytes filled",
            stats[0], stats[1]
        )?;
        if name == "enough" {
            // The written vertices as a float picture, a row per vertex: position, then colour.
            let floats: Vec<f32> = bytemuck::pod_collect_to_vec(&gpu.read_buffer(
                &buffer,
                bytes as u64,
                D3D12_RESOURCE_STATE_STREAM_OUT,
            )?);
            written = Some(
                image::Rgba32FImage::from_raw(2, 12, floats).expect("12 vertices of 8 floats"),
            );
            // Drawn back mirrored, as many vertices as were written.
            unsafe {
                list.ResourceBarrier(&[transition(
                    &buffer,
                    D3D12_RESOURCE_STATE_STREAM_OUT,
                    D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER,
                )]);
                list.OMSetRenderTargets(1, Some(&gpu.cpu(&rtv, 0)), false, None);
                area(list, 0, 0, SIZE, SIZE);
                list.SetGraphicsRootSignature(&root);
                list.SetPipelineState(&back);
                list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                list.IASetVertexBuffers(
                    0,
                    Some(&[D3D12_VERTEX_BUFFER_VIEW {
                        BufferLocation: buffer.GetGPUVirtualAddress(),
                        SizeInBytes: size as u32,
                        StrideInBytes: STRIDE,
                    }]),
                );
                list.DrawInstanced(size as u32 / STRIDE, 1, 0, 0);
            }
            gpu.submit()?;
        }
    }
    let written = written.expect("the first buffer had room");
    Ok(vec![
        Output::text("results", text),
        Output::picture("buffer", written),
        Output::picture("", gpu.read(&target, 0, RT)?),
    ])
}
