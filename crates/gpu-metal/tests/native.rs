//! What this implementation does beyond the shared suite: the refusals that keep Metal from stopping the process, the
//! features it declares that WebGPU on Metal lacks, and its fast paths, each against the recording back end or against
//! values worked out by hand where the recording back end lacks the feature.

#![cfg(target_os = "macos")]

use wcontainers_gpu::{
    AddressMode, AspectOps, BindingEntry, BindingResource, BlendComponent, BlendDesc, BlendFactor,
    BlendOp, BorderColor, BufferDesc, BufferUsage, ColorAttachment, ColorWrites, CompareFn,
    CullMode, DepthAttachment, DepthStencilDesc, DrawCall, DrawRange, Extent, Features, FillMode,
    Filter, Format, FrontFace, Gpu, GpuError, LoadOp, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS,
    MapIntent, Origin, PassDesc, Program, ProvokingVertex, QueryKind, RasterDesc, ReadSource,
    RenderTargetBlend, RingKind, SamplerDesc, ShaderModule, ShaderStage, StageSet, StencilFace,
    StencilOp, StoreOp, TextureDesc, TextureDimension, TextureRegion, TextureUsage,
    TextureViewDesc, Topology, VertexLayoutDesc, ViewDimension, Viewport, WriteMode,
};
use wcontainers_gpu_metal::MetalGpu;
use wcontainers_gpu_test::Recorder;
use wcontainers_metal::StandaloneMetal;
use wcontainers_metal_direct::DirectMetal;

/// A side longer than any Apple GPU's longest texture side, which is 32768.
///
/// https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf
const BEYOND_ANY_SIDE: u32 = 40_000;

/// The side of the square targets the tests draw into.
const SIDE: u32 = 4;

/// The bytes of one `rgba8` texel.
const RGBA8_BYTES: usize = 4;

/// The tests' programs: a covering triangle or two halves, flat or uniform colors, a mirrored sample and a reinterpreted
/// load.
const PROGRAMS: &str = "
struct Constant { color: vec4<f32> }
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var words: texture_2d<u32>;
@group(0) @binding(3) var<uniform> constant: Constant;
@vertex fn covering(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4<f32>(p[i], 0.5, 1.0);
}
@vertex fn halves(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // The left half wound counter-clockwise, then the right half wound clockwise.
    var p = array(
        vec2(-1.0, -1.0), vec2(0.0, -1.0), vec2(-1.0, 1.0),
        vec2(0.0, -1.0), vec2(0.0, 1.0), vec2(-1.0, 1.0),
        vec2(0.0, -1.0), vec2(0.0, 1.0), vec2(1.0, -1.0),
        vec2(1.0, -1.0), vec2(0.0, 1.0), vec2(1.0, 1.0),
    );
    return vec4<f32>(p[i], 0.5, 1.0);
}
@fragment fn red() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
@fragment fn tinted() -> @location(0) vec4<f32> {
    return constant.color;
}
@fragment fn mirrored(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    var us = array(-0.375, -1.5, 1.375, 0.625);
    return textureSample(tex, samp, vec2<f32>(us[u32(at.x)], 0.5));
}
@fragment fn loaded(@builtin(position) at: vec4<f32>) -> @location(0) vec4<u32> {
    return textureLoad(words, vec2<i32>(at.xy), 0);
}
";

/// Returns the interface's device on the system's GPU.
fn device() -> MetalGpu<DirectMetal> {
    MetalGpu::new(DirectMetal::new(()))
}

/// Returns the recording back end with `features` added.
fn reference(features: Features) -> Recorder {
    Recorder::with_caps(|caps| caps.features |= features)
}

/// Makes a 2D texture of one level, `SIDE` high.
fn texture<B: Gpu>(
    b: &mut B,
    format: Format,
    width: u32,
    usage: TextureUsage,
    reinterpret: bool,
) -> B::Texture {
    b.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width,
        height: SIDE,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage,
        cube: false,
        reinterpret,
    })
    .expect("create_texture")
}

