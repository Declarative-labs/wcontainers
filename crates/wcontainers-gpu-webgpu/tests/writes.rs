//! Writes in their three modes keep the order plan section 4.6 promises: exact answers through
//! copies and read backs, and an ordered write inside a pass splits it without losing what it drew.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

mod allowance;

use wcontainers_gpu::{
    Backend, BufferDesc, BufferUsage, ColorAttachment, DeviceStatus, Extent, Format, LoadOp,
    MapIntent, Object, Origin, PassDesc, QueryKind, ReadSource, SlowPath, StoreOp, TextureRegion,
    TextureUsage, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

fn device() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    pollster::block_on(draw::device(backends))
        .expect("a GPU adapter")
        .0
}

fn words(v: u32) -> Vec<u8> {
    (0..4u32).flat_map(|i| (v + i).to_le_bytes()).collect()
}

/// A buffer copied out between writes of each mode: every copy sees the bytes written just before it.
#[test]
fn writes_take_effect_in_command_order() {
    let mut b = device();
    let buffer = |b: &mut WebGpu| {
        b.create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::READ_BACK,
            intent: MapIntent::Often,
        })
        .unwrap()
    };
    let src = buffer(&mut b);
    let dst: Vec<_> = (0..3).map(|_| buffer(&mut b)).collect();
    b.write_buffer(&src, 0, &words(100), WriteMode::Ordered);
    b.copy_buffer(&src, 0, &dst[0], 0, 16);
    b.write_buffer(&src, 0, &words(200), WriteMode::Ordered);
    b.copy_buffer(&src, 0, &dst[1], 0, 16);
    b.write_buffer(&src, 0, &words(300), WriteMode::Discard);
    b.copy_buffer(&src, 0, &dst[2], 0, 16);
    let reads: Vec<_> = dst
        .iter()
        .map(|d| {
            b.read_back(&ReadSource::Buffer {
                buffer: d,
                offset: 0,
                size: 16,
            })
            .unwrap()
        })
        .collect();
    b.submit();
    for (rb, want) in reads.into_iter().zip([100, 200, 300]) {
        b.wait_read(&rb);
        let mut got = vec![0; 16];
        b.finish_read(rb, &mut got);
        assert_eq!(got, words(want));
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
    let c = b.counters();
    assert_eq!((c.staged_writes, c.renames), (1, 1), "{c:?}");
}

/// An ordered write of a vertex buffer the open pass already drew with ends the pass, copies, and
/// opens it again loading what was drawn: the picture is still Direct3D's triangle.
#[test]
fn ordered_write_in_a_pass_splits_it() {
    let mut b = device();
    let p = draw::prepare(&mut b, scene::Scene::Triangle, draw::Twist::None).unwrap();
    let t = draw::targets(&mut b, None).unwrap();
    draw::begin(&mut b, &t, true);
    draw::draw(&mut b, &p);
    let vertices: Vec<u8> = bytemuck::cast_slice(&scene::TRIANGLE).to_vec();
    b.write_buffer(&p.vertices, 0, &vertices, WriteMode::Ordered);
    draw::draw(&mut b, &p);
    b.end_pass();
    let rb = draw::read(&mut b, &t).unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
    b.finish_read(rb, &mut rgba);
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(b.slow_paths().count(SlowPath::WriteSplitPass), 1);
    assert!(
        allowance::within(&rgba, &reference("triangle.png"), "triangle.png"),
        "the split pass lost or changed what it drew"
    );
}

fn read_buffer_back(
    b: &mut WebGpu,
    buffer: &<WebGpu as Backend>::Buffer,
    size: u64,
) -> <WebGpu as Backend>::Readback {
    b.read_back(&ReadSource::Buffer {
        buffer,
        offset: 0,
        size,
    })
    .unwrap()
}

fn finish(b: &mut WebGpu, rb: <WebGpu as Backend>::Readback, len: usize) -> Vec<u8> {
    b.wait_read(&rb);
    let mut got = vec![0; len];
    b.finish_read(rb, &mut got);
    got
}

