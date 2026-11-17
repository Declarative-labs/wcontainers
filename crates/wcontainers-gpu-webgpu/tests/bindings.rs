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
    AddressMode, AspectOps, Backend, BindingEntry, BindingResource, BlendComponent, BlendDesc,
    BlendFactor, BlendOp, BorderColor, BufferDesc, BufferRange, BufferUsage, ColorAttachment,
    CompareFn, DepthAttachment, DepthStencilDesc, DeviceStatus, DrawCall, DrawRange, Extent,
    Filter, Format, FormatUse, LoadOp, MapIntent, Origin, PassDesc, Program, ReadSource,
    SamplerDesc, ShaderStage, StageSet, StoreOp, TextureRegion, TextureUsage, TextureViewDesc,
    Topology, ViewDimension, Viewport, WriteMode,
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

/// A full-target triangle whose red is texel (0, 0) of a texture it only loads from.
const LOAD_WGSL: &str = r#"
@group(1) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps() -> @location(0) vec4f {
    return vec4f(textureLoad(t, vec2i(0), 0).r, 0.0, 0.0, 1.0);
}
"#;

/// A texture the program only loads from is unfilterable whatever is bound (plan 4.5): drawing it
/// with a filterable view and then a depth view, which never filters, keeps one pipeline.
#[test]
fn load_only_textures_keep_one_pipeline_for_any_view() {
    let mut b = device();
    let k = Kit {
        program: draw::program(&mut b, LOAD_WGSL).unwrap(),
        ..kit(&mut b, Format::Rgba8Unorm, 2, &draw::opaque())
    };
    let (color, color_view) = draw::texture_2d(
        &mut b,
        Format::Rgba8Unorm,
        1,
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    )
    .unwrap();
    b.write_texture(
        &TextureRegion {
            texture: &color,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: 1,
            height: 1,
            depth_or_layers: 1,
        },
        &[200, 20, 30, 255],
        4,
        1,
        WriteMode::Ordered,
    );
    let (_, depth_view) =
        draw::texture_2d(&mut b, Format::Depth32Float, 1, 1, TextureUsage::SAMPLE).unwrap();
    let sets: Vec<_> = [&color_view, &depth_view]
        .into_iter()
        .map(|v| {
            b.create_binding_set(&[BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(v),
            }])
            .unwrap()
        })
        .collect();
    let keys: Vec<_> = sets.iter().map(|s| b.binding_layout_key(s)).collect();
    assert!(
        keys[0] != keys[1],
        "the depth view's key marks it unfilterable"
    );
    let before = *b.counters();
    begin(&mut b, &k, [0.0, 1.0, 0.0, 1.0]);
    draw_at(&mut b, &k, &sets[0], Some(0));
    draw_at(&mut b, &k, &sets[1], Some(1));
    let got = finish(&mut b, &k, 4);
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(got, [200, 0, 0, 255, 0, 0, 0, 255]);
    assert_eq!(b.counters().pipelines - before.pipelines, 1);
}

const ASPECTS_WGSL: &str = r#"
@group(1) @binding(32) var d: texture_2d<f32>;
@group(1) @binding(33) var s: texture_2d<u32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps() -> @location(0) vec4f {
    let stencil = f32(textureLoad(s, vec2i(0), 0).r) / 255.0;
    return vec4f(textureLoad(d, vec2i(0), 0).r, stencil, 0.0, 1.0);
}
"#;

