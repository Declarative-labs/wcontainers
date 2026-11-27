//! Programs of the back end's own: draws where WebGPU has no command for the job (integer colour
//! clears the API would round through a float, `IntegerClearByDraw`; depth and stencil written from
//! a texture, `DepthWrittenByDraw` and `StencilWrittenByDraw`; depth and stencil drawn into a colour
//! texture to be copied out, `DepthReadByDraw`; plan section 4.2; a frame drawn into the window's
//! format, `PresentConverted`, 4.9), and the empty fragment stage of programs without one.

use std::{collections::HashMap, ops::Range};

use wcontainers_gpu::Gpu;
use wcontainers_gpu::{
    format::FORMAT_FACTS, format::Format, format::FormatKind, slow::SlowPath, state::TargetLayout,
};

use crate::{Stream, WebGpu, convert};

/// The vertices of the triangle covering the viewport.
pub(crate) const COVER_VERTICES: Range<u32> = 0..3;

/// Bits of 24-bit unorm depth (`depth24plus` as this back end reads and writes it).
const UNORM24_BITS: u32 = 24;

/// The largest 24-bit unorm depth's integer.
pub(crate) const UNORM24_MAX: u32 = (1 << UNORM24_BITS) - 1;

/// Bits of a stencil value: the stencil draw's programs, one a bit.
const STENCIL_BITS: usize = u8::BITS as usize;

/// The bits of a 32-bit float without its sign: above infinity's, the float is a NaN.
const FLOAT_MAGNITUDE_MASK: u32 = !(1 << (u32::BITS - 1));

/// Returns WGSL for the depth a draw writes from the float whose bits `bits` gives: a NaN becomes zero, as
/// plan 4.12 lists, whatever a device's depth clamp does with it (lavapipe makes it one).
fn depth_of_float_bits(bits: &str) -> String {
    let infinity = f32::INFINITY.to_bits();
    format!("select(bitcast<f32>({bits}), 0.0, ({bits} & {FLOAT_MAGNITUDE_MASK}u) > {infinity}u)")
}

/// The name of the empty fragment stage.
pub(crate) const NO_FRAGMENT: &str = "none";

/// The output type and value of a clear drawing a float format's bits, in WGSL.
const FLOAT_CLEAR_OUTPUT: (&str, &str) = ("vec4<f32>", "bitcast<vec4<f32>>(value)");

/// The texture a depth draw reads a float's bits from in an `r32sint` texture, in WGSL.
const SIGNED_BITS_TEXTURE: &str = "texture_2d<i32>";

/// A uniform of four words, as the programs' `vec4`.
pub(crate) type Words = [u32; 4];

/// A triangle covering the viewport.
const COVER_WGSL: &str = "
@vertex fn cover(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
";

/// How the source texels of a depth draw become depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DepthSource {
    /// Read as a float: `r32float`, or a depth format's depth.
    Float,
    /// The bits of a 32-bit float in an `r32uint` texture.
    FloatBits,
    /// The bits of a 32-bit float in an `r32sint` texture.
    FloatBitsSigned,
    /// 24-bit unorm depth in the low bits of an `r32uint` texture.
    Unorm24,
    /// A 24-bit depth texture's stored values, written again as the same values.
    Depth24,
    /// 16-bit unorm depth as the integers of an `r16uint` texture.
    Unorm16,
}

impl DepthSource {
    /// How a copy into the depth of `dst` reads a source of `src`, a format of its typeless group.
    pub(crate) fn of(src: Format, dst: Format) -> Option<DepthSource> {
        use Format::*;
        match (dst, src) {
            (
                Depth32Float | Depth32FloatStencil8,
                R32Float | Depth32Float | Depth32FloatStencil8,
            ) => Some(DepthSource::Float),
            (Depth32Float | Depth32FloatStencil8, R32Uint) => Some(DepthSource::FloatBits),
            (Depth32Float | Depth32FloatStencil8, R32Sint) => Some(DepthSource::FloatBitsSigned),
            (Depth24Plus | Depth24PlusStencil8, Depth24Plus | Depth24PlusStencil8) => {
                Some(DepthSource::Depth24)
            }
            (Depth16Unorm, Depth16Unorm | R16Unorm) => Some(DepthSource::Float),
            (Depth16Unorm, R16Uint) => Some(DepthSource::Unorm16),
            _ => None,
        }
    }

    /// Whether a texture of `format` may be the source of a depth draw, so it is made bindable.
    pub(crate) fn readable(format: Format) -> bool {
        matches!(
            format,
            Format::R32Float
                | Format::R32Uint
                | Format::R32Sint
                | Format::R16Unorm
                | Format::R16Uint
        ) || matches!(
            FORMAT_FACTS[format as usize].kind,
            FormatKind::Depth | FormatKind::DepthStencil
        )
    }
}

/// How a draw reading depth or stencil writes it into a colour texture (`DepthReadByDraw`): the
/// bytes a copy of that aspect into a buffer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AspectRead {
    /// 16-bit unorm depth into `r16uint`.
    Unorm16,
    /// 24-bit unorm depth into the low bits of `r32uint`.
    Unorm24,
    /// 32-bit float depth's bits into `r32uint`.
    Float,
    /// Stencil into `r8uint`.
    Stencil,
}

