//! Direct3D 11's rasterizer, multisampling and multiple-target scenes drawn through Direct3D 12:
//! the same geometry and states, as pipeline state objects (with a geometry shader choosing among
//! eight viewports), multisampled targets resolved by the list, and four targets bound as one
//! descriptor range.

use std::mem::transmute_copy;

use anyhow::Result;
use image::DynamicImage;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::states::{Immediate, RT};
use super::textures::texture_root;
use super::{Gpu, area, tex2d_desc, transition};
use crate::d3d11::scenes::blend::no_blend;
use crate::d3d11::scenes::{msaa, raster as raster11, targets as targets11};
use crate::d3d11::{cell, solid};
use crate::dxil;
use crate::output::Output;

const RGBA: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

fn scissor(list: &ID3D12GraphicsCommandList, rect: RECT) {
    unsafe { list.RSSetScissorRects(&[rect]) };
}

/// Direct3D 11's rasterizer scene. Direct3D 12's scissor test is always on, so the cell drawn
/// without it has the whole target as its rectangle.
pub fn raster(gpu: &Gpu) -> Result<Vec<Output>> {
    let mut im = Immediate::new(gpu)?;
    let mut outputs = Vec::new();

    let (target, rtv) = gpu.target(192, 128, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 192, 128);
    for (i, (ccw, cull)) in raster11::culls().into_iter().enumerate() {
        im.raster = D3D11_RASTERIZER_DESC {
            CullMode: cull,
            FrontCounterClockwise: ccw.into(),
            ..solid()
        };
        im.viewport(cell(i as u32, 3, 64.0));
        im.draw_colored(&raster11::windings())?;
    }
    outputs.push(Output::picture("cull", gpu.read(&target, 0, RT)?));

    let (target, rtv) = gpu.target(256, 64, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 256, 64);
    for (i, (fill, cull)) in raster11::FILLS.into_iter().enumerate() {
        im.raster = D3D11_RASTERIZER_DESC {
            FillMode: fill,
            CullMode: cull,
            ..solid()
        };
        im.viewport(cell(i as u32, 4, 64.0));
        let mut v = raster11::fan();
        v.extend(raster11::windings());
        im.draw_colored(&v)?;
    }
    outputs.push(Output::picture("fill", gpu.read(&target, 0, RT)?));

    let (target, rtv) = gpu.target(128, 64, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 128, 64);
    im.raster = solid();
    for (i, on) in [true, false].into_iter().enumerate() {
        im.viewport(cell(i as u32, 2, 64.0));
        let whole = RECT {
            left: 0,
            top: 0,
            right: 128,
            bottom: 64,
        };
        scissor(
            &gpu.list,
            if on {
                raster11::scissor(i as i32)
            } else {
                whole
            },
        );
        im.draw_colored(&raster11::fan())?;
    }
    outputs.push(Output::picture("scissor", gpu.read(&target, 0, RT)?));

    let (target, rtv) = gpu.target(256, 128, RGBA)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, 256, 128);
    im.vs = dxil!("raster.vs");
    im.gs = Some(dxil!("raster.gs"));
    im.ps = dxil!("raster.ps");
    let viewports: Vec<D3D12_VIEWPORT> = raster11::viewports()
        .iter()
        .map(|v| unsafe { transmute_copy(v) })
        .collect();
    unsafe {
        gpu.list.RSSetViewports(&viewports);
        gpu.list.RSSetScissorRects(&raster11::scissors());
    }
    im.draw_colored(&raster11::spread())?;
    outputs.push(Output::picture("viewports", gpu.read(&target, 0, RT)?));
    Ok(outputs)
}

fn standard(samples: u32) -> DXGI_SAMPLE_DESC {
    DXGI_SAMPLE_DESC {
        Count: samples,
        Quality: D3D12_STANDARD_MULTISAMPLE_PATTERN.0 as u32,
    }
}

/// A multisampled target with the standard pattern, in the render target state, and its RTV heap.
fn ms_target(gpu: &Gpu, samples: u32) -> Result<(ID3D12Resource, ID3D12DescriptorHeap)> {
    let mut desc = tex2d_desc(
        msaa::SIZE,
        msaa::SIZE,
        msaa::FORMAT,
        D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
    );
    desc.SampleDesc = standard(samples);
    let clear = D3D12_CLEAR_VALUE {
        Format: msaa::FORMAT,
        Anonymous: D3D12_CLEAR_VALUE_0 {
            Color: crate::scene::CLEAR,
        },
    };
    let texture = gpu.committed(D3D12_HEAP_TYPE_DEFAULT, &desc, RT, Some(&clear))?;
    let heap = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, false)?;
    unsafe {
        gpu.device
            .CreateRenderTargetView(&texture, None, gpu.cpu(&heap, 0))
    };
    Ok((texture, heap))
}

/// Resolves a multisampled target into a single-sampled texture and reads that back.
fn resolve(gpu: &Gpu, texture: &ID3D12Resource) -> Result<DynamicImage> {
    let single = gpu.committed(
        D3D12_HEAP_TYPE_DEFAULT,
        &tex2d_desc(
            msaa::SIZE,
            msaa::SIZE,
            msaa::FORMAT,
            D3D12_RESOURCE_FLAG_NONE,
        ),
        D3D12_RESOURCE_STATE_RESOLVE_DEST,
        None,
    )?;
    unsafe {
        gpu.list
            .ResourceBarrier(&[transition(texture, RT, D3D12_RESOURCE_STATE_RESOLVE_SOURCE)]);
        gpu.list
            .ResolveSubresource(&single, 0, texture, 0, msaa::FORMAT);
        gpu.list
            .ResourceBarrier(&[transition(texture, D3D12_RESOURCE_STATE_RESOLVE_SOURCE, RT)]);
    }
    gpu.read(&single, 0, D3D12_RESOURCE_STATE_RESOLVE_DEST)
}

