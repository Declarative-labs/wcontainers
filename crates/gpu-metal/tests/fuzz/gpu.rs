//! The batches that drive every method of `Gpu` on `MetalGpu` with random arguments.

use wcontainers_gpu::{
    AddressMode, AspectOps, BindingEntry, BindingResource, BlendComponent, BlendDesc, BlendFactor,
    BlendOp, BorderColor, BufferDesc, BufferRange, BufferUsage, ClearValue, ColorAttachment,
    ColorWrites, CompareFn, CopyBufferTexture, CullMode, DepthAttachment, DepthStencilDesc,
    DispatchCall, DispatchGroups, DrawCall, DrawRange, EmptyPass, Extent, FanOrLoop, FillMode,
    Filter, Format, FrontFace, Gpu, IndexBinding, IndexFormat, IndexSource, LoadOp,
    MAX_COLOR_TARGETS, MAX_VERTEX_STREAMS, MapIntent, Object, Origin, PassDesc, PrepareDesc,
    Program, ProvokingVertex, QueryKind, RasterDesc, ReadSource, RenderTargetBlend, RingKind,
    ScissorRect, ShaderModule, ShaderStage, SlowPath, StageSet, StencilFace, StencilOp, StepMode,
    StoreOp, TargetLayout, TextureDesc, TextureDimension, TextureRegion, TextureUsage,
    TextureViewDesc, Topology, VertexAttribute, VertexBinding, VertexFormat, VertexLayoutDesc,
    VertexStream, ViewDimension, Viewport, WriteMode,
};
use wcontainers_gpu_metal::MetalGpu;
use wcontainers_metal::StandaloneMetal;
use wcontainers_metal_direct::DirectMetal;
use wcontainers_test_window::StandaloneTestWindow;
use wcontainers_test_window_appkit::AppkitTestWindow;

use crate::driver::Counts;
use crate::random::Random;
use crate::shadow::{Shadow, Tracked};
use crate::{LARGEST_MADE, POOL, TEXTURE_BUDGET};

type B = MetalGpu<DirectMetal>;
type Buffer = <B as Gpu>::Buffer;

/// A range of a buffer that a call may change; a buffer of the ring has no shadow.
type Allowed = (Option<Tracked>, u64, u64);

/// The bytes of one query's result, a `u64`.
const RESULT_BYTES: u64 = size_of::<u64>() as u64;

/// Notes a range that a call may change.
fn allow(allowed: &Allowed) {
    if let (Some(shadow), offset, size) = allowed {
        shadow.borrow_mut().allow(*offset, *size);
    }
}

/// Returns the rows of blocks a copy between a buffer and a texture region touches in the buffer: one range a row.
fn copy_rows(
    format: Format,
    side: &CopyBufferTexture<'_, B>,
    extent: Extent,
    stencil: bool,
) -> Vec<(u64, u64)> {
    let (width, height, bytes) = wcontainers_gpu::FORMAT_FACTS[format as usize].block;
    // A stencil aspect is one byte a texel; a depth and stencil format's depth is taken at its widest.
    let (width, height, bytes) = if stencil {
        (1, 1, 1)
    } else {
        (width.max(1), height.max(1), bytes)
    };
    let row = u64::from(extent.width.div_ceil(width)).saturating_mul(u64::from(bytes));
    let rows = extent.height.div_ceil(height);
    let layers = extent.depth_or_layers;
    if u64::from(rows).saturating_mul(u64::from(layers)) > 4096 {
        return vec![(side.offset, u64::MAX)];
    }
    let mut ranges = Vec::new();
    for layer in 0..layers {
        for at in 0..rows {
            let start = side
                .offset
                .saturating_add(
                    u64::from(layer)
                        .saturating_mul(u64::from(side.rows_per_layer))
                        .saturating_mul(u64::from(side.row_pitch)),
                )
                .saturating_add(u64::from(at).saturating_mul(u64::from(side.row_pitch)));
            ranges.push((start, row));
        }
    }
    ranges
}

/// The programs the batches draw and dispatch with, which read and write at indices the data chooses.
const PROGRAMS: &str = "
struct Color { value: vec4<f32> }
@group(0) @binding(0) var<uniform> color: Color;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@group(0) @binding(2) var<storage, read_write> sink: array<u32>;
@group(0) @binding(3) var tex: texture_2d<f32>;
@group(0) @binding(4) var samp: sampler;
@group(0) @binding(5) var utex: texture_2d<u32>;
@vertex fn vs_input(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(p, 0.5, 1.0);
}
@vertex fn vs_index(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4<f32>(p[i % 3u], 0.5, 1.0);
}
struct Captured { @builtin(position) p: vec4<f32>, @location(0) at: vec2<f32>, @location(1) @interpolate(flat) i: u32 }
@vertex fn vs_captured(@location(0) p: vec2<f32>, @builtin(vertex_index) i: u32) -> Captured {
    return Captured(vec4<f32>(p, 0.5, 1.0), p, i);
}
@vertex fn vs_pulled(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let w = source[i];
    return vec4<f32>(f32(w & 3u) - 1.0, f32((w >> 2u) & 3u) - 1.0, 0.5, 1.0);
}
@fragment fn fs_color() -> @location(0) vec4<f32> {
    return color.value;
}
@fragment fn fs_texture(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, at.xy / 4.0);
}
@fragment fn fs_uint(@builtin(position) at: vec4<f32>) -> @location(0) vec4<u32> {
    return textureLoad(utex, vec2<i32>(at.xy) * i32(source[0]), 0);
}
@fragment fn fs_write(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    sink[source[u32(at.x)] ^ u32(at.y)] = 1u;
    return vec4<f32>(1.0);
}
@compute @workgroup_size(64) fn cs_scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    sink[source[id.x]] = id.x;
}
@compute @workgroup_size(1) fn cs_texture(@builtin(global_invocation_id) id: vec3<u32>) {
    sink[id.x] = textureLoad(utex, vec2<i32>(i32(source[0]), i32(source[1])), i32(source[2])).x;
}
";

/// The entry points of the programs, with their stage.
const ENTRIES: &[(&str, ShaderStage)] = &[
    ("vs_input", ShaderStage::Vertex),
    ("vs_index", ShaderStage::Vertex),
    ("vs_pulled", ShaderStage::Vertex),
    ("vs_captured", ShaderStage::Vertex),
    ("fs_color", ShaderStage::Fragment),
    ("fs_texture", ShaderStage::Fragment),
    ("fs_uint", ShaderStage::Fragment),
    ("fs_write", ShaderStage::Fragment),
    ("cs_scatter", ShaderStage::Compute),
    ("cs_texture", ShaderStage::Compute),
];

/// The number the fuzzer's window goes by, and one no window does.
const FUZZ_WINDOW: u32 = 2;
const NO_WINDOW: u32 = 3;

/// Every format.
const FORMATS: &[Format] = &[
    Format::R8Unorm,
    Format::R8Snorm,
    Format::R8Uint,
    Format::R8Sint,
    Format::R16Unorm,
    Format::R16Snorm,
    Format::R16Uint,
    Format::R16Sint,
    Format::R16Float,
    Format::Rg8Unorm,
    Format::Rg8Snorm,
    Format::Rg8Uint,
    Format::Rg8Sint,
    Format::R32Uint,
    Format::R32Sint,
    Format::R32Float,
    Format::Rg16Unorm,
    Format::Rg16Snorm,
    Format::Rg16Uint,
    Format::Rg16Sint,
    Format::Rg16Float,
    Format::Rgba8Unorm,
    Format::Rgba8UnormSrgb,
    Format::Rgba8Snorm,
    Format::Rgba8Uint,
    Format::Rgba8Sint,
    Format::Bgra8Unorm,
    Format::Bgra8UnormSrgb,
    Format::Bgrx8Unorm,
    Format::Bgrx8UnormSrgb,
    Format::Rgb10a2Unorm,
    Format::Rgb10a2Uint,
    Format::Rg11b10Float,
    Format::Rgb9e5Float,
    Format::B5g6r5Unorm,
    Format::Bgr5a1Unorm,
    Format::Bgra4Unorm,
    Format::Rgbg8Unorm,
    Format::Grgb8Unorm,
    Format::Rg32Uint,
    Format::Rg32Sint,
    Format::Rg32Float,
    Format::Rgba16Unorm,
    Format::Rgba16Snorm,
    Format::Rgba16Uint,
    Format::Rgba16Sint,
    Format::Rgba16Float,
    Format::Rgb32Uint,
    Format::Rgb32Sint,
    Format::Rgb32Float,
    Format::Rgba32Uint,
    Format::Rgba32Sint,
    Format::Rgba32Float,
    Format::A8Unorm,
    Format::Stencil8,
    Format::Depth16Unorm,
    Format::Depth24Plus,
    Format::Depth24PlusStencil8,
    Format::Depth32Float,
    Format::Depth32FloatStencil8,
    Format::Bc1RgbaUnorm,
    Format::Bc1RgbaUnormSrgb,
    Format::Bc2RgbaUnorm,
    Format::Bc3RgbaUnorm,
    Format::Bc4RUnorm,
    Format::Bc5RgSnorm,
    Format::Bc6hRgbUfloat,
    Format::Bc7RgbaUnorm,
    Format::Etc2Rgb8Unorm,
    Format::Etc2Rgba8UnormSrgb,
    Format::Astc4x4Unorm,
    Format::Astc4x4UnormSrgb,
];

