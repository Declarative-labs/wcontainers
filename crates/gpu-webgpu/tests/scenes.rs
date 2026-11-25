//! The rig's three Direct3D 11 reference scenes drawn through the back end on a real GPU and compared
//! with real Direct3D's pictures; changes that must fail are seen to fail; the caches are counted.
//!
//! Needs a GPU. `WEBGPU_API` picks wgpu's API (dx12, vulkan, metal); the default is the platform's.

#[path = "../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

mod allowance;

use draw::Twist;
use scene::Scene;
use wcontainers_gpu::{Backend, DeviceStatus};
use wcontainers_gpu_webgpu::WebGpu;

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
        "{}/../graphics-test/references/d3d11/{}.png",
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

/// Each scene within the rig's allowance of Direct3D's picture (on the RTX 3080 every pixel equal).
#[test]
fn scenes_match_direct3d() {
    let mut b = device();
    let mut failed = Vec::new();
    for which in Scene::ALL {
        let (drawn, expected) = (render(&mut b, which, Twist::None), reference(which));
        if !allowance::within(&drawn, &expected, &format!("{}.png", which.name())) {
            failed.push(which.name());
        }
    }
    assert!(failed.is_empty(), "differ from Direct3D: {failed:?}");
}

#[test]
fn changed_blend_factor_and_depth_function_fail() {
    let mut b = device();
    let reference = reference(Scene::Quads);
    let (tolerance, pixels) = allowance::allowance("quads.png");
    for twist in [Twist::BlendFactor, Twist::DepthFunction] {
        let (beyond, worst) =
            allowance::beyond(&render(&mut b, Scene::Quads, twist), &reference, tolerance);
        eprintln!(
            "quads with {twist:?}: {beyond} pixels beyond {tolerance}, largest difference {worst}"
        );
        assert!(beyond > pixels, "{twist:?} drew the reference picture");
    }
}

/// The quads drawn with each depth format of at least 24 bits the device holds, stencil cleared too
/// where the format has one, within the rig's allowance of Direct3D's picture.
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
        eprint!("on {format:?}: ");
        assert!(
            allowance::within(&rgba, &reference, "quads.png"),
            "quads on {format:?} differ from Direct3D"
        );
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

/// Destroying a program or the last of a block's creations frees the pipelines and layouts made
/// with it; a block made twice keeps them until both are destroyed; the scene drawn again after
/// is still Direct3D's.
#[test]
fn destroyed_programs_and_blocks_free_their_pipelines() {
    use wcontainers_gpu::Object;
    let mut b = device();
    let t = draw::targets(&mut b, None).unwrap();
    let frame = |b: &mut WebGpu, p: &draw::Prepared| {
        draw::begin(b, &t, true);
        draw::draw(b, p);
        b.end_pass();
        let rb = draw::read(b, &t).unwrap();
        b.submit();
        b.wait_read(&rb);
        let mut rgba = vec![0; (scene::WIDTH * scene::HEIGHT * 4) as usize];
        b.finish_read(rb, &mut rgba);
        assert_eq!(b.status(), DeviceStatus::Ok);
        rgba
    };
    let base = b.held();
    let p = draw::prepare(&mut b, Scene::Triangle, Twist::None).unwrap();
    frame(&mut b, &p);
    let drawn = b.held();
    assert!(drawn.0 > base.0 && drawn.1 > base.1, "{base:?} {drawn:?}");
    // An equal blend made again is the same block; destroying one creation keeps the pipeline.
    let again = b.create_blend(&draw::opaque()).unwrap();
    b.destroy(Object::Blend(again));
    assert_eq!(b.held(), drawn);
    b.destroy(Object::Blend(p.blend.clone()));
    let after_blend = b.held();
    assert_eq!(
        after_blend.0, base.0,
        "pipelines of the destroyed blend kept"
    );
    b.destroy(Object::Program(p.program.clone()));
    assert_eq!(b.held(), base, "layouts of the destroyed program kept");
    for o in [
        Object::DepthStencil(p.depth_stencil.clone()),
        Object::Raster(p.raster.clone()),
        Object::VertexLayout(p.vertex_layout.clone()),
    ] {
        b.destroy(o);
    }
    let q = draw::prepare(&mut b, Scene::Triangle, Twist::None).unwrap();
    let rgba = frame(&mut b, &q);
    assert!(
        allowance::within(&rgba, &reference(Scene::Triangle), "triangle.png"),
        "the triangle drawn after the destroys differs"
    );
    assert_eq!(b.held(), drawn);
}

