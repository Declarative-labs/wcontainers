//! The WebGPU back end of the graphics interface, on `wgpu`: natively (Direct3D 12, Vulkan, Metal) for
//! tests and the Windows libraries, and on the browser's WebGPU when built for WebAssembly.
//!
//! Made from the `wgpu` device it drives ([`WebGpu::new`]), so the features the device was made with
//! decide what it declares. Blocks are plain data; a draw's combination of blocks, target formats,
//! topology and layout keys names a cached pipeline, and binding sets become cached bind groups whose
//! uniform buffers take dynamic offsets (plan sections 4.4 and 4.5).

// The package waits on its tests to inherit the workspace's ban; the back end itself is held to it now.
#![forbid(unsafe_code)]

mod blocks;
mod convert;
mod draw;
mod edge;
mod fill;
mod layout;
mod memory;
mod packed;
mod probe;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod unit_tests;
mod vertex;
mod wait;

// The unit tests include modules the scenes page and the integration tests share, which name the
// crate by its package name.
#[cfg(all(test, not(target_arch = "wasm32")))]
extern crate self as wcontainers_gpu_webgpu;

use crate::draw::{
    begin_pass_impl, binding_set, close_pass, dispatch_impl, draw_impl, prepare_impl, resolve_impl,
    split_onto_page,
};
use crate::fill::present_by_draw;
use crate::memory::{
    arena_slice, clear_buffer_impl, copy_buffer_impl, copy_buffer_texture, copy_texture_impl,
    finish_read_impl, finish_read_into_impl, give_back_spares, give_back_texture_spares,
    read_back_impl, read_queries_impl, reclaim_spares, release_slice, resolve_queries_impl,
    write_buffer_from_impl, write_buffer_impl, write_texture_impl,
};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use wcontainers_volatile_bytes::VolatileBytes;

use wcontainers_gpu::{
    Gpu, MAX_BINDING_SETS, Object, StandaloneGpu, caps::Caps, caps::Features, caps::FormatCaps,
    caps::FormatUse, caps::Limits, draw::PassDesc, error::DeviceStatus, error::GpuError,
    error::LossReason, format::FORMAT_COUNT, format::FORMAT_FACTS, format::Format,
    format::FormatKind, helpers::Helpers, resource::BindingEntry, resource::BindingLayoutKey,
    resource::BufferDesc, resource::BufferUsage, resource::CopyBufferTexture, resource::Extent,
    resource::Filter, resource::MapIntent, resource::QueryKind, resource::ReadSource,
    resource::ReadState, resource::SamplerDesc, resource::TextureDesc, resource::TextureDimension,
    resource::TextureRegion, resource::TextureUsage, resource::TextureViewDesc,
    resource::WriteMode, shader::Program, shader::ShaderModule, shader::ShaderStage,
    slow::SLOW_PATHS, slow::SlowPaths, state::BlendDesc, state::DepthStencilDesc, state::FillMode,
    state::ProvokingVertex, state::RasterDesc, state::StepMode, state::VertexLayoutDesc,
};

use draw::Counters;
use probe::Depth24;
#[cfg(target_arch = "wasm32")]
pub use wait::block_on;

/// Readback states, set by the map callback.
const MAP_PENDING: u8 = 0;
const MAP_READY: u8 = 1;
const MAP_FAILED: u8 = 2;

/// The WebGPU back end. One thread owns it.
pub struct WebGpu {
    /// Asked which formats a window takes.
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    caps: Caps,
    slow: SlowPaths,
    helpers: Helpers<WebGpu>,
    /// Set by wgpu's device-lost and uncaptured-error callbacks, and by the back end on a rule it
    /// cannot carry out.
    lost: Arc<Mutex<Option<LossReason>>>,
    next_id: u64,
    /// The number of the command encoder being recorded; [`Gpu::submit`] moves to the next.
    serial: u64,
    /// The highest serial whose work the GPU has finished.
    completed: Arc<AtomicU64>,
    encoder: Option<wgpu::CommandEncoder>,
    /// Submitted ahead of `encoder`: writes that take effect at once but that the queue cannot make
    /// (bytes not in whole words), in call order.
    lead: Option<wgpu::CommandEncoder>,
    pass: Option<draw::Pass>,
    /// Read backs recorded in the current encoder, mapped once it is submitted.
    to_map: Vec<HeldReadback>,
    /// What every handle given out names.
    objects: Objects,
    /// Destroyed objects the current encoder still uses, destroyed once it is submitted.
    destroy_after_submit: Vec<Doomed>,
    blocks: draw::BlockTables,
    caches: draw::Caches,
    pool: memory::Pool,
    texture_pool: memory::Pool,
    arena: Option<memory::Arena>,
    ring: Option<memory::Ring>,
    /// The ring resolved query results are taken from.
    query_ring: Option<memory::Ring>,
    edge: Option<edge::EdgeProgram>,
    /// The program adding the counts of queries carried across pass splits.
    query_sum: Option<(wgpu::ComputePipeline, wgpu::BindGroupLayout)>,
    vertices: vertex::Vertices,
    fills: fill::Fills,
    /// What the probe of how `depth24plus` is stored found, which the unit tests read.
    #[cfg(test)]
    depth24: Depth24,
    /// Whether the device samples stored 24-bit depth as a product with a rounded reciprocal rather than as the
    /// nearest float ([`Depth24::sampled_by_product`]), which the 24-bit depth read checks against.
    depth24_by_product: bool,
    /// The stored 24-bit words this device samples to another word's float ([`fill::colliding`]),
    /// which a 24-bit depth read decides by comparison passes; empty where none is (no pass).
    depth24_colliding: Vec<u32>,
    /// Colour clears reach the API as floats, so integers past 2^24 cannot be cleared exactly.
    float_clears: bool,
    /// Texture reads into a buffer at an offset off Direct3D 12's placement alignment overwrite
    /// the bytes between their rows (wgpu's Direct3D 12 copies them through a buffer of its own
    /// and then copies that whole, gaps included, where the driver lacks unrestricted pitches).
    padded_reads: bool,
    /// Frames presented, for [`FRAME_REPORT`].
    presented: u64,
    /// Changed whenever a buffer a binding set names moves so that a bind group made with it may no
    /// longer be right; draws rebind nothing while it stays the same.
    bind_epoch: u64,
    /// Host windows the graphics hook registered, by the number it gives them.
    windows: HashMap<u32, wgpu::Surface<'static>>,
    counters: Counters,
    /// The texture usages each format allows on this device, by `Format as usize`.
    usages: [wgpu::TextureUsages; FORMAT_COUNT],
    /// Whether a resolve of each format is a draw averaging its samples, by `Format as usize`.
    resolved_by_draw: [bool; FORMAT_COUNT],
}

#[derive(Debug)]
enum Doomed {
    Buffer(wgpu::Buffer),
    Texture(wgpu::Texture),
}