/// A texture with depth and stencil is drawn into through a view, and then that view and a stencil
/// view of it are read by a program: each binding sees its own aspect.
#[test]
fn combined_depth_stencil_views_draw_and_read_each_aspect() {
    let mut b = device();
    let mut ran = 0;
    for format in [Format::Depth24PlusStencil8, Format::Depth32FloatStencil8] {
        if !b
            .caps()
            .formats
            .get(format)
            .contains(FormatUse::DEPTH_STENCIL | FormatUse::SAMPLE)
        {
            eprintln!("{format:?}: not held by this device");
            continue;
        }
        let (texture, depth_view) = draw::texture_2d(
            &mut b,
            format,
            1,
            1,
            TextureUsage::RENDER | TextureUsage::SAMPLE,
        )
        .unwrap();
        let stencil_view = b
            .create_view(
                &texture,
                &TextureViewDesc {
                    format,
                    dimension: ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: 1,
                    stencil: true,
                },
            )
            .unwrap();
        b.begin_pass(&PassDesc {
            colors: Default::default(),
            depth: Some(DepthAttachment {
                view: &depth_view,
                depth: Some(AspectOps {
                    load: LoadOp::Clear(0.25f32.to_bits()),
                    store: StoreOp::Store,
                    read_only: false,
                }),
                stencil: Some(AspectOps {
                    load: LoadOp::Clear(7),
                    store: StoreOp::Store,
                    read_only: false,
                }),
            }),
            empty: None,
            occlusion: None,
        });
        b.end_pass();
        let k = Kit {
            program: draw::program(&mut b, ASPECTS_WGSL).unwrap(),
            ..kit(&mut b, Format::Rgba8Unorm, 1, &draw::opaque())
        };
        let set = b
            .create_binding_set(&[
                BindingEntry {
                    binding: 32,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(&depth_view),
                },
                BindingEntry {
                    binding: 33,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::Texture(&stencil_view),
                },
            ])
            .unwrap();
        begin(&mut b, &k, [0.0; 4]);
        draw_at(&mut b, &k, &set, None);
        let got = finish(&mut b, &k, 4);
        assert_eq!(b.status(), DeviceStatus::Ok, "{format:?}");
        // Depth 0.25 is 63.75 of 255, stored as 64.
        assert_eq!(got, [64, 7, 0, 255], "{format:?}");
        ran += 1;
    }
    assert!(ran > 0, "every device holds depth24plus-stencil8");
}

/// A vertex stage alone, moving the triangle by its constants: depth 0.5 with no fragment stage.
const VERTEX_ONLY_WGSL: &str = r#"
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.5, 1.0);
}
"#;

/// Each stage's helper constants in set 3 (plan 6.4: binding 0 the vertex stage's, 12 the
/// fragment stage's): the vertex block moves the triangle onto the right pixel, the fragment
/// block colours it.
const HELPERS_WGSL: &str = r#"
struct V { shift: vec4f }
struct F { color: vec4f }
@group(3) @binding(0) var<uniform> v: V;
@group(3) @binding(12) var<uniform> f: F;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0 + v.shift.xy, 0.0, 1.0);
}
@fragment fn ps() -> @location(0) vec4f { return f.color; }
"#;

fn draw_call<'a>(
    k: &'a Kit,
    bindings: [Option<&'a <WebGpu as Backend>::BindingSet>; 4],
    viewport: &'a [Viewport],
) -> DrawCall<'a, WebGpu> {
    DrawCall {
        program: &k.program,
        blend: &k.blend,
        depth_stencil: &k.depth_stencil,
        raster: &k.raster,
        vertex_layout: &k.vertex_layout,
        bindings,
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: viewport,
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
    }
}

fn whole(width: u32) -> [Viewport; 1] {
    [Viewport {
        x: 0,
        y: 0,
        width: (width as f32).to_bits(),
        height: 1.0f32.to_bits(),
        min_depth: 0,
        max_depth: 1.0f32.to_bits(),
    }]
}

