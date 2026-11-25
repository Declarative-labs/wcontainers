//! What this implementation does beyond the shared suite: refusals, the features WebGPU on Metal lacks, and fast paths.
//!
//! Each is checked against the recording back end, or against values worked out by hand where it lacks the feature.

#![cfg(target_os = "macos")]

use wcontainers_gpu::{
    AddressMode, AspectOps, BindingEntry, BindingResource, BlendComponent, BlendDesc, BlendFactor,
    BlendOp, BorderColor, BufferDesc, BufferUsage, ColorAttachment, ColorWrites, CompareFn,
    CullMode, DepthAttachment, DepthStencilDesc, DrawCall, DrawRange, Extent, Features, FillMode,
    Filter, Format, FrontFace, Gpu, GpuError, LoadOp, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS,
    MapIntent, Origin, PassDesc, Program, ProvokingVertex, QueryKind, RasterDesc, ReadSource,
    ReadState, RenderTargetBlend, RingKind, SamplerDesc, ShaderModule, ShaderStage, SlowPath,
    StageSet, StencilFace, StencilOp, StoreOp, TextureDesc, TextureDimension, TextureRegion,
    TextureUsage, TextureViewDesc, Topology, VertexLayoutDesc, ViewDimension, Viewport, WriteMode,
};
use wcontainers_gpu_metal::MetalGpu;
use wcontainers_gpu_test::Recorder;
use wcontainers_metal::StandaloneMetal;
use wcontainers_metal_direct::DirectMetal;

/// The side of the square targets the tests draw into: four texels, one for each coordinate the mirrored sample reads.
///
/// https://developer.apple.com/documentation/metal/mtlsampleraddressmode/mirrorclamptoedge
const DRAWN_SIDE: u32 = 4;

/// The bytes of one `rgba8` texel.
const RGBA8_BYTES: usize = wcontainers_gpu::FORMAT_FACTS[Format::Rgba8Unorm as usize]
    .block
    .2 as usize;

/// The bytes of one row of a target `DRAWN_SIDE` wide.
const DRAWN_ROW_BYTES: u32 = DRAWN_SIDE * RGBA8_BYTES as u32;

/// The bytes of a square target of `DRAWN_SIDE`.
const DRAWN_BYTES: u32 = DRAWN_ROW_BYTES * DRAWN_SIDE;

/// Returns the product of a test's sizes, which fits.
fn product(numbers: &[usize]) -> usize {
    numbers
        .iter()
        .try_fold(1usize, |total, n| total.checked_mul(*n))
        .expect("a test's size fits")
}