macro_rules! handle {
    ($(#[$m:meta])* $name:ident, $held:ident, $obj:ty) => {
        $(#[$m])*
        ///
        /// Plain data: the number the back end keeps the object under until it is destroyed.
        #[derive(Clone, Debug)]
        pub struct $name(pub u64);

        /// What a handle names, as the back end holds it: equal when it is the same object.
        #[derive(Clone, Debug)]
        pub(crate) struct $held(pub(crate) Rc<$obj>);

        impl PartialEq for $held {
            fn eq(&self, other: &Self) -> bool {
                Rc::ptr_eq(&self.0, &other.0)
            }
        }

        impl Eq for $held {}
    };
}

handle!(
    /// A buffer; its storage may be renamed by `Discard` writes.
    Buffer,
    HeldBuffer,
    BufferObj
);
handle!(Texture, HeldTexture, TextureObj);
handle!(TextureView, HeldView, ViewObj);
handle!(Sampler, HeldSampler, SamplerObj);
handle!(Shader, HeldShader, ShaderObj);
handle!(ProgramHandle, HeldProgram, ProgramObj);
handle!(Blend, HeldBlend, draw::Block<BlendDesc>);
handle!(
    DepthStencil,
    HeldDepthStencil,
    draw::Block<DepthStencilDesc>
);
handle!(Raster, HeldRaster, draw::Block<RasterDesc>);
handle!(
    VertexLayout,
    HeldVertexLayout,
    draw::Block<VertexLayoutDesc>
);
handle!(BindingSet, HeldSet, draw::SetObj);
handle!(QuerySet, HeldQuerySet, QuerySetObj);
handle!(Readback, HeldReadback, ReadbackObj);
handle!(Surface, HeldSurface, SurfaceObj);

/// Answers the object a handle's number names in `table`, while it is not destroyed.
fn held<H: Clone>(table: &HashMap<u64, H>, number: u64) -> Option<H> {
    table.get(&number).cloned()
}

fn buffer_of(gpu: &WebGpu, b: &Buffer) -> Option<Rc<BufferObj>> {
    held(&gpu.objects.buffers, b.0).map(|h| h.0)
}

fn texture_of(gpu: &WebGpu, t: &Texture) -> Option<Rc<TextureObj>> {
    held(&gpu.objects.textures, t.0).map(|h| h.0)
}

fn view_of(gpu: &WebGpu, v: &TextureView) -> Option<Rc<ViewObj>> {
    held(&gpu.objects.views, v.0).map(|h| h.0)
}

fn program_of(gpu: &WebGpu, p: &ProgramHandle) -> Option<Rc<ProgramObj>> {
    held(&gpu.objects.programs, p.0).map(|h| h.0)
}

fn set_of(gpu: &WebGpu, s: &BindingSet) -> Option<Rc<draw::SetObj>> {
    held(&gpu.objects.sets, s.0).map(|h| h.0)
}

fn query_set_of(gpu: &WebGpu, q: &QuerySet) -> Option<Rc<QuerySetObj>> {
    held(&gpu.objects.query_sets, q.0).map(|h| h.0)
}

fn readback_of(gpu: &WebGpu, r: &Readback) -> Option<Rc<ReadbackObj>> {
    held(&gpu.objects.readbacks, r.0).map(|h| h.0)
}

fn surface_of(gpu: &WebGpu, s: &Surface) -> Option<Rc<SurfaceObj>> {
    held(&gpu.objects.surfaces, s.0).map(|h| h.0)
}

/// Takes a texture the back end made for itself out of the table; recorded work keeps its hold.
fn forget_texture(gpu: &mut WebGpu, t: &Texture) -> Option<Rc<TextureObj>> {
    gpu.objects.textures.remove(&t.0).map(|h| h.0)
}

/// The objects the handles name, by their numbers, from their making to their destroying.
#[derive(Debug, Default)]
struct Objects {
    buffers: HashMap<u64, HeldBuffer>,
    textures: HashMap<u64, HeldTexture>,
    views: HashMap<u64, HeldView>,
    samplers: HashMap<u64, HeldSampler>,
    shaders: HashMap<u64, HeldShader>,
    programs: HashMap<u64, HeldProgram>,
    blends: HashMap<u64, HeldBlend>,
    depth_stencils: HashMap<u64, HeldDepthStencil>,
    rasters: HashMap<u64, HeldRaster>,
    vertex_layouts: HashMap<u64, HeldVertexLayout>,
    sets: HashMap<u64, HeldSet>,
    query_sets: HashMap<u64, HeldQuerySet>,
    readbacks: HashMap<u64, HeldReadback>,
    surfaces: HashMap<u64, HeldSurface>,
}

#[derive(Debug)]
struct BufferObj {
    id: u64,
    desc: BufferDesc,
    usage: wgpu::BufferUsages,
    /// The GPU buffer holding its bytes: its own, or the uniform arena's.
    raw: RefCell<wgpu::Buffer>,
    /// Where its bytes start in `raw`: zero unless it lives in the arena.
    base: Cell<u64>,
    /// A number for `raw` that no other GPU buffer had, changed when `raw` is.
    storage: Cell<u64>,
    /// Bumped whenever `raw` or `base` changes, so bindings naming the old place are made again.
    generation: Cell<u64>,
    /// It lives in the uniform arena: uniform-only and written often.
    in_arena: bool,
    /// A binding set names it, so a change of `raw` may change bind groups.
    in_sets: Cell<bool>,
    /// A bind group names it at a fixed offset, so a change of `base` changes that group.
    static_bound: Cell<bool>,
    /// The serial of the last encoder that named it.
    used_in: Cell<u64>,
    /// The serial of the last lead encoder that writes it: queue writes to it then go behind those.
    lead_in: Cell<u64>,
    /// Moved by every command that may write its bytes, so shadows made from them are made again.
    written: Cell<u64>,
    /// GPU buffers its renames made, destroyed from the pool when it is destroyed.
    spares: Cell<u64>,
}

impl BufferObj {
    /// Notes a command that may write its bytes.
    fn wrote(&self) {
        self.written.set(self.written.get() + 1);
    }
}

#[derive(Debug)]
struct TextureObj {
    desc: TextureDesc,
    /// The format the GPU texture holds: `desc.format`, a BC format's decoded one, or a converted
    /// format's wider one.
    held: Format,
    /// A decoded BC texture's blocks (`BlockDecoded`), per level and layer.
    blocks: Option<RefCell<Vec<Vec<u8>>>>,
    /// The GPU texture holding its texels, replaced when a `Discard` write renames it.
    raw: RefCell<wgpu::Texture>,
    /// Bumped whenever `raw` is replaced, so views made from the old one are made again.
    generation: Cell<u64>,
    used_in: Cell<u64>,
    /// GPU textures its renames made, destroyed from the pool when it is destroyed.
    spares: Cell<u64>,
}

impl TextureObj {
    fn raw(&self) -> wgpu::Texture {
        self.raw.borrow().clone()
    }
}

impl ViewObj {
    /// Makes the views again if the texture was renamed since they were made.
    fn fresh(&self) {
        if let (Some(t), Some(desc)) = (&self.texture, &self.desc)
            && t.0.generation.get() != self.made_for.get()
        {
            let views = raw_views(&t.0, desc, self.format_usages)
                .expect("made once from the same description");
            self.views.replace(views);
            self.made_for.set(t.0.generation.get());
        }
    }

    /// The view a pass draws into.
    fn raw(&self) -> wgpu::TextureView {
        self.fresh();
        self.views.borrow().0.clone()
    }

    /// The view a binding reads.
    fn read(&self) -> wgpu::TextureView {
        self.fresh();
        self.views.borrow().1.clone()
    }
}

#[derive(Debug)]
struct ViewObj {
    id: u64,
    format: Format,
    /// None for a surface's texture.
    texture: Option<HeldTexture>,
    /// Every aspect, what a pass draws into; and the aspect the view reads, what a binding names.
    views: RefCell<(wgpu::TextureView, wgpu::TextureView)>,
    /// What it was made with, to make it again once its texture is renamed; None for a surface's.
    desc: Option<TextureViewDesc>,
    /// The texture's generation `views` were made from.
    made_for: Cell<u64>,
    /// The usages the view's format allows on the device.
    format_usages: wgpu::TextureUsages,
    width: u32,
    height: u32,
    samples: u32,
    three_d: bool,
}

#[derive(Debug)]
struct SamplerObj {
    id: u64,
    raw: wgpu::Sampler,
    /// Any of its filters is linear.
    filtering: bool,
}

#[derive(Debug)]
struct ShaderObj {
    module: naga::Module,
    info: naga::valid::ModuleInfo,
    entry_index: usize,
    entry: String,
    stage: naga::ShaderStage,
    raw: wgpu::ShaderModule,
    /// Per set, the texture bits of the textures it samples through a sampler.
    paired: [u128; MAX_BINDING_SETS],
}

#[derive(Debug)]
enum ProgramObj {
    Render {
        id: u64,
        vertex: HeldShader,
        fragment: Option<HeldShader>,
    },
    Compute {
        id: u64,
        shader: HeldShader,
    },
}

impl ProgramObj {
    fn id(&self) -> u64 {
        match self {
            ProgramObj::Render { id, .. } | ProgramObj::Compute { id, .. } => *id,
        }
    }

    /// The bytes of immediates its stages declare, in whole words: what its layout holds.
    fn immediate_size(&self) -> u32 {
        let stages: [Option<&HeldShader>; 2] = match self {
            ProgramObj::Render {
                vertex, fragment, ..
            } => [Some(vertex), fragment.as_ref()],
            ProgramObj::Compute { shader, .. } => [Some(shader), None],
        };
        stages
            .into_iter()
            .flatten()
            .flat_map(|s| {
                let m = &s.0.module;
                m.global_variables
                    .iter()
                    .filter(|(_, g)| g.space == naga::AddressSpace::Immediate)
                    .map(|(_, g)| m.types[g.ty].inner.size(m.to_ctx()))
            })
            .max()
            .unwrap_or(0)
            .next_multiple_of(wgpu::IMMEDIATE_DATA_ALIGNMENT)
    }

    /// The layout key of each bound set, kept to the textures the program samples through a
    /// sampler: the only bits its layout depends on.
    fn layout_keys(
        &self,
        sets: &[Option<Rc<draw::SetObj>>; MAX_BINDING_SETS],
    ) -> [BindingLayoutKey; MAX_BINDING_SETS] {
        self.own_keys(std::array::from_fn(|i| {
            sets[i]
                .as_ref()
                .map_or_else(BindingLayoutKey::default, |s| s.key)
        }))
    }

    /// The parts of the sets' layout keys this program's layout depends on: the textures it samples
    /// through a sampler.
    fn own_keys(
        &self,
        sets: [BindingLayoutKey; MAX_BINDING_SETS],
    ) -> [BindingLayoutKey; MAX_BINDING_SETS] {
        let mut keys = [BindingLayoutKey::default(); MAX_BINDING_SETS];
        for shader in self.stages() {
            for ((k, s), paired) in keys.iter_mut().zip(&sets).zip(shader.paired) {
                k.unfilterable_textures |= s.unfilterable_textures & paired;
            }
        }
        keys
    }

    fn stages(&self) -> impl Iterator<Item = &ShaderObj> {
        let (first, second) = match self {
            ProgramObj::Render {
                vertex, fragment, ..
            } => (vertex, fragment.as_ref()),
            ProgramObj::Compute { shader, .. } => (shader, None),
        };
        std::iter::once(&*first.0).chain(second.map(|s| &*s.0))
    }
}

#[derive(Debug)]
struct QuerySetObj {
    kind: QueryKind,
    count: u32,
    /// WebGPU query sets of at most its 4096 queries. First the own pages: query i in slot
    /// i % 4096 of page i / 4096, the last going on with the spare slots occlusion queries
    /// carried across a split go on in. Then further pages of spare slots, made when those run
    /// out. A pass part begins a query in its own slot when it counts on that query's page, and
    /// otherwise in a spare slot of its page.
    pages: RefCell<Vec<wgpu::QuerySet>>,
    own_pages: usize,
    /// The parts of each query since it was last begun outside its own slot.
    segments: RefCell<HashMap<u32, Segments>>,
    /// Each page's free spare slots.
    free: RefCell<Vec<Vec<u32>>>,
    /// Slots given back in the open pass part, as (page, slot).
    released: RefCell<Vec<(usize, u32)>>,
}

/// Where a query counted since it was last begun: its own slot (unless `own` is false) plus
/// spare slots, as (page, slot).
#[derive(Debug, Default)]
struct Segments {
    own: bool,
    slots: Vec<(usize, u32)>,
}

impl QuerySetObj {
    /// Gives back the spare slots of query `index`, begun again, once the pass part ends (a
    /// slot is written once a pass).
    fn forget_segments(&self, index: u32) {
        if let Some(s) = self.segments.borrow_mut().remove(&index) {
            self.released.borrow_mut().extend(s.slots);
        }
    }

    /// Frees the slots given back while the pass part that just ended was open.
    fn settle(&self) {
        let mut free = self.free.borrow_mut();
        for (page, slot) in self.released.borrow_mut().drain(..) {
            free[page].push(slot);
        }
    }

    /// Takes a free spare slot of `page`.
    fn take(&self, page: usize) -> Option<u32> {
        self.free.borrow_mut()[page].pop()
    }

    /// The first page with a free spare slot, making a page of WebGPU's largest set if none has.
    fn page_with_room(&self, device: &wgpu::Device) -> usize {
        let mut free = self.free.borrow_mut();
        if let Some(p) = free.iter().position(|f| !f.is_empty()) {
            return p;
        }
        let n = wgpu::QUERY_SET_MAX_QUERIES;
        self.pages
            .borrow_mut()
            .push(device.create_query_set(&wgpu::QuerySetDescriptor {
                label: None,
                ty: wgpu::QueryType::Occlusion,
                count: n,
            }));
        free.push((0..n).rev().collect());
        free.len() - 1
    }

    /// The WebGPU query set of `page`.
    fn page(&self, page: usize) -> wgpu::QuerySet {
        self.pages.borrow()[page].clone()
    }

    /// The own page and slot of query `index`.
    fn own_slot(index: u32) -> (usize, u32) {
        let max = wgpu::QUERY_SET_MAX_QUERIES;
        ((index / max) as usize, index % max)
    }

    /// Records that query `index` goes on in `slot` of `page`; `first` when this begins it.
    fn carry(&self, index: u32, page: usize, slot: u32, first: bool) {
        let mut segments = self.segments.borrow_mut();
        let s = segments.entry(index).or_default();
        if first {
            s.own = false;
        } else if s.slots.is_empty() {
            s.own = true;
        }
        s.slots.push((page, slot));
    }
}

#[derive(Debug)]
struct ReadbackObj {
    /// The mapped buffer the answer arrives in; none when the host holds it.
    buffer: Option<wgpu::Buffer>,
    host: Vec<u8>,
    state: Arc<AtomicU8>,
    /// The serial of the encoder holding its copy.
    serial: u64,
    /// The converted format the copied `rgba8unorm` rows are packed into when finished.
    packed: Option<Format>,
    /// Bytes to skip at the start, bytes per packed row, bytes per copied row, and rows.
    skip: u64,
    row: u64,
    pitch: u64,
    rows: u64,
}

#[derive(Debug)]
struct SurfaceObj {
    raw: wgpu::Surface<'static>,
    /// The frame handed out by `current_texture`, shown by `present`.
    current: RefCell<Option<Frame>>,
    format: Cell<Format>,
    config: RefCell<Option<wgpu::SurfaceConfiguration>>,
    /// Drawn into while the window shows nothing (occluded, or no texture in time); kept at the
    /// configured size.
    unshown: RefCell<Option<wgpu::Texture>>,
    /// Where frames are drawn when the window lacks the configured format (`PresentConverted`).
    stand_in: RefCell<Option<wgpu::Texture>>,
}

/// This frame's texture: the window's, or the unshown one when the window has none to give.
#[derive(Debug)]
enum Frame {
    Shown(wgpu::SurfaceTexture),
    Unshown,
}

/// The encoder a command is recorded in.
#[derive(Clone, Copy, Debug)]
enum Stream {
    /// The commands in the order they were recorded.
    Main,
    /// Ahead of them: writes taking effect at once.
    Lead,
}

/// Never made: the back end has no resource tables.
#[derive(Clone, Debug)]
pub enum NoTable {}

/// A point in the submitted work.
#[derive(Clone, Debug)]
pub struct Fence {
    /// Set once the queue has finished the work submitted up to this point.
    pub done: Arc<AtomicBool>,
    /// The submission the point ends, which a wait polls the device for.
    pub index: wgpu::SubmissionIndex,
}

/// Threads per workgroup of the back end's compute programs (`GROUP` in their text).
const GROUP: u64 = 64;

/// A compute program's text with its workgroup size, `GROUP`, defined ahead of it.
fn with_group(program: &str) -> String {
    format!("const GROUP: u32 = {GROUP}u;\n{program}")
}

/// WebGPU's largest sampler anisotropy: `maxAnisotropy` is clamped to it.
const MAX_ANISOTROPY: u16 = 16;

/// The start of the line the back end prints on standard error at each power of two frames
/// presented: the frames, then every counter ([`Counters`]) and every slow path taken, each as
/// `name=count`, so any program's log gives what a frame costs between two reports.
const FRAME_REPORT: &str = "webgpu: frames=";

/// What a WebGPU back end built alone is made from.
///
/// Which of wgpu's native APIs to open, whether lavapipe (Mesa's Vulkan driver that draws on the
/// CPU) is taken when no GPU is there, whether the system's software adapter is asked for instead,
/// and which wgpu features the device must have or lack. wgpu ranks a GPU before a CPU adapter, so
/// `lavapipe` matters only on a machine without one (the build machines). Any other CPU adapter is
/// refused unless `software` asks for it, and lavapipe too unless the Vulkan loader is told to load
/// only it ([`VULKAN_DRIVER_VARIABLES`] names its manifest). A host that made its adapter and
/// device itself (the browser, whose adapter answers only asynchronously, and a host whose windows'
/// surfaces must come from the device's own wgpu instance) builds the back end from them instead.
#[derive(Debug)]
pub struct WebGpuConfig {
    /// The native APIs wgpu may open.
    pub backends: wgpu::Backends,
    /// Lavapipe is taken when it is the only adapter and the loader's variables name it.
    pub lavapipe: bool,
    /// Whether the system's software adapter (WARP on Windows) is taken instead of a GPU.
    ///
    /// A test drawing against Direct3D on WARP asks for it.
    pub software: bool,
    /// The `wgpu` features the device is made with beyond the back end's own.
    ///
    /// The back end is not made where the adapter lacks one.
    pub required: wgpu::Features,
    /// The `wgpu` features the device is made without though the adapter has them.
    ///
    /// A browser lacking them gives such a device.
    pub without: wgpu::Features,
}

/// The Vulkan loader's variables that name the only driver manifests it loads.
/// https://github.com/KhronosGroup/Vulkan-Loader/blob/main/docs/LoaderInterfaceArchitecture.md#table-of-debug-environment-variables
const VULKAN_DRIVER_VARIABLES: [&str; 2] = ["VK_DRIVER_FILES", "VK_ICD_FILENAMES"];

/// The start of the manifest name of lavapipe.
/// https://gitlab.freedesktop.org/mesa/mesa/-/blob/mesa-26.3.1/src/gallium/targets/lavapipe/meson.build
const LAVAPIPE_MANIFEST: &str = "lvp_icd";

/// The name lavapipe's adapter reports.
/// https://gitlab.freedesktop.org/mesa/mesa/-/blob/mesa-26.3.1/src/gallium/frontends/lavapipe/lvp_device.c
const LAVAPIPE_DRIVER: &str = "llvmpipe";

/// Tells whether the run names lavapipe as its only Vulkan driver, as the build machines do.
fn lavapipe_named() -> bool {
    VULKAN_DRIVER_VARIABLES
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|files| files.contains(LAVAPIPE_MANIFEST)))
}

