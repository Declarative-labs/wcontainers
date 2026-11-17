//! The immediate context: its state, passes opened at draws, draws, copies, writes and maps.

use std::rc::Rc;

use wcontainers_gpu::{
    AspectOps, Backend, BindingEntry, BindingResource, BufferRange, ClearValue, ColorAttachment,
    DepthAttachment, DispatchCall, DispatchGroups, DrawCall, DrawRange, Extent, FormatKind,
    IndexBinding, IndexFormat, LoadOp, MAX_BINDING_SETS, Object, Origin, PassDesc, Program,
    ReadSource, ReadState, ScissorRect, SlotKind, SlowPath, StageSet, StepMode, StoreOp,
    SystemValue, TextureUsage, Topology, VertexAttribute, VertexBinding, VertexLayoutDesc,
    VertexStream, Viewport as GpuViewport, WriteMode,
};

use crate::desc::{
    self, Box3, SubresourceData, TexDim, TextureDesc, Viewport, map, topology, usage,
};
use crate::device::{Device, ZERO_BYTES, gpu_buffer, integer_format};
use crate::error::{Error, Result, not_implemented};
use crate::format;
use crate::object::*;

/// The shader stages whose slots this context keeps: vertex (set 0), pixel (set 1) and compute
/// (set 0 of its own programs).
pub const VERTEX: usize = 0;
pub const PIXEL: usize = 1;
pub const COMPUTE: usize = 2;
const STAGES: usize = 3;

/// Unordered access slots of the pixel and compute stages at feature level 11_0.
pub const UAV_SLOTS: usize = 8;

/// The binding set a stage's program reads its slots from.
fn set_of(stage: usize) -> u32 {
    if stage == PIXEL { 1 } else { 0 }
}

/// A vertex buffer slot: buffer, stride, offset.
type VertexSlot<B> = (Rc<Buffer<B>>, u32, u32);

/// Direct3D 11's vertex buffer slots and viewports.
const VERTEX_SLOTS: usize = 32;
const VIEWPORTS: usize = 16;

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
    cs: Option<Rc<Shader<B>>>,
    layout: Option<Rc<InputLayout>>,
    topology: u32,
    vertex_buffers: Vec<Option<VertexSlot<B>>>,
    index: Option<(Rc<Buffer<B>>, IndexFormat, u32)>,
    constant_buffers: [Vec<Option<Rc<Buffer<B>>>>; STAGES],
    /// Per constant buffer slot, the range bound (first constant, constants), or none for the
    /// whole buffer.
    constant_ranges: [Vec<Option<(u32, u32)>>; STAGES],
    resources: [Vec<Option<Rc<ShaderView<B>>>>; STAGES],
    samplers: [Vec<Option<Rc<Sampler<B>>>>; STAGES],
    /// Per stage, the unordered access views bound (none for the vertex stage).
    uavs: [Vec<Option<Rc<ShaderView<B>>>>; STAGES],
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
    sets: [Option<Kept<B, B::BindingSet>>; STAGES],
    sets_dirty: [bool; STAGES],
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
            cs: None,
            layout: None,
            topology: topology::UNDEFINED,
            vertex_buffers: vec![None; VERTEX_SLOTS],
            index: None,
            constant_buffers: std::array::from_fn(|_| vec![None; 14]),
            constant_ranges: std::array::from_fn(|_| vec![None; 14]),
            resources: std::array::from_fn(|_| vec![None; 128]),
            samplers: std::array::from_fn(|_| vec![None; 16]),
            uavs: [Vec::new(), vec![None; UAV_SLOTS], vec![None; UAV_SLOTS]],
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
            sets: [None, None, None],
            sets_dirty: [true; STAGES],
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

