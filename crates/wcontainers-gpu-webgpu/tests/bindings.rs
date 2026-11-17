//! Bind groups as plan section 4.5 asks: constants rewritten before every draw make no bind group
//! per draw, sets differing only in a dynamic offset share one, and blending into float targets is
//! done or refused, never dropped. Every picture is checked.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../wcontainers-graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use wcontainers_gpu::{
    Backend, BindingEntry, BindingResource, BlendComponent, BlendDesc, BlendFactor, BlendOp,
    BufferDesc, BufferRange, BufferUsage, ColorAttachment, DeviceStatus, DrawCall, DrawRange,
    Extent, Format, FormatUse, LoadOp, MapIntent, Origin, PassDesc, ReadSource, StageSet, StoreOp,
    TextureRegion, TextureUsage, Topology, Viewport, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;

/// A full-target triangle whose colour is the constant buffer's first vector.
const CONSTANT_COLOR_WGSL: &str = r#"
struct C { color: vec4f }
@group(1) @binding(0) var<uniform> c: C;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps() -> @location(0) vec4f { return c.color; }
"#;

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

/// What every draw here shares but its set and viewport.
struct Kit {
    program: <WebGpu as Backend>::Program,
    blend: <WebGpu as Backend>::Blend,
    depth_stencil: <WebGpu as Backend>::DepthStencil,
    raster: <WebGpu as Backend>::Raster,
    vertex_layout: <WebGpu as Backend>::VertexLayout,
    target: <WebGpu as Backend>::Texture,
    view: <WebGpu as Backend>::TextureView,
    width: u32,
}

fn kit(b: &mut WebGpu, format: Format, width: u32, blend: &BlendDesc) -> Kit {
    let (target, view) = draw::texture_2d(
        b,
        format,
        width,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    Kit {
        program: draw::program(b, CONSTANT_COLOR_WGSL).unwrap(),
        blend: b.create_blend(blend).unwrap(),
        depth_stencil: b.create_depth_stencil(&draw::no_depth()).unwrap(),
        raster: b.create_raster(&draw::solid()).unwrap(),
        vertex_layout: b.create_vertex_layout(&draw::layout(&[], &[])).unwrap(),
        target,
        view,
        width,
    }
}

fn begin(b: &mut WebGpu, k: &Kit, clear: [f32; 4]) {
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &k.view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(clear.map(f32::to_bits)),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
}

/// Draws the triangle with `set` into pixel `x` alone, or the whole target.
fn draw_at(b: &mut WebGpu, k: &Kit, set: &<WebGpu as Backend>::BindingSet, x: Option<u32>) {
    let (x, width) = x.map_or((0.0, k.width as f32), |x| (x as f32, 1.0));
    let viewport = [Viewport {
        x: x.to_bits(),
        y: 0,
        width: width.to_bits(),
        height: 1.0f32.to_bits(),
        min_depth: 0,
        max_depth: 1.0f32.to_bits(),
    }];
    b.draw(&DrawCall {
        program: &k.program,
        blend: &k.blend,
        depth_stencil: &k.depth_stencil,
        raster: &k.raster,
        vertex_layout: &k.vertex_layout,
        bindings: [None, Some(set), None, None],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &viewport,
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 3,
            first_instance: 0,
            instances: 1,
        },
    });
}

