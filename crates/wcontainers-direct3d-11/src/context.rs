//! The immediate context: its state, passes opened at draws, draws, copies, writes and maps.

use std::rc::Rc;

use wcontainers_gpu::{
    AspectOps, Backend, BindingEntry, BindingResource, BufferRange, ClearValue, ColorAttachment,
    DepthAttachment, DrawCall, DrawRange, Extent, FormatKind, IndexBinding, IndexFormat, LoadOp,
    MAX_BINDING_SETS, Object, Origin, PassDesc, Program, ReadSource, ScissorRect, SlotKind,
    SlowPath, StageSet, StepMode, StoreOp, TextureRegion, Topology, VertexAttribute, VertexBinding,
    VertexLayoutDesc, VertexStream, Viewport as GpuViewport, WriteMode,
};

use crate::desc::{self, Box3, Viewport, map, topology, usage};
use crate::device::{Device, integer_format};
use crate::error::{Error, Result, not_implemented};
use crate::format;
use crate::object::*;

/// The shader stages whose slots this context keeps: vertex (set 0) and pixel (set 1).
pub const VERTEX: usize = 0;
pub const PIXEL: usize = 1;

/// A vertex buffer slot: buffer, stride, offset.
type VertexSlot<B> = (Rc<Buffer<B>>, u32, u32);

/// A clear recorded but not yet run: it becomes the next pass's load, or a `clear_texture`.
struct PendingClear<B: Backend> {
    view: Rc<View<B>>,
    value: ClearValue,
}

/// A mapped subresource, released by `unmap`.
pub struct Mapped {
    pub data: *mut u8,
    pub row_pitch: u32,
    pub depth_pitch: u32,
}

pub struct ContextState<B: Backend> {
    vs: Option<Rc<Shader<B>>>,
    ps: Option<Rc<Shader<B>>>,
    layout: Option<Rc<InputLayout>>,
    topology: u32,
    vertex_buffers: Vec<Option<VertexSlot<B>>>,
    index: Option<(Rc<Buffer<B>>, IndexFormat, u32)>,
    constant_buffers: [Vec<Option<Rc<Buffer<B>>>>; 2],
    resources: [Vec<Option<Rc<View<B>>>>; 2],
    samplers: [Vec<Option<Rc<Sampler<B>>>>; 2],
    targets: Vec<Option<Rc<View<B>>>>,
    depth: Option<Rc<View<B>>>,
    blend: Option<Rc<Blend<B>>>,
    blend_factor: [f32; 4],
    sample_mask: u32,
    depth_stencil: Option<Rc<DepthStencil<B>>>,
    stencil_ref: u32,
    raster: Option<Rc<Rasterizer<B>>>,
    viewports: Vec<Viewport>,
    scissors: Vec<ScissorRect>,
    sets: [Option<Kept<B, B::BindingSet>>; 2],
    sets_dirty: [bool; 2],
    pass_open: bool,
    clears: Vec<PendingClear<B>>,
    /// Mapped subresources: the resource's address, subresource, map type.
    mapped: Vec<(usize, u32, u32)>,
}

impl<B: Backend> ContextState<B> {
    pub fn new() -> Self {
        ContextState {
            vs: None,
            ps: None,
            layout: None,
            topology: topology::UNDEFINED,
            vertex_buffers: vec![None; 32],
            index: None,
            constant_buffers: [vec![None; 14], vec![None; 14]],
            resources: [vec![None; 128], vec![None; 128]],
            samplers: [vec![None; 16], vec![None; 16]],
            targets: vec![None; 8],
            depth: None,
            blend: None,
            blend_factor: [1.0; 4],
            sample_mask: u32::MAX,
            depth_stencil: None,
            stencil_ref: 0,
            raster: None,
            viewports: Vec::new(),
            scissors: Vec::new(),
            sets: [None, None],
            sets_dirty: [true; 2],
            pass_open: false,
            clears: Vec::new(),
            mapped: Vec::new(),
        }
    }
}

impl<B: Backend> Default for ContextState<B> {
    fn default() -> Self {
        Self::new()
    }
}

fn put<T: Clone>(slots: &mut [Option<T>], start: u32, items: &[Option<T>]) -> Result<()> {
    let start = start as usize;
    if start + items.len() > slots.len() {
        return Err(Error::InvalidArg("slots past the stage's last"));
    }
    slots[start..start + items.len()].clone_from_slice(items);
    Ok(())
}

