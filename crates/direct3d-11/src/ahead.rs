//! Building ahead (plan 4.4): the state tuples each vertex shader is drawn with, counted per draw, and
//! the pipelines of a new pair of shaders prepared with them before the pair's first draw.

use crate::error::WARNING_PREFIX;
use std::cmp::Ordering;
use std::rc::{Rc, Weak};

use wcontainers_gpu::{
    BindingLayoutKey, FORMAT_FACTS, Format, Gpu, MAX_BINDING_SETS, PrepareDesc, ScalarKind,
    SlowPath, SystemValue, TargetLayout, Topology,
};
use wcontainers_shaders::front::{Combination, CombinationTracker};

use crate::cache::Record;
use crate::context::VERTEX_SLOTS;
use crate::device::Device;
use crate::object::{Blend, DepthStencil, InputLayout, Rasterizer, Shader};
use crate::{PIXEL, VERTEX};

/// What a pipeline is built from besides its program: state objects, the input layout and the
/// strides it is read at, the targets, the topology and the binding sets' layout keys. It holds its
/// objects, so a tuple picked later can still be prepared.
pub(crate) struct Tuple<B: Gpu> {
    blend: Rc<Blend<B>>,
    depth_stencil: Rc<DepthStencil<B>>,
    raster: Rc<Rasterizer<B>>,
    layout: Option<Rc<InputLayout>>,
    strides: [u32; VERTEX_SLOTS],
    targets: TargetLayout,
    topology: Topology,
    layouts: [BindingLayoutKey; MAX_BINDING_SETS],
}

/// The tuple's objects (blend, depth stencil, rasterizer, input layout ids), then its other parts.
type TupleKey = (
    (u64, u64, u64, u64),
    [u32; VERTEX_SLOTS],
    [Option<Format>; wcontainers_gpu::MAX_COLOR_TARGETS],
    Option<Format>,
    u32,
    u8,
    [u128; MAX_BINDING_SETS],
);

impl<B: Gpu> Tuple<B> {
    fn key(&self) -> TupleKey {
        (
            (
                self.blend.gpu.id,
                self.depth_stencil.gpu.id,
                self.raster.gpu.id,
                self.layout.as_ref().map_or(0, |l| l.id),
            ),
            self.strides,
            self.targets.colors,
            self.targets.depth_stencil,
            self.targets.samples,
            self.topology as u8,
            self.layouts.map(|k| k.unfilterable_textures),
        )
    }
}

impl<B: Gpu> Clone for Tuple<B> {
    fn clone(&self) -> Self {
        Tuple {
            blend: self.blend.clone(),
            depth_stencil: self.depth_stencil.clone(),
            raster: self.raster.clone(),
            layout: self.layout.clone(),
            strides: self.strides,
            targets: self.targets,
            topology: self.topology,
            layouts: self.layouts,
        }
    }
}

impl<B: Gpu> PartialEq for Tuple<B> {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl<B: Gpu> Eq for Tuple<B> {}

impl<B: Gpu> PartialOrd for Tuple<B> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<B: Gpu> Ord for Tuple<B> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

/// The last draw's combination, and the binding sets its layout keys were read from.
struct Last<B: Gpu> {
    program: (u64, u64),
    sets: [u64; 2],
    tuple: Tuple<B>,
    combination: Combination,
}

/// What `report_ahead` reports: prepared combinations drawn and not, combinations built at their
/// first draw, and recorded combinations built at start and drawn since.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct UseCounts {
    drawn: u64,
    not: u64,
    missed: u64,
    started: u64,
    started_drawn: u64,
}

/// The device's record of combinations drawn and prepared.
pub(crate) struct Ahead<B: Gpu> {
    /// Keyed by vertex shader id, program (vertex and pixel shader ids) and tuple.
    tracker: CombinationTracker<u64, (u64, u64), Tuple<B>>,
    last: Option<Last<B>>,
    /// The shaders made most recently of each stage, which a new shader of the other is paired with.
    last_vertex: Weak<Shader<B>>,
    last_pixel: Weak<Shader<B>>,
    /// Combinations whose record was written.
    recorded: std::collections::HashSet<Combination>,
    /// The counts last reported.
    reported: UseCounts,
}

impl<B: Gpu> Ahead<B> {
    pub fn new() -> Self {
        Ahead {
            tracker: CombinationTracker::default(),
            last: None,
            last_vertex: Weak::new(),
            last_pixel: Weak::new(),
            recorded: Default::default(),
            reported: UseCounts::default(),
        }
    }

    /// How many prepared combinations were drawn since, and how many were not.
    pub fn prepared_use(&self) -> (u64, u64) {
        self.tracker.prepared_use()
    }
}