/// The tests' programs: a covering triangle or two halves, flat or uniform colors, a mirrored sample and a load.
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
@vertex fn sloped(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // Depth one eighth along the bottom edge, rising to one half along the top edge of the target.
    var p = array(vec3(-1.0, -1.0, 0.125), vec3(3.0, -1.0, 0.125), vec3(-1.0, 3.0, 0.875));
    return vec4<f32>(p[i], 1.0);
}
@vertex fn packed(@builtin(vertex_index) i: u32, @location(0) value: vec4<f32>) -> @builtin(position) vec4<f32> {
    // A covering triangle that the stream's values cannot move: each is a finite float times zero.
    var p = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4<f32>(p[i % 3u] + min(abs(value.xy), vec2(1.0)) * 0.0, 0.5, 1.0);
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

/// Makes a 2D texture of one level, `DRAWN_SIDE` high.
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
        height: DRAWN_SIDE,
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
    let mut out = vec![0; product(&[width as usize, DRAWN_SIDE as usize, texel])];
    let extent = Extent {
        width,
        height: DRAWN_SIDE,
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
        height: (DRAWN_SIDE as f32).to_bits(),
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
        width: wcontainers_metal::MAX_TEXTURE_SIDE + 1,
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
        | Features::OCCLUSION_COUNT
        | Features::POINT_SIZE;
    assert!(device().caps().features.contains(wanted));
    // Metal drops a constant bias's fraction, so a front adds the bias in the program.
    assert!(
        !device()
            .caps()
            .features
            .contains(Features::FRACTIONAL_DEPTH_BIAS)
    );
}

/// Draws one point at the target's center, of the size its vertex program writes, and returns the target's texels.
fn point_scene(size: f32) -> Vec<u8> {
    let mut b = device();
    let source = format!(
        "struct Point {{ @builtin(position) at: vec4<f32>, @location(0) size: f32 }}
         @vertex fn sized() -> Point {{ return Point(vec4<f32>(0.0, 0.0, 0.5, 1.0), {size:?}); }}
         @fragment fn red() -> @location(0) vec4<f32> {{ return vec4<f32>(1.0, 0.0, 0.0, 1.0); }}"
    );
    let mut sized = ShaderModule::from_wgsl(&source, "sized", ShaderStage::Vertex).expect("parses");
    // WGSL writes no point size: the second member becomes it, as the fronts write `gl_PointSize`.
    let module = &mut sized.module;
    let point = module.entry_points[0]
        .function
        .result
        .as_ref()
        .expect("a result")
        .ty;
    let naga::TypeInner::Struct { mut members, span } = module.types[point].inner.clone() else {
        panic!("a struct result");
    };
    members[1].binding = Some(naga::Binding::BuiltIn(naga::BuiltIn::PointSize));
    let sized_point = module.types.insert(
        naga::Type {
            name: Some("SizedPoint".into()),
            inner: naga::TypeInner::Struct { members, span },
        },
        naga::Span::UNDEFINED,
    );
    let function = &mut module.entry_points[0].function;
    if let Some(result) = &mut function.result {
        result.ty = sized_point;
    }
    for (_, expression) in function.expressions.iter_mut() {
        if let naga::Expression::Compose { ty, .. } = expression
            && *ty == point
        {
            *ty = sized_point;
        }
    }
    sized.info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&sized.module)
    .expect("a point size validates");
    sized.key[0] ^= 1;
    let vertex = b.create_shader(&sized).expect("create_shader");
    let red = ShaderModule::from_wgsl(&source, "red", ShaderStage::Fragment).expect("parses");
    let fragment = b.create_shader(&red).expect("create_shader");
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .expect("create_program");
    let states = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
    let (drawn, drawn_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
    b.begin_pass(&color_pass(&drawn_view, [0; 4]));
    b.draw(&DrawCall {
        program: &program,
        blend: &states.blend,
        depth_stencil: &states.depth_stencil,
        raster: &states.raster,
        vertex_layout: &states.layout,
        bindings: [None, None, None, None],
        vertex_buffers: &[],
        index: None,
        topology: Topology::PointList,
        restart: false,
        viewports: &viewport(DRAWN_SIDE),
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Vertices {
            first: 0,
            count: 1,
            first_instance: 0,
            instances: 1,
        },
    });
    b.end_pass();
    read_texture(&mut b, &drawn, DRAWN_SIDE, RGBA8_BYTES)
}

#[test]
fn a_point_is_as_large_as_its_program_writes() {
    assert!(device().caps().features.contains(Features::POINT_SIZE));
    let red = |texels: Vec<u8>| -> Vec<bool> {
        texels
            .chunks(RGBA8_BYTES)
            .map(|t| t[0] == u8::MAX)
            .collect()
    };
    let (o, x) = (false, true);
    // Two texels wide about the center covers the middle four of sixteen; four wide, all of them.
    assert_eq!(
        red(point_scene(2.0)),
        [o, o, o, o, o, x, x, o, o, x, x, o, o, o, o, o]
    );
    assert_eq!(red(point_scene(4.0)), [x; 16]);
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
    let (_, target_view) = target(b, Format::Rgba8Unorm, DRAWN_SIDE);
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
        draw(
            b,
            &tinted,
            &states,
            Some(&set),
            DRAWN_SIDE,
            3,
            [0; 2],
            [0.0; 4],
        );
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
        DRAWN_SIDE,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        false,
    );
    let texels: Vec<u8> = (0..DRAWN_SIDE * DRAWN_SIDE)
        .flat_map(|i| [[10, 20, 30, 40][(i % DRAWN_SIDE) as usize], 0, 0, u8::MAX])
        .collect();
    b.write_texture(
        &whole(&source),
        Extent {
            width: DRAWN_SIDE,
            height: DRAWN_SIDE,
            depth_or_layers: 1,
        },
        &texels,
        DRAWN_ROW_BYTES,
        DRAWN_SIDE,
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
    let (drawn, drawn_view) = target(b, Format::Rgba8Unorm, DRAWN_SIDE);
    let mirrored = program(b, "covering", "mirrored");
    let states = states(b, writes_all(None), &no_tests(), &plain_raster(0.0));
    b.begin_pass(&color_pass(&drawn_view, [0; 4]));
    draw(
        b,
        &mirrored,
        &states,
        Some(&set),
        DRAWN_SIDE,
        3,
        [0; 2],
        [0.0; 4],
    );
    b.end_pass();
    let texels = read_texture(b, &drawn, DRAWN_SIDE, RGBA8_BYTES);
    texels[..DRAWN_SIDE as usize * RGBA8_BYTES]
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
    let (drawn, drawn_view) = target(b, Format::Rgba8Unorm, DRAWN_SIDE);
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
        DRAWN_SIDE,
        3,
        [0; 2],
        [0.0, 0.0, 0.0, 0.25],
    );
    b.end_pass();
    read_texture(b, &drawn, DRAWN_SIDE, RGBA8_BYTES)
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
    let (drawn, drawn_view) = target(b, Format::Rgba8Unorm, DRAWN_SIDE);
    let stencil_texture = texture(
        b,
        Format::Depth24PlusStencil8,
        DRAWN_SIDE,
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
    draw(b, &red, &replace, None, DRAWN_SIDE, quads, [3, 7], [0.0; 4]);
    draw(b, &red, &reds, None, DRAWN_SIDE, quads, [3, 3], [0.0; 4]);
    draw(b, &red, &greens, None, DRAWN_SIDE, quads, [7, 7], [0.0; 4]);
    b.end_pass();
    read_texture(b, &drawn, DRAWN_SIDE, RGBA8_BYTES)
}

#[test]
fn stencil_references_per_face_as_the_reference_does() {
    let got = stencil_per_face_scene(&mut device());
    assert_eq!(
        got,
        stencil_per_face_scene(&mut reference(Features::STENCIL_PER_FACE))
    );
    // The left half is front-facing and red; the right is back-facing and would be red too with one reference.
    let right = &got[(DRAWN_SIDE as usize - 1) * RGBA8_BYTES..DRAWN_SIDE as usize * RGBA8_BYTES];
    assert_eq!(right, [0, 0, 0, u8::MAX]);
}

/// Returns the 32-bit float depth of each texel of a triangle from depth 1/8 to 1/2, drawn with a constant bias.
fn depth_bias_scene<B: Gpu>(b: &mut B, bias: f32) -> Vec<f32> {
    let (_, drawn_view) = target(b, Format::Rgba8Unorm, DRAWN_SIDE);
    let depth = texture(
        b,
        Format::Depth32Float,
        DRAWN_SIDE,
        TextureUsage::RENDER | TextureUsage::COPY_SRC,
        false,
    );
    let depth_view = view(b, &depth, Format::Depth32Float, false);
    let red = program(b, "sloped", "red");
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
    draw(b, &red, &states, None, DRAWN_SIDE, 3, [0; 2], [0.0; 4]);
    b.end_pass();
    read_texture(b, &depth, DRAWN_SIDE, 4)
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| f32::from_le_bytes(*word))
        .collect()
}