impl StandaloneGpu for WebGpu {
    type Config = WebGpuConfig;
    type NewError = String;

    fn new(config: WebGpuConfig) -> Result<Self, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = config.backends;
        desc.flags |= Self::INSTANCE_FLAGS;
        let instance = wgpu::Instance::new(desc);
        let adapter = ready(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: config.software,
            ..Default::default()
        }))?
        .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        let lavapipe = config.lavapipe
            && info.backend == wgpu::Backend::Vulkan
            && info.name.contains(LAVAPIPE_DRIVER)
            && lavapipe_named();
        let cpu = info.device_type == wgpu::DeviceType::Cpu;
        if config.software && !cpu {
            return Err(format!(
                "the software adapter was asked for, and {} is not one",
                info.name
            ));
        }
        if cpu && !lavapipe && !config.software {
            return Err(format!(
                "the adapter is the software {}, not a GPU (lavapipe is taken only where the Vulkan loader is told to load it alone)",
                info.name
            ));
        }
        let lacking = config.required - adapter.features();
        if !lacking.is_empty() {
            return Err(format!("{} lacks {lacking:?}", info.name));
        }
        let mut desc = Self::device_descriptor(&adapter);
        desc.required_features |= config.required;
        desc.required_features.remove(config.without);
        let (device, queue) = ready(adapter.request_device(&desc))?.map_err(|e| e.to_string())?;
        Ok(WebGpu::new(&adapter, device, queue))
    }
}

/// Takes the answer of a wgpu request that a native API gives at once.
fn ready<F: core::future::Future>(request: F) -> Result<F::Output, String> {
    let mut request = core::pin::pin!(request);
    match request.as_mut().poll(&mut core::task::Context::from_waker(
        core::task::Waker::noop(),
    )) {
        core::task::Poll::Ready(answer) => Ok(answer),
        core::task::Poll::Pending => Err(String::from(
            "the adapter answers only asynchronously: build the back end from a device made by the host",
        )),
    }
}

impl WebGpu {
    /// Flags the host adds to its wgpu instance: timestamps resolved in nanoseconds natively, which
    /// `TIMESTAMPS` needs where the GPU's clock ticks in other units.
    pub const INSTANCE_FLAGS: wgpu::InstanceFlags =
        wgpu::InstanceFlags::AUTOMATIC_TIMESTAMP_NORMALIZATION;

    /// The features this back end uses where the adapter has them, and the adapter's highest
    /// limits: what to make its device with.
    pub fn device_descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
        let wanted = wgpu::Features::DEPTH_CLIP_CONTROL
            | wgpu::Features::TEXTURE_COMPRESSION_BC
            | wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D
            | wgpu::Features::INDIRECT_FIRST_INSTANCE
            | wgpu::Features::SHADER_F16
            | wgpu::Features::FLOAT32_FILTERABLE
            | wgpu::Features::FLOAT32_BLENDABLE
            | wgpu::Features::DUAL_SOURCE_BLENDING
            | wgpu::Features::CLIP_DISTANCES
            | wgpu::Features::DEPTH32FLOAT_STENCIL8
            | wgpu::Features::RG11B10UFLOAT_RENDERABLE
            | wgpu::Features::BGRA8UNORM_STORAGE
            | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | wgpu::Features::TIMESTAMP_QUERY
            | wgpu::Features::POLYGON_MODE_LINE
            | wgpu::Features::POLYGON_MODE_POINT
            | wgpu::Features::SUBGROUP
            | wgpu::Features::PRIMITIVE_INDEX
            | wgpu::Features::IMMEDIATES;
        wgpu::DeviceDescriptor {
            label: None,
            required_features: adapter.features() & wanted,
            required_limits: adapter.limits(),
            ..Default::default()
        }
    }

    /// Makes the back end on a device made from `adapter`, declaring what that device can do.
    pub fn new(adapter: &wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let lost = Arc::new(Mutex::new(None));
        let on_error = lost.clone();
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
            eprintln!("webgpu: device error: {e}");
            let reason = match e {
                wgpu::Error::OutOfMemory { .. } => LossReason::OutOfMemory,
                _ => LossReason::Error,
            };
            on_error.lock().unwrap().get_or_insert(reason);
            wait::wake();
        }));
        let on_lost = lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            eprintln!("webgpu: device lost: {message}");
            let reason = match reason {
                wgpu::DeviceLostReason::Destroyed => LossReason::Removed,
                _ => LossReason::Reset,
            };
            on_lost.lock().unwrap().get_or_insert(reason);
            wait::wake();
        });
        let (
            mut caps,
            FormatTables {
                usages,
                resolved_by_draw,
            },
        ) = caps(adapter, &device);
        let depth24 = probe::run(&device, &queue);
        if let Depth24::Unorm(_) = depth24 {
            caps.features |= Features::DEPTH24_UNORM;
        }
        // Timestamps resolve in nanoseconds only where the clock ticks in them or wgpu converts
        // them (an instance made with `AUTOMATIC_TIMESTAMP_NORMALIZATION`; always in browsers).
        if device.features().contains(wgpu::Features::TIMESTAMP_QUERY)
            && queue.get_timestamp_period() == 1.0
        {
            caps.features |= Features::TIMESTAMPS;
        }
        // wgpu's Direct3D 12 clears colour targets through 32-bit floats.
        let float_clears = adapter.get_info().backend == wgpu::Backend::Dx12;
        let padded_reads = adapter.get_info().backend == wgpu::Backend::Dx12;
        WebGpu {
            adapter: adapter.clone(),
            device,
            queue,
            caps,
            slow: wcontainers_gpu::slow::NO_SLOW_PATHS,
            helpers: Helpers::default(),
            lost,
            next_id: 1,
            serial: 1,
            completed: Arc::new(AtomicU64::new(0)),
            encoder: None,
            lead: None,
            pass: None,
            to_map: Vec::new(),
            objects: Objects::default(),
            destroy_after_submit: Vec::new(),
            blocks: Default::default(),
            caches: Default::default(),
            pool: Default::default(),
            texture_pool: Default::default(),
            arena: None,
            ring: None,
            query_ring: None,
            edge: None,
            query_sum: None,
            vertices: Default::default(),
            fills: Default::default(),
            float_clears,
            padded_reads,
            presented: 0,
            bind_epoch: 0,
            windows: HashMap::new(),
            counters: Counters::default(),
            usages,
            resolved_by_draw,
            #[cfg(test)]
            depth24,
            depth24_by_product: depth24.sampled_by_product(),
            depth24_colliding: fill::colliding(depth24.sampled_by_product()),
        }
    }

    /// What the probe of how `depth24plus` is stored found.
    #[cfg(test)]
    pub(crate) fn depth24(&self) -> Depth24 {
        self.depth24
    }

    /// Registers a host window for [`Gpu::create_surface`] under the number the graphics hook
    /// uses for it.
    pub fn add_window(&mut self, window: u32, surface: wgpu::Surface<'static>) {
        self.windows.insert(window, surface);
    }

    /// What the caches have made so far.
    #[cfg(test)]
    pub(crate) fn counters(&self) -> &Counters {
        &self.counters
    }

    /// The line printed at each power of two frames presented ([`FRAME_REPORT`]).
    fn frame_report(&self) -> String {
        let mut line = format!("{FRAME_REPORT}{}", self.presented);
        if let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(self.counters) {
            for (name, count) in fields {
                line += &format!(" {name}={count}");
            }
        }
        for path in SLOW_PATHS {
            let count = self.slow.counts[path as usize];
            if count > 0 {
                line += &format!(" {path:?}={count}");
            }
        }
        line
    }

    /// How many pipelines (render and compute) and program layouts the caches hold now.
    #[cfg(test)]
    pub(crate) fn held(&self) -> (usize, usize) {
        self.caches.held()
    }

    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Records a rule the back end could not carry out: the device is lost with `Error`, as plan
    /// section 4.10 says of recording calls.
    fn fail(&self, why: &str) {
        eprintln!("webgpu: {why}");
        self.lost.lock().unwrap().get_or_insert(LossReason::Error);
    }

    fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.stream(Stream::Main)
    }

    fn stream(&mut self, stream: Stream) -> &mut wgpu::CommandEncoder {
        let e = match stream {
            Stream::Main => &mut self.encoder,
            Stream::Lead => &mut self.lead,
        };
        e.get_or_insert_with(|| {
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default())
        })
    }

    /// Whether the lead encoder being recorded writes this buffer.
    fn lead_pending(&self, b: &BufferObj) -> bool {
        b.lead_in.get() == self.serial && self.lead.is_some()
    }
}