/// Makes a view of a whole texture of one level in `format`.
fn view<B: Gpu>(b: &mut B, texture: &B::Texture, format: Format, stencil: bool) -> B::TextureView {
    b.create_view(
        texture,
        &TextureViewDesc {
            format,
            dimension: ViewDimension::D2,
            base_mip: 0,
            mip_count: 1,
            base_layer: 0,
            layer_count: 1,
            stencil,
        },
    )
    .expect("create_view")
}

/// Makes a buffer of `size` bytes.
fn buffer<B: Gpu>(b: &mut B, size: u64) -> B::Buffer {
    b.create_buffer(&BufferDesc {
        size,
        usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::UNIFORM,
        intent: MapIntent::Rarely,
    })
    .expect("create_buffer")
}

/// Returns the whole of a texture's first level and layer, from its corner.
fn whole<B: Gpu>(texture: &B::Texture) -> TextureRegion<'_, B> {
    TextureRegion {
        texture,
        mip: 0,
        layer: 0,
        origin: Origin { x: 0, y: 0, z: 0 },
        stencil: false,
    }
}

/// Reads a texture back, tightly packed.
fn read_texture<B: Gpu>(b: &mut B, texture: &B::Texture, width: u32, texel: usize) -> Vec<u8> {
    let mut out = vec![0; width as usize * SIDE as usize * texel];
    let extent = Extent {
        width,
        height: SIDE,
        depth_or_layers: 1,
    };
    b.submit();
    b.read_back_now(
        &ReadSource::Texture {
            region: whole(texture),
            extent,
        },
        &mut out,
    )
    .expect("read_back_now");
    out
}

/// Makes a program of two entry points of the tests' programs.
fn program<B: Gpu>(b: &mut B, vertex: &str, fragment: &str) -> B::Program {
    let module = |entry, stage| ShaderModule::from_wgsl(PROGRAMS, entry, stage).expect("parses");
    let vertex = b
        .create_shader(&module(vertex, ShaderStage::Vertex))
        .expect("create_shader");
    let fragment = b
        .create_shader(&module(fragment, ShaderStage::Fragment))
        .expect("create_shader");
    b.create_program(&Program::Render {
        vertex: &vertex,
        fragment: Some(&fragment),
    })
    .expect("create_program")
}

/// The fixed states of a draw: blend, depth and stencil, raster and an empty vertex layout.
struct States<B: Gpu> {
    blend: B::Blend,
    depth_stencil: B::DepthStencil,
    raster: B::Raster,
    layout: B::VertexLayout,
}

/// Makes a draw's states from their descriptions.
fn states<B: Gpu>(
    b: &mut B,
    target: RenderTargetBlend,
    depth_stencil: &DepthStencilDesc,
    raster: &RasterDesc,
) -> States<B> {
    States {
        blend: b
            .create_blend(&BlendDesc {
                targets: [target; MAX_COLOR_TARGETS],
                same_for_all: true,
                alpha_to_coverage: false,
                logic_op: None,
            })
            .expect("create_blend"),
        depth_stencil: b
            .create_depth_stencil(depth_stencil)
            .expect("create_depth_stencil"),
        raster: b.create_raster(raster).expect("create_raster"),
        layout: b
            .create_vertex_layout(&VertexLayoutDesc {
                streams: [None; MAX_VERTEX_STREAMS],
                attributes: [None; MAX_VERTEX_STREAMS],
            })
            .expect("create_vertex_layout"),
    }
}

/// Returns a target's blend that writes every channel, blended or not.
fn writes_all(blend: Option<(BlendComponent, BlendComponent)>) -> RenderTargetBlend {
    RenderTargetBlend {
        blend,
        writes: ColorWrites::all(),
    }
}

/// Returns a depth and stencil state that tests nothing.
fn no_tests() -> DepthStencilDesc {
    DepthStencilDesc {
        depth_compare: None,
        depth_write: false,
        stencil: None,
        stencil_read_mask: [u8::MAX; 2],
        stencil_write_mask: [u8::MAX; 2],
        depth_bounds: None,
    }
}