/// The unit of a constant depth bias of a primitive whose greatest 32-bit float depth lies in `[1/2, 1)`: `2^-24`.
///
/// https://learn.microsoft.com/en-us/windows/win32/direct3d11/d3d10-graphics-programming-guide-output-merger-stage-depth-bias
const BIAS_UNIT: f64 = f32::EPSILON as f64 / 2.0;

/// Returns how far a bias moved each texel, in units of the bias.
fn bias_moves<B: Gpu>(b: &mut B, bias: f32) -> Vec<f64> {
    let unbiased = depth_bias_scene(b, 0.0);
    let biased = depth_bias_scene(b, bias);
    unbiased
        .iter()
        .zip(&biased)
        .map(|(depth, moved)| (f64::from(*moved) - f64::from(*depth)) / BIAS_UNIT)
        .collect()
}

#[test]
fn a_whole_depth_bias_moves_depth_as_the_reference_does_and_a_fraction_is_absent() {
    // Every texel's depth lies in `[1/8, 1/2)`, where a step of 32-bit float depth is at most the bias's unit, so a
    // whole bias lands exactly: dropping the bias, or scaling its unit, moves every texel off.
    for bias in [1.0f32, 2.0, 3.0] {
        let reference_moves = bias_moves(&mut reference(Features::empty()), bias);
        let moves = bias_moves(&mut device(), bias);
        assert_eq!(
            moves.len(),
            product(&[DRAWN_SIDE as usize, DRAWN_SIDE as usize])
        );
        assert!(
            reference_moves.iter().all(|m| *m == f64::from(bias)),
            "the reference's moves {reference_moves:?}"
        );
        assert_eq!(moves, reference_moves, "a bias of {bias}");
    }
    // MEASURED on an Apple M5: a bias of 2.25, 2.5 or 2.75 moves depth by 2 units and 0.5 by none.
    let fraction = device().create_raster(&plain_raster(2.25));
    assert!(matches!(
        fraction,
        Err(GpuError::Absent(Features::FRACTIONAL_DEPTH_BIAS))
    ));
}

