//! The device on the recording back end: the rig's Direct3D 12 triangle scene, call for call as
//! gt-d3d12 makes it, replayed and checked in the recorded stream.

#[allow(dead_code)]
#[path = "../../wcontainers-directx-graphics-infrastructure/tests/support/record.rs"]
mod record;
#[path = "support/triangle.rs"]
mod triangle;

use record::Recorder;
use triangle::{PS, RGBA8, VS};
use wcontainers_direct3d_12::list::*;
use wcontainers_direct3d_12::resource::*;
use wcontainers_direct3d_12::{Device, Error, stand_in};
use wcontainers_gpu::Backend;

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

#[test]
fn unknown_dxil_is_refused() {
    let mut device = Device::new(Recorder::new());
    let mut other = VS.to_vec();
    other[100] ^= 1;
    assert!(matches!(
        triangle::pipeline(&mut device, &other),
        Err(Error::NotImplemented(_))
    ));
}

#[test]
fn the_triangle_scene_replays() {
    let mut device = Device::new(Recorder::new());
    triangle::pipeline(&mut device, VS).unwrap();
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

    let scene = triangle::record(&mut device);
    assert_eq!((scene.footprint.row_pitch, scene.total), (1024, 1024 * 256));
    let fence = device.create_fence(0);
    let start = device.backend().calls.len();
    device.execute(&scene.lists).unwrap();
    device.queue_signal(fence, 1).unwrap();
    assert!(device.wait(fence, 1));
    assert_eq!(device.completed(fence), 1);
    assert!(!device.wait(fence, 2));
    device.map(scene.readback).unwrap();

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
            .any(|c| c.starts_with("finish_read") && c.ends_with(&scene.total.to_string()))
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
