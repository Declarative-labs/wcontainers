//! Blocks, binding sets, the pipeline and bind group caches, passes and draws (plan sections 4.4, 4.5
//! and 4.7).

use std::cell::{Cell, RefCell};
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
    /// Uniform arena buffers moved to a new slice by a `Discard` write.
    pub slices: u64,
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

/// The most dynamic offsets one bind group takes: the uniform buffer slots of one set (plan 6.4).
pub const MAX_GROUP_DYNAMIC: usize = 16;

/// A binding set: its resources, its layout key, and its bind group for each group layout.
#[derive(Debug)]
pub struct SetObj {
    pub key: BindingLayoutKey,
    entries: Vec<(u32, SetRes)>,
    groups: RefCell<Vec<Rc<SetGroup>>>,
}

/// A set's bind group for one group layout, and where the set's dynamic offsets come from.
#[derive(Debug)]
pub struct SetGroup {
    layout: u64,
    /// The bind epoch at which `group` was last known to be the set's.
    epoch: Cell<u64>,
    group: Rc<GroupObj>,
    /// The set's entries giving the layout's dynamic offsets, in the layout's order.
    dynamic: Box<[usize]>,
}

/// What a bind group is made of: its layout, and per entry the binding, the resource (a buffer's
/// storage number), the fixed offset (zero for a dynamic one) and the size. Sets differing only in
/// dynamic offsets, or naming different buffers of the uniform arena, share one bind group.
type GroupKey = (u64, Vec<(u32, u64, u64, u64)>);

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
    /// The key of the bind group this set makes with `gl`, and the entries giving its dynamic
    /// offsets.
    fn group_key(&self, gl: &GroupLayout) -> Result<(GroupKey, Vec<usize>), &'static str> {
        let mut key = Vec::with_capacity(gl.entries.len());
        let mut dynamic = Vec::new();
        for le in &gl.entries {
            let i = self
                .entries
                .iter()
                .position(|(b, _)| *b == le.binding)
                .ok_or("a binding set lacks a binding its program reads")?;
            key.push(match (&self.entries[i].1, &le.ty) {
                (
                    SetRes::Buffer {
                        buffer,
                        offset,
                        size,
                    },
                    wgpu::BindingType::Buffer {
                        has_dynamic_offset, ..
                    },
                ) => {
                    let b = &buffer.0;
                    if *has_dynamic_offset {
                        dynamic.push(i);
                        (le.binding, b.storage.get(), 0, *size)
                    } else {
                        (le.binding, b.storage.get(), b.base.get() + offset, *size)
                    }
                }
                (
                    SetRes::View(v),
                    wgpu::BindingType::Texture { .. } | wgpu::BindingType::StorageTexture { .. },
                ) => (le.binding, v.0.id, 0, 0),
                (SetRes::Sampler(s), wgpu::BindingType::Sampler(_)) => (le.binding, s.0.id, 0, 0),
                _ => return Err("a binding set's resource is not the kind its program reads"),
            });
        }
        Ok(((gl.id, key), dynamic))
    }

    /// Fills `out` with the dynamic offsets of `group`; says how many.
    fn offsets(&self, group: &SetGroup, out: &mut [u32; MAX_GROUP_DYNAMIC]) -> usize {
        for (o, &i) in out.iter_mut().zip(&group.dynamic) {
            if let SetRes::Buffer { buffer, offset, .. } = &self.entries[i].1 {
                *o = (buffer.0.base.get() + offset) as u32;
            }
        }
        group.dynamic.len()
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
    key: GroupKey,
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
    /// How many binding sets were alive at the last pruning.
    sets_alive: usize,
    /// Bind groups by what they are made of.
    groups: HashMap<GroupKey, Weak<GroupObj>>,
    /// How many bind groups were alive at the last pruning.
    groups_alive: usize,
    group_layouts: HashMap<Vec<wgpu::BindGroupLayoutEntry>, Rc<GroupLayout>>,
    layouts: HashMap<(u64, [BindingLayoutKey; MAX_BINDING_SETS]), Rc<ProgramLayout>>,
    pipelines: HashMap<PipelineKey, (u64, wgpu::RenderPipeline, Rc<ProgramLayout>)>,
    computes: HashMap<
        (u64, [BindingLayoutKey; MAX_BINDING_SETS]),
        (wgpu::ComputePipeline, Rc<ProgramLayout>),
    >,
    own_targets: HashMap<(u32, u32, u32), wgpu::TextureView>,
}

impl Caches {
    /// Forgets binding sets and bind groups nothing holds any more.
    pub fn prune(&mut self) {
        prune(&mut self.sets, &mut self.sets_alive);
        prune(&mut self.groups, &mut self.groups_alive);
    }
}

/// Forgets entries nothing holds, once the table has doubled since it was last pruned, so each
/// insert costs constant time on average.
fn prune<K, V>(table: &mut HashMap<K, Weak<V>>, alive: &mut usize) {
    if table.len() > 2 * (*alive).max(64) {
        table.retain(|_, v| v.strong_count() > 0);
        *alive = table.len();
    }
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
    groups: [Option<LastGroup>; MAX_BINDING_SETS],
    vertex: Vec<Option<(u64, u64, u64)>>,
    index: Option<(u64, u64, u64, wgpu::IndexFormat)>,
    viewport: Option<[u32; 6]>,
    scissor: Option<[u32; 4]>,
    blend_constant: Option<[u32; 4]>,
    stencil_reference: Option<u8>,
}

