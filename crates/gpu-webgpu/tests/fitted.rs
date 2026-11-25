//! A pass fitted by the shared pass tracker (`AttachmentsFitted`, plan 4.7) with multisampled colour
//! targets of different sizes, drawn on a real GPU: the larger target's common area is loaded into
//! and stored from a temporary sample by sample, and every sample outside the drawn area keeps its
//! value.
//!
//! Needs a GPU; `WEBGPU_API` as in tests/scenes.rs.

#[path = "../../graphics-test/src/scene.rs"]
#[allow(dead_code)]
mod scene;

#[allow(dead_code)]
#[path = "../examples/scenes/draw.rs"]
mod draw;

use wcontainers_gpu::{
    Gpu, BindingEntry, BindingResource, BufferRange, BufferUsage, ColorTarget, DeviceStatus,
    DrawCall, DrawRange, Extent, Features, Format, MAX_COLOR_TARGETS, Origin, PassTargets,
    PassTracker, Program, ReadSource, ScissorRect, ShaderModule, ShaderStage, SlowPath, StageSet,
    Target, TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc, Topology,
    ViewDimension, Viewport, WriteMode,
};
use wcontainers_gpu_webgpu::WebGpu;
use wcontainers_shaders::front::cut_scissors;

type B = WebGpu;

const PROGRAMS: &str = "
struct Params { color: vec4<f32> }
@group(0) @binding(0) var<uniform> params: Params;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
struct Two { @location(0) a: vec4<f32>, @location(1) b: vec4<f32> }
@fragment fn two() -> Two {
    return Two(params.color, params.color);
}
@fragment fn per_sample(@builtin(sample_index) i: u32) -> @location(0) vec4<f32> {
    return vec4<f32>(f32(i) / 3.0, 0.0, 0.0, 1.0);
}
@group(1) @binding(32) var samples: texture_multisampled_2d<f32>;
@fragment fn samples_of(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let c = vec2<i32>(p.xy);
    return vec4<f32>(
        textureLoad(samples, c, 0).r,
        textureLoad(samples, c, 1).r,
        textureLoad(samples, c, 2).r,
        textureLoad(samples, c, 3).r,
    );
}
";

fn device() -> B {
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

fn program(b: &mut B, fragment: &str) -> <B as Gpu>::Program {
    let shader = |b: &mut B, entry: &str, stage| {
        let module = naga::front::wgsl::parse_str(PROGRAMS).unwrap();
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::MULTISAMPLED_SHADING,
        )
        .validate(&module)
        .unwrap();
        b.create_shader(&ShaderModule {
            module,
            info,
            entry_point: entry.into(),
            stage,
            key: [0; 32],
        })
        .unwrap()
    };
    let vs = shader(b, "vs", ShaderStage::Vertex);
    let fs = shader(b, fragment, ShaderStage::Fragment);
    b.create_program(&Program::Render {
        vertex: &vs,
        fragment: Some(&fs),
    })
    .unwrap()
}