impl AspectRead {
    /// How the depth of `format`, or its stencil, is read, and the colour format it is drawn into.
    pub(crate) fn of(format: Format, stencil: bool) -> Option<(AspectRead, Format)> {
        use Format::*;
        Some(match (stencil, format) {
            (true, Stencil8 | Depth24PlusStencil8 | Depth32FloatStencil8) => {
                (AspectRead::Stencil, R8Uint)
            }
            (false, Depth16Unorm) => (AspectRead::Unorm16, R16Uint),
            (false, Depth24Plus | Depth24PlusStencil8) => (AspectRead::Unorm24, R32Uint),
            (false, Depth32Float | Depth32FloatStencil8) => (AspectRead::Float, R32Uint),
            _ => return None,
        })
    }
}

struct DepthProgram {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

/// The draws writing stencil from a texture of `r8uint` texels: one clearing the rectangle, then one
/// per bit setting it where the source has it.
struct StencilProgram {
    clear: wgpu::RenderPipeline,
    bits: [wgpu::RenderPipeline; STENCIL_BITS],
    layout: wgpu::BindGroupLayout,
}

/// The programs of these draws, made at their first use.
#[derive(Default)]
pub(crate) struct Fills {
    cover: Option<wgpu::ShaderModule>,
    no_fragment: Option<wgpu::ShaderModule>,
    clear_layout: Option<(wgpu::BindGroupLayout, wgpu::PipelineLayout)>,
    clears: HashMap<(TargetLayout, usize), wgpu::RenderPipeline>,
    depths: HashMap<(wgpu::TextureFormat, DepthSource), DepthProgram>,
    stencils: HashMap<wgpu::TextureFormat, StencilProgram>,
    reads: HashMap<AspectRead, DepthProgram>,
    presents: HashMap<wgpu::TextureFormat, DepthProgram>,
    resolves: HashMap<wgpu::TextureFormat, DepthProgram>,
    /// Keyed by the converted format, whether it widens into the texture, and whether that is 3D.
    converts: HashMap<(Format, bool, bool), DepthProgram>,
    /// Keyed by the packed format and the sample count.
    quantises: HashMap<(Format, u32), DepthProgram>,
    /// The copies packed targets are rounded from, by width, height and sample count.
    quantise_copies: HashMap<(u32, u32, u32), wgpu::Texture>,
}

/// One level and layer (or depth slice) of a texture of a 16-bit packed format a pass or resolve
/// drew into, rounded to the format's bits when it ends (`PackedTargetQuantised`).
pub(crate) struct PackedTarget {
    pub(crate) format: Format,
    pub(crate) texture: wgpu::Texture,
    pub(crate) mip: u32,
    /// The array layer, or the depth slice of a 3D texture.
    pub(crate) z: u32,
}

/// A view of one level and layer of `texture` in `aspect`, in that aspect's own format.
fn level_view(
    texture: &wgpu::Texture,
    aspect: wgpu::TextureAspect,
    mip: u32,
    layer: u32,
) -> wgpu::TextureView {
    let format = texture.format();
    texture.create_view(&wgpu::TextureViewDescriptor {
        label: None,
        format: Some(format.aspect_specific_format(aspect).unwrap_or(format)),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: None,
        aspect,
        base_mip_level: mip,
        mip_level_count: Some(1),
        base_array_layer: layer,
        array_layer_count: Some(1),
    })
}

/// The layout of a draw reading one texture at a shifted position: a uniform shift and the texture.
fn loader_layout(
    device: &wgpu::Device,
    sample_type: wgpu::TextureSampleType,
) -> (wgpu::BindGroupLayout, wgpu::PipelineLayout) {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            fragment_entry(0, UNIFORM),
            fragment_entry(
                1,
                wgpu::BindingType::Texture {
                    sample_type,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            ),
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    (layout, pipeline_layout)
}

/// The instance of a stencil draw that clears: past every bit's.
const STENCIL_CLEAR: u32 = STENCIL_BITS as u32;

/// The source of a stencil draw's own program: the first instance names the bit, `STENCIL_CLEAR`
/// the clear.
fn stencil_wgsl() -> String {
    format!(
        "
@group(0) @binding(0) var<uniform> shift: vec4<i32>;
@group(0) @binding(1) var src: texture_2d<u32>;
struct Covered {{
    @builtin(position) p: vec4f,
    @location(0) @interpolate(flat) bit: u32,
}}
@vertex fn cover(@builtin(vertex_index) i: u32, @builtin(instance_index) bit: u32) -> Covered {{
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return Covered(vec4f(p * 2.0 - 1.0, 0.0, 1.0), bit);
}}
@fragment fn stencil(c: Covered) {{
    let s = textureLoad(src, vec2i(c.p.xy) + shift.xy, 0).x;
    if c.bit < {STENCIL_CLEAR}u && ((s >> c.bit) & 1u) == 0u {{
        discard;
    }}
}}
"
    )
}

/// The fragment stage of a present draw: the frame's texel at the same place.
const PRESENT_WGSL: &str = "
@group(0) @binding(0) var frame: texture_2d<f32>;
@fragment fn copy(@builtin(position) p: vec4f) -> @location(0) vec4f {
    return textureLoad(frame, vec2i(p.xy), 0);
}
";

/// The fragment stage of a resolve by draw: the average of every sample at the same place, read
/// through a view in the format the resolve names.
const RESOLVE_WGSL: &str = "
@group(0) @binding(0) var samples: texture_multisampled_2d<f32>;
@fragment fn average(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let n = textureNumSamples(samples);
    var sum = vec4f(0.0);
    for (var i = 0u; i < n; i++) {
        sum += textureLoad(samples, vec2i(p.xy), i);
    }
    return sum / f32(n);
}
";

/// A uniform buffer holding `words`.
fn uniform(device: &wgpu::Device, words: Words) -> wgpu::Buffer {
    let b = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: size_of::<Words>() as u64,
        usage: wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: true,
    });
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    b.slice(..)
        .get_mapped_range_mut()
        .expect("a buffer mapped at creation")
        .copy_from_slice(&bytes);
    b.unmap();
    b
}