/// The view a pass draws into (every aspect) and the view a binding reads (the aspect `stencil`
/// names, in that aspect's own format); one view where the two are the same. `format_usages` are
/// the usages the view's format allows on the device.
fn raw_views(
    texture: &TextureObj,
    desc: &TextureViewDesc,
    format_usages: wgpu::TextureUsages,
) -> Option<(wgpu::TextureView, wgpu::TextureView)> {
    let seen = match texture.blocks {
        Some(_) => blocks::decoded(desc.format)?,
        None => desc.format,
    };
    let format = convert::texture_format(seen)?;
    let aspect = convert::aspect(desc.stencil, desc.format);
    let raw = texture.raw.borrow().clone();
    let usage = convert::view_usage(&raw, format, format_usages);
    let view = |aspect, format| {
        raw.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(format),
            dimension: Some(convert::view_dimension(desc.dimension)),
            usage,
            aspect,
            base_mip_level: desc.base_mip,
            mip_level_count: Some(desc.mip_count),
            base_array_layer: desc.base_layer,
            array_layer_count: Some(desc.layer_count),
        })
    };
    let whole = view(wgpu::TextureAspect::All, format);
    if aspect == wgpu::TextureAspect::All {
        return Some((whole.clone(), whole));
    }
    let read = view(aspect, format.aspect_specific_format(aspect)?);
    Some((whole, read))
}

/// What the device can do, in the interface's words (plan section 4.11). Only what this back end
/// carries out is declared.
fn caps(adapter: &wgpu::Adapter, device: &wgpu::Device) -> (Caps, FormatTables) {
    let f = device.features();
    let l = device.limits();
    let mut features = Features::COMPUTE
        | Features::VERTEX_STORAGE_READ
        | Features::FRAGMENT_STORAGE_WRITE
        | Features::PROVOKING_VERTEX_FIRST
        | Features::INDEPENDENT_BLEND
        | Features::SAMPLE_SHADING
        | Features::SAMPLE_MASK
        | Features::MULTISAMPLE_READ
        | Features::INDIRECT
        | Features::BASE_VERTEX_INSTANCE
        | Features::TEXTURE_VIEWS
        | Features::SEPARATE_SAMPLERS
        | Features::CLIP_DEPTH_ZERO_ONE
        | Features::DEPTH_BIAS_CLAMP
        | Features::TEXTURE_LEVEL_QUERY
        | Features::FINE_DERIVATIVES;
    for (wgpu_feature, ours) in [
        (wgpu::Features::DEPTH_CLIP_CONTROL, Features::DEPTH_CLAMP),
        (wgpu::Features::CLIP_DISTANCES, Features::CLIP_DISTANCE),
        (
            wgpu::Features::DUAL_SOURCE_BLENDING,
            Features::DUAL_SOURCE_BLEND,
        ),
        (
            wgpu::Features::INDIRECT_FIRST_INSTANCE,
            Features::INDIRECT_FIRST_INSTANCE,
        ),
        (wgpu::Features::SHADER_F16, Features::SHADER_F16),
        (wgpu::Features::SUBGROUP, Features::SUBGROUPS),
        (wgpu::Features::PRIMITIVE_INDEX, Features::PRIMITIVE_INDEX),
        (wgpu::Features::POLYGON_MODE_LINE, Features::POLYGON_MODE),
        (
            wgpu::Features::POLYGON_MODE_POINT,
            Features::POLYGON_MODE_POINT,
        ),
        (
            wgpu::Features::TEXTURE_COMPRESSION_BC,
            Features::BLOCK_COMPRESSION,
        ),
    ] {
        if f.contains(wgpu_feature) {
            features |= ours;
        }
    }
    // In the browser `prepare`'s pipeline compiles in the GPU process while the page goes on;
    // natively wgpu compiles it at once on the calling thread.
    if cfg!(target_arch = "wasm32") {
        features |= Features::BACKGROUND_PREPARE;
    }
    // Proven against Direct3D's picture natively by tests/raster.rs and in Chrome by web/check.
    features |= Features::FRACTIONAL_VIEWPORT;
    let adapter_specific = f.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
    let mut formats = FormatCaps {
        uses: [FormatUse::empty(); FORMAT_COUNT],
        samples: [0; FORMAT_COUNT],
    };
    let mut usages = [wgpu::TextureUsages::empty(); FORMAT_COUNT];
    let mut resolved_by_draw = [false; FORMAT_COUNT];
    for (i, allowed) in usages.iter_mut().enumerate() {
        let format = format_at(i);
        // Without native BC, a BC format is held decoded (`BlockDecoded`): sampled, filtered and
        // read back as its decoded format is, nothing else.
        let decoded =
            blocks::decoded(format).filter(|_| !f.contains(wgpu::Features::TEXTURE_COMPRESSION_BC));
        let converted = packed::held(format);
        let Some(raw) = convert::texture_format(decoded.unwrap_or(format)) else {
            continue;
        };
        if !raw.required_features().is_empty() && !f.contains(raw.required_features()) {
            continue;
        }
        let guaranteed = raw.guaranteed_format_features(f);
        let ff = if adapter_specific {
            adapter.get_texture_format_features(raw)
        } else {
            guaranteed
        };
        // Copies follow WebGPU's rules whatever the adapter has: `depth24plus` is never copied.
        let copies = wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST;
        let mut u = (ff.allowed_usages - copies) | (guaranteed.allowed_usages & copies);
        let mut flags = ff.flags;
        // `bgra8unorm` is stored into only on a device with its feature, which gives programs
        // storage writes without a format, whatever the adapter's own flags say.
        if raw == wgpu::TextureFormat::Bgra8Unorm && !f.contains(wgpu::Features::BGRA8UNORM_STORAGE)
        {
            u -= wgpu::TextureUsages::STORAGE_BINDING;
            flags -= wgpu::TextureFormatFeatureFlags::STORAGE_READ_ONLY
                | wgpu::TextureFormatFeatureFlags::STORAGE_WRITE_ONLY
                | wgpu::TextureFormatFeatureFlags::STORAGE_READ_WRITE
                | wgpu::TextureFormatFeatureFlags::STORAGE_ATOMIC;
        }
        *allowed = u;
        let mut uses = FormatUse::empty();
        let depth = FORMAT_FACTS[format as usize].depth_stencil;
        // A resolve averages: integer formats never resolve, as in WebGPU and Direct3D, whatever
        // the adapter's own flags allow. A float format drawn multisampled that WebGPU does not
        // resolve (`r32float` without float32 resolves) is resolved by a draw averaging its samples.
        let averaged = matches!(
            raw.sample_type(None, None),
            Some(wgpu::TextureSampleType::Float { .. })
        );
        // A resolve writes a texture drawn into, which some adapters' flags leave out.
        let native_resolve = averaged
            && u.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            && flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE);
        let drawn_resolve = averaged
            && !native_resolve
            && !depth
            && u.contains(
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            )
            && flags
                .supported_sample_counts()
                .iter()
                .any(|&count| count > 1);
        resolved_by_draw[i] = drawn_resolve;
        let pairs = [
            (
                u.contains(wgpu::TextureUsages::TEXTURE_BINDING),
                FormatUse::SAMPLE,
            ),
            (
                !depth && flags.contains(wgpu::TextureFormatFeatureFlags::FILTERABLE),
                FormatUse::FILTER,
            ),
            (
                !depth && u.contains(wgpu::TextureUsages::RENDER_ATTACHMENT),
                FormatUse::RENDER,
            ),
            // wgpu marks every filterable float format blendable, renderable or not.
            (
                !depth
                    && u.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
                    && flags.contains(wgpu::TextureFormatFeatureFlags::BLENDABLE),
                FormatUse::BLEND,
            ),
            (native_resolve || drawn_resolve, FormatUse::RESOLVE),
            (
                flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_READ_ONLY),
                FormatUse::STORAGE_READ,
            ),
            (
                flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_WRITE_ONLY),
                FormatUse::STORAGE_WRITE,
            ),
            (
                depth && u.contains(wgpu::TextureUsages::RENDER_ATTACHMENT),
                FormatUse::DEPTH_STENCIL,
            ),
            (
                u.contains(wgpu::TextureUsages::COPY_SRC),
                FormatUse::COPY_OUT,
            ),
            (
                matches!(
                    format,
                    Format::Bgra8Unorm | Format::Rgba8Unorm | Format::Rgba16Float
                ),
                FormatUse::PRESENT,
            ),
        ];
        for (has, bit) in pairs {
            if has {
                uses |= bit;
            }
        }
        if decoded.is_some() {
            uses &= FormatUse::SAMPLE | FormatUse::FILTER | FormatUse::COPY_OUT;
            *allowed = wgpu::TextureUsages::TEXTURE_BINDING | copies;
            formats.uses[i] = uses;
            formats.samples[i] = 1;
            continue;
        }
        // Converted formats (`FormatConverted`): some are drawn into with only their own channels
        // written, the rest only sampled (packed.rs).
        if converted.is_some() {
            let drawn = packed::drawn(format).is_some();
            let mut kept = FormatUse::SAMPLE | FormatUse::FILTER | FormatUse::COPY_OUT;
            *allowed &= wgpu::TextureUsages::TEXTURE_BINDING | copies;
            if drawn {
                kept |= FormatUse::RENDER | FormatUse::BLEND | FormatUse::RESOLVE;
                *allowed |= u & wgpu::TextureUsages::RENDER_ATTACHMENT;
            }
            if packed::stored(format) {
                kept |= FormatUse::STORAGE_READ | FormatUse::STORAGE_WRITE;
                *allowed |= u & wgpu::TextureUsages::STORAGE_BINDING;
            }
            uses &= kept;
            if !drawn {
                formats.uses[i] = uses;
                formats.samples[i] = 1;
                continue;
            }
        }
        formats.uses[i] = uses;
        formats.samples[i] = flags
            .supported_sample_counts()
            .into_iter()
            .filter_map(|count| u8::try_from(count).ok())
            .fold(0, |samples, count| samples | count);
    }
    let limits = Limits {
        max_texture_2d: l.max_texture_dimension_2d,
        max_texture_cube: l.max_texture_dimension_2d,
        max_texture_3d: l.max_texture_dimension_3d,
        max_texture_layers: l.max_texture_array_layers,
        max_buffer_size: l.max_buffer_size,
        max_uniform_buffer_size: l.max_uniform_buffer_binding_size as u32,
        max_storage_buffer_size: l.max_storage_buffer_binding_size,
        uniform_buffers_per_stage: l.max_uniform_buffers_per_shader_stage,
        textures_per_stage: l.max_sampled_textures_per_shader_stage,
        samplers_per_stage: l.max_samplers_per_shader_stage,
        storage_buffers_per_stage: l.max_storage_buffers_per_shader_stage,
        storage_textures_per_stage: l.max_storage_textures_per_shader_stage,
        vertex_streams: l.max_vertex_buffers,
        vertex_attributes: l.max_vertex_attributes,
        max_vertex_stride: l.max_vertex_buffer_array_stride,
        color_targets: l.max_color_attachments,
        color_bytes_per_sample: l.max_color_attachment_bytes_per_sample,
        inter_stage_slots: l.max_inter_stage_shader_variables,
        min_uniform_offset_alignment: l.min_uniform_buffer_offset_alignment,
        min_storage_offset_alignment: l.min_storage_buffer_offset_alignment,
        buffer_copy_alignment: wgpu::COPY_BUFFER_ALIGNMENT as u32,
        // Where wgpu has them (natively); a pipeline layout of each program takes them all.
        immediate_bytes: if device.features().contains(wgpu::Features::IMMEDIATES) {
            l.max_immediate_size
        } else {
            0
        },
        compute_workgroup_storage: l.max_compute_workgroup_storage_size,
        compute_invocations: l.max_compute_invocations_per_workgroup,
        compute_size: [
            l.max_compute_workgroup_size_x,
            l.max_compute_workgroup_size_y,
            l.max_compute_workgroup_size_z,
        ],
        max_dispatch: l.max_compute_workgroups_per_dimension,
        max_point_size: 1.0,
        max_line_width: 1.0,
        max_anisotropy: MAX_ANISOTROPY,
        capture_buffers: 0,
        capture_components: 0,
    };
    let info = adapter.get_info();
    let caps = Caps {
        adapter: wcontainers_gpu::caps::AdapterInfo {
            name: info.name,
            vendor: info.vendor,
            device: info.device,
            software: info.device_type == wgpu::DeviceType::Cpu,
        },
        features,
        limits,
        formats,
    };
    (
        caps,
        FormatTables {
            usages,
            resolved_by_draw,
        },
    )
}

