//! Programs of the back end's own: draws where WebGPU has no command for the job (integer colour
//! clears the API would round through a float, `IntegerClearByDraw`; depth written from a texture,
//! `DepthWrittenByDraw`, plan section 4.2), and the empty fragment stage of programs without one.

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
            | (Depth16Unorm, Depth16Unorm) => Some(DepthSource::Float),
            _ => None,
        }
    }

    /// Whether a texture of `format` may be the source of a depth draw, so it is made bindable.
    pub fn readable(format: Format) -> bool {
        matches!(format, Format::R32Float | Format::R32Uint | Format::R32Sint)
            || matches!(format.kind(), FormatKind::Depth | FormatKind::DepthStencil)
    }
}

struct DepthProgram {
    pipeline: wgpu::RenderPipeline,
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
}

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
                            view_dimension: wgpu::TextureViewDimension::D2,
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
}
