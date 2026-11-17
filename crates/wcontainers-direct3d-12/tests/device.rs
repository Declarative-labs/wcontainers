//! The device on the recording back end: the rig's Direct3D 12 triangle scene, call for call as
//! gt-d3d12 makes it, replayed and checked in the recorded stream.

#[allow(dead_code)]
#[path = "../../wcontainers-directx-graphics-infrastructure/tests/support/record.rs"]
mod record;

use std::sync::Arc;

use record::Recorder;
use wcontainers_gpu::Backend;
use wcontainers_direct3d_12::list::*;
use wcontainers_direct3d_12::resource::*;
use wcontainers_direct3d_12::root::{RootSignatureDesc, serialize};
use wcontainers_direct3d_12::state::*;
use wcontainers_direct3d_12::{Device, Error, GraphicsPipelineDesc, stand_in};

const VS: &[u8] = include_bytes!("../../wcontainers-graphics-test/shaders/dxil/triangle.vs.dxil");
const PS: &[u8] = include_bytes!("../../wcontainers-graphics-test/shaders/dxil/triangle.ps.dxil");
const RGBA8: u32 = 28;

#[test]
fn stand_ins_are_the_rig_corpus_shaders() {
    for (dxil, stage) in [
        (VS, wcontainers_gpu::ShaderStage::Vertex),
        (PS, wcontainers_gpu::ShaderStage::Fragment),
    ] {
        let found = stand_in::find(dxil);
        let hash = blake3::hash(dxil);
        assert_eq!(
            found.map(|s| s.stage),
            Some(stage),
            "no stand-in for {hash}"
        );
    }
    assert!(stand_in::find(&VS[1..]).is_none());
}

#[test]
fn footprints_follow_the_pitch_and_placement_rules() {
    let tex = ResourceDesc {
        dimension: DIMENSION_TEXTURE2D,
        width: 100,
        height: 30,
        depth_or_array_size: 2,
        mip_levels: 2,
        format: RGBA8,
        sample_count: 1,
        ..Default::default()
    };
    let (f, total) = tex.footprints(0, 4, 0).unwrap();
    assert_eq!(f[0].footprint.row_pitch, 512);
    assert_eq!((f[0].rows, f[0].row_size), (30, 400));
    assert_eq!(f[1].footprint.offset, 15360);
    assert_eq!((f[1].footprint.width, f[1].footprint.row_pitch), (50, 256));
    assert_eq!(f[2].footprint.offset, 19456);
    assert_eq!(total, f[3].footprint.offset + 256 * 14 + 200);
    let buffer = ResourceDesc {
        dimension: DIMENSION_BUFFER,
        width: 60,
        height: 1,
        depth_or_array_size: 1,
        mip_levels: 1,
        ..Default::default()
    };
    assert_eq!(buffer.footprints(0, 1, 0).unwrap().1, 60);
}