fn fragment_entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    }
}

/// Loads and stores an attachment, keeping what it holds.
fn keep<V>() -> wgpu::Operations<V> {
    wgpu::Operations {
        load: wgpu::LoadOp::Load,
        store: wgpu::StoreOp::Store,
    }
}

const UNIFORM: wgpu::BindingType = wgpu::BindingType::Buffer {
    ty: wgpu::BufferBindingType::Uniform,
    has_dynamic_offset: false,
    min_binding_size: None,
};

/// A fragment stage writing nothing, for programs without one that draw into colour targets.
pub(crate) fn no_fragment(gpu: &mut WebGpu) -> wgpu::ShaderModule {
    let device = &gpu.device;
    gpu.fills
        .no_fragment
        .get_or_insert_with(|| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(
                    format!("@fragment fn {NO_FRAGMENT}() {{}}").into(),
                ),
            })
        })
        .clone()
}

fn cover(gpu: &mut WebGpu) -> wgpu::ShaderModule {
    let device = &gpu.device;
    gpu.fills
        .cover
        .get_or_insert_with(|| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(COVER_WGSL.into()),
            })
        })
        .clone()
}

/// A program drawing a whole target of `format` and `samples` from one bound texture, the
/// fragment stage of `source`.
fn texel_program(
    gpu: &mut WebGpu,
    source: &str,
    multisampled: bool,
    format: wgpu::TextureFormat,
    samples: u32,
) -> DepthProgram {
    let cover = cover(gpu);
    let device = &gpu.device;
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[fragment_entry(
            0,
            wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled,
            },
        )],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &cover,
            entry_point: None,
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            targets: &[Some(format.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    DepthProgram { pipeline, layout }
}

/// Resolves `src` (multisampled, bound for sampling) into `dst`, both views in the format the
/// resolve names, by a draw averaging every sample (`ResolveByDraw`).
pub(crate) fn resolve_by_draw(
    gpu: &mut WebGpu,
    src: &wgpu::TextureView,
    dst: &wgpu::TextureView,
    format: wgpu::TextureFormat,
) {
    gpu.record_slow_path(SlowPath::ResolveByDraw);
    if !gpu.fills.resolves.contains_key(&format) {
        let program = texel_program(gpu, RESOLVE_WGSL, true, format, 1);
        gpu.fills.resolves.insert(format, program);
    }
    let program = &gpu.fills.resolves[&format];
    let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &program.layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(src),
        }],
    });
    let pipeline = program.pipeline.clone();
    let mut rp = gpu
        .encoder()
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: keep(),
            })],
            ..Default::default()
        });
    rp.set_pipeline(&pipeline);
    rp.set_bind_group(0, &group, &[]);
    rp.draw(COVER_VERTICES, 0..1);
}

/// Rounds each channel of `target`, held in `rgba8unorm`, to its packed format's bits: copies
/// the subresource and draws it back from the copy, per sample where multisampled
/// (`PackedTargetQuantised`). In the frame's encoder, outside a pass.
pub(crate) fn quantise(gpu: &mut WebGpu, target: &PackedTarget) {
    gpu.record_slow_path(SlowPath::PackedTargetQuantised);
    let samples = target.texture.sample_count();
    let size = target
        .texture
        .size()
        .mip_level_size(target.mip, target.texture.dimension());
    let (width, height) = (size.width, size.height);
    let device = &gpu.device;
    let copy = gpu
        .fills
        .quantise_copies
        .entry((width, height, samples))
        .or_insert_with(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                // WebGPU makes multisampled textures only as attachments.
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        })
        .clone();
    let key = (target.format, samples);
    if !gpu.fills.quantises.contains_key(&key) {
        let source = crate::packed::quantise_program(target.format, samples > 1);
        let program = texel_program(
            gpu,
            &source,
            samples > 1,
            wgpu::TextureFormat::Rgba8Unorm,
            samples,
        );
        gpu.fills.quantises.insert(key, program);
    }
    let program = &gpu.fills.quantises[&key];
    let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &program.layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&copy.create_view(&Default::default())),
        }],
    });
    let pipeline = program.pipeline.clone();
    let three_d = target.texture.dimension() == wgpu::TextureDimension::D3;
    let view = target.texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_mip_level: target.mip,
        mip_level_count: Some(1),
        base_array_layer: if three_d { 0 } else { target.z },
        array_layer_count: Some(1),
        ..Default::default()
    });
    let encoder = gpu.encoder();
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &target.texture,
            mip_level: target.mip,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: target.z,
            },
            aspect: wgpu::TextureAspect::All,
        },
        copy.as_image_copy(),
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: three_d.then_some(target.z),
            resolve_target: None,
            ops: keep(),
        })],
        ..Default::default()
    });
    rp.set_pipeline(&pipeline);
    rp.set_bind_group(0, &group, &[]);
    rp.draw(COVER_VERTICES, 0..1);
}