/// What the device does with each format beside its [`Caps`], by `Format as usize`.
struct FormatTables {
    /// The texture usages each format allows.
    usages: [wgpu::TextureUsages; FORMAT_COUNT],
    /// Whether a resolve of the format is a draw averaging its samples (`ResolveByDraw`).
    resolved_by_draw: [bool; FORMAT_COUNT],
}

/// The format at position `i` of [`Format`]'s list.
fn format_at(i: usize) -> Format {
    // Every format, in order, so the table indexes match `Format as usize`.
    const ALL: [Format; FORMAT_COUNT] = {
        use Format::*;
        [
            R8Unorm,
            R8Snorm,
            R8Uint,
            R8Sint,
            R16Unorm,
            R16Snorm,
            R16Uint,
            R16Sint,
            R16Float,
            Rg8Unorm,
            Rg8Snorm,
            Rg8Uint,
            Rg8Sint,
            R32Uint,
            R32Sint,
            R32Float,
            Rg16Unorm,
            Rg16Snorm,
            Rg16Uint,
            Rg16Sint,
            Rg16Float,
            Rgba8Unorm,
            Rgba8UnormSrgb,
            Rgba8Snorm,
            Rgba8Uint,
            Rgba8Sint,
            Bgra8Unorm,
            Bgra8UnormSrgb,
            Bgrx8Unorm,
            Bgrx8UnormSrgb,
            Rgb10a2Unorm,
            Rgb10a2Uint,
            Rg11b10Float,
            Rgb9e5Float,
            B5g6r5Unorm,
            Bgr5a1Unorm,
            Bgra4Unorm,
            Rgbg8Unorm,
            Grgb8Unorm,
            Rg32Uint,
            Rg32Sint,
            Rg32Float,
            Rgba16Unorm,
            Rgba16Snorm,
            Rgba16Uint,
            Rgba16Sint,
            Rgba16Float,
            Rgb32Uint,
            Rgb32Sint,
            Rgb32Float,
            Rgba32Uint,
            Rgba32Sint,
            Rgba32Float,
            A8Unorm,
            Stencil8,
            Depth16Unorm,
            Depth24Plus,
            Depth24PlusStencil8,
            Depth32Float,
            Depth32FloatStencil8,
            Bc1RgbaUnorm,
            Bc1RgbaUnormSrgb,
            Bc2RgbaUnorm,
            Bc2RgbaUnormSrgb,
            Bc3RgbaUnorm,
            Bc3RgbaUnormSrgb,
            Bc4RUnorm,
            Bc4RSnorm,
            Bc5RgUnorm,
            Bc5RgSnorm,
            Bc6hRgbUfloat,
            Bc6hRgbFloat,
            Bc7RgbaUnorm,
            Bc7RgbaUnormSrgb,
            Etc2Rgb8Unorm,
            Etc2Rgb8UnormSrgb,
            Etc2Rgba8Unorm,
            Etc2Rgba8UnormSrgb,
            Astc4x4Unorm,
            Astc4x4UnormSrgb,
        ]
    };
    ALL[i]
}

/// Every buffer can be copied either way, and written by the edge program of unaligned ranges.
fn buffer_usage(u: BufferUsage) -> wgpu::BufferUsages {
    let mut w =
        wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::STORAGE;
    for (ours, theirs) in [
        (BufferUsage::VERTEX, wgpu::BufferUsages::VERTEX),
        (BufferUsage::INDEX, wgpu::BufferUsages::INDEX),
        (BufferUsage::UNIFORM, wgpu::BufferUsages::UNIFORM),
        (BufferUsage::STORAGE, wgpu::BufferUsages::STORAGE),
        (BufferUsage::INDIRECT, wgpu::BufferUsages::INDIRECT),
    ] {
        if u.contains(ours) {
            w |= theirs;
        }
    }
    w
}

impl Gpu for WebGpu {
    type Buffer = Buffer;
    type Texture = Texture;
    type TextureView = TextureView;
    type Sampler = Sampler;
    type Shader = Shader;
    type Program = ProgramHandle;
    type Blend = Blend;
    type DepthStencil = DepthStencil;
    type Raster = Raster;
    type VertexLayout = VertexLayout;
    type BindingSet = BindingSet;
    type QuerySet = QuerySet;
    type Readback = Readback;
    type Fence = Fence;
    type ResourceTable = NoTable;
    type Surface = Surface;

    fn caps(&self) -> &Caps {
        &self.caps
    }

    fn slow_paths(&mut self) -> &mut SlowPaths {
        &mut self.slow
    }

    fn status(&mut self) -> DeviceStatus {
        match *self.lost.lock().unwrap() {
            Some(reason) => DeviceStatus::Lost(reason),
            None => DeviceStatus::Ok,
        }
    }

