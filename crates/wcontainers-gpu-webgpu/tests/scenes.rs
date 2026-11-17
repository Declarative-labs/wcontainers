//! The rig's three Direct3D 11 reference scenes drawn through the back end on a real GPU and compared
//! with real Direct3D's pictures; changes that must fail are seen to fail; the caches are counted.
//!
//! Needs a GPU. `WEBGPU_API` picks wgpu's API (dx12, vulkan, metal); the default is the platform's.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use draw::Twist;
use scene::Scene;
use wcontainers_gpu::{Backend, DeviceStatus};
use wcontainers_gpu_webgpu::WebGpu;

/// The largest difference in any 8-bit channel allowed against Direct3D. The references were drawn
/// by real Direct3D on the RTX 3080, and through this back end on that GPU every pixel is equal
/// (Vulkan and Direct3D 12), so any allowance would hide real differences. On another GPU the
/// rig's allowance for WARP, 1, is what to expect (wgpu on Metal differs by 1 on the triangle).
const TOLERANCE: u8 = 0;

fn device() -> WebGpu {
    let backends = match std::env::var("WEBGPU_API").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("metal") => wgpu::Backends::METAL,
        _ => wgpu::Backends::PRIMARY,
    };
    let (b, adapter) = pollster::block_on(draw::device(backends)).expect("a GPU adapter");
    eprintln!("adapter: {adapter}");
    b
}

fn render(b: &mut WebGpu, which: Scene, twist: Twist) -> Vec<u8> {
    let rb = draw::render_on(b, which, twist, wcontainers_gpu::Format::Depth32Float).unwrap();
    b.wait_read(&rb);
    let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
    b.finish_read(rb, &mut rgba);
    assert_eq!(b.status(), DeviceStatus::Ok);
    rgba
}

fn reference(which: Scene) -> Vec<u8> {
    let path = format!(
        "{}/../wcontainers-graphics-test/references/d3d11/{}.png",
        env!("CARGO_MANIFEST_DIR"),
        which.name()
    );
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(
        (info.color_type, info.bit_depth),
        (png::ColorType::Rgba, png::BitDepth::Eight),
        "{path}"
    );
    buf.truncate(info.buffer_size());
    buf
}

/// Pixels differing by more than `TOLERANCE` in any channel, and the largest difference.
fn compare(a: &[u8], b: &[u8]) -> (usize, u8) {
    let mut worst = 0;
    let mut beyond = 0;
    for (p, q) in a.chunks(4).zip(b.chunks(4)) {
        let d = p
            .iter()
            .zip(q)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        worst = worst.max(d);
        if d > TOLERANCE {
            beyond += 1;
        }
    }
    (beyond, worst)
}

#[test]
fn scenes_match_direct3d() {
    let mut b = device();
    let mut failed = Vec::new();
    for which in Scene::ALL {
        let (drawn, expected) = (render(&mut b, which, Twist::None), reference(which));
        let (beyond, worst) = compare(&drawn, &expected);
        let differ = compare_exact(&drawn, &expected);
        eprintln!(
            "{}: {differ} pixels differ, {beyond} by more than {TOLERANCE}, largest difference {worst}",
            which.name()
        );
        if beyond > 0 {
            failed.push(which.name());
        }
    }
    assert!(failed.is_empty(), "differ from Direct3D: {failed:?}");
}

fn compare_exact(a: &[u8], b: &[u8]) -> usize {
    a.chunks(4).zip(b.chunks(4)).filter(|(p, q)| p != q).count()
}

#[test]
fn changed_blend_factor_and_depth_function_fail() {
    let mut b = device();
    let reference = reference(Scene::Quads);
    for twist in [Twist::BlendFactor, Twist::DepthFunction] {
        let (beyond, worst) = compare(&render(&mut b, Scene::Quads, twist), &reference);
        eprintln!(
            "quads with {twist:?}: {beyond} pixels beyond {TOLERANCE}, largest difference {worst}"
        );
        assert!(beyond > 0, "{twist:?} drew the reference picture");
    }
}

/// The quads drawn with each depth format of at least 24 bits the device holds, stencil cleared too
/// where the format has one, equal Direct3D's picture.
#[test]
fn quads_match_direct3d_on_every_depth_format() {
    use wcontainers_gpu::{Format, FormatUse};
    let mut b = device();
    let reference = reference(Scene::Quads);
    for format in [
        Format::Depth24Plus,
        Format::Depth24PlusStencil8,
        Format::Depth32Float,
        Format::Depth32FloatStencil8,
    ] {
        if !b
            .caps()
            .formats
            .get(format)
            .contains(FormatUse::DEPTH_STENCIL)
        {
            eprintln!("{format:?}: not held by this device");
            continue;
        }
        let rb = draw::render_on(&mut b, Scene::Quads, Twist::None, format).unwrap();
        b.wait_read(&rb);
        let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
        b.finish_read(rb, &mut rgba);
        assert_eq!(b.status(), DeviceStatus::Ok, "{format:?}");
        let (beyond, worst) = compare(&rgba, &reference);
        eprintln!(
            "quads on {format:?}: {beyond} pixels beyond {TOLERANCE}, largest difference {worst}"
        );
        assert_eq!(beyond, 0, "quads on {format:?} differ from Direct3D");
    }
}

/// Repeated draws of one combination build one pipeline and one bind group, and equal blocks made
/// twice are one block.
#[test]
fn pipelines_and_bind_groups_are_cached() {
    let mut b = device();
    let first = draw::prepare(&mut b, Scene::Quads, Twist::None).unwrap();
    let again = draw::prepare(&mut b, Scene::Quads, Twist::None).unwrap();
    assert!(
        first.blend == again.blend
            && first.depth_stencil == again.depth_stencil
            && first.raster == again.raster
            && first.vertex_layout == again.vertex_layout,
        "equal blocks made twice must be one block"
    );
    assert!(first.program != again.program);
    let t = draw::targets(&mut b, Some(wcontainers_gpu::Format::Depth32Float)).unwrap();
    let before = *b.counters();
    draw::begin(&mut b, &t, true);
    for _ in 0..10 {
        draw::draw(&mut b, &first);
    }
    // A second program with equal blocks and its own set: a second pipeline and bind group, but the
    // same bind group layout.
    draw::draw(&mut b, &again);
    b.end_pass();
    draw::begin(&mut b, &t, false);
    for _ in 0..10 {
        draw::draw(&mut b, &first);
    }
    b.end_pass();
    let rb = draw::read(&mut b, &t).unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
    b.finish_read(rb, &mut rgba);
    assert_eq!(b.status(), DeviceStatus::Ok);
    let c = *b.counters();
    eprintln!("before {before:?}, after {c:?}");
    assert_eq!(
        c.pipelines - before.pipelines,
        2,
        "one pipeline per program"
    );
    assert_eq!(
        c.bind_groups - before.bind_groups,
        2,
        "one bind group per set"
    );
    assert_eq!(
        c.group_layouts - before.group_layouts,
        1,
        "one layout for both programs' set"
    );
    assert_eq!(
        c.pipeline_binds - before.pipeline_binds,
        3,
        "setPipeline only when it changes"
    );
    assert_eq!(
        b.slow_paths()
            .count(wcontainers_gpu::SlowPath::PipelineMiss),
        2
    );
}