/// Draws `frame` into `window`, a texture of its size in the window's own format
/// (`PresentConverted`), in the frame's encoder.
pub(crate) fn present_by_draw(gpu: &mut WebGpu, frame: &wgpu::Texture, window: &wgpu::Texture) {
    gpu.record_slow_path(SlowPath::PresentConverted);
    let format = window.format();
    if !gpu.fills.presents.contains_key(&format) {
        let program = texel_program(gpu, PRESENT_WGSL, false, format, 1);
        gpu.fills.presents.insert(format, program);
    }
    let program = &gpu.fills.presents[&format];
    let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &program.layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&frame.create_view(&Default::default())),
        }],
    });
    let pipeline = program.pipeline.clone();
    let target = window.create_view(&Default::default());
    let mut rp = gpu
        .stream(Stream::Main)
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    rp.set_pipeline(&pipeline);
    rp.set_bind_group(0, &group, &[]);
    rp.draw(COVER_VERTICES, 0..1);
}

/// Converts between the held texels of a texture of the converted `format` (level `held.1`,
/// slices from `held.2`, the region at `held.3` of `extent`) and its raw blocks in `raw` (at
/// its origin, a layer a slice), in the frame's encoder outside a pass: widening the blocks into
/// the texture where `into`, else packing the texels into the blocks. One draw a slice.
pub(crate) fn convert_by_draw(
    gpu: &mut WebGpu,
    format: Format,
    into: bool,
    held: (&wgpu::Texture, u32, u32, [u32; 2]),
    raw: &wgpu::Texture,
    extent: wcontainers_gpu::resource::Extent,
) {
    let (texture, mip, first, [x, y]) = held;
    let three_d = texture.dimension() == wgpu::TextureDimension::D3;
    let key = (format, into, three_d && !into);
    // Held texels are moved as their bytes: an sRGB texture through its linear twin.
    let linear = texture.format().remove_srgb_suffix();
    if !gpu.fills.converts.contains_key(&key) {
        let target = if into { linear } else { raw.format() };
        let program = packing_program(gpu, format, into, key.2, target);
        gpu.fills.converts.insert(key, program);
    }
    let held_view = |slice: Option<u32>| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(linear),
            dimension: Some(match slice {
                Some(_) => wgpu::TextureViewDimension::D2,
                None => wgpu::TextureViewDimension::D3,
            }),
            base_mip_level: mip,
            mip_level_count: Some(1),
            base_array_layer: slice.unwrap_or(0),
            array_layer_count: Some(1),
            ..Default::default()
        })
    };
    let whole_3d = three_d.then(|| held_view(None));
    for l in 0..extent.depth_or_layers {
        let slice = first + l;
        let own = level_view(raw, wgpu::TextureAspect::All, 0, l);
        let held = match &whole_3d {
            Some(v) => v.clone(),
            None => held_view(Some(slice)),
        };
        let (source, target) = if into { (&own, &held) } else { (&held, &own) };
        let value = uniform(&gpu.device, [x, y, slice, extent.width]);
        let program = &gpu.fills.converts[&key];
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &program.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: value.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(source),
                },
            ],
        });
        let pipeline = program.pipeline.clone();
        let mut rp = gpu
            .stream(Stream::Main)
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: (into && three_d).then_some(slice),
                    resolve_target: None,
                    ops: keep(),
                })],
                ..Default::default()
            });
        let (at, size) = if into {
            ([x, y], [extent.width, extent.height])
        } else {
            (
                [0, 0],
                [
                    crate::packed::raw_width(format, extent.width),
                    extent.height,
                ],
            )
        };
        let [w, h] = size.map(|n| n as f32);
        rp.set_viewport(at[0] as f32, at[1] as f32, w, h, 0.0, 1.0);
        rp.set_scissor_rect(at[0], at[1], size[0], size[1]);
        rp.set_pipeline(&pipeline);
        rp.set_bind_group(0, &group, &[]);
        rp.draw(COVER_VERTICES, 0..1);
    }
}