/// Direct3D 11's multisampling scene.
pub fn msaa_scene(gpu: &Gpu) -> Result<Vec<Output>> {
    const SIZE: u32 = msaa::SIZE;
    let mut im = Immediate::new(gpu)?;
    let mut outputs = Vec::new();
    // A cleared target of `samples` bound with the coloured program and no blending.
    let fresh = |im: &mut Immediate, samples: u32| -> Result<ID3D12Resource> {
        let (texture, rtv) = ms_target(gpu, samples)?;
        im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, SIZE, SIZE);
        im.samples = standard(samples);
        im.raster = D3D11_RASTERIZER_DESC {
            MultisampleEnable: true.into(),
            ..solid()
        };
        im.blend = no_blend();
        im.sample_mask = u32::MAX;
        im.ps = dxil!("common.ps_color");
        Ok(texture)
    };

    for samples in [2, 4, 8] {
        let texture = fresh(&mut im, samples)?;
        im.draw_colored(&msaa::wedges())?;
        outputs.push(Output::picture(
            &format!("edges{samples}"),
            resolve(gpu, &texture)?,
        ));
    }

    let texture = fresh(&mut im, 4)?;
    im.sample_mask = 0b0101;
    im.draw_colored(&msaa::wedges())?;
    outputs.push(Output::picture("sample_mask", resolve(gpu, &texture)?));

    let texture = fresh(&mut im, 4)?;
    im.blend.AlphaToCoverageEnable = true.into();
    im.draw_colored(&msaa::fade())?;
    outputs.push(Output::picture(
        "alpha_to_coverage",
        resolve(gpu, &texture)?,
    ));

    let texture = fresh(&mut im, 4)?;
    im.ps = dxil!("msaa.ps_sample");
    im.draw_colored(&msaa::wedges())?;
    outputs.push(Output::picture("per_sample", resolve(gpu, &texture)?));

    let texture = fresh(&mut im, 4)?;
    im.ps = dxil!("msaa.ps_coverage");
    im.draw_colored(&msaa::square())?;
    outputs.push(Output::picture("shader_coverage", resolve(gpu, &texture)?));

    // The wedges at 4 samples, then each quarter of a single-sampled picture shows one sample.
    let texture = fresh(&mut im, 4)?;
    im.draw_colored(&msaa::wedges())?;
    let views = gpu.descriptor_heap(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 1, true)?;
    unsafe {
        gpu.list.ResourceBarrier(&[transition(
            &texture,
            RT,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        )]);
        gpu.device
            .CreateShaderResourceView(&texture, None, gpu.cpu(&views, 0));
        gpu.list.SetDescriptorHeaps(&[Some(views.clone())]);
    }
    let (single, rtv) = gpu.target(SIZE, SIZE, msaa::FORMAT)?;
    im.begin(Some(&rtv), None, DXGI_FORMAT_UNKNOWN, SIZE, SIZE);
    im.samples = DXGI_SAMPLE_DESC {
        Count: 1,
        Quality: 0,
    };
    im.raster = solid();
    im.root = texture_root(gpu, D3D12_TEXTURE_ADDRESS_MODE_WRAP)?;
    im.vs = dxil!("common.vs_full");
    im.ps = dxil!("msaa.ps_load");
    im.colored = false;
    im.tables = vec![(1, gpu.gpu(&views, 0))];
    im.draw(3, [0.0; 4])?;
    outputs.push(Output::picture("load", gpu.read(&single, 0, RT)?));
    Ok(outputs)
}

/// Direct3D 11's multiple-target scene: four targets of different formats, each cleared to its
/// own value, bound as one range of descriptors and blended their own ways.
pub fn targets(gpu: &Gpu) -> Result<Vec<Output>> {
    const SIZE: u32 = targets11::SIZE;
    let heap = gpu.descriptor_heap(
        D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
        targets11::FORMATS.len() as u32,
        false,
    )?;
    let mut textures = Vec::new();
    for (i, &(format, clear, _)) in targets11::FORMATS.iter().enumerate() {
        let value = D3D12_CLEAR_VALUE {
            Format: format,
            Anonymous: D3D12_CLEAR_VALUE_0 { Color: clear },
        };
        let texture = gpu.committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &tex2d_desc(SIZE, SIZE, format, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET),
            RT,
            Some(&value),
        )?;
        let rtv = gpu.cpu(&heap, i as u32);
        unsafe {
            gpu.device.CreateRenderTargetView(&texture, None, rtv);
            gpu.list.ClearRenderTargetView(rtv, &clear, None);
        }
        textures.push(texture);
    }
    let mut im = Immediate::new(gpu)?;
    unsafe {
        gpu.list
            .OMSetRenderTargets(textures.len() as u32, Some(&gpu.cpu(&heap, 0)), true, None)
    };
    area(&gpu.list, 0, 0, SIZE, SIZE);
    im.targets = targets11::FORMATS.iter().map(|f| f.0).collect();
    im.blend = targets11::blend();
    im.ps = dxil!("targets.ps");
    im.draw_colored(&targets11::triangles())?;
    targets11::FORMATS
        .iter()
        .zip(&textures)
        .map(|(&(_, _, name), texture)| Ok(Output::picture(name, gpu.read(texture, 0, RT)?)))
        .collect()
}
