//! Direct3D's 32 input slots and 32 registers from the last vertex stage to the pixel stage, which
//! feature level 11_0 requires, drawn on a real GPU through a back end with fewer: slots past its
//! vertex buffers are fetched in the program and values past its inter-stage locations moved onto
//! free ones, behind a geometry or domain shader and beside clip distances the front links too.
//! Programs from fxc (tests/registers/compile.cmd). Needs a GPU: run it through
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
use wcontainers_direct3d_12::root::RootSignature;
use wcontainers_direct3d_12::root::{
    FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT, RootSignatureDesc, VERSION_1_0, serialize,
};
use wcontainers_direct3d_12::state::{
    Blend, CULL_MODE_NONE, FILL_MODE_SOLID, InputElement, Raster, TOPOLOGY_TRIANGLELIST,
    TOPOLOGY_TYPE_PATCH, TOPOLOGY_TYPE_TRIANGLE,
};
use wcontainers_direct3d_12::{Device, GraphicsPipelineDesc, PipelineId};
use wcontainers_directx_graphics_infrastructure::dxgi;
use wcontainers_directx_graphics_infrastructure::stage_registers::INPUT_SLOTS;
use wcontainers_gpu::{Backend, ColorWrites, DeviceStatus, SlowPath};
use wcontainers_gpu_webgpu::WebGpu;

const VS: &[u8] = include_bytes!("registers/vs.dxbc");
const PS: &[u8] = include_bytes!("registers/ps.dxbc");
const GS_VS: &[u8] = include_bytes!("registers/gs-vs.dxbc");
const GS: &[u8] = include_bytes!("registers/gs.dxbc");
const VALUES_PS: &[u8] = include_bytes!("registers/values-ps.dxbc");
const DS_VS: &[u8] = include_bytes!("registers/ds-vs.dxbc");
const HS: &[u8] = include_bytes!("registers/hs.dxbc");
const DS: &[u8] = include_bytes!("registers/ds.dxbc");
const LINKED_VS: &[u8] = include_bytes!("registers/linked-vs.dxbc");
const LINKED_PS: &[u8] = include_bytes!("registers/linked-ps.dxbc");
/// The target's side.
const TARGET_SIDE: u32 = 4;
/// The target's side where the stages' values vary across it.
const VARIED_SIDE: u32 = 8;
/// `D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST`.
const TOPOLOGY_3_CONTROL_POINT_PATCHLIST: u32 = 35;
/// What real Direct3D 12 draws of the programs behind a geometry shader, behind a domain shader (the
/// same values at the patch's corners) and beside linked clip distances, on the RTX 3080 and on
/// WARP alike: the target's floats rounded to 1/1024, hashed ([`rounded`]).
const DRAWN_BEHIND_GEOMETRY: u64 = 0x530e_f171_d232_12b6;
const DRAWN_BEHIND_DOMAIN: u64 = 0x530e_f171_d232_12b6;
const DRAWN_BESIDE_LINKED: u64 = 0xd769_ff7f_c937_20f6;
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

/// A root signature with no parameters.
fn empty_root(device: &mut Device<WebGpu>) -> RootSignature {
    device
        .create_root_signature(&serialize(&RootSignatureDesc {
            version: VERSION_1_0,
            parameters: vec![],
            samplers: vec![],
            flags: FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        }))
        .unwrap()
}

/// The pipeline of `vs`, `hs`, `ds`, `gs` and `ps` (empty where absent) reading `inputs`, drawing
/// triangles (or patches, with a hull shader) into one `R32G32B32A32_FLOAT` target.
fn pipeline(
    device: &mut Device<WebGpu>,
    root: &RootSignature,
    [vs, hs, ds, gs, ps]: [&[u8]; 5],
    inputs: Vec<InputElement>,
) -> PipelineId {
    let mut blend = Blend::default();
    blend.targets[0].write_mask = ColorWrites::all().bits();
    device
        .create_graphics_pipeline(&GraphicsPipelineDesc {
            root: Some(root.clone()),
            vs,
            hs,
            ds,
            gs,
            ps,
            blend,
            sample_mask: u32::MAX,
            raster: Raster {
                fill: FILL_MODE_SOLID,
                cull: CULL_MODE_NONE,
                depth_clip: true,
                ..Default::default()
            },
            inputs,
            topology_type: match hs.is_empty() {
                true => TOPOLOGY_TYPE_TRIANGLE,
                false => TOPOLOGY_TYPE_PATCH,
            },
            rtv_formats: vec![dxgi::R32G32B32A32_FLOAT],
            samples: 1,
            ..Default::default()
        })
        .unwrap()
}