/// Binds `items` from `start`; true if any slot changed.
fn put_rc<T>(slots: &mut [Option<Rc<T>>], start: u32, items: &[Option<Rc<T>>]) -> Result<bool> {
    let start = start as usize;
    if start + items.len() > slots.len() {
        return Err(Error::InvalidArg("slots past the stage's last"));
    }
    let mut changed = false;
    for (slot, item) in slots[start..start + items.len()].iter_mut().zip(items) {
        if !same(slot, item) {
            *slot = item.clone();
            changed = true;
        }
    }
    Ok(changed)
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

/// Direct3D's answer to a copy between textures of DXGI formats `a` and `b`: formats of one typeless
/// group copy their bits; Direct3D 10.1's conversions between groups are not served yet; others are
/// invalid.
fn copy_formats(a: u32, b: u32) -> Result<()> {
    if format::typeless_group(a) == format::typeless_group(b) {
        Ok(())
    } else if format::copy_cast(a, b) {
        not_implemented(
            "copies between a block-compressed or shared-exponent format and an integer one",
        )
    } else {
        Err(Error::InvalidArg(
            "a copy between formats of different groups",
        ))
    }
}

/// The back end's object a draw binds for an empty slot.
enum Zero<B: Backend> {
    /// A buffer of zeros, bound for this many bytes.
    Buffer(B::Buffer, u64),
    View(B::TextureView),
    Sampler(B::Sampler),
    /// The zero buffer read as a structured or raw buffer.
    Storage(B::Buffer),
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

    pub fn cs_set_shader(&mut self, shader: Option<Rc<Shader<B>>>) {
        if !same(&self.ctx.cs, &shader) {
            self.ctx.cs = shader;
            self.ctx.sets_dirty[COMPUTE] = true;
        }
    }

    /// Binds unordered access views to `stage` (pixel or compute) from `start`, setting the hidden
    /// counters of those given an initial count (`u32::MAX` keeps a counter as it is).
    pub fn set_uavs(
        &mut self,
        stage: usize,
        start: u32,
        views: &[Option<Rc<ShaderView<B>>>],
        counts: Option<&[u32]>,
    ) -> Result<()> {
        if put_rc(&mut self.ctx.uavs[stage], start, views)? {
            self.ctx.sets_dirty[stage] = true;
            // What a pipeline writes it no longer reads: the pixel stage's views are bound with the
            // targets, for every stage.
            if stage == COMPUTE {
                self.unbind_written(&[COMPUTE]);
            } else {
                self.unbind_written(&[VERTEX, PIXEL, COMPUTE]);
            }
        }
        for (i, v) in views.iter().enumerate() {
            let count = counts.and_then(|c| c.get(i)).copied().unwrap_or(u32::MAX);
            if let (Some(ShaderView::Buffer(span)), true) = (v.as_deref(), count != u32::MAX)
                && let Some(counter) = &span.counter
            {
                self.end_pass();
                self.backend.write_buffer(
                    &counter.handle,
                    0,
                    &count.to_le_bytes(),
                    WriteMode::Ordered,
                );
            }
        }
        Ok(())
    }

    /// Whether a view is bound to unordered access slot `slot` of `stage`.
    pub fn uav_bound(&self, stage: usize, slot: u32) -> bool {
        self.ctx.uavs[stage]
            .get(slot as usize)
            .is_some_and(Option::is_some)
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
        self.set_constant_ranges(stage, start, b, &vec![None; b.len()])
    }

    /// Binds constant buffers from `start`, each with its range of 16-byte constants (first,
    /// count; none for the whole buffer), as Direct3D 11.1 binds them.
    pub fn set_constant_ranges(
        &mut self,
        stage: usize,
        start: u32,
        b: &[Option<Rc<Buffer<B>>>],
        ranges: &[Option<(u32, u32)>],
    ) -> Result<()> {
        for r in ranges.iter().flatten() {
            if r.0 % 16 != 0 || r.1 % 16 != 0 || r.1 > 4096 {
                return Err(Error::InvalidArg(
                    "a constant range not in multiples of 16 constants, or past 4096",
                ));
            }
        }
        let changed = put_rc(&mut self.ctx.constant_buffers[stage], start, b)?;
        let slots = &mut self.ctx.constant_ranges[stage][start as usize..][..ranges.len()];
        if changed || slots != ranges {
            slots.copy_from_slice(ranges);
            self.ctx.sets_dirty[stage] = true;
        }
        Ok(())
    }

    pub fn set_shader_resources(
        &mut self,
        stage: usize,
        start: u32,
        v: &[Option<Rc<ShaderView<B>>>],
    ) -> Result<()> {
        // A view of a subresource the pipeline writes is bound as none, as Direct3D binds it.
        let v: Vec<_> = v
            .iter()
            .map(|v| v.clone().filter(|v| !self.written(stage, v)))
            .collect();
        if put_rc(&mut self.ctx.resources[stage], start, &v)? {
            self.ctx.sets_dirty[stage] = true;
        }
        Ok(())
    }

    /// Whether a shader resource view is bound to `slot` of `stage`.
    pub fn shader_resource_bound(&self, stage: usize, slot: u32) -> bool {
        self.ctx.resources[stage]
            .get(slot as usize)
            .is_some_and(Option::is_some)
    }

    /// Whether `stage`'s pipeline writes a subresource of `view`: through a target, or an unordered
    /// access view of the pixel stage (graphics stages) or of the compute stage.
    fn written(&self, stage: usize, view: &ShaderView<B>) -> bool {
        let writer = if stage == COMPUTE { COMPUTE } else { PIXEL };
        view.texture().is_some_and(|t| self.bound_as_target(t))
            || self.ctx.uavs[writer]
                .iter()
                .flatten()
                .any(|u| u.overlaps(view))
    }

    /// Unbinds the shader resource views of `stages` that name what their pipeline writes.
    fn unbind_written(&mut self, stages: &[usize]) {
        for &stage in stages {
            let mut resources = std::mem::take(&mut self.ctx.resources[stage]);
            for slot in &mut resources {
                if slot.as_ref().is_some_and(|v| self.written(stage, v)) {
                    *slot = None;
                    self.ctx.sets_dirty[stage] = true;
                }
            }
            self.ctx.resources[stage] = resources;
        }
    }

    fn bound_as_target(&self, view: &View<B>) -> bool {
        self.ctx
            .targets
            .iter()
            .chain([&self.ctx.depth])
            .flatten()
            .any(|t| t.overlaps(view))
    }

    pub fn set_samplers(
        &mut self,
        stage: usize,
        start: u32,
        s: &[Option<Rc<Sampler<B>>>],
    ) -> Result<()> {
        if put_rc(&mut self.ctx.samplers[stage], start, s)? {
            self.ctx.sets_dirty[stage] = true;
        }
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
            // Binding a subresource as a target unbinds its shader resource views, as in Direct3D.
            self.unbind_written(&[VERTEX, PIXEL, COMPUTE]);
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

    /// More than Direct3D's 16 viewports is invalid and dropped.
    pub fn rs_set_viewports(&mut self, viewports: &[Viewport]) {
        if viewports.len() <= VIEWPORTS {
            self.ctx.viewports = viewports.to_vec();
        }
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
            self.close_stretch(false);
            self.backend.end_pass();
            self.ctx.pass_open = false;
            self.queries.pass_set = None;
        }
    }

    pub(crate) fn ctx_pass_open(&self) -> bool {
        self.ctx.pass_open
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
                let _ = self.gpu::<()>(Err(e));
            }
        }
    }

    /// Opens a pass on the bound targets, taking pending clears of them as its loads.
    fn open_pass(&mut self) {
        if self.ctx.pass_open {
            return;
        }
        // A clear becomes a load only where the attachment is all the view sees.
        let bound = |v: &Rc<View<B>>, ctx: &ContextState<B>| {
            v.desc.layer_count == 1
                && (ctx.targets.iter().flatten().any(|t| Rc::ptr_eq(t, v))
                    || ctx.depth.as_ref().is_some_and(|d| Rc::ptr_eq(d, v)))
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
                depth_slice: if v.texture.desc.dimension == TexDim::D3 {
                    v.desc.first_layer
                } else {
                    0
                },
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
        let occlusion = match self.pass_occlusion() {
            Ok(set) => set,
            Err(e) => {
                self.lose(e);
                None
            }
        };
        self.backend.begin_pass(&PassDesc {
            colors,
            depth: depth_attachment,
            empty: None,
            occlusion: occlusion.as_ref().map(|s| &s.handle),
        });
        self.ctx.pass_open = true;
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
        });
        let program = self.gpu(program)?;
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
        let gpu = self.backend.create_vertex_layout(&desc);
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        self.layouts.insert(
            key,
            Kept::new(gpu.clone(), id, Object::VertexLayout, &self.graveyard),
        );
        Ok(gpu)
    }

    /// Grows each constant buffer a stage whose binding set changed reads that is smaller than its
    /// shader declares (`ConstantBufferGrown`). A grown buffer is a new back-end buffer, so every set
    /// is made again.
    fn grow_constants(&mut self) -> Result<()> {
        let mut small = Vec::new();
        for stage in [VERTEX, PIXEL, COMPUTE] {
            let shader = self.stage_shader(stage);
            let Some(shader) = shader.as_ref().filter(|_| self.ctx.sets_dirty[stage]) else {
                continue;
            };
            for b in &shader.reflection.bindings {
                let bound = self.ctx.constant_buffers[stage].get(b.register as usize);
                let whole = self.ctx.constant_ranges[stage]
                    .get(b.register as usize)
                    .is_some_and(Option::is_none);
                if let (SlotKind::Uniform, Some(Some(c))) = (b.kind, bound)
                    && whole
                    && b.set == set_of(stage)
                    && let Some(&(_, declared)) =
                        shader.constant_sizes.iter().find(|(n, _)| *n == b.binding)
                    && declared > c.size.get()
                {
                    small.push((c.clone(), declared));
                }
            }
        }
        for (c, declared) in &small {
            if *declared > c.size.get() {
                self.grow(c, *declared)?;
            }
        }
        if !small.is_empty() {
            self.ctx.sets_dirty = [true; STAGES];
        }
        Ok(())
    }

    /// Moves a buffer into a back-end buffer of `size` bytes whose bytes past its own are zero.
    fn grow(&mut self, b: &Buffer<B>, size: u64) -> Result<()> {
        self.backend.slow_paths().hit(SlowPath::ConstantBufferGrown);
        self.settle();
        let old = b.size.get();
        let new = self.backend.create_buffer(&gpu_buffer(&b.desc, size));
        let new = self.gpu(new)?;
        self.backend.clear_buffer(&new, old, size - old, &[0]);
        self.backend.copy_buffer(&b.handle(), 0, &new, 0, old);
        let id = self.id();
        *b.gpu.borrow_mut() = Kept::new(new, id, Object::Buffer, &self.graveyard);
        b.size.set(size);
        b.grown.set(true);
        let mut shadow = b.shadow.borrow_mut();
        if !shadow.is_empty() {
            shadow.resize(size as usize, 0);
        }
        Ok(())
    }

    /// The shader bound to `stage`.
    fn stage_shader(&self, stage: usize) -> &Option<Rc<Shader<B>>> {
        match stage {
            VERTEX => &self.ctx.vs,
            PIXEL => &self.ctx.ps,
            _ => &self.ctx.cs,
        }
    }

    /// `Dispatch` and `DispatchIndirect`; one this front cannot serve removes the device.
    pub fn dispatch(&mut self, groups: [u32; 3], indirect: Option<(&Rc<Buffer<B>>, u32)>) {
        if let Err(e) = self.try_dispatch(groups, indirect) {
            self.lose(e);
        }
    }

    fn try_dispatch(
        &mut self,
        groups: [u32; 3],
        indirect: Option<(&Rc<Buffer<B>>, u32)>,
    ) -> Result<()> {
        if self.removed.is_some() {
            return Ok(());
        }
        self.bury();
        // Without a compute shader Direct3D runs nothing.
        let Some(cs) = self.ctx.cs.clone() else {
            return Ok(());
        };
        self.settle();
        self.grow_constants()?;
        self.update_set(COMPUTE)?;
        let key = (cs.gpu.id, 0);
        let program = match self.programs.get(&key) {
            Some(p) => p.handle.clone(),
            None => {
                let p = self
                    .backend
                    .create_program(&Program::Compute(&cs.gpu.handle));
                let p = self.gpu(p)?;
                let id = self.id();
                self.programs.insert(
                    key,
                    Kept::new(p.clone(), id, Object::Program, &self.graveyard),
                );
                p
            }
        };
        let args = indirect.map(|(b, offset)| (b.gpu.borrow(), offset));
        let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
        bindings[0] = self.ctx.sets[COMPUTE].as_ref().map(|k| &k.handle);
        let call = DispatchCall {
            program: &program,
            bindings,
            immediates: &[],
            groups: match &args {
                None => DispatchGroups::Direct(groups),
                Some((b, offset)) => DispatchGroups::Indirect {
                    args: &b.handle,
                    offset: u64::from(*offset),
                },
            },
        };
        let r = self.backend.dispatch(&call);
        drop(args);
        self.gpu(r)
    }

    /// `CopyStructureCount`: the hidden counter of a buffer view, written into `dst` at `offset`.
    pub fn copy_structure_count(
        &mut self,
        dst: &Buffer<B>,
        offset: u32,
        view: &ShaderView<B>,
    ) -> Result<()> {
        let ShaderView::Buffer(BufferSpan {
            counter: Some(c), ..
        }) = view
        else {
            return Err(Error::InvalidArg("a view without a counter"));
        };
        if !offset.is_multiple_of(4) || u64::from(offset) + 4 > u64::from(dst.desc.byte_width) {
            return Err(Error::InvalidArg("a count written past the buffer"));
        }
        self.settle();
        self.backend
            .copy_buffer(&c.handle, 0, &dst.handle(), u64::from(offset), 4);
        Ok(())
    }

    /// `ClearUnorderedAccessViewUint` (`float` false) and `Float`: buffer views take the first value
    /// for each 32-bit word; texture views each texel, as raw bits of the view's format.
    pub fn clear_uav(&mut self, view: &ShaderView<B>, values: [u32; 4], float: bool) -> Result<()> {
        self.settle();
        match view {
            ShaderView::Buffer(span) => {
                if float {
                    return not_implemented("a float clear of a buffer view");
                }
                self.backend.clear_buffer(
                    &span.buffer.handle(),
                    span.offset,
                    span.size,
                    &values[0].to_le_bytes(),
                );
                Ok(())
            }
            ShaderView::Texture(v) => {
                if !v.texture.gpu_desc.usage.contains(TextureUsage::RENDER) {
                    return not_implemented(
                        "a clear of an unordered access view of a format no pass draws into",
                    );
                }
                let integer = matches!(v.format.kind(), FormatKind::Uint | FormatKind::Sint);
                if integer == float {
                    return not_implemented(
                        "a clear of an unordered access view converting between float and integer",
                    );
                }
                let r = self.backend.clear_texture(
                    &v.texture.gpu.handle,
                    &v.texture.gpu_desc,
                    v.format,
                    v.desc.first_mip..v.desc.first_mip + 1,
                    v.desc.first_layer..v.desc.first_layer + v.desc.layer_count,
                    ClearValue::Color(values),
                );
                self.gpu(r)
            }
        }
    }

    /// Makes the binding set of `stage` from its slots and its shader's reflection, when it changed.
    fn update_set(&mut self, stage: usize) -> Result<()> {
        if !self.ctx.sets_dirty[stage] {
            return Ok(());
        }
        let set = set_of(stage);
        let shader = self.stage_shader(stage).clone();
        let Some(shader) = shader.filter(|s| s.reflection.bindings.iter().any(|b| b.set == set))
        else {
            self.ctx.sets[stage] = None;
            self.ctx.sets_dirty[stage] = false;
            return Ok(());
        };
        let wanted = || {
            shader
                .reflection
                .bindings
                .iter()
                .filter(move |b| b.set == set)
        };
        let stages = match stage {
            VERTEX => StageSet::VERTEX,
            PIXEL => StageSet::FRAGMENT,
            _ => StageSet::COMPUTE,
        };
        let stand_in = |n| {
            shader
                .stand_ins
                .iter()
                .find(|(b, _)| *b == n)
                .and_then(|s| s.1)
        };
        // Whether a shader resource binding reads a structured or raw buffer, not a texture.
        let reads_buffer = |n| matches!(stand_in(n), Some(StandIn::ZeroBuffer));
        // What stands in for each empty slot the shader reads, by binding number.
        let mut empty: Vec<(u32, Zero<B>)> = Vec::new();
        for b in wanted() {
            let slot = b.register as usize;
            let filled = match b.kind {
                SlotKind::Uniform => self.ctx.constant_buffers[stage]
                    .get(slot)
                    .is_some_and(Option::is_some),
                // A view of the other kind (a texture where a buffer is read) reads as none.
                SlotKind::Texture => {
                    match self.ctx.resources[stage]
                        .get(slot)
                        .and_then(Option::as_deref)
                    {
                        Some(ShaderView::Texture(_)) => !reads_buffer(b.binding),
                        Some(ShaderView::Buffer(_)) => reads_buffer(b.binding),
                        None => false,
                    }
                }
                SlotKind::Sampler => self.ctx.samplers[stage]
                    .get(slot)
                    .is_some_and(Option::is_some),
                SlotKind::Storage => match self.ctx.uavs[stage].get(slot) {
                    Some(Some(_)) => true,
                    _ => return not_implemented("an empty unordered access slot a shader uses"),
                },
                SlotKind::Counter => match self.ctx.uavs[stage].get(slot) {
                    Some(Some(u))
                        if matches!(
                            &**u,
                            ShaderView::Buffer(BufferSpan {
                                counter: Some(_),
                                ..
                            })
                        ) =>
                    {
                        true
                    }
                    _ => {
                        return Err(Error::InvalidArg(
                            "a shader counts with a view that has no counter",
                        ));
                    }
                },
            };
            if filled {
                continue;
            }
            let zero = match (b.kind, stand_in(b.binding)) {
                (SlotKind::Uniform, _) => {
                    let declared = shader
                        .constant_sizes
                        .iter()
                        .find(|(n, _)| *n == b.binding)
                        .map_or(16, |s| s.1);
                    Zero::Buffer(self.zero_buffer()?, declared)
                }
                (_, Some(StandIn::Zeros(dimension, kind))) => {
                    Zero::View(self.zero_view(dimension, kind)?)
                }
                (_, Some(StandIn::DefaultSampler)) => Zero::Sampler(self.default_sampler()?),
                (_, Some(StandIn::ZeroBuffer)) => Zero::Storage(self.zero_storage()?),
                (_, None) => {
                    return not_implemented(
                        "an empty slot read as a multisampled or depth texture, or a comparison sampler",
                    );
                }
            };
            empty.push((b.binding, zero));
        }
        let mut cbs = Vec::new();
        for (c, range) in self.ctx.constant_buffers[stage]
            .iter()
            .zip(&self.ctx.constant_ranges[stage])
        {
            let Some(c) = c else {
                cbs.push(None);
                continue;
            };
            let (offset, size) = match *range {
                None => (0, c.size.get()),
                // A range past the buffer's end is cut at it.
                Some((first, count)) => {
                    let offset = u64::from(first) * 16;
                    let size = (u64::from(count) * 16).min(c.size.get().saturating_sub(offset));
                    (offset, size)
                }
            };
            cbs.push(Some((c.gpu.borrow(), offset, size)));
        }
        for b in wanted().filter(|b| b.kind == SlotKind::Uniform) {
            let declared = shader
                .constant_sizes
                .iter()
                .find(|(n, _)| *n == b.binding)
                .map_or(0, |s| s.1);
            if let Some(Some((_, _, size))) = cbs.get(b.register as usize)
                && self.ctx.constant_ranges[stage][b.register as usize].is_some()
                && *size < declared
            {
                return not_implemented(
                    "a constant buffer range shorter than its shader declares, or empty",
                );
            }
        }
        // The back-end buffers of the buffer views bound for unordered access.
        let spans: Vec<_> = self.ctx.uavs[stage]
            .iter()
            .map(|u| match u.as_deref() {
                Some(ShaderView::Buffer(span)) => Some((span.buffer.gpu.borrow(), span)),
                _ => None,
            })
            .collect();
        let srvs: Vec<_> = self.ctx.resources[stage]
            .iter()
            .map(|v| match v.as_deref() {
                Some(ShaderView::Buffer(span)) => Some((span.buffer.gpu.borrow(), span)),
                _ => None,
            })
            .collect();
        let uavs = &self.ctx.uavs[stage];
        let views = &self.ctx.resources[stage];
        let samplers = &self.ctx.samplers[stage];
        let mut entries = Vec::new();
        for b in wanted() {
            let slot = b.register as usize;
            let resource = match b.kind {
                SlotKind::Uniform => {
                    cbs.get(slot)
                        .and_then(Option::as_ref)
                        .map(|(c, offset, size)| {
                            BindingResource::UniformBuffer(BufferRange {
                                buffer: &c.handle,
                                offset: *offset,
                                size: *size,
                            })
                        })
                }
                SlotKind::Texture => match (
                    views.get(slot).and_then(Option::as_deref),
                    srvs.get(slot).and_then(Option::as_ref),
                ) {
                    (Some(ShaderView::Texture(v)), _) if !reads_buffer(b.binding) => {
                        Some(BindingResource::Texture(&v.gpu.handle))
                    }
                    (_, Some((buffer, span))) if reads_buffer(b.binding) => {
                        Some(BindingResource::StorageBuffer {
                            range: BufferRange {
                                buffer: &buffer.handle,
                                offset: span.offset,
                                size: span.size,
                            },
                            writable: false,
                        })
                    }
                    _ => None,
                },
                SlotKind::Sampler => samplers
                    .get(slot)
                    .and_then(Option::as_ref)
                    .map(|s| BindingResource::Sampler(&s.gpu.handle)),
                SlotKind::Storage => match (uavs[slot].as_deref(), &spans[slot]) {
                    (Some(ShaderView::Texture(v)), _) => Some(BindingResource::StorageTexture {
                        view: &v.gpu.handle,
                        writable: true,
                    }),
                    (_, Some((buffer, span))) => Some(BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer: &buffer.handle,
                            offset: span.offset,
                            size: span.size,
                        },
                        writable: true,
                    }),
                    _ => None,
                },
                SlotKind::Counter => match &spans[slot] {
                    Some((
                        _,
                        BufferSpan {
                            counter: Some(c), ..
                        },
                    )) => Some(BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer: &c.handle,
                            offset: 0,
                            size: 4,
                        },
                        writable: true,
                    }),
                    _ => None,
                },
            };
            let resource = match resource {
                Some(r) => r,
                None => match &empty
                    .iter()
                    .find(|(n, _)| *n == b.binding)
                    .expect("a stand-in")
                    .1
                {
                    Zero::Buffer(buffer, size) => BindingResource::UniformBuffer(BufferRange {
                        buffer,
                        offset: 0,
                        size: *size,
                    }),
                    Zero::View(view) => BindingResource::Texture(view),
                    Zero::Storage(buffer) => BindingResource::StorageBuffer {
                        range: BufferRange {
                            buffer,
                            offset: 0,
                            size: ZERO_BYTES,
                        },
                        writable: false,
                    },
                    Zero::Sampler(sampler) => BindingResource::Sampler(sampler),
                },
            };
            entries.push(BindingEntry {
                binding: b.binding,
                stages,
                resource,
            });
        }
        let set = self.backend.create_binding_set(&entries);
        drop(entries);
        drop(cbs);
        drop(spans);
        drop(srvs);
        let set = self.gpu(set)?;
        let id = self.id();
        self.ctx.sets[stage] = Some(Kept::new(set, id, Object::BindingSet, &self.graveyard));
        self.ctx.sets_dirty[stage] = false;
        Ok(())
    }

    fn try_draw(&mut self, range: Range, first_instance: u32, instances: u32) -> Result<()> {
        if self.removed.is_some() {
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
        // The common form's vertex and instance indices include the draw's bases, which SV_VertexID
        // and SV_InstanceID leave out; the helper constants that take them off are not served yet.
        let vertex_base = match range {
            Range::Vertices { start, .. } => start != 0,
            Range::Indexed { base, .. } => base != 0,
        };
        let reads = |sv| vs.reflection.inputs.iter().any(|i| i.system == Some(sv));
        if (vertex_base && reads(SystemValue::VertexId))
            || (first_instance != 0 && reads(SystemValue::InstanceId))
        {
            return not_implemented(
                "SV_VertexID or SV_InstanceID with a non-zero start or base location",
            );
        }
        // The converter writes no program whose second output feeds the blend yet.
        if self
            .ctx
            .blend
            .as_ref()
            .is_some_and(|b| b.desc.dual_source())
        {
            return not_implemented("dual-source blending");
        }
        let ps = self.ctx.ps.clone();
        let program = self.program(&vs, ps.as_ref())?;
        let vertex_layout = self.vertex_layout()?;
        // Direct3D reads zeros from a slot the layout reads that has no buffer: the zero buffer, at
        // stride zero.
        let empty_streams = self.ctx.layout.as_ref().is_some_and(|l| {
            l.elements
                .iter()
                .any(|e| self.ctx.vertex_buffers[e.slot as usize].is_none())
        });
        let zero_stream = if empty_streams {
            Some(self.zero_buffer()?)
        } else {
            None
        };
        self.grow_constants()?;
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
                );
                let ring = self.gpu(ring)?;
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
        // Direct3D draws lines by the quadrilateral rule when the state asks for multisampling and
        // alpha-antialiased when it asks for antialiased lines; the back ends draw aliased lines.
        let raster = &self
            .ctx
            .raster
            .as_ref()
            .unwrap_or(&self.defaults.raster)
            .desc;
        if matches!(topology, Topology::LineList | Topology::LineStrip)
            && (raster.multisample || raster.antialiased_lines)
        {
            return not_implemented("lines drawn by the quadrilateral or alpha-antialiased rule");
        }
        let nothing = matches!(
            gpu_range,
            DrawRange::Indexed { count: 0, .. } | DrawRange::Vertices { count: 0, .. }
        );
        if nothing || self.ctx.viewports.is_empty() || (indexed && self.ctx.index.is_none()) {
            return Ok(());
        }
        self.open_pass();
        if !self.open_stretch() {
            // The pass's occlusion set has no slot left: the draw opens the next pass on another.
            self.end_pass();
            self.open_pass();
            self.open_stretch();
        }

        let ctx = &self.ctx;
        // Fixed arrays: no allocation per draw (Direct3D has 32 vertex slots and 16 viewports).
        let vertex_handles: [_; VERTEX_SLOTS] = std::array::from_fn(|slot| {
            ctx.vertex_buffers[slot]
                .as_ref()
                .map(|(b, _, offset)| (b.gpu.borrow(), *offset))
        });
        let read = |slot: usize| {
            ctx.layout
                .as_ref()
                .is_some_and(|l| l.elements.iter().any(|e| e.slot as usize == slot))
        };
        let vertex_buffers: [Option<VertexBinding<'_, B>>; VERTEX_SLOTS] =
            std::array::from_fn(|slot| match (&vertex_handles[slot], &zero_stream) {
                (Some((b, offset)), _) => Some(VertexBinding {
                    buffer: &b.handle,
                    offset: u64::from(*offset),
                }),
                (None, Some(zero)) if read(slot) => Some(VertexBinding {
                    buffer: zero,
                    offset: 0,
                }),
                _ => None,
            });
        let index_handle = ctx.index.as_ref().map(|(b, _, _)| b.gpu.borrow());
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
            (None, Some((_, f, offset))) if indexed => Some(IndexBinding {
                buffer: &index_handle.as_ref().expect("an index buffer").handle,
                offset: u64::from(*offset),
                format: *f,
            }),
            _ => None,
        };
        let viewport_count = ctx.viewports.len().min(VIEWPORTS);
        let viewports: [GpuViewport; VIEWPORTS] = std::array::from_fn(|i| {
            let v = ctx.viewports.get(i).copied().unwrap_or_default();
            GpuViewport {
                x: v.x.to_bits(),
                y: v.y.to_bits(),
                width: v.width.to_bits(),
                height: v.height.to_bits(),
                min_depth: v.min_depth.to_bits(),
                max_depth: v.max_depth.to_bits(),
            }
        });
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
            viewports: &viewports[..viewport_count],
            scissors: &ctx.scissors,
            blend_constant: ctx.blend_factor.map(f32::to_bits),
            stencil_reference: [ctx.stencil_ref as u8; 2],
            immediates: &[],
            range: gpu_range,
        };
        self.backend.draw(&call);
        Ok(())
    }

    /// `CopyResource`: two resources of one shape.
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
                    &s.handle(),
                    0,
                    &d.handle(),
                    0,
                    u64::from(s.desc.byte_width),
                );
                self.buffer_written(d)
            }
            (Resource::Texture(d), Resource::Texture(s)) => {
                let (a, b) = (&d.desc, &s.desc);
                let shape = |t: &TextureDesc| {
                    (
                        t.dimension,
                        t.width,
                        t.height,
                        t.depth,
                        t.mip_levels,
                        t.array_size,
                    )
                };
                if shape(a) != shape(b) || Rc::ptr_eq(d, s) {
                    return Err(Error::InvalidArg(
                        "CopyResource needs two textures of one shape",
                    ));
                }
                copy_formats(a.format, b.format)?;
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
                if to <= from
                    || to > s.desc.byte_width
                    || at[0] + (to - from) > d.desc.byte_width
                    || src_box.is_some_and(Box3::empty)
                {
                    return Ok(());
                }
                self.settle();
                self.backend.copy_buffer(
                    &s.handle(),
                    u64::from(from),
                    &d.handle(),
                    u64::from(at[0]),
                    u64::from(to - from),
                );
                self.buffer_written(d)
            }
            (Resource::Texture(d), Resource::Texture(s)) => {
                copy_formats(d.desc.format, s.desc.format)?;
                let (Some((src_mip, src_layer)), Some((dst_mip, dst_layer))) =
                    (s.subresource(src_sub), d.subresource(dst_sub))
                else {
                    return Err(Error::InvalidArg("subresource past the resource"));
                };
                let se = s.mip_extent(src_mip);
                let b = src_box.copied().unwrap_or(Box3 {
                    left: 0,
                    top: 0,
                    front: 0,
                    right: se.width,
                    bottom: se.height,
                    back: se.depth_or_layers,
                });
                let de = d.mip_extent(dst_mip);
                if b.right <= b.left
                    || b.bottom <= b.top
                    || b.back <= b.front
                    || b.right > se.width
                    || b.bottom > se.height
                    || b.back > se.depth_or_layers
                {
                    return Ok(());
                }
                let extent = Extent {
                    width: b.right - b.left,
                    height: b.bottom - b.top,
                    depth_or_layers: b.back - b.front,
                };
                if at[0] + extent.width > de.width
                    || at[1] + extent.height > de.height
                    || at[2] + extent.depth_or_layers > de.depth_or_layers
                {
                    return Ok(());
                }
                // Depth-stencil subresources are copied whole; Direct3D drops a part.
                if s.format.kind() == FormatKind::DepthStencil
                    && (at != [0; 3]
                        || (extent.width, extent.height) != (se.width, se.height)
                        || (de.width, de.height) != (se.width, se.height))
                {
                    return Ok(());
                }
                self.settle();
                let (x, y, z) = (b.left, b.top, b.front);
                self.backend.copy_texture(
                    &s.region(src_mip, src_layer, Origin { x, y, z }),
                    &d.region(
                        dst_mip,
                        dst_layer,
                        Origin {
                            x: at[0],
                            y: at[1],
                            z: at[2],
                        },
                    ),
                    extent,
                );
                self.texture_written(d, dst_sub)
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
        slice_pitch: u32,
    ) -> Result<()> {
        match dst {
            Resource::Buffer(b) => {
                if b.desc.usage != usage::DEFAULT {
                    return Err(Error::InvalidArg("UpdateSubresource needs default usage"));
                }
                let (from, to) = dst_box.map_or((0, b.desc.byte_width), |x| (x.left, x.right));
                if to <= from || to > b.desc.byte_width || dst_box.is_some_and(Box3::empty) {
                    return Ok(());
                }
                let bytes = &bytes[..(to - from) as usize];
                if from == 0 && to == b.desc.byte_width {
                    // A whole write discards; a grown buffer's bytes past the program's stay zero.
                    if b.grown.get() {
                        let mut padded = bytes.to_vec();
                        padded.resize(b.size.get() as usize, 0);
                        self.backend
                            .write_buffer(&b.handle(), 0, &padded, WriteMode::Discard);
                    } else {
                        self.backend
                            .write_buffer(&b.handle(), 0, bytes, WriteMode::Discard);
                    }
                } else {
                    self.backend.write_buffer(
                        &b.handle(),
                        u64::from(from),
                        bytes,
                        WriteMode::Ordered,
                    );
                }
                Ok(())
            }
            Resource::Texture(t) => {
                if t.desc.usage != usage::DEFAULT {
                    return Err(Error::InvalidArg("UpdateSubresource needs default usage"));
                }
                let Some((mip, layer)) = t.subresource(sub) else {
                    return Err(Error::InvalidArg("subresource past the resource"));
                };
                let e = t.mip_extent(mip);
                let b = dst_box.copied().unwrap_or(Box3 {
                    right: e.width,
                    bottom: e.height,
                    back: e.depth_or_layers,
                    ..Default::default()
                });
                if b.right <= b.left
                    || b.bottom <= b.top
                    || b.back <= b.front
                    || b.right > e.width
                    || b.bottom > e.height
                    || b.back > e.depth_or_layers
                {
                    return Ok(());
                }
                self.settle();
                let origin = Origin {
                    x: b.left,
                    y: b.top,
                    z: b.front,
                };
                self.write_texels(
                    t.format,
                    &t.region(mip, layer, origin),
                    Extent {
                        width: b.right - b.left,
                        height: b.bottom - b.top,
                        depth_or_layers: b.back - b.front,
                    },
                    &SubresourceData {
                        bytes,
                        row_pitch,
                        slice_pitch,
                    },
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
        flags: u32,
    ) -> Result<Mapped> {
        self.live()?;
        let reads = matches!(kind, map::READ | map::READ_WRITE);
        let writes = kind != map::READ;
        let wait = flags & map::DO_NOT_WAIT == 0;
        let mapped = self.map_any(resource, sub, kind, reads, writes, wait);
        if let Err(e @ Error::DeviceRemoved) = mapped {
            self.lose(e);
        }
        // A read back the back end failed is a loss it reports through its status.
        self.live()?;
        mapped
    }

    fn map_any(
        &mut self,
        resource: &Resource<B>,
        sub: u32,
        kind: u32,
        reads: bool,
        writes: bool,
        wait: bool,
    ) -> Result<Mapped> {
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
                    let handle = b.handle();
                    let src = ReadSource::Buffer {
                        buffer: &handle,
                        offset: 0,
                        size: u64::from(d.byte_width),
                    };
                    let mut staged = b.staged.borrow_mut();
                    self.refresh(&mut staged, &src, &mut b.shadow.borrow_mut(), reads, wait)?;
                }
                self.ctx.mapped.push((Rc::as_ptr(b) as usize, 0, kind));
                let data = b.shadow.borrow_mut().as_mut_ptr();
                Ok(Mapped {
                    data,
                    row_pitch: d.byte_width,
                    depth_pitch: d.byte_width,
                })
            }
            Resource::Texture(t) => {
                let d = &t.desc;
                let ok = match d.usage {
                    usage::STAGING => {
                        (!reads || d.cpu_access_flags & desc::cpu::READ != 0)
                            && (!writes || d.cpu_access_flags & desc::cpu::WRITE != 0)
                            && matches!(kind, map::READ | map::WRITE | map::READ_WRITE)
                    }
                    usage::DYNAMIC => kind == map::WRITE_DISCARD,
                    _ => false,
                };
                let whole = t.whole(sub).filter(|_| ok);
                let Some((region, extent)) = whole else {
                    return Err(Error::InvalidArg("the resource cannot be mapped this way"));
                };
                let (row, rows, _) = t.packed(region.mip);
                let src = ReadSource::Texture { region, extent };
                let mut shadow = t.shadow.borrow_mut();
                let mut staged = t.staged.borrow_mut();
                let s = sub as usize;
                // A dynamic texture's shadow is the program's to fill; nothing is read back.
                if d.usage == usage::STAGING {
                    self.refresh(&mut staged[s], &src, &mut shadow[s], reads, wait)?;
                }
                self.ctx.mapped.push((Rc::as_ptr(t) as usize, sub, kind));
                Ok(Mapped {
                    data: shadow[sub as usize].as_mut_ptr(),
                    row_pitch: row,
                    depth_pitch: row * rows,
                })
            }
        }
    }

    /// After a copy into `b`: a staging buffer the program can read starts its read back now (plan
    /// 4.6), so a map frames later finds it ready.
    fn buffer_written(&mut self, b: &Buffer<B>) -> Result<()> {
        if b.desc.usage != usage::STAGING {
            return Ok(());
        }
        let state = if b.desc.cpu_access_flags & desc::cpu::READ != 0 {
            let handle = b.handle();
            let r = self.backend.read_back(&ReadSource::Buffer {
                buffer: &handle,
                offset: 0,
                size: u64::from(b.desc.byte_width),
            });
            Staged::Reading(self.gpu(r)?)
        } else {
            Staged::Written
        };
        *b.staged.borrow_mut() = state;
        Ok(())
    }

    /// After a copy into subresource `sub` of `t`, as [`Self::buffer_written`].
    fn texture_written(&mut self, t: &Texture<B>, sub: u32) -> Result<()> {
        if t.desc.usage != usage::STAGING {
            return Ok(());
        }
        let state = if t.desc.cpu_access_flags & desc::cpu::READ != 0 {
            let (region, extent) = t.whole(sub).expect("a subresource the copy checked");
            let r = self
                .backend
                .read_back(&ReadSource::Texture { region, extent });
            Staged::Reading(self.gpu(r)?)
        } else {
            Staged::Written
        };
        t.staged.borrow_mut()[sub as usize] = state;
        Ok(())
    }

    /// Brings a staging subresource's shadow up to the GPU's bytes before a map hands it out: the
    /// read back started at the copy, or one started now; a write-only map that needs it is
    /// `StagingRefreshed`. Without `wait`, a read back not yet done answers `StillDrawing` (after
    /// submitting, so it gets done).
    fn refresh(
        &mut self,
        staged: &mut Staged<B>,
        src: &ReadSource<'_, B>,
        shadow: &mut [u8],
        reads: bool,
        wait: bool,
    ) -> Result<()> {
        let rb = match std::mem::replace(staged, Staged::Current) {
            Staged::Current => return Ok(()),
            Staged::Written => {
                self.settle();
                let r = self.backend.read_back(src);
                self.gpu(r)?
            }
            Staged::Reading(rb) => rb,
        };
        if self.backend.read_state(&rb) == ReadState::Pending {
            if !wait {
                *staged = Staged::Reading(rb);
                self.flush();
                return Err(Error::StillDrawing);
            }
            self.backend.slow_paths().hit(SlowPath::ReadBackWait);
            self.backend.wait_read(&rb);
        }
        if !reads {
            self.backend.slow_paths().hit(SlowPath::StagingRefreshed);
        }
        self.backend.finish_read(rb, shadow);
        Ok(())
    }

    pub fn unmap(&mut self, resource: &Resource<B>, sub: u32) {
        let address = match resource {
            Resource::Buffer(b) => Rc::as_ptr(b) as usize,
            Resource::Texture(t) => Rc::as_ptr(t) as usize,
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
                    .write_buffer(&b.handle(), 0, &b.shadow.borrow(), mode);
            }
            Resource::Texture(t) => {
                let (region, extent) = t.whole(sub).expect("a subresource the map checked");
                let (row, rows, _) = t.packed(region.mip);
                self.end_pass();
                self.backend.write_texture(
                    &region,
                    extent,
                    &t.shadow.borrow()[sub as usize],
                    row,
                    rows,
                    WriteMode::Ordered,
                );
            }
        }
    }

    /// `GenerateMips`: fills the view's levels below its first from the first, in its format. A
    /// texture made without mip generation is left alone, as Direct3D leaves it.
    pub fn generate_mips(&mut self, view: &View<B>) {
        let t = &view.texture;
        if t.desc.misc_flags & desc::MISC_GENERATE_MIPS == 0 {
            return;
        }
        let v = &view.desc;
        let layers = v.first_layer..v.first_layer + v.layer_count;
        self.settle();
        let r = self.backend.generate_mips(
            &t.gpu.handle,
            &t.gpu_desc,
            view.format,
            v.first_mip + 1..v.first_mip + v.mip_count,
            layers,
        );
        if let Err(e) = r {
            let _ = self.gpu::<()>(Err(e));
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
        let fence = self.backend.submit();
        self.submitted(fence);
        self.poll_removed();
    }
}