/// The formats the batches draw into most.
const DRAWN_FORMATS: &[Format] = &[
    Format::Rgba8Unorm,
    Format::Rgba8Uint,
    Format::Bgra8Unorm,
    Format::R32Float,
    Format::Depth32Float,
    Format::Depth24PlusStencil8,
];

const COMPARES: &[CompareFn] = &[
    CompareFn::Never,
    CompareFn::Less,
    CompareFn::Equal,
    CompareFn::LessEqual,
    CompareFn::Greater,
    CompareFn::NotEqual,
    CompareFn::GreaterEqual,
    CompareFn::Always,
];

const STENCIL_OPS: &[StencilOp] = &[
    StencilOp::Keep,
    StencilOp::Zero,
    StencilOp::Replace,
    StencilOp::IncrementClamp,
    StencilOp::DecrementClamp,
    StencilOp::Invert,
    StencilOp::IncrementWrap,
    StencilOp::DecrementWrap,
];

const BLEND_FACTORS: &[BlendFactor] = &[
    BlendFactor::Zero,
    BlendFactor::One,
    BlendFactor::SrcAlpha,
    BlendFactor::OneMinusSrcAlpha,
    BlendFactor::Constant,
    BlendFactor::ConstantAlpha,
    BlendFactor::Src1,
    BlendFactor::DstAlpha,
];

const TOPOLOGIES: &[Topology] = &[
    Topology::PointList,
    Topology::LineList,
    Topology::LineStrip,
    Topology::TriangleList,
    Topology::TriangleList,
    Topology::TriangleStrip,
    Topology::TriangleFan,
    Topology::LineLoop,
];

const VERTEX_FORMATS: &[VertexFormat] = &[
    VertexFormat::Float32x2,
    VertexFormat::Float32x4,
    VertexFormat::Uint8x3,
    VertexFormat::Unorm8x4,
    VertexFormat::Float16x3,
    VertexFormat::Uint10_10_10_2,
    VertexFormat::Float11_11_10,
    VertexFormat::Bgrx8Unorm,
    VertexFormat::Sint32x1,
];

const SLOW_PATHS: &[SlowPath] = &[
    SlowPath::FanConverted,
    SlowPath::LoopConverted,
    SlowPath::ReadBackWait,
    SlowPath::PipelineMiss,
    SlowPath::WriteSplitPass,
];

/// A texture the batch made, with its description.
#[derive(Clone)]
struct MadeTexture {
    texture: <B as Gpu>::Texture,
    desc: TextureDesc,
}

/// A program the batch made, and whether it computes.
#[derive(Clone)]
struct MadeProgram {
    program: <B as Gpu>::Program,
    compute: bool,
}

/// Everything a batch has made, and what it found.
struct World {
    b: B,
    /// Every buffer the batch keeps, with what it must still hold.
    buffers: Vec<(Buffer, Option<Tracked>)>,
    textures: Vec<MadeTexture>,
    views: Vec<<B as Gpu>::TextureView>,
    samplers: Vec<<B as Gpu>::Sampler>,
    shaders: Vec<<B as Gpu>::Shader>,
    programs: Vec<MadeProgram>,
    blends: Vec<<B as Gpu>::Blend>,
    depth_stencils: Vec<<B as Gpu>::DepthStencil>,
    rasters: Vec<<B as Gpu>::Raster>,
    layouts: Vec<<B as Gpu>::VertexLayout>,
    /// Binding sets, with the storage ranges a draw or dispatch through each may write.
    sets: Vec<(<B as Gpu>::BindingSet, Vec<Allowed>)>,
    queries: Vec<<B as Gpu>::QuerySet>,
    readbacks: Vec<<B as Gpu>::Readback>,
    fences: Vec<<B as Gpu>::Fence>,
    surfaces: Vec<<B as Gpu>::Surface>,
    /// What a capture after a draw of its own buffers draws with.
    ///
    /// It is a program that writes location outputs, its vertex layout, and a target to draw into.
    capture: Option<CaptureScene>,
    in_pass: bool,
    /// Whether the device was lost: once it is, a write may be dropped, so it no longer makes its bytes known.
    lost: bool,
    counts: Counts,
}

/// How often, in percent, a call the interface makes outside a pass comes after the open pass is ended.
const LEAVES_PASS: u64 = 80;

/// How often, in percent, a call whose refusal inside a pass loses the device comes after the open pass is ended.
const LEAVES_PASS_BEFORE_A_LOSS: u64 = 98;

/// What a capture after a draw of its own buffers draws with.
#[derive(Clone)]
struct CaptureScene {
    program: <B as Gpu>::Program,
    layout: <B as Gpu>::VertexLayout,
    target: <B as Gpu>::TextureView,
}

/// The side of the texture a capture after a draw draws into, and of its viewport.
const CAPTURE_SIDE: u32 = 4;

/// The bytes of the target of a capture after a draw, which hold every vertex such a capture makes.
const CAPTURE_AFTER_DRAW_TARGET_BYTES: u64 = 4096;

/// Makes what a capture after a draw of its own buffers draws with.
fn capture_scene(b: &mut B) -> Option<CaptureScene> {
    let module = ShaderModule::from_wgsl(PROGRAMS, "vs_captured", ShaderStage::Vertex).ok()?;
    let shader = b.create_shader(&module).ok()?;
    let program = b
        .create_program(&Program::Render {
            vertex: &shader,
            fragment: None,
        })
        .ok()?;
    let mut streams = [None; MAX_VERTEX_STREAMS];
    let mut attributes = [None; MAX_VERTEX_STREAMS];
    streams[0] = Some(VertexStream {
        stride: size_of::<[f32; 2]>() as u32,
        step: StepMode::Vertex,
    });
    attributes[0] = Some(VertexAttribute {
        location: 0,
        stream: 0,
        offset: 0,
        format: VertexFormat::Float32x2,
    });
    let layout = b
        .create_vertex_layout(&VertexLayoutDesc {
            streams,
            attributes,
        })
        .ok()?;
    let texture = b
        .create_texture(&TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::Rgba8Unorm,
            width: CAPTURE_SIDE,
            height: CAPTURE_SIDE,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER,
            cube: false,
            reinterpret: false,
        })
        .ok()?;
    let target = b
        .create_view(
            &texture,
            &TextureViewDesc {
                format: Format::Rgba8Unorm,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )
        .ok()?;
    Some(CaptureScene {
        program,
        layout,
        target,
    })
}

/// Keeps at most `POOL` objects, dropping a random old one to make room.
fn keep<T>(random: &mut Random, pool: &mut Vec<T>, item: T) {
    if pool.len() >= POOL {
        let at = random.index(pool.len());
        pool[at] = item;
    } else {
        pool.push(item);
    }
}

/// Returns a random object of a pool, if it has one.
fn any<T: Clone>(random: &mut Random, pool: &[T]) -> Option<T> {
    (!pool.is_empty()).then(|| pool[random.index(pool.len())].clone())
}

/// Returns the bytes a texture of `desc` holds at most.
fn texture_bytes(desc: &TextureDesc) -> u64 {
    [
        desc.width,
        desc.height,
        desc.depth_or_layers,
        desc.samples.max(1),
        16,
    ]
    .iter()
    .try_fold(1u64, |total, n| total.checked_mul(u64::from(*n)))
    .unwrap_or(u64::MAX)
}

/// Returns a region of a texture the batch made.
fn region(
    made: &MadeTexture,
    mip: u32,
    layer: u32,
    origin: Origin,
    stencil: bool,
) -> TextureRegion<'_, B> {
    TextureRegion {
        texture: &made.texture,
        mip,
        layer,
        origin,
        stencil,
    }
}

/// Runs one batch of `calls` calls of `Gpu`.
pub fn run(random: &mut Random, calls: u64, window: bool) -> Counts {
    let mut world = World {
        b: MetalGpu::new(DirectMetal::new(())),
        buffers: Vec::new(),
        textures: Vec::new(),
        views: Vec::new(),
        samplers: Vec::new(),
        shaders: Vec::new(),
        programs: Vec::new(),
        blends: Vec::new(),
        depth_stencils: Vec::new(),
        rasters: Vec::new(),
        layouts: Vec::new(),
        sets: Vec::new(),
        queries: Vec::new(),
        readbacks: Vec::new(),
        fences: Vec::new(),
        surfaces: Vec::new(),
        capture: None,
        in_pass: false,
        lost: false,
        counts: Counts::default(),
    };
    // A window of four pixels, which takes no focus and closes when the process ends.
    let test_window = window.then(|| AppkitTestWindow::new(()));
    if let Some(test_window) = &test_window {
        let added = world.b.add_window(FUZZ_WINDOW, test_window);
        world.counts.call(added.is_err());
    }
    for index in 0..ENTRIES.len() {
        world.create_shader(random, index);
    }
    world.capture = capture_scene(&mut world.b);
    for _ in 0..6 {
        world.create_buffer(random);
        world.create_texture(random);
        world.create_states(random);
        world.create_program(random);
        world.create_sampler(random);
        world.create_set(random);
    }
    for _ in 0..calls {
        world.step(random);
        world.note_loss();
    }
    world.finish()
}