/// The bind group a group index holds, and the set and offsets it was bound for.
struct LastGroup {
    set: Rc<SetObj>,
    group: Rc<SetGroup>,
    offsets: [u32; MAX_GROUP_DYNAMIC],
    count: usize,
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
                | BindingResource::StorageBuffer { range: r, .. } => {
                    r.buffer.0.in_sets.set(true);
                    SetRes::Buffer {
                        buffer: r.buffer.clone(),
                        offset: r.offset,
                        size: r.size,
                    }
                }
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
        let groups = layout::derive(&program.stages().collect::<Vec<_>>(), &keys, max_dynamic)?;
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

    /// The bind group of `set` for `gl`: checked again only when the bind epoch moved, and shared
    /// between sets made of the same things.
    fn set_group(&mut self, set: &SetObj, gl: &GroupLayout) -> Result<Rc<SetGroup>, &'static str> {
        let found = set
            .groups
            .borrow()
            .iter()
            .find(|g| g.layout == gl.id)
            .cloned();
        if let Some(g) = &found
            && g.epoch.get() == self.bind_epoch
        {
            return Ok(g.clone());
        }
        let (key, dynamic) = set.group_key(gl)?;
        if let Some(g) = found
            && g.group.key == key
        {
            g.epoch.set(self.bind_epoch);
            return Ok(g);
        }
        let group = match self.caches.groups.get(&key).and_then(Weak::upgrade) {
            Some(g) => g,
            None => self.make_group(set, gl, key)?,
        };
        let g = Rc::new(SetGroup {
            layout: gl.id,
            epoch: Cell::new(self.bind_epoch),
            group,
            dynamic: dynamic.into(),
        });
        let mut groups = set.groups.borrow_mut();
        groups.retain(|g| g.layout != gl.id);
        groups.push(g.clone());
        Ok(g)
    }

    fn make_group(
        &mut self,
        set: &SetObj,
        gl: &GroupLayout,
        key: GroupKey,
    ) -> Result<Rc<GroupObj>, &'static str> {
        let buffers: Vec<Option<wgpu::Buffer>> = set
            .entries
            .iter()
            .map(|(_, r)| match r {
                SetRes::Buffer { buffer, .. } => Some(buffer.0.raw.borrow().clone()),
                _ => None,
            })
            .collect();
        let mut entries = Vec::with_capacity(gl.entries.len());
        for (le, &(_, _, offset, size)) in gl.entries.iter().zip(&key.1) {
            let i = set
                .entries
                .iter()
                .position(|(b, _)| *b == le.binding)
                .ok_or("a binding set lacks a binding its program reads")?;
            let resource = match &set.entries[i].1 {
                SetRes::Buffer { buffer, .. } => {
                    if !matches!(
                        le.ty,
                        wgpu::BindingType::Buffer {
                            has_dynamic_offset: true,
                            ..
                        }
                    ) {
                        buffer.0.static_bound.set(true);
                    }
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: buffers[i].as_ref().ok_or("buffer")?,
                        offset,
                        size: NonZeroU64::new(size),
                    })
                }
                SetRes::View(v) => wgpu::BindingResource::TextureView(&v.0.read),
                SetRes::Sampler(s) => wgpu::BindingResource::Sampler(&s.0.raw),
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
        let g = Rc::new(GroupObj {
            id: self.id(),
            raw,
            key: key.clone(),
        });
        self.caches.groups.insert(key, Rc::downgrade(&g));
        Ok(g)
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
                let Some(f) = *format else {
                    targets.push(None);
                    continue;
                };
                let t = blend.targets[if blend.same_for_all { 0 } else { i }];
                // Integer targets ignore blending, as in every API; any other format this device
                // cannot blend is the front's to blend in the program (BlendInShader).
                let blend = match t.blend {
                    Some(_) if matches!(f.kind(), FormatKind::Uint | FormatKind::Sint) => None,
                    Some(_) if !self.caps.formats.get(f).contains(FormatUse::BLEND) => {
                        return Err(
                            "blending into a format this device cannot blend; fronts blend in the program (BlendInShader)",
                        );
                    }
                    b => b.map(|(c, a)| wgpu::BlendState {
                        color: blend_component(c),
                        alpha: blend_component(a),
                    }),
                };
                targets.push(Some(wgpu::ColorTargetState {
                    format: convert::texture_format(f).unwrap_or(wgpu::TextureFormat::R8Unorm),
                    blend,
                    write_mask: convert::color_writes(t.writes),
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
            for t in std::iter::once(&**v)
                .chain(c.resolve.map(|r| &*r.0))
                .filter_map(|v| v.texture.as_ref())
            {
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
                    LoadOp::Clear(bits) => {
                        let kind = v.format.kind();
                        let exact = |b: u32| match kind {
                            FormatKind::Uint => b <= 1 << 24,
                            FormatKind::Sint => (b as i32).unsigned_abs() <= 1 << 24,
                            _ => true,
                        };
                        if self.float_clears && !bits.iter().all(|b| exact(*b)) {
                            self.fail(
                                "an integer clear past 2^24, which wgpu's Direct3D 12 rounds through a float, is not built yet",
                            );
                        }
                        Some(color_value(bits, kind))
                    }
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

    /// Ends the open pass, records `between` outside it, and opens the pass again loading what it
    /// drew (slow path `WriteSplitPass`).
    pub(crate) fn split_pass(&mut self, between: impl FnOnce(&mut WebGpu)) {
        let Some(mut pass) = self.pass.take() else {
            between(self);
            return;
        };
        if pass.query_running {
            self.fail("an ordered write splitting a pass with a running query is not built yet");
            self.pass = Some(pass);
            return;
        }
        self.slow.hit(SlowPath::WriteSplitPass);
        drop(pass.rp);
        between(self);
        let encoder = self.encoder.as_mut().expect("a pass implies an encoder");
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
        if call.viewports.len() > 1 || call.scissors.len() > 1 {
            return Err(
                "one viewport and scissor per draw; fronts split the others (MultiViewportSplit)",
            );
        }
        if call.stencil_reference[0] != call.stencil_reference[1] {
            return Err(
                "one stencil reference per draw without STENCIL_PER_FACE (StencilFacesSplit)",
            );
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
        let layouts = call.program.0.layout_keys(&call.bindings);
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
        let layout = last.layout.as_ref().expect("set with the pipeline");
        for (i, gl) in layout.groups.iter().enumerate() {
            let Some(gl) = gl else { continue };
            let set = &call.bindings[i]
                .ok_or("a draw lacks a binding set its program reads")?
                .0;
            let held = last.groups[i].as_ref();
            let same_set = held.is_some_and(|l| Rc::ptr_eq(&l.set, set));
            let group = match held {
                Some(l)
                    if same_set
                        && l.group.layout == gl.id
                        && l.group.epoch.get() == self.bind_epoch =>
                {
                    l.group.clone()
                }
                _ => self.set_group(set, gl)?,
            };
            let mut offsets = [0; MAX_GROUP_DYNAMIC];
            let count = set.offsets(&group, &mut offsets);
            let bound = held.is_some_and(|l| {
                l.group.group.id == group.group.id && l.offsets[..l.count] == offsets[..count]
            });
            if !bound {
                rp.set_bind_group(i as u32, &group.group.raw, &offsets[..count]);
            }
            if !same_set {
                set.mark_used(self.serial);
            }
            if !bound || !same_set {
                last.groups[i] = Some(LastGroup {
                    set: set.clone(),
                    group,
                    offsets,
                    count,
                });
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
                rp.set_vertex_buffer(
                    slot as u32,
                    b.raw.borrow().slice(b.base.get() + vb.offset..),
                );
                b.used_in.set(self.serial);
                last.vertex[slot] = now;
            }
        }
        if let (Some(i), Some(format)) = (&call.index, index_format) {
            let b = &i.buffer.0;
            let now = Some((b.id, b.generation.get(), i.offset, format));
            if last.index != now {
                rp.set_index_buffer(b.raw.borrow().slice(b.base.get() + i.offset..), format);
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
                rp.draw_indirect(&args.0.raw.borrow(), args.0.base.get() + offset);
            }
            DrawRange::IndexedIndirect { args, offset } => {
                args.0.used_in.set(self.serial);
                rp.draw_indexed_indirect(&args.0.raw.borrow(), args.0.base.get() + offset);
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
        let keys = call.program.0.layout_keys(&call.bindings);
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
            let group = self.set_group(&set.0, gl).map_err(GpuError::Invalid)?;
            set.0.mark_used(self.serial);
            let mut offsets = [0; MAX_GROUP_DYNAMIC];
            let count = set.0.offsets(&group, &mut offsets);
            groups.push((i as u32, group, offsets, count));
        }
        let indirect = match &call.groups {
            DispatchGroups::Indirect { args, offset } => {
                args.0.used_in.set(self.serial);
                Some((args.0.raw.borrow().clone(), args.0.base.get() + offset))
            }
            DispatchGroups::Direct(_) => None,
        };
        let mut cp = self
            .encoder()
            .begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        cp.set_pipeline(&pipeline);
        for (i, g, offsets, count) in &groups {
            cp.set_bind_group(*i, &g.group.raw, &offsets[..*count]);
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
        format: wcontainers_gpu::Format,
    ) {
        if self.pass.is_some() {
            self.fail("resolve inside a pass");
            return;
        }
        // Both views in the resolve format, so an sRGB twin averages in linear light.
        let Some(format) = convert::texture_format(format) else {
            self.fail("resolve in a format this back end does not hold");
            return;
        };
        let view = |r: &wcontainers_gpu::TextureRegion<'_, Self>| {
            r.texture.0.used_in.set(self.serial);
            r.texture.0.raw.create_view(&wgpu::TextureViewDescriptor {
                format: Some(format),
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
