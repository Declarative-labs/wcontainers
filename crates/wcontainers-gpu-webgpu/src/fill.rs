//! Programs of the back end's own: draws where WebGPU has no command for the job (integer colour
//! clears the API would round through a float, `IntegerClearByDraw`; depth and stencil written from
//! a texture, `DepthWrittenByDraw` and `StencilWrittenByDraw`; depth and stencil drawn into a colour
//! texture to be copied out, `DepthReadByDraw`; plan section 4.2; a frame drawn into the window's
//! format, `PresentConverted`, 4.9), and the empty fragment stage of programs without one.

use std::collections::HashMap;

use wcontainers_gpu::{Format, FormatKind, SlowPath, TargetLayout};

use crate::{Stream, WebGpu, convert};

/// A triangle covering the viewport.
const COVER_WGSL: &str = "
@vertex fn cover(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(p * 2.0 - 1.0, 0.0, 1.0);
}
";

/// How the source texels of a depth draw become depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DepthSource {
    /// Read as a float: `r32float`, or a depth format's depth.
    Float,
    /// The bits of a 32-bit float in an `r32uint` or `r32sint` texture.
    FloatBits,
    /// 24-bit unorm depth in the low bits of an `r32uint` texture.
    Unorm24,
    /// 16-bit unorm depth as the integers of an `r16uint` texture.
    Unorm16,
}

impl DepthSource {
    /// How a copy into the depth of `dst` reads a source of `src`, a format of its typeless group.
    pub fn of(src: Format, dst: Format) -> Option<DepthSource> {
        use Format::*;
        match (dst, src) {
            (
                Depth32Float | Depth32FloatStencil8,
                R32Float | Depth32Float | Depth32FloatStencil8,
            ) => Some(DepthSource::Float),
            (Depth32Float | Depth32FloatStencil8, R32Uint | R32Sint) => {
                Some(DepthSource::FloatBits)
            }
            (Depth24Plus | Depth24PlusStencil8, Depth24Plus | Depth24PlusStencil8)
            | (Depth16Unorm, Depth16Unorm | R16Unorm) => Some(DepthSource::Float),
            (Depth16Unorm, R16Uint) => Some(DepthSource::Unorm16),
            _ => None,
        }
    }

    /// Whether a texture of `format` may be the source of a depth draw, so it is made bindable.
    pub fn readable(format: Format) -> bool {
        matches!(
            format,
            Format::R32Float
                | Format::R32Uint
                | Format::R32Sint
                | Format::R16Unorm
                | Format::R16Uint
        ) || matches!(format.kind(), FormatKind::Depth | FormatKind::DepthStencil)
    }
}

/// How a draw reading depth or stencil writes it into a colour texture (`DepthReadByDraw`): the
/// bytes a copy of that aspect into a buffer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AspectRead {
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
    pub fn of(format: Format, stencil: bool) -> Option<(AspectRead, Format)> {
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
    bits: [wgpu::RenderPipeline; 8],
    layout: wgpu::BindGroupLayout,
}

