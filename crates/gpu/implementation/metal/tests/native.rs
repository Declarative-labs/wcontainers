//! What this implementation does beyond the shared suite: refusals, the features WebGPU on Metal lacks, and fast paths.
//!
//! Each is checked against the recording back end, or against values worked out by hand where it lacks the feature.

#![cfg(target_os = "macos")]

use wcontainers_gpu::{
    Gpu, MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS, RingKind, StandaloneGpu,
    caps::Features,
    draw::{
        AspectOps, ColorAttachment, DepthAttachment, DrawCall, DrawRange, LoadOp, PassDesc,
        StoreOp, Viewport,
    },
    error::GpuError,
    format::Format,
    resource::{
        AddressMode, BindingEntry, BindingResource, BorderColor, BufferDesc, BufferUsage,
        CompareFn, Extent, Filter, MapIntent, Origin, QueryKind, ReadSource, ReadState,
        SamplerDesc, TextureDesc, TextureDimension, TextureRegion, TextureUsage, TextureViewDesc,
        ViewDimension, WriteMode,
    },
    shader::{Program, ShaderModule, ShaderStage, StageSet},
    slow::SlowPath,
    state::{
        BlendComponent, BlendDesc, BlendFactor, BlendOp, ColorWrites, CullMode, DepthStencilDesc,
        FillMode, FrontFace, ProvokingVertex, RasterDesc, RenderTargetBlend, StencilFace,
        StencilOp, Topology, VertexLayoutDesc,
    },
};
use wcontainers_gpu_metal::{MetalGpu, MetalGpuConfig};
use wcontainers_gpu_recording::{Recorder, RecorderConfig};
use wcontainers_metal::StandaloneMetal;
// The steps of `SPIN` that keep one GPU thread busy for tens of milliseconds, as in the Metal trait's own suite.
use wcontainers_metal_conformance::SPIN_STEPS;
use wcontainers_metal_direct::DirectMetal;

/// The side of the square targets the tests draw into: four texels, one for each coordinate the mirrored sample reads.
///
/// https://developer.apple.com/documentation/metal/mtlsampleraddressmode/mirrorclamptoedge
const DRAWN_SIDE: u32 = 4;

/// The bytes of one `rgba8` texel.
const RGBA8_BYTES: usize = wcontainers_gpu::format::FORMAT_FACTS[Format::Rgba8Unorm as usize]
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
    <MetalGpu<DirectMetal> as StandaloneGpu>::new(MetalGpuConfig {
        metal: DirectMetal::new(()),
    })
    .expect("Metal opens")
}