fn packing_program(
    gpu: &mut WebGpu,
    format: Format,
    into: bool,
    three_d: bool,
    target: wgpu::TextureFormat,
) -> DepthProgram {
    let cover = cover(gpu);
    let source = crate::packed::program(format, into, three_d);
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let (sample_type, view_dimension) = match (into, three_d) {
        (true, _) => (
            wgpu::TextureSampleType::Uint,
            wgpu::TextureViewDimension::D2,
        ),
        (false, d3) => (
            crate::packed::held_sample_type(format),
            if d3 {
                wgpu::TextureViewDimension::D3
            } else {
                wgpu::TextureViewDimension::D2
            },
        ),
    };
    let layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                fragment_entry(0, UNIFORM),
                fragment_entry(
                    1,
                    wgpu::BindingType::Texture {
                        sample_type,
                        view_dimension,
                        multisampled: false,
                    },
                ),
            ],
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
    gpu.counters.pipelines += 1;
    let pipeline = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cover,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: None,
                compilation_options: Default::default(),
                targets: &[Some(target.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
    DepthProgram { pipeline, layout }
}

/// Clears colour target `index` of the open pass to `bits` (an integer format's value, or a
/// float format's bits) by a draw covering it. The pass has just begun: its viewport and
/// scissor cover the targets, and no query runs.
pub(crate) fn clear_by_draw(
    gpu: &mut WebGpu,
    rp: &mut wgpu::RenderPass<'static>,
    targets: &TargetLayout,
    index: usize,
    bits: Words,
) {
    let float =
        targets.colors[index].is_some_and(|f| FORMAT_FACTS[f as usize].kind == FormatKind::Float);
    gpu.record_slow_path(if float {
        SlowPath::FloatBitsClearByDraw
    } else {
        SlowPath::IntegerClearByDraw
    });
    let pipeline = match gpu.fills.clears.get(&(*targets, index)) {
        Some(p) => p.clone(),
        None => {
            let p = clear_pipeline(gpu, targets, index);
            gpu.fills.clears.insert((*targets, index), p.clone());
            p
        }
    };
    let value = uniform(&gpu.device, bits);
    let (layout, _) = gpu
        .fills
        .clear_layout
        .as_ref()
        .expect("made with the pipeline");
    let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: value.as_entire_binding(),
        }],
    });
    rp.set_pipeline(&pipeline);
    rp.set_bind_group(0, &group, &[]);
    rp.draw(COVER_VERTICES, 0..1);
}

fn clear_pipeline(gpu: &mut WebGpu, targets: &TargetLayout, index: usize) -> wgpu::RenderPipeline {
    let cover = cover(gpu);
    let device = &gpu.device;
    let (_, pipeline_layout) = gpu.fills.clear_layout.get_or_insert_with(|| {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[fragment_entry(0, UNIFORM)],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        (layout, pl)
    });
    let format = targets.colors[index].expect("the cleared target");
    let (ty, value) = match FORMAT_FACTS[format as usize].kind {
        FormatKind::Sint => ("vec4<i32>", "bitcast<vec4<i32>>(value)"),
        FormatKind::Float => FLOAT_CLEAR_OUTPUT,
        _ => ("vec4<u32>", "value"),
    };
    let source = format!(
        "@group(0) @binding(0) var<uniform> value: vec4<u32>;\n\
         @fragment fn clear() -> @location({index}) {ty} {{ return {value}; }}\n"
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let used = targets
        .colors
        .iter()
        .rposition(Option::is_some)
        .map_or(0, |i| i + 1);
    let colors: Vec<Option<wgpu::ColorTargetState>> = targets.colors[..used]
        .iter()
        .enumerate()
        .map(|(i, f)| {
            f.map(|f| wgpu::ColorTargetState {
                format: convert::texture_format(f).unwrap_or(wgpu::TextureFormat::R8Unorm),
                blend: None,
                write_mask: if i == index {
                    wgpu::ColorWrites::ALL
                } else {
                    wgpu::ColorWrites::empty()
                },
            })
        })
        .collect();
    let depth_stencil = targets.depth_stencil.map(|f| wgpu::DepthStencilState {
        format: convert::texture_format(f).unwrap_or(wgpu::TextureFormat::Depth32Float),
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Always),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    });
    gpu.counters.pipelines += 1;
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: &cover,
            entry_point: None,
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil,
        multisample: wgpu::MultisampleState {
            count: targets.samples,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            targets: &colors,
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Writes the depth of layers `dst_layer..` of `dst` at `dst_mip`, in the rectangle at
/// `dst_origin` of `size`, from the texels of `src` at `src_mip`, `src_layer..` and `src_origin`,
/// one draw per layer in `stream`, outside a pass (`DepthWrittenByDraw`). Both textures are
/// single-sampled; `src` has binding usage and `dst` render usage.
#[allow(clippy::too_many_arguments)]
pub(crate) fn depth_by_draw(
    gpu: &mut WebGpu,
    stream: Stream,
    src: (&wgpu::Texture, Format, DepthSource),
    src_at: (u32, u32, [u32; 2]),
    dst: (&wgpu::Texture, Format),
    dst_at: (u32, u32, [u32; 2]),
    size: [u32; 2],
    layers: u32,
) {
    gpu.record_slow_path(SlowPath::DepthWrittenByDraw);
    let (src_tex, src_format, how) = src;
    let (dst_tex, dst_format) = dst;
    let Some(raw_dst) = convert::texture_format(dst_format) else {
        gpu.fail("a depth draw into a format this back end does not hold");
        return;
    };
    let Some(raw_src) = convert::texture_format(src_format) else {
        gpu.fail("a depth draw from a format this back end does not hold");
        return;
    };
    if !gpu.fills.depths.contains_key(&(raw_dst, how)) {
        let program = depth_program(gpu, raw_dst, how);
        gpu.fills.depths.insert((raw_dst, how), program);
    }
    let src_aspect = convert::aspect(false, src_format);
    let src_view_format = raw_src
        .aspect_specific_format(src_aspect)
        .unwrap_or(raw_src);
    // The source texel a fragment reads is its own position moved by this shift.
    let shift = [
        src_at.2[0].wrapping_sub(dst_at.2[0]),
        src_at.2[1].wrapping_sub(dst_at.2[1]),
        0,
        0,
    ];
    let value = uniform(&gpu.device, shift);
    let view = |t: &wgpu::Texture, format, aspect, mip, layer| {
        t.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(format),
            dimension: Some(wgpu::TextureViewDimension::D2),
            usage: None,
            aspect,
            base_mip_level: mip,
            mip_level_count: Some(1),
            base_array_layer: layer,
            array_layer_count: Some(1),
        })
    };
    for l in 0..layers {
        let from = view(src_tex, src_view_format, src_aspect, src_at.0, src_at.1 + l);
        let to = view(
            dst_tex,
            raw_dst,
            wgpu::TextureAspect::All,
            dst_at.0,
            dst_at.1 + l,
        );
        let program = &gpu.fills.depths[&(raw_dst, how)];
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &program.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: value.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&from),
                },
            ],
        });
        let pipeline = program.pipeline.clone();

        let mut rp = gpu
            .stream(stream)
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &to,
                    depth_ops: Some(keep()),
                    stencil_ops: crate::has_stencil(dst_format).then(keep),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        let (x, y) = (dst_at.2[0], dst_at.2[1]);
        rp.set_viewport(x as f32, y as f32, size[0] as f32, size[1] as f32, 0.0, 1.0);
        rp.set_scissor_rect(x, y, size[0], size[1]);
        rp.set_pipeline(&pipeline);
        rp.set_bind_group(0, &group, &[]);
        rp.draw(COVER_VERTICES, 0..1);
    }
}