fn buffer(
    b: &mut WebGpu,
    size: u64,
    usage: BufferUsage,
    intent: MapIntent,
) -> <WebGpu as Backend>::Buffer {
    b.create_buffer(&BufferDesc {
        size,
        usage: usage | BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::READ_BACK,
        intent,
    })
    .unwrap()
}

/// A read back recorded before a write sees the bytes from before it, in every mode that may meet
/// recorded work, for buffers and textures.
#[test]
fn read_backs_see_the_bytes_before_a_later_write() {
    let mut b = device();
    for mode in [WriteMode::Ordered, WriteMode::Discard] {
        let buf = buffer(&mut b, 16, BufferUsage::empty(), MapIntent::Rarely);
        b.write_buffer(&buf, 0, &words(1), WriteMode::Ordered);
        let f = b.submit();
        b.wait_fence(&f);
        let rb = read_buffer_back(&mut b, &buf, 16);
        b.write_buffer(&buf, 0, &words(2), mode);
        let after = read_buffer_back(&mut b, &buf, 16);
        b.submit();
        assert_eq!(finish(&mut b, rb, 16), words(1), "{mode:?}");
        assert_eq!(finish(&mut b, after, 16), words(2), "{mode:?}");
    }
    let (texture, _) = draw::texture_2d(
        &mut b,
        Format::Rgba8Unorm,
        4,
        4,
        TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
    )
    .unwrap();
    let region = TextureRegion {
        texture: &texture,
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    let extent = Extent {
        width: 4,
        height: 4,
        depth_or_layers: 1,
    };
    b.write_texture(&region, extent, &[11; 64], 16, 4, WriteMode::Ordered);
    let f = b.submit();
    b.wait_fence(&f);
    let rb = b
        .read_back(&ReadSource::Texture {
            region: TextureRegion { ..region },
            extent,
        })
        .unwrap();
    b.write_texture(&region, extent, &[22; 64], 16, 4, WriteMode::Ordered);
    let after = b
        .read_back(&ReadSource::Texture { region, extent })
        .unwrap();
    b.submit();
    assert_eq!(finish(&mut b, rb, 64), [11; 64]);
    assert_eq!(finish(&mut b, after, 64), [22; 64]);
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Destroying a buffer and a texture after recording a read back of them, before the submit, leaves
/// the recorded read back its bytes.
#[test]
fn destroy_after_a_recorded_read_back_waits_for_it() {
    let mut b = device();
    let buf = buffer(&mut b, 16, BufferUsage::empty(), MapIntent::Rarely);
    b.write_buffer(&buf, 0, &words(5), WriteMode::Ordered);
    let (texture, _) = draw::texture_2d(
        &mut b,
        Format::Rgba8Unorm,
        4,
        4,
        TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
    )
    .unwrap();
    let region = TextureRegion {
        texture: &texture,
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    let extent = Extent {
        width: 4,
        height: 4,
        depth_or_layers: 1,
    };
    b.write_texture(&region, extent, &[33; 64], 16, 4, WriteMode::Ordered);
    let f = b.submit();
    b.wait_fence(&f);
    let from_buffer = read_buffer_back(&mut b, &buf, 16);
    let from_texture = b
        .read_back(&ReadSource::Texture { region, extent })
        .unwrap();
    b.destroy(Object::Buffer(buf));
    b.destroy(Object::Texture(texture));
    b.submit();
    assert_eq!(finish(&mut b, from_buffer, 16), words(5));
    assert_eq!(finish(&mut b, from_texture, 64), [33; 64]);
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A clear to a non-zero pattern over more than one pattern block, at an offset, leaves the bytes
/// around it alone; the slow path is counted once.
#[test]
fn clears_to_a_pattern_fill_exactly() {
    const SIZE: u64 = 200_000;
    let mut b = device();
    let buf = buffer(&mut b, SIZE + 32, BufferUsage::empty(), MapIntent::Rarely);
    b.write_buffer(
        &buf,
        0,
        &vec![0xee; (SIZE + 32) as usize],
        WriteMode::Ordered,
    );
    let pattern = [1, 2, 3, 4, 5, 6, 7, 8];
    b.clear_buffer(&buf, 16, SIZE, &pattern);
    let rb = read_buffer_back(&mut b, &buf, SIZE + 32);
    b.submit();
    let got = finish(&mut b, rb, (SIZE + 32) as usize);
    let mut want = vec![0xee; 16];
    want.extend(pattern.iter().cycle().take(SIZE as usize));
    want.extend([0xee; 16]);
    assert!(got == want, "the pattern clear wrote the wrong bytes");
    assert_eq!(b.slow_paths().count(SlowPath::PatternClearCopied), 1);
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Uniform buffers written often live in the arena: discards between copies move them and each copy
/// sees its own bytes; copies between two of them, read backs and destroying them keep exact bytes.
#[test]
fn arena_buffers_keep_command_order() {
    let mut b = device();
    let arena = |b: &mut WebGpu| {
        b.create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::UNIFORM,
            intent: MapIntent::Often,
        })
        .unwrap()
    };
    let (a, c) = (arena(&mut b), arena(&mut b));
    let out: Vec<_> = (0..3)
        .map(|_| buffer(&mut b, 16, BufferUsage::empty(), MapIntent::Rarely))
        .collect();
    b.write_buffer(&a, 0, &words(10), WriteMode::Discard);
    b.copy_buffer(&a, 0, &out[0], 0, 16);
    b.write_buffer(&a, 0, &words(20), WriteMode::Discard);
    b.copy_buffer(&a, 0, &out[1], 0, 16);
    b.copy_buffer(&a, 0, &c, 0, 16);
    b.write_buffer(&a, 0, &words(30), WriteMode::Ordered);
    b.copy_buffer(&c, 0, &out[2], 0, 16);
    let reads: Vec<_> = out
        .iter()
        .map(|o| read_buffer_back(&mut b, o, 16))
        .collect();
    let direct = read_buffer_back(&mut b, &a, 16);
    b.destroy(Object::Buffer(a));
    b.submit();
    for (rb, want) in reads.into_iter().zip([10, 20, 20]) {
        assert_eq!(finish(&mut b, rb, 16), words(want));
    }
    assert_eq!(finish(&mut b, direct, 16), words(30));
    assert_eq!(b.status(), DeviceStatus::Ok);
    let k = b.counters();
    assert_eq!((k.renames, k.slices, k.staged_writes), (0, 1, 1), "{k:?}");
    assert_eq!(b.slow_paths().count(SlowPath::CopyWithinBuffer), 1);
}

/// Seeded random buffer writes in every mode, copies between buffers and within one, and clears to
/// patterns, at any byte offset and length, checked against a model of the bytes after each batch.
/// A `NoOverwrite` write is made only where it keeps its promise: over bytes no command and no
/// `Ordered` write since the last wait touches; elsewhere the write is `Ordered`.
/// `WEBGPU_SEED` replays a run; `WEBGPU_CASES` is how many operations it runs (default 400).
#[test]
fn byte_granular_buffer_operations_match_a_model() {
    const SIZE: usize = 1024;
    let seed = std::env::var("WEBGPU_SEED").map_or(0x5eed, |s| s.parse().unwrap());
    let cases: usize = std::env::var("WEBGPU_CASES").map_or(400, |s| s.parse().unwrap());
    eprintln!("seed {seed}, {cases} operations");
    let mut rng = fastrand::Rng::with_seed(seed);
    let mut b = device();
    let bufs = [
        buffer(&mut b, SIZE as u64, BufferUsage::VERTEX, MapIntent::Rarely),
        buffer(&mut b, SIZE as u64, BufferUsage::empty(), MapIntent::Rarely),
    ];
    let mut model = [vec![0u8; SIZE], vec![0u8; SIZE]];
    // Bytes a command or an ordered write touched since the last wait.
    let mut busy = [vec![true; SIZE], vec![true; SIZE]];
    let mark = |busy: &mut [Vec<bool>; 2], i: usize, at: usize, len: usize| {
        busy[i][at..at + len].fill(true);
    };
    for (buf, m) in bufs.iter().zip(&mut model) {
        rng.fill(m);
        b.write_buffer(buf, 0, m, WriteMode::Ordered);
    }
    let (mut no_overwrites, mut demoted, mut within) = (0, 0, 0);
    for case in 0..cases {
        let i = rng.usize(0..2);
        let len = rng.usize(1..=64);
        let at = rng.usize(0..=SIZE - len);
        match rng.u8(0..3) {
            0 => {
                let data: Vec<u8> = (0..len).map(|_| rng.u8(..)).collect();
                let mut mode = [
                    WriteMode::Ordered,
                    WriteMode::NoOverwrite,
                    WriteMode::Discard,
                ][rng.usize(0..3)];
                if mode == WriteMode::NoOverwrite {
                    if busy[i][at..at + len].contains(&true) {
                        mode = WriteMode::Ordered;
                        demoted += 1;
                    } else {
                        no_overwrites += 1;
                    }
                }
                if mode == WriteMode::Discard {
                    // Discard drops the old contents: write the whole buffer.
                    rng.fill(&mut model[i]);
                    model[i][at..at + len].copy_from_slice(&data);
                    let whole = model[i].clone();
                    b.write_buffer(&bufs[i], 0, &whole, mode);
                    busy[i].fill(false);
                } else {
                    b.write_buffer(&bufs[i], at as u64, &data, mode);
                    model[i][at..at + len].copy_from_slice(&data);
                    if mode == WriteMode::Ordered {
                        mark(&mut busy, i, at, len);
                    }
                }
            }
            1 => {
                let j = rng.usize(0..2);
                let mut from = rng.usize(0..=SIZE - len);
                if i == j {
                    within += 1;
                    // Within one buffer the ranges do not overlap.
                    while from < at + len && at < from + len {
                        from = rng.usize(0..=SIZE - len);
                    }
                }
                b.copy_buffer(&bufs[j], from as u64, &bufs[i], at as u64, len as u64);
                let bytes = model[j][from..from + len].to_vec();
                model[i][at..at + len].copy_from_slice(&bytes);
                mark(&mut busy, j, from, len);
                mark(&mut busy, i, at, len);
            }
            _ => {
                let pattern: Vec<u8> = (0..[1, 2, 4, 8, 16][rng.usize(0..5)])
                    .map(|_| rng.u8(..))
                    .collect();
                let len = (len / pattern.len()).max(1) * pattern.len();
                let at = at.min(SIZE - len);
                b.clear_buffer(&bufs[i], at as u64, len as u64, &pattern);
                for (k, v) in model[i][at..at + len].iter_mut().enumerate() {
                    *v = pattern[k % pattern.len()];
                }
                mark(&mut busy, i, at, len);
            }
        }
        if case % 50 == 49 || case == cases - 1 {
            for (buf, m) in bufs.iter().zip(&model) {
                let rb = read_buffer_back(&mut b, buf, SIZE as u64);
                b.submit();
                assert!(
                    finish(&mut b, rb, SIZE) == *m,
                    "bytes differ from the model after operation {case} (seed {seed})"
                );
            }
            for x in &mut busy {
                x.fill(false);
            }
        }
    }
    eprintln!(
        "{no_overwrites} no-overwrite writes, {demoted} made ordered, {within} copies within one buffer"
    );
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(b.slow_paths().count(SlowPath::CopyWithinBuffer), within);
}

/// Writes of any length after a submit, so that none meets a recorded command, take effect in the
/// order they were made in every pair of modes, also when the first is not whole words.
#[test]
fn writes_of_any_length_keep_their_order() {
    let mut b = device();
    let modes = [
        WriteMode::Ordered,
        WriteMode::NoOverwrite,
        WriteMode::Discard,
    ];
    for first in modes {
        for second in modes {
            for (one, two) in [(&[1u8, 1, 1][..], &[2u8, 2, 2, 2][..]), (&[1; 4], &[2; 3])] {
                let x = buffer(&mut b, 16, BufferUsage::empty(), MapIntent::Rarely);
                b.write_buffer(&x, 0, &[0; 16], WriteMode::Ordered);
                let f = b.submit();
                b.wait_fence(&f);
                b.write_buffer(&x, 0, one, first);
                b.write_buffer(&x, 0, two, second);
                let rb = read_buffer_back(&mut b, &x, 4);
                b.submit();
                // A `Discard` leaves the bytes outside what it writes undefined.
                let defined = if second == WriteMode::Discard {
                    two.len()
                } else {
                    one.len().max(two.len())
                };
                let mut want = vec![1; defined];
                want[..two.len()].copy_from_slice(two);
                let got = finish(&mut b, rb, 4);
                assert_eq!(
                    got[..want.len()],
                    want[..],
                    "{first:?} {} bytes, then {second:?} {} bytes",
                    one.len(),
                    two.len()
                );
            }
        }
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// A write of bytes not in whole words into a buffer the open pass does not use, and one after a
/// `Discard` renamed a buffer it does use, go ahead of the pass without splitting it, also while an
/// occlusion query runs; the picture is still Direct3D's triangle.
#[test]
fn writes_ahead_of_a_pass_do_not_split_it() {
    let mut b = device();
    let p = draw::prepare(&mut b, scene::Scene::Triangle, draw::Twist::None).unwrap();
    let t = draw::targets(&mut b, None).unwrap();
    let other = buffer(&mut b, 16, BufferUsage::empty(), MapIntent::Rarely);
    let queries = b.create_query_set(QueryKind::OcclusionAny, 1).unwrap();
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: Some(&queries),
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &t.color_view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(scene::CLEAR.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    b.begin_query(0);
    draw::draw(&mut b, &p);
    b.write_buffer(&other, 1, &[7, 7, 7], WriteMode::Ordered);
    let mut vertices: Vec<u8> = bytemuck::cast_slice(&scene::TRIANGLE).to_vec();
    b.write_buffer(&p.vertices, 0, &vertices, WriteMode::Discard);
    // The last byte again alone, with the same value: not whole words.
    let last = vertices.pop().unwrap();
    b.write_buffer(
        &p.vertices,
        vertices.len() as u64,
        &[last],
        WriteMode::NoOverwrite,
    );
    draw::draw(&mut b, &p);
    b.end_query();
    b.end_pass();
    let rb = draw::read(&mut b, &t).unwrap();
    let small = read_buffer_back(&mut b, &other, 4);
    b.submit();
    assert_eq!(finish(&mut b, small, 4)[1..], [7, 7, 7]);
    let rgba = finish(&mut b, rb, (scene::WIDTH * scene::HEIGHT * 4) as usize);
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(b.slow_paths().count(SlowPath::WriteSplitPass), 0);
    assert!(
        allowance::within(&rgba, &reference("triangle.png"), "triangle.png"),
        "the picture changed"
    );
}

fn reference(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../wcontainers-graphics-test/references/d3d11/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut reader = png::Decoder::new(std::io::Cursor::new(std::fs::read(path).unwrap()))
        .read_info()
        .unwrap();
    let mut want = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut want).unwrap();
    want
}

/// A copy whose source and destination sit at different places within a word, larger than one
/// dispatch of the edge program may merge, is exact.
#[test]
fn large_copies_between_places_in_a_word_are_exact() {
    const SIZE: u64 = 16 << 20;
    let mut b = device();
    let total = SIZE + 16;
    let (s, d) = (
        buffer(&mut b, total, BufferUsage::empty(), MapIntent::Rarely),
        buffer(&mut b, total, BufferUsage::empty(), MapIntent::Rarely),
    );
    let src: Vec<u8> = (0..total).map(|i| (i * 7 + i / 251) as u8).collect();
    b.write_buffer(&s, 0, &src, WriteMode::Ordered);
    b.write_buffer(&d, 0, &vec![0xee; total as usize], WriteMode::Ordered);
    b.copy_buffer(&s, 1, &d, 2, SIZE);
    let rb = read_buffer_back(&mut b, &d, total);
    b.submit();
    let got = finish(&mut b, rb, total as usize);
    let mut want = vec![0xee; total as usize];
    want[2..2 + SIZE as usize].copy_from_slice(&src[1..1 + SIZE as usize]);
    assert_eq!(b.status(), DeviceStatus::Ok);
    let wrong = got.iter().zip(&want).filter(|(a, b)| a != b).count();
    assert_eq!(wrong, 0, "bytes wrong");
}

/// Clears to patterns of lengths that do not divide the pattern block, at an offset not in whole
/// words, fill exactly.
#[test]
fn clears_take_patterns_of_any_length() {
    const SIZE: u64 = 120_000;
    let mut b = device();
    for len in [3u8, 12, 16] {
        let buf = buffer(&mut b, SIZE + 2, BufferUsage::empty(), MapIntent::Rarely);
        b.write_buffer(
            &buf,
            0,
            &vec![0xee; (SIZE + 2) as usize],
            WriteMode::Ordered,
        );
        let pattern: Vec<u8> = (1..=len).collect();
        b.clear_buffer(&buf, 1, SIZE, &pattern);
        let rb = read_buffer_back(&mut b, &buf, SIZE + 2);
        b.submit();
        let got = finish(&mut b, rb, (SIZE + 2) as usize);
        let mut want = vec![0xee];
        want.extend(pattern.iter().cycle().take(SIZE as usize));
        want.push(0xee);
        let first = got.iter().zip(&want).position(|(a, b)| a != b);
        assert_eq!(first, None, "pattern of {len} bytes");
    }
    assert_eq!(b.status(), DeviceStatus::Ok);
}

/// Copies between non-overlapping ranges of one buffer, at places in a word alike and different,
/// and between two buffers of the uniform arena (one GPU buffer), go through a temporary buffer,
/// each counted once as `CopyWithinBuffer`.
#[test]
fn copies_within_one_buffer_are_exact_and_counted() {
    let mut b = device();
    let buf = buffer(&mut b, 64, BufferUsage::empty(), MapIntent::Rarely);
    let mut model: Vec<u8> = (0..64).map(|i| i * 3 + 1).collect();
    b.write_buffer(&buf, 0, &model, WriteMode::Ordered);
    for (from, to, len) in [(1, 37, 20), (0, 48, 16), (40, 2, 9)] {
        b.copy_buffer(&buf, from as u64, &buf, to as u64, len as u64);
        model.copy_within(from..from + len, to);
    }
    let arena = |b: &mut WebGpu| {
        b.create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::UNIFORM,
            intent: MapIntent::Often,
        })
        .unwrap()
    };
    let (a, c) = (arena(&mut b), arena(&mut b));
    b.write_buffer(&a, 0, &words(7), WriteMode::Ordered);
    b.copy_buffer(&a, 0, &c, 0, 16);
    let reads = [
        read_buffer_back(&mut b, &buf, 64),
        read_buffer_back(&mut b, &c, 16),
    ];
    b.submit();
    let [whole, copied] = reads;
    assert_eq!(finish(&mut b, whole, 64), model);
    assert_eq!(finish(&mut b, copied, 16), words(7));
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(b.slow_paths().count(SlowPath::CopyWithinBuffer), 4);
}

/// A scripted scene of four frames, each drawing the triangle four times with a `Discard` write of
/// its vertex buffer between draws: the first frame's three renames make GPU buffers, every later
/// frame's take the storage the frame before gave up, and destroying the buffer destroys the three.
#[test]
fn renamed_buffers_reuse_the_pools_storage() {
    let mut b = device();
    let p = draw::prepare(&mut b, scene::Scene::Triangle, draw::Twist::None).unwrap();
    let t = draw::targets(&mut b, None).unwrap();
    let vertices: Vec<u8> = bytemuck::cast_slice(&scene::TRIANGLE).to_vec();
    let mut seen = Vec::new();
    for _ in 0..4 {
        let before = *b.counters();
        draw::begin(&mut b, &t, true);
        draw::draw(&mut b, &p);
        for _ in 0..3 {
            b.write_buffer(&p.vertices, 0, &vertices, WriteMode::Discard);
            draw::draw(&mut b, &p);
        }
        b.end_pass();
        let rb = draw::read(&mut b, &t).unwrap();
        let fence = b.submit();
        b.wait_fence(&fence);
        b.wait_read(&rb);
        let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
        b.finish_read(rb, &mut rgba);
        assert!(allowance::within(
            &rgba,
            &reference("triangle.png"),
            "triangle.png"
        ));
        let c = b.counters();
        seen.push((
            c.renames - before.renames,
            c.spares_reused - before.spares_reused,
        ));
    }
    eprintln!("renames and spares reused per frame: {seen:?}");
    assert_eq!(seen, [(3, 0), (3, 3), (3, 3), (3, 3)]);
    b.destroy(Object::Buffer(p.vertices.clone()));
    let fence = b.submit();
    b.wait_fence(&fence);
    b.submit();
    assert_eq!(b.counters().spares_dropped, 3, "{:?}", b.counters());
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(b.slow_paths().count(SlowPath::WriteSplitPass), 0);
}

/// Ordered writes of a vertex buffer the open pass drew with split it while occlusion queries run:
/// each query goes on in the next part and its result adds both parts. Query 0 passes before the
/// split, query 1 only after it, query 2 draws nothing.
#[test]
fn queries_carry_across_pass_splits() {
    let mut b = device();
    let p = draw::prepare(&mut b, scene::Scene::Triangle, draw::Twist::None).unwrap();
    let t = draw::targets(&mut b, None).unwrap();
    let queries = b.create_query_set(QueryKind::OcclusionAny, 3).unwrap();
    let results = buffer(&mut b, 24, BufferUsage::empty(), MapIntent::Rarely);
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: Some(&queries),
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &t.color_view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(scene::CLEAR.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    let vertices: Vec<u8> = bytemuck::cast_slice(&scene::TRIANGLE).to_vec();
    b.begin_pass(&pass);
    b.begin_query(0);
    draw::draw(&mut b, &p);
    b.write_buffer(&p.vertices, 0, &vertices, WriteMode::Ordered);
    b.end_query();
    b.begin_query(1);
    b.write_buffer(&p.vertices, 0, &vertices, WriteMode::Ordered);
    draw::draw(&mut b, &p);
    b.end_query();
    b.begin_query(2);
    b.write_buffer(&p.vertices, 0, &vertices, WriteMode::Ordered);
    b.end_query();
    b.end_pass();
    b.resolve_queries(&queries, 0, 3, &results, 0).unwrap();
    let rb = draw::read(&mut b, &t).unwrap();
    let answers = read_buffer_back(&mut b, &results, 24);
    b.submit();
    let words: Vec<u64> = finish(&mut b, answers, 24)
        .chunks(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect();
    let rgba = finish(&mut b, rb, (scene::WIDTH * scene::HEIGHT * 4) as usize);
    assert_eq!(b.status(), DeviceStatus::Ok);
    eprintln!("query results: {words:?}");
    assert!(words[0] != 0 && words[1] != 0 && words[2] == 0, "{words:?}");
    assert_eq!(b.slow_paths().count(SlowPath::WriteSplitPass), 3);
    assert_eq!(b.slow_paths().count(SlowPath::QueriesSegmented), 3);
    assert!(
        allowance::within(&rgba, &reference("triangle.png"), "triangle.png"),
        "the picture changed"
    );
}
