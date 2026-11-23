//! Direct3D's 32 input slots and 32 registers from the vertex to the pixel stage, which feature
//! level 11_0 requires, drawn on a real GPU through a back end with fewer: slots past its vertex
//! buffers are fetched in the program and values past its inter-stage locations moved onto free
//! ones. Programs from fxc (tests/registers/compile.cmd). Needs a GPU: run it through
//! `tools/graphics-rig/gpu-job`. `WEBGPU_API` picks wgpu's API (dx12, vulkan, metal).

use std::sync::Arc;

use wcontainers_direct3d_12::features::has_abilities_of_11_0;
use wcontainers_direct3d_12::list::{
    Command, CommandList, CopyLocation, Descriptor, TargetView, VertexBufferView, ViewportF,
};
use wcontainers_direct3d_12::resource::{
    DIMENSION_BUFFER, DIMENSION_TEXTURE2D, FLAG_RENDER_TARGET, HeapType, LAYOUT_ROW_MAJOR,
    ResourceDesc,
};
use wcontainers_direct3d_12::root::{
    FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT, RootSignatureDesc, VERSION_1_0, serialize,
};
use wcontainers_direct3d_12::state::{
    Blend, CULL_MODE_NONE, FILL_MODE_SOLID, InputElement, Raster, TOPOLOGY_TRIANGLELIST,
    TOPOLOGY_TYPE_TRIANGLE,
};
use wcontainers_direct3d_12::{Device, GraphicsPipelineDesc};
use wcontainers_directx_graphics_infrastructure::dxgi;
use wcontainers_directx_graphics_infrastructure::stage_registers::INPUT_SLOTS;
use wcontainers_gpu::{Backend, ColorWrites, DeviceStatus, SlowPath};
use wcontainers_gpu_webgpu::WebGpu;

const VS: &[u8] = include_bytes!("registers/vs.dxbc");
const PS: &[u8] = include_bytes!("registers/ps.dxbc");
/// The target's side.
const TARGET_SIDE: u32 = 4;
/// What real Direct3D 12 draws, on the RTX 3080 and on WARP alike: the pixel shader's weighted
/// sum of the 31 values (tests/registers/ps.hlsl) of the first vertex.
const DRAWN: [f32; 4] = [409.25, 454.25, 1395.0, 797.5];

fn webgpu() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    desc.flags |= WebGpu::INSTANCE_FLAGS;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("a GPU adapter");
    let desc = WebGpu::device_descriptor(&adapter);
    let (device, queue) = pollster::block_on(adapter.request_device(&desc)).unwrap();
    WebGpu::new(&adapter, device, queue)
}

fn buffer(size: u64) -> ResourceDesc {
    ResourceDesc {
        dimension: DIMENSION_BUFFER,
        width: size,
        height: 1,
        depth_or_array_size: 1,
        mip_levels: 1,
        sample_count: 1,
        layout: LAYOUT_ROW_MAJOR,
        ..Default::default()
    }
}

/// The values a stream's vertices hold: slot 0 a triangle covering the target, each other slot
/// three floats of its own.
fn vertex_bytes() -> Vec<u8> {
    let corners = [-1.0f32, 1.0, 3.0, 1.0, -1.0, -3.0];
    let values = (0..(INPUT_SLOTS - 1) * 3).map(|k| k as f32 * 0.25 - 7.0);
    corners
        .into_iter()
        .chain(values)
        .flat_map(f32::to_le_bytes)
        .collect()
}