/// WGSL for the float nearest the 24-bit unorm value at [`VALUE`], by integers only.
///
/// The value times 2^-24 is exact and the quotient lies above it by more than half a float step and
/// at most one, so the nearest float is its bits plus one (zero stays zero); a sum of products may be
/// regrouped (Mesa does) and WGSL's division may be 2.5 ulp off.
const NEAREST_OF_UNORM24: &str =
    "bitcast<f32>(bitcast<u32>(f32({V}) * 0x1p-24f) + select(0u, 1u, {V} != 0u))";
/// Where [`NEAREST_OF_UNORM24`] takes its value.
const VALUE: &str = "{V}";
/// The stored 24-bit value of a sampled depth `v.x`, by [`unorm24_functions`].
const UNORM24_OF_SAMPLE: &str = "unorm24(v.x)";
/// A texture sampled as floats.
const FLOAT_TEXTURE: &str = "texture_2d<f32>";

fn depth_program(gpu: &mut WebGpu, format: wgpu::TextureFormat, how: DepthSource) -> DepthProgram {
    let cover = cover(gpu);
    let mut functions = String::new();
    let (texture, depth) = match how {
        DepthSource::Float => ("texture_2d<f32>", depth_of_float_bits("bitcast<u32>(v.x)")),
        DepthSource::FloatBits => ("texture_2d<u32>", depth_of_float_bits("v.x")),
        DepthSource::FloatBitsSigned => (
            SIGNED_BITS_TEXTURE,
            depth_of_float_bits("bitcast<u32>(v.x)"),
        ),
        DepthSource::Unorm24 => (
            "texture_2d<u32>",
            NEAREST_OF_UNORM24.replace(VALUE, &format!("(v.x & {UNORM24_MAX}u)")),
        ),
        // The stored value found by the device's own conversion, as a read finds it, and written as
        // its nearest float: the sampled float itself can store as the next value (llvmpipe samples
        // k as k times the rounded reciprocal, up to two float steps above k / (2^24 - 1)).
        DepthSource::Depth24 => {
            functions = unorm24_functions(gpu);
            (
                FLOAT_TEXTURE,
                NEAREST_OF_UNORM24.replace(VALUE, UNORM24_OF_SAMPLE),
            )
        }
        DepthSource::Unorm16 => ("texture_2d<u32>", format!("f32(v.x) / {}.0", u16::MAX)),
    };
    let source = format!(
        "@group(0) @binding(0) var<uniform> shift: vec4<i32>;\n\
         @group(0) @binding(1) var src: {texture};\n\
         {functions}\
         @fragment fn depth(@builtin(position) p: vec4f) -> @builtin(frag_depth) f32 {{\n\
             let v = textureLoad(src, vec2i(p.xy) + shift.xy, 0);\n\
             return {depth};\n\
         }}\n"
    );
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let sample_type = match how {
        DepthSource::Float | DepthSource::Depth24 => {
            wgpu::TextureSampleType::Float { filterable: false }
        }
        DepthSource::FloatBitsSigned => wgpu::TextureSampleType::Sint,
        _ => wgpu::TextureSampleType::Uint,
    };
    let (layout, pipeline_layout) = loader_layout(&gpu.device, sample_type);
    gpu.counters.pipelines += 1;
    let pipeline = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cover,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: None,
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });
    DepthProgram { pipeline, layout }
}