/// Returns a raster state that culls nothing, with a constant depth bias.
fn plain_raster(depth_bias: f32) -> RasterDesc {
    RasterDesc {
        fill: FillMode::Solid,
        cull: CullMode::None,
        front: FrontFace::CounterClockwise,
        depth_bias: depth_bias.to_bits(),
        depth_bias_slope: 0,
        depth_bias_clamp: 0,
        depth_clip: true,
        scissor: false,
        line_width: 1.0f32.to_bits(),
        provoking: ProvokingVertex::First,
        depth_minus_one: false,
        sample_mask: !0,
    }
}

/// Returns a viewport over a target `width` wide.
fn viewport(width: u32) -> [Viewport; 1] {
    [Viewport {
        x: 0,
        y: 0,
        width: (width as f32).to_bits(),
        height: (SIDE as f32).to_bits(),
        min_depth: 0,
        max_depth: 1.0f32.to_bits(),
    }]
}

/// Draws `count` vertices of `program` with `states` and one binding set in the open pass.
#[allow(clippy::too_many_arguments)]
fn draw<B: Gpu>(
    b: &mut B,
    program: &B::Program,
    states: &States<B>,
    set: Option<&B::BindingSet>,
    width: u32,
    count: u32,
    stencil_reference: [u8; 2],
    blend_constant: [f32; 4],
) {
    b.draw(&DrawCall {
        program,
        blend: &states.blend,
        depth_stencil: &states.depth_stencil,
        raster: &states.raster,
        vertex_layout: &states.layout,
        bindings: [set, None, None, None],
        vertex_buffers: &[],
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &viewport(width),
        scissors: &[],
        blend_constant: blend_constant.map(f32::to_bits),
        stencil_reference,
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count,
            first_instance: 0,
            instances: 1,
        },
    });
}

/// The bytes of the uniform the tinted fragment reads.
const UNIFORM_BYTES: u64 = 16;

/// Returns the binding of the tinted fragment's uniform at `offset` of `buffer`.
fn uniform<B: Gpu>(buffer: &B::Buffer, offset: u64) -> BindingEntry<'_, B> {
    BindingEntry {
        binding: 3,
        stages: StageSet::FRAGMENT,
        resource: BindingResource::UniformBuffer(wcontainers_gpu::BufferRange {
            buffer,
            offset,
            size: UNIFORM_BYTES,
        }),
    }
}

/// Returns a pass into one color view, cleared to `clear`.
fn color_pass<'a, B: Gpu>(target: &'a B::TextureView, clear: [u32; 4]) -> PassDesc<'a, B> {
    let mut pass = PassDesc {
        colors: Default::default(),
        depth: None,
        empty: None,
        occlusion: None,
    };
    pass.colors[0] = Some(ColorAttachment {
        view: target,
        depth_slice: 0,
        resolve: None,
        load: LoadOp::Clear(clear),
        store: StoreOp::Store,
    });
    pass
}

/// Returns an `rgba8Unorm` target to draw into, and its view.
fn target<B: Gpu>(b: &mut B, format: Format, width: u32) -> (B::Texture, B::TextureView) {
    let texture = texture(
        b,
        format,
        width,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
        false,
    );
    let view = view(b, &texture, format, false);
    (texture, view)
}

#[test]
fn a_texture_wider_than_metal_makes_is_refused_not_fatal() {
    let mut gpu = device();
    let texture = gpu.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width: BEYOND_ANY_SIDE,
        height: 1,
        depth_or_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: TextureUsage::SAMPLE,
        cube: false,
        reinterpret: false,
    });
    assert!(matches!(texture, Err(GpuError::Invalid(_))));
}

#[test]
fn declares_the_metal_features_tested_here() {
    let wanted = Features::READ_BACK_NOW
        | Features::FORMAT_REINTERPRET
        | Features::MIRROR_ONCE
        | Features::BLEND_CONSTANT_ALPHA
        | Features::STENCIL_PER_FACE
        | Features::FRACTIONAL_DEPTH_BIAS
        | Features::OCCLUSION_COUNT;
    assert!(device().caps().features.contains(wanted));
}