/// Writes `r32Float` texels, loads them through a view in `rgba8Uint`, and returns what the loads wrote.
///
/// The format has no sRGB twin, so only the texture's `reinterpret` lets Metal make the view.
fn reinterpret_scene<B: Gpu>(b: &mut B) -> Vec<u8> {
    let source = texture(
        b,
        Format::R32Float,
        DRAWN_SIDE,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        true,
    );
    let texels: Vec<u8> = (0..DRAWN_BYTES)
        .map(|i| (i as u8).wrapping_mul(29))
        .collect();
    b.write_texture(
        &whole(&source),
        Extent {
            width: DRAWN_SIDE,
            height: DRAWN_SIDE,
            depth_or_layers: 1,
        },
        &texels,
        DRAWN_ROW_BYTES,
        DRAWN_SIDE,
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
    let (drawn, drawn_view) = target(b, Format::Rgba8Uint, DRAWN_SIDE);
    let loaded = program(b, "covering", "loaded");
    let states = states(b, writes_all(None), &no_tests(), &plain_raster(0.0));
    b.begin_pass(&color_pass(&drawn_view, [0; 4]));
    draw(
        b,
        &loaded,
        &states,
        Some(&set),
        DRAWN_SIDE,
        3,
        [0; 2],
        [0.0; 4],
    );
    b.end_pass();
    read_texture(b, &drawn, DRAWN_SIDE, RGBA8_BYTES)
}

#[test]
fn a_view_reinterprets_its_texture_as_the_reference_does() {
    let got = reinterpret_scene(&mut device());
    assert_eq!(got, reinterpret_scene(&mut reference(Features::empty())));
    // Without `reinterpret` the texture is not seen in another format of its size.
    let mut b = device();
    let plain = texture(
        &mut b,
        Format::R32Float,
        DRAWN_SIDE,
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        false,
    );
    let refused = b.create_view(
        &plain,
        &TextureViewDesc {
            format: Format::Rgba8Uint,
            dimension: ViewDimension::D2,
            base_mip: 0,
            mip_count: 1,
            base_layer: 0,
            layer_count: 1,
            stencil: false,
        },
    );
    assert!(
        refused.is_err(),
        "a view in another format of a texture not made to be reinterpreted"
    );
}

#[test]
fn mipmaps_of_an_integer_format_are_refused_and_the_device_stays() {
    let mut b = device();
    for format in [Format::Rgba8Uint, Format::R32Sint] {
        let desc = TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: DRAWN_SIDE,
            height: DRAWN_SIDE,
            depth_or_layers: 1,
            mip_levels: 3,
            samples: 1,
            usage: TextureUsage::SAMPLE | TextureUsage::RENDER | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        };
        let made = b.create_texture(&desc).expect("create_texture");
        let generated = b.generate_mips(&made, &desc, format, 1..3, 0..1);
        assert!(
            matches!(generated, Err(GpuError::Invalid(_))),
            "{format:?}: {generated:?}"
        );
    }
    let fence = b.submit();
    b.wait_fence(&fence);
    assert_eq!(b.status(), wcontainers_gpu::DeviceStatus::Ok);
}