/// Three vertices of `views` drawn with `pso` as `topology` into a cleared `side` by `side`
/// target: its floats, row by row.
fn draw(
    device: &mut Device<WebGpu>,
    root: RootSignature,
    pso: PipelineId,
    topology: u32,
    views: Vec<VertexBufferView>,
    side: u32,
) -> Vec<f32> {
    let float4 = dxgi::R32G32B32A32_FLOAT;
    let desc = ResourceDesc {
        dimension: DIMENSION_TEXTURE2D,
        width: u64::from(side),
        height: side,
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
            width: side as f32,
            height: side as f32,
            max_depth: 1.0,
            ..Default::default()
        }]),
        Command::Scissors(vec![[0, 0, side as i32, side as i32]]),
        Command::Topology(topology),
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
    let row = 4 * size_of::<f32>() * side as usize;
    let read: Vec<u8> = device.map(readback).unwrap()[..total as usize]
        .iter()
        .map(std::cell::Cell::get)
        .collect();
    (0..side as usize)
        .flat_map(|y| read[y * pitch..][..row].chunks(4))
        .map(|w| f32::from_le_bytes(w.try_into().unwrap()))
        .collect()
}

/// `floats` rounded to 1/1024 and hashed (FNV-1a over each as eight little-endian bytes), as the
/// check that measured real Direct3D 12 hashed its targets.
fn rounded(floats: &[f32]) -> u64 {
    floats
        .iter()
        .flat_map(|v| ((v * 1024.0).round() as i64).to_le_bytes())
        .fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        })
}

#[test]
fn thirty_two_input_slots_and_stage_registers_draw_as_direct3d_12() {
    let mut device = Device::new(webgpu());
    let caps = device.backend().caps().clone();
    assert!(has_abilities_of_11_0(&caps), "{:?}", caps.limits);
    let root = empty_root(&mut device);
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
    let pso = pipeline(&mut device, &root, [VS, &[], &[], &[], PS], inputs);
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
    let drawn = draw(
        &mut device,
        root,
        pso,
        TOPOLOGY_TRIANGLELIST,
        views,
        TARGET_SIDE,
    );
    for (i, texel) in drawn.chunks(4).enumerate() {
        assert_eq!(texel, DRAWN, "texel {i}");
    }
    // Slots past the back end's vertex buffers were fetched in the program.
    let pulled = device
        .backend()
        .slow_paths()
        .count(SlowPath::VertexPulledInShader);
    assert_eq!(pulled > 0, caps.limits.vertex_streams < INPUT_SLOTS);
}

#[test]
fn registers_behind_geometry_and_domain_shaders_and_beside_linked_values_draw_as_direct3d_12() {
    let mut device = Device::new(webgpu());
    let root = empty_root(&mut device);
    let none: &[u8] = &[];
    for (name, programs, topology, expected) in [
        (
            "geometry",
            [GS_VS, none, none, GS, VALUES_PS],
            TOPOLOGY_TRIANGLELIST,
            DRAWN_BEHIND_GEOMETRY,
        ),
        (
            "domain",
            [DS_VS, HS, DS, none, VALUES_PS],
            TOPOLOGY_3_CONTROL_POINT_PATCHLIST,
            DRAWN_BEHIND_DOMAIN,
        ),
        (
            "linked",
            [LINKED_VS, none, none, none, LINKED_PS],
            TOPOLOGY_TRIANGLELIST,
            DRAWN_BESIDE_LINKED,
        ),
    ] {
        let pso = pipeline(&mut device, &root, programs, vec![]);
        let drawn = draw(
            &mut device,
            root.clone(),
            pso,
            topology,
            vec![],
            VARIED_SIDE,
        );
        assert_eq!(rounded(&drawn), expected, "{name}: {:?}", &drawn[..4]);
    }
}
