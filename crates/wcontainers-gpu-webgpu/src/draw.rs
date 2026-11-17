//! Blocks, binding sets, the pipeline and bind group caches, passes and draws (plan sections 4.4, 4.5
//! and 4.7).

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::num::NonZeroU64;
use std::rc::{Rc, Weak};

use wcontainers_gpu::{
    BindingEntry, BindingLayoutKey, BindingResource, BlendDesc, DepthStencilDesc, DispatchCall,
    DispatchGroups, DrawCall, DrawRange, FormatKind, FormatUse, GpuError, LoadOp, MAX_BINDING_SETS,
    MAX_COLOR_TARGETS, PassDesc, RasterDesc, SlowPath, StepMode, TargetLayout, VertexLayoutDesc,
};

use crate::{Buffer, Sampler, TextureView, WebGpu, convert, layout};

/// How many objects the caches made, which tests read to prove the caching.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    /// Render and compute pipelines built.
    pub pipelines: u64,
    /// `setPipeline` calls.
    pub pipeline_binds: u64,
    /// Bind groups made.
    pub bind_groups: u64,
    /// Bind group layouts made.
    pub group_layouts: u64,
    /// Buffers given fresh storage by a `Discard` write.
    pub renames: u64,
    /// Writes ordered through a staging buffer and a copy.
    pub staged_writes: u64,
}

/// An immutable block, made once per distinct value.
#[derive(Debug)]
pub struct Block<D> {
    pub id: u64,
    pub desc: D,
}

/// The blocks made so far, by value, so equal blocks are one block and one pipeline key.
#[derive(Default)]
pub struct BlockTables {
    pub blends: BlockTable<BlendDesc>,
    pub depth_stencils: BlockTable<DepthStencilDesc>,
    pub rasters: BlockTable<RasterDesc>,
    pub vertex_layouts: BlockTable<VertexLayoutDesc>,
}

pub struct BlockTable<D>(HashMap<D, Weak<Block<D>>>);

impl<D> Default for BlockTable<D> {
    fn default() -> Self {
        BlockTable(HashMap::new())
    }
}

impl<D: Hash + Eq + Copy> BlockTable<D> {
    /// The block equal to `desc`, made with `id` if none is alive; says whether it was made.
    pub fn get(&mut self, desc: &D, id: u64) -> (Rc<Block<D>>, bool) {
        if let Some(b) = self.0.get(desc).and_then(Weak::upgrade) {
            return (b, false);
        }
        let b = Rc::new(Block { id, desc: *desc });
        self.0.insert(*desc, Rc::downgrade(&b));
        (b, true)
    }
}

/// A binding set: its resources, its layout key, and the bind groups made from it per layout.
#[derive(Debug)]
pub struct SetObj {
    pub key: BindingLayoutKey,
    entries: Vec<(u32, SetRes)>,
    /// Bind groups by group layout id, with the buffer generations they were made at.
    groups: RefCell<Vec<(u64, u64, Rc<GroupObj>)>>,
}

#[derive(Debug)]
enum SetRes {
    Buffer {
        buffer: Buffer,
        offset: u64,
        size: u64,
    },
    View(TextureView),
    Sampler(Sampler),
}

impl SetObj {
    /// Changes whenever a buffer of the set is renamed.
    fn stamp(&self) -> u64 {
        self.entries
            .iter()
            .map(|(_, r)| match r {
                SetRes::Buffer { buffer, .. } => buffer.0.generation.get(),
                _ => 0,
            })
            .sum()
    }

    fn mark_used(&self, serial: u64) {
        for (_, r) in &self.entries {
            match r {
                SetRes::Buffer { buffer, .. } => buffer.0.used_in.set(serial),
                SetRes::View(v) => {
                    if let Some(t) = &v.0.texture {
                        t.0.used_in.set(serial);
                    }
                }
                SetRes::Sampler(_) => {}
            }
        }
    }
}

#[derive(Debug)]
pub struct GroupObj {
    id: u64,
    raw: wgpu::BindGroup,
}

pub struct GroupLayout {
    id: u64,
    raw: wgpu::BindGroupLayout,
    entries: Vec<wgpu::BindGroupLayoutEntry>,
}

/// A program's pipeline layout for one combination of layout keys.
pub struct ProgramLayout {
    raw: wgpu::PipelineLayout,
    groups: [Option<Rc<GroupLayout>>; MAX_BINDING_SETS],
}

/// Everything a render pipeline is built from (plan section 4.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    program: u64,
    blend: u64,
    depth_stencil: u64,
    raster: u64,
    vertex_layout: u64,
    targets: TargetLayout,
    /// The pass draws into the back end's own attachment (a pass without attachments).
    own_target: bool,
    topology: wgpu::PrimitiveTopology,
    strip_index: Option<wgpu::IndexFormat>,
    layouts: [BindingLayoutKey; MAX_BINDING_SETS],
}