fn buffer(size: u64) -> ResourceDesc {
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

fn triangle_pipeline(
    device: &mut Device<Recorder>,
    vs: &[u8],
) -> Result<wcontainers_direct3d_12::PipelineId, Error> {
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

#[test]
fn unknown_dxil_is_refused() {
    let mut device = Device::new(Recorder::new());
    let mut other = VS.to_vec();
    other[100] ^= 1;
    assert!(matches!(
        triangle_pipeline(&mut device, &other),
        Err(Error::NotImplemented(_))
    ));
}

#[test]
fn the_triangle_scene_replays() {
    let mut device = Device::new(Recorder::new());
    let pso = triangle_pipeline(&mut device, VS).unwrap();
    assert_eq!(device.stand_ins_used(), 2);
    assert!(
        device
            .take_warnings()
            .contains(&"DXIL drawn through a hand-written WGSL stand-in")
    );
    let prepared = device.backend().calls.len();
    assert!(
        device.backend().calls[..prepared]
            .iter()
            .any(|c| c.starts_with("create") && c.contains("vertex_layout"))
    );

    let target_desc = ResourceDesc {
        dimension: DIMENSION_TEXTURE2D,
        width: 256,
        height: 256,
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
    let data: Vec<u8> = [
        [0.0f32, 0.5, 1.0, 0.0, 0.0],
        [0.5, -0.5, 0.0, 1.0, 0.0],
        [-0.5, -0.5, 0.0, 0.0, 1.0],
    ]
    .iter()
    .flatten()
    .flat_map(|f| f.to_le_bytes())
    .collect();
    let mapped = device.map(vertices).unwrap();
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), mapped, data.len()) };

    let rtv = Descriptor::RenderTarget(TargetView {
        resource: target,
        format: RGBA8,
        mip: 0,
        first_layer: 0,
        layers: 1,
    });
    let clear = [26.0 / 255.0, 26.0 / 255.0, 38.0 / 255.0, 1.0];
    let mut list = CommandList::new();
    list.record(Command::Pipeline(pso));
    list.record(Command::ClearTarget {
        view: rtv,
        color: clear,
        rects: vec![],
    });
    list.record(Command::Targets {
        colors: vec![rtv],
        depth: None,
    });
    list.record(Command::Viewports(vec![ViewportF {
        width: 256.0,
        height: 256.0,
        max_depth: 1.0,
        ..Default::default()
    }]));
    list.record(Command::Scissors(vec![[0, 0, 256, 256]]));
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

    let fence = device.create_fence(0);
    let start = device.backend().calls.len();
    let lists: Vec<Arc<[Command]>> = vec![
        list.commands().unwrap().clone(),
        copy.commands().unwrap().clone(),
    ];
    device.execute(&lists).unwrap();
    device.queue_signal(fence, 1).unwrap();
    assert!(device.wait(fence, 1));
    assert_eq!(device.completed(fence), 1);
    assert!(!device.wait(fence, 2));
    device.map(readback).unwrap();

    let calls = &device.backend().calls[start..];
    let pos = |k: &str| {
        calls
            .iter()
            .position(|c| {
                c.starts_with(k) || c.starts_with("create") && c.contains(&format!("(\"{k}\""))
            })
            .unwrap_or_else(|| panic!("no {k} in {calls:#?}"))
    };
    // Upload bytes first, then one pass cleared by its load, the draw, the copy out and its read back.
    assert!(
        calls[0].starts_with("write_buffer")
            && calls[0].contains("NoOverwrite")
            && calls[0].contains(" 256 ")
    );
    assert!(calls[pos("begin_pass")].contains("Clear"));
    assert!(pos("begin_pass") < pos("draw") && pos("draw") < pos("end_pass"));
    assert!(pos("end_pass") < pos("copy_texture_to_buffer"));
    let copy = &calls[pos("copy_texture_to_buffer")];
    assert!(copy.contains("row_pitch: 1024") && copy.contains("rows_per_layer: 256"));
    assert!(pos("copy_texture_to_buffer") < pos("read_back") && pos("read_back") < pos("submit"));
    assert!(
        calls
            .iter()
            .any(|c| c.starts_with("finish_read") && c.ends_with(&total.to_string()))
    );
    let draw = &calls[pos("draw")];
    assert!(
        draw.contains("TriangleList") && draw.contains("count: 3") && draw.contains("offset: 0")
    );
    assert_eq!(calls.iter().filter(|c| c.starts_with("draw")).count(), 1);
    assert_eq!(
        device
            .backend()
            .slow_paths()
            .count(wcontainers_gpu::SlowPath::MappedUploadWhole),
        1
    );
}

#[test]
fn a_bad_list_fails_at_close() {
    let mut list = CommandList::new();
    list.fail(Error::NotImplemented("root descriptor tables"));
    list.record(Command::Topology(4));
    assert_eq!(
        list.close(),
        Err(Error::NotImplemented("root descriptor tables"))
    );
}
