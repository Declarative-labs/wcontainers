//! The device on the recording back end: the rig's Direct3D 12 triangle scene, call for call as
//! gt-d3d12 makes it, replayed and checked in the recorded stream.

#[allow(dead_code)]
#[path = "../../wcontainers-directx-graphics-infrastructure/tests/support/record.rs"]
mod record;
#[path = "support/triangle.rs"]
mod triangle;

use record::Recorder;
use triangle::{RGBA8, VS};
use wcontainers_direct3d_12::list::*;
use wcontainers_direct3d_12::resource::*;
use wcontainers_direct3d_12::{Device, Error};
use wcontainers_gpu::Backend;

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
fn bytecode_that_is_not_dxil_is_refused() {
    let mut device = Device::new(Recorder::new());
    assert!(matches!(
        triangle::pipeline(&mut device, &VS[..64]),
        Err(Error::Invalid(_))
    ));
    assert!(!device.take_warnings().is_empty());
}

#[test]
fn the_triangle_scene_replays() {
    let mut device = Device::new(Recorder::new());
    let queue = device.create_queue();
    triangle::pipeline(&mut device, VS).unwrap();
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
    device.execute(queue, &scene.lists).unwrap();
    device.queue_signal(queue, fence, 1).unwrap();
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

#[test]
fn a_rectangle_clear_comes_after_a_whole_clear_before_it() {
    let mut device = Device::new(Recorder::new());
    let queue = device.create_queue();
    let desc = ResourceDesc {
        dimension: DIMENSION_TEXTURE2D,
        width: 64,
        height: 64,
        depth_or_array_size: 1,
        mip_levels: 1,
        format: RGBA8,
        sample_count: 1,
        flags: FLAG_RENDER_TARGET,
        ..Default::default()
    };
    let target = device.create_committed(HeapType::Default, &desc).unwrap();
    let view = Descriptor::RenderTarget(TargetView {
        resource: target,
        format: RGBA8,
        mip: 0,
        first_layer: 0,
        layers: 1,
    });
    let mut list = CommandList::new();
    for rects in [vec![], vec![[4, 40, 28, 60]]] {
        list.record(Command::ClearTarget {
            view,
            color: [0.5; 4],
            rects,
        });
    }
    list.close().unwrap();
    let start = device.backend().calls.len();
    device
        .execute(queue, &[list.commands().unwrap().clone()])
        .unwrap();
    let passes: Vec<&String> = device.backend().calls[start..]
        .iter()
        .filter(|c| c.starts_with("begin_pass"))
        .collect();
    assert_eq!(passes.len(), 2, "{passes:#?}");
    assert!(passes[0].contains("Clear(") && passes[1].contains("Load"));
}

#[test]
fn fence_events_are_set_without_waiting() {
    let mut device = Device::new(Recorder::new());
    let queue = device.create_queue();
    let fence = device.create_fence(0);
    device.add_waiter(fence, 1, 7);
    device.queue_signal(queue, fence, 1).unwrap();
    // The recorder's GPU has not finished the work, and nothing waits for it.
    assert!(device.reached_events().is_empty() && device.has_waiters());
    device.backend().finish();
    assert_eq!(device.reached_events(), [7]);
    assert!(!device.has_waiters());
    assert!(device.backend().named("wait_fence").is_empty());
    // A CPU signal reaches its waiters at the next look.
    device.add_waiter(fence, 5, 8);
    device.cpu_signal(fence, 5);
    assert_eq!(device.reached_events(), [8]);
}

#[test]
fn back_buffers_are_resources_and_present_follows_the_work() {
    use wcontainers_directx_graphics_infrastructure::{Model, SwapChainDesc};
    let mut device = Device::new(Recorder::new());
    let queue = device.create_queue();
    let chain = device
        .create_swap_chain(
            1,
            &SwapChainDesc {
                width: 64,
                height: 32,
                format: RGBA8,
                buffers: 2,
                model: Model::FlipDiscard,
                usage: wcontainers_gpu::TextureUsage::empty(),
            },
        )
        .unwrap();
    let back = device.chain_buffer(chain, 0).unwrap();
    assert!(device.chain_buffer(chain, 1).is_some() && device.chain_buffer(chain, 2).is_none());
    assert_eq!(device.resource_desc(back).unwrap().width, 64);
    let mut list = CommandList::new();
    list.record(Command::ClearTarget {
        view: Descriptor::RenderTarget(TargetView {
            resource: back,
            format: RGBA8,
            mip: 0,
            first_layer: 0,
            layers: 1,
        }),
        color: [0.25; 4],
        rects: vec![],
    });
    list.close().unwrap();
    let start = device.backend().calls.len();
    device
        .execute(queue, &[list.commands().unwrap().clone()])
        .unwrap();
    device.present(chain, 1).unwrap();
    let calls = &device.backend().calls[start..];
    let at = |k: &str| calls.iter().position(|c| c.starts_with(k)).unwrap();
    assert!(at("begin_pass") < at("submit") && at("submit") < at("draw"));
    assert!(at("draw") < at("present "));
    assert_eq!(device.chain(chain).unwrap().current_index(), 1);
    device.resize_swap_chain(chain, 0, 16, 8, 0).unwrap();
    let back = device.chain_buffer(chain, 0).unwrap();
    assert_eq!(device.resource_desc(back).unwrap().height, 8);
    // The swap chain owns the textures: releasing its resources leaves them to it.
    let textures = |d: &mut Device<Recorder>| d.backend().named("destroy Texture(").len();
    let before = textures(&mut device);
    device.destroy_swap_chain(chain);
    assert_eq!(textures(&mut device), before + 2);
}

#[test]
fn queues_wait_for_fences_signalled_late() {
    let mut device = Device::new(Recorder::new());
    let (copy, compute) = (device.create_queue(), device.create_queue());
    let (upload, done) = (device.create_fence(0), device.create_fence(0));
    let mut list = CommandList::new();
    list.close().unwrap();
    let lists = [list.commands().unwrap().clone()];
    let submits = |d: &mut Device<Recorder>| d.backend().named("submit").len();
    // The compute queue waits for the copy queue, which waits for the CPU.
    device.queue_wait(compute, done, 1).unwrap();
    device.execute(compute, &lists).unwrap();
    device.queue_signal(compute, done, 2).unwrap();
    device.queue_wait(copy, upload, 1).unwrap();
    device.execute(copy, &lists).unwrap();
    device.queue_signal(copy, done, 1).unwrap();
    let before = submits(&mut device);
    assert_eq!(device.completed(done), 0);
    device.cpu_signal(upload, 1);
    // Released in the order of the calls: the copy queue's list and signal, then the compute
    // queue's.
    assert_eq!(submits(&mut device), before + 4);
    device.backend().finish();
    assert_eq!(device.completed(done), 2);
    // A wait never met holds its queue's work for good; other queues go on.
    device.queue_wait(copy, upload, 9).unwrap();
    device.execute(copy, &lists).unwrap();
    device.execute(compute, &lists).unwrap();
    assert_eq!(submits(&mut device), before + 5);
}

#[test]
fn placed_resources_lie_in_their_heap_and_overlaps_are_refused() {
    let mut device = Device::new(Recorder::new());
    let page = 64 << 10;
    let heap = device.create_heap(4 * page, HeapType::Default, 0).unwrap();
    let buffer = |width| ResourceDesc {
        dimension: DIMENSION_BUFFER,
        width,
        height: 1,
        depth_or_array_size: 1,
        mip_levels: 1,
        sample_count: 1,
        ..Default::default()
    };
    let a = device.create_placed(heap, 0, &buffer(page)).unwrap();
    let b = device.create_placed(heap, page, &buffer(2 * page)).unwrap();
    assert_ne!(device.address(a), device.address(b));
    assert!(matches!(
        device.create_placed(heap, 2 * page, &buffer(page)),
        Err(Error::NotImplemented(_))
    ));
    assert!(matches!(
        device.create_placed(heap, 100, &buffer(page)),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        device.create_placed(heap, 3 * page, &buffer(2 * page)),
        Err(Error::Invalid(_))
    ));
    device.destroy_resource(b);
    device.create_placed(heap, 2 * page, &buffer(page)).unwrap();
}