/// Writes the stencil of layers `dst_at.1..` of `dst` at level `dst_at.0`, in the rectangle at
/// `dst_at.2` of `size`, from the `u32` texels of `src` in `aspect` (an `r8uint` texture or a
/// stencil) at `src_at`, in `stream`, outside a pass (`StencilWrittenByDraw`): per layer one draw
/// clears the rectangle's stencil, then one per bit sets that bit where the source has it. `src`
/// is single-sampled with binding usage, `dst` has render usage.
#[allow(clippy::too_many_arguments)]
pub(crate) fn stencil_by_draw(
    gpu: &mut WebGpu,
    stream: Stream,
    src: (&wgpu::Texture, wgpu::TextureAspect),
    src_at: (u32, u32, [u32; 2]),
    dst: &wgpu::Texture,
    dst_at: (u32, u32, [u32; 2]),
    size: [u32; 2],
    layers: u32,
) {
    gpu.record_slow_path(SlowPath::StencilWrittenByDraw);
    let raw = dst.format();
    if !gpu.fills.stencils.contains_key(&raw) {
        let program = stencil_program(gpu, raw);
        gpu.fills.stencils.insert(raw, program);
    }
    let shift = [
        src_at.2[0].wrapping_sub(dst_at.2[0]),
        src_at.2[1].wrapping_sub(dst_at.2[1]),
        0,
        0,
    ];
    let value = uniform(&gpu.device, shift);
    for l in 0..layers {
        let from = level_view(src.0, src.1, src_at.0, src_at.1 + l);
        let to = level_view(dst, wgpu::TextureAspect::All, dst_at.0, dst_at.1 + l);
        let program = &gpu.fills.stencils[&raw];
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &program.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: value.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&from),
                },
            ],
        });
        let (clear, bits) = (program.clear.clone(), program.bits.clone());
        let mut rp = gpu
            .stream(stream)
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &to,
                    depth_ops: raw.has_depth_aspect().then(keep),
                    stencil_ops: Some(keep()),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        let (x, y) = (dst_at.2[0], dst_at.2[1]);
        rp.set_viewport(x as f32, y as f32, size[0] as f32, size[1] as f32, 0.0, 1.0);
        rp.set_scissor_rect(x, y, size[0], size[1]);
        rp.set_bind_group(0, &group, &[]);
        rp.set_pipeline(&clear);
        rp.set_stencil_reference(0);
        rp.draw(COVER_VERTICES, STENCIL_CLEAR..STENCIL_CLEAR + 1);
        rp.set_stencil_reference(u8::MAX.into());
        for (bit, pipeline) in (0u32..).zip(&bits) {
            rp.set_pipeline(pipeline);
            rp.draw(COVER_VERTICES, bit..bit + 1);
        }
    }
}

fn stencil_program(gpu: &mut WebGpu, format: wgpu::TextureFormat) -> StencilProgram {
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(stencil_wgsl().into()),
        });
    let (layout, pipeline_layout) = loader_layout(&gpu.device, wgpu::TextureSampleType::Uint);
    let depth = format.has_depth_aspect();
    let make = |write_mask: u32| {
        let face = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Replace,
        };
        gpu.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: None,
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format,
                    depth_write_enabled: depth.then_some(false),
                    depth_compare: depth.then_some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState {
                        front: face,
                        back: face,
                        read_mask: u8::MAX.into(),
                        write_mask,
                    },
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: None,
                    compilation_options: Default::default(),
                    targets: &[],
                }),
                multiview_mask: None,
                cache: None,
            })
    };
    let program = StencilProgram {
        clear: make(u8::MAX.into()),
        bits: std::array::from_fn(|bit| make(1 << bit)),
        layout,
    };
    gpu.counters.pipelines += program.bits.len() as u64 + 1;
    program
}