fn same<T>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// Indices of the vertices an adjacency topology draws without a geometry shader, as a list of its
/// base topology (OpenGL's definition of primitives with adjacency, which Direct3D shares).
fn adjacency_indices(topology: u32, count: u32) -> Option<(Topology, Vec<u32>)> {
    let mut out = Vec::new();
    let base = match topology {
        topology::LINELIST_ADJ => {
            for k in 0..count / 4 {
                out.extend([4 * k + 1, 4 * k + 2]);
            }
            Topology::LineList
        }
        topology::LINESTRIP_ADJ => {
            for i in 1..count.saturating_sub(2) {
                out.extend([i, i + 1]);
            }
            Topology::LineList
        }
        topology::TRIANGLELIST_ADJ => {
            for k in 0..count / 6 {
                out.extend([6 * k, 6 * k + 2, 6 * k + 4]);
            }
            Topology::TriangleList
        }
        topology::TRIANGLESTRIP_ADJ => {
            let triangles = count.saturating_sub(4) / 2;
            for i in 0..triangles {
                if i % 2 == 0 {
                    out.extend([2 * i, 2 * i + 2, 2 * i + 4]);
                } else {
                    out.extend([2 * i + 2, 2 * i, 2 * i + 4]);
                }
            }
            Topology::TriangleList
        }
        _ => return None,
    };
    Some((base, out))
}

fn plain_topology(t: u32) -> Option<Topology> {
    Some(match t {
        topology::POINTLIST => Topology::PointList,
        topology::LINELIST => Topology::LineList,
        topology::LINESTRIP => Topology::LineStrip,
        topology::TRIANGLELIST => Topology::TriangleList,
        topology::TRIANGLESTRIP => Topology::TriangleStrip,
        _ => return None,
    })
}

/// How a draw names its range before the front picks the interface's form.
#[derive(Clone, Copy)]
enum Range {
    Vertices { count: u32, start: u32 },
    Indexed { count: u32, start: u32, base: i32 },
}

impl<B: Backend> Device<B> {
    pub fn vs_set_shader(&mut self, shader: Option<Rc<Shader<B>>>) {
        if !same(&self.ctx.vs, &shader) {
            self.ctx.vs = shader;
            self.ctx.sets_dirty[VERTEX] = true;
        }
    }

    pub fn ps_set_shader(&mut self, shader: Option<Rc<Shader<B>>>) {
        if !same(&self.ctx.ps, &shader) {
            self.ctx.ps = shader;
            self.ctx.sets_dirty[PIXEL] = true;
        }
    }

    pub fn ia_set_input_layout(&mut self, layout: Option<Rc<InputLayout>>) {
        self.ctx.layout = layout;
    }

    pub fn ia_set_primitive_topology(&mut self, topology: u32) {
        self.ctx.topology = topology;
    }

    pub fn ia_set_vertex_buffers(
        &mut self,
        start: u32,
        slots: &[Option<VertexSlot<B>>],
    ) -> Result<()> {
        put(&mut self.ctx.vertex_buffers, start, slots)
    }

    pub fn ia_set_index_buffer(
        &mut self,
        buffer: Option<Rc<Buffer<B>>>,
        format: u32,
        offset: u32,
    ) -> Result<()> {
        self.ctx.index = match buffer {
            None => None,
            Some(b) => Some((
                b,
                format::index_format(format).ok_or(Error::InvalidArg("index format"))?,
                offset,
            )),
        };
        Ok(())
    }

    pub fn set_constant_buffers(
        &mut self,
        stage: usize,
        start: u32,
        b: &[Option<Rc<Buffer<B>>>],
    ) -> Result<()> {
        put(&mut self.ctx.constant_buffers[stage], start, b)?;
        self.ctx.sets_dirty[stage] = true;
        Ok(())
    }

    pub fn set_shader_resources(
        &mut self,
        stage: usize,
        start: u32,
        v: &[Option<Rc<View<B>>>],
    ) -> Result<()> {
        put(&mut self.ctx.resources[stage], start, v)?;
        self.ctx.sets_dirty[stage] = true;
        Ok(())
    }

    pub fn set_samplers(
        &mut self,
        stage: usize,
        start: u32,
        s: &[Option<Rc<Sampler<B>>>],
    ) -> Result<()> {
        put(&mut self.ctx.samplers[stage], start, s)?;
        self.ctx.sets_dirty[stage] = true;
        Ok(())
    }

    pub fn om_set_render_targets(
        &mut self,
        targets: &[Option<Rc<View<B>>>],
        depth: Option<Rc<View<B>>>,
    ) -> Result<()> {
        if targets.len() > 8 {
            return Err(Error::InvalidArg("more than eight render targets"));
        }
        let mut new = vec![None; 8];
        new[..targets.len()].clone_from_slice(targets);
        let changed = !same(&self.ctx.depth, &depth)
            || new.iter().zip(&self.ctx.targets).any(|(a, b)| !same(a, b));
        if changed {
            self.end_pass();
            self.ctx.targets = new;
            self.ctx.depth = depth;
        }
        Ok(())
    }

    pub fn om_set_blend_state(
        &mut self,
        blend: Option<Rc<Blend<B>>>,
        factor: Option<[f32; 4]>,
        mask: u32,
    ) {
        self.ctx.blend = blend;
        self.ctx.blend_factor = factor.unwrap_or([1.0; 4]);
        self.ctx.sample_mask = mask;
    }

