//! The device on the recording back end: the rig's Direct3D 12 triangle scene, call for call as
//! gt-d3d12 makes it, replayed and checked in the recorded stream.

#[allow(dead_code)]
#[path = "../../../../directx-graphics-infrastructure/implementation/translated/tests/support/record.rs"]
mod record;
#[path = "support/triangle.rs"]
mod triangle;

use record::Recorder;
use triangle::{PS, RGBA8, VS};
use wcontainers_direct3d_12_translated::list::*;
use wcontainers_direct3d_12_translated::resource::*;
use wcontainers_direct3d_12_translated::state::*;
use wcontainers_direct3d_12_translated::{Device, Error};
use wcontainers_directx_graphics_infrastructure::dxgi::{self, D32_FLOAT};
use wcontainers_gpu::{Backend, Features};

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
fn every_texture_shape_check_feature_support_answers_is_made() {
    use wcontainers_direct3d_12_translated::features::format_support;
    use wcontainers_directx_graphics_infrastructure::support1;
    let mut device = Device::new(Recorder::new());
    let caps = device.backend().caps().clone();
    let shapes = [
        (support1::TEXTURE1D, DIMENSION_TEXTURE1D),
        (support1::TEXTURE2D, DIMENSION_TEXTURE2D),
        (support1::TEXTURE3D, DIMENSION_TEXTURE3D),
    ];
    let mut made = 0;
    for value in dxgi::R32G32B32A32_TYPELESS..=dxgi::B4G4R4A4_UNORM {
        let support = format_support(&caps, value).support1;
        for (shape, dimension) in shapes {
            if support & shape == 0 {
                continue;
            }
            let one = ResourceDesc {
                dimension,
                width: 1,
                height: 1,
                depth_or_array_size: 1,
                mip_levels: 1,
                format: value,
                sample_count: 1,
                ..Default::default()
            };
            let (format, _) = one.texture_format().unwrap();
            let (block_width, block_height, _) = format.block();
            let blocks = 4;
            let desc = ResourceDesc {
                width: u64::from(block_width * blocks),
                height: match dimension {
                    DIMENSION_TEXTURE1D => 1,
                    _ => block_height * blocks,
                },
                ..one
            };
            let made_here = check_creation(HeapType::Default, &desc, 0, false)
                .and_then(|()| device.create_committed(HeapType::Default, &desc));
            assert!(
                made_here.is_ok(),
                "format {value} in dimension {dimension}: {made_here:?}"
            );
            made += 1;
        }
    }
    assert!(made > 0);
}