/// Returns the recording back end with `features` added.
fn reference(features: Features) -> Recorder {
    let full = Recorder::new(RecorderConfig { caps: None }).expect("a recorder opens");
    let mut caps = full.caps().clone();
    caps.features |= features;
    Recorder::new(RecorderConfig { caps: Some(caps) }).expect("a recorder opens")
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
        resource: BindingResource::UniformBuffer(wcontainers_gpu::resource::BufferRange {
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
        | Features::POINT_SIZE
        | Features::INDIRECT
        | Features::INDIRECT_FIRST_INSTANCE
        | Features::TIMESTAMPS
        | Features::TEXTURE_LEVEL_QUERY
        | Features::PRIMITIVE_INDEX
        | Features::SHADER_F16
        | Features::VERTEX_STORAGE_WRITE
        | Features::CLIP_DISTANCE
        | Features::FRACTIONAL_VIEWPORT
        | Features::BACKGROUND_PREPARE;
    assert!(device().caps().features.contains(wanted));
    // Depth bounds where Metal has them: macOS 26 on.
    assert_eq!(
        device().caps().features.contains(Features::DEPTH_BOUNDS),
        wcontainers_metal::Metal::supports_depth_test_bounds(&DirectMetal::new(()))
    );
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
    assert_eq!(b.status(), wcontainers_gpu::error::DeviceStatus::Ok);
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
            targets: wcontainers_gpu::state::TargetLayout {
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
    let before = b.slow_paths().counts[wcontainers_gpu::slow::SlowPath::PipelineMiss as usize];
    b.begin_pass(&color_pass(&drawn_view, [0; 4]));
    draw(&mut b, &red, &states, None, DRAWN_SIDE, 3, [0; 2], [0.0; 4]);
    b.end_pass();
    let after = b.slow_paths().counts[wcontainers_gpu::slow::SlowPath::PipelineMiss as usize];
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
        wcontainers_gpu::helpers::Helpers::generate_mips_by_draws(
            &mut b,
            &texture,
            &desc,
            desc.format,
            mips,
            layers,
        )
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
fn clear_scene(size: u64, pattern: &[u8]) -> (wcontainers_gpu::error::DeviceStatus, Vec<u8>) {
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
    let lost =
        wcontainers_gpu::error::DeviceStatus::Lost(wcontainers_gpu::error::LossReason::Error);
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
    assert_eq!(status, wcontainers_gpu::error::DeviceStatus::Ok);
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
                (i == 0).then_some(wcontainers_gpu::state::VertexStream {
                    stride: 4,
                    step: wcontainers_gpu::state::StepMode::Vertex,
                })
            }),
            attributes: std::array::from_fn(|i| {
                (i == 0).then_some(wcontainers_gpu::state::VertexAttribute {
                    location: 0,
                    stream: 0,
                    offset: 0,
                    format: wcontainers_gpu::state::VertexFormat::Float11_11_10,
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
    let streams = [Some(wcontainers_gpu::draw::VertexBinding {
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
        index: indexed.then_some(wcontainers_gpu::draw::IndexBinding {
            buffer: &stream,
            offset: 0,
            format: wcontainers_gpu::state::IndexFormat::U32,
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

/// Steps a generator `words[1]` times in one thread, to keep the GPU busy for tens of milliseconds.
const SPIN: &str = "
@group(0) @binding(0) var<storage, read_write> words: array<u32>;
@compute @workgroup_size(1) fn spin() {
    var x = words[0];
    for (var i = 0u; i < words[1]; i++) {
        x = x * 1664525u + 1013904223u;
    }
    words[2] = x;
}
";

#[test]
fn timestamps_are_nanoseconds_of_the_time_the_cpu_waits() {
    use wcontainers_gpu::{
        draw::{DispatchCall, DispatchGroups},
        resource::BufferRange,
    };
    let mut b = device();
    let set = b
        .create_query_set(QueryKind::Timestamp, 2)
        .expect("create_query_set");
    let words = b
        .create_buffer(&BufferDesc {
            size: 16,
            usage: BufferUsage::STORAGE | BufferUsage::COPY_DST | BufferUsage::COPY_SRC,
            intent: MapIntent::Rarely,
        })
        .expect("create_buffer");
    let start: Vec<u8> = [1, SPIN_STEPS, 0, 0]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    b.write_buffer(&words, 0, &start, WriteMode::Discard);
    let shader = b
        .create_shader(
            &ShaderModule::from_wgsl(SPIN, "spin", ShaderStage::Compute).expect("a program"),
        )
        .expect("create_shader");
    let spin = b
        .create_program(&Program::Compute(&shader))
        .expect("create_program");
    let bound = b
        .create_binding_set(&[BindingEntry {
            binding: 0,
            stages: StageSet::COMPUTE,
            resource: BindingResource::StorageBuffer {
                range: BufferRange {
                    buffer: &words,
                    offset: 0,
                    size: 16,
                },
                writable: true,
            },
        }])
        .expect("create_binding_set");
    b.write_timestamp(&set, 0).expect("write_timestamp");
    b.dispatch(&DispatchCall {
        program: &spin,
        bindings: [Some(&bound), None, None, None],
        immediates: &[],
        groups: DispatchGroups::Direct([1, 1, 1]),
    })
    .expect("dispatch");
    b.write_timestamp(&set, 1).expect("write_timestamp");
    let readback = b.read_queries(&set, 0, 2).expect("read_queries");
    let clock = std::time::Instant::now();
    let fence = b.submit();
    b.wait_fence(&fence);
    let waited = clock.elapsed().as_secs_f64() * 1e9;
    let mut bytes = [0; 16];
    b.finish_read(readback, &mut bytes);
    let (first, second) = bytes.split_at(8);
    let first = u64::from_le_bytes(first.try_into().expect("eight bytes"));
    let second = u64::from_le_bytes(second.try_into().expect("eight bytes"));
    let span = second.saturating_sub(first) as f64;
    eprintln!("timestamps: {span} ns of GPU work in {waited} ns waited");
    assert!(first <= second, "{first} then {second}");
    assert!(
        span <= waited * 1.05 && span >= waited * 0.25,
        "{span} ns of GPU work in {waited} ns waited"
    );
    assert_eq!(b.timestamps_disjoint(), Ok(false));
    assert_eq!(b.status(), wcontainers_gpu::error::DeviceStatus::Ok);
}

/// The side of the depth textures the conversion tests fill: 2^12, so that one holds every 24-bit word.
const DEPTH_SIDE: u32 = 1 << 12;

/// The texels of one of those textures.
const DEPTH_TEXELS: usize = (DEPTH_SIDE as usize) * (DEPTH_SIDE as usize);

/// The conversion tests' programs: depths drawn from float bits, and the bits read of a depth texture.
///
/// A covering triangle's fragments write the depths of the float bits their place numbers from a base, and a compute
/// writes the bits of every float a depth texture holds.
const DEPTH_PROGRAMS: &str = "
struct Base { value: vec4<u32> }
@group(0) @binding(0) var depth: texture_depth_2d;
@group(0) @binding(1) var<storage, read_write> bits: array<u32>;
@group(0) @binding(2) var<uniform> base: Base;
@vertex fn covering(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4<f32>(p[i], 0.5, 1.0);
}
@fragment fn numbered(@builtin(position) at: vec4<f32>) -> @builtin(frag_depth) f32 {
    return bitcast<f32>(base.value.x + u32(at.y) * 4096u + u32(at.x));
}
@compute @workgroup_size(64) fn bits_of(@builtin(global_invocation_id) id: vec3<u32>) {
    bits[id.y * 4096u + id.x] = bitcast<u32>(textureLoad(depth, vec2<u32>(id.xy), 0));
}
";

/// Returns a `DEPTH_SIDE` square texture of `format`, made to be drawn into, sampled and copied.
fn depth_texture<B: Gpu>(b: &mut B, format: Format) -> (B::Texture, B::TextureView) {
    let texture = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: DEPTH_SIDE,
            height: DEPTH_SIDE,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER
                | TextureUsage::SAMPLE
                | TextureUsage::COPY_SRC
                | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        })
        .expect("create_texture");
    let view = view(b, &texture, format, false);
    (texture, view)
}

/// Returns a buffer of `size` bytes that programs may write.
fn storage<B: Gpu>(b: &mut B, size: u64) -> B::Buffer {
    b.create_buffer(&BufferDesc {
        size,
        usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::STORAGE,
        intent: MapIntent::Rarely,
    })
    .expect("create_buffer")
}

/// Returns the whole of a `DEPTH_SIDE` square texture.
fn depth_extent() -> Extent {
    Extent {
        width: DEPTH_SIDE,
        height: DEPTH_SIDE,
        depth_or_layers: 1,
    }
}

/// Returns the tight pitches of a `DEPTH_SIDE` square texture of words, from byte 0.
fn depth_side<B: Gpu>(buffer: &B::Buffer) -> wcontainers_gpu::resource::CopyBufferTexture<'_, B> {
    wcontainers_gpu::resource::CopyBufferTexture {
        buffer,
        offset: 0,
        row_pitch: DEPTH_SIDE * 4,
        rows_per_layer: DEPTH_SIDE,
    }
}

/// Reads `size` bytes of a buffer, after the work so far.
fn read_words<B: Gpu>(b: &mut B, buffer: &B::Buffer, size: u64) -> Vec<u32> {
    let readback = b
        .read_back(&ReadSource::Buffer {
            buffer,
            offset: 0,
            size,
        })
        .expect("read_back");
    b.submit();
    let mut bytes = vec![0; usize::try_from(size).expect("a test's size fits")];
    b.finish_read(readback, &mut bytes);
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect()
}

/// Returns the float the host makes of a 24-bit depth word, as gpu-metal's writes from the host make it.
fn host_float(word: u32) -> u32 {
    ((f64::from(word & 0x00ff_ffff) / 16_777_215.0) as f32).to_bits()
}

#[test]
fn depth_words_copied_in_become_the_floats_the_host_makes_of_them() {
    use wcontainers_gpu::draw::{DispatchCall, DispatchGroups};
    let mut b = device();
    let size = (DEPTH_TEXELS * 4) as u64;
    // Every 24-bit word once, each with a high byte that the copy leaves out.
    let words: Vec<u8> = (0..DEPTH_TEXELS as u32)
        .flat_map(|n| (n | (n.wrapping_mul(0x9e37_79b9) & 0xff00_0000)).to_le_bytes())
        .collect();
    let source = storage(&mut b, size);
    b.write_buffer(&source, 0, &words, WriteMode::Ordered);
    for format in [Format::Depth24Plus, Format::Depth24PlusStencil8] {
        let (texture, view) = depth_texture(&mut b, format);
        b.copy_buffer_to_texture(&depth_side(&source), &whole(&texture), depth_extent());
        let bits = storage(&mut b, size);
        let module =
            |entry, stage| ShaderModule::from_wgsl(DEPTH_PROGRAMS, entry, stage).expect("parses");
        let compute = b
            .create_shader(&module("bits_of", ShaderStage::Compute))
            .expect("create_shader");
        let program = b
            .create_program(&Program::Compute(&compute))
            .expect("create_program");
        let set = b
            .create_binding_set(&[
                BindingEntry {
                    binding: 0,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::Texture(&view),
                },
                BindingEntry {
                    binding: 1,
                    stages: StageSet::COMPUTE,
                    resource: BindingResource::StorageBuffer {
                        range: wcontainers_gpu::resource::BufferRange {
                            buffer: &bits,
                            offset: 0,
                            size,
                        },
                        writable: true,
                    },
                },
            ])
            .expect("create_binding_set");
        b.dispatch(&DispatchCall {
            program: &program,
            bindings: [Some(&set), None, None, None],
            immediates: &[],
            groups: DispatchGroups::Direct([DEPTH_SIDE / 64, DEPTH_SIDE, 1]),
        })
        .expect("dispatch");
        let got = read_words(&mut b, &bits, size);
        let mut wrong = 0usize;
        for (n, got) in got.iter().enumerate() {
            let wanted = host_float(n as u32);
            if *got != wanted {
                if wrong < 8 {
                    eprintln!("{format:?}: word {n:#x} became {got:#x}, not {wanted:#x}");
                }
                wrong += 1;
            }
        }
        assert_eq!(wrong, 0, "{format:?}: words whose float is not the host's");
    }
    assert_eq!(b.status(), wcontainers_gpu::error::DeviceStatus::Ok);
}

/// Returns the 24-bit depth word the host makes of a float's bits, as gpu-metal's reads back make it.
fn host_word(bits: u32) -> u32 {
    let depth = f64::from(f32::from_bits(bits)).clamp(0.0, 1.0);
    (depth * 16_777_215.0).round_ties_even() as u32
}

/// The textures of floats the copy-out test draws: every float from zero to two, a texture of them at a time.
///
/// Each is a base of the float bits [`floats_copied_out_become_the_words_the_host_makes_of_them`] draws.
const FLOAT_CHUNKS: u32 = (1 << 30) / (DEPTH_TEXELS as u32);

#[test]
fn floats_copied_out_become_the_words_the_host_makes_of_them() {
    let mut b = device();
    let size = (DEPTH_TEXELS * 4) as u64;
    let module =
        |entry, stage| ShaderModule::from_wgsl(DEPTH_PROGRAMS, entry, stage).expect("parses");
    let vertex = b
        .create_shader(&module("covering", ShaderStage::Vertex))
        .expect("create_shader");
    let fragment = b
        .create_shader(&module("numbered", ShaderStage::Fragment))
        .expect("create_shader");
    let numbered = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .expect("create_program");
    let states = states(
        &mut b,
        writes_all(None),
        &DepthStencilDesc {
            depth_compare: Some(CompareFn::Always),
            depth_write: true,
            ..no_tests()
        },
        &plain_raster(0.0),
    );
    let base = buffer(&mut b, UNIFORM_BYTES);
    let words = storage(&mut b, size);
    let start = std::time::Instant::now();
    let mut held = std::collections::BTreeSet::new();
    let mut chunks_drawn = 0u32;
    for format in [Format::Depth24Plus, Format::Depth24PlusStencil8] {
        let (texture, view) = depth_texture(&mut b, format);
        let chunks = if format == Format::Depth24Plus {
            0..FLOAT_CHUNKS
        } else {
            // The depth of the combined format, at the floats from 2^-7 to two.
            (0x3c00_0000 / DEPTH_TEXELS as u32)..FLOAT_CHUNKS
        };
        for chunk in chunks {
            let first = chunk * DEPTH_TEXELS as u32;
            b.write_buffer(
                &base,
                0,
                &[first, 0, 0, 0]
                    .iter()
                    .flat_map(|w: &u32| w.to_le_bytes())
                    .collect::<Vec<u8>>(),
                WriteMode::Ordered,
            );
            let set = b
                .create_binding_set(&[BindingEntry {
                    binding: 2,
                    stages: StageSet::FRAGMENT,
                    resource: BindingResource::UniformBuffer(
                        wcontainers_gpu::resource::BufferRange {
                            buffer: &base,
                            offset: 0,
                            size: UNIFORM_BYTES,
                        },
                    ),
                }])
                .expect("create_binding_set");
            b.begin_pass(&PassDesc {
                colors: Default::default(),
                depth: Some(DepthAttachment {
                    view: &view,
                    depth: Some(AspectOps {
                        load: LoadOp::Clear(0.0f32.to_bits()),
                        store: StoreOp::Store,
                        read_only: false,
                    }),
                    stencil: (format == Format::Depth24PlusStencil8).then_some(AspectOps {
                        load: LoadOp::Clear(0),
                        store: StoreOp::Store,
                        read_only: false,
                    }),
                }),
                empty: None,
                occlusion: None,
            });
            b.draw(&DrawCall {
                program: &numbered,
                blend: &states.blend,
                depth_stencil: &states.depth_stencil,
                raster: &states.raster,
                vertex_layout: &states.layout,
                bindings: [Some(&set), None, None, None],
                vertex_buffers: &[],
                index: None,
                topology: Topology::TriangleList,
                restart: false,
                viewports: &[Viewport {
                    x: 0,
                    y: 0,
                    width: (DEPTH_SIDE as f32).to_bits(),
                    height: (DEPTH_SIDE as f32).to_bits(),
                    min_depth: 0,
                    max_depth: 1.0f32.to_bits(),
                }],
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
            b.end_pass();
            b.copy_texture_to_buffer(&whole(&texture), &depth_side(&words), depth_extent());
            let by_gpu = read_words(&mut b, &words, size);
            let mut by_host = vec![0; DEPTH_TEXELS * 4];
            b.submit();
            b.read_back_now(
                &ReadSource::Texture {
                    region: whole(&texture),
                    extent: depth_extent(),
                },
                &mut by_host,
            )
            .expect("read_back_now");
            let by_host: Vec<u32> = by_host
                .as_chunks::<4>()
                .0
                .iter()
                .map(|w| u32::from_le_bytes(*w))
                .collect();
            let wrong = by_gpu.iter().zip(&by_host).filter(|(g, h)| g != h).count();
            if wrong > 0 {
                let (at, (g, h)) = by_gpu
                    .iter()
                    .zip(&by_host)
                    .enumerate()
                    .find(|(_, (g, h))| g != h)
                    .expect("a difference");
                panic!(
                    "{format:?}, floats from {first:#x}: {wrong} words differ, the first at {at}: {g:#x}, not {h:#x}"
                );
            }
            // The texture holds the floats drawn, so each word is the host's of the float its place numbers.
            let unlike = (0..DEPTH_TEXELS)
                .filter(|i| by_host[*i] != host_word(first + *i as u32))
                .count();
            assert_eq!(
                unlike, 0,
                "{format:?}, floats from {first:#x}: words not of the floats drawn"
            );
            held.insert(by_host[0]);
            held.insert(by_host[DEPTH_TEXELS - 1]);
            chunks_drawn += 1;
        }
    }
    eprintln!(
        "{chunks_drawn} textures of floats, {} distinct end words; {} s",
        held.len(),
        start.elapsed().as_secs()
    );
    assert_eq!(chunks_drawn, 68);
    assert_eq!(b.status(), wcontainers_gpu::error::DeviceStatus::Ok);
}

#[test]
fn a_depth_copy_past_its_buffer_or_off_its_words_is_refused_and_writes_nothing() {
    // From byte 4, rows 20 bytes apart: a 4 by 4 copy ends at 4 + 3 * 20 + 16 = 80 bytes.
    for (offset, row_pitch, size, refused) in [
        (4u64, 20u32, 80u64, false),
        (4, 20, 79, true),
        (8, 20, 80, true),
        (2, 20, 96, true),
        (4, 18, 96, true),
        (4, 12, 96, true),
        (u64::MAX - 3, 20, 96, true),
    ] {
        let mut b = device();
        let texture = texture(
            &mut b,
            Format::Depth24Plus,
            DRAWN_SIDE,
            TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            false,
        );
        let into = buffer(&mut b, 96);
        let before = vec![0xa5; 96];
        b.write_buffer(&into, 0, &before, WriteMode::Ordered);
        let short = buffer(&mut b, size);
        for copy_in in [false, true] {
            let side = wcontainers_gpu::resource::CopyBufferTexture {
                buffer: &short,
                offset,
                row_pitch,
                rows_per_layer: DRAWN_SIDE,
            };
            let extent = Extent {
                width: DRAWN_SIDE,
                height: DRAWN_SIDE,
                depth_or_layers: 1,
            };
            if copy_in {
                b.copy_buffer_to_texture(&side, &whole(&texture), extent);
            } else {
                b.copy_texture_to_buffer(&whole(&texture), &side, extent);
            }
            let lost = b.status() != wcontainers_gpu::error::DeviceStatus::Ok;
            assert_eq!(
                lost,
                refused,
                "a copy {} of {size} bytes from {offset} at {row_pitch} apart",
                if copy_in { "in" } else { "out" }
            );
        }
        if !refused {
            // The other buffer is as it was.
            assert_eq!(read_words(&mut b, &into, 96), vec![0xa5a5_a5a5; 24]);
        }
    }
}

/// Returns a program Metal has not compiled before, whose numbers are `salt`'s.
///
/// Its fragments loop over a few sines so that compiling it takes a while.
fn fresh_program(
    b: &mut MetalGpu<DirectMetal>,
    salt: u64,
) -> <MetalGpu<DirectMetal> as Gpu>::Program {
    let source = format!(
        "
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
    var p = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4<f32>(p[i] + vec2(f32({}) * 0.0), 0.5, 1.0);
}}
@fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {{
    var x = at.x;
    for (var j = 0; j < 8; j++) {{ x = sin(x * {}.0 + cos(at.y)); }}
    return vec4<f32>(1.0, x * 0.0, 0.0, 1.0);
}}",
        salt % 1_000_003,
        salt % 999_983 + 1
    );
    let module = |entry, stage| ShaderModule::from_wgsl(&source, entry, stage).expect("parses");
    let vertex = b
        .create_shader(&module("vs", ShaderStage::Vertex))
        .expect("create_shader");
    let fragment = b
        .create_shader(&module("fs", ShaderStage::Fragment))
        .expect("create_shader");
    b.create_program(&Program::Render {
        vertex: &vertex,
        fragment: Some(&fragment),
    })
    .expect("create_program")
}

#[test]
fn prepare_returns_before_metal_compiles_and_the_first_draw_waits_for_it() {
    let mut b = device();
    assert!(b.caps().features.contains(Features::BACKGROUND_PREPARE));
    let states = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
    let salt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after 1970")
        .as_nanos() as u64;
    // More programs than Metal builds at once, so that some are built on the calling thread.
    let programs: Vec<_> = (0..24)
        .map(|k| fresh_program(&mut b, salt.wrapping_add(k)))
        .collect();
    let mut prepares = Vec::new();
    for program in &programs {
        let clock = std::time::Instant::now();
        b.prepare(&wcontainers_gpu::PrepareDesc {
            program,
            blend: &states.blend,
            depth_stencil: &states.depth_stencil,
            raster: &states.raster,
            vertex_layout: &states.layout,
            targets: wcontainers_gpu::state::TargetLayout {
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
        prepares.push(clock.elapsed().as_micros());
    }
    let before = b.slow_paths().counts[wcontainers_gpu::slow::SlowPath::PipelineMiss as usize];
    let (drawn, drawn_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
    let clock = std::time::Instant::now();
    for program in &programs {
        b.begin_pass(&color_pass(&drawn_view, [0; 4]));
        draw(
            &mut b, program, &states, None, DRAWN_SIDE, 3, [0; 2], [0.0; 4],
        );
        b.end_pass();
    }
    let draws = clock.elapsed().as_micros();
    let after = b.slow_paths().counts[wcontainers_gpu::slow::SlowPath::PipelineMiss as usize];
    let mut sorted = prepares.clone();
    sorted.sort_unstable();
    eprintln!(
        "prepare in µs: {prepares:?} (median {}); the draws after: {draws} µs",
        sorted[sorted.len() / 2]
    );
    assert_eq!(after, before, "a draw missed its prepared pipeline");
    let red = [255, 0, 0, 255].repeat((DRAWN_SIDE * DRAWN_SIDE) as usize);
    assert_eq!(read_texture(&mut b, &drawn, DRAWN_SIDE, RGBA8_BYTES), red);
    assert_eq!(b.status(), wcontainers_gpu::error::DeviceStatus::Ok);
}

// The second check's hostile cases: three for each feature of the pass before, each refused before Metal or taken
// whole, with nothing written outside what it names.

/// The usages of the depth textures the hostile cases copy: drawn into, sampled and copied both ways.
const DEPTH_USAGE: TextureUsage = TextureUsage::RENDER
    .union(TextureUsage::SAMPLE)
    .union(TextureUsage::COPY_SRC)
    .union(TextureUsage::COPY_DST);

/// The byte the hostile cases fill a buffer with before a copy or a program writes it, so that a byte changed shows.
const CANARY: u8 = 0xa5;

/// The bytes of one 32-bit word.
const U32_BYTES: u32 = u32::BITS / u8::BITS;

/// Returns whether the device still works, which a refused call ends.
fn works(b: &mut MetalGpu<DirectMetal>) -> bool {
    b.status() == wcontainers_gpu::error::DeviceStatus::Ok
}

/// Makes a 2D texture `side` square of `layers` layers and `levels` levels, or the error that refuses it.
fn square<B: Gpu>(
    b: &mut B,
    format: Format,
    side: u32,
    (layers, levels): (u32, u32),
    usage: TextureUsage,
    reinterpret: bool,
) -> Result<B::Texture, GpuError> {
    b.create_texture(&TextureDesc {
        dimension: TextureDimension::D2,
        format,
        width: side,
        height: side,
        depth_or_layers: layers,
        mip_levels: levels,
        samples: 1,
        usage,
        cube: false,
        reinterpret,
    })
}

/// Returns the description of a view of one level and one layer in `format`.
fn one_view(format: Format, layer: u32) -> TextureViewDesc {
    TextureViewDesc {
        format,
        dimension: ViewDimension::D2,
        base_mip: 0,
        mip_count: 1,
        base_layer: layer,
        layer_count: 1,
        stencil: format == Format::Stencil8,
    }
}

/// Returns the region of a texture's first layer at `level`, from `(x, y)`.
fn at<B: Gpu>(texture: &B::Texture, level: u32, x: u32, y: u32) -> TextureRegion<'_, B> {
    TextureRegion {
        texture,
        mip: level,
        layer: 0,
        origin: Origin { x, y, z: 0 },
        stencil: false,
    }
}

/// Parses and validates a module with `capabilities`, which `ShaderModule::from_wgsl` validates without.
fn module_with(
    source: &str,
    entry: &str,
    stage: ShaderStage,
    capabilities: naga::valid::Capabilities,
) -> ShaderModule {
    use std::hash::{Hash, Hasher};
    let module = naga::front::wgsl::parse_str(source).expect("parses");
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities)
        .validate(&module)
        .expect("validates");
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (source, entry).hash(&mut hasher);
    let mut key = [0; 32];
    for (n, byte) in key.iter_mut().enumerate() {
        *byte = hasher.finish().to_le_bytes()[n % 8];
        n.hash(&mut hasher);
    }
    ShaderModule {
        module,
        info,
        entry_point: entry.into(),
        stage,
        key,
    }
}

/// Fills a buffer with the canary byte.
fn fill_canary<B: Gpu>(b: &mut B, buffer: &B::Buffer, size: u64) {
    let bytes = vec![CANARY; usize::try_from(size).expect("a test's size fits")];
    b.write_buffer(buffer, 0, &bytes, WriteMode::Ordered);
}

/// Returns the bytes of a buffer, after the work so far.
fn read_bytes<B: Gpu>(b: &mut B, buffer: &B::Buffer, size: u64) -> Vec<u8> {
    read_words(b, buffer, size)
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect()
}

#[test]
fn a_depth_copy_whose_layer_spacing_overflows_is_refused() {
    // Rows nearly 2^32 bytes apart and 2^32 - 1 rows to a layer: the second layer starts past 2^64.
    for copy_in in [true, false] {
        let mut b = device();
        let texture = square(
            &mut b,
            Format::Depth24Plus,
            DRAWN_SIDE,
            (2, 1),
            DEPTH_USAGE,
            false,
        )
        .expect("create_texture");
        let words = storage(&mut b, u64::from(DRAWN_BYTES));
        let side = wcontainers_gpu::resource::CopyBufferTexture {
            buffer: &words,
            offset: 0,
            row_pitch: u32::MAX - (U32_BYTES - 1),
            rows_per_layer: u32::MAX,
        };
        let extent = Extent {
            width: DRAWN_SIDE,
            height: DRAWN_SIDE,
            depth_or_layers: 2,
        };
        if copy_in {
            b.copy_buffer_to_texture(&side, &whole(&texture), extent);
        } else {
            b.copy_texture_to_buffer(&whole(&texture), &side, extent);
        }
        assert!(!works(&mut b), "copy in {copy_in}: taken");
    }
}

#[test]
fn a_depth_copy_out_keeps_the_gaps_between_rows_and_layers() {
    let mut b = device();
    let texture = square(
        &mut b,
        Format::Depth24PlusStencil8,
        DRAWN_SIDE,
        (2, 1),
        DEPTH_USAGE,
        false,
    )
    .expect("create_texture");
    let quarter = 0.25f32;
    for layer in 0..2 {
        let view = b
            .create_view(&texture, &one_view(Format::Depth24PlusStencil8, layer))
            .expect("create_view");
        b.begin_pass(&PassDesc {
            colors: Default::default(),
            depth: Some(DepthAttachment {
                view: &view,
                depth: Some(AspectOps {
                    load: LoadOp::Clear(quarter.to_bits()),
                    store: StoreOp::Store,
                    read_only: false,
                }),
                stencil: Some(AspectOps {
                    load: LoadOp::Clear(u8::MAX),
                    store: StoreOp::Store,
                    read_only: false,
                }),
            }),
            empty: None,
            occlusion: None,
        });
        b.end_pass();
    }
    // Rows of four words six words apart, five rows to a layer (one spare), from word 3, and spare words after.
    let (first, pitch, rows, after) = (3u32, 6u32, 5u32, 8u32);
    let words = first + pitch * rows * 2 + after;
    let size = u64::from(words * U32_BYTES);
    let out = storage(&mut b, size);
    fill_canary(&mut b, &out, size);
    let side = wcontainers_gpu::resource::CopyBufferTexture {
        buffer: &out,
        offset: u64::from(first * U32_BYTES),
        row_pitch: pitch * U32_BYTES,
        rows_per_layer: rows,
    };
    let extent = Extent {
        width: DRAWN_SIDE,
        height: DRAWN_SIDE,
        depth_or_layers: 2,
    };
    b.copy_texture_to_buffer(&whole(&texture), &side, extent);
    assert!(works(&mut b), "a copy within its buffer refused");
    let got = read_words(&mut b, &out, size);
    let wanted = host_word(quarter.to_bits());
    let canary = u32::from_le_bytes([CANARY; 4]);
    let mut texels = 0;
    for (n, word) in got.iter().enumerate() {
        let inside = (n as u32).checked_sub(first).is_some_and(|rel| {
            let (layer, rest) = (rel / (pitch * rows), rel % (pitch * rows));
            layer < 2 && rest / pitch < DRAWN_SIDE && rest % pitch < DRAWN_SIDE
        });
        if inside {
            assert_eq!(*word, wanted, "texel word {n}");
            texels += 1;
        } else {
            assert_eq!(*word, canary, "gap word {n} changed");
        }
    }
    assert_eq!(texels, DRAWN_SIDE * DRAWN_SIDE * 2);
    assert!(works(&mut b));
}

#[test]
fn depth_copies_at_the_edges_of_levels_and_buffers_are_refused_or_taken_whole() {
    let one = Extent {
        width: 1,
        height: 1,
        depth_or_layers: 1,
    };
    // Level, origin, extent, offset, refused: past level 1's two by two, from past level 0's side, of no width, and
    // the last word of the buffer exactly.
    let buffer_bytes = DRAWN_BYTES;
    let cases = [
        (
            1,
            (0, 0),
            Extent {
                width: 3,
                height: 2,
                depth_or_layers: 1,
            },
            0,
            true,
        ),
        (0, (DRAWN_SIDE, 0), one, 0, true),
        (0, (0, 0), Extent { width: 0, ..one }, 0, true),
        (1, (1, 1), one, buffer_bytes - U32_BYTES, false),
    ];
    for (n, (level, (x, y), extent, offset, refused)) in cases.into_iter().enumerate() {
        for copy_in in [true, false] {
            let mut b = device();
            let texture = square(
                &mut b,
                Format::Depth24Plus,
                DRAWN_SIDE,
                (1, 3),
                DEPTH_USAGE,
                false,
            )
            .expect("create_texture");
            let words = storage(&mut b, u64::from(buffer_bytes));
            fill_canary(&mut b, &words, u64::from(buffer_bytes));
            let side = wcontainers_gpu::resource::CopyBufferTexture {
                buffer: &words,
                offset: u64::from(offset),
                row_pitch: DRAWN_ROW_BYTES,
                rows_per_layer: DRAWN_SIDE,
            };
            let region = at(&texture, level, x, y);
            if copy_in {
                b.copy_buffer_to_texture(&side, &region, extent);
            } else {
                b.copy_texture_to_buffer(&region, &side, extent);
            }
            assert_eq!(!works(&mut b), refused, "case {n}, copy in {copy_in}");
            if !refused && !copy_in {
                // Only the last word changed.
                let got = read_bytes(&mut b, &words, u64::from(buffer_bytes));
                let last = (buffer_bytes - U32_BYTES) as usize;
                assert!(got[..last].iter().all(|byte| *byte == CANARY), "case {n}");
            }
        }
    }
}

#[test]
fn a_bgra4_texture_made_to_be_drawn_into_or_stored_to_is_refused() {
    for usage in [TextureUsage::RENDER, TextureUsage::STORAGE] {
        let mut b = device();
        let made = square(
            &mut b,
            Format::Bgra4Unorm,
            DRAWN_SIDE,
            (1, 1),
            usage | TextureUsage::SAMPLE,
            false,
        );
        assert!(made.is_err(), "{usage:?}: made");
        assert!(works(&mut b));
    }
}

#[test]
fn a_bgra4_view_of_a_drawn_texture_and_a_pass_into_a_view_of_a_bgra4_texture_are_refused() {
    let mut b = device();
    let drawn = square(
        &mut b,
        Format::B5g6r5Unorm,
        DRAWN_SIDE,
        (1, 1),
        TextureUsage::RENDER | TextureUsage::SAMPLE,
        true,
    )
    .expect("create_texture");
    assert!(
        b.create_view(&drawn, &one_view(Format::Bgra4Unorm, 0))
            .is_err(),
        "a swizzled view of a texture drawn into"
    );
    let sampled = square(
        &mut b,
        Format::Bgra4Unorm,
        DRAWN_SIDE,
        (1, 1),
        TextureUsage::SAMPLE | TextureUsage::COPY_DST,
        true,
    )
    .expect("create_texture");
    let other = b
        .create_view(&sampled, &one_view(Format::B5g6r5Unorm, 0))
        .expect("a view of a sampled texture in another 16-bit format");
    assert!(works(&mut b));
    b.begin_pass(&color_pass(&other, [0; 4]));
    b.end_pass();
    assert!(
        !works(&mut b),
        "a pass into a texture not made to be drawn into"
    );
}

#[test]
fn copies_between_bgra4_and_another_format_and_its_mipmaps_are_refused() {
    let usage = TextureUsage::SAMPLE | TextureUsage::COPY_DST | TextureUsage::COPY_SRC;
    let mut b = device();
    let bgra4 = square(&mut b, Format::Bgra4Unorm, DRAWN_SIDE, (1, 1), usage, false)
        .expect("create_texture");
    let other = square(
        &mut b,
        Format::B5g6r5Unorm,
        DRAWN_SIDE,
        (1, 1),
        usage,
        false,
    )
    .expect("create_texture");
    let extent = Extent {
        width: DRAWN_SIDE,
        height: DRAWN_SIDE,
        depth_or_layers: 1,
    };
    b.copy_texture(&whole(&bgra4), &whole(&other), extent);
    b.submit();
    assert!(!works(&mut b), "a copy between two formats");
    let mut b = device();
    let levels = 3;
    let desc = TextureDesc {
        dimension: TextureDimension::D2,
        format: Format::Bgra4Unorm,
        width: DRAWN_SIDE,
        height: DRAWN_SIDE,
        depth_or_layers: 1,
        mip_levels: levels,
        samples: 1,
        usage,
        cube: false,
        reinterpret: false,
    };
    let mipped = b.create_texture(&desc).expect("create_texture");
    let made = b.generate_mips(&mipped, &desc, Format::Bgra4Unorm, 0..levels, 0..1);
    assert!(made.is_err() || !works(&mut b), "mipmaps drawn into bgra4");
}

/// Returns a depth block that keeps fragments whose stored depth lies from `low` to `high`, as bits.
fn bounded(low: u32, high: u32) -> DepthStencilDesc {
    DepthStencilDesc {
        depth_compare: Some(CompareFn::Always),
        depth_bounds: Some([low, high]),
        ..no_tests()
    }
}

#[test]
fn depth_bounds_at_the_edges_of_zero_to_one_are_taken_and_just_past_them_refused() {
    let mut b = device();
    let tiny = f32::from_bits(1);
    let above_one = f32::from_bits(1.0f32.to_bits() + 1);
    for (low, high, taken) in [
        (-0.0f32, 0.0f32, true),
        (tiny, tiny, true),
        (1.0, 1.0, true),
        (0.5, 0.5, true),
        (0.0, above_one, false),
        (-tiny, 0.5, false),
    ] {
        let made = b.create_depth_stencil(&bounded(low.to_bits(), high.to_bits()));
        assert_eq!(made.is_ok(), taken, "[{low:e}, {high:e}]");
        let Ok(depth_stencil) = made else { continue };
        let mut drawing = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
        drawing.depth_stencil = depth_stencil;
        let red = program(&mut b, "covering", "red");
        let depth = square(
            &mut b,
            Format::Depth32Float,
            DRAWN_SIDE,
            (1, 1),
            DEPTH_USAGE,
            false,
        )
        .expect("create_texture");
        let depth_view = view(&mut b, &depth, Format::Depth32Float, false);
        let (_color, color_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
        let mut pass = color_pass(&color_view, [0; 4]);
        pass.depth = Some(DepthAttachment {
            view: &depth_view,
            depth: Some(AspectOps {
                load: LoadOp::Clear(0.5f32.to_bits()),
                store: StoreOp::Store,
                read_only: false,
            }),
            stencil: None,
        });
        b.begin_pass(&pass);
        draw(
            &mut b, &red, &drawing, None, DRAWN_SIDE, 3, [0; 2], [0.0; 4],
        );
        b.end_pass();
        b.submit();
        assert!(works(&mut b), "[{low:e}, {high:e}] drawn");
    }
}

#[test]
fn depth_bounds_in_a_pass_without_depth_are_left_unset_or_refused() {
    // Stencil only: the bounds have nothing to test, so the draw draws. The combined format with no depth aspect named:
    // the pass holds no depth, which the pipeline was made with, so the draw is refused before Metal.
    for (format, drawn) in [
        (Format::Stencil8, true),
        (Format::Depth24PlusStencil8, false),
    ] {
        let mut b = device();
        let depth_stencil = b
            .create_depth_stencil(&DepthStencilDesc {
                depth_compare: None,
                ..bounded(0.25f32.to_bits(), 0.75f32.to_bits())
            })
            .expect("create_depth_stencil");
        let mut drawing = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
        drawing.depth_stencil = depth_stencil;
        let red = program(&mut b, "covering", "red");
        let texture =
            square(&mut b, format, DRAWN_SIDE, (1, 1), DEPTH_USAGE, false).expect("create_texture");
        let attached = b
            .create_view(&texture, &one_view(format, 0))
            .expect("create_view");
        let (_color, color_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
        let mut pass = color_pass(&color_view, [0; 4]);
        pass.depth = Some(DepthAttachment {
            view: &attached,
            depth: None,
            stencil: Some(AspectOps {
                load: LoadOp::Clear(0),
                store: StoreOp::Store,
                read_only: false,
            }),
        });
        b.begin_pass(&pass);
        draw(
            &mut b, &red, &drawing, None, DRAWN_SIDE, 3, [0; 2], [0.0; 4],
        );
        b.end_pass();
        b.submit();
        assert_eq!(works(&mut b), drawn, "{format:?}");
    }
}

#[test]
fn depth_bounds_of_nan_payloads_and_infinities_are_refused() {
    let mut b = device();
    let (half, one) = (0.5f32.to_bits(), 1.0f32.to_bits());
    // A NaN with the sign bit and a payload, a NaN payload above, and the infinities.
    let signed_nan = f32::NAN.to_bits() | 1 << 31 | 1;
    let payload_nan = f32::INFINITY.to_bits() | 1;
    for (low, high) in [
        (signed_nan, half),
        (0, payload_nan),
        (f32::NEG_INFINITY.to_bits(), one),
        (0, f32::INFINITY.to_bits()),
    ] {
        assert!(
            b.create_depth_stencil(&bounded(low, high)).is_err(),
            "[{low:#x}, {high:#x}] made"
        );
    }
    assert!(works(&mut b));
}

/// A vertex program that writes words at wild indices of a storage binding.
///
/// It writes one before the binding, one at 2^30, and one counted down from the last `u32`.
const WILD_WRITES: &str = "
@group(0) @binding(0) var<storage, read_write> words: array<u32>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    words[i32(i) - 1] = 0x11111111u + i;
    words[1u << 30u] = 0x22222222u;
    words[0xffffffffu - i] = 0x33333333u;
    var p = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4<f32>(p[i % 3u], 0.5, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
";

/// The offset of the storage bindings the wild writes go through, past the buffer's start so writes before it show.
const BOUND_AT: u64 = 256;

/// The bytes of the buffers the wild writes are bound within.
const WILD_BYTES: u64 = 1024;

/// Returns the bytes of `got` outside `bound` bytes from `BOUND_AT` that are not the canary.
fn changed_outside(got: &[u8], bound: u64) -> usize {
    let inside = BOUND_AT as usize..(BOUND_AT + bound) as usize;
    got.iter()
        .enumerate()
        .filter(|(n, byte)| !inside.contains(n) && **byte != CANARY)
        .count()
}

#[test]
fn vertex_storage_writes_at_wild_indices_stay_in_their_word() {
    let mut b = device();
    let module = |entry, stage| {
        module_with(
            WILD_WRITES,
            entry,
            stage,
            naga::valid::Capabilities::empty(),
        )
    };
    let vertex = b
        .create_shader(&module("vs", ShaderStage::Vertex))
        .expect("create_shader");
    let fragment = b
        .create_shader(&module("fs", ShaderStage::Fragment))
        .expect("create_shader");
    let wild = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .expect("create_program");
    let words = storage(&mut b, WILD_BYTES);
    fill_canary(&mut b, &words, WILD_BYTES);
    let set = b
        .create_binding_set(&[BindingEntry {
            binding: 0,
            stages: StageSet::VERTEX,
            resource: BindingResource::StorageBuffer {
                range: wcontainers_gpu::resource::BufferRange {
                    buffer: &words,
                    offset: BOUND_AT,
                    size: u64::from(U32_BYTES),
                },
                writable: true,
            },
        }])
        .expect("create_binding_set");
    let drawing = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
    let (_color, color_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
    b.begin_pass(&color_pass(&color_view, [0; 4]));
    draw(
        &mut b,
        &wild,
        &drawing,
        Some(&set),
        DRAWN_SIDE,
        3,
        [0; 2],
        [0.0; 4],
    );
    b.end_pass();
    let got = read_bytes(&mut b, &words, WILD_BYTES);
    assert!(works(&mut b));
    assert_eq!(changed_outside(&got, u64::from(U32_BYTES)), 0);
}

#[test]
fn sixteen_bit_storage_of_an_odd_number_of_halves_stays_inside_its_binding() {
    const HALVES: &str = "
enable f16;
@group(0) @binding(0) var<storage, read_write> halves: array<f16>;
@compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    halves[id.x] = f16(1.0) + f16(id.x);
}
";
    let mut b = device();
    let compute = b
        .create_shader(&module_with(
            HALVES,
            "main",
            ShaderStage::Compute,
            naga::valid::Capabilities::SHADER_FLOAT16,
        ))
        .expect("create_shader");
    let program = b
        .create_program(&Program::Compute(&compute))
        .expect("create_program");
    let halves = storage(&mut b, WILD_BYTES);
    // Three halves, then one.
    for bound in [6u64, 2] {
        fill_canary(&mut b, &halves, WILD_BYTES);
        let set = b
            .create_binding_set(&[BindingEntry {
                binding: 0,
                stages: StageSet::COMPUTE,
                resource: BindingResource::StorageBuffer {
                    range: wcontainers_gpu::resource::BufferRange {
                        buffer: &halves,
                        offset: BOUND_AT,
                        size: bound,
                    },
                    writable: true,
                },
            }])
            .expect("create_binding_set");
        b.dispatch(&wcontainers_gpu::draw::DispatchCall {
            program: &program,
            bindings: [Some(&set), None, None, None],
            immediates: &[],
            groups: wcontainers_gpu::draw::DispatchGroups::Direct([2, 1, 1]),
        })
        .expect("dispatch");
        let got = read_bytes(&mut b, &halves, WILD_BYTES);
        assert!(works(&mut b), "{bound} bytes");
        assert_eq!(changed_outside(&got, bound), 0, "{bound} bytes");
    }
}

#[test]
fn wild_fractional_viewports_draw_and_keep_the_device() {
    let side = DRAWN_SIDE as f32;
    // Far outside the target, of negative width, empty, of the smallest width, wider than the target by far, and of
    // the largest height.
    for (n, (x, y, width, height)) in [
        (1e30, 0.5, side, side),
        (0.25, 0.25, -side, side),
        (0.5, 0.5, 0.0, 0.0),
        (0.0, 0.0, f32::from_bits(1), side),
        (-1e30, -1e30, 3e30, 3e30),
        (0.5, 0.5, side, f32::MAX),
    ]
    .into_iter()
    .enumerate()
    {
        let mut b = device();
        let red = program(&mut b, "covering", "red");
        let drawing = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
        let (_color, color_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
        b.begin_pass(&color_pass(&color_view, [0; 4]));
        b.draw(&DrawCall {
            program: &red,
            blend: &drawing.blend,
            depth_stencil: &drawing.depth_stencil,
            raster: &drawing.raster,
            vertex_layout: &drawing.layout,
            bindings: [None; 4],
            vertex_buffers: &[],
            index: None,
            topology: Topology::TriangleList,
            restart: false,
            viewports: &[Viewport {
                x: f32::to_bits(x),
                y: f32::to_bits(y),
                width: width.to_bits(),
                height: height.to_bits(),
                min_depth: 0,
                max_depth: 1.0f32.to_bits(),
            }],
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
        b.end_pass();
        b.submit();
        assert!(works(&mut b), "viewport {n}");
    }
}

/// Returns nanoseconds since 1970, which makes the programs of one run differ from those Metal has cached.
fn salt() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after 1970")
        .as_nanos() as u64
}

/// Returns the targets of one color attachment of `color` and an optional depth attachment.
fn targets_of(
    color: Format,
    depth: Option<Format>,
    samples: u32,
) -> wcontainers_gpu::state::TargetLayout {
    let mut colors = [None; MAX_COLOR_TARGETS];
    colors[0] = Some(color);
    wcontainers_gpu::state::TargetLayout {
        colors,
        depth_stencil: depth,
        samples,
    }
}

/// Prepares `program` with `states` for `targets`, drawn as lists.
fn prepare_for(
    b: &mut MetalGpu<DirectMetal>,
    program: &<MetalGpu<DirectMetal> as Gpu>::Program,
    drawing: &States<MetalGpu<DirectMetal>>,
    targets: wcontainers_gpu::state::TargetLayout,
) {
    b.prepare(&wcontainers_gpu::PrepareDesc {
        program,
        blend: &drawing.blend,
        depth_stencil: &drawing.depth_stencil,
        raster: &drawing.raster,
        vertex_layout: &drawing.layout,
        targets,
        topology: Topology::TriangleList,
        layouts: Default::default(),
    });
}

#[test]
fn a_device_dropped_while_its_prepares_build_leaves_the_next_device_drawing() {
    let base = salt();
    {
        let mut b = device();
        let drawing = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
        // Many more than Metal builds at once.
        let programs: Vec<_> = (0..64)
            .map(|k| fresh_program(&mut b, base.wrapping_add(k)))
            .collect();
        for program in &programs {
            prepare_for(
                &mut b,
                program,
                &drawing,
                targets_of(Format::Rgba8Unorm, None, 1),
            );
        }
        assert!(works(&mut b));
    }
    let mut b = device();
    let red = program(&mut b, "covering", "red");
    let drawing = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
    let (color, color_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
    b.begin_pass(&color_pass(&color_view, [0; 4]));
    draw(
        &mut b, &red, &drawing, None, DRAWN_SIDE, 3, [0; 2], [0.0; 4],
    );
    b.end_pass();
    let reds = [255, 0, 0, 255].repeat((DRAWN_SIDE * DRAWN_SIDE) as usize);
    assert_eq!(read_texture(&mut b, &color, DRAWN_SIDE, RGBA8_BYTES), reds);
    assert!(works(&mut b));
}

#[test]
fn prepares_of_combinations_metal_refuses_keep_the_device() {
    let mut b = device();
    let fresh = fresh_program(&mut b, salt());
    let drawing = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
    let empty = b
        .create_shader(
            &ShaderModule::from_wgsl(
                "@compute @workgroup_size(1) fn main() {}",
                "main",
                ShaderStage::Compute,
            )
            .expect("parses"),
        )
        .expect("create_shader");
    let compute = b
        .create_program(&Program::Compute(&empty))
        .expect("create_program");
    for (name, program, targets) in [
        (
            "bgra4 color",
            &fresh,
            targets_of(Format::Bgra4Unorm, None, 1),
        ),
        (
            "three samples",
            &fresh,
            targets_of(Format::Rgba8Unorm, None, 3),
        ),
        (
            "depth as color",
            &fresh,
            targets_of(Format::Depth32Float, None, 1),
        ),
        (
            "color as depth",
            &fresh,
            targets_of(Format::Rgba8Unorm, Some(Format::Rgba8Unorm), 1),
        ),
        ("compute", &compute, targets_of(Format::Rgba8Unorm, None, 1)),
        (
            "no samples",
            &fresh,
            targets_of(Format::Rgba8Unorm, None, 0),
        ),
    ] {
        prepare_for(&mut b, program, &drawing, targets);
        assert!(works(&mut b), "{name}");
    }
    let (color, color_view) = target(&mut b, Format::Rgba8Unorm, DRAWN_SIDE);
    b.begin_pass(&color_pass(&color_view, [0; 4]));
    draw(
        &mut b, &fresh, &drawing, None, DRAWN_SIDE, 3, [0; 2], [0.0; 4],
    );
    b.end_pass();
    let reds = [255, 0, 0, 255].repeat((DRAWN_SIDE * DRAWN_SIDE) as usize);
    assert_eq!(read_texture(&mut b, &color, DRAWN_SIDE, RGBA8_BYTES), reds);
    assert!(works(&mut b));
}

#[test]
fn repeated_and_crossed_prepares_all_draw_without_a_miss() {
    let mut b = device();
    let base = salt();
    let programs: Vec<_> = (0..3)
        .map(|k| fresh_program(&mut b, base.wrapping_add(k)))
        .collect();
    let drawing = states(&mut b, writes_all(None), &no_tests(), &plain_raster(0.0));
    let formats = [
        Format::Rgba8Unorm,
        Format::Bgra8Unorm,
        Format::Rgba16Float,
        Format::R8Unorm,
        Format::Rg8Unorm,
        Format::Rgba8UnormSrgb,
        Format::R16Float,
        Format::Rg16Float,
        Format::Rgba32Float,
        Format::R32Float,
    ];
    // One combination a hundred times, then every program for every format.
    for _ in 0..100 {
        prepare_for(
            &mut b,
            &programs[0],
            &drawing,
            targets_of(formats[0], None, 1),
        );
    }
    for program in &programs {
        for format in formats {
            prepare_for(&mut b, program, &drawing, targets_of(format, None, 1));
        }
    }
    let before = b.slow_paths().counts[SlowPath::PipelineMiss as usize];
    for program in programs.iter().rev() {
        for format in formats.iter().rev() {
            let (_color, color_view) = target(&mut b, *format, DRAWN_SIDE);
            b.begin_pass(&color_pass(&color_view, [0; 4]));
            draw(
                &mut b, program, &drawing, None, DRAWN_SIDE, 3, [0; 2], [0.0; 4],
            );
            b.end_pass();
        }
    }
    b.submit();
    let after = b.slow_paths().counts[SlowPath::PipelineMiss as usize];
    assert_eq!(after, before, "a draw missed its prepared pipeline");
    assert!(works(&mut b));
}

/// The capture tests' program, whose vertex index comes in a structure beside its position.
///
/// It writes the index, the position doubled and the instance negated.
const CAPTURED: &str = "
struct In { @location(0) p: vec2<f32>, @builtin(vertex_index) v: u32 }
struct V {
    @builtin(position) at: vec4<f32>,
    @location(0) @interpolate(flat) v: u32,
    @location(1) p: vec2<f32>,
    @location(2) @interpolate(flat) i: i32,
}
@vertex fn vs(input: In, @builtin(instance_index) i: u32) -> V {
    return V(vec4<f32>(input.p, 0.5, 1.0), input.v, input.p * 2.0, -i32(i));
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
";

/// The components of each of the capture program's outputs.
const CAPTURED_COMPONENTS: [u32; 3] = [1, 2, 1];

/// The vertices of the capture tests' stream.
const CAPTURED_VERTICES: u32 = 8;

/// The bytes of each capture target of the capture tests, more than any of their draws writes.
const CAPTURE_TARGET_BYTES: u64 = 4096;

/// What a capture test draws: the program, its states, its stream and its layout.
struct CaptureScene<B: Gpu> {
    program: B::Program,
    states: States<B>,
    stream: B::Buffer,
    layout: B::VertexLayout,
}

/// Makes the capture tests' program, states and a stream of `CAPTURED_VERTICES` positions.
fn capture_scene<B: Gpu>(b: &mut B) -> CaptureScene<B> {
    let module = |entry, stage| ShaderModule::from_wgsl(CAPTURED, entry, stage).expect("parses");
    let vertex = b
        .create_shader(&module("vs", ShaderStage::Vertex))
        .expect("create_shader");
    let fragment = b
        .create_shader(&module("fs", ShaderStage::Fragment))
        .expect("create_shader");
    let program = b
        .create_program(&Program::Render {
            vertex: &vertex,
            fragment: Some(&fragment),
        })
        .expect("create_program");
    let states = states(b, writes_all(None), &no_tests(), &plain_raster(0.0));
    let mut layout = VertexLayoutDesc {
        streams: [None; MAX_VERTEX_STREAMS],
        attributes: [None; MAX_VERTEX_STREAMS],
    };
    let pair = 2 * U32_BYTES;
    layout.streams[0] = Some(wcontainers_gpu::state::VertexStream {
        stride: pair,
        step: wcontainers_gpu::state::StepMode::Vertex,
    });
    layout.attributes[0] = Some(wcontainers_gpu::state::VertexAttribute {
        location: 0,
        stream: 0,
        offset: 0,
        format: wcontainers_gpu::state::VertexFormat::Float32x2,
    });
    let layout = b
        .create_vertex_layout(&layout)
        .expect("create_vertex_layout");
    let stream = b
        .create_buffer(&BufferDesc {
            size: u64::from(CAPTURED_VERTICES * pair),
            usage: BufferUsage::VERTEX | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .expect("create_buffer");
    let positions: Vec<u8> = (0..CAPTURED_VERTICES * 2)
        .flat_map(|n| (n as f32 * 0.125 - 1.0).to_le_bytes())
        .collect();
    b.write_buffer(&stream, 0, &positions, WriteMode::Ordered);
    CaptureScene {
        program,
        states,
        stream,
        layout,
    }
}

/// Makes a capture target of `CAPTURE_TARGET_BYTES`, filled with the canary byte.
fn capture_target<B: Gpu>(b: &mut B) -> B::Buffer {
    let target = b
        .create_buffer(&BufferDesc {
            size: CAPTURE_TARGET_BYTES,
            usage: BufferUsage::CAPTURE | BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .expect("create_buffer");
    fill_canary(b, &target, CAPTURE_TARGET_BYTES);
    target
}

/// Captures `range` of the scene drawn as `topology` into `targets` targets outside a pass, and returns what each holds.
fn captured<B: Gpu>(
    b: &mut B,
    topology: Topology,
    range: DrawRange<'_, B>,
    targets: usize,
) -> Result<Vec<Vec<u8>>, GpuError> {
    let scene = capture_scene(b);
    let buffers: Vec<B::Buffer> = (0..targets).map(|_| capture_target(b)).collect();
    let streams = [Some(wcontainers_gpu::draw::VertexBinding {
        buffer: &scene.stream,
        offset: 0,
    })];
    let call = DrawCall {
        program: &scene.program,
        blend: &scene.states.blend,
        depth_stencil: &scene.states.depth_stencil,
        raster: &scene.states.raster,
        vertex_layout: &scene.layout,
        bindings: [None; 4],
        vertex_buffers: &streams,
        index: None,
        topology,
        restart: false,
        viewports: &viewport(DRAWN_SIDE),
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range,
    };
    let ranges: Vec<_> = buffers
        .iter()
        .map(|buffer| wcontainers_gpu::resource::BufferRange {
            buffer,
            offset: 0,
            size: CAPTURE_TARGET_BYTES,
        })
        .collect();
    b.capture_vertices(&call, &ranges, false)?;
    Ok(buffers
        .iter()
        .map(|buffer| {
            let readback = b
                .read_back(&ReadSource::Buffer {
                    buffer,
                    offset: 0,
                    size: CAPTURE_TARGET_BYTES,
                })
                .expect("read_back");
            b.submit();
            let mut bytes = vec![0; CAPTURE_TARGET_BYTES as usize];
            b.finish_read(readback, &mut bytes);
            bytes
        })
        .collect())
}

/// Returns the recording back end with capture switched on.
fn capturing_reference() -> Recorder {
    reference(Features::TRANSFORM_FEEDBACK)
}

#[test]
fn captures_as_the_reference_does_in_every_list_and_strip() {
    for topology in [
        Topology::PointList,
        Topology::LineList,
        Topology::LineStrip,
        Topology::TriangleList,
        Topology::TriangleStrip,
    ] {
        for count in [1, 2, 5, 7] {
            for targets in [1, CAPTURED_COMPONENTS.len()] {
                let mut b = device();
                let got =
                    captured(&mut b, topology, from_one(count), targets).expect("capture_vertices");
                assert!(works(&mut b));
                let mut r = capturing_reference();
                let wanted =
                    captured(&mut r, topology, from_one(count), targets).expect("capture_vertices");
                assert_eq!(got, wanted, "{topology:?} of {count}, {targets} targets");
            }
        }
    }
}

#[test]
fn captures_of_fans_indirect_draws_and_compute_programs_are_refused() {
    let mut b = device();
    assert_eq!(
        captured(&mut b, Topology::TriangleFan, vertices_of(4), 1).err(),
        Some(GpuError::Absent(Features::FAN_AND_LOOP))
    );
    let args = buffer(&mut b, 16);
    assert!(matches!(
        captured(
            &mut b,
            Topology::TriangleList,
            DrawRange::Indirect {
                args: &args,
                offset: 0,
            },
            1,
        ),
        Err(GpuError::Invalid(_))
    ));
    assert!(works(&mut b));
}

/// Returns a range of `count` vertices from vertex 1, two instances from instance 3.
fn from_one<'a, B: Gpu>(count: u32) -> DrawRange<'a, B> {
    DrawRange::Vertices {
        first: 1,
        count,
        first_instance: 3,
        instances: 2,
    }
}

/// Returns a range of `count` vertices from vertex 0, one instance.
fn vertices_of<'a, B: Gpu>(count: u32) -> DrawRange<'a, B> {
    DrawRange::Vertices {
        first: 0,
        count,
        first_instance: 0,
        instances: 1,
    }
}

#[test]
fn a_capture_past_its_targets_off_a_word_or_into_what_it_reads_is_refused_and_writes_nothing() {
    let mut b = device();
    let scene = capture_scene(&mut b);
    let target = capture_target(&mut b);
    let streams = [Some(wcontainers_gpu::draw::VertexBinding {
        buffer: &scene.stream,
        offset: 0,
    })];
    let call = |range| DrawCall {
        program: &scene.program,
        blend: &scene.states.blend,
        depth_stencil: &scene.states.depth_stencil,
        raster: &scene.states.raster,
        vertex_layout: &scene.layout,
        bindings: [None; 4],
        vertex_buffers: &streams,
        index: None,
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range,
    };
    // Three vertices of four words each.
    let whole = u64::from(3 * 4 * U32_BYTES);
    let at = |buffer, offset, size| wcontainers_gpu::resource::BufferRange {
        buffer,
        offset,
        size,
    };
    for (name, targets) in [
        (
            "one word short",
            vec![at(&target, 0, whole - u64::from(U32_BYTES))],
        ),
        ("off a word", vec![at(&target, 2, whole)]),
        (
            "past the buffer",
            vec![at(&target, CAPTURE_TARGET_BYTES - 4, whole)],
        ),
        (
            "past the largest offset",
            vec![at(&target, u64::MAX - 3, whole)],
        ),
        ("into its own stream", vec![at(&scene.stream, 0, whole)]),
        (
            "two targets of three outputs",
            vec![at(&target, 0, whole); 2],
        ),
        (
            "more targets than capture_buffers",
            vec![at(&target, 0, whole); 5],
        ),
        ("no targets", vec![]),
    ] {
        let refused = b.capture_vertices(&call(vertices_of(3)), &targets, false);
        assert!(
            matches!(refused, Err(GpuError::Invalid(_))),
            "{name}: {refused:?}"
        );
    }
    // A capture that rasterises outside a pass, and a draw of u32::MAX vertices into one small target, refused before
    // the vertices are listed.
    let small = [at(&target, 0, whole)];
    assert!(
        b.capture_vertices(&call(vertices_of(3)), &small, true)
            .is_err()
    );
    let clock = std::time::Instant::now();
    assert!(
        b.capture_vertices(&call(vertices_of(u32::MAX)), &small, false)
            .is_err()
    );
    assert!(
        clock.elapsed() < std::time::Duration::from_secs(1),
        "listed every vertex"
    );
    assert!(works(&mut b));
    let got = read_bytes(&mut b, &target, CAPTURE_TARGET_BYTES);
    assert!(
        got.iter().all(|byte| *byte == CANARY),
        "a refused capture wrote"
    );
    assert!(works(&mut b));
}

#[test]
fn a_capture_of_wild_indices_writes_only_its_range_of_its_target() {
    let mut b = device();
    let scene = capture_scene(&mut b);
    let target = capture_target(&mut b);
    let streams = [Some(wcontainers_gpu::draw::VertexBinding {
        buffer: &scene.stream,
        offset: 0,
    })];
    // Indices past the stream, at the largest value with restart off, and below zero through a base vertex of -1.
    let indices: [u32; 6] = [0, 1000, u32::MAX, 1, 0, CAPTURED_VERTICES];
    let index_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let index = b
        .create_buffer(&BufferDesc {
            size: index_bytes.len() as u64,
            usage: BufferUsage::INDEX | BufferUsage::COPY_DST,
            intent: MapIntent::Rarely,
        })
        .expect("create_buffer");
    b.write_buffer(&index, 0, &index_bytes, WriteMode::Ordered);
    let call = DrawCall {
        program: &scene.program,
        blend: &scene.states.blend,
        depth_stencil: &scene.states.depth_stencil,
        raster: &scene.states.raster,
        vertex_layout: &scene.layout,
        bindings: [None; 4],
        vertex_buffers: &streams,
        index: Some(wcontainers_gpu::draw::IndexBinding {
            buffer: &index,
            offset: 0,
            format: wcontainers_gpu::state::IndexFormat::U32,
        }),
        topology: Topology::TriangleList,
        restart: false,
        viewports: &[],
        scissors: &[],
        blend_constant: [0; 4],
        stencil_reference: [0; 2],
        immediates: &[],
        range: DrawRange::Indexed {
            first: 0,
            count: 6,
            base_vertex: -1,
            first_instance: 0,
            instances: 1,
        },
    };
    // Six vertices of four words, from byte 256 of the target, which holds exactly them.
    let (offset, size) = (256u64, u64::from(6 * 4 * U32_BYTES));
    b.capture_vertices(
        &call,
        &[wcontainers_gpu::resource::BufferRange {
            buffer: &target,
            offset,
            size,
        }],
        false,
    )
    .expect("capture_vertices");
    let got = read_words(&mut b, &target, CAPTURE_TARGET_BYTES);
    assert!(works(&mut b));
    let canary = u32::from_le_bytes([CANARY; 4]);
    let (first, last) = ((offset / 4) as usize, ((offset + size) / 4) as usize);
    assert!(
        got[..first]
            .iter()
            .chain(&got[last..])
            .all(|w| *w == canary),
        "a word outside the range changed"
    );
    // Each vertex's number, wrapped as the reference wraps it; past the stream its position reads zero.
    for (n, index) in indices.iter().enumerate() {
        let number = index.wrapping_sub(1);
        let vertex = &got[first + n * 4..first + n * 4 + 4];
        assert_eq!(vertex[0], number, "vertex {n}'s number");
        if number >= CAPTURED_VERTICES {
            assert_eq!(&vertex[1..3], &[0, 0], "vertex {n} past the stream");
        }
        assert_eq!(vertex[3], 0, "vertex {n}'s instance");
    }
}