/// Counts the left half's samples once in query 0 and twice in query 1, around a write that splits the pass.
///
/// Returns the counts as read back and as resolved on the GPU.
fn occlusion_count_scene<B: Gpu>(b: &mut B) -> (Vec<u8>, Vec<u8>) {
    let (_, drawn_view) = target(b, Format::Rgba8Unorm, DRAWN_SIDE);
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
        DRAWN_SIDE,
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
        DRAWN_SIDE,
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
        DRAWN_SIDE,
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
    let half = u64::from(DRAWN_SIDE * DRAWN_SIDE / 2);
    let expected: Vec<u8> = [half, 2 * half]
        .iter()
        .flat_map(|n| n.to_le_bytes())
        .collect();
    assert_eq!(read, expected);
    assert_eq!(resolved, expected);
}

/// Draws red, built ahead by `prepare` or not, and returns the texels and the pipelines the draw had to build.
fn prepared_scene(prepare: bool) -> (Vec<u8>, u64) {
    let mut b = device();
    let (drawn, drawn_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
    let red = program(&mut b, "covering", "red");
    let states = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
    if prepare {
        b.prepare(&wcontainers_gpu::PrepareDesc {
            program: &red,
            blend: &states.blend,
            depth_stencil: &states.depth_stencil,
            raster: &states.raster,
            vertex_layout: &states.layout,
            targets: wcontainers_gpu::TargetLayout {
                colors: {
                    let mut colors = [None; MAX_COLOR_TARGETS];
                    colors[0] = Some(Format::Rgba8Unorm);
                    colors
                },
                depth_stencil: None,
                samples: 1,
            },
            topology: Topology::TriangleList,
            layouts: Default::default(),
        });
    }
    let before = b.slow_paths().counts[wcontainers_gpu::SlowPath::PipelineMiss as usize];
    b.begin_pass(&color_pass(&drawn_view, [0; 4]));
    draw(&mut b, &red, &states, None, DRAWN_SIDE, 3, [0; 2], [0.0; 4]);
    b.end_pass();
    let after = b.slow_paths().counts[wcontainers_gpu::SlowPath::PipelineMiss as usize];
    (
        read_texture(&mut b, &drawn, DRAWN_SIDE, RGBA8_BYTES),
        after.checked_sub(before).expect("misses only grow"),
    )
}

#[test]
fn prepare_builds_the_pipeline_so_the_draw_finds_it() {
    let (prepared, misses) = prepared_scene(true);
    let (built_at_draw, misses_at_draw) = prepared_scene(false);
    assert_eq!(prepared, built_at_draw);
    assert_eq!((misses, misses_at_draw), (0, 1));
}

/// Fills levels `mips` of layers `layers`, natively or by the default's draws, and returns every level of every layer.
///
/// Every level of every layer is written apart first, so a level the call must keep shows if it was filled.
fn mips_scene(
    native: bool,
    (width, height, levels, layer_count): (u32, u32, u32, u32),
    mips: std::ops::Range<u32>,
    layers: std::ops::Range<u32>,
) -> Vec<u8> {
    let mut b = device();
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Rgba8Unorm,
        width,
        height,
        depth_or_layers: layer_count,
        mip_levels: levels,
        samples: 1,
        usage: TextureUsage::RENDER
            | TextureUsage::SAMPLE
            | TextureUsage::COPY_SRC
            | TextureUsage::COPY_DST,
        cube: false,
        reinterpret: false,
    };
    let texture = b.create_texture(&desc).expect("create_texture");
    let side = |n: u32, mip: u32| n.checked_shr(mip).unwrap_or(0).max(1);
    let bytes = |w: u32, h: u32| product(&[w as usize, h as usize, RGBA8_BYTES]);
    // Each level of each layer holds its own bytes, so a level the call must keep shows if it was filled.
    for layer in 0..layer_count {
        for mip in 0..levels {
            let (w, h) = (side(width, mip), side(height, mip));
            let seed = layer.wrapping_mul(levels).wrapping_add(mip).wrapping_add(1);
            let texels: Vec<u8> = (0..bytes(w, h))
                .map(|i| ((i as u32).wrapping_add(seed).wrapping_mul(2_654_435_761) >> 24) as u8)
                .collect();
            b.write_texture(
                &TextureRegion {
                    mip,
                    layer,
                    ..whole(&texture)
                },
                Extent {
                    width: w,
                    height: h,
                    depth_or_layers: 1,
                },
                &texels,
                w.checked_mul(RGBA8_BYTES as u32).expect("a row fits"),
                h,
                WriteMode::Ordered,
            );
        }
    }
    if native {
        b.generate_mips(&texture, &desc, desc.format, mips, layers)
    } else {
        wcontainers_gpu::generate_mips_by_draws(&mut b, &texture, &desc, desc.format, mips, layers)
    }
    .expect("generate_mips");
    b.submit();
    let mut all = Vec::new();
    for layer in 0..layer_count {
        for mip in 0..levels {
            let (w, h) = (side(width, mip), side(height, mip));
            let mut out = vec![0; bytes(w, h)];
            b.read_back_now(
                &ReadSource::Texture {
                    region: TextureRegion {
                        mip,
                        layer,
                        ..whole(&texture)
                    },
                    extent: Extent {
                        width: w,
                        height: h,
                        depth_or_layers: 1,
                    },
                },
                &mut out,
            )
            .expect("read_back_now");
            all.extend(out);
        }
    }
    all
}