/// Draws the depth or stencil (`read`) of layers `src_at.1..` of `src` at level `src_at.0`, in
/// the rectangle at `src_at.2` of `size`, into layers `0..layers` of the colour texture `dst` at
/// its origin, outside a pass (`DepthReadByDraw`). `src` is single-sampled with binding usage.
pub(crate) fn aspect_by_draw(
    gpu: &mut WebGpu,
    src: (&wgpu::Texture, AspectRead),
    src_at: (u32, u32, [u32; 2]),
    dst: &wgpu::Texture,
    size: [u32; 2],
    layers: u32,
) {
    gpu.record_slow_path(SlowPath::DepthReadByDraw);
    let (src_tex, read) = src;
    if !gpu.fills.reads.contains_key(&read) {
        let program = read_program(gpu, read, dst.format());
        gpu.fills.reads.insert(read, program);
    }
    let aspect = match read {
        AspectRead::Stencil => wgpu::TextureAspect::StencilOnly,
        _ => wgpu::TextureAspect::DepthOnly,
    };
    let value = uniform(&gpu.device, [src_at.2[0], src_at.2[1], 0, 0]);
    for l in 0..layers {
        let from = level_view(src_tex, aspect, src_at.0, src_at.1 + l);
        let to = level_view(dst, wgpu::TextureAspect::All, 0, l);
        let program = &gpu.fills.reads[&read];
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &program.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: value.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&from),
                },
            ],
        });
        let pipeline = program.pipeline.clone();
        let mut rp = gpu
            .stream(Stream::Main)
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &to,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        rp.set_viewport(0.0, 0.0, size[0] as f32, size[1] as f32, 0.0, 1.0);
        rp.set_scissor_rect(0, 0, size[0], size[1]);
        rp.set_pipeline(&pipeline);
        rp.set_bind_group(0, &group, &[]);
        rp.draw(COVER_VERTICES, 0..1);
    }
}

/// The 24-bit value stored for a sampled depth `d`, by the device's own conversion: `{SAMPLED}` is the
/// float the device gives for a stored `c`. Rounding d (2^24 - 1) alone can be one off either way: in the
/// upper part of each binade the nearest float and a product with the rounded reciprocal give one float
/// for neighbouring values, and the product itself rounds. So the rounded value and its two neighbours
/// are each converted as the device does, the one giving d's bits kept (the rounded value where none does).
const UNORM24_READ: &str = "fn sampled24(c: u32) -> u32 {
    return {SAMPLED};
}
fn unorm24(d: f32) -> u32 {
    let e = min(u32(round(d * {MAX}.0)), {MAX}u);
    let bits = bitcast<u32>(d);
    if (sampled24(e) == bits) { return e; }
    if (e > 0u && sampled24(e - 1u) == bits) { return e - 1u; }
    if (e < {MAX}u && sampled24(e + 1u) == bits) { return e + 1u; }
    return e;
}
";

/// The float a device sampling stored 24-bit depth `c` as the float nearest c / (2^24 - 1) gives (as
/// Direct3D asks): c 2^-24 is exact and the quotient more than half and at most one of its float steps
/// above it, so the nearest float is its bits plus one.
const SAMPLED_NEAREST: &str = "select(bitcast<u32>(f32(c) * 0x1p-24f) + 1u, 0u, c == 0u)";
/// The float a device sampling stored 24-bit depth `c` as c times the float nearest 1 / (2^24 - 1),
/// 2^-24 (1 + 2^-23), gives (llvmpipe and lavapipe).
const SAMPLED_PRODUCT: &str = "bitcast<u32>(f32(c) * 0x1.000002p-24f)";

/// Writes the WGSL function `unorm24(d)`: the 24-bit value this device stored for a sampled depth.
fn unorm24_functions(gpu: &WebGpu) -> String {
    let sampled = if gpu.depth24_by_product {
        SAMPLED_PRODUCT
    } else {
        SAMPLED_NEAREST
    };
    UNORM24_READ
        .replace("{SAMPLED}", sampled)
        .replace("{MAX}", &UNORM24_MAX.to_string())
}

fn read_program(gpu: &mut WebGpu, read: AspectRead, target: wgpu::TextureFormat) -> DepthProgram {
    let cover = cover(gpu);
    // The conversions are exact: a unorm value stored as the nearest float is within half a
    // step of its integer after one correctly rounded multiply; 24-bit depth is checked against
    // the device's own conversion, which the probe at creation found.
    let mut functions = String::new();
    let (texture, value) = match read {
        AspectRead::Unorm16 => (
            "texture_2d<f32>",
            format!("u32(round(v.x * {}.0))", u16::MAX),
        ),
        AspectRead::Unorm24 => {
            functions = unorm24_functions(gpu);
            (FLOAT_TEXTURE, UNORM24_OF_SAMPLE.to_string())
        }
        AspectRead::Float => ("texture_2d<f32>", "bitcast<u32>(v.x)".into()),
        AspectRead::Stencil => ("texture_2d<u32>", "v.x".into()),
    };
    let source = format!(
        "@group(0) @binding(0) var<uniform> shift: vec4<i32>;\n\
         @group(0) @binding(1) var src: {texture};\n\
         {functions}\
         @fragment fn read(@builtin(position) p: vec4f) -> @location(0) vec4<u32> {{\n\
             let v = textureLoad(src, vec2i(p.xy) + shift.xy, 0);\n\
             return vec4<u32>({value}, 0u, 0u, 0u);\n\
         }}\n"
    );
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let sample_type = match read {
        AspectRead::Stencil => wgpu::TextureSampleType::Uint,
        _ => wgpu::TextureSampleType::Float { filterable: false },
    };
    let (layout, pipeline_layout) = loader_layout(&gpu.device, sample_type);
    gpu.counters.pipelines += 1;
    let pipeline = gpu
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cover,
                entry_point: None,
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: None,
                compilation_options: Default::default(),
                targets: &[Some(target.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
    DepthProgram { pipeline, layout }
}