/// A program with no fragment stage drawn into a colour and a depth target: the colour keeps its
/// clear and the depth test and write happen.
#[test]
fn programs_without_a_fragment_stage_write_only_depth() {
    let mut b = device();
    let vs = draw::shader(&mut b, VERTEX_ONLY_WGSL, "vs", ShaderStage::Vertex).unwrap();
    let program = b
        .create_program(&Program::Render {
            vertex: &vs,
            fragment: None,
        })
        .unwrap();
    let depth_stencil = b
        .create_depth_stencil(&DepthStencilDesc {
            depth_compare: Some(CompareFn::Less),
            depth_write: true,
            ..draw::no_depth()
        })
        .unwrap();
    let k = Kit {
        program,
        depth_stencil,
        ..kit(&mut b, Format::Rgba8Unorm, 1, &draw::opaque())
    };
    let (depth, depth_view) = draw::texture_2d(
        &mut b,
        Format::Depth32Float,
        1,
        1,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
    )
    .unwrap();
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: Some(DepthAttachment {
            view: &depth_view,
            depth: Some(AspectOps {
                load: LoadOp::Clear(1.0f32.to_bits()),
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: None,
        }),
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: &k.view,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear([0.2f32, 0.4, 0.6, 1.0].map(f32::to_bits)),
        store: StoreOp::Store,
    });
    b.begin_pass(&pass);
    b.draw(&draw_call(&k, [None; 4], &whole(1)));
    let color = finish(&mut b, &k, 4);
    let rb = b
        .read_back(&ReadSource::Texture {
            region: TextureRegion {
                texture: &depth,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: 1,
                height: 1,
                depth_or_layers: 1,
            },
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut got = [0u8; 4];
    b.finish_read(rb, &mut got);
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(color, [51, 102, 153, 255]);
    assert_eq!(f32::from_le_bytes(got), 0.5);
}

/// One draw's vertex and fragment helper blocks, two entries of set 3, each read by its own stage.
#[test]
fn helper_blocks_of_each_stage_read_their_own_values() {
    let mut b = device();
    let k = Kit {
        program: draw::program(&mut b, HELPERS_WGSL).unwrap(),
        ..kit(&mut b, Format::Rgba8Unorm, 2, &draw::opaque())
    };
    let vertex = constants(&mut b, 16, MapIntent::Often);
    let fragment = constants(&mut b, 16, MapIntent::Often);
    // Clip space moves right by one: only the right pixel is covered.
    b.write_buffer(&vertex, 0, &unorm([255, 0, 0, 0]), WriteMode::Discard);
    b.write_buffer(&fragment, 0, &unorm([10, 20, 30, 255]), WriteMode::Discard);
    let block = |buffer| BufferRange {
        buffer,
        offset: 0,
        size: 16,
    };
    let helpers = b
        .create_binding_set(&[
            BindingEntry {
                binding: 0,
                stages: StageSet::VERTEX,
                resource: BindingResource::UniformBuffer(block(&vertex)),
            },
            BindingEntry {
                binding: 12,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::UniformBuffer(block(&fragment)),
            },
        ])
        .unwrap();
    begin(&mut b, &k, [0.0; 4]);
    b.draw(&draw_call(
        &k,
        [None, None, None, Some(&helpers)],
        &whole(2),
    ));
    let got = finish(&mut b, &k, 4);
    assert_eq!(b.status(), DeviceStatus::Ok);
    assert_eq!(got, [0, 0, 0, 0, 10, 20, 30, 255]);
}

/// Samples a 2 by 1 texture between its two texels: 0.5 filtered linearly, the right texel's value
/// at a point, written to an `r32float` target.
const SAMPLE_MIDDLE_WGSL: &str = r#"
@group(1) @binding(16) var s: sampler;
@group(1) @binding(32) var t: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn ps() -> @location(0) vec4f {
    return vec4f(textureSampleLevel(t, s, vec2f(0.5, 0.5), 0.0).x, 0.0, 0.0, 1.0);
}
"#;

fn sampler(b: &mut WebGpu, filter: Filter) -> <WebGpu as Backend>::Sampler {
    b.create_sampler(&SamplerDesc {
        min: filter,
        mag: filter,
        mip: Filter::Nearest,
        address: [AddressMode::Clamp; 3],
        lod_range: [0, f32::MAX.to_bits()],
        anisotropy: 1,
        compare: None,
        border: BorderColor::TransparentBlack,
    })
    .unwrap()
}

/// A 2 by 1 texture of `format` holding 0 and 1 (`depth32float` written through the depth draw).
fn zero_one(b: &mut WebGpu, format: Format) -> <WebGpu as Backend>::TextureView {
    let (texture, view) = draw::texture_2d(
        b,
        format,
        2,
        1,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
    )
    .unwrap();
    let bytes: Vec<u8> = match format {
        Format::Rgba8Unorm => vec![0, 0, 0, 0, 255, 255, 255, 255],
        _ => [0.0f32, 1.0].iter().flat_map(|v| v.to_le_bytes()).collect(),
    };
    b.write_texture(
        &TextureRegion {
            texture: &texture,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        },
        Extent {
            width: 2,
            height: 1,
            depth_or_layers: 1,
        },
        &bytes,
        bytes.len() as u32,
        1,
        WriteMode::Ordered,
    );
    view
}

/// Draws the middle sample of `view` through `sampler` and returns it.
fn sample_middle(
    b: &mut WebGpu,
    k: &Kit,
    view: &<WebGpu as Backend>::TextureView,
    sampler: &<WebGpu as Backend>::Sampler,
) -> f32 {
    let set = b
        .create_binding_set(&[
            BindingEntry {
                binding: 16,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Sampler(sampler),
            },
            BindingEntry {
                binding: 32,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(view),
            },
        ])
        .unwrap();
    begin(b, k, [0.0; 4]);
    b.draw(&draw_call(k, [None, Some(&set), None, None], &whole(1)));
    f32::from_le_bytes(finish(b, k, 4).try_into().unwrap())
}

/// Linear filtering halfway between 0 and 1, within the filter weights' precision (Direct3D asks
/// for at least 8 fractional bits; the RTX 3080 gives 0.5000076).
fn halfway(v: f32) -> bool {
    (v - 0.5).abs() < 1.0 / 256.0
}

/// Layouts follow plan 4.5: a filterable view takes a filtering sampler slot, which also takes a
/// point sampler, so one program bound with a point and then a linear sampler is one pipeline; a
/// depth view and an `r32float` view on a device made without `float32-filterable` take
/// unfilterable slots and are sampled at points, and refuse a linear sampler loudly; with
/// `float32-filterable`, `r32float` filters.
#[test]
fn layouts_follow_each_views_filterability() {
    for float32_filterable in [true, false] {
        let without = if float32_filterable {
            wgpu::Features::empty()
        } else {
            // As in a browser: natively the adapter's own format features would filter anyway.
            wgpu::Features::FLOAT32_FILTERABLE
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
        };
        let backends = match std::env::var("WEBGPU_API").as_deref() {
            Ok("dx12") => wgpu::Backends::DX12,
            Ok("vulkan") => wgpu::Backends::VULKAN,
            Ok("metal") => wgpu::Backends::METAL,
            _ => wgpu::Backends::PRIMARY,
        };
        let mut b = pollster::block_on(draw::device_without(&draw::instance(backends), without))
            .unwrap()
            .0;
        let filters = b
            .caps()
            .formats
            .get(Format::R32Float)
            .contains(FormatUse::FILTER);
        if float32_filterable && !filters {
            eprintln!("this adapter has no float32-filterable");
            continue;
        }
        assert_eq!(filters, float32_filterable);
        let k = Kit {
            program: draw::program(&mut b, SAMPLE_MIDDLE_WGSL).unwrap(),
            ..kit(&mut b, Format::R32Float, 1, &draw::opaque())
        };
        let (point, linear) = (
            sampler(&mut b, Filter::Nearest),
            sampler(&mut b, Filter::Linear),
        );
        let rgba8 = zero_one(&mut b, Format::Rgba8Unorm);
        let before = b.counters().pipelines;
        assert_eq!(sample_middle(&mut b, &k, &rgba8, &point), 1.0);
        assert!(halfway(sample_middle(&mut b, &k, &rgba8, &linear)));
        assert_eq!(
            b.counters().pipelines - before,
            1,
            "point then linear: one pipeline"
        );
        for format in [Format::R32Float, Format::Depth32Float] {
            let view = zero_one(&mut b, format);
            assert_eq!(sample_middle(&mut b, &k, &view, &point), 1.0, "{format:?}");
            assert_eq!(b.status(), DeviceStatus::Ok, "{format:?}");
        }
        let r32 = zero_one(&mut b, Format::R32Float);
        if filters {
            assert!(halfway(sample_middle(&mut b, &k, &r32, &linear)));
            assert_eq!(b.status(), DeviceStatus::Ok);
        } else {
            sample_middle(&mut b, &k, &r32, &linear);
            assert_ne!(
                b.status(),
                DeviceStatus::Ok,
                "a linear sampler on an unfilterable view"
            );
        }
    }
}