#[test]
fn mipmaps_by_metal_match_the_default() {
    // Whole chains, which Metal's blit fills, and parts of chains and of layers, which the levels and layers outside
    // must keep: a blit for a part would fill them too.
    let cases = [
        ((8, 8, 4, 1), 1..4, 0..1),
        ((6, 5, 3, 1), 1..3, 0..1),
        ((2, 2, 2, 1), 1..2, 0..1),
        ((8, 8, 4, 1), 2..4, 0..1),
        ((8, 8, 4, 1), 1..2, 0..1),
        ((8, 8, 4, 2), 1..4, 1..2),
        ((8, 8, 4, 2), 1..4, 0..2),
    ];
    for (shape, mips, layers) in cases {
        let native = mips_scene(true, shape, mips.clone(), layers.clone());
        let drawn = mips_scene(false, shape, mips.clone(), layers.clone());
        assert!(native.iter().any(|&byte| byte != 0), "no level was filled");
        let worst = native.iter().zip(&drawn).map(|(n, d)| n.abs_diff(*d)).max();
        assert!(
            worst <= Some(1),
            "{shape:?} levels {mips:?} layers {layers:?}: this implementation's mipmaps differ from the default's by \
             {worst:?}"
        );
    }
}

/// The device this file's tests draw with.
type Device = MetalGpu<DirectMetal>;

/// Runs the trait's default `read_back_now`, which this implementation overrides, as the trait writes it.
fn read_back_now_by_default(
    b: &mut Device,
    src: &ReadSource<'_, Device>,
    dst: &mut [u8],
) -> Result<(), GpuError> {
    let readback = b.read_back(src)?;
    if b.read_state(&readback) == ReadState::Pending {
        b.record_slow_path(SlowPath::ReadBackWait);
        b.wait_read(&readback);
    }
    b.finish_read(readback, dst);
    Ok(())
}

/// Runs the trait's default `resolve_queries`, which this implementation overrides, as the trait writes it.
fn resolve_queries_by_default(
    b: &mut Device,
    set: &<Device as Gpu>::QuerySet,
    first: u32,
    count: u32,
    dst: &<Device as Gpu>::Buffer,
    dst_offset: u64,
) -> Result<(), GpuError> {
    if !dst_offset.is_multiple_of(8) {
        return Err(GpuError::Invalid(
            "query results go to multiples of 8 bytes",
        ));
    }
    let readback = b.read_queries(set, first, count)?;
    if b.read_state(&readback) == ReadState::Pending {
        b.record_slow_path(SlowPath::QueryResolveWait);
        b.wait_read(&readback);
    }
    let mut bytes = vec![0; product(&[count as usize, 8])];
    b.finish_read(readback, &mut bytes);
    b.write_buffer(dst, dst_offset, &bytes, WriteMode::Ordered);
    Ok(())
}

/// A byte no read back or resolve writes, in every byte a call must leave alone.
const UNTOUCHED: u8 = 0xEE;