fn target(b: &mut B, width: u32, height: u32, samples: u32) -> Target<B> {
    let format = Format::Rgba8Unorm;
    let texture = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width,
            height,
            depth_or_layers: 1,
            mip_levels: 1,
            samples,
            usage: TextureUsage::RENDER
                | TextureUsage::COPY_SRC
                | TextureUsage::COPY_DST
                | TextureUsage::SAMPLE,
            cube: false,
            reinterpret: false,
        })
        .unwrap();
    let view = b
        .create_view(
            &texture,
            &TextureViewDesc {
                format,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .unwrap();
    Target {
        view,
        texture,
        format,
        mip: 0,
        layer: 0,
        depth_slice: 0,
        width,
        height,
        samples,
    }
}

fn colors(t: Target<B>) -> PassTargets<B> {
    let mut colors: [Option<ColorTarget<B>>; MAX_COLOR_TARGETS] = Default::default();
    colors[0] = Some(ColorTarget {
        target: t,
        resolve: None,
    });
    PassTargets {
        colors,
        depth: None,
        empty: None,
    }
}

struct Kit {
    blend: <B as Gpu>::Blend,
    depth_stencil: <B as Gpu>::DepthStencil,
    raster: <B as Gpu>::Raster,
    scissored: <B as Gpu>::Raster,
    layout: <B as Gpu>::VertexLayout,
}

/// Draws the covering triangle over `size`, cut to the area the tracker gave.
fn draw_in(
    b: &mut B,
    k: &Kit,
    size: (u32, u32),
    area: Option<ScissorRect>,
    program: &<B as Gpu>::Program,
    sets: [Option<&<B as Gpu>::BindingSet>; 4],
) {
    let mut scissors = Vec::new();
    if let Some(area) = area {
        cut_scissors(&[], area, &mut scissors);
    }
    b.draw(&DrawCall {
        program,
        blend: &k.blend,
        depth_stencil: &k.depth_stencil,
        raster: if area.is_some() {
            &k.scissored
        } else {
            &k.raster
        },
        vertex_layout: &k.layout,
        bindings: sets,
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[Viewport {
            x: 0,
            y: 0,
            width: (size.0 as f32).to_bits(),
            height: (size.1 as f32).to_bits(),
            min_depth: 0,
            max_depth: 1f32.to_bits(),
        }],
        scissors: &scissors,
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

/// A 3 by 3 four-sample target whose sample i holds i / 3, and a 2 by 2 one, drawn in one pass of
/// the smaller size by a draw covering its top row: the top row's two texels turn white in every
/// sample, every other sample of the larger target keeps its value.
#[test]
fn multisampled_targets_of_different_sizes_are_fitted() {
    let mut b = device();
    assert!(b.caps().features.contains(Features::MULTISAMPLE_READ));
    let k = Kit {
        blend: b.create_blend(&draw::opaque()).unwrap(),
        depth_stencil: b.create_depth_stencil(&draw::no_depth()).unwrap(),
        raster: b.create_raster(&draw::solid()).unwrap(),
        scissored: b
            .create_raster(&wcontainers_gpu::RasterDesc {
                scissor: true,
                ..draw::solid()
            })
            .unwrap(),
        layout: b.create_vertex_layout(&draw::layout(&[], &[])).unwrap(),
    };
    let uniforms = draw::buffer(&mut b, &[0; 32], BufferUsage::UNIFORM).unwrap();
    let white: Vec<u8> = [1.0f32; 4].iter().flat_map(|f| f.to_le_bytes()).collect();
    b.write_buffer(&uniforms, 0, &white, WriteMode::Ordered);
    let params = b
        .create_binding_set(&[BindingEntry {
            binding: 0,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::UniformBuffer(BufferRange {
                buffer: &uniforms,
                offset: 0,
                size: 16,
            }),
        }])
        .unwrap();
    let mut passes = PassTracker::default();
    let small = target(&mut b, 2, 2, 4);
    let large = target(&mut b, 3, 3, 4);
    let per_sample = program(&mut b, "per_sample");
    passes.set_targets(&mut b, colors(large.clone())).unwrap();
    let area = passes.before_draw(&mut b, &[]).unwrap();
    draw_in(
        &mut b,
        &k,
        (3, 3),
        area,
        &per_sample,
        [Some(&params), None, None, None],
    );

    let mut targets = colors(small.clone());
    targets.colors[1] = Some(ColorTarget {
        target: large.clone(),
        resolve: None,
    });
    passes.set_targets(&mut b, targets).unwrap();
    let two = program(&mut b, "two");
    let area = passes.before_draw(&mut b, &[]).unwrap();
    assert_eq!(area.map(|a| (a.width, a.height)), Some((2, 2)));
    let top = area.map(|a| ScissorRect { height: 1, ..a });
    draw_in(
        &mut b,
        &k,
        (2, 2),
        top,
        &two,
        [Some(&params), None, None, None],
    );
    passes.close(&mut b).unwrap();
    assert_eq!(b.slow_paths().count(SlowPath::AttachmentsFitted), 1);
    assert_eq!(b.slow_paths().count(SlowPath::MultisampleCopyResolved), 0);

    let check = target(&mut b, 3, 3, 1);
    let read = b
        .create_binding_set(&[BindingEntry {
            binding: 32,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&large.view),
        }])
        .unwrap();
    let samples_of = program(&mut b, "samples_of");
    passes.set_targets(&mut b, colors(check.clone())).unwrap();
    let area = passes.before_draw(&mut b, &[]).unwrap();
    draw_in(
        &mut b,
        &k,
        (3, 3),
        area,
        &samples_of,
        [Some(&params), Some(&read), None, None],
    );
    passes.close(&mut b).unwrap();
    let rb = b
        .read_back(&ReadSource::Texture {
            region: TextureRegion {
                texture: &check.texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            extent: Extent {
                width: 3,
                height: 3,
                depth_or_layers: 1,
            },
        })
        .unwrap();
    b.submit();
    b.wait_read(&rb);
    let mut got = vec![0; 36];
    b.finish_read(rb, &mut got);
    assert_eq!(b.status(), DeviceStatus::Ok);
    for y in 0..3 {
        for x in 0..3 {
            let at = (y * 3 + x) * 4;
            let texel = &got[at..at + 4];
            if x < 2 && y == 0 {
                assert_eq!(texel, [255; 4], "drawn at ({x}, {y})");
            } else {
                assert_eq!(texel, [0, 85, 170, 255], "kept at ({x}, {y})");
            }
        }
    }
}