/// Writes bytes, copies them on the GPU, and reads the copy back at once.
fn read_back_now_scene<B: Gpu>(b: &mut B) -> Vec<u8> {
    let bytes: Vec<u8> = (0..64).map(|i: u8| i.wrapping_mul(37)).collect();
    let source = buffer(b, bytes.len() as u64);
    let copy = buffer(b, bytes.len() as u64);
    b.write_buffer(&source, 0, &bytes, WriteMode::Ordered);
    b.copy_buffer(&source, 1, &copy, 3, 50);
    let mut read = vec![0; bytes.len()];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &copy,
            offset: 0,
            size: read.len() as u64,
        },
        &mut read,
    )
    .expect("read_back_now");
    read
}

#[test]
fn reads_back_at_once_as_the_reference_does() {
    assert_eq!(
        read_back_now_scene(&mut device()),
        read_back_now_scene(&mut reference(Features::empty()))
    );
}

/// Writes ring parts before each of three passes that read the ring, and returns what each part holds afterwards.
fn ring_scene<B: Gpu>(b: &mut B) -> Vec<Vec<u8>> {
    let (_, target_view) = target(b, Format::Rgba8Unorm, SIDE);
    let tinted = program(b, "covering", "tinted");
    let states = states(b, writes_all(None), &no_tests(), &plain_raster(0.0));
    let parts: [&[u8]; 3] = [&[1, 2, 3], &[4, 5, 6, 7, 8], &[9; 300]];
    let mut written = Vec::new();
    for round in 0..3 {
        let mut offsets = [0; 3];
        let ring = b
            .write_buffer_ring(RingKind::Data, &parts, 256, &mut offsets)
            .expect("write_buffer_ring");
        let set = b
            .create_binding_set(&[uniform(&ring, offsets[0])])
            .expect("create_binding_set");
        b.begin_pass(&color_pass(&target_view, [0; 4]));
        draw(b, &tinted, &states, Some(&set), SIDE, 3, [0; 2], [0.0; 4]);
        b.end_pass();
        written.push((ring, offsets, round));
    }
    b.submit();
    let mut held = Vec::new();
    for (ring, offsets, _) in &written {
        for (part, offset) in parts.iter().zip(offsets) {
            let mut read = vec![0; part.len()];
            b.read_back_now(
                &ReadSource::Buffer {
                    buffer: ring,
                    offset: *offset,
                    size: part.len() as u64,
                },
                &mut read,
            )
            .expect("read_back_now");
            held.push(read);
        }
    }
    held
}

#[test]
fn ring_writes_stay_where_they_say_while_passes_read_them() {
    // The recording back end's ring is not made to be read back, so the parts are checked against themselves.
    let parts: [&[u8]; 3] = [&[1, 2, 3], &[4, 5, 6, 7, 8], &[9; 300]];
    let expected: Vec<Vec<u8>> = (0..3).flat_map(|_| parts.map(<[u8]>::to_vec)).collect();
    assert_eq!(ring_scene(&mut device()), expected);
}