/// Reads a 64-byte buffer that a GPU copy wrote, by the override or by the default, and returns all the call did.
fn read_back_now_edge(by_default: bool, offset: u64, size: u64, length: usize) -> String {
    let mut b = device();
    let bytes: Vec<u8> = (0..64).map(|i: u8| i.wrapping_mul(37)).collect();
    let source = buffer(&mut b, 64);
    let copy = buffer(&mut b, 64);
    b.write_buffer(&source, 0, &bytes, WriteMode::Ordered);
    b.copy_buffer(&source, 4, &copy, 0, 60);
    let mut out = vec![UNTOUCHED; length];
    let src = ReadSource::Buffer {
        buffer: &copy,
        offset,
        size,
    };
    let result = if by_default {
        read_back_now_by_default(&mut b, &src, &mut out)
    } else {
        b.read_back_now(&src, &mut out)
    };
    let fence = b.submit();
    b.wait_fence(&fence);
    format!("{result:?} {:?} {out:?}", b.status())
}

#[test]
fn read_back_now_agrees_with_its_default_on_every_edge() {
    // A range past the end is the error `read_back` gives, as the recorder gives it; the device stays usable.
    for (offset, size, length) in [
        (0, 64, 64),
        (3, 5, 5),
        (8, 16, 4),
        (0, 0, 4),
        (64, 0, 0),
        (60, 8, 8),
        (65, 0, 0),
        (u64::MAX, 2, 2),
        (1, u64::MAX, 4),
    ] {
        assert_eq!(
            read_back_now_edge(false, offset, size, length),
            read_back_now_edge(true, offset, size, length),
            "offset {offset}, size {size}, into {length}"
        );
    }
    assert!(read_back_now_edge(false, 60, 8, 8).starts_with("Err(Invalid"));
}

/// Resolves queries of a set of three into a buffer, by the override or by the default, and returns all the call did.
fn resolve_queries_edge(
    by_default: bool,
    first: u32,
    count: u32,
    dst_offset: u64,
    size: u64,
) -> String {
    let mut b = device();
    let set = b
        .create_query_set(QueryKind::Occlusion, 3)
        .expect("create_query_set");
    let dst = buffer(&mut b, size);
    let fill = vec![UNTOUCHED; usize::try_from(size).expect("a test size fits")];
    b.write_buffer(&dst, 0, &fill, WriteMode::Ordered);
    let result = if by_default {
        resolve_queries_by_default(&mut b, &set, first, count, &dst, dst_offset)
    } else {
        b.resolve_queries(&set, first, count, &dst, dst_offset)
    };
    let mut out = fill;
    let fence = b.submit();
    b.wait_fence(&fence);
    let status = b.status();
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &dst,
            offset: 0,
            size,
        },
        &mut out,
    )
    .expect("read_back_now");
    format!("{result:?} {status:?} {out:?}")
}

#[test]
fn resolve_queries_agrees_with_its_default_on_every_edge() {
    // No queries write nothing and answer Ok, as the recorder does; results past the buffer's end lose the device, as
    // the default's `write_buffer` does.
    for (first, count, dst_offset, size) in [
        (0, 3, 0, 24),
        (1, 2, 8, 32),
        (0, 0, 0, 8),
        (3, 0, 0, 8),
        (0, 0, 1024, 8),
        (0, 3, 0, 22),
        (0, 3, 0, 16),
        (2, 2, 0, 32),
        (0, 1, 4, 16),
        (u32::MAX, 2, 0, 16),
    ] {
        assert_eq!(
            resolve_queries_edge(false, first, count, dst_offset, size),
            resolve_queries_edge(true, first, count, dst_offset, size),
            "queries {first} and {count} on, into {dst_offset} of {size} bytes"
        );
    }
}

/// Clears a 64-byte buffer once and returns the device's status and the buffer's bytes.
fn clear_scene(size: u64, pattern: &[u8]) -> (wcontainers_gpu::DeviceStatus, Vec<u8>) {
    let mut b = device();
    let cleared = buffer(&mut b, 64);
    b.write_buffer(&cleared, 0, &[UNTOUCHED; 64], WriteMode::Ordered);
    b.clear_buffer(&cleared, 0, size, pattern);
    let fence = b.submit();
    b.wait_fence(&fence);
    let status = b.status();
    let mut read = vec![0; 64];
    b.read_back_now(
        &ReadSource::Buffer {
            buffer: &cleared,
            offset: 0,
            size: 64,
        },
        &mut read,
    )
    .expect("read_back_now");
    (status, read)
}