impl World {
    /// Makes one random call.
    fn step(&mut self, random: &mut Random) {
        match random.below(100) {
            0..=5 => self.create_buffer(random),
            6..=9 => self.create_texture(random),
            10..=12 => self.create_view(random),
            13 => self.create_sampler(random),
            14 => {
                let index = random.index(ENTRIES.len());
                self.create_shader(random, index)
            }
            15..=16 => self.create_program(random),
            17..=19 => self.create_states(random),
            20..=22 => self.create_set(random),
            23 => self.create_query_set(random),
            24..=26 => self.write(random),
            27..=29 => self.pass(random),
            30..=39 => self.draw(random),
            40..=41 => self.capture_after_draw(random),
            42..=43 => self.query(random),
            44..=51 => self.copy(random),
            52..=53 => self.clear_buffer(random),
            54 => self.resolve(random),
            55..=58 => self.submit_or_wait(random),
            59..=64 => self.read(random),
            65..=66 => self.surface(random),
            67..=68 => self.prepare(random),
            69..=70 => self.clear_texture(random),
            71..=72 => self.generate_mips(random),
            73..=74 => self.ring(random),
            75..=76 => self.resolve_queries(random),
            77..=81 => self.dispatch(random),
            82..=83 => self.absent(random),
            84..=85 => self.bookkeeping(random),
            _ => self.destroy(random),
        }
    }

    /// Notes the device's loss once: a call it refused may have dropped writes recorded since the last submission.
    fn note_loss(&mut self) {
        if self.lost || self.b.status() == wcontainers_gpu::DeviceStatus::Ok {
            return;
        }
        self.lost = true;
        for (_, shadow) in &self.buffers {
            if let Some(shadow) = shadow {
                shadow.borrow_mut().lost();
            }
        }
    }

    /// Notes that the work recorded so far was submitted.
    fn submitted(&mut self) {
        for (_, shadow) in &self.buffers {
            if let Some(shadow) = shadow {
                shadow.borrow_mut().submitted();
            }
        }
    }

    /// Ends the open pass, before calls that come outside one.
    fn leave_pass(&mut self) {
        if self.in_pass {
            self.b.end_pass();
            self.in_pass = false;
        }
    }

    /// Ends the open pass most times, before calls the interface makes outside one.
    ///
    /// The rest reach the places that end the pass and open it again after, or refuse inside one.
    fn mostly_leave_pass(&mut self, random: &mut Random) {
        if random.chance(LEAVES_PASS) {
            self.leave_pass();
        }
    }

    /// Ends the open pass nearly every time, before calls whose refusal inside one loses the device.
    ///
    /// A lost device drops the batch's checks of later writes.
    fn nearly_always_leave_pass(&mut self, random: &mut Random) {
        if random.chance(LEAVES_PASS_BEFORE_A_LOSS) {
            self.leave_pass();
        }
    }

    /// Makes a buffer of a random size, filled with random bytes as far as 64 KiB.
    fn create_buffer(&mut self, random: &mut Random) {
        let most = self.b.caps().limits.max_buffer_size;
        let size = if random.chance(5) {
            random.pick(&[most, most.saturating_add(1)])
        } else {
            random.u64(&[4, 16, 64, 256, 4096])
        };
        // A size past the limit reaches `create_buffer` as drawn, which must refuse it; one within it but past
        // `LARGEST_MADE` would make a large buffer on the owner's Mac, so it is cut after that check.
        let refuse = size > most;
        let size = if !refuse && size > LARGEST_MADE {
            LARGEST_MADE
        } else {
            size
        };
        let made = self.b.create_buffer(&BufferDesc {
            size,
            usage: BufferUsage::all(),
            intent: MapIntent::Rarely,
        });
        self.counts.call(made.is_err());
        let Ok(buffer) = made else {
            return;
        };
        if refuse {
            eprintln!("ACCEPTED a buffer of {size} bytes, past max_buffer_size, {most}");
            self.counts.accepted = self.counts.accepted.saturating_add(1);
            return;
        }
        let shadow = Shadow::new(size);
        let fill: Vec<u8> = (0..size.min(65536)).map(|_| random.next() as u8).collect();
        self.b.write_buffer(&buffer, 0, &fill, WriteMode::Ordered);
        if self.lost {
            shadow.borrow_mut().allow(0, fill.len() as u64);
        } else {
            shadow.borrow_mut().wrote_pending(0, &fill);
        }
        self.keep_buffer(random, (buffer, Some(shadow)));
    }

    /// Adds a buffer to the pool, checking the one it replaces.
    fn keep_buffer(&mut self, random: &mut Random, held: (Buffer, Option<Tracked>)) {
        if self.buffers.len() >= POOL {
            let at = random.index(self.buffers.len());
            let old = std::mem::replace(&mut self.buffers[at], held);
            self.check(&old);
        } else {
            self.buffers.push(held);
        }
    }

    /// Reads a buffer back, after the work recorded before, and counts its bytes changed outside every allowed range.
    fn check(&mut self, (buffer, shadow): &(Buffer, Option<Tracked>)) {
        let Some(shadow) = shadow else {
            return;
        };
        let size = shadow.borrow().length();
        if size == 0 {
            return;
        }
        self.leave_pass();
        let mut read = vec![0; size as usize];
        let source = ReadSource::Buffer {
            buffer,
            offset: 0,
            size,
        };
        match self.b.read_back_now(&source, &mut read) {
            Ok(()) => {
                let changed = shadow.borrow().changed(&read);
                self.counts.canary_bytes = self.counts.canary_bytes.saturating_add(changed);
                self.counts.free_bytes = self
                    .counts
                    .free_bytes
                    .saturating_add(shadow.borrow().free());
                self.counts.checked_bytes = self.counts.checked_bytes.saturating_add(size);
            }
            Err(error) => eprintln!("CANARY a buffer not read back: {error:?}"),
        }
    }

    /// Makes a texture of random sizes.
    fn create_texture(&mut self, random: &mut Random) {
        let plausible = random.chance(70);
        let format = if plausible {
            random.pick(DRAWN_FORMATS)
        } else {
            random.pick(FORMATS)
        };
        let side = random.u32(&[1, 4, 8, 16]);
        let mut desc = TextureDesc {
            dimension: if random.chance(85) {
                TextureDimension::D2
            } else {
                TextureDimension::D3
            },
            format,
            width: side,
            height: if plausible { side } else { random.u32(&[1, 4]) },
            depth_or_layers: if plausible {
                random.pick(&[1, 1, 2, 6])
            } else {
                random.u32(&[1, 6, 12])
            },
            mip_levels: random.u32(&[1, 2, 3]),
            samples: if random.chance(85) {
                1
            } else {
                random.u32(&[4, 2, 8])
            },
            usage: TextureUsage::from_bits_truncate(random.next() as u16)
                | TextureUsage::COPY_SRC
                | TextureUsage::COPY_DST,
            cube: random.chance(15),
            reinterpret: random.chance(20),
        };
        if plausible {
            desc.usage |= TextureUsage::SAMPLE | TextureUsage::RENDER;
        }
        // A side, layer count or sample count past the device's reaches the call as drawn and must be refused; a
        // texture the limits take but past the budget keeps its widest side and loses the rest.
        let limits = self.b.caps().limits;
        let three_d = desc.dimension == TextureDimension::D3;
        let most = if three_d {
            limits.max_texture_3d
        } else {
            limits.max_texture_2d
        };
        let layers = if three_d {
            limits.max_texture_3d
        } else {
            limits.max_texture_layers
        };
        let past = desc.width > most
            || desc.height > most
            || desc.depth_or_layers > layers
            || desc.width == 0
            || desc.height == 0
            || desc.depth_or_layers == 0
            || ![1, 4].contains(&desc.samples);
        if !past && texture_bytes(&desc) > TEXTURE_BUDGET {
            desc.height = desc.height.min(1);
            desc.depth_or_layers = desc.depth_or_layers.min(1);
            if texture_bytes(&desc) > TEXTURE_BUDGET {
                desc.width = desc.width.min(4096);
            }
        }
        let made = self.b.create_texture(&desc);
        self.counts.call(made.is_err());
        if past && made.is_ok() {
            eprintln!("ACCEPTED a texture past the device's limits: {desc:?}");
            self.counts.accepted = self.counts.accepted.saturating_add(1);
            return;
        }
        if let Ok(texture) = made {
            keep(random, &mut self.textures, MadeTexture { texture, desc });
            self.create_view(random);
        }
    }