/// The programs of these draws, made at their first use.
#[derive(Default)]
pub struct Fills {
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

/// The source of a stencil draw's own program: the first instance names the bit, 8 the clear.
const STENCIL_WGSL: &str = "
@group(0) @binding(0) var<uniform> shift: vec4<i32>;
@group(0) @binding(1) var src: texture_2d<u32>;
struct Covered {
    @builtin(position) p: vec4f,
    @location(0) @interpolate(flat) bit: u32,
}
@vertex fn cover(@builtin(vertex_index) i: u32, @builtin(instance_index) bit: u32) -> Covered {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return Covered(vec4f(p * 2.0 - 1.0, 0.0, 1.0), bit);
}
@fragment fn stencil(c: Covered) {
    let s = textureLoad(src, vec2i(c.p.xy) + shift.xy, 0).x;
    if c.bit < 8u && ((s >> c.bit) & 1u) == 0u {
        discard;
    }
}
";

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

/// A 16-byte uniform buffer holding `words`.
fn uniform(device: &wgpu::Device, words: [u32; 4]) -> wgpu::Buffer {
    let b = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
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

impl WebGpu {
    /// A fragment stage writing nothing, for programs without one that draw into colour targets.
    pub(crate) fn no_fragment(&mut self) -> wgpu::ShaderModule {
        let device = &self.device;
        self.fills
            .no_fragment
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl("@fragment fn none() {}".into()),
                })
            })
            .clone()
    }

    fn cover(&mut self) -> wgpu::ShaderModule {
        let device = &self.device;
        self.fills
            .cover
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: None,
                    source: wgpu::ShaderSource::Wgsl(COVER_WGSL.into()),
                })
            })
            .clone()
    }

    /// A program drawing a whole target of `format` from one bound texture, the fragment stage
    /// `entry` of `source`.
    fn texel_program(
        &mut self,
        source: &str,
        entry: &str,
        multisampled: bool,
        format: wgpu::TextureFormat,
    ) -> DepthProgram {
        let cover = self.cover();
        let device = &self.device;
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
                entry_point: Some("cover"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(entry),
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
        &mut self,
        src: &wgpu::TextureView,
        dst: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) {
        self.slow.hit(SlowPath::ResolveByDraw);
        if !self.fills.resolves.contains_key(&format) {
            let program = self.texel_program(RESOLVE_WGSL, "average", true, format);
            self.fills.resolves.insert(format, program);
        }
        let program = &self.fills.resolves[&format];
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &program.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(src),
            }],
        });
        let pipeline = program.pipeline.clone();
        let mut rp = self
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
        rp.draw(0..3, 0..1);
    }

    /// Draws `frame` into `window`, a texture of its size in the window's own format
    /// (`PresentConverted`), in the frame's encoder.
    pub(crate) fn present_by_draw(&mut self, frame: &wgpu::Texture, window: &wgpu::Texture) {
        self.slow.hit(SlowPath::PresentConverted);
        let format = window.format();
        if !self.fills.presents.contains_key(&format) {
            let program = self.texel_program(PRESENT_WGSL, "copy", false, format);
            self.fills.presents.insert(format, program);
        }
        let program = &self.fills.presents[&format];
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &program.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &frame.create_view(&Default::default()),
                ),
            }],
        });
        let pipeline = program.pipeline.clone();
        let target = window.create_view(&Default::default());
        let mut rp = self
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
        rp.draw(0..3, 0..1);
    }

    /// Converts between the held texels of a texture of the converted `format` (level `held.1`,
    /// slices from `held.2`, the region at `held.3` of `extent`) and its raw blocks in `raw` (at
    /// its origin, a layer a slice), in the frame's encoder outside a pass: widening the blocks into
    /// the texture where `into`, else packing the texels into the blocks. One draw a slice.
    pub(crate) fn convert_by_draw(
        &mut self,
        format: Format,
        into: bool,
        held: (&wgpu::Texture, u32, u32, [u32; 2]),
        raw: &wgpu::Texture,
        extent: wcontainers_gpu::Extent,
    ) {
        let (texture, mip, first, [x, y]) = held;
        let three_d = texture.dimension() == wgpu::TextureDimension::D3;
        let key = (format, into, three_d && !into);
        // Held texels are moved as their bytes: an sRGB texture through its linear twin.
        let linear = texture.format().remove_srgb_suffix();
        if !self.fills.converts.contains_key(&key) {
            let target = if into { linear } else { raw.format() };
            let program = self.packing_program(format, into, key.2, target);
            self.fills.converts.insert(key, program);
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
        let (bw, _, _) = format.block();
        for l in 0..extent.depth_or_layers {
            let slice = first + l;
            let own = level_view(raw, wgpu::TextureAspect::All, 0, l);
            let held = match &whole_3d {
                Some(v) => v.clone(),
                None => held_view(Some(slice)),
            };
            let (source, target) = if into { (&own, &held) } else { (&held, &own) };
            let value = uniform(&self.device, [x, y, slice, extent.width]);
            let program = &self.fills.converts[&key];
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
            let mut rp = self
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
                ([0, 0], [extent.width.div_ceil(bw), extent.height])
            };
            let [w, h] = size.map(|n| n as f32);
            rp.set_viewport(at[0] as f32, at[1] as f32, w, h, 0.0, 1.0);
            rp.set_scissor_rect(at[0], at[1], size[0], size[1]);
            rp.set_pipeline(&pipeline);
            rp.set_bind_group(0, &group, &[]);
            rp.draw(0..3, 0..1);
        }
    }

    fn packing_program(
        &mut self,
        format: Format,
        into: bool,
        three_d: bool,
        target: wgpu::TextureFormat,
    ) -> DepthProgram {
        let cover = self.cover();
        let source = crate::packed::program(format, into, three_d);
        let module = self
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
                wgpu::TextureSampleType::Float { filterable: false },
                if d3 {
                    wgpu::TextureViewDimension::D3
                } else {
                    wgpu::TextureViewDimension::D2
                },
            ),
        };
        let layout = self
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
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        self.counters.pipelines += 1;
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &cover,
                    entry_point: Some("cover"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("convert"),
                    compilation_options: Default::default(),
                    targets: &[Some(target.into())],
                }),
                multiview_mask: None,
                cache: None,
            });
        DepthProgram { pipeline, layout }
    }

    /// Clears colour target `index` of the open pass, of an integer format, to `bits` by a draw
    /// covering it. The pass has just begun: its viewport and scissor cover the targets, and no
    /// query runs.
    pub(crate) fn clear_by_draw(
        &mut self,
        rp: &mut wgpu::RenderPass<'static>,
        targets: &TargetLayout,
        index: usize,
        bits: [u32; 4],
    ) {
        self.slow.hit(SlowPath::IntegerClearByDraw);
        let pipeline = match self.fills.clears.get(&(*targets, index)) {
            Some(p) => p.clone(),
            None => {
                let p = self.clear_pipeline(targets, index);
                self.fills.clears.insert((*targets, index), p.clone());
                p
            }
        };
        let value = uniform(&self.device, bits);
        let (layout, _) = self
            .fills
            .clear_layout
            .as_ref()
            .expect("made with the pipeline");
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: value.as_entire_binding(),
            }],
        });
        rp.set_pipeline(&pipeline);
        rp.set_bind_group(0, &group, &[]);
        rp.draw(0..3, 0..1);
    }

    fn clear_pipeline(&mut self, targets: &TargetLayout, index: usize) -> wgpu::RenderPipeline {
        let cover = self.cover();
        let device = &self.device;
        let (_, pipeline_layout) = self.fills.clear_layout.get_or_insert_with(|| {
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
        let (ty, value) = match format.kind() {
            FormatKind::Sint => ("vec4<i32>", "bitcast<vec4<i32>>(value)"),
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
        self.counters.pipelines += 1;
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cover,
                entry_point: Some("cover"),
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
                entry_point: Some("clear"),
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
        &mut self,
        stream: Stream,
        src: (&wgpu::Texture, Format, DepthSource),
        src_at: (u32, u32, [u32; 2]),
        dst: (&wgpu::Texture, Format),
        dst_at: (u32, u32, [u32; 2]),
        size: [u32; 2],
        layers: u32,
    ) {
        self.slow.hit(SlowPath::DepthWrittenByDraw);
        let (src_tex, src_format, how) = src;
        let (dst_tex, dst_format) = dst;
        let Some(raw_dst) = convert::texture_format(dst_format) else {
            self.fail("a depth draw into a format this back end does not hold");
            return;
        };
        let Some(raw_src) = convert::texture_format(src_format) else {
            self.fail("a depth draw from a format this back end does not hold");
            return;
        };
        if !self.fills.depths.contains_key(&(raw_dst, how)) {
            let program = self.depth_program(raw_dst, how);
            self.fills.depths.insert((raw_dst, how), program);
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
        let value = uniform(&self.device, shift);
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
            let program = &self.fills.depths[&(raw_dst, how)];
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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

            let mut rp = self
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
            rp.draw(0..3, 0..1);
        }
    }

    fn depth_program(&mut self, format: wgpu::TextureFormat, how: DepthSource) -> DepthProgram {
        let cover = self.cover();
        let (texture, depth) = match how {
            DepthSource::Float => ("texture_2d<f32>", "v.x"),
            DepthSource::FloatBits => ("texture_2d<u32>", "bitcast<f32>(v.x)"),
            DepthSource::Unorm24 => ("texture_2d<u32>", "f32(v.x & 0xffffffu) / 16777215.0"),
            DepthSource::Unorm16 => ("texture_2d<u32>", "f32(v.x) / 65535.0"),
        };
        let source = format!(
            "@group(0) @binding(0) var<uniform> shift: vec4<i32>;\n\
             @group(0) @binding(1) var src: {texture};\n\
             @fragment fn depth(@builtin(position) p: vec4f) -> @builtin(frag_depth) f32 {{\n\
                 let v = textureLoad(src, vec2i(p.xy) + shift.xy, 0);\n\
                 return {depth};\n\
             }}\n"
        );
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let sample_type = match how {
            DepthSource::Float => wgpu::TextureSampleType::Float { filterable: false },
            _ => wgpu::TextureSampleType::Uint,
        };
        let (layout, pipeline_layout) = loader_layout(&self.device, sample_type);
        self.counters.pipelines += 1;
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &cover,
                    entry_point: Some("cover"),
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
                    entry_point: Some("depth"),
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
        &mut self,
        stream: Stream,
        src: (&wgpu::Texture, wgpu::TextureAspect),
        src_at: (u32, u32, [u32; 2]),
        dst: &wgpu::Texture,
        dst_at: (u32, u32, [u32; 2]),
        size: [u32; 2],
        layers: u32,
    ) {
        self.slow.hit(SlowPath::StencilWrittenByDraw);
        let raw = dst.format();
        if !self.fills.stencils.contains_key(&raw) {
            let program = self.stencil_program(raw);
            self.fills.stencils.insert(raw, program);
        }
        let shift = [
            src_at.2[0].wrapping_sub(dst_at.2[0]),
            src_at.2[1].wrapping_sub(dst_at.2[1]),
            0,
            0,
        ];
        let value = uniform(&self.device, shift);
        for l in 0..layers {
            let from = level_view(src.0, src.1, src_at.0, src_at.1 + l);
            let to = level_view(dst, wgpu::TextureAspect::All, dst_at.0, dst_at.1 + l);
            let program = &self.fills.stencils[&raw];
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
            let mut rp = self
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
            rp.draw(0..3, 8..9);
            rp.set_stencil_reference(0xff);
            for (bit, pipeline) in (0u32..).zip(&bits) {
                rp.set_pipeline(pipeline);
                rp.draw(0..3, bit..bit + 1);
            }
        }
    }

    fn stencil_program(&mut self, format: wgpu::TextureFormat) -> StencilProgram {
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(STENCIL_WGSL.into()),
            });
        let (layout, pipeline_layout) = loader_layout(&self.device, wgpu::TextureSampleType::Uint);
        let depth = format.has_depth_aspect();
        let make = |write_mask: u32| {
            let face = wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Always,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: wgpu::StencilOperation::Replace,
            };
            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: None,
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("cover"),
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
                            read_mask: 0xff,
                            write_mask,
                        },
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some("stencil"),
                        compilation_options: Default::default(),
                        targets: &[],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        };
        let program = StencilProgram {
            clear: make(0xff),
            bits: std::array::from_fn(|bit| make(1 << bit)),
            layout,
        };
        self.counters.pipelines += 9;
        program
    }

    /// Draws the depth or stencil (`read`) of layers `src_at.1..` of `src` at level `src_at.0`, in
    /// the rectangle at `src_at.2` of `size`, into layers `0..layers` of the colour texture `dst` at
    /// its origin, outside a pass (`DepthReadByDraw`). `src` is single-sampled with binding usage.
    pub(crate) fn aspect_by_draw(
        &mut self,
        src: (&wgpu::Texture, AspectRead),
        src_at: (u32, u32, [u32; 2]),
        dst: &wgpu::Texture,
        size: [u32; 2],
        layers: u32,
    ) {
        self.slow.hit(SlowPath::DepthReadByDraw);
        let (src_tex, read) = src;
        if !self.fills.reads.contains_key(&read) {
            let program = self.read_program(read, dst.format());
            self.fills.reads.insert(read, program);
        }
        let aspect = match read {
            AspectRead::Stencil => wgpu::TextureAspect::StencilOnly,
            _ => wgpu::TextureAspect::DepthOnly,
        };
        let value = uniform(&self.device, [src_at.2[0], src_at.2[1], 0, 0]);
        for l in 0..layers {
            let from = level_view(src_tex, aspect, src_at.0, src_at.1 + l);
            let to = level_view(dst, wgpu::TextureAspect::All, 0, l);
            let program = &self.fills.reads[&read];
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
            let mut rp = self
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
            rp.draw(0..3, 0..1);
        }
    }

    fn read_program(&mut self, read: AspectRead, target: wgpu::TextureFormat) -> DepthProgram {
        let cover = self.cover();
        // The conversions are exact: a unorm value stored as the nearest float is within half a
        // step of its integer after one correctly rounded multiply.
        let (texture, value) = match read {
            AspectRead::Unorm16 => ("texture_2d<f32>", "u32(round(v.x * 65535.0))"),
            AspectRead::Unorm24 => ("texture_2d<f32>", "u32(round(v.x * 16777215.0))"),
            AspectRead::Float => ("texture_2d<f32>", "bitcast<u32>(v.x)"),
            AspectRead::Stencil => ("texture_2d<u32>", "v.x"),
        };
        let source = format!(
            "@group(0) @binding(0) var<uniform> shift: vec4<i32>;\n\
             @group(0) @binding(1) var src: {texture};\n\
             @fragment fn read(@builtin(position) p: vec4f) -> @location(0) vec4<u32> {{\n\
                 let v = textureLoad(src, vec2i(p.xy) + shift.xy, 0);\n\
                 return vec4<u32>({value}, 0u, 0u, 0u);\n\
             }}\n"
        );
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let sample_type = match read {
            AspectRead::Stencil => wgpu::TextureSampleType::Uint,
            _ => wgpu::TextureSampleType::Float { filterable: false },
        };
        let (layout, pipeline_layout) = loader_layout(&self.device, sample_type);
        self.counters.pipelines += 1;
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &cover,
                    entry_point: Some("cover"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("read"),
                    compilation_options: Default::default(),
                    targets: &[Some(target.into())],
                }),
                multiview_mask: None,
                cache: None,
            });
        DepthProgram { pipeline, layout }
    }
}