/// The layouts real Direct3D 12 gives on the RTX 3080 (case `video-formats`).
#[test]
fn video_formats_are_laid_out_as_planes_and_pixel_pairs() {
    let desc = |format, width, height, mip_levels| ResourceDesc {
        dimension: DIMENSION_TEXTURE2D,
        width,
        height,
        depth_or_array_size: 1,
        mip_levels,
        format,
        sample_count: 1,
        ..Default::default()
    };
    let pitch = PITCH_ALIGNMENT as u32;
    let planar_pitch = 2 * pitch;
    let nv12 = desc(dxgi::NV12, 64, 32, 1);
    let (f, total) = nv12.footprints(0, 2, 0).unwrap();
    let shape = |i: usize| {
        let p = f[i].footprint;
        (
            p.format,
            p.offset,
            p.width,
            p.height,
            p.row_pitch,
            f[i].rows,
            f[i].row_size,
        )
    };
    assert_eq!(
        shape(0),
        (dxgi::R8_TYPELESS, 0, 64, 32, planar_pitch, 32, 64)
    );
    assert_eq!(
        shape(1),
        (
            dxgi::R8G8_TYPELESS,
            u64::from(planar_pitch) * 32,
            32,
            16,
            planar_pitch,
            16,
            64
        )
    );
    assert_eq!(total, 24128);
    let (f, total) = desc(dxgi::OPAQUE_420, 64, 32, 1)
        .footprints(0, 1, 0)
        .unwrap();
    assert_eq!(
        (f[0].footprint.format, f[0].footprint.row_pitch),
        (dxgi::R8_TYPELESS, planar_pitch)
    );
    assert_eq!(total, 15936);
    let yuy2 = desc(dxgi::YUY2, 64, 32, 1);
    let (f, total) = yuy2.footprints(0, 1, 0).unwrap();
    assert_eq!(
        (
            f[0].footprint.width,
            f[0].row_size,
            f[0].footprint.row_pitch
        ),
        (64, 128, pitch)
    );
    assert_eq!(total, 8064);
    for refused in [
        desc(dxgi::NV12, 64, 32, 2),
        desc(dxgi::NV12, 63, 32, 1),
        desc(dxgi::NV12, 64, 31, 1),
        desc(dxgi::YUY2, 63, 32, 1),
    ] {
        assert!(matches!(
            check_creation(HeapType::Default, &refused, 0, false),
            Err(Error::Invalid(_))
        ));
    }
    let mut device = Device::new(Recorder::new());
    for made in [nv12, yuy2, desc(dxgi::YUY2, 64, 31, 1)] {
        check_creation(HeapType::Default, &made, 0, false).unwrap();
        device.create_committed(HeapType::Default, &made).unwrap();
    }
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
    let queue = device.create_queue().unwrap();
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

/// The scene's inputs, rasterised with depth clipping `depth_clip` and fill mode `fill`.
fn scene_raster(depth_clip: bool, fill: u32) -> (Vec<InputElement>, Raster) {
    let raster = Raster {
        fill,
        cull: CULL_MODE_NONE,
        depth_clip,
        ..Default::default()
    };
    (triangle::inputs(), raster)
}

#[test]
fn depth_clamping_the_back_end_lacks_is_done_in_the_programs() {
    let mut device = Device::new(Recorder::with_caps(|c| {
        c.features.remove(Features::DEPTH_CLAMP)
    }));
    let (inputs, raster) = scene_raster(false, FILL_MODE_SOLID);
    let start = device.backend().calls.len();
    triangle::pipeline_with(
        &mut device,
        VS,
        PS,
        inputs.clone(),
        raster,
        TOPOLOGY_TYPE_TRIANGLE,
        dxgi::UNKNOWN,
    )
    .unwrap();
    // The back end clips; the programs keep depth inside the clip volume and clamp it.
    assert!(
        device.backend().calls[start..]
            .iter()
            .any(|c| c.starts_with("create")
                && c.contains("\"raster\"")
                && c.contains("depth_clip: true"))
    );
    // Without a depth target the vertex program's clamp is all; with one, the pixel program
    // writes the depth, and with no pixel shader the pixel program for the variant writes it.
    let depth_only = |device: &mut Device<Recorder>, dsv| {
        triangle::pipeline_with(
            device,
            VS,
            &[],
            inputs.clone(),
            raster,
            TOPOLOGY_TYPE_TRIANGLE,
            dsv,
        )
    };
    assert!(depth_only(&mut device, dxgi::UNKNOWN).is_ok());
    let start = device.backend().calls.len();
    let made = depth_only(&mut device, D32_FLOAT);
    assert!(made.is_ok(), "{made:?}");
    assert!(
        device.backend().calls[start..]
            .iter()
            .any(|c| c.starts_with("create") && c.contains("Fragment")),
        "a pixel program made for the variant"
    );
}

#[test]
fn wireframe_without_polygon_mode_is_refused_by_name() {
    let mut device = Device::new(Recorder::with_caps(|c| {
        c.features.remove(Features::POLYGON_MODE)
    }));
    let (inputs, raster) = scene_raster(true, FILL_MODE_WIREFRAME);
    let refused = triangle::pipeline_with(
        &mut device,
        VS,
        PS,
        inputs,
        raster,
        TOPOLOGY_TYPE_TRIANGLE,
        dxgi::UNKNOWN,
    );
    assert!(
        matches!(refused, Err(Error::NotImplemented(_))),
        "{refused:?}"
    );
}

#[test]
fn a_pipeline_description_with_no_topology_type_is_invalid() {
    let mut device = Device::new(Recorder::new());
    for (bias, ps) in [(0, PS), (1, PS), (0, &[][..])] {
        let (inputs, mut raster) = scene_raster(true, FILL_MODE_SOLID);
        raster.depth_bias = bias;
        let refused = triangle::pipeline_with(
            &mut device,
            VS,
            ps,
            inputs,
            raster,
            TOPOLOGY_TYPE_UNDEFINED,
            dxgi::UNKNOWN,
        );
        assert!(matches!(refused, Err(Error::Invalid(_))), "{refused:?}");
    }
}

#[test]
fn heap_flags_direct3d_12_refuses_are_invalid() {
    let buffer = triangle::buffer(1);
    let texture = ResourceDesc {
        dimension: DIMENSION_TEXTURE2D,
        ..buffer
    };
    let check = |flags, heap, desc: Option<&ResourceDesc>| check_heap_flags(flags, heap, desc);
    // Measured by `check_d3d12 answers` on committed default-heap buffers.
    for flags in [
        HEAP_FLAG_ALLOW_DISPLAY,
        HEAP_FLAG_SHARED_CROSS_ADAPTER,
        HEAP_FLAG_HARDWARE_PROTECTED,
        HEAP_FLAG_ALLOW_WRITE_WATCH,
        HEAP_FLAG_MANUAL_WRITE_TRACKING,
    ] {
        let answer = check(flags, HeapType::Default, Some(&buffer));
        assert!(matches!(answer, Err(Error::Invalid(_))), "{flags:#x}");
    }
    // Abilities the front lacks where Direct3D 12 may give them.
    let shared_across = HEAP_FLAG_SHARED | HEAP_FLAG_SHARED_CROSS_ADAPTER;
    for (flags, heap, desc) in [
        (HEAP_FLAG_SHARED, HeapType::Default, Some(&buffer)),
        (shared_across, HeapType::Default, Some(&buffer)),
        (HEAP_FLAG_ALLOW_DISPLAY, HeapType::Default, Some(&texture)),
        (HEAP_FLAG_ALLOW_WRITE_WATCH, HeapType::Upload, Some(&buffer)),
    ] {
        let answer = check(flags, heap, desc);
        assert!(
            matches!(answer, Err(Error::NotImplemented(_))),
            "{flags:#x}"
        );
    }
    assert_eq!(check(0, HeapType::Default, Some(&buffer)), Ok(()));
}

#[test]
fn a_back_end_that_lost_its_device_removes_it() {
    let mut device = Device::new(Recorder::new());
    let queue = device.create_queue().unwrap();
    assert_eq!(device.removed(), None);
    device.backend().lost = Some(wcontainers_gpu::LossReason::Error);
    assert_eq!(device.execute(queue, &[]), Err(Error::DeviceRemoved));
    assert_eq!(device.removed(), Some(Error::DeviceRemoved));
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
    let queue = device.create_queue().unwrap();
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
    let queue = device.create_queue().unwrap();
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
fn events_wait_for_every_fence_or_any_one() {
    let mut device = Device::new(Recorder::new());
    let (a, b) = (device.create_fence(0), device.create_fence(0));
    device.add_waiter_many(vec![(a, 1), (b, 1)], false, 1);
    device.add_waiter_many(vec![(a, 1), (b, 1)], true, 2);
    device.cpu_signal(a, 1);
    assert_eq!(device.reached_events(), [2]);
    device.cpu_signal(b, 1);
    assert_eq!(device.reached_events(), [1]);
    // A destroyed fence takes the events waiting on it with it.
    device.add_waiter_many(vec![(a, 2), (b, 1)], false, 3);
    device.destroy_fence(a);
    assert!(device.reached_events().is_empty() && !device.has_waiters());
}

#[test]
fn back_buffers_are_resources_and_present_follows_the_work() {
    use wcontainers_directx_graphics_infrastructure::{Model, SwapChainDesc};
    let mut device = Device::new(Recorder::new());
    let queue = device.create_queue().unwrap();
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
    let (copy, compute) = (
        device.create_queue().unwrap(),
        device.create_queue().unwrap(),
    );
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
fn placed_resources_lie_in_their_heap_and_overlapping_buffers_inherit_bytes() {
    use wcontainers_gpu::SlowPath::AliasCopied;
    let mut device = Device::new(Recorder::new());
    let queue = device.create_queue().unwrap();
    let page =
        wcontainers_directx_graphics_infrastructure::d3d::DEFAULT_RESOURCE_PLACEMENT_ALIGNMENT;
    let heap = device
        .create_heap(4 * page, 0, HeapType::Default, 0)
        .unwrap();
    let buffer = |width| ResourceDesc {
        dimension: DIMENSION_BUFFER,
        width,
        height: 1,
        depth_or_array_size: 1,
        mip_levels: 1,
        sample_count: 1,
        ..Default::default()
    };
    assert!(matches!(
        device.create_placed(heap, 100, &buffer(page)),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        device.create_placed(heap, 3 * page, &buffer(2 * page)),
        Err(Error::Invalid(_))
    ));
    // a and b share their second and first page; c overlaps neither.
    let a = device.create_placed(heap, 0, &buffer(2 * page)).unwrap();
    let b = device.create_placed(heap, page, &buffer(2 * page)).unwrap();
    let c = device.create_placed(heap, 3 * page, &buffer(page)).unwrap();
    assert_ne!(device.address(a), device.address(b));
    let upload = device
        .create_committed(HeapType::Upload, &buffer(2 * page))
        .unwrap();
    let readback = device
        .create_committed(HeapType::Readback, &buffer(2 * page))
        .unwrap();
    let copy = |dst, src| Command::CopyBuffer {
        dst,
        dst_offset: 0,
        src,
        src_offset: 0,
        size: page,
    };
    let run = |d: &mut Device<Recorder>, commands: Vec<Command>| {
        let mut list = CommandList::new();
        for c in commands {
            list.record(c);
        }
        list.close().unwrap();
        let start = d.backend().calls.len();
        d.execute(queue, &[list.commands().unwrap().clone()])
            .unwrap();
        d.backend().calls[start..]
            .iter()
            .filter(|c| c.starts_with("copy_buffer"))
            .cloned()
            .collect::<Vec<_>>()
    };
    let copied = |d: &mut Device<Recorder>| d.backend().slow_paths().count(AliasCopied);
    // Nothing written yet, and c overlaps nothing: no copies of inherited bytes.
    assert_eq!(
        run(&mut device, vec![copy(c, upload), copy(readback, c)]).len(),
        2
    );
    assert_eq!(copied(&mut device), 0);
    // b, read after a was written, first takes the page they share from a.
    let calls = run(&mut device, vec![copy(a, upload), copy(readback, b)]);
    assert_eq!(calls.len(), 3, "{calls:#?}");
    assert!(calls[1].ends_with(&format!(
        " {page} {} 0 {page}",
        calls[1].split(' ').nth(3).unwrap()
    )));
    assert_eq!(copied(&mut device), 1);
    // Up to date now: read again, b copies nothing; written, b gives a its bytes back.
    assert_eq!(run(&mut device, vec![copy(readback, b)]).len(), 1);
    let calls = run(&mut device, vec![copy(b, upload), copy(readback, a)]);
    assert_eq!(calls.len(), 3, "{calls:#?}");
    assert!(calls[1].ends_with(&format!(
        " 0 {} {page} {page}",
        calls[1].split(' ').nth(3).unwrap()
    )));
    assert_eq!(copied(&mut device), 2);
    // Buffers placed in an upload heap map the heap's one memory.
    let shared = device
        .create_heap(2 * page, 0, HeapType::Upload, 0)
        .unwrap();
    let x = device.create_placed(shared, 0, &buffer(2 * page)).unwrap();
    let y = device.create_placed(shared, page, &buffer(page)).unwrap();
    let (px, py) = (device.map(x).unwrap(), device.map(y).unwrap());
    assert_eq!(py as usize - px as usize, page as usize);
    device.destroy_resource(b);
    device.destroy_resource(a);
    assert_eq!(run(&mut device, vec![copy(readback, c)]).len(), 1);
}

#[test]
fn shared_buffers_take_the_program_bytes_and_give_back_the_gpu_ones() {
    let mut device = Device::new(Recorder::new());
    let queue = device.create_queue().unwrap();
    let buffer = ResourceDesc {
        dimension: DIMENSION_BUFFER,
        width: 64,
        height: 1,
        depth_or_array_size: 1,
        mip_levels: 1,
        sample_count: 1,
        layout: 1,
        ..Default::default()
    };
    assert!(check_creation(HeapType::Shared, &buffer, 0, false).is_ok());
    let storage = ResourceDesc {
        flags: FLAG_UNORDERED_ACCESS,
        ..buffer
    };
    assert!(matches!(
        check_creation(HeapType::Shared, &storage, 0, false),
        Err(Error::NotImplemented(_))
    ));
    let shared = device.create_committed(HeapType::Shared, &buffer).unwrap();
    let other = device.create_committed(HeapType::Default, &buffer).unwrap();
    let memory = device.map(shared).unwrap();
    // SAFETY: the 64 bytes `Map` gave.
    unsafe { std::slice::from_raw_parts_mut(memory, 64).fill(7) };
    let copy = |dst, dst_offset, src, size| Command::CopyBuffer {
        dst,
        dst_offset,
        src,
        src_offset: 0,
        size,
    };
    let run = |d: &mut Device<Recorder>, c: Command| {
        let mut list = CommandList::new();
        list.record(c);
        list.close().unwrap();
        let start = d.backend().calls.len();
        d.execute(queue, &[list.commands().unwrap().clone()])
            .unwrap();
        d.backend().calls[start..]
            .iter()
            .filter(|c| c.starts_with("write_buffer") || c.starts_with("finish_read"))
            .map(|c| c.split(" [").next().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    // Read by the GPU: the program's bytes go first, all of them.
    let calls = run(&mut device, copy(other, 0, shared, 64));
    assert_eq!(calls.len(), 1, "{calls:#?}");
    let id = calls[0].split(' ').nth(1).unwrap().to_string();
    assert_eq!(calls[0], format!("write_buffer {id} 0 64 NoOverwrite"));
    // Written by the GPU at 16..32, then read again before the program has seen that work end: the
    // GPU's bytes there are newer than the program's and are left out.
    assert!(run(&mut device, copy(shared, 16, other, 16)).is_empty());
    let calls = run(&mut device, copy(other, 0, shared, 64));
    assert_eq!(
        calls,
        [
            format!("write_buffer {id} 0 16 NoOverwrite"),
            format!("write_buffer {id} 32 32 NoOverwrite"),
        ]
    );
    // Once the work is done and the program maps the buffer, its memory holds the GPU's bytes.
    device.backend().finish();
    let start = device.backend().calls.len();
    assert_eq!(device.map(shared).unwrap(), memory);
    assert_eq!(device.backend().named("finish_read").len(), 1);
    assert!(device.backend().calls[start..][0].ends_with(" 16"));
    let calls = run(&mut device, copy(other, 0, shared, 64));
    assert_eq!(calls, [format!("write_buffer {id} 0 64 NoOverwrite")]);
}