/// Samples a 4-texel row with mirror-once addressing at u of -0.375, -1.5, 1.375 and 0.625, and returns the red row.
fn mirror_once_scene<B: Gpu>(b: &mut B) -> Vec<u8> {
    let source = texture(
        b,
        Format::Rgba8Unorm,
        SIDE,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        false,
    );
    let texels: Vec<u8> = (0..SIDE * SIDE)
        .flat_map(|i| [10 * (1 + (i % SIDE) as u8), 0, 0, u8::MAX])
        .collect();
    b.write_texture(
        &whole(&source),
        Extent {
            width: SIDE,
            height: SIDE,
            depth_or_layers: 1,
        },
        &texels,
        SIDE * RGBA8_BYTES as u32,
        SIDE,
        WriteMode::Ordered,
    );
    let source_view = view(b, &source, Format::Rgba8Unorm, false);
    let sampler = b
        .create_sampler(&SamplerDesc {
            min: Filter::Nearest,
            mag: Filter::Nearest,
            mip: Filter::Nearest,
            address: [AddressMode::MirrorOnce; 3],
            lod_range: [0, 0],
            anisotropy: 1,
            compare: None,
            border: BorderColor::TransparentBlack,
        })
        .expect("create_sampler");
    let set = b
        .create_binding_set(&[
            BindingEntry {
                binding: 0,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Texture(&source_view),
            },
            BindingEntry {
                binding: 1,
                stages: StageSet::FRAGMENT,
                resource: BindingResource::Sampler(&sampler),
            },
        ])
        .expect("create_binding_set");
    let (drawn, drawn_view) = target(b, Format::Rgba8Unorm, SIDE);
    let mirrored = program(b, "covering", "mirrored");
    let states = states(b, writes_all(None), &no_tests(), &plain_raster(0.0));
    b.begin_pass(&color_pass(&drawn_view, [0; 4]));
    draw(b, &mirrored, &states, Some(&set), SIDE, 3, [0; 2], [0.0; 4]);
    b.end_pass();
    let texels = read_texture(b, &drawn, SIDE, RGBA8_BYTES);
    texels[..SIDE as usize * RGBA8_BYTES]
        .chunks(RGBA8_BYTES)
        .map(|t| t[0])
        .collect()
}

#[test]
fn mirror_once_mirrors_once_then_clamps() {
    // -0.375 mirrors to 0.375, texel 1; -1.5 mirrors to 1.5 and clamps to texel 3, where repeating mirrors would give
    // texel 2; 1.375 clamps to texel 3; 0.625 is texel 2.
    assert_eq!(mirror_once_scene(&mut device()), [20, 40, 40, 30]);
}

/// Draws red into a target cleared to half grey, blended by the constant's alpha both ways.
fn constant_alpha_scene<B: Gpu>(b: &mut B) -> Vec<u8> {
    let (drawn, drawn_view) = target(b, Format::Rgba8Unorm, SIDE);
    let red = program(b, "covering", "red");
    let component = BlendComponent {
        src: BlendFactor::ConstantAlpha,
        dst: BlendFactor::OneMinusConstantAlpha,
        op: BlendOp::Add,
    };
    let states = states(
        b,
        writes_all(Some((component, component))),
        &no_tests(),
        &plain_raster(0.0),
    );
    let grey = 0.5f32.to_bits();
    b.begin_pass(&color_pass(&drawn_view, [grey; 4]));
    draw(
        b,
        &red,
        &states,
        None,
        SIDE,
        3,
        [0; 2],
        [0.0, 0.0, 0.0, 0.25],
    );
    b.end_pass();
    read_texture(b, &drawn, SIDE, RGBA8_BYTES)
}

#[test]
fn blends_by_the_constant_alpha_as_the_reference_does() {
    let got = constant_alpha_scene(&mut device());
    let expected = constant_alpha_scene(&mut reference(Features::BLEND_CONSTANT_ALPHA));
    assert!(
        got.iter().zip(&expected).all(|(g, e)| g.abs_diff(*e) <= 1),
        "{got:?} against {expected:?}"
    );
}