    pub fn om_set_depth_stencil_state(
        &mut self,
        state: Option<Rc<DepthStencil<B>>>,
        reference: u32,
    ) {
        self.ctx.depth_stencil = state;
        self.ctx.stencil_ref = reference;
    }

    pub fn rs_set_state(&mut self, state: Option<Rc<Rasterizer<B>>>) {
        self.ctx.raster = state;
    }

    pub fn rs_set_viewports(&mut self, viewports: &[Viewport]) {
        self.ctx.viewports = viewports.to_vec();
    }

    /// Scissor rectangles as (left, top, right, bottom).
    pub fn rs_set_scissor_rects(&mut self, rects: &[[i32; 4]]) {
        self.ctx.scissors = rects
            .iter()
            .map(|&[l, t, r, b]| ScissorRect {
                x: l.max(0) as u32,
                y: t.max(0) as u32,
                width: (r - l.max(0)).max(0) as u32,
                height: (b - t.max(0)).max(0) as u32,
            })
            .collect();
    }

    pub fn clear_render_target_view(&mut self, view: &Rc<View<B>>, color: [f32; 4]) {
        let bits = if integer_format(view.format) {
            color.map(|c| c as i64 as u32)
        } else {
            color.map(f32::to_bits)
        };
        self.add_clear(view, ClearValue::Color(bits));
    }

    pub fn clear_depth_stencil_view(
        &mut self,
        view: &Rc<View<B>>,
        flags: u32,
        depth: f32,
        stencil: u8,
    ) {
        let kind = view.format.kind();
        let has_stencil = matches!(kind, FormatKind::DepthStencil | FormatKind::Stencil);
        let value = ClearValue::DepthStencil {
            depth: (flags & desc::clear::DEPTH != 0 && kind != FormatKind::Stencil)
                .then(|| depth.clamp(0.0, 1.0).to_bits()),
            stencil: (flags & desc::clear::STENCIL != 0 && has_stencil).then_some(stencil),
        };
        self.add_clear(view, value);
    }

    fn add_clear(&mut self, view: &Rc<View<B>>, value: ClearValue) {
        self.end_pass();
        if let Some(i) = self
            .ctx
            .clears
            .iter()
            .position(|c| Rc::ptr_eq(&c.view, view))
        {
            let old = self.ctx.clears.remove(i);
            if let (
                ClearValue::DepthStencil {
                    depth: d0,
                    stencil: s0,
                },
                ClearValue::DepthStencil { depth, stencil },
            ) = (old.value, value)
            {
                let merged = ClearValue::DepthStencil {
                    depth: depth.or(d0),
                    stencil: stencil.or(s0),
                };
                self.ctx.clears.push(PendingClear {
                    view: view.clone(),
                    value: merged,
                });
                return;
            }
        }
        self.ctx.clears.push(PendingClear {
            view: view.clone(),
            value,
        });
    }

    pub(crate) fn end_pass(&mut self) {
        if self.ctx.pass_open {
            self.backend.end_pass();
            self.ctx.pass_open = false;
        }
    }

    /// Ends the pass and runs every pending clear, before work outside a pass.
    pub(crate) fn settle(&mut self) {
        self.end_pass();
        let clears = std::mem::take(&mut self.ctx.clears);
        self.run_clears(clears);
    }

    fn run_clears(&mut self, clears: Vec<PendingClear<B>>) {
        for c in clears {
            let v = &c.view;
            let mips = v.desc.first_mip..v.desc.first_mip + 1;
            let layers = v.desc.first_layer..v.desc.first_layer + v.desc.layer_count;
            if let Err(e) = self.backend.clear_texture(
                &v.texture.gpu.handle,
                &v.texture.gpu_desc,
                v.format,
                mips,
                layers,
                c.value,
            ) {
                self.lose(e.into());
            }
        }
    }