type SetKey = Vec<(u32, u8, u64, u64, u64)>;

#[derive(Default)]
pub struct Caches {
    sets: HashMap<SetKey, Weak<SetObj>>,
    group_layouts: HashMap<Vec<wgpu::BindGroupLayoutEntry>, Rc<GroupLayout>>,
    layouts: HashMap<(u64, [BindingLayoutKey; MAX_BINDING_SETS]), Rc<ProgramLayout>>,
    pipelines: HashMap<PipelineKey, (u64, wgpu::RenderPipeline, Rc<ProgramLayout>)>,
    computes: HashMap<
        (u64, [BindingLayoutKey; MAX_BINDING_SETS]),
        (wgpu::ComputePipeline, Rc<ProgramLayout>),
    >,
    own_targets: HashMap<(u32, u32, u32), wgpu::TextureView>,
}

/// One color attachment as the pass opened it.
struct ColorRaw {
    view: wgpu::TextureView,
    resolve: Option<wgpu::TextureView>,
    slice: Option<u32>,
    clear: Option<wgpu::Color>,
}

/// The depth-stencil attachment: for each aspect it has and writes, its clear value if any.
struct DepthRaw {
    view: wgpu::TextureView,
    depth: Option<Option<f32>>,
    stencil: Option<Option<u32>>,
}

/// The open pass, and what it last set, so a draw sets only what changed.
pub struct Pass {
    pub rp: wgpu::RenderPass<'static>,
    pub occlusion: bool,
    pub query_running: bool,
    targets: TargetLayout,
    own_target: bool,
    width: u32,
    height: u32,
    colors: Vec<Option<ColorRaw>>,
    depth: Option<DepthRaw>,
    occlusion_set: Option<wgpu::QuerySet>,
    last: Last,
}

#[derive(Default)]
struct Last {
    key: Option<PipelineKey>,
    pipeline: u64,
    layout: Option<Rc<ProgramLayout>>,
    groups: [Option<(u64, Vec<u32>)>; MAX_BINDING_SETS],
    vertex: Vec<Option<(u64, u64, u64)>>,
    index: Option<(u64, u64, u64, wgpu::IndexFormat)>,
    viewport: Option<[u32; 6]>,
    scissor: Option<[u32; 4]>,
    blend_constant: Option<[u32; 4]>,
    stencil_reference: Option<u8>,
}

fn color_value(bits: [u32; 4], kind: FormatKind) -> wgpu::Color {
    let v: [f64; 4] = match kind {
        FormatKind::Uint => bits.map(f64::from),
        FormatKind::Sint => bits.map(|b| f64::from(b as i32)),
        _ => bits.map(|b| f64::from(f32::from_bits(b))),
    };
    wgpu::Color {
        r: v[0],
        g: v[1],
        b: v[2],
        a: v[3],
    }
}

/// Opens a render pass on `encoder`; `resume` loads every attachment instead of clearing.
fn open(
    encoder: &mut wgpu::CommandEncoder,
    colors: &[Option<ColorRaw>],
    depth: &Option<DepthRaw>,
    occlusion: Option<&wgpu::QuerySet>,
    resume: bool,
) -> wgpu::RenderPass<'static> {
    // Attachments always store: a pass may be split by an ordered write, and its first part must
    // keep what it drew.
    fn load<V>(clear: Option<V>, resume: bool) -> wgpu::LoadOp<V> {
        match clear {
            Some(v) if !resume => wgpu::LoadOp::Clear(v),
            _ => wgpu::LoadOp::Load,
        }
    }
    let attachments: Vec<_> = colors
        .iter()
        .map(|c| {
            c.as_ref().map(|c| wgpu::RenderPassColorAttachment {
                view: &c.view,
                depth_slice: c.slice,
                resolve_target: c.resolve.as_ref(),
                ops: wgpu::Operations {
                    load: load(c.clear, resume),
                    store: wgpu::StoreOp::Store,
                },
            })
        })
        .collect();
    let depth_stencil_attachment = depth
        .as_ref()
        .map(|d| wgpu::RenderPassDepthStencilAttachment {
            view: &d.view,
            depth_ops: d.depth.map(|clear| wgpu::Operations {
                load: load(clear, resume),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: d.stencil.map(|clear| wgpu::Operations {
                load: load(clear, resume),
                store: wgpu::StoreOp::Store,
            }),
        });
    encoder
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &attachments,
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: occlusion,
            multiview_mask: None,
        })
        .forget_lifetime()
}