/// Writes stencil per face, 3 on the front-facing left half and 7 on the back-facing right, then colors each value.
fn stencil_per_face_scene<B: Gpu>(b: &mut B) -> Vec<u8> {
    let (drawn, drawn_view) = target(b, Format::Rgba8Unorm, SIDE);
    let stencil_texture = texture(
        b,
        Format::Depth24PlusStencil8,
        SIDE,
        TextureUsage::RENDER,
        false,
    );
    let stencil_view = view(b, &stencil_texture, Format::Depth24PlusStencil8, false);
    let red = program(b, "halves", "red");
    let face = |compare, pass| StencilFace {
        compare,
        fail: StencilOp::Keep,
        depth_fail: StencilOp::Keep,
        pass,
    };
    let stencil = |compare, pass| DepthStencilDesc {
        stencil: Some((face(compare, pass), face(compare, pass))),
        ..no_tests()
    };
    let replace = states(
        b,
        RenderTargetBlend {
            blend: None,
            writes: ColorWrites::empty(),
        },
        &stencil(CompareFn::Always, StencilOp::Replace),
        &plain_raster(0.0),
    );
    let equal = |b: &mut B, writes| {
        states(
            b,
            RenderTargetBlend {
                blend: None,
                writes,
            },
            &stencil(CompareFn::Equal, StencilOp::Keep),
            &plain_raster(0.0),
        )
    };
    let (reds, greens) = (equal(b, ColorWrites::R), equal(b, ColorWrites::G));
    let mut pass = color_pass(&drawn_view, [0, 0, 0, 1.0f32.to_bits()]);
    pass.depth = Some(DepthAttachment {
        view: &stencil_view,
        depth: Some(AspectOps {
            load: LoadOp::Clear(1.0f32.to_bits()),
            store: StoreOp::Discard,
            read_only: false,
        }),
        stencil: Some(AspectOps {
            load: LoadOp::Clear(0),
            store: StoreOp::Discard,
            read_only: false,
        }),
    });
    let quads = 12;
    b.begin_pass(&pass);
    draw(b, &red, &replace, None, SIDE, quads, [3, 7], [0.0; 4]);
    draw(b, &red, &reds, None, SIDE, quads, [3, 3], [0.0; 4]);
    draw(b, &red, &greens, None, SIDE, quads, [7, 7], [0.0; 4]);
    b.end_pass();
    read_texture(b, &drawn, SIDE, RGBA8_BYTES)
}

#[test]
fn stencil_references_per_face_as_the_reference_does() {
    let got = stencil_per_face_scene(&mut device());
    assert_eq!(
        got,
        stencil_per_face_scene(&mut reference(Features::STENCIL_PER_FACE))
    );
    // The left half is front-facing and red; the right is back-facing and would be red too with one reference.
    let right = &got[(SIDE as usize - 1) * RGBA8_BYTES..SIDE as usize * RGBA8_BYTES];
    assert_eq!(right, [0, 0, 0, u8::MAX]);
}

/// Draws at depth one half with a constant depth bias of a half step into 32-bit float depth, and returns the depth.
fn fractional_bias_scene<B: Gpu>(b: &mut B, bias: f32) -> Vec<u8> {
    let (_, drawn_view) = target(b, Format::Rgba8Unorm, SIDE);
    let depth = texture(
        b,
        Format::Depth32Float,
        SIDE,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
        false,
    );
    let depth_view = view(b, &depth, Format::Depth32Float, false);
    let red = program(b, "covering", "red");
    let states = states(
        b,
        writes_all(None),
        &DepthStencilDesc {
            depth_compare: Some(CompareFn::Always),
            depth_write: true,
            ..no_tests()
        },
        &plain_raster(bias),
    );
    let mut pass = color_pass(&drawn_view, [0; 4]);
    pass.depth = Some(DepthAttachment {
        view: &depth_view,
        depth: Some(AspectOps {
            load: LoadOp::Clear(1.0f32.to_bits()),
            store: StoreOp::Store,
            read_only: false,
        }),
        stencil: None,
    });
    b.begin_pass(&pass);
    draw(b, &red, &states, None, SIDE, 3, [0; 2], [0.0; 4]);
    b.end_pass();
    read_texture(b, &depth, SIDE, 4)
}

#[test]
fn a_fractional_depth_bias_moves_depth_as_the_reference_does() {
    let reference_depth =
        fractional_bias_scene(&mut reference(Features::FRACTIONAL_DEPTH_BIAS), 2.5);
    assert_eq!(fractional_bias_scene(&mut device(), 2.5), reference_depth);
    assert_ne!(fractional_bias_scene(&mut device(), 0.0), reference_depth);
}