/// Ends the pass, reads the target back and waits for it.
fn finish(b: &mut WebGpu, k: &Kit, texel: u32) -> Vec<u8> {
    b.end_pass();
    let rb = b
        .read_back(&ReadSource::Texture {
            region: TextureRegion {
                texture: &k.target,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: k.width,
                height: 1,
                depth_or_layers: 1,
            },
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut out = vec![0; (k.width * texel) as usize];
    b.finish_read(rb, &mut out);
    out
}

fn constants(b: &mut WebGpu, size: u64, intent: MapIntent) -> <WebGpu as Backend>::Buffer {
    b.create_buffer(&BufferDesc {
        size,
        usage: BufferUsage::UNIFORM,
        intent,
    })
    .unwrap()
}

fn set(
    b: &mut WebGpu,
    buffer: &<WebGpu as Backend>::Buffer,
    offset: u64,
) -> <WebGpu as Backend>::BindingSet {
    b.create_binding_set(&[BindingEntry {
        binding: 0,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::UniformBuffer(BufferRange {
            buffer,
            offset,
            size: 16,
        }),
    }])
    .unwrap()
}

fn color_of(i: u32) -> [u8; 4] {
    [i as u8 * 10, 255 - i as u8 * 10, 0x40, 255]
}

fn unorm(c: [u8; 4]) -> Vec<u8> {
    c.iter()
        .flat_map(|v| (f32::from(*v) / 255.0).to_le_bytes())
        .collect()
}

/// Twenty draws, each after a `Discard` of the constant buffer they read (Direct3D 11's
/// `Map(WRITE_DISCARD)` per draw): one bind group, no renames, and each pixel its own draw's colour.
#[test]
fn constants_rewritten_per_draw_make_no_bind_group_per_draw() {
    const DRAWS: u32 = 20;
    let mut b = device();
    let k = kit(&mut b, Format::Rgba8Unorm, DRAWS, &draw::opaque());
    let ub = constants(&mut b, 16, MapIntent::Often);
    let s = set(&mut b, &ub, 0);
    let before = *b.counters();
    begin(&mut b, &k, [0.0; 4]);
    for i in 0..DRAWS {
        b.write_buffer(&ub, 0, &unorm(color_of(i)), WriteMode::Discard);
        draw_at(&mut b, &k, &s, Some(i));
    }
    let got = finish(&mut b, &k, 4);
    assert_eq!(b.status(), DeviceStatus::Ok);
    let c = *b.counters();
    eprintln!("before {before:?}, after {c:?}");
    let want: Vec<u8> = (0..DRAWS).flat_map(color_of).collect();
    assert_eq!(got, want, "a draw read another draw's constants");
    assert_eq!(c.bind_groups - before.bind_groups, 1, "one bind group");
    assert_eq!(c.renames - before.renames, 0, "no renames");
    assert_eq!(
        c.slices - before.slices,
        u64::from(DRAWS - 1),
        "every write after the first draw moves to a new slice"
    );
}

/// Two sets on one buffer at offsets 0 and 256 share one bind group; each draw reads its own range.
#[test]
fn sets_differing_only_in_a_dynamic_offset_share_a_bind_group() {
    let mut b = device();
    let k = kit(&mut b, Format::Rgba8Unorm, 2, &draw::opaque());
    let ub = constants(&mut b, 512, MapIntent::Rarely);
    b.write_buffer(&ub, 0, &unorm(color_of(3)), WriteMode::Ordered);
    b.write_buffer(&ub, 256, &unorm(color_of(7)), WriteMode::Ordered);
    let (s0, s1) = (set(&mut b, &ub, 0), set(&mut b, &ub, 256));
    assert!(s0 != s1);
    let before = *b.counters();
    begin(&mut b, &k, [0.0; 4]);
    draw_at(&mut b, &k, &s0, Some(0));
    draw_at(&mut b, &k, &s1, Some(1));
    let got = finish(&mut b, &k, 4);
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(got, [color_of(3), color_of(7)].concat());
    assert_eq!(b.counters().bind_groups - before.bind_groups, 1);
}

/// Blending into a 32-bit float target follows Direct3D's rule where the device can blend it, and
/// otherwise fails loudly; it is never left out.
#[test]
fn float_targets_blend_or_fail_loudly() {
    let mut b = device();
    let mut blend = draw::opaque();
    blend.targets[0].blend = Some((
        BlendComponent {
            src: BlendFactor::SrcAlpha,
            dst: BlendFactor::OneMinusSrcAlpha,
            op: BlendOp::Add,
        },
        BlendComponent {
            src: BlendFactor::One,
            dst: BlendFactor::Zero,
            op: BlendOp::Add,
        },
    ));
    let blendable = b
        .caps()
        .formats
        .get(Format::Rgba32Float)
        .contains(FormatUse::BLEND);
    eprintln!("rgba32float blendable on this device: {blendable}");
    let k = kit(&mut b, Format::Rgba32Float, 1, &blend);
    let ub = constants(&mut b, 16, MapIntent::Rarely);
    let src: Vec<u8> = [0.5f32, 0.25, 1.0, 0.5]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    b.write_buffer(&ub, 0, &src, WriteMode::Ordered);
    let s = set(&mut b, &ub, 0);
    begin(&mut b, &k, [1.0, 0.0, 0.0, 1.0]);
    draw_at(&mut b, &k, &s, None);
    if !blendable {
        b.end_pass();
        assert!(matches!(b.status(), DeviceStatus::Lost(_)));
        return;
    }
    let got: Vec<f32> = finish(&mut b, &k, 16)
        .chunks(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(got, [0.75, 0.125, 0.5, 0.5]);
}