/// The triangle with its colour from immediates the fragment stage declares, where the device has
/// them (`immediate_bytes` above zero): each draw shows the colour it set, over the triangle's
/// pixels in Direct3D's picture.
#[test]
fn immediates_reach_the_program() {
    const WGSL: &str = "
struct Colour { rgb: vec3f, scale: f32 }
var<immediate> colour: Colour;
struct VsOut { @builtin(position) pos: vec4f }
@vertex fn vs(@location(0) pos: vec2f, @location(1) c: vec3f) -> VsOut {
    return VsOut(vec4f(pos, 0.0, 1.0));
}
@fragment fn ps(i: VsOut) -> @location(0) vec4f {
    return vec4f(colour.rgb * colour.scale, 1.0);
}";
    let mut b = device();
    let limit = b.caps().limits.immediate_bytes;
    eprintln!("immediate_bytes: {limit}");
    if limit < 16 {
        eprintln!("immediates absent on this device: nothing to draw");
        return;
    }
    let mut p = draw::prepare(&mut b, Scene::Triangle, Twist::None).unwrap();
    // The example's helper validates with WebGPU's core capabilities; immediates need their own.
    let module = naga::front::wgsl::parse_str(WGSL).unwrap();
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::IMMEDIATES,
    )
    .validate(&module)
    .unwrap();
    let mut stage = |entry: &str, stage| {
        b.create_shader(&wcontainers_gpu::ShaderModule {
            module: module.clone(),
            info: info.clone(),
            entry_point: entry.into(),
            stage,
            key: [0; 32],
        })
        .unwrap()
    };
    let (vs, ps) = (
        stage("vs", wcontainers_gpu::ShaderStage::Vertex),
        stage("ps", wcontainers_gpu::ShaderStage::Fragment),
    );
    p.program = b
        .create_program(&wcontainers_gpu::Program::Render {
            vertex: &vs,
            fragment: Some(&ps),
        })
        .unwrap();
    let t = draw::targets(&mut b, None).unwrap();
    let want = reference(Scene::Triangle);
    let clear = want[..4].to_vec();
    for (rgb, scale, shown) in [
        ([0.2f32, 0.4, 0.6], 1.0f32, [51u8, 102, 153, 255]),
        ([1.0, 0.0, 0.5], 0.4, [102, 0, 51, 255]),
    ] {
        let bytes: Vec<u8> = rgb
            .iter()
            .chain([&scale])
            .flat_map(|v| v.to_le_bytes())
            .collect();
        draw::begin(&mut b, &t, true);
        draw::draw_with(&mut b, &p, &bytes);
        b.end_pass();
        let rb = draw::read(&mut b, &t).unwrap();
        b.submit();
        b.wait_read(&rb);
        let mut rgba = vec![0; want.len()];
        b.finish_read(rb, &mut rgba);
        assert_eq!(b.status(), DeviceStatus::Ok);
        let wrong = rgba
            .chunks(4)
            .zip(want.chunks(4))
            .filter(|(got, d3d)| {
                let inside = *d3d != &clear[..];
                *got != if inside { &shown[..] } else { &clear[..] }
            })
            .count();
        assert_eq!(wrong, 0, "pixels not the immediates' colour or the clear");
    }
}

/// A resolve averages, so no integer format is declared resolvable, whatever wgpu's adapter flags
/// allow (wgpu's Vulkan flags every integer colour format resolvable on the RTX 3080).
#[test]
fn integer_formats_are_not_resolvable() {
    use Format::*;
    use wcontainers_gpu::{Format, FormatKind, FormatUse};
    let b = device();
    let integer = [
        R8Uint,
        R8Sint,
        R16Uint,
        R16Sint,
        Rg8Uint,
        Rg8Sint,
        R32Uint,
        R32Sint,
        Rg16Uint,
        Rg16Sint,
        Rgba8Uint,
        Rgba8Sint,
        Rgb10a2Uint,
        Rg32Uint,
        Rg32Sint,
        Rgba16Uint,
        Rgba16Sint,
        Rgb32Uint,
        Rgb32Sint,
        Rgba32Uint,
        Rgba32Sint,
    ];
    let resolvable: Vec<Format> = integer
        .into_iter()
        .inspect(|f| assert!(matches!(f.kind(), FormatKind::Uint | FormatKind::Sint)))
        .filter(|&f| b.caps().formats.get(f).contains(FormatUse::RESOLVE))
        .collect();
    assert!(resolvable.is_empty(), "declared resolvable: {resolvable:?}");
    assert!(
        b.caps()
            .formats
            .get(Rgba8Unorm)
            .contains(FormatUse::RESOLVE)
    );
}
