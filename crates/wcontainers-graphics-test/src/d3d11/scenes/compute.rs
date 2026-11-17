//! Compute shaders and unordered access: a typed texture written by an indirect dispatch,
//! structured and raw buffers, group-shared reduction, atomics, append and consume buffers with
//! CopyStructureCount (all appended values consumed again), a hidden counter, and a pixel shader counting overlaps through a texture.
//! Results whose order the GPU chooses (appends, counters) are sorted before they are recorded.

use std::fmt::Write;

use anyhow::Result;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

use super::{colored_program, draw_colored, hue};
use crate::d3d11::{Gpu, solid, tex2d};
use crate::dxbc;
use crate::output::Output;

/// A default buffer bindable for unordered access and as a shader resource; structured when
/// `stride` > 0, raw otherwise.
fn buffer(gpu: &Gpu, bytes: &[u8], stride: u32, extra: u32) -> Result<ID3D11Buffer> {
    let misc = if stride > 0 {
        D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0
    } else {
        D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0
    };
    gpu.buffer(
        &D3D11_BUFFER_DESC {
            ByteWidth: bytes.len() as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_UNORDERED_ACCESS.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            MiscFlags: (misc | extra as i32) as u32,
            StructureByteStride: stride,
            ..Default::default()
        },
        Some(bytes),
    )
}

/// An unordered access view of a whole buffer: raw words when `stride` is 0, else elements.
fn buffer_uav(
    gpu: &Gpu,
    buffer: &ID3D11Buffer,
    elements: u32,
    stride: u32,
    flags: D3D11_BUFFER_UAV_FLAG,
) -> Result<ID3D11UnorderedAccessView> {
    let mut desc = D3D11_UNORDERED_ACCESS_VIEW_DESC {
        Format: if stride > 0 {
            DXGI_FORMAT_UNKNOWN
        } else {
            DXGI_FORMAT_R32_TYPELESS
        },
        ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
        ..Default::default()
    };
    let raw = if stride > 0 {
        0
    } else {
        D3D11_BUFFER_UAV_FLAG_RAW.0 as u32
    };
    desc.Anonymous.Buffer = D3D11_BUFFER_UAV {
        FirstElement: 0,
        NumElements: elements,
        Flags: raw | flags.0 as u32,
    };
    gpu.uav(&buffer.cast()?, Some(&desc))
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytemuck::pod_collect_to_vec(bytes)
}

fn bind(
    gpu: &Gpu,
    cs: &[u8],
    views: &[Option<ID3D11UnorderedAccessView>],
    counts: &[u32],
) -> Result<()> {
    unsafe {
        gpu.context.CSSetShader(&gpu.cs(cs)?, None);
        gpu.context.CSSetUnorderedAccessViews(
            0,
            views.len() as u32,
            Some(views.as_ptr()),
            Some(counts.as_ptr()),
        );
    }
    Ok(())
}