/// Writes `rgba8Unorm` texels, loads them through a view in `rgba8Uint`, and returns what the loads wrote.
fn reinterpret_scene<B: Gpu>(b: &mut B) -> Vec<u8> {
    let source = texture(
        b,
        Format::Rgba8Unorm,
        SIDE,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        true,
    );
    let texels: Vec<u8> = (0..SIDE * SIDE * RGBA8_BYTES as u32)
        .map(|i| (i as u8).wrapping_mul(29))
        .collect();
    b.write_texture(
        &whole(&source),
        Extent {
            width: SIDE,
            height: SIDE,
            depth_or_layers: 1,
        },
        &texels,
        SIDE * RGBA8_BYTES as u32,
        SIDE,
        WriteMode::Ordered,
    );
    let words = view(b, &source, Format::Rgba8Uint, false);
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 2,
            stages: StageSet::FRAGMENT,
            resource: BindingResource::Texture(&words),
        }])
        .expect("create_binding_set");
    let (drawn, drawn_view) = target(b, Format::Rgba8Uint, SIDE);
    let loaded = program(b, "covering", "loaded");
    let states = states(b, writes_all(None), &no_tests(), &plain_raster(0.0));
    b.begin_pass(&color_pass(&drawn_view, [0; 4]));
    draw(b, &loaded, &states, Some(&set), SIDE, 3, [0; 2], [0.0; 4]);
    b.end_pass();
    read_texture(b, &drawn, SIDE, RGBA8_BYTES)
}

#[test]
fn a_view_reinterprets_its_texture_as_the_reference_does() {
    let got = reinterpret_scene(&mut device());
    assert_eq!(got, reinterpret_scene(&mut reference(Features::empty())));
}

/// Counts samples of the left half in query 0, and in query 1 of the left half twice around a write that splits the
/// pass; returns the counts as read back and as resolved on the GPU.
fn occlusion_count_scene<B: Gpu>(b: &mut B) -> (Vec<u8>, Vec<u8>) {
    let (_, drawn_view) = target(b, Format::Rgba8Unorm, SIDE);
    let tinted = program(b, "halves", "tinted");
    let states = states(b, writes_all(None), &no_tests(), &plain_raster(0.0));
    let queries = b
        .create_query_set(QueryKind::Occlusion, 2)
        .expect("create_query_set");
    let busy = buffer(b, UNIFORM_BYTES);
    let set = b
        .create_binding_set(&[uniform(&busy, 0)])
        .expect("create_binding_set");
    let resolved = buffer(b, 16);
    let mut pass = color_pass(&drawn_view, [0; 4]);
    pass.occlusion = Some(&queries);
    let left_half = 6;
    b.begin_pass(&pass);
    b.begin_query(0);
    draw(
        b,
        &tinted,
        &states,
        Some(&set),
        SIDE,
        left_half,
        [0; 2],
        [0.0; 4],
    );
    b.end_query();
    b.begin_query(1);
    draw(
        b,
        &tinted,
        &states,
        Some(&set),
        SIDE,
        left_half,
        [0; 2],
        [0.0; 4],
    );
    // The buffer the draws read is written in order, which ends the pass and opens it again.
    b.write_buffer(&busy, 0, &[1; 8], WriteMode::Ordered);
    draw(
        b,
        &tinted,
        &states,
        Some(&set),
        SIDE,
        left_half,
        [0; 2],
        [0.0; 4],
    );
    b.end_query();
    b.end_pass();
    b.resolve_queries(&queries, 0, 2, &resolved, 0)
        .expect("resolve_queries");
    let readback = b.read_queries(&queries, 0, 2).expect("read_queries");
    b.submit();
    b.wait_read(&readback);
    let mut read = vec![0; 16];
    b.finish_read(readback, &mut read);
    let mut copied = vec![0; 16];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &resolved,
            offset: 0,
            size: 16,
        },
        &mut copied,
    )
    .expect("read_back_now");
    (read, copied)
}

#[test]
fn occlusion_counts_every_sample_across_a_split_pass() {
    let (read, resolved) = occlusion_count_scene(&mut device());
    let half = u64::from(SIDE * SIDE / 2);
    let expected: Vec<u8> = [half, 2 * half]
        .iter()
        .flat_map(|n| n.to_le_bytes())
        .collect();
    assert_eq!(read, expected);
    assert_eq!(resolved, expected);
}