/// The vertex shader writes every value the pixel shader reads, at its location, with its type and
/// components: a pair a pipeline can be built from.
fn covers<B: Gpu>(vs: &Shader<B>, ps: &Shader<B>) -> bool {
    ps.reflection
        .inputs
        .iter()
        .filter(|i| i.system.is_none())
        .all(|i| {
            vs.reflection.outputs.iter().any(|o| {
                o.system.is_none()
                    && o.location == i.location
                    && o.scalar == i.scalar
                    && o.mask & i.mask == i.mask
            })
        })
}

/// Whether the pixel shader's colour outputs are of the kinds (float, unsigned or signed) of
/// the formats `target` gives for each target: a pipeline with targets of another kind is refused
/// by the back ends.
pub(crate) fn writes<B: Gpu>(
    ps: &Shader<B>,
    target: impl Fn(usize) -> Option<wcontainers_gpu::Format>,
) -> bool {
    ps.reflection.outputs.iter().all(|o| {
        let Some(SystemValue::Target(n)) = o.system else {
            return true;
        };
        target(usize::from(n)).is_none_or(|f| fits_target(o.scalar, f))
    })
}

/// Whether an output of `scalar`'s kind is of the kind (float, unsigned or signed) of `format`.
pub(crate) fn fits_target(scalar: ScalarKind, format: wcontainers_gpu::Format) -> bool {
    let kind = match FORMAT_FACTS[format as usize].kind {
        wcontainers_gpu::FormatKind::Uint => ScalarKind::Uint,
        wcontainers_gpu::FormatKind::Sint => ScalarKind::Sint,
        _ => ScalarKind::Float,
    };
    kind == scalar
}

/// The input layout gives the vertex shader every input it reads, at its location and of its type.
fn fits<B: Gpu>(vs: &Shader<B>, layout: Option<&InputLayout>) -> bool {
    vs.reflection
        .inputs
        .iter()
        .filter(|i| i.system.is_none())
        .all(|i| {
            layout.is_some_and(|l| {
                l.elements.iter().any(|e| {
                    e.location == i.location
                        && crate::format::vertex_scalar(crate::format::vertex_format_read_as(
                            e.format, i.scalar,
                        )) == i.scalar
                })
            })
        })
}

impl<B: Gpu> Device<B> {
    /// How many combinations built ahead were drawn since, and how many were not.
    pub fn prepared_use(&self) -> (u64, u64) {
        self.ahead.prepared_use()
    }

    /// How many recorded combinations this device built at the program's start, and how many of
    /// them were drawn since.
    pub fn started_use(&self) -> (u64, u64) {
        self.warm.used()
    }

    /// Counts a draw of program `vs` and `ps` (made of the modules `keys`) with the bound state and
    /// the raster twin `masked`; a combination drawn the first time is added to the cache's record.
    /// The tuple is looked up only when something in it changed since the last draw.
    pub(crate) fn count_combination(
        &mut self,
        vs: &Shader<B>,
        ps: Option<&Rc<Shader<B>>>,
        keys: crate::cache::ModuleKeys,
        masked: Option<crate::context::RasterKey>,
        topology: Topology,
        samples: u32,
    ) {
        let program = (vs.gpu.id, ps.map_or(0, |p| p.gpu.id));
        let ctx = &self.ctx;
        let blend = ctx.blend.as_ref().unwrap_or(&self.defaults.blend);
        let depth_stencil = &self.drawn_depth_stencil();
        let raster = ctx.raster.as_ref().unwrap_or(&self.defaults.raster);
        let set_id = |stage: usize| ctx.sets[stage].as_ref().map_or(0, |k| k.id);
        let sets = [set_id(VERTEX), set_id(PIXEL)];
        let strides = self.strides();
        let mut targets = TargetLayout {
            colors: [None; wcontainers_gpu::MAX_COLOR_TARGETS],
            depth_stencil: ctx.depth.as_ref().map(|d| d.format),
            samples,
        };
        for (c, t) in targets.colors.iter_mut().zip(&ctx.targets) {
            *c = t.as_ref().map(|v| v.format);
        }
        if let Some(last) = &self.ahead.last
            && last.program == program
            && last.sets == sets
            && Rc::ptr_eq(&last.tuple.blend, blend)
            && Rc::ptr_eq(&last.tuple.depth_stencil, depth_stencil)
            && Rc::ptr_eq(&last.tuple.raster, raster)
            && last.tuple.layout.as_ref().map(Rc::as_ptr) == ctx.layout.as_ref().map(Rc::as_ptr)
            && last.tuple.strides == strides
            && last.tuple.targets == targets
            && last.tuple.topology == topology
        {
            let c = last.combination;
            self.ahead.tracker.drawn(c);
            return;
        }
        let key = |stage: usize| {
            ctx.sets[stage]
                .as_ref()
                .map_or(BindingLayoutKey::default(), |k| {
                    self.backend.binding_layout_key(&k.handle)
                })
        };
        let mut layouts = [BindingLayoutKey::default(); MAX_BINDING_SETS];
        layouts[0] = key(VERTEX);
        layouts[1] = key(PIXEL);
        let tuple = Tuple {
            blend: blend.clone(),
            depth_stencil: depth_stencil.clone(),
            raster: raster.clone(),
            layout: ctx.layout.clone(),
            strides,
            targets,
            topology,
            layouts,
        };
        let c = self.ahead.tracker.combination(&vs.gpu.id, &program, &tuple);
        self.ahead.tracker.drawn(c);
        if self.ahead.recorded.insert(c) {
            let record = Record::new(
                keys,
                crate::device::blend_block(&tuple.blend.desc),
                crate::device::depth_stencil_block(&tuple.depth_stencil.desc),
                self.raster_desc(masked),
                crate::context::layout_desc(
                    tuple.layout.as_deref(),
                    tuple.strides,
                    &vs.reflection.inputs,
                ),
                tuple.targets,
                topology,
                tuple.layouts,
            );
            crate::cache::record(&record);
            if let Some(drawn) = self.warm.records.get_mut(&record) {
                *drawn = true;
            }
        }
        self.ahead.last = Some(Last {
            program,
            sets,
            tuple,
            combination: c,
        });
    }