    /// Opens a pass on the bound targets, taking pending clears of them as its loads.
    fn open_pass(&mut self) {
        if self.ctx.pass_open {
            return;
        }
        let bound = |v: &Rc<View<B>>, ctx: &ContextState<B>| {
            ctx.targets.iter().flatten().any(|t| Rc::ptr_eq(t, v))
                || ctx.depth.as_ref().is_some_and(|d| Rc::ptr_eq(d, v))
        };
        let (mine, others): (Vec<_>, Vec<_>) = std::mem::take(&mut self.ctx.clears)
            .into_iter()
            .partition(|c| bound(&c.view, &self.ctx));
        self.run_clears(others);
        let clear_of = |v: &Rc<View<B>>| {
            mine.iter()
                .find(|c| Rc::ptr_eq(&c.view, v))
                .map(|c| c.value)
        };
        let targets = self.ctx.targets.clone();
        let depth = self.ctx.depth.clone();
        let colors = std::array::from_fn(|i| {
            targets[i].as_ref().map(|v| ColorAttachment {
                view: &v.gpu.handle,
                depth_slice: 0,
                resolve: None,
                load: match clear_of(v) {
                    Some(ClearValue::Color(bits)) => LoadOp::Clear(bits),
                    _ => LoadOp::Load,
                },
                store: StoreOp::Store,
            })
        });
        let depth_attachment = depth.as_ref().map(|v| {
            let (d, s) = match clear_of(v) {
                Some(ClearValue::DepthStencil { depth, stencil }) => (depth, stencil),
                _ => (None, None),
            };
            let kind = v.format.kind();
            fn ops<V>(clear: Option<V>) -> AspectOps<V> {
                AspectOps {
                    load: clear.map_or(LoadOp::Load, LoadOp::Clear),
                    store: StoreOp::Store,
                    read_only: false,
                }
            }
            DepthAttachment {
                view: &v.gpu.handle,
                depth: (kind != FormatKind::Stencil).then(|| ops(d)),
                stencil: matches!(kind, FormatKind::DepthStencil | FormatKind::Stencil)
                    .then(|| ops(s)),
            }
        });
        self.backend.begin_pass(&PassDesc {
            colors,
            depth: depth_attachment,
            empty: None,
            occlusion: None,
        });
        self.ctx.pass_open = true;
    }

    /// Records that the device is lost, as Direct3D reports it from then on.
    pub(crate) fn lose(&mut self, e: Error) {
        eprintln!("wcontainers direct3d 11: device removed: {e:?}");
        self.removed = true;
    }

    pub fn draw(&mut self, count: u32, start: u32) {
        self.draw_any(Range::Vertices { count, start }, 0, 1);
    }

    pub fn draw_instanced(&mut self, count: u32, instances: u32, start: u32, first_instance: u32) {
        self.draw_any(Range::Vertices { count, start }, first_instance, instances);
    }

    pub fn draw_indexed(&mut self, count: u32, start: u32, base: i32) {
        self.draw_any(Range::Indexed { count, start, base }, 0, 1);
    }

    pub fn draw_indexed_instanced(
        &mut self,
        count: u32,
        instances: u32,
        start: u32,
        base: i32,
        first_instance: u32,
    ) {
        self.draw_any(
            Range::Indexed { count, start, base },
            first_instance,
            instances,
        );
    }

    /// Draws; a draw this front cannot serve removes the device, the only answer Direct3D has for it.
    fn draw_any(&mut self, range: Range, first_instance: u32, instances: u32) {
        if let Err(e) = self.try_draw(range, first_instance, instances) {
            self.lose(e);
        }
    }

    fn program(&mut self, vs: &Rc<Shader<B>>, ps: Option<&Rc<Shader<B>>>) -> Result<B::Program> {
        let key = (vs.gpu.id, ps.map_or(0, |p| p.gpu.id));
        if let Some(p) = self.programs.get(&key) {
            return Ok(p.handle.clone());
        }
        let program = self.backend.create_program(&Program::Render {
            vertex: &vs.gpu.handle,
            fragment: ps.map(|p| &p.gpu.handle),
        })?;
        let id = self.id();
        self.programs.insert(
            key,
            Kept::new(program.clone(), id, Object::Program, &self.graveyard),
        );
        Ok(program)
    }

    fn vertex_layout(&mut self) -> Result<B::VertexLayout> {
        let layout = self.ctx.layout.clone();
        let mut strides = [0u32; 32];
        if let Some(l) = &layout {
            for e in &l.elements {
                strides[e.slot as usize] = self.ctx.vertex_buffers[e.slot as usize]
                    .as_ref()
                    .map_or(0, |v| v.1);
            }
        }
        let key = (layout.as_ref().map_or(0, |l| l.id), strides);
        if let Some(v) = self.layouts.get(&key) {
            return Ok(v.handle.clone());
        }
        let mut desc = VertexLayoutDesc {
            streams: [None; 32],
            attributes: [None; 32],
        };
        if let Some(l) = &layout {
            for (i, e) in l.elements.iter().enumerate() {
                let slot = e.slot as usize;
                desc.streams[slot] = Some(VertexStream {
                    stride: strides[slot],
                    step: match l.instance_steps[slot] {
                        None => StepMode::Vertex,
                        Some(rate) => StepMode::Instance(rate),
                    },
                });
                desc.attributes[i] = Some(VertexAttribute {
                    location: e.location,
                    stream: e.slot as u8,
                    offset: e.offset,
                    format: e.format,
                });
            }
        }
        let gpu = self.backend.create_vertex_layout(&desc)?;
        let id = self.id();
        self.layouts.insert(
            key,
            Kept::new(gpu.clone(), id, Object::VertexLayout, &self.graveyard),
        );
        Ok(gpu)
    }