#[test]
fn thirty_two_input_slots_and_stage_registers_draw_as_direct3d_12() {
    let mut device = Device::new(webgpu());
    let caps = device.backend().caps().clone();
    assert!(has_abilities_of_11_0(&caps), "{:?}", caps.limits);
    let root = device
        .create_root_signature(&serialize(&RootSignatureDesc {
            version: VERSION_1_0,
            parameters: vec![],
            samplers: vec![],
            flags: FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        }))
        .unwrap();
    let element = |semantic: &str, index: u32, format: u32| InputElement {
        semantic: semantic.into(),
        semantic_index: index,
        format,
        slot: index,
        offset: 0,
        per_instance: false,
        step_rate: 0,
    };
    let mut inputs = vec![element("P", 0, dxgi::R32G32_FLOAT)];
    inputs.extend((1..INPUT_SLOTS).map(|i| element("A", i, dxgi::R32_FLOAT)));
    let mut blend = Blend::default();
    blend.targets[0].write_mask = ColorWrites::all().bits();
    let float4 = dxgi::R32G32B32A32_FLOAT;
    let pso = device
        .create_graphics_pipeline(&GraphicsPipelineDesc {
            root: Some(root.clone()),
            vs: VS,
            ps: PS,
            blend,
            sample_mask: u32::MAX,
            raster: Raster {
                fill: FILL_MODE_SOLID,
                cull: CULL_MODE_NONE,
                depth_clip: true,
                ..Default::default()
            },
            inputs,
            topology_type: TOPOLOGY_TYPE_TRIANGLE,
            rtv_formats: vec![float4],
            samples: 1,
            ..Default::default()
        })
        .unwrap();
    let bytes = vertex_bytes();
    let vertices = device
        .create_committed(HeapType::Upload, &buffer(bytes.len() as u64))
        .unwrap();
    let mapped = device.map(vertices).unwrap();
    mapped.iter().zip(&bytes).for_each(|(c, b)| c.set(*b));
    let base = device.address(vertices);
    let float = size_of::<f32>() as u32;
    let position = 2 * float;
    let mut views = vec![VertexBufferView {
        address: base,
        size: 3 * position,
        stride: position,
    }];
    views.extend((0..INPUT_SLOTS - 1).map(|i| VertexBufferView {
        address: base + u64::from(3 * position + i * 3 * float),
        size: 3 * float,
        stride: float,
    }));
    let desc = ResourceDesc {
        dimension: DIMENSION_TEXTURE2D,
        width: u64::from(TARGET_SIDE),
        height: TARGET_SIDE,
        depth_or_array_size: 1,
        mip_levels: 1,
        format: float4,
        sample_count: 1,
        flags: FLAG_RENDER_TARGET,
        ..Default::default()
    };
    let target = device.create_committed(HeapType::Default, &desc).unwrap();
    let (footprints, total) = desc.footprints(0, 1, 0).unwrap();
    let readback = device
        .create_committed(HeapType::Readback, &buffer(total))
        .unwrap();
    let view = Descriptor::RenderTarget(TargetView {
        resource: target,
        format: float4,
        mip: 0,
        first_layer: 0,
        layers: 1,
    });
    let commands = vec![
        Command::ClearTarget {
            view,
            color: [0.0; 4],
            rects: vec![],
        },
        Command::RootSignature {
            compute: false,
            root: Some(root),
        },
        Command::Pipeline(pso),
        Command::Targets {
            colors: vec![view],
            depth: None,
        },
        Command::Viewports(vec![ViewportF {
            width: TARGET_SIDE as f32,
            height: TARGET_SIDE as f32,
            max_depth: 1.0,
            ..Default::default()
        }]),
        Command::Scissors(vec![[0, 0, TARGET_SIDE as i32, TARGET_SIDE as i32]]),
        Command::Topology(TOPOLOGY_TRIANGLELIST),
        Command::VertexBuffers { start: 0, views },
        Command::Draw {
            vertices: 3,
            instances: 1,
            first_vertex: 0,
            first_instance: 0,
        },
        Command::CopyTexture {
            dst: CopyLocation::Footprint {
                resource: readback,
                footprint: footprints[0].footprint,
            },
            at: [0; 3],
            src: CopyLocation::Subresource {
                resource: target,
                index: 0,
            },
            area: None,
        },
    ];
    let mut list = CommandList::new();
    for c in commands {
        list.record(c);
    }
    list.close().unwrap();
    let lists: Vec<Arc<[Command]>> = vec![list.commands().unwrap().clone()];
    let queue = device.create_queue().unwrap();
    let fence = device.create_fence(0);
    device.execute(queue, &lists).unwrap();
    device.queue_signal(queue, fence, 1).unwrap();
    assert!(device.wait(fence, 1));
    assert_eq!(device.backend().status(), DeviceStatus::Ok);
    assert!(device.removed().is_none(), "{:?}", device.removed());
    let pitch = footprints[0].footprint.row_pitch as usize;
    let texel = 4 * float as usize;
    let read: Vec<u8> = device.map(readback).unwrap()[..total as usize]
        .iter()
        .map(std::cell::Cell::get)
        .collect();
    for y in 0..TARGET_SIDE as usize {
        for x in 0..TARGET_SIDE as usize {
            let at = &read[y * pitch + x * texel..][..texel];
            let drawn: Vec<f32> = at
                .chunks(4)
                .map(|w| f32::from_le_bytes(w.try_into().unwrap()))
                .collect();
            assert_eq!(drawn, DRAWN, "texel {x},{y}");
        }
    }
    // Slots past the back end's vertex buffers were fetched in the program.
    let pulled = device
        .backend()
        .slow_paths()
        .count(SlowPath::VertexPulledInShader);
    assert_eq!(pulled > 0, caps.limits.vertex_streams < INPUT_SLOTS);
}