#[test]
fn a_clear_of_a_huge_size_or_an_empty_pattern_is_refused_not_fatal() {
    // Each was once fatal: an allocation of the caller's size before any check, or the first byte of no pattern.
    let lost = wcontainers_gpu::DeviceStatus::Lost(wcontainers_gpu::LossReason::Error);
    for (size, pattern) in [
        (1 << 62, &[1, 2][..]),
        (u64::MAX, &[1, 2, 3, 4][..]),
        (u64::MAX - 1, &[1, 2][..]),
        (66, &[1, 2][..]),
        (64, &[][..]),
        (6, &[1, 2, 3, 4][..]),
    ] {
        assert_eq!(
            clear_scene(size, pattern),
            (lost, vec![UNTOUCHED; 64]),
            "{size} bytes of {pattern:?}"
        );
    }
    let (status, read) = clear_scene(64, &[1, 2]);
    assert_eq!(status, wcontainers_gpu::DeviceStatus::Ok);
    assert_eq!(read, [1, 2].repeat(32));
}

/// The bytes of a stream of `Float11_11_10` values whose widened copy, 16 bytes an element, passes 64 MiB.
const LONG_STREAM_BYTES: u64 = 17 << 20;

/// Returns the device's status after a draw of three vertices from a long stream of a format naga lacks.
///
/// The vertices are a list from `first`, or three indices.
fn long_stream_scene(indexed: bool, first: u32) -> String {
    let mut b = device();
    let (_, drawn_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
    let packed = program(&mut b, "packed", "red");
    let mut states = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
    states.layout = b
        .create_vertex_layout(&VertexLayoutDesc {
            streams: std::array::from_fn(|i| {
                (i == 0).then_some(wcontainers_gpu::VertexStream {
                    stride: 4,
                    step: wcontainers_gpu::StepMode::Vertex,
                })
            }),
            attributes: std::array::from_fn(|i| {
                (i == 0).then_some(wcontainers_gpu::VertexAttribute {
                    location: 0,
                    stream: 0,
                    offset: 0,
                    format: wcontainers_gpu::VertexFormat::Float11_11_10,
                })
            }),
        })
        .expect("create_vertex_layout");
    let stream = b
        .create_buffer(&BufferDesc {
            size: LONG_STREAM_BYTES,
            usage: BufferUsage::VERTEX | BufferUsage::INDEX | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .expect("create_buffer");
    // The stream's first words are the indices 0, 1 and 2, which are also small floats.
    let indices: Vec<u8> = [0u32, 1, 2].iter().flat_map(|i| i.to_le_bytes()).collect();
    b.write_buffer(&stream, 0, &indices, WriteMode::Ordered);
    b.begin_pass(&color_pass(&drawn_view, [0; 4]));
    let streams = [Some(wcontainers_gpu::VertexBinding {
        buffer: &stream,
        offset: 0,
    })];
    b.draw(&DrawCall {
        program: &packed,
        blend: &states.blend,
        depth_stencil: &states.depth_stencil,
        raster: &states.raster,
        vertex_layout: &states.layout,
        bindings: [None; 4],
        vertex_buffers: &streams,
        index: indexed.then_some(wcontainers_gpu::IndexBinding {
            buffer: &stream,
            offset: 0,
            format: wcontainers_gpu::IndexFormat::U32,
        }),
        topology: Topology::TriangleList,
        restart: false,
        viewports: &viewport(DRAWN_SIDE),
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: if indexed {
            DrawRange::Indexed {
                first: 0,
                count: 3,
                base_vertex: 0,
                first_instance: 0,
                instances: 1,
            }
        } else {
            DrawRange::Vertices {
                first,
                count: 3,
                first_instance: 0,
                instances: 1,
            }
        },
    });
    b.end_pass();
    let fence = b.submit();
    b.wait_fence(&fence);
    format!("{:?}", b.status())
}

#[test]
fn a_widened_stream_is_converted_as_far_as_the_draw_reads_and_refused_past_its_bound() {
    let healthy = format!("{:?}", device().status());
    // A list of three from the start, or from past the stream's end, converts at most three elements.
    assert_eq!(long_stream_scene(false, 0), healthy);
    assert_eq!(long_stream_scene(false, u32::MAX - 3), healthy);
    // An indexed draw may read every element, whose widened copy would pass the bound, so it is refused.
    assert_ne!(long_stream_scene(true, 0), healthy);
    // So is a list whose elements before its first, which the copy also holds, pass the bound.
    assert_ne!(long_stream_scene(false, 1 << 22), healthy);
}
