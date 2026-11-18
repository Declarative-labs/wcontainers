//! The immediate context: its state, passes opened at draws, draws, copies, writes and maps.

use std::cell::Cell;
use std::rc::Rc;

use wcontainers_gpu::{
    AspectOps, Backend, BindingEntry, BindingResource, BufferRange, ClearValue, ColorAttachment,
    ColorTarget, CountedDraw, DRAW_ARGUMENTS_BINDING, DepthAttachment, DispatchCall,
    DispatchGroups, DrawCall, DrawRange, EmptyPass, Extent, Features, Filter, Format, FormatKind,
    FormatUse, HELPER_SET, HelperValue, IndexBinding, IndexFormat, Interpolation, LoadOp,
    MAX_BINDING_SETS, Object, Origin, PassDesc, PassTargets, Program, ReadSource, ReadState,
    RingKind, SamplerDesc as GpuSampler, ScissorRect, ShaderModule, ShaderStage, SlotKind,
    SlowPath, SourceStage, StageSet, StepMode, StoreOp, Target, TextureUsage, Topology, VariantKey,
    VertexAttribute, VertexBinding, VertexLayoutDesc, VertexStream, Viewport as GpuViewport,
    WriteMode, helper_binding,
};

use crate::desc::{
    self, Box3, DepthStencilDesc, SubresourceData, TexDim, TextureDesc, Viewport, map, topology,
    usage,
};
use crate::device::{
    Device, HelperBlocks, Linked, Variant, ZERO_BYTES, gpu_buffer, integer_format, resolvable,
};
use crate::error::{Error, Result, not_implemented};
use crate::format;
use crate::object::*;
use crate::shader::{Helpers, StageVariant};

/// The shader stages whose slots this context keeps: vertex (set 0), pixel (set 1) and compute
/// (set 0 of its own programs).
pub const VERTEX: usize = 0;
pub const PIXEL: usize = 1;
pub const COMPUTE: usize = 2;
const STAGES: usize = 3;

/// Unordered access slots of the pixel and compute stages at feature level 11_0.
pub const UAV_SLOTS: usize = 8;

/// The binding set a stage's program reads its slots from.
/// The bytes a constant buffer window of `(first, count)` 16-byte constants covers: offset and
/// length, cut at the buffer's end.
fn window_of<B: Backend>(c: &Buffer<B>, (first, count): (u32, u32)) -> (u64, u64) {
    let offset = u64::from(first) * 16;
    let length = (u64::from(count) * 16).min(c.size.get().saturating_sub(offset));
    (offset, length)
}

/// A constant buffer window a stage reads through a copy: slot, buffer, offset, length and the
/// block's size.
type ConstantWindow<B> = (u32, Rc<Buffer<B>>, u64, u64, u64);

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
    pub(crate) layout: Option<Rc<InputLayout>>,
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
    pub(crate) targets: Vec<Option<Rc<View<B>>>>,
    pub(crate) depth: Option<Rc<View<B>>>,
    pub(crate) blend: Option<Rc<Blend<B>>>,
    blend_factor: [f32; 4],
    sample_mask: u32,
    pub(crate) depth_stencil: Option<Rc<DepthStencil<B>>>,
    stencil_ref: u32,
    pub(crate) raster: Option<Rc<Rasterizer<B>>>,
    viewports: Vec<Viewport>,
    scissors: Vec<ScissorRect>,
    pub(crate) sets: [Option<Kept<B, B::BindingSet>>; STAGES],
    sets_dirty: [bool; STAGES],
    /// The constant buffer windows each stage's set reads through copies: slot, buffer, offset,
    /// length and the block's size.
    windows: [Vec<ConstantWindow<B>>; STAGES],
    /// The input layout whose pulled streams the vertex stage's set binds (its id; zero for none).
    pulled_set: u64,
    pass_open: bool,
    /// The size of the open pass when it has no attachments.
    empty_pass: Option<EmptyPass>,
    clears: Vec<PendingClear<B>>,
    /// Mapped subresources: the resource's address, subresource, map type.
    mapped: Vec<(usize, u32, u32)>,
    /// The targets last set into the pass tracker for counted occlusion queries, by view address,
    /// and whether the counter has copied the depth its segment's draws began from.
    counted_targets: Vec<usize>,
    counted_copy: bool,
    /// The predicate draws, dispatches, clears, copies and writes are predicated on, and the value
    /// that skips them; none with the hint flag, whose operations are kept, as Direct3D allows.
    predication: Option<(Rc<crate::query::Query<B>>, bool)>,
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
            windows: Default::default(),
            pulled_set: 0,
            pass_open: false,
            empty_pass: None,
            clears: Vec::new(),
            mapped: Vec::new(),
            counted_targets: Vec::new(),
            counted_copy: false,
            predication: None,
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

/// The pixel counter's programs in the device's counting programs, by the place of a pixel
/// shader's id: one count a pixel, the same on multisampled targets (the first covered sample
/// counts), and one count a sample.
const PER_PIXEL: u64 = u64::MAX;
const PER_PIXEL_MASKED: u64 = u64::MAX - 1;
const PER_SAMPLE: u64 = u64::MAX - 2;

/// Whether topology `t` is a strip, whose indexed draws restart at a cut index.
fn strip_topology(t: u32) -> bool {
    matches!(t, topology::LINESTRIP | topology::TRIANGLESTRIP)
}

/// The primitives `count` vertices of topology `t` make (Direct3D's input assembler: incomplete
/// primitives are dropped).
fn primitives(t: u32, count: u32) -> u64 {
    let n = u64::from(count);
    match t {
        topology::POINTLIST => n,
        topology::LINELIST => n / 2,
        topology::LINESTRIP => n.saturating_sub(1),
        topology::TRIANGLELIST => n / 3,
        topology::TRIANGLESTRIP => n.saturating_sub(2),
        topology::LINELIST_ADJ => n / 4,
        topology::LINESTRIP_ADJ => n.saturating_sub(3),
        topology::TRIANGLELIST_ADJ => n / 6,
        topology::TRIANGLESTRIP_ADJ => n.saturating_sub(4) / 2,
        // Patch lists of 1 to 32 control points.
        33..=64 => n / u64::from(t - 32),
        _ => 0,
    }
}