    /// Pairs a new vertex or pixel shader with the latest shader of the other stage whose values
    /// match it, and prepares the pair's pipelines with the tuples the tracker picks: those its
    /// vertex shader was drawn with, or the most drawn one its inputs fit.
    pub(crate) fn build_ahead(&mut self, new: &Rc<Shader<B>>, vertex: bool) {
        let (vs, ps) = if vertex {
            self.ahead.last_vertex = Rc::downgrade(new);
            (Some(new.clone()), self.ahead.last_pixel.upgrade())
        } else {
            self.ahead.last_pixel = Rc::downgrade(new);
            (self.ahead.last_vertex.upgrade(), Some(new.clone()))
        };
        let (Some(vs), Some(ps)) = (vs, ps) else {
            return;
        };
        if !covers(&vs, &ps) {
            return;
        }
        let program_key = (vs.gpu.id, ps.gpu.id);
        let picked = self
            .ahead
            .tracker
            .pick(&vs.gpu.id, &program_key, |t| fits(&vs, t.layout.as_deref()));
        if picked.is_empty() {
            return;
        }
        // A pair the back end cannot build is left to its first draw, which reports it.
        let vk = self.unbound_variant(crate::VERTEX, &vs);
        let pk = self.unbound_variant(crate::PIXEL, &ps);
        let Ok((program, _)) = self.program(&vs, vk, Some(&ps), pk) else {
            return;
        };
        for t in picked
            .into_iter()
            .filter(|t| writes(&ps, |n| t.targets.colors.get(n).copied().flatten()))
        {
            let Ok(vertex_layout) = self.vertex_layout_for(t.layout.clone(), t.strides, &vs) else {
                return;
            };
            self.backend.prepare(&PrepareDesc {
                program: &program,
                blend: &t.blend.gpu.handle,
                depth_stencil: &t.depth_stencil.gpu.handle,
                raster: &t.raster.gpu.handle,
                vertex_layout: &vertex_layout,
                targets: t.targets,
                topology: t.topology,
                layouts: t.layouts,
            });
        }
    }
}

impl<B: Gpu> Device<B> {
    /// Reports how the pipelines built ahead were used (plan 4.4), beside the combinations the
    /// back end built at their first draw, where these changed since the last report: at each
    /// present, since programs may end without releasing the device.
    pub(crate) fn report_ahead(&mut self) {
        let (drawn, not) = self.ahead.prepared_use();
        let missed = self.backend.slow_paths().counts[SlowPath::PipelineMiss as usize];
        let (started, started_drawn) = self.warm.used();
        let counts = UseCounts {
            drawn,
            not,
            missed,
            started,
            started_drawn,
        };
        if counts == self.ahead.reported {
            return;
        }
        self.ahead.reported = counts;
        eprintln!(
            "{WARNING_PREFIX}: built at start {started} recorded combinations: \
             {started_drawn} drawn since, {} not ({} records not built); built ahead {} \
             combinations: {drawn} drawn since, {not} not; {missed} built at their first draw",
            started - started_drawn,
            self.warm.unbuilt,
            drawn + not
        );
    }
}

impl<B: Gpu> Drop for Device<B> {
    fn drop(&mut self) {
        self.report_ahead();
    }
}