fn unbind(gpu: &Gpu) {
    let none: [Option<ID3D11UnorderedAccessView>; 2] = [None, None];
    unsafe {
        gpu.context
            .CSSetUnorderedAccessViews(0, 2, Some(none.as_ptr()), None)
    };
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let c = &gpu.context;
    let mut outputs = Vec::new();
    let mut text = String::new();
    let keep = [u32::MAX; 2];

    // A 64 by 64 float texture written by an indirect dispatch of 8 by 8 groups.
    let image = gpu.texture2d(
        &tex2d(
            64,
            64,
            DXGI_FORMAT_R32G32B32A32_FLOAT,
            D3D11_BIND_UNORDERED_ACCESS,
        ),
        &[],
    )?;
    let args = gpu.buffer(
        &D3D11_BUFFER_DESC {
            ByteWidth: 12,
            Usage: D3D11_USAGE_DEFAULT,
            MiscFlags: D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS.0 as u32,
            ..Default::default()
        },
        Some(bytemuck::cast_slice(&[8u32, 8, 1])),
    )?;
    bind(
        gpu,
        dxbc!("compute.cs_texture"),
        &[Some(gpu.uav(&image.cast()?, None)?)],
        &keep,
    )?;
    unsafe { c.DispatchIndirect(&args, 0) };
    unbind(gpu);
    outputs.push(Output::picture("texture", gpu.read(&image, 0, 0)?));

    // Structured buffers of 64 items of float2 and uint.
    let items: Vec<u32> = (0..64u32)
        .flat_map(|i| [(i as f32 * 0.5).to_bits(), (-(i as f32)).to_bits(), i * 3])
        .collect();
    let input = buffer(gpu, bytemuck::cast_slice(&items), 12, 0)?;
    let output = buffer(gpu, &vec![0; 64 * 12], 12, 0)?;
    unsafe { c.CSSetShaderResources(0, Some(&[Some(gpu.srv(&input.cast()?, None)?)])) };
    bind(
        gpu,
        dxbc!("compute.cs_structured"),
        &[Some(buffer_uav(
            gpu,
            &output,
            64,
            12,
            D3D11_BUFFER_UAV_FLAG(0),
        )?)],
        &keep,
    )?;
    unsafe { c.Dispatch(1, 1, 1) };
    unbind(gpu);
    let result = words(&gpu.read_buffer(&output)?);
    for item in result.chunks(3).take(8).chain(result.chunks(3).skip(60)) {
        writeln!(
            text,
            "structured: {} {} {:#010x}",
            f32::from_bits(item[0]),
            f32::from_bits(item[1]),
            item[2]
        )?;
    }

    // Group-shared sums of 4 groups, then atomics on a raw buffer.
    let mut start = vec![0u32; 12];
    start[9] = u32::MAX;
    let raw = buffer(gpu, bytemuck::cast_slice(&start), 0, 0)?;
    let raw_view = buffer_uav(gpu, &raw, 12, 0, D3D11_BUFFER_UAV_FLAG(0))?;
    bind(
        gpu,
        dxbc!("compute.cs_shared"),
        &[Some(raw_view.clone())],
        &keep,
    )?;
    unsafe { c.Dispatch(4, 1, 1) };
    let sums = words(&gpu.read_buffer(&raw)?);
    writeln!(text, "group sums: {:?}", &sums[..4])?;
    bind(gpu, dxbc!("compute.cs_atomic"), &[Some(raw_view)], &keep)?;
    unsafe { c.Dispatch(4, 1, 1) };
    unbind(gpu);
    writeln!(text, "atomics: {:?}", words(&gpu.read_buffer(&raw)?))?;

    // Append every third index of 256, count them, consume 32 of them.
    let appended = buffer(gpu, &[0; 256 * 4], 4, 0)?;
    let append_view = buffer_uav(gpu, &appended, 256, 4, D3D11_BUFFER_UAV_FLAG_APPEND)?;
    bind(
        gpu,
        dxbc!("compute.cs_append"),
        &[Some(append_view.clone())],
        &[0, 0],
    )?;
    unsafe { c.Dispatch(4, 1, 1) };
    unbind(gpu);
    let count = buffer(gpu, &[0; 16], 0, 0)?;
    unsafe { c.CopyStructureCount(&count, 4, &append_view) };
    let counted = words(&gpu.read_buffer(&count)?)[1];
    let mut values = words(&gpu.read_buffer(&appended)?)[..counted as usize].to_vec();
    values.sort_unstable();
    writeln!(text, "appended {counted}: {values:?}")?;
    let total = buffer(gpu, &[0; 8], 0, 0)?;
    bind(
        gpu,
        dxbc!("compute.cs_consume"),
        &[
            Some(append_view.clone()),
            Some(buffer_uav(gpu, &total, 2, 0, D3D11_BUFFER_UAV_FLAG(0))?),
        ],
        &[u32::MAX, u32::MAX],
    )?;
    // All 86 are consumed, so the total does not depend on the order they were appended in.
    unsafe { c.Dispatch(counted / 2, 1, 1) };
    unbind(gpu);
    unsafe { c.CopyStructureCount(&count, 8, &append_view) };
    writeln!(
        text,
        "consumed: total and count {:?}, left {}",
        words(&gpu.read_buffer(&total)?),
        words(&gpu.read_buffer(&count)?)[2]
    )?;

    // A hidden counter handing out slots.
    let slots = buffer(gpu, &[0xff; 128 * 4], 4, 0)?;
    let slots_view = buffer_uav(gpu, &slots, 128, 4, D3D11_BUFFER_UAV_FLAG_COUNTER)?;
    bind(
        gpu,
        dxbc!("compute.cs_counter"),
        &[Some(slots_view.clone())],
        &[5, 0],
    )?;
    unsafe { c.Dispatch(4, 1, 1) };
    unbind(gpu);
    unsafe { c.CopyStructureCount(&count, 12, &slots_view) };
    let used = words(&gpu.read_buffer(&count)?)[3];
    let mut handed = words(&gpu.read_buffer(&slots)?);
    let untouched = handed[..5].to_vec();
    handed = handed[5..used as usize].to_vec();
    handed.sort_unstable();
    writeln!(
        text,
        "counter at {used}, below the start {untouched:x?}, handed out {handed:?}"
    )?;
    outputs.push(Output::text("results", text));

    // A pixel shader counting how many triangles cover each pixel.
    let size = 64;
    let (target, rtv) = gpu.target(size, size, DXGI_FORMAT_R8G8B8A8_UNORM)?;
    let overlaps = gpu.texture2d(
        &tex2d(
            size,
            size,
            DXGI_FORMAT_R32_UINT,
            D3D11_BIND_UNORDERED_ACCESS,
        ),
        &[],
    )?;
    let overlap_view = gpu.uav(&overlaps.cast()?, None)?;
    unsafe { c.ClearUnorderedAccessViewUint(&overlap_view, &[0; 4]) };
    gpu.begin(&rtv, None, size, size);
    colored_program(gpu)?;
    unsafe {
        c.RSSetState(&gpu.rasterizer(&solid())?);
        c.PSSetShader(&gpu.ps(dxbc!("compute.ps_overlap"))?, None);
        c.OMSetRenderTargetsAndUnorderedAccessViews(
            Some(&[Some(rtv)]),
            None,
            1,
            1,
            Some(&Some(overlap_view)),
            None,
        );
    }
    let mut v = Vec::new();
    for k in 0..6u32 {
        let a = k as f32 * 1.05;
        let [r, g, b] = hue(k, 6);
        for (dx, dy) in [(0.0, 0.9), (0.8, -0.6), (-0.8, -0.6)] {
            v.push([
                0.3 * a.cos() + dx * 0.8,
                0.3 * a.sin() + dy * 0.8,
                0.5,
                r,
                g,
                b,
                1.0,
            ]);
        }
    }
    draw_colored(gpu, &v)?;
    outputs.push(Output::picture("overlaps", gpu.read(&overlaps, 0, 0)?));
    outputs.push(Output::picture("", gpu.read(&target, 0, 0)?));
    Ok(outputs)
}