    /// Makes a view of a random texture.
    fn create_view(&mut self, random: &mut Random) {
        let Some(made) = any(random, &self.textures) else {
            return;
        };
        let dimensions = [
            ViewDimension::D2,
            ViewDimension::D2Array,
            ViewDimension::Cube,
            ViewDimension::CubeArray,
            ViewDimension::D3,
        ];
        let desc = TextureViewDesc {
            format: if random.chance(80) {
                made.desc.format
            } else {
                random.pick(FORMATS)
            },
            dimension: if random.chance(80) {
                made.desc.own_view()
            } else {
                random.pick(&dimensions)
            },
            base_mip: random.u32(&[0, 0, 1]),
            mip_count: if random.chance(70) {
                made.desc.mip_levels
            } else {
                random.u32(&[1])
            },
            base_layer: random.u32(&[0, 0, 1]),
            layer_count: if random.chance(70) {
                made.desc.depth_or_layers
            } else {
                random.u32(&[1, 6])
            },
            stencil: random.chance(10),
        };
        let view = self.b.create_view(&made.texture, &desc);
        self.counts.call(view.is_err());
        if let Ok(view) = view {
            keep(random, &mut self.views, view);
        }
    }

    /// Makes a sampler of random modes.
    fn create_sampler(&mut self, random: &mut Random) {
        let modes = [
            AddressMode::Repeat,
            AddressMode::Mirror,
            AddressMode::Clamp,
            AddressMode::Border,
            AddressMode::MirrorOnce,
        ];
        let filters = [Filter::Nearest, Filter::Linear];
        let made = self.b.create_sampler(&wcontainers_gpu::SamplerDesc {
            min: random.pick(&filters),
            mag: random.pick(&filters),
            mip: random.pick(&filters),
            address: [
                random.pick(&modes),
                random.pick(&modes),
                random.pick(&modes),
            ],
            lod_range: [
                random.f32(&[0.0]).to_bits(),
                random.f32(&[1000.0]).to_bits(),
            ],
            anisotropy: u16::try_from(random.u32(&[1, 16])).unwrap_or(u16::MAX),
            compare: random.chance(30).then(|| random.pick(COMPARES)),
            border: random.pick(&[
                BorderColor::TransparentBlack,
                BorderColor::OpaqueBlack,
                BorderColor::OpaqueWhite,
                BorderColor::Custom([0, 1, 2, 3]),
            ]),
        });
        self.counts.call(made.is_err());
        if let Ok(sampler) = made {
            keep(random, &mut self.samplers, sampler);
        }
    }

    /// Makes a shader of one entry point, at times of another stage than its own.
    fn create_shader(&mut self, random: &mut Random, index: usize) {
        let (entry, stage) = ENTRIES[index];
        let stage = if random.chance(95) {
            stage
        } else {
            random.pick(&[
                ShaderStage::Vertex,
                ShaderStage::Fragment,
                ShaderStage::Compute,
            ])
        };
        let Ok(module) = ShaderModule::from_wgsl(PROGRAMS, entry, stage) else {
            self.counts.call(true);
            return;
        };
        let made = self.b.create_shader(&module);
        self.counts.call(made.is_err());
        if let Ok(shader) = made {
            keep(random, &mut self.shaders, shader);
        }
    }

    /// Makes a render or compute program of random shaders.
    fn create_program(&mut self, random: &mut Random) {
        let Some(first) = any(random, &self.shaders) else {
            return;
        };
        let compute = random.chance(30);
        let second = any(random, &self.shaders);
        let made = if compute {
            self.b.create_program(&Program::Compute(&first))
        } else {
            self.b.create_program(&Program::Render {
                vertex: &first,
                fragment: second.as_ref().filter(|_| random.chance(90)),
            })
        };
        self.counts.call(made.is_err());
        if let Ok(program) = made {
            keep(random, &mut self.programs, MadeProgram { program, compute });
        }
    }

    /// Makes a blend, a depth and stencil, a raster and a vertex layout block of random fields.
    fn create_states(&mut self, random: &mut Random) {
        let component = |random: &mut Random| BlendComponent {
            src: random.pick(BLEND_FACTORS),
            dst: random.pick(BLEND_FACTORS),
            op: random.pick(&[BlendOp::Add, BlendOp::Subtract, BlendOp::Min, BlendOp::Max]),
        };
        let target = RenderTargetBlend {
            blend: random
                .chance(30)
                .then(|| (component(random), component(random))),
            writes: ColorWrites::from_bits_truncate(random.next() as u8),
        };
        let blend = self.b.create_blend(&BlendDesc {
            targets: [target; MAX_COLOR_TARGETS],
            same_for_all: random.chance(80),
            alpha_to_coverage: random.chance(10),
            logic_op: None,
        });
        self.counts.call(blend.is_err());
        if let Ok(blend) = blend {
            keep(random, &mut self.blends, blend);
        }
        let face = |random: &mut Random| StencilFace {
            compare: random.pick(COMPARES),
            fail: random.pick(STENCIL_OPS),
            depth_fail: random.pick(STENCIL_OPS),
            pass: random.pick(STENCIL_OPS),
        };
        let depth_stencil = self.b.create_depth_stencil(&DepthStencilDesc {
            depth_compare: random.chance(40).then(|| random.pick(COMPARES)),
            depth_write: random.chance(30),
            stencil: random.chance(20).then(|| (face(random), face(random))),
            stencil_read_mask: [random.next() as u8, u8::MAX],
            stencil_write_mask: [u8::MAX, random.next() as u8],
            depth_bounds: random.chance(10).then(|| {
                random
                    .pick(&[
                        [0.0f32, 1.0],
                        [0.25, 0.75],
                        [0.5, 0.5],
                        [0.75, 0.25],
                        [-1.0, 1.0],
                        [0.0, 2.0],
                        [f32::NAN, 1.0],
                    ])
                    .map(f32::to_bits)
            }),
        });
        self.counts.call(depth_stencil.is_err());
        if let Ok(state) = depth_stencil {
            keep(random, &mut self.depth_stencils, state);
        }
        let raster = self.b.create_raster(&RasterDesc {
            fill: random.pick(&[
                FillMode::Solid,
                FillMode::Solid,
                FillMode::Line,
                FillMode::Point,
            ]),
            cull: random.pick(&[CullMode::None, CullMode::Front, CullMode::Back]),
            front: random.pick(&[FrontFace::CounterClockwise, FrontFace::Clockwise]),
            depth_bias: random.f32(&[0.0, 2.5]).to_bits(),
            depth_bias_slope: random.f32(&[0.0]).to_bits(),
            depth_bias_clamp: random.f32(&[0.0]).to_bits(),
            depth_clip: random.chance(80),
            scissor: random.chance(30),
            line_width: if random.chance(95) {
                1.0f32.to_bits()
            } else {
                2.0f32.to_bits()
            },
            provoking: if random.chance(95) {
                ProvokingVertex::First
            } else {
                ProvokingVertex::Last
            },
            depth_minus_one: random.chance(3),
            sample_mask: if random.chance(95) {
                !0
            } else {
                random.u32(&[1])
            },
        });
        self.counts.call(raster.is_err());
        if let Ok(raster) = raster {
            keep(random, &mut self.rasters, raster);
        }
        let mut streams = [None; MAX_VERTEX_STREAMS];
        let mut attributes = [None; MAX_VERTEX_STREAMS];
        let count = random.index(3);
        for (at, stream) in streams.iter_mut().enumerate().take(count) {
            *stream = Some(VertexStream {
                stride: random.u32(&[8, 16, 0, 4]),
                step: if random.chance(80) {
                    StepMode::Vertex
                } else {
                    StepMode::Instance(random.u32(&[1, 2, 0, 3]))
                },
            });
            attributes[at] = Some(VertexAttribute {
                location: u32::try_from(at).unwrap_or(0),
                stream: u8::try_from(at).unwrap_or(0),
                offset: random.u32(&[0, 4]),
                format: random.pick(VERTEX_FORMATS),
            });
        }
        if random.chance(5) {
            attributes[0] = Some(VertexAttribute {
                location: random.u32(&[0]),
                stream: random.next() as u8,
                offset: random.u32(&[0]),
                format: random.pick(VERTEX_FORMATS),
            });
        }
        let layout = self.b.create_vertex_layout(&VertexLayoutDesc {
            streams,
            attributes,
        });
        self.counts.call(layout.is_err());
        if let Ok(layout) = layout {
            keep(random, &mut self.layouts, layout);
        }
    }

    /// Returns a random range of a random buffer.
    ///
    /// The last of the tuple is the buffer's shadow.
    fn range(&mut self, random: &mut Random) -> Option<(Buffer, u64, u64, Option<Tracked>)> {
        let (buffer, shadow) = any(random, &self.buffers)?;
        let offset = if random.chance(70) {
            random.pick(&[0, 256])
        } else {
            random.u64(&[0, 4])
        };
        let size = if random.chance(70) {
            random.pick(&[16, 64, 256])
        } else {
            random.u64(&[0, 4])
        };
        Some((buffer, offset, size, shadow))
    }