/// Direct3D's answer to a copy between textures of DXGI formats `a` and `b`: formats of one typeless
/// group, and from feature level 10_1 its conversions between groups, copy their bits; others are
/// invalid.
fn copy_formats(a: u32, b: u32, level: u32) -> Result<()> {
    if format::typeless_group(a) == format::typeless_group(b)
        || (level >= desc::level::L10_1 && format::copy_cast(a, b))
    {
        Ok(())
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
    Vertices {
        count: u32,
        start: u32,
    },
    Indexed {
        count: u32,
        start: u32,
        base: i32,
    },
    /// The arguments are in a buffer, at a byte offset.
    Indirect {
        indexed: bool,
        offset: u32,
    },
}

/// Texels of Direct3D's layout of a format with depth and stencil, `texel` bytes each
/// (D24_UNORM_S8_UINT: depth in the low 24 bits of a word, stencil in its high byte;
/// D32_FLOAT_S8X24_UINT: the float, the stencil byte, three unused), as the back end takes them:
/// depth in four bytes per texel (24-bit depth in the low bits of a word), stencil in one.
pub(crate) fn split_depth_stencil(texel: u32, bytes: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let n = bytes.len() / texel as usize;
    let mut depth = Vec::with_capacity(n * 4);
    let mut stencil = Vec::with_capacity(n);
    for c in bytes.chunks_exact(texel as usize) {
        if texel == 4 {
            depth.extend_from_slice(&[c[0], c[1], c[2], 0]);
            stencil.push(c[3]);
        } else {
            depth.extend_from_slice(&c[..4]);
            stencil.push(c[4]);
        }
    }
    (depth, stencil)
}

/// Bytes of one indirect draw's arguments: four words, five for an indexed draw.
const fn indirect_bytes(indexed: bool) -> u32 {
    if indexed { 20 } else { 16 }
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
        // The vertex stage's set binds the streams its program fetches.
        if self.ctx.pulled_set != 0 {
            self.ctx.sets_dirty[VERTEX] = true;
        }
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
        if self.predicated_out() {
            return;
        }
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
        if self.predicated_out() {
            return;
        }
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

    /// Ends the open pass. While counted occlusion queries run, the counter then counts the draws
    /// it kept, before anything changes what they read or drew on.
    pub(crate) fn end_pass(&mut self) {
        if self.ctx.pass_open {
            self.close_stretch(false);
            self.backend.end_pass();
            self.ctx.pass_open = false;
            self.queries.pass_set = None;
        }
        if !self.queries.counted.is_empty() {
            let r = self.counter.flush(&mut self.backend, &mut self.passes);
            if let Err(e) = r {
                let _ = self.gpu::<()>(Err(e));
            }
            self.ctx.counted_copy = false;
        }
        if !self.queries.statistics.is_empty() {
            let r = self
                .pixel_counter
                .flush(&mut self.backend, &mut self.passes);
            if let Err(e) = r {
                let _ = self.gpu::<()>(Err(e));
            }
        }
    }

    /// `SetPredication`.
    pub fn set_predication(&mut self, q: Option<Rc<crate::query::Query<B>>>, value: bool) {
        self.ctx.predication = q
            .filter(|q| q.misc & crate::query::PREDICATE_HINT == 0)
            .map(|q| (q, value));
    }

    /// Whether the predicate set skips the operation: its answer equals the value set. A predicate
    /// not ended skips nothing.
    fn predicated_out(&mut self) -> bool {
        let Some((q, value)) = self.ctx.predication.clone() else {
            return false;
        };
        match self.wait_answer(&q) {
            Ok(Some(v)) => (v[0] != 0) == value,
            _ => false,
        }
    }

    /// Before a resource is written while counted occlusion queries run: the draws kept read it as
    /// it was.
    fn before_write(&mut self) {
        if !self.queries.counted.is_empty() || !self.queries.statistics.is_empty() {
            self.end_pass();
        }
    }

    /// Before a draw while counted occlusion queries run: the targets set into the pass tracker,
    /// pending clears run where the counter must copy the depth the draw begins from, and the
    /// draw's counting program, with whether it writes depth or stencil.
    fn prepare_counted(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
    ) -> Result<(B::Program, bool)> {
        let program = self.counting_program(vs, vk, ps)?;
        self.track_targets()?;
        let ds = &self
            .ctx
            .depth_stencil
            .as_ref()
            .unwrap_or(&self.defaults.depth_stencil)
            .desc;
        let writes = self.ctx.depth.is_some()
            && ((ds.depth_enable && ds.depth_write)
                || (ds.stencil_enable && ds.stencil_write_mask != 0));
        if writes && !self.ctx.counted_copy {
            if !self.ctx.clears.is_empty() {
                self.open_pass();
            }
            self.end_pass();
            self.ctx.counted_copy = true;
        }
        Ok((program, writes))
    }

    /// Sets the bound targets into the pass tracker for the counters, where they changed.
    fn track_targets(&mut self) -> Result<()> {
        let key: Vec<usize> = self
            .ctx
            .targets
            .iter()
            .chain([&self.ctx.depth])
            .map(|v| v.as_ref().map_or(0, |v| Rc::as_ptr(v) as usize))
            .collect();
        if self.ctx.counted_targets != key {
            self.end_pass();
            let targets = self.pass_targets();
            let r = self.passes.set_targets(&mut self.backend, targets);
            self.gpu(r)?;
            self.ctx.counted_targets = key;
        }
        Ok(())
    }

    /// Before a draw while pipeline statistics queries run and a pixel shader is bound: the targets
    /// set into the pass tracker, the depth-stencil block with both tests off, and the program the
    /// pixel counter replays the draw with, counting one per pixel shader invocation.
    fn prepare_pixel_counted(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: &Rc<Shader<B>>,
        samples: u32,
    ) -> Result<(B::Program, Rc<DepthStencil<B>>)> {
        let per_sample = matches!(
            ps.reflection.extras,
            wcontainers_gpu::StageExtras::Pixel {
                per_sample: true,
                ..
            }
        ) || ps.reflection.inputs.iter().any(|i| {
            i.system == Some(wcontainers_gpu::SystemValue::SampleIndex)
                || matches!(
                    i.interpolation,
                    Interpolation::PerspectiveSample | Interpolation::LinearSample
                )
        });
        let (id, code) = match (samples > 1, per_sample) {
            (false, _) => (
                PER_PIXEL,
                "@fragment fn main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }",
            ),
            (true, false) => (
                PER_PIXEL_MASKED,
                "struct Out { @location(0) count: vec4<f32>, @builtin(sample_mask) mask: u32 }
                 @fragment fn main(@builtin(sample_mask) covered: u32) -> Out {
                     return Out(vec4<f32>(1.0), covered & (~covered + 1u));
                 }",
            ),
            (true, true) => (
                PER_SAMPLE,
                "@fragment fn main(@builtin(sample_index) i: u32) -> @location(0) vec4<f32> {
                     return vec4<f32>(1.0);
                 }",
            ),
        };
        self.track_targets()?;
        let off = match &self.tests_off {
            Some(d) => d.clone(),
            None => {
                let d = self.create_depth_stencil_state(&DepthStencilDesc {
                    depth_enable: false,
                    depth_write: false,
                    stencil_enable: false,
                    ..DepthStencilDesc::default_state()
                })?;
                self.tests_off = Some(d.clone());
                d
            }
        };
        let key = (vs.gpu.id, vk, id);
        if let Some(p) = self.counting.get(&key) {
            return Ok((p.handle.clone(), off));
        }
        let module = ShaderModule::from_wgsl(code, "main", ShaderStage::Fragment)
            .or_else(|_| not_implemented("the pixel counter's program on this back end"))?;
        let fragment = self.backend.create_shader(&module);
        let fragment = self.gpu(fragment)?;
        let vertex = self
            .interpolated
            .get(&(vs.gpu.id, vk, ps.gpu.id))
            .map_or(&vs.gpu.handle, |s| &s.shader.handle);
        let program = self.backend.create_program(&Program::Render {
            vertex,
            fragment: Some(&fragment),
        });
        self.backend.destroy(Object::Shader(fragment));
        let program = self.gpu(program)?;
        let id = self.id();
        self.counting.insert(
            key,
            Kept::new(program.clone(), id, Object::Program, &self.graveyard),
        );
        Ok((program, off))
    }

    /// Adds a draw to the running pipeline statistics queries: input assembler, vertex shader and
    /// clipper counts from its arguments, as a GPU without vertex reuse or clipping splits counts
    /// them (indirect arguments and the indices of indexed strips read back).
    fn count_draw_statistics(
        &mut self,
        range: Range,
        instances: u32,
        args: Option<&Rc<Buffer<B>>>,
    ) -> Result<()> {
        let (range, instances) = match range {
            Range::Indirect { indexed, offset } => {
                let args = args.expect("indirect arguments");
                let (r, _, n) = self.read_indirect(args, offset, indexed)?;
                (r, n)
            }
            r => (r, instances),
        };
        let t = self.ctx.topology;
        let (count, per_instance) = match range {
            Range::Indexed { count, start, .. } if strip_topology(t) => {
                self.indexed_strip(start, count)?
            }
            Range::Indexed { count, .. } | Range::Vertices { count, .. } => {
                (u64::from(count), primitives(t, count))
            }
            Range::Indirect { .. } => unreachable!("read back above"),
        };
        let vertices = count * u64::from(instances);
        let prims = per_instance * u64::from(instances);
        use crate::query::statistic::*;
        self.add_statistics(&[
            (IA_VERTICES, vertices),
            (IA_PRIMITIVES, prims),
            (VS_INVOCATIONS, vertices),
            (C_INVOCATIONS, prims),
            (C_PRIMITIVES, prims),
        ]);
        Ok(())
    }

    /// The vertices and primitives of an indexed strip of `count` indices from `start`, read
    /// back: each cut index (all ones) ends one strip and starts the next, and is no vertex.
    fn indexed_strip(&mut self, start: u32, count: u32) -> Result<(u64, u64)> {
        let Some((buffer, format, offset)) = self.ctx.index.clone() else {
            return Ok((0, 0));
        };
        let size = match format {
            IndexFormat::U16 => 2u64,
            _ => 4,
        };
        let first = u64::from(offset) + u64::from(start) * size;
        let bytes = (u64::from(count) * size).min(buffer.size.get().saturating_sub(first));
        if bytes == 0 {
            return Ok((0, 0));
        }
        self.backend.slow_paths().hit(SlowPath::ReadBackWait);
        self.settle();
        let handle = buffer.handle();
        let rb = self.backend.read_back(&ReadSource::Buffer {
            buffer: &handle,
            offset: first,
            size: bytes,
        });
        drop(handle);
        let rb = self.gpu(rb)?;
        if self.backend.read_state(&rb) == ReadState::Pending {
            self.backend.wait_read(&rb);
        }
        let mut data = vec![0u8; bytes as usize];
        self.backend.finish_read(rb, &mut data);
        let cut = |i: &[u8]| i.iter().all(|&b| b == 0xff);
        let line = self.ctx.topology == topology::LINESTRIP;
        let (mut vertices, mut total) = (0u64, 0u64);
        for run in data
            .chunks(size as usize)
            .collect::<Vec<_>>()
            .split(|i| cut(i))
        {
            let n = run.len() as u64;
            vertices += n;
            total += if line {
                n.saturating_sub(1)
            } else {
                n.saturating_sub(2)
            };
        }
        Ok((vertices, total))
    }

    /// The bound targets as the pass tracker takes them.
    fn pass_targets(&self) -> PassTargets<B> {
        let target = |v: &Rc<View<B>>| {
            let t = &v.texture;
            let mip = v.desc.first_mip;
            let d3 = t.desc.dimension == TexDim::D3;
            Target {
                view: v.gpu.handle.clone(),
                texture: t.gpu.handle.clone(),
                format: v.format,
                mip,
                layer: if d3 { 0 } else { v.desc.first_layer },
                depth_slice: if d3 { v.desc.first_layer } else { 0 },
                width: (t.desc.width >> mip).max(1),
                height: (t.desc.height >> mip).max(1),
                samples: t.desc.sample_count,
            }
        };
        let empty = (self.ctx.targets.iter().all(Option::is_none) && self.ctx.depth.is_none())
            .then(|| self.empty_size());
        PassTargets {
            colors: std::array::from_fn(|i| {
                self.ctx.targets[i].as_ref().map(|v| ColorTarget {
                    target: target(v),
                    resolve: None,
                })
            }),
            depth: self.ctx.depth.as_ref().map(target),
            empty,
        }
    }

    /// The counting program of a vertex and pixel shader pair, made at its first counted draw.
    fn counting_program(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
    ) -> Result<B::Program> {
        let key = (vs.gpu.id, vk, ps.map_or(0, |p| p.gpu.id));
        if let Some(p) = self.counting.get(&key) {
            return Ok(p.handle.clone());
        }
        // Made after the pair's own program, so the vertex shader converted for its interpolation
        // and variant is there.
        self.program(vs, vk, ps, StageVariant::default())?;
        let module = match ps {
            Some(p) => {
                let mut c = crate::shader::convert(&p.bytecode, SourceStage::Pixel)?.0;
                crate::counting::from_pixel_shader(&c.modules.pop().expect("one module"))?
            }
            None => crate::counting::without_pixel_shader()?,
        };
        let fragment = self.backend.create_shader(&module);
        let fragment = self.gpu(fragment)?;
        let vertex = self
            .interpolated
            .get(&key)
            .map_or(&vs.gpu.handle, |s| &s.shader.handle);
        let program = self.backend.create_program(&Program::Render {
            vertex,
            fragment: Some(&fragment),
        });
        self.backend.destroy(Object::Shader(fragment));
        let program = self.gpu(program)?;
        let id = self.id();
        self.counting.insert(
            key,
            Kept::new(program.clone(), id, Object::Program, &self.graveyard),
        );
        Ok(program)
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
        let empty =
            (targets.iter().all(Option::is_none) && depth.is_none()).then(|| self.empty_size());
        self.backend.begin_pass(&PassDesc {
            colors,
            depth: depth_attachment,
            empty,
            occlusion: occlusion.as_ref().map(|s| &s.handle),
        });
        self.ctx.pass_open = true;
        self.ctx.empty_pass = empty;
    }

    /// The size of a pass without attachments (Direct3D's drawing with only unordered access views
    /// bound): what the first viewport reaches, one pixel to the largest 2D texture, one sample.
    fn empty_size(&self) -> EmptyPass {
        let max = self.backend.caps().limits.max_texture_2d;
        let reach = |at: f32, len: f32| ((at + len).ceil().max(1.0) as u32).min(max);
        let v = self.ctx.viewports.first().copied().unwrap_or_default();
        EmptyPass {
            width: reach(v.x, v.width),
            height: reach(v.y, v.height),
            samples: 1,
        }
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

    /// `DrawInstancedIndirect` (`indexed` false) and `DrawIndexedInstancedIndirect`. Arguments
    /// Direct3D does not read are invalid; a draw this front cannot serve removes the device.
    pub fn draw_indirect(
        &mut self,
        args: &Rc<Buffer<B>>,
        offset: u32,
        indexed: bool,
    ) -> Result<()> {
        self.check_indirect(args, offset, indexed)?;
        if self.removed.is_some() {
            return Ok(());
        }
        // Fetching in the program steps by the first instance, which only the arguments hold.
        let pulls = self
            .ctx
            .layout
            .as_ref()
            .is_some_and(|l| !l.pulled.is_empty());
        let r = if !pulls
            && self
                .backend
                .caps()
                .has(Features::INDIRECT | Features::INDIRECT_FIRST_INSTANCE)
        {
            self.try_draw(Range::Indirect { indexed, offset }, 0, 1, Some(args))
        } else {
            self.read_indirect(args, offset, indexed)
                .and_then(|(range, first, n)| self.try_draw(range, first, n, None))
        };
        if let Err(e) = r {
            self.lose(e);
        }
        Ok(())
    }

    /// An indirect dispatch's group counts read back (`IndirectReadBack`), for pipeline statistics.
    fn read_dispatch(&mut self, args: &Buffer<B>, offset: u32) -> Result<[u32; 3]> {
        self.backend.slow_paths().hit(SlowPath::IndirectReadBack);
        self.settle();
        let handle = args.handle();
        let rb = self.backend.read_back(&ReadSource::Buffer {
            buffer: &handle,
            offset: u64::from(offset),
            size: 12,
        });
        drop(handle);
        let rb = self.gpu(rb)?;
        if self.backend.read_state(&rb) == ReadState::Pending {
            self.backend.wait_read(&rb);
        }
        let mut bytes = [0u8; 12];
        self.backend.finish_read(rb, &mut bytes);
        Ok(std::array::from_fn(|i| {
            u32::from_le_bytes(bytes[4 * i..4 * i + 4].try_into().unwrap())
        }))
    }

    /// An indirect draw's arguments read back (`IndirectReadBack`), as the range, first instance and
    /// instance count of a direct draw: for back ends without indirect draws or without a first
    /// instance in them.
    fn read_indirect(
        &mut self,
        args: &Buffer<B>,
        offset: u32,
        indexed: bool,
    ) -> Result<(Range, u32, u32)> {
        self.backend.slow_paths().hit(SlowPath::IndirectReadBack);
        self.settle();
        let size = indirect_bytes(indexed) as usize;
        let handle = args.handle();
        let rb = self.backend.read_back(&ReadSource::Buffer {
            buffer: &handle,
            offset: u64::from(offset),
            size: size as u64,
        });
        let rb = self.gpu(rb)?;
        if self.backend.read_state(&rb) == ReadState::Pending {
            self.backend.wait_read(&rb);
        }
        let mut bytes = [0u8; 20];
        self.backend.finish_read(rb, &mut bytes[..size]);
        let word = |i: usize| u32::from_le_bytes(bytes[4 * i..4 * i + 4].try_into().unwrap());
        // D3D11_DRAW_INSTANCED_INDIRECT_ARGS and D3D11_DRAW_INDEXED_INSTANCED_INDIRECT_ARGS.
        Ok(if indexed {
            let range = Range::Indexed {
                count: word(0),
                start: word(2),
                base: word(3) as i32,
            };
            (range, word(4), word(1))
        } else {
            let range = Range::Vertices {
                count: word(0),
                start: word(2),
            };
            (range, word(3), word(1))
        })
    }

    /// Direct3D reads indirect arguments only from buffers made for them, at whole words inside it.
    fn check_indirect(&self, args: &Buffer<B>, offset: u32, indexed: bool) -> Result<()> {
        if args.desc.misc_flags & desc::MISC_DRAWINDIRECT_ARGS == 0 {
            return Err(Error::InvalidArg(
                "indirect arguments in a buffer not made for them",
            ));
        }
        if !offset.is_multiple_of(4)
            || u64::from(offset) + u64::from(indirect_bytes(indexed))
                > u64::from(args.desc.byte_width)
        {
            return Err(Error::InvalidArg("indirect arguments past the buffer"));
        }
        Ok(())
    }

    /// Draws; a draw this front cannot serve removes the device, the only answer Direct3D has for it.
    fn draw_any(&mut self, range: Range, first_instance: u32, instances: u32) {
        if let Err(e) = self.try_draw(range, first_instance, instances, None) {
            self.lose(e);
        }
    }

    /// The program of a vertex and pixel shader pair as the variants `vk` and `pk`, made at its
    /// first use, and where its two stages read their helper constants.
    pub(crate) fn program(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
        pk: StageVariant,
    ) -> Result<(B::Program, [Rc<Helpers>; 2])> {
        let key = (vs.gpu.id, vk, ps.map_or(0, |p| p.gpu.id), pk);
        if let Some(p) = self.programs.get(&key) {
            return Ok((p.program.handle.clone(), p.helpers.clone()));
        }
        self.vertex_variant(vs, vk, ps)?;
        if let Some(p) = ps
            && pk != StageVariant::default()
            && !self.variants.contains_key(&(p.gpu.id, pk))
        {
            let v =
                self.convert_kept(&p.bytecode, SourceStage::Pixel, &pk.key(SourceStage::Pixel))?;
            self.variants.insert((p.gpu.id, pk), v);
        }
        let (vertex, vh) = self
            .interpolated
            .get(&(key.0, vk, key.2))
            .map_or((&vs.gpu.handle, &vs.helpers), |v| {
                (&v.shader.handle, &v.helpers)
            });
        let (fragment, ph) = match ps {
            None => (None, Rc::default()),
            Some(p) => self
                .variants
                .get(&(p.gpu.id, pk))
                .map_or((Some(&p.gpu.handle), p.helpers.clone()), |v| {
                    (Some(&v.shader.handle), v.helpers.clone())
                }),
        };
        let helpers = [vh.clone(), ph];
        let program = self
            .backend
            .create_program(&Program::Render { vertex, fragment });
        let program = self.gpu(program)?;
        let id = self.id();
        self.programs.insert(
            key,
            Linked {
                program: Kept::new(program.clone(), id, Object::Program, &self.graveyard),
                helpers: helpers.clone(),
            },
        );
        Ok((program, helpers))
    }

    /// Converts `vs` again where the pixel shader interpolates its outputs other than by perspective
    /// or the draw asks for a variant, kept in `interpolated`.
    fn vertex_variant(
        &mut self,
        vs: &Rc<Shader<B>>,
        vk: StageVariant,
        ps: Option<&Rc<Shader<B>>>,
    ) -> Result<()> {
        let key = (vs.gpu.id, vk, ps.map_or(0, |p| p.gpu.id));
        if self.interpolated.contains_key(&key) {
            return Ok(());
        }
        // WebGPU and GLSL ES require the stages to agree on interpolation, which in Direct3D the
        // pixel shader alone chooses: the vertex shader is converted again to write its outputs so.
        let mut interpolation: Vec<(u32, Interpolation)> = ps
            .iter()
            .flat_map(|p| &p.reflection.inputs)
            .filter(|i| i.system.is_none() && i.interpolation != Interpolation::Perspective)
            .map(|i| (i.location, i.interpolation))
            .collect();
        interpolation.sort_by_key(|i| i.0);
        interpolation.dedup_by_key(|i| i.0);
        if interpolation.is_empty() && vk == StageVariant::default() {
            return Ok(());
        }
        let pulled = match &self.ctx.layout {
            Some(l) if vk.pulled == l.id => {
                l.pulled.iter().map(|e| (e.location, e.format)).collect()
            }
            _ => Vec::new(),
        };
        let variant = VariantKey {
            output_interpolation: interpolation,
            pulled,
            ..vk.key(SourceStage::Vertex)
        };
        let v = self.convert_kept(&vs.bytecode, SourceStage::Vertex, &variant)?;
        self.interpolated.insert(key, v);
        Ok(())
    }

    /// `bytecode` converted as `key` and handed to the back end.
    fn convert_kept(
        &mut self,
        bytecode: &[u8],
        stage: SourceStage,
        key: &VariantKey,
    ) -> Result<Variant<B>> {
        let mut c = crate::shader::convert_variant(bytecode, stage, key)?;
        let helpers = Rc::new(Helpers::of(&c));
        let module = c.modules.pop().expect("one module");
        let shader = self.backend.create_shader(&module);
        let shader = self.gpu(shader)?;
        let id = self.id();
        Ok(Variant {
            shader: Kept::new(shader, id, Object::Shader, &self.graveyard),
            helpers,
        })
    }

    /// What a draw asks of `stage`'s program `s`: the sampler slots it reads through a biased
    /// sampler, and the targets' `samples` where it reads them.
    fn stage_variant(&self, stage: usize, s: &Shader<B>, samples: u8) -> StageVariant {
        let caps = self.backend.caps();
        let mut v = StageVariant {
            samples: if s.reads_samples { samples } else { 0 },
            ..StageVariant::default()
        };
        if s.samplers != 0 {
            for (i, b) in self.ctx.samplers[stage].iter().enumerate().take(16) {
                let Some(b) = b.as_ref().filter(|_| s.samplers & (1 << i) != 0) else {
                    continue;
                };
                if b.desc.mip_lod_bias != 0.0 {
                    v.lod_bias |= 1 << i;
                }
                // The program filters through a border sampler, every address mode included, and
                // mirrors once around plain samples.
                let mirror = b.desc.address.contains(&5) && !caps.has(Features::MIRROR_ONCE);
                if b.desc.address.contains(&4) && !caps.has(Features::BORDER_COLOR)
                    || mirror && s.mirror_filtered & (1 << i) != 0
                {
                    v.border_in_shader |= 1 << i;
                } else if mirror {
                    v.mirror_once_in_shader |= 1 << i;
                }
            }
        }
        if s.textures != 0 {
            for i in 0..self.ctx.resources[stage].len().min(128) {
                if s.textures & (1 << i) != 0 && self.min_lod_of(stage, i as u32).is_some() {
                    v.min_lod_clamped |= 1 << i;
                }
            }
        }
        // Textures of formats the back end cannot filter, sampled through a filtering sampler (an
        // empty slot reads Direct3D's default, which filters linearly).
        for &(t, smp) in &s.reflection.sampled_pairs {
            let filters = match self.ctx.samplers[stage]
                .get(smp as usize)
                .and_then(Option::as_ref)
            {
                Some(b) => {
                    b.made.compare.is_none()
                        && [b.made.min, b.made.mag, b.made.mip].contains(&Filter::Linear)
                }
                None => true,
            };
            let unfiltered = match self.ctx.resources[stage]
                .get(t as usize)
                .and_then(Option::as_deref)
            {
                Some(ShaderView::Texture(view)) => {
                    !caps.formats.get(view.format).contains(FormatUse::FILTER)
                }
                _ => false,
            };
            if t < 128 && filters && unfiltered {
                v.filter_in_shader |= 1 << t;
            }
        }
        v
    }

    /// The sampler bound to `stage`'s `slot`.
    fn bound_sampler(&self, stage: usize, slot: u32) -> Option<&Sampler<B>> {
        self.ctx.samplers[stage]
            .get(slot as usize)
            .and_then(Option::as_deref)
    }

    /// The minimum level-of-detail clamp of the texture view bound to `stage`'s `slot`, in the view's
    /// levels, and the view's level count; none where the clamp leaves every level of the view.
    fn min_lod_of(&self, stage: usize, slot: u32) -> Option<(f32, u32)> {
        let v = self.ctx.resources[stage]
            .get(slot as usize)?
            .as_deref()?
            .texture()?;
        let clamp = v.texture.min_lod.get() - v.desc.first_mip as f32;
        (clamp > 0.0).then_some((clamp, v.desc.mip_count))
    }

    /// The sampler bound to `stage`'s `slot` (Direct3D's default where none is) with its minimum
    /// level raised to `clamp`, made at its first use.
    fn clamped_sampler(
        &mut self,
        stage: usize,
        slot: u32,
        clamp: f32,
    ) -> Result<(B::Sampler, u64)> {
        let s = match self.ctx.samplers[stage]
            .get(slot as usize)
            .cloned()
            .flatten()
        {
            Some(s) => s,
            None => {
                self.default_sampler()?;
                self.zeros.sampler.clone().expect("made")
            }
        };
        let [min, max] = s.made.lod_range.map(f32::from_bits);
        // Direct3D clamps the level to the sampler's range and then to the resource's clamp.
        let lod_range = [min.max(clamp), max.max(clamp)].map(f32::to_bits);
        if let Some((_, k)) = s.clamped.borrow().iter().find(|(m, _)| *m == lod_range[0]) {
            return Ok((k.handle.clone(), k.id));
        }
        let gpu = self.backend.create_sampler(&GpuSampler {
            lod_range,
            ..s.made
        });
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        s.clamped.borrow_mut().push((
            lod_range[0],
            Kept::new(gpu.clone(), id, Object::Sampler, &self.graveyard),
        ));
        Ok((gpu, id))
    }

    /// The strides of the bound vertex buffers in the slots the input layout reads, zero elsewhere.
    pub(crate) fn strides(&self) -> [u32; 32] {
        let mut strides = [0u32; 32];
        if let Some(l) = &self.ctx.layout {
            for e in &l.elements {
                strides[e.slot as usize] = self.ctx.vertex_buffers[e.slot as usize]
                    .as_ref()
                    .map_or(0, |v| v.1);
            }
        }
        strides
    }

    fn vertex_layout(&mut self) -> Result<B::VertexLayout> {
        let strides = self.strides();
        self.vertex_layout_for(self.ctx.layout.clone(), strides)
    }

    /// The vertex layout block of an input layout read at `strides`, made at its first use.
    pub(crate) fn vertex_layout_for(
        &mut self,
        layout: Option<Rc<InputLayout>>,
        strides: [u32; 32],
    ) -> Result<B::VertexLayout> {
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

    /// The copy of `c`'s window of `length` bytes at `offset` read by a block of `size` bytes: its
    /// bytes past the window are zero; copied again where `c` was written since.
    fn window(&mut self, c: &Buffer<B>, offset: u64, length: u64, size: u64) -> Result<B::Buffer> {
        let source = (c.writes.get(), c.gpu.borrow().id);
        let found = c
            .windows
            .borrow()
            .iter()
            .position(|w| (w.offset, w.length, w.size) == (offset, length, size));
        let i = match found {
            Some(i) => i,
            None => {
                self.settle();
                let twin = self.backend.create_buffer(&gpu_buffer(&c.desc, size));
                let twin = self.gpu(twin)?;
                self.backend.clear_buffer(&twin, 0, size, &[0]);
                let id = self.id();
                let mut windows = c.windows.borrow_mut();
                windows.push(Window {
                    offset,
                    length,
                    size,
                    copied: Cell::new(None),
                    twin: Kept::new(twin, id, Object::Buffer, &self.graveyard),
                });
                windows.len() - 1
            }
        };
        let windows = c.windows.borrow();
        let w = &windows[i];
        if w.copied.get() != Some(source) {
            self.backend.slow_paths().hit(SlowPath::ConstantBufferGrown);
            self.settle();
            if length > 0 {
                self.backend
                    .copy_buffer(&c.handle(), offset, &w.twin.handle, 0, length);
            }
            w.copied.set(Some(source));
        }
        Ok(w.twin.handle.clone())
    }

    /// Copies again the constant buffer windows bound shorter than their blocks whose buffers were
    /// written since.
    fn refresh_windows(&mut self) -> Result<()> {
        for stage in [VERTEX, PIXEL, COMPUTE] {
            if self.ctx.windows[stage].is_empty() {
                continue;
            }
            let windows = self.ctx.windows[stage].clone();
            for (_, c, offset, length, size) in &windows {
                self.window(c, *offset, *length, *size)?;
            }
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
        if self.predicated_out() {
            return Ok(());
        }
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
        self.refresh_windows()?;
        self.update_set(COMPUTE, &[])?;
        if self.stage_variant(COMPUTE, &cs, 1) != StageVariant::default() {
            return not_implemented(
                "a compute shader sampling in a way the back end's sampler lacks (a bias, a resource clamp, border or mirror-once addressing, an unfilterable format)",
            );
        }
        let key = (
            cs.gpu.id,
            StageVariant::default(),
            0,
            StageVariant::default(),
        );
        let program = match self.programs.get(&key) {
            Some(p) => p.program.handle.clone(),
            None => {
                let p = self
                    .backend
                    .create_program(&Program::Compute(&cs.gpu.handle));
                let p = self.gpu(p)?;
                let id = self.id();
                self.programs.insert(
                    key,
                    Linked {
                        program: Kept::new(p.clone(), id, Object::Program, &self.graveyard),
                        helpers: Default::default(),
                    },
                );
                p
            }
        };
        if !self.queries.statistics.is_empty() {
            let groups = match indirect {
                None => groups,
                Some((b, offset)) => self.read_dispatch(b, offset)?,
            };
            let size = match cs.reflection.extras {
                wcontainers_gpu::StageExtras::Compute { workgroup_size } => workgroup_size,
                _ => [1; 3],
            };
            let threads = groups
                .iter()
                .chain(&size)
                .fold(1u64, |n, &g| n * u64::from(g));
            self.add_statistics(&[(crate::query::statistic::CS_INVOCATIONS, threads)]);
        }
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
        if self.predicated_out() {
            return Ok(());
        }
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
        if self.predicated_out() {
            return Ok(());
        }
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
    /// Makes `stage`'s binding set again where its bindings changed; `pulled` holds the binding of
    /// each vertex input the program fetches itself, by location.
    fn update_set(&mut self, stage: usize, pulled: &[(u32, u32)]) -> Result<()> {
        if !self.ctx.sets_dirty[stage] {
            return Ok(());
        }
        let set = set_of(stage);
        let shader = self.stage_shader(stage).clone();
        let Some(shader) = shader
            .filter(|s| !pulled.is_empty() || s.reflection.bindings.iter().any(|b| b.set == set))
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
        // A sampler read the other way than it compares reads through its twin.
        let mut twins: Vec<(u32, B::Sampler)> = Vec::new();
        for b in wanted().filter(|b| b.kind == SlotKind::Sampler) {
            let compares = !matches!(stand_in(b.binding), Some(StandIn::DefaultSampler));
            let Some(s) = self.ctx.samplers[stage]
                .get(b.register as usize)
                .and_then(Option::clone)
            else {
                continue;
            };
            if s.made.compare.is_some() != compares {
                twins.push((b.binding, self.sampler_twin(&s, compares)?));
            }
        }
        let zero_stream = if pulled.is_empty() {
            None
        } else {
            Some(self.zero_storage()?)
        };
        // Windows shorter than their shader's block read through a copy of the block's size whose
        // bytes past the window are zero.
        let mut windows = Vec::new();
        for b in wanted().filter(|b| b.kind == SlotKind::Uniform) {
            let slot = b.register as usize;
            let declared = shader
                .constant_sizes
                .iter()
                .find(|(n, _)| *n == b.binding)
                .map_or(0, |s| s.1);
            if let (Some(Some(c)), Some(Some(range))) = (
                self.ctx.constant_buffers[stage].get(slot),
                self.ctx.constant_ranges[stage].get(slot),
            ) {
                let (offset, length) = window_of(c, *range);
                if length < declared {
                    windows.push((b.register, c.clone(), offset, length, declared));
                }
            }
        }
        let mut copies = Vec::new();
        for (register, c, offset, length, declared) in &windows {
            copies.push((
                *register,
                self.window(c, *offset, *length, *declared)?,
                *declared,
            ));
        }
        self.ctx.windows[stage] = windows;
        let mut cbs = Vec::new();
        for (slot, (c, range)) in self.ctx.constant_buffers[stage]
            .iter()
            .zip(&self.ctx.constant_ranges[stage])
            .enumerate()
        {
            let Some(c) = c else {
                cbs.push(None);
                continue;
            };
            if let Some((_, twin, size)) = copies.iter().find(|t| t.0 as usize == slot) {
                cbs.push(Some((twin.clone(), 0, *size)));
                continue;
            }
            let (offset, size) = match *range {
                None => (0, c.size.get()),
                Some(range) => window_of(c, range),
            };
            cbs.push(Some((c.handle(), offset, size)));
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
        // The streams the vertex program fetches, each bound from its offset rounded down to the
        // storage offset alignment to the buffer's end; an empty slot, or an offset past the end,
        // reads the buffer of zeros.
        let limits = &self.backend.caps().limits;
        let align = u64::from(limits.min_storage_offset_alignment.max(4));
        let largest = limits.max_storage_buffer_size & !3;
        let streams: Vec<_> = pulled
            .iter()
            .map(|&(binding, location)| {
                let bound = self
                    .ctx
                    .layout
                    .as_ref()
                    .and_then(|l| l.pulled.iter().find(|e| e.location == location))
                    .and_then(|e| self.ctx.vertex_buffers[e.slot as usize].as_ref())
                    .map(|(b, _, offset)| (b, u64::from(*offset) / align * align))
                    .filter(|(b, start)| start + 4 <= b.size.get());
                (
                    binding,
                    bound.map(|(b, start)| {
                        let size = ((b.size.get() - start) & !3).min(largest);
                        (b.gpu.borrow(), start, size)
                    }),
                )
            })
            .collect();
        let uavs = &self.ctx.uavs[stage];
        let views = &self.ctx.resources[stage];
        let samplers = &self.ctx.samplers[stage];
        let mut entries = Vec::new();
        for (binding, bound) in &streams {
            let range = match (bound, &zero_stream) {
                (Some((b, offset, size)), _) => BufferRange {
                    buffer: &b.handle,
                    offset: *offset,
                    size: *size,
                },
                (None, Some(zero)) => BufferRange {
                    buffer: zero,
                    offset: 0,
                    size: ZERO_BYTES,
                },
                (None, None) => unreachable!("made with the streams"),
            };
            entries.push(BindingEntry {
                binding: *binding,
                stages,
                resource: BindingResource::StorageBuffer {
                    range,
                    writable: false,
                },
            });
        }
        for b in wanted() {
            let slot = b.register as usize;
            let resource = match b.kind {
                SlotKind::Uniform => {
                    cbs.get(slot)
                        .and_then(Option::as_ref)
                        .map(|(c, offset, size)| {
                            BindingResource::UniformBuffer(BufferRange {
                                buffer: c,
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
                SlotKind::Sampler => samplers.get(slot).and_then(Option::as_ref).map(|s| {
                    BindingResource::Sampler(
                        twins
                            .iter()
                            .find(|t| t.0 == b.binding)
                            .map_or(&s.gpu.handle, |t| &t.1),
                    )
                }),
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
        drop(streams);
        drop(cbs);
        drop(spans);
        drop(srvs);
        let set = self.gpu(set)?;
        let id = self.id();
        self.ctx.sets[stage] = Some(Kept::new(set, id, Object::BindingSet, &self.graveyard));
        self.ctx.sets_dirty[stage] = false;
        Ok(())
    }

    /// The bound raster block made with sample mask `mask`, kept on its rasterizer state from its
    /// first use.
    fn masked_raster(&mut self, mask: u32) -> Result<B::Raster> {
        let r = self
            .ctx
            .raster
            .clone()
            .unwrap_or_else(|| self.defaults.raster.clone());
        if let Some((_, k)) = r.masked.borrow().iter().find(|(m, _)| *m == mask) {
            return Ok(k.handle.clone());
        }
        let gpu = self.backend.create_raster(&wcontainers_gpu::RasterDesc {
            sample_mask: mask,
            ..crate::device::raster_block(&r.desc)
        });
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        r.masked.borrow_mut().push((
            mask,
            Kept::new(gpu.clone(), id, Object::Raster, &self.graveyard),
        ));
        Ok(gpu)
    }

    /// `s` made comparing or not as `compares` says, kept on `s` from its first use.
    fn sampler_twin(&mut self, s: &Sampler<B>, compares: bool) -> Result<B::Sampler> {
        if let Some(t) = &*s.twin.borrow() {
            return Ok(t.handle.clone());
        }
        // Direct3D leaves comparing through a sampler that does not compare undefined; the twin
        // compares with the function its description names.
        let compare = if compares {
            Some(crate::device::compare_fn(s.desc.comparison)?)
        } else {
            None
        };
        let gpu = self
            .backend
            .create_sampler(&GpuSampler { compare, ..s.made });
        let gpu = self.gpu(gpu)?;
        let id = self.id();
        *s.twin.borrow_mut() = Some(Kept::new(gpu.clone(), id, Object::Sampler, &self.graveyard));
        Ok(gpu)
    }

    /// Writes the helper constants of the draw's two program stages where they differ from the
    /// last written, and keeps the set binding them (with an indirect draw's arguments, for a vertex
    /// program that reads its bases from them).
    /// How the vertex program fetches its input at `location` for a draw from `first_instance`:
    /// the first element's byte offset in the stream's binding, the stride, a divisor and a bias.
    fn pulled_stream(&self, location: u32, first_instance: u32) -> [u32; 4] {
        let Some(e) = self
            .ctx
            .layout
            .as_ref()
            .and_then(|l| l.pulled.iter().find(|e| e.location == location))
        else {
            return [0; 4];
        };
        let align = self
            .backend
            .caps()
            .limits
            .min_storage_offset_alignment
            .max(4);
        let (stride, offset) = self.ctx.vertex_buffers[e.slot as usize]
            .as_ref()
            .map_or((0, 0), |(_, stride, offset)| (*stride, *offset));
        let offset = offset % align + e.offset;
        match self
            .ctx
            .layout
            .as_ref()
            .and_then(|l| l.instance_steps[e.slot as usize])
        {
            // Every instance reads the first instance's element.
            Some(0) => [
                offset.wrapping_add(first_instance.wrapping_mul(stride)),
                0,
                1,
                0,
            ],
            Some(rate) => [
                offset,
                stride,
                rate,
                (rate - 1).wrapping_mul(first_instance),
            ],
            None => [offset, stride, 0, 0],
        }
    }

    fn write_helpers(
        &mut self,
        helpers: &[Rc<Helpers>; 2],
        bases: [u32; 2],
        args: Option<(&Rc<Buffer<B>>, u32, bool)>,
    ) -> Result<()> {
        const MAX: usize = 256;
        let mut blocks = [[0u8; MAX]; 2];
        for (i, (h, stage)) in helpers.iter().zip([VERTEX, PIXEL]).enumerate() {
            if h.size as usize > MAX {
                return not_implemented("a helper block larger than 256 bytes");
            }
            for slot in &h.values {
                let mut words = [0u32; 9];
                let n = match slot.value {
                    HelperValue::DrawBase => {
                        words[..3].copy_from_slice(&[bases[0], bases[1], 0]);
                        3
                    }
                    HelperValue::DrawArguments => {
                        let (_, offset, indexed) = args.expect("an indirect draw");
                        words[..3].copy_from_slice(&[offset / 4, u32::from(indexed), 0]);
                        3
                    }
                    HelperValue::PulledStream { input } => {
                        words[..4].copy_from_slice(&self.pulled_stream(input, bases[1]));
                        4
                    }
                    HelperValue::LodBias { sampler: s } => {
                        words[0] = self
                            .bound_sampler(stage, s)
                            .map_or(0.0, |s| s.desc.mip_lod_bias)
                            .to_bits();
                        1
                    }
                    HelperValue::SamplerState { sampler: s } => {
                        words = match self.bound_sampler(stage, s) {
                            Some(b) => crate::device::sampler_state(b),
                            None => {
                                self.default_sampler()?;
                                crate::device::sampler_state(
                                    self.zeros.sampler.as_ref().expect("made"),
                                )
                            }
                        };
                        9
                    }
                    HelperValue::BorderColor { sampler: s } => {
                        let border = self
                            .bound_sampler(stage, s)
                            .map_or([0.0; 4], |s| s.desc.border);
                        words[..4].copy_from_slice(&border.map(f32::to_bits));
                        4
                    }
                    HelperValue::TextureComponents { texture } => {
                        words[0] = match self.ctx.resources[stage]
                            .get(texture as usize)
                            .and_then(Option::as_deref)
                        {
                            Some(ShaderView::Texture(v)) => crate::format::components(v.format),
                            _ => 0b1111,
                        };
                        1
                    }
                    HelperValue::MinLod { texture } => {
                        let (clamp, levels) = self.min_lod_of(stage, texture).unwrap_or((0.0, 1));
                        words[..2].copy_from_slice(&[clamp.to_bits(), levels]);
                        2
                    }
                    _ => unreachable!("refused at conversion"),
                };
                for (k, w) in words[..n].iter().enumerate() {
                    let at = slot.offset as usize + 4 * k;
                    blocks[i][at..at + 4].copy_from_slice(&w.to_le_bytes());
                }
            }
        }
        // The merged samplers of textures under a minimum level-of-detail clamp; both stages
        // number theirs from the same binding, so only one of them may have any.
        if !helpers[0].merged.is_empty() && !helpers[1].merged.is_empty() {
            return not_implemented(
                "minimum level-of-detail clamps on textures both draw stages sample",
            );
        }
        let mut merged = Vec::new();
        for (h, stage) in helpers.iter().zip([VERTEX, PIXEL]) {
            for &(binding, (texture, sampler)) in &h.merged {
                let (clamp, _) = self.min_lod_of(stage, texture).unwrap_or((0.0, 1));
                let (s, id) = self.clamped_sampler(stage, sampler, clamp)?;
                merged.push((binding, stage, s, id));
            }
        }
        let samplers: Vec<u64> = merged.iter().map(|m| m.3).collect();
        let blocks = [
            &blocks[0][..helpers[0].size as usize],
            &blocks[1][..helpers[1].size as usize],
        ];
        let arguments = args.map_or(0, |(b, ..)| b.gpu.borrow().id);
        if self.helper_blocks.as_ref().is_some_and(|h| {
            h.arguments == arguments
                && h.blocks[0] == blocks[0]
                && h.blocks[1] == blocks[1]
                && h.samplers == samplers
        }) {
            return Ok(());
        }
        if helpers[0]
            .values
            .iter()
            .any(|h| h.value == HelperValue::DrawBase)
        {
            self.backend.slow_paths().hit(SlowPath::DrawBaseInHelpers);
        }
        let align = self.backend.caps().limits.min_uniform_offset_alignment;
        let mut offsets = [0u64; 2];
        let ring = self
            .backend
            .write_buffer_ring(RingKind::Data, &blocks, align, &mut offsets);
        let ring = self.gpu(ring)?;
        let args = args.map(|(b, ..)| (b.gpu.borrow(), b.size.get() & !3));
        let mut entries = Vec::with_capacity(3);
        for (i, stage) in [ShaderStage::Vertex, ShaderStage::Fragment]
            .into_iter()
            .enumerate()
        {
            if !blocks[i].is_empty() {
                entries.push(BindingEntry {
                    binding: helper_binding(stage),
                    stages: if i == 0 {
                        StageSet::VERTEX
                    } else {
                        StageSet::FRAGMENT
                    },
                    resource: BindingResource::UniformBuffer(BufferRange {
                        buffer: &ring,
                        offset: offsets[i],
                        size: blocks[i].len() as u64,
                    }),
                });
            }
        }
        if let Some((b, size)) = &args {
            entries.push(BindingEntry {
                binding: DRAW_ARGUMENTS_BINDING,
                stages: StageSet::VERTEX,
                resource: BindingResource::StorageBuffer {
                    range: BufferRange {
                        buffer: &b.handle,
                        offset: 0,
                        size: *size,
                    },
                    writable: false,
                },
            });
        }
        for (binding, stage, s, _) in &merged {
            entries.push(BindingEntry {
                binding: *binding,
                stages: if *stage == VERTEX {
                    StageSet::VERTEX
                } else {
                    StageSet::FRAGMENT
                },
                resource: BindingResource::Sampler(s),
            });
        }
        let set = self.backend.create_binding_set(&entries);
        drop(entries);
        drop(args);
        let set = self.gpu(set)?;
        let id = self.id();
        self.helper_blocks = Some(HelperBlocks {
            blocks: blocks.map(<[u8]>::to_vec),
            arguments,
            samplers,
            set: Kept::new(set, id, Object::BindingSet, &self.graveyard),
        });
        Ok(())
    }

    /// A draw; `args` holds an indirect draw's arguments.
    fn try_draw(
        &mut self,
        range: Range,
        first_instance: u32,
        instances: u32,
        args: Option<&Rc<Buffer<B>>>,
    ) -> Result<()> {
        if self.removed.is_some() || self.predicated_out() {
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
        // A mask leaving out samples of multisampled targets draws through a twin of the raster
        // block made with it.
        let samples = self
            .ctx
            .targets
            .iter()
            .chain([&self.ctx.depth])
            .flatten()
            .map(|v| v.texture.desc.sample_count)
            .max()
            .unwrap_or(1);
        let all = u32::MAX >> (32 - samples.min(32));
        let mask = self.ctx.sample_mask & all;
        let masked = if samples > 1 && mask != all {
            if !self.backend.caps().has(Features::SAMPLE_MASK) {
                return not_implemented(
                    "a sample mask without the back end's (SampleMaskInShader)",
                );
            }
            Some(self.masked_raster(mask)?)
        } else {
            None
        };
        let ps = self.ctx.ps.clone();
        let dual_source = ps.is_some()
            && self
                .ctx
                .blend
                .as_ref()
                .is_some_and(|b| b.desc.dual_source());
        if dual_source && !self.backend.caps().has(Features::DUAL_SOURCE_BLEND) {
            return not_implemented("dual-source blending without the back end's");
        }
        let target_samples = samples.min(u32::from(u8::MAX)) as u8;
        let indirect = matches!(range, Range::Indirect { .. });
        let pulled = self
            .ctx
            .layout
            .as_ref()
            .filter(|l| !l.pulled.is_empty())
            .map_or(0, |l| l.id);
        let vk = StageVariant {
            from_arguments: indirect && vs.helpers.size > 0,
            pulled,
            ..self.stage_variant(VERTEX, &vs, target_samples)
        };
        let pk = ps
            .as_ref()
            .map_or(StageVariant::default(), |p| StageVariant {
                dual_source,
                ..self.stage_variant(PIXEL, p, target_samples)
            });
        if vk.lod_bias | pk.lod_bias != 0 {
            self.backend.slow_paths().hit(SlowPath::LodBiasInShader);
        }
        if vk.min_lod_clamped | pk.min_lod_clamped != 0 {
            self.backend.slow_paths().hit(SlowPath::MinLodClamp);
        }
        if vk.border_in_shader | pk.border_in_shader != 0 {
            self.backend.slow_paths().hit(SlowPath::BorderInShader);
        }
        if vk.mirror_once_in_shader | pk.mirror_once_in_shader != 0 {
            self.backend.slow_paths().hit(SlowPath::MirrorOnceInShader);
        }
        if vk.filter_in_shader | pk.filter_in_shader != 0 {
            self.backend.slow_paths().hit(SlowPath::FilterInShader);
        }
        if vk.from_arguments {
            self.backend
                .slow_paths()
                .hit(SlowPath::DrawBaseFromArguments);
        }
        if vk.pulled != 0 {
            self.backend
                .slow_paths()
                .hit(SlowPath::InstanceRateInShader);
        }
        let (program, helpers) = self.program(&vs, vk, ps.as_ref(), pk)?;
        if !helpers[0].pulled.is_empty() {
            let storage = vs
                .reflection
                .bindings
                .iter()
                .filter(|b| b.kind == SlotKind::Storage)
                .count()
                + helpers[0].pulled.len();
            if storage > self.backend.caps().limits.storage_buffers_per_stage as usize {
                return not_implemented(
                    "more vertex inputs fetched in the program than the back end's storage bindings",
                );
            }
        }
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
        self.refresh_windows()?;
        if vk.pulled != self.ctx.pulled_set {
            self.ctx.sets_dirty[VERTEX] = true;
        }
        self.update_set(VERTEX, &helpers[0].pulled)?;
        self.ctx.pulled_set = vk.pulled;
        self.update_set(PIXEL, &[])?;
        // SV_VertexID and SV_InstanceID leave out the draw's bases, which the common form's indices
        // include: a vertex program reading them takes the bases from its helper constants or from
        // the indirect arguments; level-of-detail biases are helper constants too.
        let reads_helpers = helpers.iter().any(|h| h.size > 0 || !h.merged.is_empty());
        if reads_helpers {
            let (base, arguments) = match range {
                Range::Vertices { start, .. } => (start, None),
                Range::Indexed { base, .. } => (base as u32, None),
                Range::Indirect { indexed, offset } => (0, args.map(|b| (b, offset, indexed))),
            };
            self.write_helpers(&helpers, [base, first_instance], arguments)?;
        }
        if !self.queries.statistics.is_empty() {
            self.count_draw_statistics(range, instances, args)?;
        }

        let indexed = matches!(
            range,
            Range::Indexed { .. } | Range::Indirect { indexed: true, .. }
        );
        let args = args.map(|b| b.gpu.borrow());
        let (topology, gpu_range, generated) = match plain_topology(self.ctx.topology) {
            Some(t) => {
                let r = match range {
                    Range::Indirect { indexed, offset } => {
                        let args = &args.as_ref().expect("indirect arguments").handle;
                        let offset = u64::from(offset);
                        if indexed {
                            DrawRange::IndexedIndirect { args, offset }
                        } else {
                            DrawRange::Indirect { args, offset }
                        }
                    }
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
                    return not_implemented(
                        "indexed or indirect adjacency draws without a geometry shader",
                    );
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
        // The back ends bias triangles only, and move 24-bit depth by Direct3D's steps only where
        // they store it as such.
        if raster.depth_bias != 0 || raster.slope_scaled_depth_bias != 0.0 {
            if !matches!(topology, Topology::TriangleList | Topology::TriangleStrip) {
                return not_implemented("depth bias on lines and points");
            }
            if raster.depth_bias != 0
                && self.ctx.depth.as_ref().is_some_and(|d| {
                    matches!(d.format, Format::Depth24Plus | Format::Depth24PlusStencil8)
                })
                && !self.backend.caps().has(Features::DEPTH24_UNORM)
            {
                return not_implemented("a constant depth bias on 24-bit depth stored as float");
            }
        }
        let nothing = matches!(
            gpu_range,
            DrawRange::Indexed { count: 0, .. } | DrawRange::Vertices { count: 0, .. }
        );
        if nothing || self.ctx.viewports.is_empty() || (indexed && self.ctx.index.is_none()) {
            return Ok(());
        }
        // A pass without attachments is as large as the viewport of its first draw.
        if self.ctx.pass_open
            && let Some(open) = self.ctx.empty_pass
        {
            let wanted = self.empty_size();
            if wanted.width > open.width || wanted.height > open.height {
                self.end_pass();
            }
        }
        let counting = if self.queries.counted.is_empty() {
            None
        } else {
            if pk != StageVariant::default() {
                return not_implemented("counting the samples of a pixel shader variant");
            }
            Some(self.prepare_counted(&vs, vk, ps.as_ref())?)
        };
        // A draw without a pixel shader invokes none.
        let pixel_counting = match &ps {
            Some(p) if !self.queries.statistics.is_empty() => {
                Some(self.prepare_pixel_counted(&vs, vk, p, samples)?)
            }
            _ => None,
        };
        self.count_combination(&vs, ps.as_ref(), topology, samples);
        let alpha_to_coverage = self
            .ctx
            .blend
            .as_ref()
            .is_some_and(|b| b.desc.alpha_to_coverage);
        let instances_step_by_one = self
            .ctx
            .layout
            .as_ref()
            .is_none_or(|l| l.instance_steps.iter().flatten().all(|&s| s == 1));

        // Builds the draw's call and gives it to `$then`: once to the occlusion counter, which
        // keeps a counted draw before its pass opens, and once to the back end.
        macro_rules! with_call {
            ($call:ident => $then:expr) => {{
                let ctx = &self.ctx;
                // Fixed arrays: no allocation per draw (Direct3D has 32 vertex slots).
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
                // Without a shader choosing one (none is served), Direct3D draws through the first viewport
                // and scissor rectangle; with no rectangle set, the scissor test passes nothing.
                let v = ctx.viewports[0];
                let viewport = GpuViewport {
                    x: v.x.to_bits(),
                    y: v.y.to_bits(),
                    width: v.width.to_bits(),
                    height: v.height.to_bits(),
                    min_depth: v.min_depth.to_bits(),
                    max_depth: v.max_depth.to_bits(),
                };
                let blend = ctx.blend.as_ref().unwrap_or(&self.defaults.blend);
                let depth_stencil = ctx
                    .depth_stencil
                    .as_ref()
                    .unwrap_or(&self.defaults.depth_stencil);
                let raster = ctx.raster.as_ref().unwrap_or(&self.defaults.raster);
                let no_rect = [ScissorRect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                }];
                let scissors: &[ScissorRect] = match ctx.scissors.first() {
                    _ if !raster.desc.scissor => &[],
                    Some(rect) => std::slice::from_ref(rect),
                    None => &no_rect,
                };
                let mut bindings: [Option<&B::BindingSet>; MAX_BINDING_SETS] = [None; MAX_BINDING_SETS];
                bindings[0] = ctx.sets[VERTEX].as_ref().map(|k| &k.handle);
                bindings[1] = ctx.sets[PIXEL].as_ref().map(|k| &k.handle);
                if reads_helpers {
                    bindings[HELPER_SET as usize] =
                        self.helper_blocks.as_ref().map(|h| &h.set.handle);
                }
                let strip = matches!(topology, Topology::LineStrip | Topology::TriangleStrip);
                let call = DrawCall {
                    program: &program,
                    blend: &blend.gpu.handle,
                    depth_stencil: &depth_stencil.gpu.handle,
                    raster: masked.as_ref().unwrap_or(&raster.gpu.handle),
                    vertex_layout: &vertex_layout,
                    bindings,
                    vertex_buffers: &vertex_buffers[..used],
                    index,
                    topology,
                    restart: strip && indexed,
                    viewports: std::slice::from_ref(&viewport),
                    scissors,
                    blend_constant: ctx.blend_factor.map(f32::to_bits),
                    stencil_reference: [ctx.stencil_ref as u8; 2],
                    immediates: &[],
                    range: same_range(&gpu_range),
                };
                let $call = call;
                $then
            }};
        }
        if let Some((counting, writes)) = &counting {
            let counted = CountedDraw {
                counting,
                alpha_to_coverage,
                writes_depth_stencil: *writes,
                instances_step_by_one,
            };
            let kept = with_call!(call => self.counter.draw(
                &mut self.backend,
                &mut self.passes,
                &call,
                &counted,
            ));
            self.gpu(kept)?;
        }
        if let Some((counting, off)) = &pixel_counting {
            let counted = CountedDraw {
                counting,
                alpha_to_coverage: false,
                writes_depth_stencil: false,
                instances_step_by_one,
            };
            let kept = with_call!(call => self.pixel_counter.draw(
                &mut self.backend,
                &mut self.passes,
                &DrawCall { depth_stencil: &off.gpu.handle, ..call },
                &counted,
            ));
            self.gpu(kept)?;
        }
        self.open_pass();
        if !self.open_stretch() {
            // The pass's occlusion set has no slot left: the draw opens the next pass on another.
            self.end_pass();
            self.open_pass();
            self.open_stretch();
        }
        with_call!(call => self.backend.draw(&call));
        Ok(())
    }

    /// `CopyResource`: two resources of one shape.
    pub fn copy_resource(&mut self, dst: &Resource<B>, src: &Resource<B>) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        match (dst, src) {
            (Resource::Buffer(d), Resource::Buffer(s)) => {
                if d.desc.byte_width != s.desc.byte_width || Rc::ptr_eq(d, s) {
                    return Err(Error::InvalidArg(
                        "CopyResource needs two resources of one size",
                    ));
                }
                self.settle();
                d.writes.set(d.writes.get() + 1);
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
                // Measured in blocks: a block-compressed texture copies with an uncompressed one
                // whose texels are its blocks.
                let shape = |t: &TextureDesc, f: Format| {
                    let (bw, bh, _) = f.block();
                    (
                        t.dimension,
                        t.width.div_ceil(bw),
                        t.height.div_ceil(bh),
                        t.depth,
                        t.mip_levels,
                        t.array_size,
                        t.sample_count,
                    )
                };
                if shape(a, d.format) != shape(b, s.format) || Rc::ptr_eq(d, s) {
                    return Err(Error::InvalidArg(
                        "CopyResource needs two textures of one shape",
                    ));
                }
                copy_formats(a.format, b.format, self.feature_level())?;
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
        if self.predicated_out() {
            return Ok(());
        }
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
                d.writes.set(d.writes.get() + 1);
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
                copy_formats(d.desc.format, s.desc.format, self.feature_level())?;
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
                // The destination's extent covers the same blocks; a block-compressed level is
                // as large as its whole blocks.
                let ((sbw, sbh, _), (dbw, dbh, _)) = (s.format.block(), d.format.block());
                if at[0] + extent.width.div_ceil(sbw) * dbw > de.width.div_ceil(dbw) * dbw
                    || at[1] + extent.height.div_ceil(sbh) * dbh > de.height.div_ceil(dbh) * dbh
                    || at[2] + extent.depth_or_layers > de.depth_or_layers
                {
                    return Ok(());
                }
                // Subresources of depth-stencil textures are copied whole; Direct3D drops a part.
                if (s.desc.bind_flags | d.desc.bind_flags) & desc::bind::DEPTH_STENCIL != 0
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

    /// `ResolveSubresource`: the samples of a multisampled subresource averaged into a
    /// single-sampled subresource of its size, both read as `format`.
    pub fn resolve_subresource(
        &mut self,
        dst: &Resource<B>,
        dst_sub: u32,
        src: &Resource<B>,
        src_sub: u32,
        format: u32,
    ) -> Result<()> {
        if self.predicated_out() {
            return Ok(());
        }
        let (Resource::Texture(d), Resource::Texture(s)) = (dst, src) else {
            return Err(Error::InvalidArg("a resolve between buffers"));
        };
        let (dm, sm) = (d.desc.mip_levels, s.desc.mip_levels);
        if s.desc.sample_count < 2
            || d.desc.sample_count != 1
            || dst_sub >= dm * d.desc.array_size
            || src_sub >= sm * s.desc.array_size
        {
            return Err(Error::InvalidArg(
                "a resolve of a multisampled subresource into a single-sampled one",
            ));
        }
        let (de, se) = (d.mip_extent(dst_sub % dm), s.mip_extent(src_sub % sm));
        if (de.width, de.height) != (se.width, se.height) {
            return Err(Error::InvalidArg(
                "a resolve between subresources of two sizes",
            ));
        }
        let (into, _) = format::view_format(format, d.desc.format, d.format)?;
        format::view_format(format, s.desc.format, s.format)?;
        if !resolvable(into) {
            return Err(Error::InvalidArg(
                "a resolve in a format that does not average",
            ));
        }
        self.settle();
        self.backend.resolve(
            &s.region(src_sub % sm, src_sub / sm, Origin { x: 0, y: 0, z: 0 }),
            &d.region(dst_sub % dm, dst_sub / dm, Origin { x: 0, y: 0, z: 0 }),
            into,
        );
        self.texture_written(d, dst_sub)
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
        if self.predicated_out() {
            return Ok(());
        }
        self.before_write();
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
                b.writes.set(b.writes.get() + 1);
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
                    let shadow = &mut b.shadow.borrow_mut();
                    self.refresh(&mut staged, &src, None, shadow, reads, wait)?;
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
                let stencil = t.stencil_region(sub);
                let mut shadow = t.shadow.borrow_mut();
                let mut staged = t.staged.borrow_mut();
                let s = sub as usize;
                // A dynamic texture's shadow is the program's to fill; nothing is read back.
                if d.usage == usage::STAGING {
                    let stencil =
                        stencil.map(|(region, extent)| ReadSource::Texture { region, extent });
                    let texel = t.format.block().2 as usize;
                    let stencil = stencil.as_ref().map(|s| (s, texel));
                    self.refresh(&mut staged[s], &src, stencil, &mut shadow[s], reads, wait)?;
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
            Staged::Reading(self.gpu(r)?, None)
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
            let r = self.gpu(r)?;
            let stencil = match t.stencil_region(sub) {
                Some((region, extent)) => {
                    let s = self
                        .backend
                        .read_back(&ReadSource::Texture { region, extent });
                    Some(self.gpu(s)?)
                }
                None => None,
            };
            Staged::Reading(r, stencil)
        } else {
            Staged::Written
        };
        t.staged.borrow_mut()[sub as usize] = state;
        Ok(())
    }

    /// Brings a staging subresource's shadow up to the GPU's bytes before a map hands it out: the
    /// read back started at the copy, or one started now from `src` (and from `stencil`, the stencil
    /// aspect of a format with depth and stencil, with the bytes of the shadow's texels); a write-only map that needs it is
    /// `StagingRefreshed`. Without `wait`, a read back not yet done answers `StillDrawing` (after
    /// submitting, so it gets done).
    fn refresh(
        &mut self,
        staged: &mut Staged<B>,
        src: &ReadSource<'_, B>,
        stencil: Option<(&ReadSource<'_, B>, usize)>,
        shadow: &mut [u8],
        reads: bool,
        wait: bool,
    ) -> Result<()> {
        let (rb, stencil_rb) = match std::mem::replace(staged, Staged::Current) {
            Staged::Current => return Ok(()),
            Staged::Written => {
                self.settle();
                let r = self.backend.read_back(src);
                let r = self.gpu(r)?;
                let s = match stencil {
                    Some((src, _)) => {
                        let s = self.backend.read_back(src);
                        Some(self.gpu(s)?)
                    }
                    None => None,
                };
                (r, s)
            }
            Staged::Reading(rb, s) => (rb, s),
        };
        let pending = self.backend.read_state(&rb) == ReadState::Pending
            || stencil_rb
                .as_ref()
                .is_some_and(|s| self.backend.read_state(s) == ReadState::Pending);
        if pending {
            if !wait {
                *staged = Staged::Reading(rb, stencil_rb);
                self.flush();
                return Err(Error::StillDrawing);
            }
            self.backend.slow_paths().hit(SlowPath::ReadBackWait);
            self.backend.wait_read(&rb);
            if let Some(s) = &stencil_rb {
                self.backend.wait_read(s);
            }
        }
        if !reads {
            self.backend.slow_paths().hit(SlowPath::StagingRefreshed);
        }
        match stencil_rb {
            None => self.backend.finish_read(rb, shadow),
            Some(s) => {
                // Direct3D's layouts: D24_UNORM_S8_UINT is depth in the low 24 bits of a word and
                // stencil in its high byte; D32_FLOAT_S8X24_UINT is the float, the stencil byte and
                // three unused bytes. The back end gives depth in four bytes and stencil in one.
                let texel = stencil.map_or(4, |s| s.1);
                let n = shadow.len() / texel;
                let mut depth = vec![0u8; n * 4];
                let mut stencil = vec![0u8; n];
                self.backend.finish_read(rb, &mut depth);
                self.backend.finish_read(s, &mut stencil);
                for i in 0..n {
                    let t = &mut shadow[i * texel..(i + 1) * texel];
                    if texel == 4 {
                        t[..3].copy_from_slice(&depth[i * 4..i * 4 + 3]);
                        t[3] = stencil[i];
                    } else {
                        t[..4].copy_from_slice(&depth[i * 4..i * 4 + 4]);
                        t[4] = stencil[i];
                        t[5..].fill(0);
                    }
                }
            }
        }
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
        self.before_write();
        self.backend.slow_paths().hit(SlowPath::MappedUploadWhole);
        match resource {
            Resource::Buffer(b) => {
                b.writes.set(b.writes.get() + 1);
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
                let shadow = &t.shadow.borrow()[sub as usize];
                let Some((stencil, _)) = t.stencil_region(sub) else {
                    self.backend.write_texture(
                        &region,
                        extent,
                        shadow,
                        row,
                        rows,
                        WriteMode::Ordered,
                    );
                    return;
                };
                // Each aspect of a format with depth and stencil apart, from Direct3D's layout
                // (see `refresh`): depth in four bytes per texel, stencil in one.
                let texel = t.format.block().2;
                let (depth, stencils) = split_depth_stencil(texel, shadow);
                let texels = row / texel;
                self.backend.write_texture(
                    &region,
                    extent,
                    &depth,
                    texels * 4,
                    rows,
                    WriteMode::Ordered,
                );
                self.backend.write_texture(
                    &stencil,
                    extent,
                    &stencils,
                    texels,
                    rows,
                    WriteMode::Ordered,
                );
            }
        }
    }

    /// `GenerateMips`: fills the view's levels below its first from the first, in its format. A
    /// texture made without mip generation is left alone, as Direct3D leaves it.
    pub fn generate_mips(&mut self, view: &View<B>) {
        if self.predicated_out() {
            return;
        }
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

/// The same range again, for a draw's call built twice.
fn same_range<'a, B: Backend>(r: &DrawRange<'a, B>) -> DrawRange<'a, B> {
    match *r {
        DrawRange::Vertices {
            first,
            count,
            first_instance,
            instances,
        } => DrawRange::Vertices {
            first,
            count,
            first_instance,
            instances,
        },
        DrawRange::Indexed {
            first,
            count,
            base_vertex,
            first_instance,
            instances,
        } => DrawRange::Indexed {
            first,
            count,
            base_vertex,
            first_instance,
            instances,
        },
        DrawRange::Indirect { args, offset } => DrawRange::Indirect { args, offset },
        DrawRange::IndexedIndirect { args, offset } => DrawRange::IndexedIndirect { args, offset },
    }
}