impl WebGpu {
    pub(crate) fn binding_set(
        &mut self,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<crate::BindingSet, GpuError> {
        let mut key: SetKey = Vec::with_capacity(entries.len());
        let mut layout_key = BindingLayoutKey::default();
        let mut res = Vec::with_capacity(entries.len());
        for e in entries {
            let r = match &e.resource {
                BindingResource::UniformBuffer(r)
                | BindingResource::StorageBuffer { range: r, .. } => SetRes::Buffer {
                    buffer: r.buffer.clone(),
                    offset: r.offset,
                    size: r.size,
                },
                BindingResource::Texture(v) => {
                    let filterable = self
                        .caps
                        .formats
                        .get(v.0.format)
                        .contains(FormatUse::FILTER);
                    if let Some(bit) = e.binding.checked_sub(32).filter(|b| *b < 128)
                        && !filterable
                    {
                        layout_key.unfilterable_textures |= 1u128 << bit;
                    }
                    SetRes::View((*v).clone())
                }
                BindingResource::StorageTexture { view, .. } => SetRes::View((*view).clone()),
                BindingResource::Sampler(s) => SetRes::Sampler((*s).clone()),
                BindingResource::TexelBuffer { .. } => {
                    return Err(GpuError::Invalid(
                        "typed buffers are read as storage on WebGPU (TexelBufferAsStorage)",
                    ));
                }
                BindingResource::Table(t) => match **t {},
            };
            let (kind, id, offset, size) = match &r {
                SetRes::Buffer {
                    buffer,
                    offset,
                    size,
                } => (0, buffer.0.id, *offset, *size),
                SetRes::View(v) => (1, v.0.id, 0, 0),
                SetRes::Sampler(s) => (2, s.0.id, 0, 0),
            };
            key.push((e.binding, kind, id, offset, size));
            res.push((e.binding, r));
        }
        key.sort_unstable();
        if let Some(set) = self.caches.sets.get(&key).and_then(Weak::upgrade) {
            return Ok(crate::BindingSet(set));
        }
        let set = Rc::new(SetObj {
            key: layout_key,
            entries: res,
            groups: RefCell::new(Vec::new()),
        });
        self.caches.sets.insert(key, Rc::downgrade(&set));
        Ok(crate::BindingSet(set))
    }

    fn program_layout(
        &mut self,
        program: &crate::ProgramObj,
        keys: [BindingLayoutKey; MAX_BINDING_SETS],
    ) -> Result<Rc<ProgramLayout>, &'static str> {
        if let Some(l) = self.caches.layouts.get(&(program.id(), keys)) {
            return Ok(l.clone());
        }
        let max_dynamic = self
            .device
            .limits()
            .max_dynamic_uniform_buffers_per_pipeline_layout;
        let groups = layout::derive(&program.stages(), &keys, max_dynamic)?;
        let used = groups
            .iter()
            .rposition(|g| !g.is_empty())
            .map_or(0, |i| i + 1);
        let mut out: [Option<Rc<GroupLayout>>; MAX_BINDING_SETS] = Default::default();
        for (i, entries) in groups.into_iter().enumerate().take(used) {
            if entries.is_empty() {
                continue;
            }
            let gl = match self.caches.group_layouts.get(&entries) {
                Some(gl) => gl.clone(),
                None => {
                    let raw =
                        self.device
                            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                                label: None,
                                entries: &entries,
                            });
                    self.counters.group_layouts += 1;
                    let gl = Rc::new(GroupLayout {
                        id: self.id(),
                        raw,
                        entries: entries.clone(),
                    });
                    self.caches.group_layouts.insert(entries, gl.clone());
                    gl
                }
            };
            out[i] = Some(gl);
        }
        let refs: Vec<Option<&wgpu::BindGroupLayout>> = out[..used]
            .iter()
            .map(|g| g.as_ref().map(|g| &g.raw))
            .collect();
        let raw = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &refs,
                immediate_size: 0,
            });
        let l = Rc::new(ProgramLayout { raw, groups: out });
        self.caches.layouts.insert((program.id(), keys), l.clone());
        Ok(l)
    }

    /// The bind group of `set` for `gl`, made once per layout and buffer generations.
    fn bind_group(&mut self, set: &SetObj, gl: &GroupLayout) -> Result<Rc<GroupObj>, &'static str> {
        let stamp = set.stamp();
        if let Some((_, _, g)) = set
            .groups
            .borrow()
            .iter()
            .find(|(id, s, _)| *id == gl.id && *s == stamp)
        {
            return Ok(g.clone());
        }
        let buffers: Vec<Option<wgpu::Buffer>> = set
            .entries
            .iter()
            .map(|(_, r)| match r {
                SetRes::Buffer { buffer, .. } => Some(buffer.0.raw.borrow().clone()),
                _ => None,
            })
            .collect();
        let mut entries = Vec::with_capacity(gl.entries.len());
        for le in &gl.entries {
            let i = set
                .entries
                .iter()
                .position(|(b, _)| *b == le.binding)
                .ok_or("a binding set lacks a binding its program reads")?;
            let resource = match (&set.entries[i].1, &le.ty) {
                (
                    SetRes::Buffer { offset, size, .. },
                    wgpu::BindingType::Buffer {
                        has_dynamic_offset, ..
                    },
                ) => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: buffers[i].as_ref().ok_or("buffer")?,
                    offset: if *has_dynamic_offset { 0 } else { *offset },
                    size: NonZeroU64::new(*size),
                }),
                (
                    SetRes::View(v),
                    wgpu::BindingType::Texture { .. } | wgpu::BindingType::StorageTexture { .. },
                ) => wgpu::BindingResource::TextureView(&v.0.raw),
                (SetRes::Sampler(s), wgpu::BindingType::Sampler(_)) => {
                    wgpu::BindingResource::Sampler(&s.0.raw)
                }
                _ => return Err("a binding set's resource is not the kind its program reads"),
            };
            entries.push(wgpu::BindGroupEntry {
                binding: le.binding,
                resource,
            });
        }
        let raw = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &gl.raw,
            entries: &entries,
        });
        self.counters.bind_groups += 1;
        let g = Rc::new(GroupObj { id: self.id(), raw });
        let mut groups = set.groups.borrow_mut();
        groups.retain(|(id, _, _)| *id != gl.id);
        groups.push((gl.id, stamp, g.clone()));
        Ok(g)
    }

    /// The dynamic offsets `set` gives in layout `gl`, in binding order.
    fn dynamic_offsets(set: &SetObj, gl: &GroupLayout) -> Vec<u32> {
        gl.entries
            .iter()
            .filter(|e| {
                matches!(
                    e.ty,
                    wgpu::BindingType::Buffer {
                        has_dynamic_offset: true,
                        ..
                    }
                )
            })
            .filter_map(|e| set.entries.iter().find(|(b, _)| *b == e.binding))
            .map(|(_, r)| match r {
                SetRes::Buffer { offset, .. } => *offset as u32,
                _ => 0,
            })
            .collect()
    }

    fn render_pipeline(
        &mut self,
        key: &PipelineKey,
        call: &DrawCall<'_, Self>,
    ) -> Result<(u64, wgpu::RenderPipeline, Rc<ProgramLayout>), &'static str> {
        let crate::ProgramObj::Render {
            vertex, fragment, ..
        } = &*call.program.0
        else {
            return Err("draw with a compute program");
        };
        let layout = self.program_layout(&call.program.0, key.layouts)?;
        let vl = &call.vertex_layout.0.desc;
        let attributes: Vec<Vec<wgpu::VertexAttribute>> = (0..vl.streams.len())
            .map(|s| {
                vl.attributes
                    .iter()
                    .flatten()
                    .filter(|a| usize::from(a.stream) == s)
                    .filter_map(|a| {
                        Some(wgpu::VertexAttribute {
                            format: convert::vertex_format(a.format)?,
                            offset: u64::from(a.offset),
                            shader_location: a.location,
                        })
                    })
                    .collect()
            })
            .collect();
        let streams = vl
            .streams
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        let buffers: Vec<Option<wgpu::VertexBufferLayout>> = vl.streams[..streams]
            .iter()
            .zip(&attributes)
            .map(|(s, attrs)| {
                s.map(|s| wgpu::VertexBufferLayout {
                    array_stride: u64::from(s.stride),
                    step_mode: match s.step {
                        StepMode::Vertex => wgpu::VertexStepMode::Vertex,
                        StepMode::Instance(_) => wgpu::VertexStepMode::Instance,
                    },
                    attributes: attrs,
                })
            })
            .collect();
        let blend = &call.blend.0.desc;
        let mut targets: Vec<Option<wgpu::ColorTargetState>> = Vec::new();
        if key.own_target {
            targets.push(Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::R8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::empty(),
            }));
        } else {
            let used = key
                .targets
                .colors
                .iter()
                .rposition(Option::is_some)
                .map_or(0, |i| i + 1);
            for (i, format) in key.targets.colors[..used].iter().enumerate() {
                targets.push(format.map(|f| {
                    let t = blend.targets[if blend.same_for_all { 0 } else { i }];
                    let blendable = self.caps.formats.get(f).contains(FormatUse::BLEND);
                    wgpu::ColorTargetState {
                        format: convert::texture_format(f).unwrap_or(wgpu::TextureFormat::R8Unorm),
                        blend: t
                            .blend
                            .filter(|_| blendable)
                            .map(|(c, a)| wgpu::BlendState {
                                color: blend_component(c),
                                alpha: blend_component(a),
                            }),
                        write_mask: convert::color_writes(t.writes),
                    }
                }));
            }
        }
        let fragment = match fragment {
            Some(f) => Some((f, targets)),
            None if targets.iter().all(Option::is_none) => None,
            None => {
                return Err(
                    "a program without a fragment shader drawing into color targets is not built yet",
                );
            }
        };
        let raster = &call.raster.0.desc;
        let ds = &call.depth_stencil.0.desc;
        let triangles = matches!(
            key.topology,
            wgpu::PrimitiveTopology::TriangleList | wgpu::PrimitiveTopology::TriangleStrip
        );
        let depth_stencil = key.targets.depth_stencil.map(|f| {
            let has_depth = matches!(f.kind(), FormatKind::Depth | FormatKind::DepthStencil);
            let stencil = ds.stencil.filter(|_| crate::has_stencil(f));
            wgpu::DepthStencilState {
                format: convert::texture_format(f).unwrap_or(wgpu::TextureFormat::Depth32Float),
                depth_write_enabled: Some(
                    has_depth && ds.depth_compare.is_some() && ds.depth_write,
                ),
                depth_compare: ds
                    .depth_compare
                    .filter(|_| has_depth)
                    .map_or(Some(wgpu::CompareFunction::Always), |c| {
                        Some(convert::compare(c))
                    }),
                stencil: match stencil {
                    Some((front, back)) => wgpu::StencilState {
                        front: convert::stencil_face(&front),
                        back: convert::stencil_face(&back),
                        read_mask: u32::from(ds.stencil_read_mask[0]),
                        write_mask: u32::from(ds.stencil_write_mask[0]),
                    },
                    None => wgpu::StencilState::default(),
                },
                bias: if triangles && has_depth {
                    wgpu::DepthBiasState {
                        constant: f32::from_bits(raster.depth_bias) as i32,
                        slope_scale: f32::from_bits(raster.depth_bias_slope),
                        clamp: f32::from_bits(raster.depth_bias_clamp),
                    }
                } else {
                    wgpu::DepthBiasState::default()
                },
            }
        });
        let raw = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&layout.raw),
                vertex: wgpu::VertexState {
                    module: &vertex.0.raw,
                    entry_point: Some(&vertex.0.entry),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: key.topology,
                    strip_index_format: key.strip_index,
                    front_face: convert::front_face(raster.front),
                    cull_mode: convert::cull(raster.cull),
                    unclipped_depth: !raster.depth_clip,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil,
                multisample: wgpu::MultisampleState {
                    count: key.targets.samples,
                    mask: u64::from(raster.sample_mask),
                    alpha_to_coverage_enabled: blend.alpha_to_coverage,
                },
                fragment: fragment.as_ref().map(|(f, targets)| wgpu::FragmentState {
                    module: &f.0.raw,
                    entry_point: Some(&f.0.entry),
                    compilation_options: Default::default(),
                    targets,
                }),
                multiview_mask: None,
                cache: None,
            });
        self.counters.pipelines += 1;
        Ok((self.id(), raw, layout))
    }

    pub(crate) fn begin_pass_impl(&mut self, pass: &PassDesc<'_, Self>) {
        if self.pass.is_some() {
            self.fail("begin_pass inside a pass");
            return;
        }
        let serial = self.serial;
        let mut targets = TargetLayout {
            colors: [None; MAX_COLOR_TARGETS],
            depth_stencil: None,
            samples: 1,
        };
        let (mut width, mut height) = (0, 0);
        let mut colors = Vec::new();
        for (i, c) in pass.colors.iter().enumerate() {
            let Some(c) = c else {
                colors.push(None);
                continue;
            };
            let v = &c.view.0;
            if let Some(t) = &v.texture {
                t.0.used_in.set(serial);
            }
            targets.colors[i] = Some(v.format);
            targets.samples = v.samples;
            (width, height) = (v.width, v.height);
            colors.push(Some(ColorRaw {
                view: v.raw.clone(),
                resolve: c.resolve.map(|r| r.0.raw.clone()),
                slice: v.three_d.then_some(c.depth_slice),
                clear: match c.load {
                    LoadOp::Load => None,
                    LoadOp::Clear(bits) => Some(color_value(bits, v.format.kind())),
                    LoadOp::DontCare => Some(wgpu::Color::TRANSPARENT),
                },
            }));
        }
        let used = colors
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        colors.truncate(used);
        let depth = pass.depth.as_ref().map(|d| {
            let v = &d.view.0;
            if let Some(t) = &v.texture {
                t.0.used_in.set(serial);
            }
            targets.depth_stencil = Some(v.format);
            targets.samples = v.samples;
            (width, height) = (v.width, v.height);
            fn aspect<V: Copy, W>(
                ops: Option<wcontainers_gpu::AspectOps<V>>,
                f: impl Fn(V) -> W,
            ) -> Option<Option<W>> {
                ops.filter(|o| !o.read_only).map(|o| match o.load {
                    LoadOp::Load => None,
                    LoadOp::Clear(v) => Some(f(v)),
                    LoadOp::DontCare => None,
                })
            }
            DepthRaw {
                view: v.raw.clone(),
                depth: aspect(d.depth, f32::from_bits),
                stencil: aspect(d.stencil, u32::from),
            }
        });
        let own_target = colors.is_empty() && depth.is_none();
        if own_target {
            let Some(e) = pass.empty else {
                self.fail("a pass with no attachment and no empty size");
                return;
            };
            targets.colors[0] = Some(wcontainers_gpu::Format::R8Unorm);
            targets.samples = e.samples;
            (width, height) = (e.width, e.height);
            let device = &self.device;
            let view = self
                .caches
                .own_targets
                .entry((e.width, e.height, e.samples))
                .or_insert_with(|| {
                    device
                        .create_texture(&wgpu::TextureDescriptor {
                            label: None,
                            size: wgpu::Extent3d {
                                width: e.width,
                                height: e.height,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: e.samples,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::R8Unorm,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                            view_formats: &[],
                        })
                        .create_view(&Default::default())
                })
                .clone();
            colors.push(Some(ColorRaw {
                view,
                resolve: None,
                slice: None,
                clear: Some(wgpu::Color::TRANSPARENT),
            }));
        }
        let occlusion_set = pass.occlusion.map(|q| q.0.raw.clone());
        let rp = open(
            self.encoder(),
            &colors,
            &depth,
            occlusion_set.as_ref(),
            false,
        );
        self.pass = Some(Pass {
            rp,
            occlusion: occlusion_set.is_some(),
            query_running: false,
            targets,
            own_target,
            width,
            height,
            colors,
            depth,
            occlusion_set,
            last: Last::default(),
        });
    }

    /// Ends the open pass, records `between` on the encoder, and opens the pass again loading what
    /// it drew (slow path `WriteSplitPass`).
    pub(crate) fn split_pass(&mut self, between: impl FnOnce(&mut wgpu::CommandEncoder)) {
        let Some(mut pass) = self.pass.take() else {
            between(self.encoder());
            return;
        };
        if pass.query_running {
            self.fail("an ordered write splitting a pass with a running query is not built yet");
            return;
        }
        self.slow.hit(SlowPath::WriteSplitPass);
        drop(pass.rp);
        let encoder = self.encoder.as_mut().expect("a pass implies an encoder");
        between(encoder);
        pass.rp = open(
            encoder,
            &pass.colors,
            &pass.depth,
            pass.occlusion_set.as_ref(),
            true,
        );
        pass.last = Last::default();
        self.pass = Some(pass);
    }

    pub(crate) fn draw_impl(&mut self, call: &DrawCall<'_, Self>) {
        let Some(mut pass) = self.pass.take() else {
            self.fail("draw outside a pass");
            return;
        };
        if let Err(why) = self.draw_in(&mut pass, call) {
            self.fail(why);
        }
        self.pass = Some(pass);
    }

    fn draw_in(&mut self, pass: &mut Pass, call: &DrawCall<'_, Self>) -> Result<(), &'static str> {
        let topology =
            convert::topology(call.topology).ok_or("fans and loops go through draw_fan_or_loop")?;
        if !call.immediates.is_empty() {
            return Err("immediates are not built yet (limit 0)");
        }
        let strip = matches!(
            topology,
            wgpu::PrimitiveTopology::LineStrip | wgpu::PrimitiveTopology::TriangleStrip
        );
        let index_format = match &call.index {
            Some(i) => Some(
                convert::index_format(i.format).ok_or("8-bit indices are widened by the front")?,
            ),
            None => None,
        };
        let mut layouts = [BindingLayoutKey::default(); MAX_BINDING_SETS];
        for (k, s) in layouts.iter_mut().zip(&call.bindings) {
            if let Some(s) = s {
                *k = s.0.key;
            }
        }
        let key = PipelineKey {
            program: call.program.0.id(),
            blend: call.blend.0.id,
            depth_stencil: call.depth_stencil.0.id,
            raster: call.raster.0.id,
            vertex_layout: call.vertex_layout.0.id,
            targets: pass.targets,
            own_target: pass.own_target,
            topology,
            strip_index: index_format.filter(|_| strip),
            layouts,
        };
        let rp = &mut pass.rp;
        let last = &mut pass.last;
        if last.key != Some(key) {
            let (id, pipeline, layout) = match self.caches.pipelines.get(&key) {
                Some(p) => p.clone(),
                None => {
                    self.slow.hit(SlowPath::PipelineMiss);
                    let p = self.render_pipeline(&key, call)?;
                    self.caches.pipelines.insert(key, p.clone());
                    p
                }
            };
            if last.pipeline != id {
                rp.set_pipeline(&pipeline);
                self.counters.pipeline_binds += 1;
                last.pipeline = id;
            }
            last.key = Some(key);
            last.layout = Some(layout);
        }
        let layout = last.layout.clone().expect("set with the pipeline");
        for (i, gl) in layout.groups.iter().enumerate() {
            let Some(gl) = gl else { continue };
            let set = call.bindings[i].ok_or("a draw lacks a binding set its program reads")?;
            let group = self.bind_group(&set.0, gl)?;
            let offsets = Self::dynamic_offsets(&set.0, gl);
            if last.groups[i].as_ref() != Some(&(group.id, offsets.clone())) {
                rp.set_bind_group(i as u32, &group.raw, &offsets);
                set.0.mark_used(self.serial);
                last.groups[i] = Some((group.id, offsets));
            }
        }
        if last.vertex.len() < call.vertex_buffers.len() {
            last.vertex.resize(call.vertex_buffers.len(), None);
        }
        for (slot, vb) in call.vertex_buffers.iter().enumerate() {
            let Some(vb) = vb else { continue };
            let b = &vb.buffer.0;
            let now = Some((b.id, b.generation.get(), vb.offset));
            if last.vertex[slot] != now {
                rp.set_vertex_buffer(slot as u32, b.raw.borrow().slice(vb.offset..));
                b.used_in.set(self.serial);
                last.vertex[slot] = now;
            }
        }
        if let (Some(i), Some(format)) = (&call.index, index_format) {
            let b = &i.buffer.0;
            let now = Some((b.id, b.generation.get(), i.offset, format));
            if last.index != now {
                rp.set_index_buffer(b.raw.borrow().slice(i.offset..), format);
                b.used_in.set(self.serial);
                last.index = now;
            }
        }
        let viewport = call.viewports.first().map_or(
            [
                0,
                0,
                (pass.width as f32).to_bits(),
                (pass.height as f32).to_bits(),
                0,
                1.0f32.to_bits(),
            ],
            |v| [v.x, v.y, v.width, v.height, v.min_depth, v.max_depth],
        );
        if last.viewport != Some(viewport) {
            let [x, y, w, h, near, far] = viewport.map(f32::from_bits);
            rp.set_viewport(x, y, w, h, near, far);
            last.viewport = Some(viewport);
        }
        let (pw, ph) = (pass.width, pass.height);
        let scissor = match call.scissors.first() {
            Some(s) if call.raster.0.desc.scissor => {
                let x = s.x.min(pw);
                let y = s.y.min(ph);
                [x, y, s.width.min(pw - x), s.height.min(ph - y)]
            }
            _ => [0, 0, pw, ph],
        };
        if last.scissor != Some(scissor) {
            rp.set_scissor_rect(scissor[0], scissor[1], scissor[2], scissor[3]);
            last.scissor = Some(scissor);
        }
        if last.blend_constant != Some(call.blend_constant) {
            let [r, g, b, a] = call.blend_constant.map(|c| f64::from(f32::from_bits(c)));
            rp.set_blend_constant(wgpu::Color { r, g, b, a });
            last.blend_constant = Some(call.blend_constant);
        }
        if last.stencil_reference != Some(call.stencil_reference[0]) {
            rp.set_stencil_reference(u32::from(call.stencil_reference[0]));
            last.stencil_reference = Some(call.stencil_reference[0]);
        }
        match &call.range {
            DrawRange::Vertices {
                first,
                count,
                first_instance,
                instances,
            } => rp.draw(
                *first..first + count,
                *first_instance..first_instance + instances,
            ),
            DrawRange::Indexed {
                first,
                count,
                base_vertex,
                first_instance,
                instances,
            } => rp.draw_indexed(
                *first..first + count,
                *base_vertex,
                *first_instance..first_instance + instances,
            ),
            DrawRange::Indirect { args, offset } => {
                args.0.used_in.set(self.serial);
                rp.draw_indirect(&args.0.raw.borrow(), *offset);
            }
            DrawRange::IndexedIndirect { args, offset } => {
                args.0.used_in.set(self.serial);
                rp.draw_indexed_indirect(&args.0.raw.borrow(), *offset);
            }
        }
        Ok(())
    }

    pub(crate) fn dispatch_impl(&mut self, call: &DispatchCall<'_, Self>) -> Result<(), GpuError> {
        if self.pass.is_some() {
            return Err(GpuError::Invalid("dispatch inside a pass"));
        }
        if !call.immediates.is_empty() {
            return Err(GpuError::Invalid("immediates are not built yet (limit 0)"));
        }
        let crate::ProgramObj::Compute { id, shader } = &*call.program.0 else {
            return Err(GpuError::Invalid("dispatch with a render program"));
        };
        let mut keys = [BindingLayoutKey::default(); MAX_BINDING_SETS];
        for (k, s) in keys.iter_mut().zip(&call.bindings) {
            if let Some(s) = s {
                *k = s.0.key;
            }
        }
        let (pipeline, layout) = match self.caches.computes.get(&(*id, keys)) {
            Some(p) => p.clone(),
            None => {
                self.slow.hit(SlowPath::PipelineMiss);
                let layout = self
                    .program_layout(&call.program.0, keys)
                    .map_err(GpuError::Invalid)?;
                let raw = self
                    .device
                    .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: None,
                        layout: Some(&layout.raw),
                        module: &shader.0.raw,
                        entry_point: Some(&shader.0.entry),
                        compilation_options: Default::default(),
                        cache: None,
                    });
                self.counters.pipelines += 1;
                self.caches
                    .computes
                    .insert((*id, keys), (raw.clone(), layout.clone()));
                (raw, layout)
            }
        };
        let mut groups = Vec::new();
        for (i, gl) in layout.groups.iter().enumerate() {
            let Some(gl) = gl else { continue };
            let set = call.bindings[i].ok_or(GpuError::Invalid(
                "a dispatch lacks a binding set its program reads",
            ))?;
            let group = self.bind_group(&set.0, gl).map_err(GpuError::Invalid)?;
            set.0.mark_used(self.serial);
            groups.push((i as u32, group, Self::dynamic_offsets(&set.0, gl)));
        }
        let indirect = match &call.groups {
            DispatchGroups::Indirect { args, offset } => {
                args.0.used_in.set(self.serial);
                Some((args.0.raw.borrow().clone(), *offset))
            }
            DispatchGroups::Direct(_) => None,
        };
        let mut cp = self
            .encoder()
            .begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        cp.set_pipeline(&pipeline);
        for (i, g, offsets) in &groups {
            cp.set_bind_group(*i, &g.raw, offsets);
        }
        match (&call.groups, indirect) {
            (_, Some((args, offset))) => cp.dispatch_workgroups_indirect(&args, offset),
            (DispatchGroups::Direct([x, y, z]), None) => cp.dispatch_workgroups(*x, *y, *z),
            (DispatchGroups::Indirect { .. }, None) => {
                unreachable!("indirect arguments taken above")
            }
        }
        Ok(())
    }

    pub(crate) fn resolve_impl(
        &mut self,
        src: &wcontainers_gpu::TextureRegion<'_, Self>,
        dst: &wcontainers_gpu::TextureRegion<'_, Self>,
    ) {
        if self.pass.is_some() {
            self.fail("resolve inside a pass");
            return;
        }
        let view = |r: &wcontainers_gpu::TextureRegion<'_, Self>| {
            r.texture.0.used_in.set(self.serial);
            r.texture.0.raw.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_mip_level: r.mip,
                mip_level_count: Some(1),
                base_array_layer: r.layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        };
        let colors = [Some(ColorRaw {
            view: view(src),
            resolve: Some(view(dst)),
            slice: None,
            clear: None,
        })];
        drop(open(self.encoder(), &colors, &None, None, false));
    }
}

fn blend_component(c: wcontainers_gpu::BlendComponent) -> wgpu::BlendComponent {
    // WebGPU requires factors of one with min and max, which ignore them in every API.
    let minmax = matches!(
        c.op,
        wcontainers_gpu::BlendOp::Min | wcontainers_gpu::BlendOp::Max
    );
    let factor = |f| {
        if minmax {
            wgpu::BlendFactor::One
        } else {
            convert::blend_factor(f).unwrap_or(wgpu::BlendFactor::One)
        }
    };
    wgpu::BlendComponent {
        src_factor: factor(c.src),
        dst_factor: factor(c.dst),
        operation: convert::blend_op(c.op),
    }
}