    /// Makes the binding set of `stage` from its slots and its shader's reflection, when it changed.
    fn update_set(&mut self, stage: usize) -> Result<()> {
        if !self.ctx.sets_dirty[stage] {
            return Ok(());
        }
        let shader = if stage == VERTEX {
            self.ctx.vs.clone()
        } else {
            self.ctx.ps.clone()
        };
        let wanted: Vec<_> = shader
            .map(|s| {
                s.reflection
                    .bindings
                    .iter()
                    .filter(|b| b.set as usize == stage)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if wanted.is_empty() {
            self.ctx.sets[stage] = None;
            self.ctx.sets_dirty[stage] = false;
            return Ok(());
        }
        let stages = if stage == VERTEX {
            StageSet::VERTEX
        } else {
            StageSet::FRAGMENT
        };
        let cbs = self.ctx.constant_buffers[stage].clone();
        let views = self.ctx.resources[stage].clone();
        let samplers = self.ctx.samplers[stage].clone();
        let mut entries = Vec::with_capacity(wanted.len());
        for b in &wanted {
            let slot = b.register as usize;
            let resource = match b.kind {
                SlotKind::Uniform => cbs.get(slot).and_then(Option::as_ref).map(|c| {
                    BindingResource::UniformBuffer(BufferRange {
                        buffer: &c.gpu.handle,
                        offset: 0,
                        size: u64::from(c.desc.byte_width),
                    })
                }),
                SlotKind::Texture => views
                    .get(slot)
                    .and_then(Option::as_ref)
                    .map(|v| BindingResource::Texture(&v.gpu.handle)),
                SlotKind::Sampler => samplers
                    .get(slot)
                    .and_then(Option::as_ref)
                    .map(|s| BindingResource::Sampler(&s.gpu.handle)),
                _ => return not_implemented("storage views"),
            };
            let Some(resource) = resource else {
                return not_implemented("a shader reading an empty slot (Direct3D reads zeros)");
            };
            entries.push(BindingEntry {
                binding: b.binding,
                stages,
                resource,
            });
        }
        // The entries borrow handles from the slot clones above, which live to the end of this call.
        let set = self.backend.create_binding_set(&entries)?;
        drop(entries);
        let id = self.id();
        self.ctx.sets[stage] = Some(Kept::new(set, id, Object::BindingSet, &self.graveyard));
        self.ctx.sets_dirty[stage] = false;
        Ok(())
    }

    fn try_draw(&mut self, range: Range, first_instance: u32, instances: u32) -> Result<()> {
        if self.removed {
            return Ok(());
        }
        self.bury();
        // Without a vertex shader Direct3D draws nothing.
        let Some(vs) = self.ctx.vs.clone() else {
            return Ok(());
        };
        if self.ctx.sample_mask & 1 == 0 || instances == 0 {
            return Ok(());
        }
        let ps = self.ctx.ps.clone();
        let program = self.program(&vs, ps.as_ref())?;
        let vertex_layout = self.vertex_layout()?;
        self.update_set(VERTEX)?;
        self.update_set(PIXEL)?;

        let indexed = matches!(range, Range::Indexed { .. });
        let (topology, gpu_range, generated) = match plain_topology(self.ctx.topology) {
            Some(t) => {
                let r = match range {
                    Range::Vertices { count, start } => DrawRange::Vertices {
                        first: start,
                        count,
                        first_instance,
                        instances,
                    },
                    Range::Indexed { count, start, base } => DrawRange::Indexed {
                        first: start,
                        count,
                        base_vertex: base,
                        first_instance,
                        instances,
                    },
                };
                (t, r, None)
            }
            None => {
                let Range::Vertices { count, start } = range else {
                    return not_implemented("indexed adjacency draws without a geometry shader");
                };
                let Some((t, list)) = adjacency_indices(self.ctx.topology, count) else {
                    return not_implemented("a topology without a geometry or hull shader");
                };
                self.backend.slow_paths().hit(SlowPath::AdjacencyDropped);
                let bytes: Vec<u8> = list.iter().flat_map(|i| i.to_le_bytes()).collect();
                let mut offsets = [0u64];
                let ring = self.backend.write_buffer_ring(
                    wcontainers_gpu::RingKind::Indices,
                    &[&bytes],
                    4,
                    &mut offsets,
                )?;
                let r = DrawRange::Indexed {
                    first: (offsets[0] / 4) as u32,
                    count: list.len() as u32,
                    base_vertex: start as i32,
                    first_instance,
                    instances,
                };
                (t, r, Some(ring))
            }
        };
        let nothing = matches!(
            gpu_range,
            DrawRange::Indexed { count: 0, .. } | DrawRange::Vertices { count: 0, .. }
        );
        if nothing || self.ctx.viewports.is_empty() || (indexed && self.ctx.index.is_none()) {
            return Ok(());
        }
        self.open_pass();

        let ctx = &self.ctx;
        let vertex_buffers: Vec<Option<VertexBinding<'_, B>>> = ctx
            .vertex_buffers
            .iter()
            .map(|s| {
                s.as_ref().map(|(b, _, offset)| VertexBinding {
                    buffer: &b.gpu.handle,
                    offset: u64::from(*offset),
                })
            })
            .collect();
        let used = vertex_buffers
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1);
        let index = match (&generated, &ctx.index) {
            (Some(ring), _) => Some(IndexBinding {
                buffer: ring,
                offset: 0,
                format: IndexFormat::U32,
            }),
            (None, Some((b, f, offset))) if indexed => Some(IndexBinding {
                buffer: &b.gpu.handle,
                offset: u64::from(*offset),
                format: *f,
            }),
            _ => None,
        };
        let viewports: Vec<GpuViewport> = ctx
            .viewports
            .iter()
            .map(|v| GpuViewport {
                x: v.x.to_bits(),
                y: v.y.to_bits(),
                width: v.width.to_bits(),
                height: v.height.to_bits(),
                min_depth: v.min_depth.to_bits(),
                max_depth: v.max_depth.to_bits(),
            })
            .collect();
        let blend = ctx.blend.as_ref().unwrap_or(&self.defaults.blend);
        let depth_stencil = ctx
            .depth_stencil
            .as_ref()
            .unwrap_or(&self.defaults.depth_stencil);
        let raster = ctx.raster.as_ref().unwrap_or(&self.defaults.raster);
        let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        bindings[0] = ctx.sets[VERTEX].as_ref().map(|k| &k.handle);
        bindings[1] = ctx.sets[PIXEL].as_ref().map(|k| &k.handle);
        let strip = matches!(topology, Topology::LineStrip | Topology::TriangleStrip);
        let call = DrawCall {
            program: &program,
            blend: &blend.gpu.handle,
            depth_stencil: &depth_stencil.gpu.handle,
            raster: &raster.gpu.handle,
            vertex_layout: &vertex_layout,
            bindings,
            vertex_buffers: &vertex_buffers[..used],
            index,
            topology,
            restart: strip && indexed,
            viewports: &viewports,
            scissors: &ctx.scissors,
            blend_constant: ctx.blend_factor.map(f32::to_bits),
            stencil_reference: [ctx.stencil_ref as u8; 2],
            immediates: &[],
            range: gpu_range,
        };
        self.backend.draw(&call);
        Ok(())
    }

    pub fn copy_resource(&mut self, dst: &Resource<B>, src: &Resource<B>) -> Result<()> {
        match (dst, src) {
            (Resource::Buffer(d), Resource::Buffer(s)) => {
                if d.desc.byte_width != s.desc.byte_width || Rc::ptr_eq(d, s) {
                    return Err(Error::InvalidArg(
                        "CopyResource needs two resources of one size",
                    ));
                }
                self.settle();
                self.backend.copy_buffer(
                    &s.gpu.handle,
                    0,
                    &d.gpu.handle,
                    0,
                    u64::from(s.desc.byte_width),
                );
                Ok(())
            }
            (Resource::Texture2D(d), Resource::Texture2D(s)) => {
                let (a, b) = (&d.desc, &s.desc);
                if (a.width, a.height, a.mip_levels, a.array_size)
                    != (b.width, b.height, b.mip_levels, b.array_size)
                    || d.format != s.format
                    || Rc::ptr_eq(d, s)
                {
                    return Err(Error::InvalidArg(
                        "CopyResource needs two textures of one shape",
                    ));
                }
                for sub in 0..a.mip_levels * a.array_size {
                    self.copy_subresource_region(dst, sub, [0; 3], src, sub, None)?;
                }
                Ok(())
            }
            _ => Err(Error::InvalidArg(
                "CopyResource between a buffer and a texture",
            )),
        }
    }

    pub fn copy_subresource_region(
        &mut self,
        dst: &Resource<B>,
        dst_sub: u32,
        at: [u32; 3],
        src: &Resource<B>,
        src_sub: u32,
        src_box: Option<&Box3>,
    ) -> Result<()> {
        match (dst, src) {
            (Resource::Buffer(d), Resource::Buffer(s)) => {
                let (from, to) = src_box.map_or((0, s.desc.byte_width), |b| (b.left, b.right));
                if to <= from || to > s.desc.byte_width || at[0] + (to - from) > d.desc.byte_width {
                    return Ok(());
                }
                self.settle();
                self.backend.copy_buffer(
                    &s.gpu.handle,
                    u64::from(from),
                    &d.gpu.handle,
                    u64::from(at[0]),
                    u64::from(to - from),
                );
                Ok(())
            }
            (Resource::Texture2D(d), Resource::Texture2D(s)) => {
                if d.format != s.format {
                    return not_implemented("copies between formats of a typeless group");
                }
                let (sm, dm) = (s.desc.mip_levels, d.desc.mip_levels);
                if src_sub >= sm * s.desc.array_size || dst_sub >= dm * d.desc.array_size {
                    return Err(Error::InvalidArg("subresource past the resource"));
                }
                let (src_mip, src_layer) = (src_sub % sm, src_sub / sm);
                let (dst_mip, dst_layer) = (dst_sub % dm, dst_sub / dm);
                let (w, h) = s.mip_size(src_mip);
                let b = src_box.copied().unwrap_or(Box3 {
                    left: 0,
                    top: 0,
                    front: 0,
                    right: w,
                    bottom: h,
                    back: 1,
                });
                let (dw, dh) = d.mip_size(dst_mip);
                if b.right <= b.left || b.bottom <= b.top || b.right > w || b.bottom > h {
                    return Ok(());
                }
                let extent = Extent {
                    width: b.right - b.left,
                    height: b.bottom - b.top,
                    depth_or_layers: 1,
                };
                if at[0] + extent.width > dw || at[1] + extent.height > dh {
                    return Ok(());
                }
                self.settle();
                self.backend.copy_texture(
                    &TextureRegion {
                        texture: &s.gpu.handle,
                        mip: src_mip,
                        layer: src_layer,
                        origin: Origin {
                            x: b.left,
                            y: b.top,
                            z: 0,
                        },
                        stencil: false,
                    },
                    &TextureRegion {
                        texture: &d.gpu.handle,
                        mip: dst_mip,
                        layer: dst_layer,
                        origin: Origin {
                            x: at[0],
                            y: at[1],
                            z: 0,
                        },
                        stencil: false,
                    },
                    extent,
                );
                Ok(())
            }
            _ => Err(Error::InvalidArg("a copy between a buffer and a texture")),
        }
    }

    /// `UpdateSubresource`: `bytes` is the stand-in's copy of the program's data.
    pub fn update_subresource(
        &mut self,
        dst: &Resource<B>,
        sub: u32,
        dst_box: Option<&Box3>,
        bytes: &[u8],
        row_pitch: u32,
    ) -> Result<()> {
        match dst {
            Resource::Buffer(b) => {
                if b.desc.usage != usage::DEFAULT {
                    return Err(Error::InvalidArg("UpdateSubresource needs default usage"));
                }
                let (from, to) = dst_box.map_or((0, b.desc.byte_width), |x| (x.left, x.right));
                if to <= from || to > b.desc.byte_width {
                    return Ok(());
                }
                let whole = from == 0 && to == b.desc.byte_width;
                let mode = if whole {
                    WriteMode::Discard
                } else {
                    WriteMode::Ordered
                };
                self.backend.write_buffer(
                    &b.gpu.handle,
                    u64::from(from),
                    &bytes[..(to - from) as usize],
                    mode,
                );
                Ok(())
            }
            Resource::Texture2D(t) => {
                if t.desc.usage != usage::DEFAULT {
                    return Err(Error::InvalidArg("UpdateSubresource needs default usage"));
                }
                let mips = t.desc.mip_levels;
                if sub >= mips * t.desc.array_size {
                    return Err(Error::InvalidArg("subresource past the resource"));
                }
                let (mip, layer) = (sub % mips, sub / mips);
                let (w, h) = t.mip_size(mip);
                let b = dst_box.copied().unwrap_or(Box3 {
                    right: w,
                    bottom: h,
                    back: 1,
                    ..Default::default()
                });
                if b.right <= b.left || b.bottom <= b.top || b.right > w || b.bottom > h {
                    return Ok(());
                }
                self.settle();
                let (_, rows) = format::row_bytes(t.format, b.right - b.left, b.bottom - b.top);
                self.backend.write_texture(
                    &TextureRegion {
                        texture: &t.gpu.handle,
                        mip,
                        layer,
                        origin: Origin {
                            x: b.left,
                            y: b.top,
                            z: 0,
                        },
                        stencil: false,
                    },
                    Extent {
                        width: b.right - b.left,
                        height: b.bottom - b.top,
                        depth_or_layers: 1,
                    },
                    bytes,
                    row_pitch,
                    rows,
                    WriteMode::Ordered,
                );
                Ok(())
            }
        }
    }

    pub fn map(
        &mut self,
        resource: &Resource<B>,
        sub: u32,
        kind: u32,
        _flags: u32,
    ) -> Result<Mapped> {
        let reads = matches!(kind, map::READ | map::READ_WRITE);
        let writes = kind != map::READ;
        match resource {
            Resource::Buffer(b) => {
                let d = &b.desc;
                let allowed = match kind {
                    map::WRITE_DISCARD | map::WRITE_NO_OVERWRITE => d.usage == usage::DYNAMIC,
                    _ => {
                        d.usage == usage::STAGING
                            && (!reads || d.cpu_access_flags & desc::cpu::READ != 0)
                            && (!writes || d.cpu_access_flags & desc::cpu::WRITE != 0)
                    }
                };
                if sub != 0 || !allowed {
                    return Err(Error::InvalidArg("the resource cannot be mapped this way"));
                }
                if d.usage == usage::STAGING {
                    self.settle();
                    let src = ReadSource::Buffer {
                        buffer: &b.gpu.handle,
                        offset: 0,
                        size: u64::from(d.byte_width),
                    };
                    self.backend
                        .read_back_now(&src, &mut b.shadow.borrow_mut())?;
                }
                self.ctx.mapped.push((Rc::as_ptr(b) as usize, 0, kind));
                let data = b.shadow.borrow_mut().as_mut_ptr();
                Ok(Mapped {
                    data,
                    row_pitch: d.byte_width,
                    depth_pitch: d.byte_width,
                })
            }
            Resource::Texture2D(t) => {
                let d = &t.desc;
                let ok = d.usage == usage::STAGING
                    && (!reads || d.cpu_access_flags & desc::cpu::READ != 0)
                    && (!writes || d.cpu_access_flags & desc::cpu::WRITE != 0)
                    && matches!(kind, map::READ | map::WRITE | map::READ_WRITE);
                if !ok || sub >= d.mip_levels * d.array_size {
                    return Err(Error::InvalidArg("the resource cannot be mapped this way"));
                }
                self.settle();
                if !reads {
                    self.backend.slow_paths().hit(SlowPath::StagingRefreshed);
                }
                let (mip, layer) = (sub % d.mip_levels, sub / d.mip_levels);
                let (w, h) = t.mip_size(mip);
                let (row, rows) = format::row_bytes(t.format, w, h);
                let src = ReadSource::Texture {
                    region: TextureRegion {
                        texture: &t.gpu.handle,
                        mip,
                        layer,
                        origin: Origin { x: 0, y: 0, z: 0 },
                        stencil: false,
                    },
                    extent: Extent {
                        width: w,
                        height: h,
                        depth_or_layers: 1,
                    },
                };
                let mut shadow = t.shadow.borrow_mut();
                self.backend
                    .read_back_now(&src, &mut shadow[sub as usize])?;
                self.ctx.mapped.push((Rc::as_ptr(t) as usize, sub, kind));
                Ok(Mapped {
                    data: shadow[sub as usize].as_mut_ptr(),
                    row_pitch: row,
                    depth_pitch: row * rows,
                })
            }
        }
    }

    pub fn unmap(&mut self, resource: &Resource<B>, sub: u32) {
        let address = match resource {
            Resource::Buffer(b) => Rc::as_ptr(b) as usize,
            Resource::Texture2D(t) => Rc::as_ptr(t) as usize,
        };
        let Some(i) = self
            .ctx
            .mapped
            .iter()
            .position(|&(a, s, _)| a == address && s == sub)
        else {
            return;
        };
        let (_, _, kind) = self.ctx.mapped.remove(i);
        if kind == map::READ {
            return;
        }
        self.backend.slow_paths().hit(SlowPath::MappedUploadWhole);
        match resource {
            Resource::Buffer(b) => {
                let mode = match kind {
                    map::WRITE_DISCARD => WriteMode::Discard,
                    map::WRITE_NO_OVERWRITE => WriteMode::NoOverwrite,
                    _ => WriteMode::Ordered,
                };
                self.backend
                    .write_buffer(&b.gpu.handle, 0, &b.shadow.borrow(), mode);
            }
            Resource::Texture2D(t) => {
                let mips = t.desc.mip_levels;
                let (mip, layer) = (sub % mips, sub / mips);
                let (w, h) = t.mip_size(mip);
                let (row, rows) = format::row_bytes(t.format, w, h);
                self.end_pass();
                self.backend.write_texture(
                    &TextureRegion {
                        texture: &t.gpu.handle,
                        mip,
                        layer,
                        origin: Origin { x: 0, y: 0, z: 0 },
                        stencil: false,
                    },
                    Extent {
                        width: w,
                        height: h,
                        depth_or_layers: 1,
                    },
                    &t.shadow.borrow()[sub as usize],
                    row,
                    rows,
                    WriteMode::Ordered,
                );
            }
        }
    }

    /// `ClearState`: every slot and state back to its default. Recorded work is kept.
    pub fn clear_state(&mut self) {
        self.end_pass();
        let clears = std::mem::take(&mut self.ctx.clears);
        let mapped = std::mem::take(&mut self.ctx.mapped);
        self.ctx = ContextState::new();
        self.ctx.clears = clears;
        self.ctx.mapped = mapped;
        self.bury();
    }

    /// `Flush`: runs what is recorded.
    pub fn flush(&mut self) {
        self.settle();
        self.bury();
        let _ = self.backend.submit();
    }
}