    fn helpers(&mut self) -> &mut Helpers<Self> {
        &mut self.helpers
    }

    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Buffer, GpuError> {
        if desc.size > self.caps.limits.max_buffer_size {
            return Err(GpuError::Invalid("buffer larger than max_buffer_size"));
        }
        let in_arena = desc.usage == BufferUsage::UNIFORM && desc.intent == MapIntent::Often;
        let (usage, raw, base, storage) = if in_arena {
            let (raw, base, storage) = arena_slice(self, desc.size);
            (memory::arena_usage(), raw, base, storage)
        } else {
            let usage = buffer_usage(desc.usage);
            let raw = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: desc.size.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
                usage,
                mapped_at_creation: false,
            });
            (usage, raw, 0, self.id())
        };
        let number = self.id();
        let made = HeldBuffer(Rc::new(BufferObj {
            id: number,
            desc: *desc,
            usage,
            raw: RefCell::new(raw),
            base: Cell::new(base),
            storage: Cell::new(storage),
            generation: Cell::new(0),
            in_arena,
            in_sets: Cell::new(false),
            static_bound: Cell::new(false),
            used_in: Cell::new(0),
            lead_in: Cell::new(0),
            written: Cell::new(0),
            spares: Cell::new(0),
        }));
        self.objects.buffers.insert(number, made);
        Ok(Buffer(number))
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Texture, GpuError> {
        let decode = !self
            .device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let held = blocks::decoded(desc.format)
            .filter(|_| decode)
            .or(packed::held(desc.format))
            .unwrap_or(desc.format);
        let format = convert::texture_format(held)
            .filter(|_| !self.caps.formats.uses[desc.format as usize].is_empty())
            .ok_or(GpuError::Invalid(
                "texture format this back end does not hold",
            ))?;
        let mut usage = wgpu::TextureUsages::empty();
        for (ours, theirs) in [
            (TextureUsage::SAMPLE, wgpu::TextureUsages::TEXTURE_BINDING),
            (TextureUsage::RENDER, wgpu::TextureUsages::RENDER_ATTACHMENT),
            (
                TextureUsage::PRESENT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            (TextureUsage::STORAGE, wgpu::TextureUsages::STORAGE_BINDING),
            (TextureUsage::COPY_SRC, wgpu::TextureUsages::COPY_SRC),
            (TextureUsage::COPY_DST, wgpu::TextureUsages::COPY_DST),
        ] {
            if desc.usage.contains(ours) {
                usage |= theirs;
            }
        }
        // Read backs and the staging route of ordered writes copy in and out of every texture whose
        // format allows it.
        let copies = wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST;
        let allowed = self.usages[desc.format as usize];
        if packed::held(desc.format).is_some() {
            if !allowed.contains(usage) {
                return Err(GpuError::Invalid(
                    "a use of a converted format this back end does not declare",
                ));
            }
            self.record_slow_path(wcontainers_gpu::slow::SlowPath::FormatConverted);
        }
        usage |= copies & allowed;
        // Copies between a buffer and a converted format are draws, into the texture or from it.
        if packed::held(desc.format).is_some() && desc.samples == 1 {
            usage |= wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        }
        // Depth and stencil copies and read backs WebGPU has no copy for are draws: from a bound
        // source into an attachment.
        if desc.usage.contains(TextureUsage::COPY_SRC)
            && (fill::DepthSource::readable(desc.format)
                || FORMAT_FACTS[desc.format as usize].depth_stencil)
        {
            usage |= allowed & wgpu::TextureUsages::TEXTURE_BINDING;
        }
        if desc.usage.contains(TextureUsage::COPY_DST)
            && FORMAT_FACTS[desc.format as usize].depth_stencil
        {
            usage |= allowed & wgpu::TextureUsages::RENDER_ATTACHMENT;
        }
        let srgb_twin = convert::srgb_twin(format);
        // In the browser a resolve in the other twin is drawn from the samples, and so is every
        // resolve of a format WebGPU does not resolve (`ResolveByDraw`).
        if desc.samples > 1
            && (cfg!(target_arch = "wasm32") && srgb_twin.is_some()
                || self.resolved_by_draw[desc.format as usize])
        {
            usage |= allowed & wgpu::TextureUsages::TEXTURE_BINDING;
        }
        let raw = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: desc.width,
                height: desc.height,
                depth_or_array_layers: desc.depth_or_layers,
            },
            mip_level_count: desc.mip_levels,
            sample_count: desc.samples,
            dimension: match desc.dimension {
                TextureDimension::D2 => wgpu::TextureDimension::D2,
                TextureDimension::D3 => wgpu::TextureDimension::D3,
            },
            format,
            usage,
            view_formats: srgb_twin.as_slice(),
        });
        let number = self.id();
        let made = HeldTexture(Rc::new(TextureObj {
            desc: *desc,
            held,
            blocks: (held != desc.format && packed::held(desc.format).is_none())
                .then(|| RefCell::new(blocks::zeroed(desc))),
            raw: RefCell::new(raw),
            generation: Cell::new(0),
            used_in: Cell::new(0),
            spares: Cell::new(0),
        }));
        self.objects.textures.insert(number, made);
        Ok(Texture(number))
    }

    fn create_view(
        &mut self,
        texture: &Texture,
        desc: &TextureViewDesc,
    ) -> Result<TextureView, GpuError> {
        let texture = held(&self.objects.textures, texture.0)
            .ok_or(GpuError::Invalid("a view of a destroyed texture"))?;
        let format_usages = self.usages[desc.format as usize];
        let views = raw_views(&texture.0, desc, format_usages).ok_or(GpuError::Invalid(
            "view format or aspect this back end does not hold",
        ))?;
        let t = texture.0.desc;
        let size = |n: u32| (n >> desc.base_mip).max(1);
        let number = self.id();
        let made = HeldView(Rc::new(ViewObj {
            id: number,
            format: desc.format,
            made_for: Cell::new(texture.0.generation.get()),
            texture: Some(texture),
            views: RefCell::new(views),
            desc: Some(*desc),
            format_usages,
            width: size(t.width),
            height: size(t.height),
            samples: t.samples,
            three_d: t.dimension == TextureDimension::D3,
        }));
        self.objects.views.insert(number, made);
        Ok(TextureView(number))
    }

    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Sampler, GpuError> {
        let address = |ours| {
            convert::address(ours).ok_or(GpuError::Absent(match ours {
                wcontainers_gpu::resource::AddressMode::MirrorOnce => Features::MIRROR_ONCE,
                _ => Features::BORDER_COLOR,
            }))
        };
        let [u, v, w] = desc.address;
        let default_sampler = wgpu::SamplerDescriptor::default();
        let raw = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: address(u)?,
            address_mode_v: address(v)?,
            address_mode_w: address(w)?,
            mag_filter: convert::filter(desc.mag),
            min_filter: convert::filter(desc.min),
            mipmap_filter: convert::mip_filter(desc.mip),
            lod_min_clamp: f32::from_bits(desc.lod_range[0]).max(0.0),
            lod_max_clamp: f32::from_bits(desc.lod_range[1]).min(default_sampler.lod_max_clamp),
            compare: desc.compare.map(convert::compare),
            // Anisotropy applies only to fully linear filtering, in WebGPU as in Direct3D.
            anisotropy_clamp: if [desc.min, desc.mag, desc.mip].contains(&Filter::Nearest) {
                1
            } else {
                desc.anisotropy.clamp(1, MAX_ANISOTROPY)
            },
            border_color: None,
        });
        let number = self.id();
        let made = HeldSampler(Rc::new(SamplerObj {
            id: number,
            raw,
            filtering: [desc.min, desc.mag, desc.mip].contains(&Filter::Linear),
        }));
        self.objects.samplers.insert(number, made);
        Ok(Sampler(number))
    }

    fn create_shader(&mut self, module: &ShaderModule) -> Result<Shader, GpuError> {
        let stage = match module.stage {
            ShaderStage::Vertex => naga::ShaderStage::Vertex,
            ShaderStage::Fragment => naga::ShaderStage::Fragment,
            ShaderStage::Compute => naga::ShaderStage::Compute,
        };
        let entry_index = module
            .module
            .entry_points
            .iter()
            .position(|e| e.name == module.entry_point && e.stage == stage)
            .ok_or(GpuError::Invalid("the module has no such entry point"))?;
        // Checked against what WebGPU's core language and this device's features allow (plan 6.3),
        // so a program the browser would refuse is refused here too, natively as well.
        let info = wgpu_naga_bridge::create_validator(
            self.device.features(),
            wgpu::DownlevelFlags::compliant(),
            naga::valid::ValidationFlags::all(),
        )
        .validate(&module.module)
        .map_err(|e| {
            eprintln!("webgpu: program refused: {}", e.as_inner());
            GpuError::Invalid("a program WebGPU's language and this device's features do not allow")
        })?;
        let raw = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Naga(Cow::Owned(module.module.clone())),
            });
        let number = self.id();
        let made = HeldShader(Rc::new(ShaderObj {
            module: module.module.clone(),
            paired: layout::paired(&module.module, info.get_entry_point(entry_index)),
            info,
            entry_index,
            entry: module.entry_point.clone(),
            stage,
            raw,
        }));
        self.objects.shaders.insert(number, made);
        Ok(Shader(number))
    }

    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<ProgramHandle, GpuError> {
        let id = self.id();
        let shader = |s: &Shader| {
            held(&self.objects.shaders, s.0).ok_or(GpuError::Invalid("a destroyed shader"))
        };
        let obj = match program {
            Program::Render { vertex, fragment } => ProgramObj::Render {
                id,
                vertex: shader(vertex)?,
                fragment: fragment.map(shader).transpose()?,
            },
            Program::Compute(compute) => ProgramObj::Compute {
                id,
                shader: shader(compute)?,
            },
        };
        self.objects.programs.insert(id, HeldProgram(Rc::new(obj)));
        Ok(ProgramHandle(id))
    }

    fn create_blend(&mut self, desc: &BlendDesc) -> Result<Blend, GpuError> {
        if desc.logic_op.is_some() {
            return Err(GpuError::Absent(Features::LOGIC_OP));
        }
        let factors = desc
            .targets
            .iter()
            .filter_map(|t| t.blend)
            .flat_map(|(c, a)| [c.src, c.dst, a.src, a.dst]);
        let dual = self.caps.features.contains(Features::DUAL_SOURCE_BLEND);
        for f in factors {
            match convert::blend_factor(f) {
                None => return Err(GpuError::Absent(Features::BLEND_CONSTANT_ALPHA)),
                Some(
                    wgpu::BlendFactor::Src1
                    | wgpu::BlendFactor::OneMinusSrc1
                    | wgpu::BlendFactor::Src1Alpha
                    | wgpu::BlendFactor::OneMinusSrc1Alpha,
                ) if !dual => return Err(GpuError::Absent(Features::DUAL_SOURCE_BLEND)),
                Some(_) => {}
            }
        }
        let id = self.next_id + 1;
        let (block, made) = self.blocks.blends.get(desc, id);
        if made {
            self.next_id = id;
        }
        let number = block.id;
        self.objects.blends.insert(number, HeldBlend(block));
        Ok(Blend(number))
    }

    fn create_depth_stencil(&mut self, desc: &DepthStencilDesc) -> Result<DepthStencil, GpuError> {
        if desc.depth_bounds.is_some() {
            return Err(GpuError::Absent(Features::DEPTH_BOUNDS));
        }
        if desc.stencil_read_mask[0] != desc.stencil_read_mask[1]
            || desc.stencil_write_mask[0] != desc.stencil_write_mask[1]
        {
            return Err(GpuError::Absent(Features::STENCIL_PER_FACE));
        }
        let id = self.next_id + 1;
        let (block, made) = self.blocks.depth_stencils.get(desc, id);
        if made {
            self.next_id = id;
        }
        let number = block.id;
        self.objects
            .depth_stencils
            .insert(number, HeldDepthStencil(block));
        Ok(DepthStencil(number))
    }

    fn create_raster(&mut self, desc: &RasterDesc) -> Result<Raster, GpuError> {
        let has = |f| self.caps.features.contains(f);
        let absent = [
            (
                desc.fill == FillMode::Line && !has(Features::POLYGON_MODE),
                Features::POLYGON_MODE,
            ),
            (
                desc.fill == FillMode::Point && !has(Features::POLYGON_MODE_POINT),
                Features::POLYGON_MODE_POINT,
            ),
            (
                desc.provoking == ProvokingVertex::Last,
                Features::PROVOKING_VERTEX_LAST,
            ),
            (desc.depth_minus_one, Features::CLIP_DEPTH_MINUS_ONE),
            (f32::from_bits(desc.line_width) != 1.0, Features::WIDE_LINES),
            (
                !desc.depth_clip && !self.caps.features.contains(Features::DEPTH_CLAMP),
                Features::DEPTH_CLAMP,
            ),
            (
                f32::from_bits(desc.depth_bias).fract() != 0.0,
                Features::FRACTIONAL_DEPTH_BIAS,
            ),
        ];
        if let Some((_, feature)) = absent.iter().find(|(missing, _)| *missing) {
            return Err(GpuError::Absent(*feature));
        }
        let id = self.next_id + 1;
        let (block, made) = self.blocks.rasters.get(desc, id);
        if made {
            self.next_id = id;
        }
        let number = block.id;
        self.objects.rasters.insert(number, HeldRaster(block));
        Ok(Raster(number))
    }

    fn create_vertex_layout(&mut self, desc: &VertexLayoutDesc) -> Result<VertexLayout, GpuError> {
        for s in desc.streams.iter().flatten() {
            if !matches!(s.step, StepMode::Vertex | StepMode::Instance(1)) {
                return Err(GpuError::Absent(Features::INSTANCE_STEP_RATE));
            }
        }
        for a in desc.attributes.iter().flatten() {
            if desc
                .streams
                .get(a.stream as usize)
                .copied()
                .flatten()
                .is_none()
            {
                return Err(GpuError::Invalid("attribute of a stream the layout lacks"));
            }
        }
        let id = self.next_id + 1;
        let (block, made) = self.blocks.vertex_layouts.get(desc, id);
        if made {
            self.next_id = id;
            if let Some(plan) = vertex::plan(desc) {
                self.vertices.plans.insert(id, plan);
            }
        }
        let number = block.id;
        self.objects
            .vertex_layouts
            .insert(number, HeldVertexLayout(block));
        Ok(VertexLayout(number))
    }

    fn create_binding_set(
        &mut self,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<BindingSet, GpuError> {
        binding_set(self, entries)
    }

    fn binding_layout_key(&self, set: &BindingSet) -> BindingLayoutKey {
        held(&self.objects.sets, set.0).map_or_else(BindingLayoutKey::default, |s| s.0.key)
    }

    fn create_query_set(&mut self, kind: QueryKind, count: u32) -> Result<QuerySet, GpuError> {
        let ty = match kind {
            QueryKind::OcclusionAny => wgpu::QueryType::Occlusion,
            QueryKind::Occlusion => return Err(GpuError::Absent(Features::OCCLUSION_COUNT)),
            QueryKind::Timestamp if self.caps.features.contains(Features::TIMESTAMPS) => {
                wgpu::QueryType::Timestamp
            }
            QueryKind::Timestamp => return Err(GpuError::Absent(Features::TIMESTAMPS)),
        };
        // Pages of WebGPU's 4096 queries a set; on the last, occlusion sets get as many spare
        // slots again as it has queries, within 4096. More pages are made when they run out.
        let max = wgpu::QUERY_SET_MAX_QUERIES;
        let own_pages = count.div_ceil(max).max(1);
        let last = count - (own_pages - 1) * max;
        let last_total = match kind {
            QueryKind::OcclusionAny => last.saturating_mul(2).min(max).max(last),
            _ => last,
        };
        let (mut pages, mut free) = (Vec::new(), Vec::new());
        for p in 0..own_pages {
            let total = if p + 1 == own_pages { last_total } else { max };
            pages.push(self.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: None,
                ty,
                count: total,
            }));
            free.push(if p + 1 == own_pages {
                (last..total).rev().collect()
            } else {
                Vec::new()
            });
        }
        let number = self.id();
        let made = HeldQuerySet(Rc::new(QuerySetObj {
            kind,
            count,
            pages: RefCell::new(pages),
            own_pages: own_pages as usize,
            released: Default::default(),
            segments: Default::default(),
            free: RefCell::new(free),
        }));
        self.objects.query_sets.insert(number, made);
        Ok(QuerySet(number))
    }

    fn destroy(&mut self, object: Object<Self>) {
        let (serial, recording) = (self.serial, self.encoder.is_some());
        let pending = |used_in: u64| used_in == serial && recording;
        // What the handle names leaves the table; work already recorded keeps its own hold on it.
        match object {
            Object::Buffer(b) => {
                let Some(b) = self.objects.buffers.remove(&b.0) else {
                    return self.fail(GpuError::GONE);
                };
                if b.0.in_arena {
                    return release_slice(self, &b.0);
                }
                self.vertices.forget(b.0.id);
                give_back_spares(self, &b.0);
                let raw = b.0.raw.borrow().clone();
                if pending(b.0.used_in.get()) || self.lead_pending(&b.0) {
                    self.destroy_after_submit.push(Doomed::Buffer(raw));
                } else {
                    raw.destroy();
                }
            }
            Object::Texture(t) => {
                let Some(t) = self.objects.textures.remove(&t.0) else {
                    return self.fail(GpuError::GONE);
                };
                give_back_texture_spares(self, &t.0);
                if pending(t.0.used_in.get()) {
                    self.destroy_after_submit.push(Doomed::Texture(t.0.raw()));
                } else {
                    t.0.raw().destroy();
                }
            }
            Object::Program(p) => {
                if let Some(p) = self.objects.programs.remove(&p.0) {
                    self.caches.forget(p.0.id());
                }
            }
            Object::Blend(b) => {
                let released = held(&self.objects.blends, b.0)
                    .and_then(|held| self.blocks.blends.release(&held.0));
                if let Some(id) = released {
                    self.objects.blends.remove(&id);
                    self.caches.forget(id);
                }
            }
            Object::DepthStencil(b) => {
                let released = held(&self.objects.depth_stencils, b.0)
                    .and_then(|held| self.blocks.depth_stencils.release(&held.0));
                if let Some(id) = released {
                    self.objects.depth_stencils.remove(&id);
                    self.caches.forget(id);
                }
            }
            Object::Raster(b) => {
                let released = held(&self.objects.rasters, b.0)
                    .and_then(|held| self.blocks.rasters.release(&held.0));
                if let Some(id) = released {
                    self.objects.rasters.remove(&id);
                    self.caches.forget(id);
                }
            }
            Object::VertexLayout(b) => {
                let released = held(&self.objects.vertex_layouts, b.0)
                    .and_then(|held| self.blocks.vertex_layouts.release(&held.0));
                if let Some(id) = released {
                    self.objects.vertex_layouts.remove(&id);
                    self.caches.forget(id);
                    self.vertices.forget_layout(id);
                }
            }
            Object::BindingSet(set) => {
                if let Some(held) = held(&self.objects.sets, set.0)
                    && held.0.release()
                {
                    self.objects.sets.remove(&set.0);
                }
            }
            Object::QuerySet(q) => {
                self.objects.query_sets.remove(&q.0);
            }
            Object::TextureView(v) => {
                self.objects.views.remove(&v.0);
            }
            Object::Sampler(s) => {
                self.objects.samplers.remove(&s.0);
            }
            Object::Shader(s) => {
                self.objects.shaders.remove(&s.0);
            }
            Object::Surface(s) => {
                self.objects.surfaces.remove(&s.0);
            }
            Object::ResourceTable(t) => match t {},
        }
    }

    fn write_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8], mode: WriteMode) {
        self.counters.buffer_bytes += data.len() as u64;
        write_buffer_impl(self, buffer, offset, data, mode);
    }

    fn write_buffer_from(
        &mut self,
        buffer: &Buffer,
        offset: u64,
        data: &impl VolatileBytes,
        mode: WriteMode,
    ) {
        self.counters.buffer_bytes += data.len();
        write_buffer_from_impl(self, buffer, offset, data, mode);
    }

    fn write_texture(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        mode: WriteMode,
    ) {
        self.counters.texture_bytes += data.len() as u64;
        write_texture_impl(self, region, extent, data, row_pitch, rows_per_layer, mode);
    }

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        begin_pass_impl(self, pass);
    }

    fn end_pass(&mut self) {
        match self.pass.take() {
            Some(pass) => close_pass(self, pass),
            None => self.fail("end_pass without a pass"),
        }
    }

    fn prepare(&mut self, desc: &wcontainers_gpu::PrepareDesc<'_, Self>) {
        prepare_impl(self, desc);
    }

    fn draw(&mut self, call: &wcontainers_gpu::draw::DrawCall<'_, Self>) {
        draw_impl(self, call);
    }

    fn begin_query(&mut self, index: u32) {
        match &mut self.pass {
            Some(p) if p.occlusion && p.query_running.is_none() => {
                let set = p
                    .occlusion_set
                    .clone()
                    .expect("an occlusion pass has its set");
                set.forget_segments(index);
                // A part on another page than the query's own begins it in a spare slot there.
                let (own_page, own_slot) = QuerySetObj::own_slot(index);
                if p.page != own_page && set.free.borrow()[p.page].is_empty() {
                    split_onto_page(
                        self,
                        wcontainers_gpu::slow::SlowPath::QueriesSegmented,
                        |_| {},
                        true,
                    );
                }
                let Some(p) = self.pass.as_mut() else { return };
                let slot = match p.page {
                    page if page == own_page => own_slot,
                    page => {
                        let slot = set.take(page).expect("the page has room");
                        set.carry(index, page, slot, true);
                        slot
                    }
                };
                p.rp.begin_occlusion_query(slot);
                p.query_running = Some(index);
            }
            _ => self.fail("begin_query outside a pass with an occlusion set, or nested"),
        }
    }

    fn end_query(&mut self) {
        match &mut self.pass {
            Some(p) if p.query_running.is_some() => {
                p.rp.end_occlusion_query();
                p.query_running = None;
            }
            _ => self.fail("end_query without a running query"),
        }
    }

    fn copy_buffer(
        &mut self,
        src: &Buffer,
        src_offset: u64,
        dst: &Buffer,
        dst_offset: u64,
        size: u64,
    ) {
        copy_buffer_impl(self, src, src_offset, dst, dst_offset, size);
    }

    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        copy_texture_impl(self, src, dst, extent);
    }

    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        copy_buffer_texture(self, src, dst, extent, true);
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        copy_buffer_texture(self, dst, src, extent, false);
    }

    fn clear_buffer(&mut self, buffer: &Buffer, offset: u64, size: u64, pattern: &[u8]) {
        clear_buffer_impl(self, buffer, offset, size, pattern);
    }

    fn resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    ) {
        resolve_impl(self, src, dst, format);
    }

    fn submit(&mut self) -> Fence {
        if self.pass.is_some() {
            self.fail("submit inside a pass");
            self.pass = None;
        }
        let commands = [self.lead.take(), self.encoder.take()]
            .into_iter()
            .flatten()
            .map(|e| e.finish());
        let index = self.queue.submit(commands);
        if let Some(a) = &mut self.arena {
            a.submitted();
        }
        for r in [&mut self.ring, &mut self.query_ring].into_iter().flatten() {
            r.submitted();
        }
        self.caches.prune();
        reclaim_spares(self);
        for rb in self.to_map.drain(..) {
            let state = rb.0.state.clone();
            let Some(buffer) = &rb.0.buffer else {
                continue;
            };
            buffer.map_async(wgpu::MapMode::Read, .., move |result| {
                let s = if result.is_ok() {
                    MAP_READY
                } else {
                    MAP_FAILED
                };
                state.store(s, Ordering::Release);
                wait::wake();
            });
        }
        for d in self.destroy_after_submit.drain(..) {
            match d {
                Doomed::Buffer(b) => b.destroy(),
                Doomed::Texture(t) => t.destroy(),
            }
        }
        let done = Arc::new(AtomicBool::new(false));
        let (flag, completed, serial) = (done.clone(), self.completed.clone(), self.serial);
        self.queue.on_submitted_work_done(move || {
            flag.store(true, Ordering::Release);
            completed.fetch_max(serial, Ordering::AcqRel);
            wait::wake();
        });
        self.serial += 1;
        Fence { done, index }
    }

    fn fence_done(&mut self, fence: &Fence) -> bool {
        if !fence.done.load(Ordering::Acquire) {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        fence.done.load(Ordering::Acquire)
    }

    fn wait_fence(&mut self, fence: &Fence) {
        block_until(
            self,
            || fence.done.load(Ordering::Acquire),
            Some(fence.index.clone()),
        );
    }

    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Readback, GpuError> {
        read_back_impl(self, src)
    }

    fn read_queries(
        &mut self,
        set: &QuerySet,
        first: u32,
        count: u32,
    ) -> Result<Readback, GpuError> {
        read_queries_impl(self, set, first, count)
    }

    fn read_state(&mut self, readback: &Readback) -> ReadState {
        let Some(readback) = readback_of(self, readback) else {
            self.fail(GpuError::GONE);
            return ReadState::Ready;
        };
        if readback.state.load(Ordering::Acquire) == MAP_PENDING {
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        match readback.state.load(Ordering::Acquire) {
            MAP_PENDING => ReadState::Pending,
            _ => ReadState::Ready,
        }
    }

    fn wait_read(&mut self, readback: &Readback) {
        let Some(readback) = readback_of(self, readback) else {
            return self.fail(GpuError::GONE);
        };
        let pending = readback.state.load(Ordering::Acquire) == MAP_PENDING;
        if pending && readback.serial == self.serial {
            self.submit();
        }
        let state = readback.state.clone();
        block_until(
            self,
            move || state.load(Ordering::Acquire) != MAP_PENDING,
            None,
        );
    }

    fn finish_read(&mut self, readback: Readback, dst: &mut [u8]) {
        finish_read_impl(self, readback, dst);
    }

    fn finish_read_into(&mut self, readback: Readback, dst: &impl VolatileBytes) {
        finish_read_into_impl(self, readback, dst);
    }

    fn create_surface(&mut self, window: u32) -> Result<Surface, GpuError> {
        let raw = self
            .windows
            .remove(&window)
            .ok_or(GpuError::Invalid("no window of that number was registered"))?;
        let number = self.id();
        let made = HeldSurface(Rc::new(SurfaceObj {
            raw,
            current: RefCell::new(None),
            format: Cell::new(Format::Rgba8Unorm),
            config: RefCell::new(None),
            unshown: RefCell::new(None),
            stand_in: RefCell::new(None),
        }));
        self.objects.surfaces.insert(number, made);
        Ok(Surface(number))
    }

    fn configure_surface(
        &mut self,
        surface: &Surface,
        format: Format,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<(), GpuError> {
        let surface = surface_of(self, surface).ok_or(GpuError::Invalid(GpuError::GONE))?;
        if !self.caps.formats.uses[format as usize].contains(FormatUse::PRESENT) {
            return Err(GpuError::Invalid("surface format without PRESENT"));
        }
        let raw = convert::texture_format(format).ok_or(GpuError::Invalid("surface format"))?;
        // A window that does not show the format's values as sRGB (a Metal layer has no
        // `rgba8unorm`; natively a `rgba16float` window takes linear light, or has no such format)
        // is configured in an 8-bit format it shows so, the other byte order first, and frames are
        // drawn into a stand-in of the format: shown as sRGB values clipped to zero to one either
        // way.
        let window = surface.raw.get_capabilities(&self.adapter);
        let takes = |f: &wgpu::TextureFormat| {
            window
                .color_spaces(*f)
                .contains(wgpu::SurfaceColorSpaces::SRGB)
        };
        let shown = if takes(&raw) {
            raw
        } else {
            use wgpu::TextureFormat as T;
            let order = match raw {
                T::Rgba8Unorm => [T::Bgra8Unorm, T::Rgba8Unorm],
                _ => [T::Rgba8Unorm, T::Bgra8Unorm],
            };
            let Some(other) = order.into_iter().find(takes) else {
                self.fail("the window takes no 8-bit unorm format to present in");
                return Err(GpuError::DeviceLost);
            };
            other
        };
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST;
        *surface.stand_in.borrow_mut() = (shown != raw).then(|| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: raw,
                usage: usage | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        });
        let config = wgpu::SurfaceConfiguration {
            usage,
            format: shown,
            width,
            height,
            present_mode: if vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Srgb,
        };
        surface.raw.configure(&self.device, &config);
        surface.format.set(format);
        *surface.config.borrow_mut() = Some(config);
        *surface.unshown.borrow_mut() = None;
        Ok(())
    }

    fn current_texture(&mut self, surface: &Surface) -> Result<TextureView, GpuError> {
        let surface = surface_of(self, surface).ok_or(GpuError::Invalid(GpuError::GONE))?;
        let config = surface.config.borrow().clone().ok_or(GpuError::Invalid(
            "current_texture before configure_surface",
        ))?;
        use wgpu::CurrentSurfaceTexture as C;
        let mut got = surface.raw.get_current_texture();
        if matches!(got, C::Outdated) {
            // The window changed under the configuration: configure it again as it was.
            surface.raw.configure(&self.device, &config);
            got = surface.raw.get_current_texture();
        }
        let (texture, width, height) = match got {
            C::Success(t) | C::Suboptimal(t) => {
                let view = match &*surface.stand_in.borrow() {
                    Some(s) => s.create_view(&Default::default()),
                    None => t.texture.create_view(&Default::default()),
                };
                let size = (t.texture.width(), t.texture.height());
                *surface.current.borrow_mut() = Some(Frame::Shown(t));
                (view, size.0, size.1)
            }
            // The window shows nothing now, as a real swap chain's occluded window: the frame is
            // drawn into a texture of the same size and not shown.
            C::Timeout | C::Occluded | C::Outdated => {
                let device = &self.device;
                let view = surface
                    .unshown
                    .borrow_mut()
                    .get_or_insert_with(|| {
                        device.create_texture(&wgpu::TextureDescriptor {
                            label: None,
                            size: wgpu::Extent3d {
                                width: config.width,
                                height: config.height,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: convert::texture_format(surface.format.get())
                                .expect("configured"),
                            usage: config.usage,
                            view_formats: &[],
                        })
                    })
                    .create_view(&Default::default());
                *surface.current.borrow_mut() = Some(Frame::Unshown);
                (view, config.width, config.height)
            }
            C::Lost => {
                // The platform lost the window's swap chain (Vulkan's surface lost, a failed wait
                // on Direct3D 12's frame latency object); only the host's hook holds the window to
                // make a surface from again.
                self.fail(
                    "the window's surface was lost; the back end holds no window to make it from",
                );
                return Err(GpuError::DeviceLost);
            }
            C::Validation => {
                self.fail("getting the window's texture failed validation");
                return Err(GpuError::DeviceLost);
            }
        };
        let number = self.id();
        let made = HeldView(Rc::new(ViewObj {
            id: number,
            format: surface.format.get(),
            texture: None,
            views: RefCell::new((texture.clone(), texture)),
            desc: None,
            made_for: Cell::new(0),
            format_usages: self.usages[surface.format.get() as usize],
            width,
            height,
            samples: 1,
            three_d: false,
        }));
        self.objects.views.insert(number, made);
        Ok(TextureView(number))
    }

    fn present(&mut self, surface: &Surface) -> Result<(), GpuError> {
        let surface = surface_of(self, surface).ok_or(GpuError::Invalid(GpuError::GONE))?;
        let frame = surface
            .current
            .borrow_mut()
            .take()
            .ok_or(GpuError::Invalid("present without current_texture"))?;
        if let (Frame::Shown(t), Some(s)) = (&frame, &*surface.stand_in.borrow()) {
            present_by_draw(self, s, &t.texture);
        }
        self.submit();
        if let Frame::Shown(texture) = frame {
            self.queue.present(texture);
        }
        self.presented += 1;
        if self.presented.is_power_of_two() {
            eprintln!("{}", self.frame_report());
        }
        Ok(())
    }

    fn resolve_queries(
        &mut self,
        set: &QuerySet,
        first: u32,
        count: u32,
        dst: &Buffer,
        dst_offset: u64,
    ) -> Result<(), GpuError> {
        resolve_queries_impl(self, set, first, count, dst, dst_offset)
    }

    fn dispatch(
        &mut self,
        call: &wcontainers_gpu::draw::DispatchCall<'_, Self>,
    ) -> Result<(), GpuError> {
        dispatch_impl(self, call)
    }

    fn write_timestamp(&mut self, set: &QuerySet, index: u32) -> Result<(), GpuError> {
        let set = query_set_of(self, set).ok_or(GpuError::Invalid(GpuError::GONE))?;
        if !self.caps.features.contains(Features::TIMESTAMPS) {
            return Err(GpuError::Absent(Features::TIMESTAMPS));
        }
        if set.kind != QueryKind::Timestamp || index >= set.count {
            return Err(GpuError::Invalid("a timestamp outside a timestamp set"));
        }
        if self.pass.is_some() {
            return Err(GpuError::Invalid("timestamps are written outside a pass"));
        }
        // WebGPU writes timestamps where passes begin and end: an empty compute pass carries it,
        // written once the work recorded before it is done.
        let (page, slot) = QuerySetObj::own_slot(index);
        let raw = set.page(page);
        self.encoder()
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set: &raw,
                    beginning_of_pass_write_index: Some(slot),
                    end_of_pass_write_index: None,
                }),
            });
        Ok(())
    }

    fn timestamps_disjoint(&mut self) -> Result<bool, GpuError> {
        if !self.caps.features.contains(Features::TIMESTAMPS) {
            return Err(GpuError::Absent(Features::TIMESTAMPS));
        }
        // WebGPU reports no clock changes.
        Ok(false)
    }

    fn capture_vertices(
        &mut self,
        call: &wcontainers_gpu::draw::DrawCall<'_, Self>,
        targets: wcontainers_gpu::draw::CaptureTargets<'_, Self>,
        rasterize: bool,
    ) -> Result<(), GpuError> {
        let _ = (call, targets, rasterize);
        Err(GpuError::Absent(Features::TRANSFORM_FEEDBACK))
    }

    fn create_resource_table(&mut self, size: u32) -> Result<Self::ResourceTable, GpuError> {
        let _ = size;
        Err(GpuError::Absent(Features::RESOURCE_TABLES))
    }

    fn write_resource_table(
        &mut self,
        table: &Self::ResourceTable,
        first: u32,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<(), GpuError> {
        let _ = (table, first, entries);
        Err(GpuError::Absent(Features::RESOURCE_TABLES))
    }
}

/// Pauses the device thread until `done` holds or the device is lost (plan section 4.8).
fn block_until(gpu: &mut WebGpu, done: impl Fn() -> bool, index: Option<wgpu::SubmissionIndex>) {
    let lost = gpu.lost.clone();
    wait::until(&gpu.device, index, || {
        done() || lost.lock().unwrap().is_some()
    });
}

/// Whether a format has stencil (true) or is a depth format.
fn has_stencil(format: Format) -> bool {
    matches!(
        FORMAT_FACTS[format as usize].kind,
        FormatKind::DepthStencil | FormatKind::Stencil
    )
}

/// Bytes of one row of blocks and rows of blocks of `extent` in `format`.
fn row_bytes(format: Format, width: u32, height: u32) -> (u64, u64) {
    let (bw, bh, bytes) = FORMAT_FACTS[format as usize].block;
    (
        u64::from(width.div_ceil(bw)) * u64::from(bytes),
        u64::from(height.div_ceil(bh)),
    )
}