    /// Makes a binding set of random resources at the programs' bindings.
    fn create_set(&mut self, random: &mut Random) {
        let (uniform, source, sink) = (self.range(random), self.range(random), self.range(random));
        let (view, uview, sampler) = (
            any(random, &self.views),
            any(random, &self.views),
            any(random, &self.samplers),
        );
        let mut entries = Vec::new();
        let all = StageSet::all();
        let binding = |random: &mut Random, usual: u32| {
            if random.chance(95) {
                usual
            } else {
                random.u32(&[usual])
            }
        };
        let mut writable = Vec::new();
        if let Some((buffer, offset, size, _)) = &uniform {
            entries.push(BindingEntry {
                binding: binding(random, 0),
                stages: all,
                resource: BindingResource::UniformBuffer(BufferRange {
                    buffer,
                    offset: *offset,
                    size: *size,
                }),
            });
        }
        for (at, range) in [(1, &source), (2, &sink)] {
            if let Some((buffer, offset, size, shadow)) = range {
                if at == 2 {
                    writable.push((shadow.clone(), *offset, *size));
                }
                entries.push(BindingEntry {
                    binding: binding(random, at),
                    stages: all,
                    resource: BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer,
                            offset: *offset,
                            size: *size,
                        },
                        writable: at == 2,
                    },
                });
            }
        }
        for (at, view) in [(3, &view), (5, &uview)] {
            if let Some(view) = view {
                entries.push(BindingEntry {
                    binding: binding(random, at),
                    stages: all,
                    resource: BindingResource::Texture(view),
                });
            }
        }
        if let Some(sampler) = &sampler {
            entries.push(BindingEntry {
                binding: binding(random, 4),
                stages: all,
                resource: BindingResource::Sampler(sampler),
            });
        }
        if random.chance(5)
            && let Some((buffer, offset, size, _)) = &source
        {
            entries.push(BindingEntry {
                binding: 9,
                stages: all,
                resource: BindingResource::TexelBuffer {
                    range: BufferRange {
                        buffer,
                        offset: *offset,
                        size: *size,
                    },
                    format: Format::R32Uint,
                    writable: false,
                },
            });
        }
        let made = self.b.create_binding_set(&entries);
        self.counts.call(made.is_err());
        if let Ok(set) = made {
            let _ = self.b.binding_layout_key(&set);
            self.counts.call(false);
            keep(random, &mut self.sets, (set, writable));
        }
    }

    /// Makes a query set of a random kind and count.
    fn create_query_set(&mut self, random: &mut Random) {
        let kind = random.pick(&[
            QueryKind::Occlusion,
            QueryKind::OcclusionAny,
            QueryKind::Timestamp,
        ]);
        // A count whose results pass `max_buffer_size` reaches the call as drawn, which must refuse it; one within it
        // but past 2^20 would make a large buffer, so it is cut after that check.
        let count = random.u32(&[1, 2, 8]);
        let refuse =
            u64::from(count).saturating_mul(RESULT_BYTES) > self.b.caps().limits.max_buffer_size;
        let count = if refuse { count } else { count.min(1 << 20) };
        let made = self.b.create_query_set(kind, count);
        self.counts.call(made.is_err());
        if let Ok(set) = made {
            if refuse {
                eprintln!("ACCEPTED a query set of {count} queries, past max_buffer_size");
                self.counts.accepted = self.counts.accepted.saturating_add(1);
            }
            keep(random, &mut self.queries, set);
        }
    }

    /// Returns a random region of a random texture.
    fn region(&mut self, random: &mut Random) -> Option<(MadeTexture, u32, u32, Origin, Extent)> {
        let made = any(random, &self.textures)?;
        Some((
            made,
            random.u32(&[0, 0, 1]),
            random.u32(&[0, 0, 1]),
            Origin {
                x: random.u32(&[0, 0, 4]),
                y: random.u32(&[0, 0, 4]),
                z: random.u32(&[0, 0, 1]),
            },
            Extent {
                width: random.u32(&[1, 4, 8]),
                height: random.u32(&[1, 4, 8]),
                depth_or_layers: random.u32(&[1, 1, 2]),
            },
        ))
    }

    /// Writes bytes into a random buffer, or texels into a random texture region.
    fn write(&mut self, random: &mut Random) {
        let mode = random.pick(&[
            WriteMode::Ordered,
            WriteMode::Discard,
            WriteMode::NoOverwrite,
        ]);
        if random.chance(50) {
            let Some((buffer, offset, _, shadow)) = self.range(random) else {
                return;
            };
            let bytes = random.bytes(&[0, 1, 3, 4, 16, 256, 4096]);
            self.b.write_buffer(&buffer, offset, &bytes, mode);
            self.counts.call(false);
            let Some(tracked) = shadow else {
                return;
            };
            let mut shadow = tracked.borrow_mut();
            // A write past the buffer is refused and writes nothing.
            if offset
                .checked_add(bytes.len() as u64)
                .is_none_or(|end| end > shadow.length())
            {
                return;
            }
            // After a discard every byte but the written ones is undefined: the batch writes them again, in order, so
            // that they are canaries again rather than free for the rest of the batch.
            if mode == WriteMode::Discard && !self.lost {
                let length = shadow.length();
                let end = offset.saturating_add(bytes.len() as u64);
                shadow.allow_all();
                shadow.wrote_pending(offset, &bytes);
                drop(shadow);
                for (start, stop) in [(0, offset), (end, length)] {
                    let refill: Vec<u8> = (start..stop).map(|_| random.next() as u8).collect();
                    if refill.is_empty() {
                        continue;
                    }
                    self.b
                        .write_buffer(&buffer, start, &refill, WriteMode::Ordered);
                    self.counts.call(false);
                    tracked.borrow_mut().wrote_pending(start, &refill);
                }
                return;
            }
            match mode {
                // The bytes outside the written part are undefined until written, on a lost device too.
                WriteMode::Discard if self.lost => shadow.allow_all(),
                _ if self.lost => shadow.allow(offset, bytes.len() as u64),
                WriteMode::Ordered => shadow.wrote_pending(offset, &bytes),
                WriteMode::Discard => {
                    shadow.allow_all();
                    shadow.wrote_pending(offset, &bytes);
                }
                // The batch does not keep the promise this mode asks, so the written bytes may land before or after
                // the work recorded earlier.
                WriteMode::NoOverwrite => shadow.allow(offset, bytes.len() as u64),
            }
            return;
        }
        let Some((made, mip, layer, origin, extent)) = self.region(random) else {
            return;
        };
        let bytes = random.bytes(&[0, 4, 64, 256, 1024]);
        let region = TextureRegion {
            texture: &made.texture,
            mip,
            layer,
            origin,
            stencil: random.chance(10),
        };
        let (row_pitch, rows) = (random.u32(&[16, 32, 64]), random.u32(&[4, 1]));
        self.b
            .write_texture(&region, extent, &bytes, row_pitch, rows, mode);
        self.counts.call(false);
    }

    /// Begins or ends a pass, of random attachments.
    fn pass(&mut self, random: &mut Random) {
        if self.in_pass && random.chance(60) {
            self.leave_pass();
            self.counts.call(false);
            return;
        }
        let views: Vec<_> = (0..MAX_COLOR_TARGETS)
            .map(|at| {
                (at == 0 || random.chance(10))
                    .then(|| any(random, &self.views))
                    .flatten()
            })
            .collect();
        let depth = random
            .chance(30)
            .then(|| any(random, &self.views))
            .flatten();
        let resolve = random
            .chance(10)
            .then(|| any(random, &self.views))
            .flatten();
        let occlusion = random
            .chance(20)
            .then(|| any(random, &self.queries))
            .flatten();
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: random.chance(10).then(|| EmptyPass {
                width: random.u32(&[4]),
                height: random.u32(&[4]),
                samples: random.u32(&[1, 4]),
            }),
            occlusion: occlusion.as_ref(),
        };
        for (at, view) in views.iter().enumerate() {
            if let Some(view) = view {
                pass.colors[at] = Some(ColorAttachment {
                    view,
                    depth_slice: random.u32(&[0, 0, 1]),
                    resolve: if at == 0 { resolve.as_ref() } else { None },
                    load: random.pick(&[
                        LoadOp::Clear([0, 0, 0, 0]),
                        LoadOp::Load,
                        LoadOp::DontCare,
                    ]),
                    store: random.pick(&[StoreOp::Store, StoreOp::Discard]),
                });
            }
        }
        if let Some(view) = &depth {
            let depth_clear = random.f32(&[1.0]).to_bits();
            let stencil_clear = random.next() as u8;
            pass.depth = Some(DepthAttachment {
                view,
                depth: random.chance(80).then(|| AspectOps {
                    load: random.pick(&[LoadOp::Clear(depth_clear), LoadOp::Load]),
                    store: StoreOp::Store,
                    read_only: random.chance(10),
                }),
                stencil: random.chance(40).then(|| AspectOps {
                    load: random.pick(&[LoadOp::Clear(stencil_clear), LoadOp::Load]),
                    store: StoreOp::Store,
                    read_only: random.chance(10),
                }),
            });
        }
        self.leave_pass();
        self.b.begin_pass(&pass);
        self.in_pass = true;
        self.counts.call(false);
    }

    /// Draws with random states, bindings, vertex streams, indices and counts, in a pass or not.
    fn draw(&mut self, random: &mut Random) {
        let render: Vec<_> = self
            .programs
            .iter()
            .filter(|p| !p.compute)
            .cloned()
            .collect();
        let (Some(program), Some(blend), Some(depth_stencil), Some(raster), Some(layout)) = (
            any(random, &render),
            any(random, &self.blends),
            any(random, &self.depth_stencils),
            any(random, &self.rasters),
            any(random, &self.layouts),
        ) else {
            return;
        };
        let (set, writable) = any(random, &self.sets).unzip();
        let writable = writable.unwrap_or_default();
        let streams: Vec<_> = (0..random.index(3)).map(|_| self.range(random)).collect();
        let index = self.range(random);
        let vertex_buffers: Vec<Option<VertexBinding<'_, B>>> = streams
            .iter()
            .map(|s| {
                s.as_ref().map(|(buffer, offset, _, _)| VertexBinding {
                    buffer,
                    offset: *offset,
                })
            })
            .collect();
        let viewports = [Viewport {
            x: random.u32(&[0]),
            y: random.u32(&[0]),
            width: random.f32(&[4.0]).to_bits(),
            height: random.f32(&[4.0]).to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }];
        let scissors = [ScissorRect {
            x: random.u32(&[0, 1]),
            y: random.u32(&[0, 1]),
            width: random.u32(&[4]),
            height: random.u32(&[4]),
        }];
        let immediates = random.bytes(&[0, 0, 4, 16, 300]);
        let mut count = random.u32(&[3, 6]);
        let mut instances = random.u32(&[1, 2]);
        let (first, first_instance) = (random.u32(&[0, 1]), random.u32(&[0]));
        let indexed = random.chance(40);
        // Counts whose sums pass the largest `u32` reach the draw as drawn, which must refuse them; an indexed draw's
        // count is bounded by its index buffer. A draw that passes those checks but would run a great many vertices is
        // cut after them: the bounds, not the GPU's time, are under test.
        let refuse = first_instance.checked_add(instances).is_none()
            || (!indexed && first.checked_add(count).is_none());
        if !refuse && u64::from(count).saturating_mul(u64::from(instances)) > 1 << 14 {
            if !indexed {
                count = count.min(1 << 10);
            }
            instances = instances.min(4);
        }
        let range = if indexed {
            DrawRange::Indexed {
                first,
                count,
                base_vertex: random.i32(),
                first_instance,
                instances,
            }
        } else {
            DrawRange::Vertices {
                first,
                count,
                first_instance,
                instances,
            }
        };
        let index_format = random.pick(&[IndexFormat::U16, IndexFormat::U32, IndexFormat::U8]);
        let index_binding = index
            .as_ref()
            .filter(|_| indexed)
            .map(|(buffer, offset, _, _)| IndexBinding {
                buffer,
                offset: *offset,
                format: index_format,
            });
        let stencil_reference = [random.next() as u8, random.next() as u8];
        let call = DrawCall {
            program: &program.program,
            blend: &blend,
            depth_stencil: &depth_stencil,
            raster: &raster,
            vertex_layout: &layout,
            bindings: [set.as_ref(), None, None, None],
            vertex_buffers: &vertex_buffers,
            index: index_binding,
            topology: random.pick(TOPOLOGIES),
            restart: random.chance(20),
            viewports: &viewports,
            scissors: &scissors,
            blend_constant: [0, 0, 0, 1.0f32.to_bits()],
            stencil_reference,
            immediates: &immediates,
            range,
        };
        for range in &writable {
            allow(range);
        }
        match random.below(10) {
            0 => {
                let kind = random.pick(&[FanOrLoop::TriangleFan, FanOrLoop::LineLoop]);
                let guest = random.bytes(&[0, 2, 12, 64]);
                let source = if random.chance(50) {
                    IndexSource::Sequential {
                        first: random.u32(&[0]),
                        // The trait's default builds three indices a vertex in memory before any check, so a count
                        // near the largest `u32` would take some 48 GiB: it is cut until the trait bounds it.
                        count: random.u32(&[3, 4, 6]).min(4096),
                    }
                } else {
                    IndexSource::Guest {
                        indices: &guest,
                        format: random.pick(&[IndexFormat::U16, IndexFormat::U32, IndexFormat::U8]),
                        base_vertex: random.i32(),
                    }
                };
                let provoking = random.pick(&[ProvokingVertex::First, ProvokingVertex::Last]);
                let drawn = self
                    .b
                    .draw_fan_or_loop(&call, kind, source, provoking, 0, instances);
                self.counts.call(drawn.is_err());
            }
            1 => {
                // Targets of their own now and then, which a capture may write; else the streams, which it must not.
                let own: Vec<_> = if random.chance(60) {
                    (0..=random.index(3)).map(|_| self.range(random)).collect()
                } else {
                    Vec::new()
                };
                let chosen = if own.is_empty() { &streams } else { &own };
                let targets: Vec<BufferRange<&<B as Gpu>::Buffer>> = chosen
                    .iter()
                    .flatten()
                    .map(|(buffer, offset, size, shadow)| {
                        allow(&(shadow.clone(), *offset, *size));
                        BufferRange {
                            buffer,
                            offset: *offset,
                            size: *size,
                        }
                    })
                    .collect();
                let rasterize = random.chance(50);
                let captured = self.b.capture_vertices(&call, &targets, rasterize);
                self.counts.call(captured.is_err());
            }
            _ => {
                self.b.draw(&call);
                self.counts.call(false);
            }
        }
    }

    /// Draws a call and then captures the same call, as a program that captures what it drew a moment ago does.
    ///
    /// The two are in one pass, the draw in a pass before the capture's not yet submitted, or the capture not
    /// rasterising after the pass; the call's stream and indices are buffers of the batch, the target one of its own.
    fn capture_after_draw(&mut self, random: &mut Random) {
        let (Some(scene), Some(blend), Some(depth_stencil), Some(raster)) = (
            self.capture.clone(),
            any(random, &self.blends),
            any(random, &self.depth_stencils),
            any(random, &self.rasters),
        ) else {
            return;
        };
        let (Some((stream, _)), Some((index, _))) =
            (any(random, &self.buffers), any(random, &self.buffers))
        else {
            return;
        };
        let target = self.b.create_buffer(&BufferDesc {
            size: CAPTURE_AFTER_DRAW_TARGET_BYTES,
            usage: BufferUsage::CAPTURE | BufferUsage::COPY_SRC,
            intent: MapIntent::Rarely,
        });
        self.counts.call(target.is_err());
        let Ok(target) = target else {
            return;
        };
        let streams = [Some(VertexBinding {
            buffer: &stream,
            offset: 0,
        })];
        let viewports = [Viewport {
            x: 0,
            y: 0,
            width: (CAPTURE_SIDE as f32).to_bits(),
            height: (CAPTURE_SIDE as f32).to_bits(),
            min_depth: 0,
            max_depth: 1.0f32.to_bits(),
        }];
        let count = random.pick(&[3, 6]);
        let instances = random.pick(&[1, 2]);
        let indexed = random.chance(50);
        let call = DrawCall {
            program: &scene.program,
            blend: &blend,
            depth_stencil: &depth_stencil,
            raster: &raster,
            vertex_layout: &scene.layout,
            bindings: [None; 4],
            vertex_buffers: &streams,
            index: indexed.then_some(IndexBinding {
                buffer: &index,
                offset: 0,
                format: random.pick(&[IndexFormat::U16, IndexFormat::U32]),
            }),
            topology: random.pick(&[
                Topology::PointList,
                Topology::LineList,
                Topology::TriangleList,
                Topology::TriangleStrip,
            ]),
            restart: random.chance(20),
            viewports: &viewports,
            scissors: &[],
            blend_constant: [0; 4],
            stencil_reference: [0; 2],
            immediates: &[],
            range: if indexed {
                DrawRange::Indexed {
                    first: 0,
                    count,
                    base_vertex: 0,
                    first_instance: 0,
                    instances,
                }
            } else {
                DrawRange::Vertices {
                    first: 0,
                    count,
                    first_instance: 0,
                    instances,
                }
            },
        };
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        pass.colors[0] = Some(ColorAttachment {
            view: &scene.target,
            depth_slice: 0,
            resolve: None,
            load: LoadOp::Load,
            store: StoreOp::Store,
        });
        let how = random.below(3);
        self.leave_pass();
        self.b.begin_pass(&pass);
        self.in_pass = true;
        self.b.draw(&call);
        self.counts.call(false);
        if how > 0 {
            self.leave_pass();
        }
        if how == 1 {
            self.b.begin_pass(&pass);
            self.in_pass = true;
        }
        let captured = self.b.capture_vertices(
            &call,
            &[BufferRange {
                buffer: &target,
                offset: 0,
                size: CAPTURE_AFTER_DRAW_TARGET_BYTES,
            }],
            how < 2,
        );
        self.counts.call(captured.is_err());
    }

    /// Begins or ends a query.
    fn query(&mut self, random: &mut Random) {
        if random.chance(50) {
            self.b.begin_query(random.u32(&[0, 1, 2]));
        } else {
            self.b.end_query();
        }
        self.counts.call(false);
    }

    /// Copies between random buffers and textures.
    fn copy(&mut self, random: &mut Random) {
        match random.below(4) {
            0 => {
                let (Some((src, src_offset, size, _)), Some((dst, dst_offset, _, shadow))) =
                    (self.range(random), self.range(random))
                else {
                    return;
                };
                self.b.copy_buffer(&src, src_offset, &dst, dst_offset, size);
                allow(&(shadow, dst_offset, size));
            }
            1 => {
                let (Some((src, sm, sl, so, extent)), Some((dst, dm, dl, d_o, _))) =
                    (self.region(random), self.region(random))
                else {
                    return;
                };
                self.b.copy_texture(
                    &region(&src, sm, sl, so, false),
                    &region(&dst, dm, dl, d_o, false),
                    extent,
                );
            }
            other => {
                let (Some((buffer, offset, _, shadow)), Some((made, mip, layer, origin, extent))) =
                    (self.range(random), self.region(random))
                else {
                    return;
                };
                let side = CopyBufferTexture {
                    buffer: &buffer,
                    offset,
                    row_pitch: random.u32(&[16, 32, 256]),
                    rows_per_layer: random.u32(&[1, 4, 8]),
                };
                let stencil = random.chance(20);
                if other == 2 {
                    self.b.copy_buffer_to_texture(
                        &side,
                        &region(&made, mip, layer, origin, stencil),
                        extent,
                    );
                } else {
                    self.b.copy_texture_to_buffer(
                        &region(&made, mip, layer, origin, stencil),
                        &side,
                        extent,
                    );
                    for (start, length) in copy_rows(made.desc.format, &side, extent, stencil) {
                        allow(&(shadow.clone(), start, length));
                    }
                }
            }
        }
        self.counts.call(false);
    }

    /// Clears a random range of a random buffer with a random pattern.
    fn clear_buffer(&mut self, random: &mut Random) {
        let Some((buffer, offset, size, shadow)) = self.range(random) else {
            return;
        };
        let pattern = random.bytes(&[1, 2, 4, 8, 16, 0, 3]);
        self.b.clear_buffer(&buffer, offset, size, &pattern);
        allow(&(shadow, offset, size));
        self.counts.call(false);
    }

    /// Resolves a random region into another.
    fn resolve(&mut self, random: &mut Random) {
        let (Some((src, sm, sl, so, _)), Some((dst, dm, dl, d_o, _))) =
            (self.region(random), self.region(random))
        else {
            return;
        };
        let format = if random.chance(70) {
            src.desc.format
        } else {
            random.pick(FORMATS)
        };
        self.nearly_always_leave_pass(random);
        self.b.resolve(
            &region(&src, sm, sl, so, false),
            &region(&dst, dm, dl, d_o, false),
            format,
        );
        self.counts.call(false);
    }

    /// Submits, or asks about or waits for a random fence.
    fn submit_or_wait(&mut self, random: &mut Random) {
        match random.below(3) {
            0 => {
                self.mostly_leave_pass(random);
                let fence = self.b.submit();
                self.submitted();
                keep(random, &mut self.fences, fence);
            }
            1 => {
                if let Some(fence) = any(random, &self.fences) {
                    let _ = self.b.fence_done(&fence);
                }
            }
            _ => {
                if let Some(fence) = any(random, &self.fences) {
                    self.mostly_leave_pass(random);
                    self.b.wait_fence(&fence);
                }
            }
        }
        self.counts.call(false);
    }

    /// Reads back random ranges, regions and queries, at once or later.
    fn read(&mut self, random: &mut Random) {
        match random.below(6) {
            0 | 1 => {
                let Some((buffer, offset, size, _)) = self.range(random) else {
                    return;
                };
                let source = ReadSource::Buffer {
                    buffer: &buffer,
                    offset,
                    size,
                };
                if random.chance(50) {
                    let readback = self.b.read_back(&source);
                    self.counts.call(readback.is_err());
                    if let Ok(readback) = readback {
                        keep(random, &mut self.readbacks, readback);
                    }
                } else {
                    self.mostly_leave_pass(random);
                    let mut out = vec![0; random.pick(&[0, 4, 64, 256])];
                    let read = self.b.read_back_now(&source, &mut out);
                    self.counts.call(read.is_err());
                }
            }
            2 => {
                let Some((made, mip, layer, origin, extent)) = self.region(random) else {
                    return;
                };
                let source = ReadSource::Texture {
                    region: TextureRegion {
                        texture: &made.texture,
                        mip,
                        layer,
                        origin,
                        stencil: random.chance(20),
                    },
                    extent,
                };
                let readback = self.b.read_back(&source);
                self.counts.call(readback.is_err());
                if let Ok(readback) = readback {
                    keep(random, &mut self.readbacks, readback);
                }
            }
            3 => {
                let Some(set) = any(random, &self.queries) else {
                    return;
                };
                let (first, count) = (random.u32(&[0, 1]), random.u32(&[0, 1, 2]));
                let readback = self.b.read_queries(&set, first, count);
                self.counts.call(readback.is_err());
                if let Ok(readback) = readback {
                    keep(random, &mut self.readbacks, readback);
                }
            }
            4 => {
                if let Some(readback) = any(random, &self.readbacks) {
                    let _ = self.b.read_state(&readback);
                    if random.chance(30) {
                        self.mostly_leave_pass(random);
                        self.b.wait_read(&readback);
                    }
                    self.counts.call(false);
                }
            }
            _ => {
                if !self.readbacks.is_empty() {
                    self.mostly_leave_pass(random);
                    let readback = self
                        .readbacks
                        .swap_remove(random.index(self.readbacks.len()));
                    let mut out = vec![0; random.pick(&[0, 4, 64, 4096])];
                    self.b.finish_read(readback, &mut out);
                    self.counts.call(false);
                }
            }
        }
    }

    /// Makes, configures and presents a surface of the batch's window, if it has one, or of none.
    fn surface(&mut self, random: &mut Random) {
        match random.below(4) {
            0 => {
                let surface = self.b.create_surface(random.u32(&[FUZZ_WINDOW, NO_WINDOW]));
                self.counts.call(surface.is_err());
                if let Ok(surface) = surface {
                    keep(random, &mut self.surfaces, surface);
                }
            }
            1 => {
                let Some(surface) = any(random, &self.surfaces) else {
                    return;
                };
                let (mut width, mut height) = (random.u32(&[4, 8]), random.u32(&[4, 8]));
                // A frame within the limits stays small; one past them is refused.
                if u64::from(width).saturating_mul(u64::from(height)) > 1 << 16
                    && width <= 16384
                    && height <= 16384
                {
                    (width, height) = (width.min(256), height.min(256));
                }
                let format = if random.chance(70) {
                    Format::Bgra8Unorm
                } else {
                    random.pick(FORMATS)
                };
                let configured = self
                    .b
                    .configure_surface(&surface, format, width, height, false);
                self.counts.call(configured.is_err());
            }
            2 => {
                let Some(surface) = any(random, &self.surfaces) else {
                    return;
                };
                let view = self.b.current_texture(&surface);
                self.counts.call(view.is_err());
                if let Ok(view) = view {
                    keep(random, &mut self.views, view);
                }
            }
            _ => {
                // Destroys drop surfaces often: with none, one of the batch's window is made and configured first.
                if self.surfaces.is_empty()
                    && let Ok(surface) = self.b.create_surface(FUZZ_WINDOW)
                {
                    let configured = self.b.configure_surface(
                        &surface,
                        Format::Bgra8Unorm,
                        wcontainers_test_window::WINDOW_SIDE as u32,
                        wcontainers_test_window::WINDOW_SIDE as u32,
                        false,
                    );
                    self.counts.call(configured.is_err());
                    keep(random, &mut self.surfaces, surface);
                }
                let Some(surface) = any(random, &self.surfaces) else {
                    return;
                };
                self.mostly_leave_pass(random);
                let presented = self.b.present(&surface);
                self.counts.call(presented.is_err());
            }
        }
    }

    /// Builds a pipeline ahead of a draw, of random states and targets.
    fn prepare(&mut self, random: &mut Random) {
        let render: Vec<_> = self
            .programs
            .iter()
            .filter(|p| !p.compute)
            .cloned()
            .collect();
        let (Some(program), Some(blend), Some(depth_stencil), Some(raster), Some(layout)) = (
            any(random, &render),
            any(random, &self.blends),
            any(random, &self.depth_stencils),
            any(random, &self.rasters),
            any(random, &self.layouts),
        ) else {
            return;
        };
        let mut colors = [None; MAX_COLOR_TARGETS];
        colors[0] = Some(random.pick(DRAWN_FORMATS));
        let depth = random.chance(30).then(|| random.pick(DRAWN_FORMATS));
        let samples = random.u32(&[1, 4]);
        self.b.prepare(&PrepareDesc {
            program: &program.program,
            blend: &blend,
            depth_stencil: &depth_stencil,
            raster: &raster,
            vertex_layout: &layout,
            targets: TargetLayout {
                colors,
                depth_stencil: depth,
                samples,
            },
            topology: random.pick(TOPOLOGIES),
            layouts: Default::default(),
        });
        self.counts.call(false);
    }

    /// Clears a random texture's levels and layers, or rectangles of a random view.
    fn clear_texture(&mut self, random: &mut Random) {
        if random.chance(50) {
            let Some(made) = any(random, &self.textures) else {
                return;
            };
            self.mostly_leave_pass(random);
            let mips = random.u32(&[0])..random.u32(&[1, made.desc.mip_levels]);
            let layers = random.u32(&[0])..random.u32(&[1, made.desc.depth_or_layers]);
            let value = if random.chance(50) {
                ClearValue::Color([random.next() as u32; 4])
            } else {
                ClearValue::DepthStencil {
                    depth: random.chance(70).then(|| random.f32(&[1.0]).to_bits()),
                    stencil: random.chance(50).then(|| random.next() as u8),
                }
            };
            let format = if random.chance(80) {
                made.desc.format
            } else {
                random.pick(FORMATS)
            };
            let cleared =
                self.b
                    .clear_texture(&made.texture, &made.desc, format, mips, layers, value);
            self.counts.call(cleared.is_err());
        } else {
            let Some(view) = any(random, &self.views) else {
                return;
            };
            let rects: Vec<ScissorRect> = (0..random.index(3))
                .map(|_| ScissorRect {
                    x: random.u32(&[0, 1]),
                    y: random.u32(&[0, 1]),
                    width: random.u32(&[1, 4]),
                    height: random.u32(&[1, 4]),
                })
                .collect();
            let format = random.pick(DRAWN_FORMATS);
            self.nearly_always_leave_pass(random);
            let cleared = self
                .b
                .clear_rects(&view, format, ClearValue::Color([0; 4]), &rects);
            self.counts.call(cleared.is_err());
        }
    }

    /// Generates mipmaps of random levels and layers of a random texture.
    fn generate_mips(&mut self, random: &mut Random) {
        let Some(made) = any(random, &self.textures) else {
            return;
        };
        let mips = random.u32(&[1])..random.u32(&[made.desc.mip_levels, 2]);
        let layers = random.u32(&[0])..random.u32(&[made.desc.depth_or_layers, 1]);
        self.mostly_leave_pass(random);
        let values = if random.chance(80) {
            made.desc.format
        } else {
            random.pick(FORMATS)
        };
        let generated = self
            .b
            .generate_mips(&made.texture, &made.desc, values, mips, layers);
        self.counts.call(generated.is_err());
    }

    /// Writes random parts into a ring.
    fn ring(&mut self, random: &mut Random) {
        let parts: Vec<Vec<u8>> = (0..random.index(4))
            .map(|_| random.bytes(&[0, 1, 3, 16, 300, 4096]))
            .collect();
        let parts: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
        let mut offsets = vec![0; parts.len()];
        let kind = random.pick(&[RingKind::Indices, RingKind::Data]);
        let align = random.u32(&[4, 256, 1]);
        let written = self.b.write_buffer_ring(kind, &parts, align, &mut offsets);
        self.counts.call(written.is_err());
        if let Ok(ring) = written
            && random.chance(10)
        {
            // The ring's other parts are written by later calls, so no byte of it is a canary.
            self.keep_buffer(random, (ring, None));
        }
    }

    /// Resolves random queries into a random buffer.
    fn resolve_queries(&mut self, random: &mut Random) {
        let (Some(set), Some((buffer, offset, _, shadow))) =
            (any(random, &self.queries), self.range(random))
        else {
            return;
        };
        self.mostly_leave_pass(random);
        let (first, count) = (random.u32(&[0, 1]), random.u32(&[0, 1, 2]));
        let resolved = self.b.resolve_queries(&set, first, count, &buffer, offset);
        self.counts.call(resolved.is_err());
        if resolved.is_ok() {
            allow(&(
                shadow,
                offset,
                u64::from(count).saturating_mul(RESULT_BYTES),
            ));
        }
    }

    /// Dispatches a random program with random bindings and groups.
    fn dispatch(&mut self, random: &mut Random) {
        let Some(program) = any(random, &self.programs) else {
            return;
        };
        if random.chance(80) {
            self.leave_pass();
        }
        let (set, writable) = any(random, &self.sets).unzip();
        let immediates = random.bytes(&[0, 0, 4, 16]);
        let args = self.range(random);
        // Counts past `max_dispatch` reach the dispatch as drawn, which must refuse them; a dispatch within it that
        // runs a great many groups is cut after that check: the bounds, not the GPU's time, are under test.
        let most = self.b.caps().limits.max_dispatch;
        let mut direct = [
            random.u32(&[1, 2, 4, 0]),
            random.u32(&[1, 2, 4, 0]),
            random.u32(&[1, 2, 0]),
        ];
        let refuse = direct.iter().any(|n| *n > most);
        let total = direct
            .iter()
            .fold(1u64, |total, n| total.saturating_mul(u64::from(*n)));
        if !refuse && total > 4096 {
            direct = [direct[0].min(64), direct[1].min(16), direct[2].min(4)];
        }
        let indirect = random.chance(10);
        let groups = match &args {
            Some((buffer, offset, _, _)) if indirect => DispatchGroups::Indirect {
                args: buffer,
                offset: *offset,
            },
            _ => DispatchGroups::Direct(direct),
        };
        let dispatched = self.b.dispatch(&DispatchCall {
            program: &program.program,
            bindings: [set.as_ref(), None, None, None],
            immediates: &immediates,
            groups,
        });
        self.counts.call(dispatched.is_err());
        if dispatched.is_ok() {
            if refuse && !indirect {
                eprintln!("ACCEPTED a dispatch of {direct:?} groups, past max_dispatch, {most}");
                self.counts.accepted = self.counts.accepted.saturating_add(1);
            }
            for range in writable.iter().flatten() {
                allow(range);
            }
        }
    }

    /// Calls the methods this implementation leaves absent, and timestamps.
    fn absent(&mut self, random: &mut Random) {
        let table = self.b.create_resource_table(random.u32(&[1, 16]));
        self.counts.call(table.is_err());
        if let Ok(table) = &table {
            let written = self.b.write_resource_table(table, random.u32(&[0]), &[]);
            self.counts.call(written.is_err());
        }
        if let Some(set) = any(random, &self.queries) {
            let written = self.b.write_timestamp(&set, random.u32(&[0, 1]));
            self.counts.call(written.is_err());
        }
        let disjoint = self.b.timestamps_disjoint();
        self.counts.call(disjoint.is_err());
    }

    /// Calls the methods that only report: caps, slow paths, status and helpers.
    fn bookkeeping(&mut self, random: &mut Random) {
        let _ = self.b.caps();
        let _ = self.b.slow_paths();
        let _ = self.b.status();
        let _ = self.b.helpers();
        self.b.record_slow_path(random.pick(SLOW_PATHS));
        let _ = self.b.take_slow_path_warnings();
        self.counts.call(false);
    }

    /// Destroys a random object, which work recorded may still use.
    fn destroy(&mut self, random: &mut Random) {
        let object = match random.below(8) {
            0 if !self.buffers.is_empty() => {
                let held = self.buffers.swap_remove(random.index(self.buffers.len()));
                self.check(&held);
                Object::Buffer(held.0)
            }
            1 if !self.textures.is_empty() => Object::Texture(
                self.textures
                    .swap_remove(random.index(self.textures.len()))
                    .texture,
            ),
            2 if !self.views.is_empty() => {
                Object::TextureView(self.views.swap_remove(random.index(self.views.len())))
            }
            3 if !self.sets.is_empty() => {
                Object::BindingSet(self.sets.swap_remove(random.index(self.sets.len())).0)
            }
            4 if !self.programs.is_empty() => Object::Program(
                self.programs
                    .swap_remove(random.index(self.programs.len()))
                    .program,
            ),
            5 if !self.queries.is_empty() => {
                Object::QuerySet(self.queries.swap_remove(random.index(self.queries.len())))
            }
            6 if !self.samplers.is_empty() => {
                Object::Sampler(self.samplers.swap_remove(random.index(self.samplers.len())))
            }
            _ if !self.surfaces.is_empty() => {
                Object::Surface(self.surfaces.swap_remove(random.index(self.surfaces.len())))
            }
            _ => return,
        };
        self.b.destroy(object);
        self.counts.call(false);
    }

    /// Ends the batch: ends the pass, submits, waits, and checks every buffer.
    fn finish(mut self) -> Counts {
        self.leave_pass();
        let fence = self.b.submit();
        self.b.wait_fence(&fence);
        for held in std::mem::take(&mut self.buffers) {
            self.check(&held);
        }
        self.counts
    }
}
