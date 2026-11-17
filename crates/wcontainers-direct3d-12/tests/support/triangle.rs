//! The rig's Direct3D 12 triangle scene (crates/wcontainers-graphics-test/src/d3d12/scenes.rs),
//! call for call as gt-d3d12 makes it, on the device of any back end.

use std::sync::Arc;

use wcontainers_direct3d_12::list::*;
use wcontainers_direct3d_12::resource::*;
use wcontainers_direct3d_12::root::{RootSignatureDesc, serialize};
use wcontainers_direct3d_12::state::*;
use wcontainers_direct3d_12::{Device, Error, GraphicsPipelineDesc, PipelineId, ResourceId};
use wcontainers_gpu::Backend;

pub const VS: &[u8] =
    include_bytes!("../../../wcontainers-graphics-test/shaders/dxil/triangle.vs.dxil");
pub const PS: &[u8] =
    include_bytes!("../../../wcontainers-graphics-test/shaders/dxil/triangle.ps.dxil");
pub const RGBA8: u32 = 28;
pub const SIZE: u32 = 256;
/// The scene's clear colour and vertices (crates/wcontainers-graphics-test/src/scene.rs).
pub const CLEAR: [f32; 4] = [26.0 / 255.0, 26.0 / 255.0, 38.0 / 255.0, 1.0];
pub const TRIANGLE: [[f32; 5]; 3] = [
    [0.0, 0.8, 1.0, 0.0, 0.0],
    [0.8, -0.7, 0.0, 1.0, 0.0],
    [-0.8, -0.7, 0.0, 0.0, 1.0],
];

pub fn buffer(size: u64) -> ResourceDesc {
    ResourceDesc {
        dimension: DIMENSION_BUFFER,
        width: size,
        height: 1,
        depth_or_array_size: 1,
        mip_levels: 1,
        sample_count: 1,
        layout: 1,
        ..Default::default()
    }
}

pub fn pipeline<B: Backend>(device: &mut Device<B>, vs: &[u8]) -> Result<PipelineId, Error> {
    let root = device.create_root_signature(&serialize(&RootSignatureDesc {
        version: 1,
        parameters: vec![],
        samplers: vec![],
        flags: 1,
    }))?;
    let mut blend = Blend::default();
    blend.targets[0].write_mask = 15;
    let element = |name: &str, format, offset| InputElement {
        semantic: name.into(),
        semantic_index: 0,
        format,
        slot: 0,
        offset,
        per_instance: false,
        step_rate: 0,
    };
    device.create_graphics_pipeline(&GraphicsPipelineDesc {
        root: Some(root),
        vs,
        ps: PS,
        blend,
        sample_mask: u32::MAX,
        raster: Raster {
            fill: 3,
            cull: 1,
            depth_clip: true,
            ..Default::default()
        },
        inputs: vec![element("POSITION", 16, 0), element("COLOR", 6, 8)],
        topology_type: 3,
        rtv_formats: vec![RGBA8],
        samples: 1,
        ..Default::default()
    })
}

/// The scene recorded and ready to execute: a list drawing it and a list copying the target into a
/// readback buffer of `total` bytes laid out as `footprint`.
pub struct Scene {
    pub lists: Vec<Arc<[Command]>>,
    pub readback: ResourceId,
    pub footprint: Footprint,
    pub total: u64,
}

pub fn record<B: Backend>(device: &mut Device<B>) -> Scene {
    let pso = pipeline(device, VS).unwrap();
    let target_desc = ResourceDesc {
        dimension: DIMENSION_TEXTURE2D,
        width: SIZE.into(),
        height: SIZE,
        depth_or_array_size: 1,
        mip_levels: 1,
        format: RGBA8,
        sample_count: 1,
        flags: FLAG_RENDER_TARGET,
        ..Default::default()
    };
    let target = device
        .create_committed(HeapType::Default, &target_desc)
        .unwrap();
    let vertices = device
        .create_committed(HeapType::Upload, &buffer(256))
        .unwrap();
    let address = device.address(vertices);
    assert_ne!(address, 0);
    let data: Vec<u8> = TRIANGLE
        .iter()
        .flatten()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let mapped = device.map(vertices).unwrap();
    // The upload heap's memory stays where it is for the resource's life.
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), mapped, data.len()) };

    let rtv = Descriptor::RenderTarget(TargetView {
        resource: target,
        format: RGBA8,
        mip: 0,
        first_layer: 0,
        layers: 1,
    });
    let mut list = CommandList::new();
    list.record(Command::Pipeline(pso));
    list.record(Command::ClearTarget {
        view: rtv,
        color: CLEAR,
        rects: vec![],
    });
    list.record(Command::Targets {
        colors: vec![rtv],
        depth: None,
    });
    list.record(Command::Viewports(vec![ViewportF {
        width: SIZE as f32,
        height: SIZE as f32,
        max_depth: 1.0,
        ..Default::default()
    }]));
    list.record(Command::Scissors(vec![[0, 0, SIZE as i32, SIZE as i32]]));
    list.record(Command::Topology(4));
    list.record(Command::VertexBuffers {
        start: 0,
        views: vec![VertexBufferView {
            address,
            size: 60,
            stride: 20,
        }],
    });
    list.record(Command::Draw {
        vertices: 3,
        instances: 1,
        first_vertex: 0,
        first_instance: 0,
    });
    list.close().unwrap();

    let (f, total) = target_desc.footprints(0, 1, 0).unwrap();
    let readback = device
        .create_committed(HeapType::Readback, &buffer(total))
        .unwrap();
    let mut copy = CommandList::new();
    copy.record(Command::CopyTexture {
        dst: CopyLocation::Footprint {
            resource: readback,
            footprint: f[0].footprint,
        },
        at: [0; 3],
        src: CopyLocation::Subresource {
            resource: target,
            index: 0,
        },
        area: None,
    });
    copy.close().unwrap();
    Scene {
        lists: vec![
            list.commands().unwrap().clone(),
            copy.commands().unwrap().clone(),
        ],
        readback,
        footprint: f[0].footprint,
        total,
    }
}
