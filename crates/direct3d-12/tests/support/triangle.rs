//! The rig's Direct3D 12 triangle scene (crates/graphics-test/src/d3d12/scenes.rs),
//! call for call as gt-d3d12 makes it, on the device of any back end.

use std::sync::Arc;

use wcontainers_direct3d_12::list::*;
use wcontainers_direct3d_12::resource::*;
use wcontainers_direct3d_12::root::{
    FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT, RootSignatureDesc, VERSION_1_0, serialize,
};
use wcontainers_direct3d_12::state::*;
use wcontainers_direct3d_12::{Device, Error, GraphicsPipelineDesc, PipelineId, ResourceId};
use wcontainers_directx_graphics_infrastructure::dxgi;
use wcontainers_gpu::{Gpu, ColorWrites};

pub const VS: &[u8] =
    include_bytes!("../../../graphics-test/shaders/dxil/triangle.vs.dxil");
pub const PS: &[u8] =
    include_bytes!("../../../graphics-test/shaders/dxil/triangle.ps.dxil");
pub const RGBA8: u32 = dxgi::R8G8B8A8_UNORM;
pub const SIZE: u32 = 256;
/// The scene's clear colour and vertices (crates/graphics-test/src/scene.rs).
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
        layout: LAYOUT_ROW_MAJOR,
        ..Default::default()
    }
}

pub fn pipeline<B: Gpu>(device: &mut Device<B>, vs: &[u8]) -> Result<PipelineId, Error> {
    pipeline_with(
        device,
        vs,
        PS,
        inputs(),
        Raster {
            fill: FILL_MODE_SOLID,
            cull: CULL_MODE_NONE,
            depth_clip: true,
            ..Default::default()
        },
        TOPOLOGY_TYPE_TRIANGLE,
        dxgi::UNKNOWN,
    )
}

/// The scene's inputs: a two-component position, then a colour.
pub fn inputs() -> Vec<InputElement> {
    vec![
        element("POSITION", dxgi::R32G32_FLOAT, 0),
        element("COLOR", dxgi::R32G32B32_FLOAT, size_of::<[f32; 2]>() as u32),
    ]
}

/// Input element `name` of `format` at `offset` in slot 0.
pub fn element(name: &str, format: u32, offset: u32) -> InputElement {
    InputElement {
        semantic: name.into(),
        semantic_index: 0,
        format,
        slot: 0,
        offset,
        per_instance: false,
        step_rate: 0,
    }
}

/// The scene's pipeline with other programs, inputs, rasterising and topology type.
pub fn pipeline_with<B: Gpu>(
    device: &mut Device<B>,
    vs: &[u8],
    ps: &[u8],
    inputs: Vec<InputElement>,
    raster: Raster,
    topology_type: u32,
    dsv_format: u32,
) -> Result<PipelineId, Error> {
    let root = device.create_root_signature(&serialize(&RootSignatureDesc {
        version: VERSION_1_0,
        parameters: vec![],
        samplers: vec![],
        flags: FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
    }))?;
    let mut blend = Blend::default();
    blend.targets[0].write_mask = ColorWrites::all().bits();
    device.create_graphics_pipeline(&GraphicsPipelineDesc {
        root: Some(root),
        vs,
        ps,
        blend,
        sample_mask: u32::MAX,
        raster,
        inputs,
        topology_type,
        dsv_format,
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

pub fn record<B: Gpu>(device: &mut Device<B>) -> Scene {
    let pso = pipeline(device, VS).unwrap();
    let data: Vec<u8> = TRIANGLE
        .iter()
        .flatten()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    // The vertex buffer's size as gt-d3d12 makes it.
    record_with(device, pso, &data, size_of::<[f32; 5]>() as u32, 256)
}

/// The scene drawn with pipeline `pso` from vertices `data`, `stride` bytes apart, in an upload
/// buffer of `buffer_bytes`.
pub fn record_with<B: Gpu>(
    device: &mut Device<B>,
    pso: PipelineId,
    data: &[u8],
    stride: u32,
    buffer_bytes: u64,
) -> Scene {
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
        .create_committed(HeapType::Upload, &buffer(buffer_bytes))
        .unwrap();
    let address = device.address(vertices);
    assert_ne!(address, 0);
    let mapped = device.map(vertices).unwrap();
    mapped.iter().zip(data.iter()).for_each(|(c, b)| c.set(*b));

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
    list.record(Command::Topology(TOPOLOGY_TRIANGLELIST));
    list.record(Command::VertexBuffers {
        start: 0,
        views: vec![VertexBufferView {
            address,
            size: data.len() as u32,
            stride,
        }],
    });
    list.record(Command::Draw {
        vertices: data.len() as u32 / stride,
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
