//! The WebGL 2 back end of the wcontainers graphics interface, written on the OpenGL ES crate's trait.
//!
//! The same code draws on any implementation of that trait, following WebGL 2's rules: natively an OpenGL ES 3
//! context of an EGL library (ANGLE's or Mesa's; a test tool only), and in the browser WebGL 2 once the OpenGL ES
//! crate has an implementation on it. Blocks are applied by difference:
//! a block is looked at only when its handle changed, and then only GL values that differ are set. The
//! interface's row 0 is the top: with EXT_clip_control the device draws upper-left where its rasterizer then
//! keeps Direct3D's top-left rule (a probe at creation); otherwise programs negate y under GL's lower-left
//! origin and the front face is swapped.

mod caps;
mod capture;
mod convert;
mod depth;
mod format;
mod host;
mod platform;
mod restart;
mod shader;
mod state;

/// The GLSL ES 3.00 the back end writes for a shader and the features it needs, for checking
/// shaders outside a context.
pub mod glsl {
    pub use crate::shader::{GlslSettings, ShaderText, needs, write};
}

use shader::{ProgramObj, ShaderObj, Slot};
use state::GlValues;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::rc::Rc;
use wcontainers_gpu::error::GpuError;
use wcontainers_gpu::{
    caps::*, draw::*, error::*, format::*, helpers::*, resource::*, shader::*, slow::*, state::*, *,
};
use wcontainers_opengl_es::constants as gles;
use wcontainers_opengl_es::{CompressedPixelUnpack, OpenGlEs, PixelPack, PixelUnpack};

/// Ids name objects in the state the back end last applied; they are never reused.
type Id = u64;

/// The sampler id a unit records when it holds the back end's nearest sampler, for textures read only
/// by loads.
const POINT_SAMPLER: Id = Id::MAX - 1;

/// What a pass drawing into targets whose formats lack different channels needs.
const MIXED_CHANNELS: &str = "a pass drawing into formats lacking different channels together without OES_draw_buffers_indexed";

/// The instance divisor of a stream that reads one element for every vertex and instance.
const NEVER_ADVANCES: u32 = 1 << 31;

#[derive(Debug)]
struct BufferObj {
    id: Id,
    size: u64,
    usage: u32,
    /// The GL buffer for everything but indices, and the one for indices (WebGL never lets one GL
    /// buffer hold both).
    data: Option<u32>,
    index: Option<u32>,
    /// A range of one of the two GL buffers that a GPU write could not reach; the other holds it.
    stale: Cell<Option<Stale>>,
    /// Where indices may be GL's restart value, for buffers that hold indices.
    maxima: RefCell<restart::Maxima>,
}

/// A stale byte range of the index GL buffer (`index`) or of the data one.
#[derive(Clone, Copy, Debug)]
struct Stale {
    index: bool,
    start: u64,
    end: u64,
}

/// Where a texture's texels live: a GL texture, or for a multisampled one a renderbuffer (WebGL 2 has
/// no multisampled textures; their samples are drawn, cleared, resolved and copied whole, never read in
/// programs, so `MULTISAMPLE_READ` is absent).
#[derive(Clone, Copy, Debug)]
enum Storage {
    Texture(u32),
    Samples(u32),
}

#[derive(Debug)]
struct TextureObj {
    id: Id,
    storage: Storage,
    target: u32,
    desc: TextureDesc,
    gl: format::GlFormat,
    /// How its texels are read back, where they can be.
    read: Option<format::ReadAs>,
    /// The base and max level GL holds for the texture, set from the view a draw samples.
    levels: Cell<(u32, u32)>,
    /// Shared with its sRGB twin: 0 while both hold the same texels, else the side (1 the texture as
    /// made, 2 its twin) holding newer ones.
    newest: Rc<Cell<u8>>,
    side: u8,
    /// On the texture as made: its twin, a second GL texture in the twin format, made at the first
    /// view in that format (WebGL 2 has no views).
    twin: std::cell::OnceCell<TextureRef>,
    /// On a twin: the texture it is the twin of.
    own: Option<std::rc::Weak<TextureObj>>,
    /// With WEBGL_stencil_texturing, whether programs fetching a depth-stencil texture get its stencil
    /// (else its depth).
    stencil_mode: Cell<bool>,
    /// Its bytes on the host, for formats GL cannot read back.
    host: Option<host::HostCopy>,
    /// The levels, by bit, draws wrote texels of that its host copy lacks (signed normalised
    /// formats with EXT_render_snorm).
    drawn: Cell<u32>,
    /// The levels, by bit, whose host copy may hold a signed normalised minimum, which a draw reads
    /// as the value above.
    minimum: Cell<u32>,
    /// The format the texture's texels are stored in where GL holds them as another: block-compressed
    /// and two-pixel formats decoded, 8-bit alpha widened to RGBA.
    decoded: Option<Format>,
}

#[derive(Debug)]
struct ViewObj {
    id: Id,
    texture: TextureRef,
    desc: TextureViewDesc,
}

#[derive(Debug)]
struct SamplerObj {
    id: Id,
    raw: u32,
    /// Its twin whose minimum level of detail is 0, for units that gathers read, where its own
    /// minimum is above 0.
    level_zero: Option<(Id, u32)>,
}

/// What one binding of a set holds.
#[derive(Debug)]
enum Bound {
    Uniform(BufferRef, u64, u64),
    Texture(ViewRef),
    Sampler(SamplerRef),
}

#[derive(Debug)]
struct SetObj {
    id: Id,
    entries: Vec<(u32, Bound)>,
}

/// A read back; `F` is the OpenGL ES fence type.
#[derive(Debug)]
struct ReadbackObj<F> {
    /// The pack buffer texels or bytes are copied into; none for query results.
    pbo: Option<u32>,
    /// The queries read, in order; none for a query never begun or written, which reads zero.
    queries: Vec<Option<Rc<u32>>>,
    /// The queries are timestamps (64-bit results).
    timestamps: bool,
    size: usize,
    sync: Option<F>,
    /// Waited for: WebGL updates a sync object's status only between tasks, so after `finish` it is
    /// known done though the sync object does not say so yet.
    waited: bool,
    /// Texels read wider than they are stored, kept narrow when finished.
    repack: Option<format::ReadAs>,
    /// Texels read from a host copy, ready when made.
    host: Option<Vec<u8>>,
}

/// A point in the submitted work: its sync object, none where the context was lost when the fence was made
/// (`F` is the OpenGL ES fence type, a number of the OpenGL ES crate's), and whether it is known done, shared
/// by every clone.
#[derive(Clone, Debug)]
pub struct Fence<F> {
    pub sync: Option<F>,
    pub done: Rc<Cell<bool>>,
}

/// An object as the back end holds it: a clone names the same object.
#[derive(Debug)]
struct Handle<T>(Rc<T>);

impl TextureObj {
    /// The GL texture; none for a multisampled texture.
    fn tex(&self) -> Option<u32> {
        match self.storage {
            Storage::Texture(t) => Some(t),
            Storage::Samples(_) => None,
        }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Handle(self.0.clone())
    }
}

impl<T> core::ops::Deref for Handle<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

type BufferRef = Handle<BufferObj>;
type TextureRef = Handle<TextureObj>;
type ViewRef = Handle<ViewObj>;
type SamplerRef = Handle<SamplerObj>;
type ShaderRef = Handle<ShaderObj>;
type ProgramRef = Handle<ProgramObj>;
/// A state block: its id and the GL values it stands for.
type BlockRef<T> = Handle<(Id, T)>;
type SetRef = Handle<SetObj>;
type ReadbackRef<F> = Handle<core::cell::RefCell<ReadbackObj<F>>>;

/// A query set: each query's GL object, made when the query is first begun or written. A read back
/// holds the objects it reads, so a query begun again before the read finished gets a new one.
#[derive(Debug)]
struct QuerySetObj {
    kind: QueryKind,
    slots: core::cell::RefCell<Vec<Option<Rc<u32>>>>,
}

type QuerySetRef = Handle<QuerySetObj>;

/// Defines a handle the interface's associated types name: the number the back end keeps the object under
/// until it is destroyed.
macro_rules! handle {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug)]
        pub struct $name(pub u64);
    };
}

handle!(Buffer);
handle!(Texture);
handle!(TextureView);
handle!(Sampler);
handle!(Shader);
handle!(Program);
handle!(Blend);
handle!(DepthStencil);
handle!(Raster);
handle!(VertexLayout);
handle!(BindingSet);
handle!(QuerySet);
handle!(Readback);

/// The objects the handles name, by their numbers, from their making to their destroying (a read back's
/// until it is finished). `F` is the OpenGL ES fence type.
#[derive(Debug)]
struct Objects<F> {
    buffers: HashMap<u64, BufferRef>,
    textures: HashMap<u64, TextureRef>,
    views: HashMap<u64, ViewRef>,
    samplers: HashMap<u64, SamplerRef>,
    shaders: HashMap<u64, ShaderRef>,
    programs: HashMap<u64, ProgramRef>,
    blends: HashMap<u64, BlockRef<state::GlBlend>>,
    depth_stencils: HashMap<u64, BlockRef<state::GlDepthStencil>>,
    rasters: HashMap<u64, BlockRef<state::GlRaster>>,
    layouts: HashMap<u64, BlockRef<state::GlLayout>>,
    sets: HashMap<u64, SetRef>,
    query_sets: HashMap<u64, QuerySetRef>,
    readbacks: HashMap<u64, ReadbackRef<F>>,
}

impl<F> Default for Objects<F> {
    fn default() -> Self {
        Objects {
            buffers: HashMap::new(),
            textures: HashMap::new(),
            views: HashMap::new(),
            samplers: HashMap::new(),
            shaders: HashMap::new(),
            programs: HashMap::new(),
            blends: HashMap::new(),
            depth_stencils: HashMap::new(),
            rasters: HashMap::new(),
            layouts: HashMap::new(),
            sets: HashMap::new(),
            query_sets: HashMap::new(),
            readbacks: HashMap::new(),
        }
    }
}

/// The object a handle's number names in `table`, while it is not destroyed.
fn held<H: Clone>(table: &HashMap<u64, H>, number: u64) -> Option<H> {
    table.get(&number).cloned()
}

/// A texture region as the back end reads it: its texture as the back end holds it.
#[derive(Clone, Debug)]
struct Region {
    texture: TextureRef,
    mip: u32,
    layer: u32,
    origin: Origin,
    stencil: bool,
}

/// A buffer's part of a copy between a buffer and a texture, its buffer as the back end holds it.
#[derive(Clone, Debug)]
struct Place {
    buffer: BufferRef,
    offset: u64,
    row_pitch: u32,
    rows_per_layer: u32,
}

/// What a read back reads, its object as the back end holds it.
#[derive(Clone, Debug)]
enum Source {
    Buffer {
        buffer: BufferRef,
        offset: u64,
        size: u64,
    },
    Texture {
        region: Region,
        extent: Extent,
    },
}

/// An object to destroy, as the back end holds it.
enum Held {
    Buffer(BufferRef),
    Texture(TextureRef),
    Sampler(SamplerRef),
    Shader(ShaderRef),
    Program(ProgramRef),
    QuerySet(QuerySetRef),
    Surface(u64),
}

/// The region `r` names, while its texture is not destroyed.
fn region_of<O: OpenGlEs>(gpu: &Gl<O>, r: &TextureRegion<'_, Gl<O>>) -> Option<Region> {
    Some(Region {
        texture: held(&gpu.objects.textures, r.texture.0)?,
        mip: r.mip,
        layer: r.layer,
        origin: r.origin,
        stencil: r.stencil,
    })
}

/// The buffer part `p` names, while its buffer is not destroyed.
fn place_of<O: OpenGlEs>(gpu: &Gl<O>, p: &CopyBufferTexture<'_, Gl<O>>) -> Option<Place> {
    Some(Place {
        buffer: held(&gpu.objects.buffers, p.buffer.0)?,
        offset: p.offset,
        row_pitch: p.row_pitch,
        rows_per_layer: p.rows_per_layer,
    })
}

/// A vertex stream of a draw, its buffer as the back end holds it.
#[derive(Clone, Debug)]
struct Stream {
    buffer: BufferRef,
    offset: u64,
}

/// A draw's index buffer, as the back end holds it.
#[derive(Clone, Debug)]
struct Index {
    buffer: BufferRef,
    offset: u64,
    format: IndexFormat,
}

/// The objects a draw names, as the back end holds them.
#[derive(Clone, Debug)]
struct Drawn {
    program: ProgramRef,
    blend: BlockRef<state::GlBlend>,
    depth_stencil: BlockRef<state::GlDepthStencil>,
    raster: BlockRef<state::GlRaster>,
    vertex_layout: BlockRef<state::GlLayout>,
    bindings: [Option<SetRef>; MAX_BINDING_SETS],
    vertex_buffers: Vec<Option<Stream>>,
    index: Option<Index>,
}

/// A colour target of a pass, its views as the back end holds them.
struct HeldColour {
    view: ViewRef,
    depth_slice: u32,
    resolve: Option<ViewRef>,
    load: LoadOp<[u32; 4]>,
    store: StoreOp,
}

/// The depth and stencil target of a pass, its view as the back end holds it.
struct HeldDepth {
    view: ViewRef,
    depth: Option<AspectOps<u32>>,
    stencil: Option<AspectOps<u8>>,
}

/// A pass as the back end holds its objects.
struct HeldPass {
    colors: [Option<HeldColour>; MAX_COLOR_TARGETS],
    depth: Option<HeldDepth>,
    empty: Option<EmptyPass>,
    occlusion: Option<QuerySetRef>,
}

/// What `pass` names, while none of it is destroyed.
fn pass_of<O: OpenGlEs>(gpu: &Gl<O>, pass: &PassDesc<'_, Gl<O>>) -> Option<HeldPass> {
    let o = &gpu.objects;
    let mut colors: [Option<HeldColour>; MAX_COLOR_TARGETS] = Default::default();
    for (slot, c) in colors.iter_mut().zip(&pass.colors) {
        if let Some(c) = c {
            *slot = Some(HeldColour {
                view: held(&o.views, c.view.0)?,
                depth_slice: c.depth_slice,
                resolve: match c.resolve {
                    Some(r) => Some(held(&o.views, r.0)?),
                    None => None,
                },
                load: c.load,
                store: c.store,
            });
        }
    }
    let depth = match &pass.depth {
        Some(d) => Some(HeldDepth {
            view: held(&o.views, d.view.0)?,
            depth: d.depth,
            stencil: d.stencil,
        }),
        None => None,
    };
    let occlusion = match pass.occlusion {
        Some(q) => Some(held(&o.query_sets, q.0)?),
        None => None,
    };
    Some(HeldPass {
        colors,
        depth,
        empty: pass.empty,
        occlusion,
    })
}

/// What `call` names, while none of it is destroyed.
fn drawn_of<O: OpenGlEs>(gpu: &Gl<O>, call: &DrawCall<'_, Gl<O>>) -> Option<Drawn> {
    let o = &gpu.objects;
    let mut bindings: [Option<SetRef>; MAX_BINDING_SETS] = Default::default();
    for (slot, set) in bindings.iter_mut().zip(&call.bindings) {
        if let Some(set) = set {
            *slot = Some(held(&o.sets, set.0)?);
        }
    }
    let mut vertex_buffers = Vec::with_capacity(call.vertex_buffers.len());
    for stream in call.vertex_buffers {
        vertex_buffers.push(match stream {
            Some(s) => Some(Stream {
                buffer: held(&o.buffers, s.buffer.0)?,
                offset: s.offset,
            }),
            None => None,
        });
    }
    let index = match &call.index {
        Some(i) => Some(Index {
            buffer: held(&o.buffers, i.buffer.0)?,
            offset: i.offset,
            format: i.format,
        }),
        None => None,
    };
    Some(Drawn {
        program: held(&o.programs, call.program.0)?,
        blend: held(&o.blends, call.blend.0)?,
        depth_stencil: held(&o.depth_stencils, call.depth_stencil.0)?,
        raster: held(&o.rasters, call.raster.0)?,
        vertex_layout: held(&o.layouts, call.vertex_layout.0)?,
        bindings,
        vertex_buffers,
        index,
    })
}

/// What `s` names, while its object is not destroyed.
fn source_of<O: OpenGlEs>(gpu: &Gl<O>, s: &ReadSource<'_, Gl<O>>) -> Option<Source> {
    Some(match s {
        ReadSource::Buffer {
            buffer,
            offset,
            size,
        } => Source::Buffer {
            buffer: held(&gpu.objects.buffers, buffer.0)?,
            offset: *offset,
            size: *size,
        },
        ReadSource::Texture { region, extent } => Source::Texture {
            region: region_of(gpu, region)?,
            extent: *extent,
        },
    })
}

/// WebGL 2 has no resource tables, so none is ever made.
#[derive(Clone, Debug)]
pub enum NoTable {}

handle!(
    /// The one surface a back end has: its context's canvas.
    Surface
);

/// The surface's frame: the texture `current_texture` hands out, copied into the canvas's drawing
/// buffer by `present` (the drawing buffer is not a texture, so nothing can draw into it directly).
#[derive(Debug)]
struct SurfaceState {
    id: Id,
    frame: Option<(TextureRef, ViewRef)>,
}

/// Which mip, layer and slice of which texture an attachment is.
type Attachment = (Id, u32, u32);

/// The attachments of a framebuffer: eight colours and depth-stencil.
type FramebufferKey = [Option<Attachment>; MAX_COLOR_TARGETS + 1];

/// What a running pass must do when it ends.
#[derive(Debug)]
struct Pass {
    framebuffer: u32,
    discard: Points,
    occlusion: Option<QuerySetRef>,
    /// An occlusion query is running.
    counting: bool,
    /// Per colour target: the multisampled texture and the view it resolves into at the end.
    resolves: [Option<(TextureRef, ViewRef)>; MAX_COLOR_TARGETS],
    /// The area every attachment holds, which scissors are clipped to.
    size: (u32, u32),
    /// How many colour targets it draws into.
    color_targets: usize,
}

/// Attachment points of one framebuffer: eight colours, depth and stencil.
#[derive(Debug, Default)]
struct Points {
    points: [u32; MAX_COLOR_TARGETS + 2],
    len: usize,
}

impl Points {
    fn push(&mut self, point: u32) {
        self.points[self.len] = point;
        self.len += 1;
    }

    fn get(&self) -> &[u32] {
        &self.points[..self.len]
    }
}

/// What the back end last set beyond [`GlValues`], by id.
#[derive(Debug)]
struct Applied {
    program: Id,
    blend: Id,
    depth_stencil: Id,
    raster: Id,
    stencil_reference: u8,
    sets: [Id; MAX_BINDING_SETS],
    /// Per unit: the view and sampler bound.
    units: Vec<(Id, Id)>,
    /// Per uniform block binding point: buffer, offset, size.
    blocks: Vec<(Id, u64, u64)>,
    /// The vertex layout, and each attribute's buffer and byte offset.
    layout: Id,
    attributes: Vec<(Id, u64)>,
    enabled: u32,
    index: Id,
    draw_framebuffer: Option<u32>,
    read_framebuffer: Option<u32>,
}

impl Applied {
    fn new() -> Applied {
        Applied {
            program: 0,
            blend: 0,
            depth_stencil: 0,
            raster: 0,
            stencil_reference: 0,
            sets: [0; MAX_BINDING_SETS],
            units: Vec::new(),
            blocks: Vec::new(),
            layout: 0,
            attributes: Vec::new(),
            enabled: 0,
            index: 0,
            draw_framebuffer: None,
            read_framebuffer: None,
        }
    }
}

/// The start of the line the back end prints on standard error at each power of two frames
/// presented: the frames, then every slow path taken as `name=count`, so any program's log gives
/// what a frame costs between two reports.
pub const FRAME_REPORT: &str = "webgl: frames=";

/// What a WebGL 2 back end is made from natively: the folder of the EGL library to load (ANGLE's or Mesa's; the
/// system's where none is named) and the WebGL extensions to leave unused.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct GlConfig {
    pub egl: Option<std::path::PathBuf>,
    pub off: Vec<String>,
}

/// Natively the back end draws on the OpenGL ES crate's direct implementation, over the EGL crate's.
#[cfg(not(target_arch = "wasm32"))]
impl StandaloneGpu for Gl<wcontainers_opengl_es_direct::OpenGlEsContext> {
    type Config = GlConfig;
    /// Why the EGL library gave no context, as its loader or EGL said.
    type NewError = String;

    fn new(config: GlConfig) -> Result<Self, String> {
        let off: Vec<&str> = config.off.iter().map(String::as_str).collect();
        let (gl, held) = platform::context(platform::library(config.egl.as_deref())?)?;
        let mut b = Gl::new(gl, UNUSABLE_UNIFORM_BLOCKS, &off);
        b.held = Some(held);
        Ok(b)
    }
}

/// What a WebGL 2 back end is made from in a browser: a canvas's WebGL 2 context as a value of the OpenGL ES
/// crate's trait (that crate's implementation on WebGL is still to come) and the WebGL extensions to leave
/// unused.
#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
pub struct GlConfig<O> {
    pub gl: O,
    pub off: Vec<String>,
}

/// In the browser the back end draws on whatever implementation of the OpenGL ES crate's trait the page
/// makes from its canvas, and presents into that canvas.
#[cfg(target_arch = "wasm32")]
impl<O: OpenGlEs> StandaloneGpu for Gl<O> {
    type Config = GlConfig<O>;
    /// The back end is made on any context it is given.
    type NewError = core::convert::Infallible;

    fn new(config: GlConfig<O>) -> Result<Self, core::convert::Infallible> {
        let off: Vec<&str> = config.off.iter().map(String::as_str).collect();
        let mut b = Gl::new(config.gl, 0, &off);
        b.presents = true;
        Ok(b)
    }
}

/// Uniform blocks GL counts that a stage cannot use natively: mozangle 0.7.1 numbers a stage's blocks from
/// register b2 (b0 and b1 hold its own constants) and asserts each register below the block count, so a
/// program using the last two aborts the process.
#[cfg(not(target_arch = "wasm32"))]
const UNUSABLE_UNIFORM_BLOCKS: u32 = 2;

/// The WebGL 2 back end, drawing through `O`, a context of the OpenGL ES crate's trait. Made on the device
/// thread, which owns the GL context.
pub struct Gl<O: OpenGlEs> {
    gl: O,
    /// Whether the context draws into something a host shows (a canvas's drawing buffer), which a surface
    /// presents into. Natively it does not: the OpenGL ES crate gives no EGL window surface yet (its window
    /// loan waits on the owner's question of a window in the interface).
    presents: bool,
    extensions: BTreeSet<&'static str>,
    renderer: String,
    caps: Caps,
    slow: SlowPaths,
    helpers: Helpers<Gl<O>>,
    lost: Option<LossReason>,
    next_id: Id,
    /// Frames presented, for [`FRAME_REPORT`].
    presented: u64,
    /// How shaders are written; where programs negate y the front face is swapped.
    glsl: shader::GlslSettings,
    /// Whether EXT_clip_control is there: it sets the clip depth per raster block, and the window
    /// origin is upper-left where programs write y as given, lower-left where they negate it.
    clip_control: bool,
    values: GlValues,
    applied: Applied,
    /// The live shaders by their text, so shaders of equal text are one.
    shaders: HashMap<Rc<shader::ShaderText>, ShaderRef>,
    programs: HashMap<(Id, Id), ProgramRef>,
    framebuffers: HashMap<FramebufferKey, u32>,
    /// Framebuffers of passes without attachments, each on an `R8` renderbuffer of its own that no
    /// draw writes (GLES 3.0 has no framebuffer without attachments).
    empty_passes: HashMap<EmptyPass, u32>,
    empty_fragment: u32,
    pass: Option<Pass>,
    /// How many buffers have a stale range, so draws look for them only when some do.
    stale_buffers: u32,
    /// How many sRGB twins exist, so draws look at their textures only when some do.
    twins: u32,
    /// The channels draws leave in each colour target of the pass, four bits a target from the
    /// first (`format::unwritten`).
    pass_unwritten: u32,
    /// GL's last compile or link log, for tests and bug reports.
    last_log: String,
    surface: Option<SurfaceState>,
    /// The depth and stencil read programs, by [`Fetch`], made at their first use.
    depth_read: [Option<depth::DepthRead>; 5],
    /// The stencil bit draws' program, made at its first use.
    bits_program: Option<u32>,
    /// The stencil write programs for 2D and 2D array sources.
    stencil_write: [Option<depth::StencilWrite>; 2],
    /// The programs of copies by draws, by source format, destination format and source target, with
    /// their uniform `at`.
    /// Programs of `copy_by_draw` by formats and source target.
    copy_programs: HashMap<(Format, Format, u32), CopyProgram>,
    /// A nearest-filtering sampler for the back end's own fetches, made at its first use.
    point_sampler: Option<u32>,
    /// Whether blits between textures of one float format keep every bit (the probe at creation).
    float_blits: bool,
    /// How the device samples 24-bit depth, for [`depth::UNORM24`]: 0 for the nearest float, else the
    /// float it multiplies the stored value by (the probe at creation).
    depth24_reciprocal: f32,
    /// The stored 24-bit words sampling gives as another word on this device ([`depth::colliding`]),
    /// which a read of 24-bit depth decides by comparison passes; empty where none is (no pass).
    depth24_colliding: Vec<u32>,
    /// The comparison pass's program ([`depth::EQUAL`]), made at its first use.
    equal_pass: Option<depth::EqualPass>,
    /// The objects the handles name.
    objects: Objects<O::Fence>,
    /// The EGL objects the context lives in, destroyed after every field above.
    #[cfg(not(target_arch = "wasm32"))]
    held: Option<platform::Held>,
}

/// A program of `copy_by_draw`, with its uniforms `at` and, into unorm depth, `unit`.
type CopyProgram = (u32, i32, Option<i32>);

/// What the back end's own fetch draws read: depth or stencil of a 2D or 2D array texture, or depth
/// of a cube.
#[derive(Clone, Copy, Debug)]
enum Fetch {
    Depth2D,
    DepthArray,
    DepthCube,
    Stencil2D,
    StencilArray,
}

impl Fetch {
    fn shape(self) -> depth::Shape {
        match self {
            Fetch::Depth2D | Fetch::Stencil2D => depth::Shape::D2,
            Fetch::DepthArray | Fetch::StencilArray => depth::Shape::Array,
            Fetch::DepthCube => depth::Shape::Cube,
        }
    }

    fn stencil(self) -> bool {
        matches!(self, Fetch::Stencil2D | Fetch::StencilArray)
    }
}

/// Whether a fence sync object has signalled.
fn signaled<O: OpenGlEs>(gl: &O, sync: &O::Fence) -> bool {
    let mut status = [0];
    gl.get_synciv(sync, gles::SYNC_STATUS, &mut status);
    status[0] == gles::SIGNALED as i32
}

/// The name in `off` that takes blits keeping every bit of floats as missing, as where the probe
/// finds they do not.
pub const FLOAT_BLITS: &str = "float_blits";

impl<O: OpenGlEs> Gl<O> {
    /// Makes the back end on an OpenGL ES 3.0 context, of which `unusable_uniform_blocks` uniform blocks
    /// cannot be used by a stage. `off` lists WebGL extension names (and [`FLOAT_BLITS`]) to treat as
    /// missing, so tests can take the routes of browsers without them.
    fn new(gl: O, unusable_uniform_blocks: u32, off: &[&str]) -> Gl<O> {
        let extensions = platform::extensions(&platform::supported(&gl), off);
        let renderer = platform::string(&gl, gles::RENDERER);
        let mut caps = caps::caps(&gl, unusable_uniform_blocks, &extensions, &renderer);
        // Timestamps where the extension is present and the GPU writes them (it may give zero bits).
        caps.features.set(
            Features::TIMESTAMPS,
            extensions.contains("EXT_disjoint_timer_query_webgl2")
                && platform::timestamp_bits(&gl) > 0,
        );
        let clip_control = extensions.contains("EXT_clip_control");
        let noperspective = extensions.contains(platform::NOPERSPECTIVE_INTERPOLATION);
        gl.pixel_storei(gles::PACK_ALIGNMENT, 1);
        gl.pixel_storei(gles::UNPACK_ALIGNMENT, 1);
        // WebGL 2 always restarts strips at the largest index; GLES has it as a switch.
        #[cfg(not(target_arch = "wasm32"))]
        gl.enable(gles::PRIMITIVE_RESTART_FIXED_INDEX);
        let vao = platform::one(|n| gl.gen_vertex_arrays(n)).expect("a vertex array object");
        gl.bind_vertex_array(vao);
        // With the upper-left origin programs write y as given, unless the rasterizer then follows
        // the bottom-left rule (the probe); otherwise they negate y under GL's lower-left origin.
        let edge_probe = if clip_control {
            probe_upper_left(&gl)
        } else {
            Ok(false)
        };
        let flip_y = !matches!(edge_probe, Ok(true));
        let clip_depth = if clip_control {
            let origin = if flip_y {
                gles::LOWER_LEFT_EXT
            } else {
                gles::UPPER_LEFT_EXT
            };
            let _ = gl.clip_control_ext(origin, gles::ZERO_TO_ONE_EXT);
            gles::ZERO_TO_ONE_EXT
        } else {
            gles::NEGATIVE_ONE_TO_ONE_EXT
        };
        let empty_fragment = gl.create_shader(gles::FRAGMENT_SHADER);
        gl.shader_source(empty_fragment, &[b"#version 300 es\nvoid main() {}\n"]);
        gl.compile_shader(empty_fragment);
        let mut b = Gl {
            gl,
            presents: false,
            extensions,
            renderer,
            caps,
            slow: wcontainers_gpu::slow::NO_SLOW_PATHS,
            helpers: Helpers::default(),
            lost: None,
            next_id: 1,
            presented: 0,
            glsl: shader::GlslSettings {
                flip_y,
                noperspective,
            },
            clip_control,
            values: GlValues::initial(clip_depth),
            applied: Applied::new(),
            shaders: HashMap::new(),
            programs: HashMap::new(),
            framebuffers: HashMap::new(),
            empty_passes: HashMap::new(),
            empty_fragment,
            pass: None,
            stale_buffers: 0,
            twins: 0,
            pass_unwritten: 0,
            last_log: String::new(),
            surface: None,
            depth_read: Default::default(),
            bits_program: None,
            stencil_write: [None, None],
            copy_programs: HashMap::new(),
            point_sampler: None,
            float_blits: false,
            depth24_reciprocal: 0.0,
            depth24_colliding: Vec::new(),
            equal_pass: None,
            objects: Objects::default(),
            #[cfg(not(target_arch = "wasm32"))]
            held: None,
        };
        if let Err(log) = edge_probe {
            own_program_failed(&mut b, format!("webgl: the edge rule probe: {log}"));
        }
        match probe_depth24(&mut b) {
            Ok(unorm) => b.caps.features.set(Features::DEPTH24_UNORM, unorm),
            Err(log) => {
                own_program_failed(&mut b, format!("webgl: the 24-bit depth probe: {log}"));
            }
        }
        if b.caps.features.contains(Features::DEPTH24_UNORM) {
            match probe_depth24_sampling(&mut b) {
                Ok(reciprocal) => {
                    b.depth24_reciprocal = reciprocal;
                    b.depth24_colliding = depth::colliding(reciprocal);
                }
                Err(log) => {
                    own_program_failed(
                        &mut b,
                        format!("webgl: the 24-bit depth sampling probe: {log}"),
                    );
                }
            }
        }
        match probe_fractional_bias(&mut b) {
            Ok(fraction) => b
                .caps
                .features
                .set(Features::FRACTIONAL_DEPTH_BIAS, fraction),
            Err(log) => {
                own_program_failed(
                    &mut b,
                    format!("webgl: the fractional depth bias probe: {log}"),
                );
            }
        }
        b.float_blits = !off.contains(&FLOAT_BLITS) && probe_float_blits(&mut b);
        b
    }

    /// Whether the probe at creation found that blits keep every bit of 16-bit and 32-bit floats, so
    /// copies within one float format are blits; else they take the counted routes.
    pub fn exact_float_blits(&self) -> bool {
        self.float_blits
    }

    /// The renderer GL reports, which names the GPU.
    pub fn renderer(&self) -> &str {
        &self.renderer
    }

    /// The line printed at each power of two frames presented ([`FRAME_REPORT`]).
    pub fn frame_report(&self) -> String {
        let mut line = format!("{FRAME_REPORT}{}", self.presented);
        for path in SLOW_PATHS {
            let count = self.slow.counts[path as usize];
            if count > 0 {
                line += &format!(" {path:?}={count}");
            }
        }
        line
    }

    /// The WebGL extensions the back end found and uses.
    pub fn extensions(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.extensions.iter().copied()
    }

    /// Every extension the context reports.
    pub fn context_extensions(&self) -> Vec<String> {
        let mut all: Vec<String> = platform::supported(&self.gl).into_iter().collect();
        all.sort();
        all
    }

    /// GL's log of the last shader or program that failed to build.
    pub fn last_log(&self) -> &str {
        &self.last_log
    }
}

/// Whether `blitFramebuffer` between textures of one float format keeps every bit (NaN payloads
/// and signs, denormals), which GL does not promise: special 16-bit and 32-bit floats blitted,
/// drawn as bits into their unsigned integer twins and read back. False where float textures are
/// no targets (without EXT_color_buffer_float).
fn probe_float_blits<O: OpenGlEs>(gpu: &mut Gl<O>) -> bool {
    let halves = [
        0x7c01u16, 0x7dc4, 0xfe01, 0x7e00, 0x0001, 0x83ff, 0x8000, 0xfc00,
    ];
    let singles = [0x7f80_0001u32, 0xffc0_0001, 0x0000_0001, 0x807f_ffff];
    let cases = [
        (
            Format::Rgba16Float,
            Format::Rgba16Uint,
            halves
                .iter()
                .flat_map(|h| h.to_le_bytes())
                .collect::<Vec<u8>>(),
        ),
        (
            Format::Rgba32Float,
            Format::Rgba32Uint,
            singles.iter().flat_map(|s| s.to_le_bytes()).collect(),
        ),
    ];
    cases.iter().all(|(format, twin, bytes)| {
        gpu.caps.formats.uses[*format as usize].contains(FormatUse::RENDER)
            && blit_keeps(gpu, *format, *twin, bytes)
    })
}

/// Whether `bytes`, one row of `format`, come back the same after a blit into a second texture.
fn blit_keeps<O: OpenGlEs>(gpu: &mut Gl<O>, format: Format, twin: Format, bytes: &[u8]) -> bool {
    let width = bytes.len() as u32 / FORMAT_FACTS[format as usize].block.2;
    let make = |b: &mut Gl<O>, format| {
        make_texture(
            b,
            &TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width,
                height: 1,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::SAMPLE
                    | TextureUsage::RENDER
                    | TextureUsage::COPY_SRC
                    | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            },
        )
    };
    let made = [make(gpu, format), make(gpu, format), make(gpu, twin)];
    let kept = match &made {
        [Ok(from), Ok(to), Ok(bits)] => {
            let region = |texture| Region {
                texture,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            };
            let extent = Extent {
                width,
                height: 1,
                depth_or_layers: 1,
            };
            let row = bytes.len() as u32;
            write_texture_held(
                gpu,
                &region(from.clone()),
                extent,
                bytes,
                row,
                1,
                WriteMode::Ordered,
            );
            blit_same(gpu, &region(from.clone()), &region(to.clone()), extent);
            let mut got = vec![0; bytes.len()];
            copy_by_draw(gpu, &region(to.clone()), &region(bits.clone()), extent)
                && read_texture_now(gpu, &region(bits.clone()), extent, &mut got).is_ok()
                && got == bytes
        }
        _ => false,
    };
    for t in made.into_iter().flatten() {
        destroy_held(gpu, Held::Texture(t));
    }
    kept
}

/// The 24-bit depth probe (plan section 4.11): a triangle at depth 0.1 drawn into a 1 by 1
/// `depth24plus` texture without and with a constant bias of one unit, each depth then fetched as
/// float bits. One 24-bit step between them means 24-bit unsigned normalised storage, one float
/// step 32-bit float storage; anything else is a bug of the back end's.
fn probe_depth24<O: OpenGlEs>(gpu: &mut Gl<O>) -> Result<bool, String> {
    let bits = probe_bias(gpu, Format::Depth24Plus, 0.1, [0.0, 1.0])?;
    // A float near 0.1 holds a 24-bit step to an eighth, so a step shows as 8 float steps or so.
    let steps = bits.map(|b| f64::from(f32::from_bits(b)) * 16_777_215.0);
    if bits[1] == bits[0] + 1 {
        Ok(false)
    } else if (steps[1] - steps[0] - 1.0).abs() < 0.25 {
        Ok(true)
    } else {
        Err(format!(
            "depths {} and {} with and without the bias",
            f32::from_bits(bits[1]),
            f32::from_bits(bits[0])
        ))
    }
}

/// The 24-bit depth sampling probe: a triangle at depth 0.45 drawn into a 1 by 1 `depth24plus` texture
/// stores some k between 2^22 1.5 and 2^23, fetched as float bits. There v / (2^24 - 1) lies more than
/// half and at most one float step above k 2^-24, whose last bit is 0, so the nearest float's last bit is
/// 1; k times the float nearest 1 / (2^24 - 1), which is 2^-24 (1 + 2^-23), lies 1.5 to 2 steps above,
/// so that product's last bit is 0. Gives [`Gl::depth24_reciprocal`]: 0 for the nearest float, else
/// that float.
fn probe_depth24_sampling<O: OpenGlEs>(gpu: &mut Gl<O>) -> Result<f32, String> {
    let [bits, _] = probe_bias(gpu, Format::Depth24Plus, 0.45, [0.0, 0.0])?;
    let d = f32::from_bits(bits);
    if !(0.375..0.5).contains(&d) {
        return Err(format!("depth {d} for 0.45"));
    }
    Ok(if bits & 1 == 1 {
        0.0
    } else {
        (1.0 / 16_777_215.0f64) as f32
    })
}

/// The edge rule probe: [`depth::build_edge_probe`]'s triangle drawn with the upper-left origin into a
/// 4 by 4 `rgba8` renderbuffer. True when it covers texel (0, 0), Direct3D's top-left rule; false when
/// the rasterizer follows the bottom-left rule there (Mesa's state tracker sets it for an upper-left
/// origin, st_atom_rasterizer.c). Leaves the program, the framebuffer, the renderbuffer and the
/// viewport unbound or unknown to the back end's state; the caller sets the origin after.
fn probe_upper_left<O: OpenGlEs>(gl: &O) -> Result<bool, String> {
    const N: i32 = 4;
    let program = depth::build_edge_probe(gl)?;
    let renderbuffer = platform::one(|n| gl.gen_renderbuffers(n)).ok_or("no renderbuffer")?;
    gl.bind_renderbuffer(gles::RENDERBUFFER, renderbuffer);
    gl.renderbuffer_storage(gles::RENDERBUFFER, gles::RGBA8, N, N);
    let framebuffer = platform::one(|n| gl.gen_framebuffers(n)).ok_or("no framebuffer")?;
    gl.bind_framebuffer(gles::FRAMEBUFFER, framebuffer);
    gl.framebuffer_renderbuffer(
        gles::FRAMEBUFFER,
        gles::COLOR_ATTACHMENT0,
        gles::RENDERBUFFER,
        renderbuffer,
    );
    gl.clear_bufferfv(gles::COLOR, 0, &[0.0; 4]);
    gl.viewport(0, 0, N, N);
    gl.use_program(program);
    let _ = gl.clip_control_ext(gles::UPPER_LEFT_EXT, gles::ZERO_TO_ONE_EXT);
    gl.draw_arrays(gles::TRIANGLES, 0, 3);
    let mut texel = [0u8; 4];
    gl.read_pixels(
        0,
        0,
        1,
        1,
        gles::RGBA,
        gles::UNSIGNED_BYTE,
        PixelPack::Bytes(&mut texel),
    );
    gl.use_program(0);
    gl.delete_program(program);
    gl.bind_framebuffer(gles::FRAMEBUFFER, 0);
    gl.delete_framebuffers(&[framebuffer]);
    gl.bind_renderbuffer(gles::RENDERBUFFER, 0);
    gl.delete_renderbuffers(&[renderbuffer]);
    Ok(texel[0] == u8::MAX)
}

/// The fractional bias probe: a triangle at 0.45 of a step above step 1000 of a 1 by 1
/// `depth16unorm` texture, without and with a constant bias of 0.1 units. True when the bias
/// takes it to step 1001, as a fractional bias must; false when it stays (GL may round the units
/// to whole steps: ANGLE on Direct3D 11 does).
fn probe_fractional_bias<O: OpenGlEs>(gpu: &mut Gl<O>) -> Result<bool, String> {
    let words = probe_bias(gpu, Format::Depth16Unorm, 1000.45 / 65535.0, [0.0, 0.1])?;
    // The fetch gives the depth as a float's bits.
    match words.map(|w| (f64::from(f32::from_bits(w)) * 65535.0).round() as u32) {
        [1000, 1001] => Ok(true),
        [1000, 1000] => Ok(false),
        got => Err(format!(
            "depth16unorm steps {got:?} without and with the bias"
        )),
    }
}

/// Draws a triangle at window depth `z` into a cleared 1 by 1 texture of the depth `format` once
/// for each constant bias in `units`, and fetches the stored depth after each as a float's bits.
fn probe_bias<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    format: Format,
    z: f32,
    units: [f32; 2],
) -> Result<[u32; 2], String> {
    let error = |e: GpuError| format!("{e:?}");
    let (program, uniform) = depth::build_probe(&gpu.gl)?;
    let texture = |b: &mut Gl<O>, format, usage| {
        make_texture(
            b,
            &TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: 1,
                height: 1,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage,
                cube: false,
                reinterpret: false,
            },
        )
    };
    let depth = texture(gpu, format, TextureUsage::RENDER).map_err(error)?;
    let out = texture(gpu, Format::R32Uint, TextureUsage::RENDER).map_err(error)?;
    let fb = texture_framebuffer(gpu, &depth, 0, 0);
    let out_fb = texture_framebuffer(gpu, &out, 0, 0);
    let mut bits = [0u32; 2];
    for (units, bits) in units.into_iter().zip(&mut bits) {
        gpu.gl.use_program(program);
        let clip = if gpu.values.clip_depth == gles::ZERO_TO_ONE_EXT {
            z
        } else {
            2.0 * z - 1.0
        };
        gpu.gl.uniform1f(uniform, clip);
        gpu.gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, fb);
        gpu.applied.draw_framebuffer = Some(fb);
        open_writes(gpu);
        gpu.gl.clear_bufferfv(gles::DEPTH, 0, &[1.0]);
        state::plain(&gpu.gl, &mut gpu.values, [0, 0, 1, 1]);
        let (gl, v) = (&gpu.gl, &mut gpu.values);
        gl.enable(gles::DEPTH_TEST);
        v.depth_test = true;
        gl.depth_func(gles::ALWAYS);
        v.depth_func = gles::ALWAYS;
        gl.enable(gles::POLYGON_OFFSET_FILL);
        gl.polygon_offset(0.0, units);
        v.polygon_offset = Some((0.0, units, 0.0));
        own_draw(gpu);
        gpu.applied.program = 0;
        fetch(gpu, Fetch::Depth2D, &depth, 0, [0; 3], 0, out_fb, (1, 1)).map_err(error)?;
        let mut word = [0u8; 4];
        read_texture_now(
            gpu,
            &Region {
                texture: out.clone(),
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            },
            Extent {
                width: 1,
                height: 1,
                depth_or_layers: 1,
            },
            &mut word,
        )
        .map_err(error)?;
        *bits = u32::from_le_bytes(word);
    }
    destroy_held(gpu, Held::Texture(depth));
    destroy_held(gpu, Held::Texture(out));
    gpu.gl.delete_program(program);
    Ok(bits)
}

fn id<O: OpenGlEs>(gpu: &mut Gl<O>) -> Id {
    gpu.next_id += 1;
    gpu.next_id
}

/// Loses the device for an error GL has recorded since the last look: a bug of ours or a lost
/// context. Called where the back end has just waited for the GPU, so the look costs no wait.
fn gl_errors<O: OpenGlEs>(gpu: &mut Gl<O>) {
    let error = gpu.gl.get_error();
    let reason = match error {
        gles::NO_ERROR => return,
        gles::OUT_OF_MEMORY => LossReason::OutOfMemory,
        gles::CONTEXT_LOST => LossReason::Reset,
        _ => LossReason::Error,
    };
    if gpu.lost.is_none() {
        gpu.last_log = format!("GL error {error:#x}");
    }
    fail(gpu, reason);
}

/// Records an error on GL's side as device loss, which fronts report in their API's words.
fn fail<O: OpenGlEs>(gpu: &mut Gl<O>, reason: LossReason) {
    if gpu.lost.is_none() {
        // The interface carries only the reason's kind; the host's error output gets the why.
        platform::report(&format!(
            "wcontainers webgl: device lost ({reason:?}): {}",
            gpu.last_log
        ));
    }
    gpu.lost.get_or_insert(reason);
}

/// A read from a GL buffer that gave no bytes: the device is lost, so nothing reads stale bytes as
/// an answer.
fn read_failed<O: OpenGlEs>(gpu: &mut Gl<O>) -> GpuError {
    fail(gpu, LossReason::Error);
    GpuError::DeviceLost
}

/// A call whose route this back end does not have yet: the device is lost with the missing piece in
/// [`Gl::last_log`], so the front reports device loss instead of drawing wrong.
fn unbuilt<O: OpenGlEs>(gpu: &mut Gl<O>, what: &'static str) -> GpuError {
    if gpu.lost.is_none() {
        gpu.last_log = format!("webgl: not built yet: {what}");
    }
    fail(gpu, LossReason::Error);
    GpuError::DeviceLost
}

/// A recording call that broke the interface's rules (a bug in a front): the device is lost with
/// the rule in [`Gl::last_log`], and nothing of the call runs.
fn invalid<O: OpenGlEs>(gpu: &mut Gl<O>, rule: String) {
    if gpu.lost.is_none() {
        gpu.last_log = format!("webgl: invalid: {rule}");
    }
    fail(gpu, LossReason::Error);
}

/// The sRGB twin of `texture` in `format`, made at its first use: a second GL texture of the same
/// shape, which [`fresh`] keeps holding the same bytes.
/// The GL query object for query `index` of `set`, a new one when a read back still holds the
/// last. None (the device lost) past the set's end or when GL makes none.
fn query_slot<O: OpenGlEs>(gpu: &mut Gl<O>, set: &QuerySetRef, index: u32) -> Option<u32> {
    let mut slots = set.slots.borrow_mut();
    let Some(slot) = slots.get_mut(index as usize) else {
        drop(slots);
        invalid(gpu, format!("query {index} past the set's end"));
        return None;
    };
    if slot.as_ref().is_none_or(|q| Rc::strong_count(q) > 1) {
        match platform::one(|n| gpu.gl.gen_queries(n)) {
            Some(q) => *slot = Some(Rc::new(q)),
            None => {
                drop(slots);
                fail(gpu, LossReason::OutOfMemory);
                return None;
            }
        }
    }
    slot.as_deref().copied()
}

/// Why the device is lost where a query's result needs a turn the host does not give (no JSPI).
#[cfg(target_arch = "wasm32")]
const NO_TURN: &str =
    "webgl: a query's result waits for a turn of the browser, which this host does not give";

/// Whether every query's result is available.
fn queries_available<O: OpenGlEs>(gpu: &Gl<O>, queries: &[Option<Rc<u32>>]) -> bool {
    queries
        .iter()
        .flatten()
        .all(|q| platform::query_value(&gpu.gl, **q, gles::QUERY_RESULT_AVAILABLE) != 0)
}

/// A multisampled texture: a renderbuffer of the format and sample count.
fn create_samples<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    desc: &TextureDesc,
    gl_format: format::GlFormat,
) -> Result<TextureRef, GpuError> {
    if !gpu.caps.formats.takes_samples(desc.format, desc.samples)
        || desc.dimension != TextureDimension::D2
        || desc.mip_levels != 1
        || desc.depth_or_layers != 1
        || desc.usage.contains(TextureUsage::STORAGE)
    {
        return Err(GpuError::Invalid(
            "a multisampled texture of a count its format lacks, not 2D with one level and \
             one layer, or storage",
        ));
    }
    let gl = &gpu.gl;
    let rb = {
        let rb = platform::one(|n| gl.gen_renderbuffers(n)).ok_or(GpuError::OutOfMemory)?;
        gl.bind_renderbuffer(gles::RENDERBUFFER, rb);
        gl.renderbuffer_storage_multisample(
            gles::RENDERBUFFER,
            desc.samples as i32,
            gl_format.internal,
            desc.width as i32,
            desc.height as i32,
        );
        rb
    };
    Ok(Handle(Rc::new(TextureObj {
        id: id(gpu),
        storage: Storage::Samples(rb),
        target: gles::RENDERBUFFER,
        desc: *desc,
        gl: gl_format,
        read: None,
        levels: Cell::new((0, 0)),
        newest: Rc::new(Cell::new(0)),
        side: 1,
        twin: std::cell::OnceCell::new(),
        own: None,
        stencil_mode: Cell::new(false),
        host: None,
        decoded: None,
        drawn: Cell::new(0),
        minimum: Cell::new(0),
    })))
}

/// Averages the samples of multisampled `src` into level `mip`, layer `layer` of single-sampled
/// `dst` of the same format, by `blitFramebuffer`.
fn blit_resolve<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    src: &TextureRef,
    dst: &TextureRef,
    mip: u32,
    layer: u32,
) {
    let mask = match FORMAT_FACTS[src.desc.format as usize].kind {
        FormatKind::DepthStencil => gles::DEPTH_BUFFER_BIT | gles::STENCIL_BUFFER_BIT,
        FormatKind::Depth => gles::DEPTH_BUFFER_BIT,
        FormatKind::Stencil => gles::STENCIL_BUFFER_BIT,
        _ => gles::COLOR_BUFFER_BIT,
    };
    open_writes(gpu);
    read_framebuffer(gpu, src, 0, 0);
    let fb = texture_framebuffer(gpu, dst, mip, layer);
    let (w, h) = (src.desc.width as i32, src.desc.height as i32);
    gpu.gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, fb);
    gpu.gl
        .blit_framebuffer(0, 0, w, h, 0, 0, w, h, mask, gles::NEAREST);
    gpu.applied.draw_framebuffer = Some(fb);
}

/// A whole copy between multisampled textures of one format and sample count: GL ES 3.0 blits
/// into no multisampled framebuffer and programs cannot read samples here, so the source is
/// resolved and drawn into every sample of the destination
/// ([`SlowPath::MultisampleCopyResolved`], a listed loss).
fn copy_samples<O: OpenGlEs>(gpu: &mut Gl<O>, src: &Region, dst: &Region) {
    let (s, d) = (&src.texture, &dst.texture);
    if s.desc.samples != d.desc.samples
        || s.desc.width != d.desc.width
        || s.desc.height != d.desc.height
        || FORMAT_FACTS[s.desc.format as usize].block != FORMAT_FACTS[d.desc.format as usize].block
    {
        return invalid(
            gpu,
            "a copy of a multisampled texture other than whole into one of its sample count".into(),
        );
    }
    match FORMAT_FACTS[s.desc.format as usize].kind {
        FormatKind::Uint | FormatKind::Sint => {
            return invalid(
                gpu,
                "a copy of integer samples, which cannot be averaged and need the absent \
                 MULTISAMPLE_READ"
                    .into(),
            );
        }
        FormatKind::Depth | FormatKind::DepthStencil | FormatKind::Stencil => {
            unbuilt(gpu, "copies of multisampled depth and stencil");
            return;
        }
        _ => {}
    }
    gpu.record_slow_path(SlowPath::MultisampleCopyResolved);
    let resolved = match make_texture(
        gpu,
        &TextureDesc {
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::SAMPLE,
            ..s.desc
        },
    ) {
        Ok(t) => t,
        Err(_) => return fail(gpu, LossReason::OutOfMemory),
    };
    blit_resolve(gpu, s, &resolved, 0, 0);
    let whole = Extent {
        width: s.desc.width,
        height: s.desc.height,
        depth_or_layers: 1,
    };
    let from = Region {
        texture: resolved.clone(),
        ..*src
    };
    if !copy_by_draw(gpu, &from, dst, whole) {
        unbuilt(gpu, "copies of multisampled textures of this format");
    }
    destroy_held(gpu, Held::Texture(resolved));
}

fn twin_of<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    texture: &TextureRef,
    format: Format,
) -> Result<TextureRef, GpuError> {
    if let Some(t) = texture.twin.get() {
        return Ok(t.clone());
    }
    let readable = |f: Format| {
        host::kept(f)
            || format::read_as(f, format::FormatExtensions::found(&gpu.extensions))
                .is_some_and(|r| r.texel_read() == r.texel_kept())
    };
    if !readable(texture.desc.format) || !readable(format) {
        return Err(unbuilt(
            gpu,
            "sRGB twins of formats read back wider than stored",
        ));
    }
    let mut twin = make_texture(
        gpu,
        &TextureDesc {
            format,
            ..texture.desc
        },
    )?;
    let t = Rc::get_mut(&mut twin.0).expect("a texture just made has one handle");
    t.newest = texture.newest.clone();
    t.side = 2;
    t.own = Some(Rc::downgrade(&texture.0));
    if texture.newest.get() == 0 {
        texture.newest.set(1);
    }
    gpu.twins += 1;
    let _ = texture.twin.set(twin.clone());
    Ok(twin)
}

/// The stencil copy of `texture`, made at its first stencil view without WEBGL_stencil_texturing:
/// an `r8uint` texture of the same shape, which [`fresh`] fills from the stencil by draws
/// whenever the stencil changed since.
fn stencil_copy_of<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    texture: &TextureRef,
) -> Result<TextureRef, GpuError> {
    if let Some(t) = texture.twin.get() {
        return Ok(t.clone());
    }
    let mut copy = make_texture(
        gpu,
        &TextureDesc {
            format: Format::R8Uint,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::SAMPLE,
            reinterpret: false,
            ..texture.desc
        },
    )?;
    let t = Rc::get_mut(&mut copy.0).expect("a texture just made has one handle");
    t.newest = texture.newest.clone();
    t.side = 2;
    t.own = Some(Rc::downgrade(&texture.0));
    texture.newest.set(1);
    gpu.twins += 1;
    let _ = texture.twin.set(copy.clone());
    Ok(copy)
}

/// Fills every level and layer of `copy` with the stencil of `src`, counted by draws.
fn fill_stencil_copy<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    src: &TextureRef,
    copy: &TextureRef,
) -> Result<(), GpuError> {
    let d = src.desc;
    for mip in 0..d.mip_levels {
        let size = ((d.width >> mip).max(1), (d.height >> mip).max(1));
        for layer in 0..d.depth_or_layers {
            let counted = stencil_bits(gpu, src, mip, layer)?;
            let fb = texture_framebuffer(gpu, copy, mip, layer);
            let size = (size.0 as i32, size.1 as i32);
            fetch(gpu, Fetch::Depth2D, &counted, 0, [0; 3], 255, fb, size)?;
            destroy_held(gpu, Held::Texture(counted));
        }
    }
    Ok(())
}

/// Before `t` is read or written: copies its twin's texels into it when the twin holds newer ones.
fn fresh<O: OpenGlEs>(gpu: &mut Gl<O>, t: &TextureRef) {
    let newest = t.newest.get();
    if newest == 0 || newest == t.side {
        return;
    }
    let other = match t.side {
        1 => t.twin.get().cloned(),
        _ => t.own.as_ref().and_then(|own| own.upgrade()).map(Handle),
    };
    if let Some(other) = other {
        if FORMAT_FACTS[other.desc.format as usize].depth_stencil {
            // A failure has lost the device.
            let _ = fill_stencil_copy(gpu, &other, t);
        } else {
            copy_whole(gpu, &other, t);
        }
    }
    t.newest.set(0);
}

/// After `t` was written: its twin no longer holds the same texels, and the next draw binds again.
fn written<O: OpenGlEs>(gpu: &mut Gl<O>, t: &TextureRef) {
    if t.side == 2 || t.twin.get().is_some() {
        t.newest.set(t.side);
        gpu.applied.sets = [Id::MAX; MAX_BINDING_SETS];
    }
}

/// Copies every level and layer of `from` into `to`, a texture of the same shape whose texels have
/// the same bytes: on the GPU, or for textures kept on the host from host copy to host copy, then
/// uploaded.
fn copy_whole<O: OpenGlEs>(gpu: &mut Gl<O>, from: &TextureRef, to: &TextureRef) {
    let d = from.desc;
    let kept = from.host.as_ref().zip(to.host.as_ref());
    if let Some((a, b)) = kept {
        gpu.record_slow_path(SlowPath::HostCopyKept);
        b.copy_from(a);
    }
    for mip in 0..d.mip_levels {
        let region = |texture| Region {
            texture,
            mip,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        let extent = Extent {
            width: (d.width >> mip).max(1),
            height: (d.height >> mip).max(1),
            depth_or_layers: if d.dimension == TextureDimension::D3 {
                (d.depth_or_layers >> mip).max(1)
            } else {
                d.depth_or_layers
            },
        };
        if kept.is_some() {
            upload_blocks(gpu, &region(to.clone()), extent);
        } else {
            copy_same_bytes(gpu, &region(from.clone()), &region(to.clone()), extent);
        }
    }
}

/// Copies a region between textures whose texels have the same bytes, the source's read by
/// `readPixels` as stored: by a draw where one carries both formats and they are not floats,
/// which stays on the GPU, else through a pixel buffer, whose unpack ANGLE waits for on the CPU
/// ([`SlowPath::CopyThroughBuffer`] either way).
fn copy_same_bytes<O: OpenGlEs>(gpu: &mut Gl<O>, src: &Region, dst: &Region, extent: Extent) {
    let float = [src, dst]
        .iter()
        .any(|r| format::float_bits(r.texture.desc.format));
    if !float && copy_by_draw(gpu, src, dst, extent) {
        gpu.record_slow_path(SlowPath::CopyThroughBuffer);
    } else {
        copy_through_pbo(gpu, src, dst, extent);
    }
}

/// Copies a region between textures whose texels have the same bytes, the source's read by
/// `readPixels` as stored: read into a pixel pack buffer, then written from it as the destination's
/// texels, all on the GPU ([`SlowPath::CopyThroughBuffer`]).
fn copy_through_pbo<O: OpenGlEs>(gpu: &mut Gl<O>, src: &Region, dst: &Region, extent: Extent) {
    gpu.record_slow_path(SlowPath::CopyThroughBuffer);
    let read = src
        .texture
        .read
        .expect("the caller checked the source is read as stored");
    let texel = read.texel_kept() as u32;
    let layer_bytes = extent.width * extent.height * texel;
    let pbo = {
        let Some(pbo) = platform::one(|n| gpu.gl.gen_buffers(n)) else {
            fail(gpu, LossReason::OutOfMemory);
            return;
        };
        gpu.gl.bind_buffer(gles::PIXEL_PACK_BUFFER, pbo);
        gpu.gl.buffer_data(
            gles::PIXEL_PACK_BUFFER,
            i64::from(layer_bytes * extent.depth_or_layers),
            None,
            gles::STREAM_COPY,
        );
        pbo
    };
    for i in 0..extent.depth_or_layers {
        let layer = read_layer(src, i);
        read_framebuffer(gpu, &src.texture, src.mip, layer);
        gpu.gl.read_pixels(
            src.origin.x as i32,
            src.origin.y as i32,
            extent.width as i32,
            extent.height as i32,
            read.format,
            read.ty,
            PixelPack::Offset(u64::from(i * layer_bytes)),
        );
    }
    gpu.gl.bind_buffer(gles::PIXEL_PACK_BUFFER, 0);
    gpu.gl.bind_buffer(gles::PIXEL_UNPACK_BUFFER, pbo);
    upload(
        gpu,
        dst,
        extent,
        Texels::Offset(0),
        extent.width * texel,
        extent.height,
    );
    gpu.gl.bind_buffer(gles::PIXEL_UNPACK_BUFFER, 0);
    gpu.gl.delete_buffers(&[pbo]);
}

/// Copies a region between colour formats of one typeless group by a draw per layer that fetches
/// each source texel and writes its bits as the destination's value ([`SlowPath::CopyThroughBuffer`],
/// convert.rs). False when no such draw carries the two formats or the destination cannot be
/// drawn into.
fn copy_by_draw<O: OpenGlEs>(gpu: &mut Gl<O>, src: &Region, dst: &Region, extent: Extent) -> bool {
    let (from, to) = (src.texture.desc.format, dst.texture.desc.format);
    let sampler = match src.texture.target {
        gles::TEXTURE_2D => "2D",
        gles::TEXTURE_2D_ARRAY => "2DArray",
        gles::TEXTURE_3D => "3D",
        _ => "Cube",
    };
    let drawn = if FORMAT_FACTS[to as usize].depth_stencil {
        FormatUse::DEPTH_STENCIL
    } else {
        FormatUse::RENDER
    };
    if !gpu.caps.formats.uses[to as usize].contains(drawn) {
        return false;
    }
    let key = (from, to, src.texture.target);
    if !gpu.copy_programs.contains_key(&key) {
        let Some(source) = convert::program(from, to, sampler) else {
            return false;
        };
        let built = depth::link(&gpu.gl, &source).and_then(|p| {
            let (at, unit) = (
                platform::location(&gpu.gl, p, "at"),
                platform::location(&gpu.gl, p, "unit"),
            );
            at.map(|at| (p, at, unit))
                .ok_or_else(|| "the copy program has no uniform at".to_string())
        });
        match built {
            Ok(p) => {
                gpu.copy_programs.insert(key, p);
            }
            Err(log) => {
                own_program_failed(gpu, log);
                return true;
            }
        }
    }
    if bind_source(gpu, &src.texture, src.mip).is_err() {
        fail(gpu, LossReason::OutOfMemory);
        return true;
    }
    if FORMAT_FACTS[from as usize].depth_stencil {
        aspect(gpu, &src.texture, false);
    }
    let rect = [
        dst.origin.x as i32,
        dst.origin.y as i32,
        extent.width as i32,
        extent.height as i32,
    ];
    for i in 0..extent.depth_or_layers {
        let fb = texture_framebuffer(gpu, &dst.texture, dst.mip, read_layer(dst, i));
        let (program, at, unit) = &gpu.copy_programs[&key];
        gpu.gl.use_program(*program);
        gpu.gl.uniform3i(
            *at,
            src.origin.x as i32 - rect[0],
            src.origin.y as i32 - rect[1],
            read_layer(src, i) as i32,
        );
        if let Some(u) = convert::depth_unit(to) {
            gpu.gl.uniform2f(unit.unwrap_or(-1), u[0], u[1]);
        }
        if FORMAT_FACTS[to as usize].depth_stencil {
            own_depth_draw_into(gpu, fb, rect);
        } else {
            own_draw_into(gpu, fb, rect);
        }
    }
    true
}

/// Whether a copy reads and writes one subresource (layers of a level that both regions cover,
/// or one level of a 3D texture, whose slices GL copies into as one image), which every GL
/// route refuses as a feedback loop.
fn within_one_subresource(src: &Region, dst: &Region, extent: Extent) -> bool {
    let (s, d, n) = (src.layer, dst.layer, extent.depth_or_layers);
    Rc::ptr_eq(&src.texture.0, &dst.texture.0)
        && src.mip == dst.mip
        && (src.texture.desc.dimension == TextureDimension::D3 || (s < d + n && d < s + n))
}

/// A copy within one subresource, through a buffer holding the source region
/// ([`SlowPath::CopyWithinTexture`]): a copy into the buffer, then one out of it, aspect by
/// aspect for a format with depth and stencil. Formats GL does not read back (16-bit floats)
/// are held in a 2D texture instead.
fn copy_within<O: OpenGlEs>(gpu: &mut Gl<O>, src: &Region, dst: &Region, extent: Extent) {
    gpu.record_slow_path(SlowPath::CopyWithinTexture);
    let format = src.texture.desc.format;
    if !gpu.caps.formats.uses[format as usize].contains(FormatUse::COPY_OUT) {
        let d = &src.texture.desc;
        let Ok(held) = make_texture(
            gpu,
            &TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width: extent.width,
                height: extent.height,
                depth_or_layers: extent.depth_or_layers,
                mip_levels: 1,
                samples: 1,
                usage: (d.usage & (TextureUsage::SAMPLE | TextureUsage::RENDER))
                    | TextureUsage::COPY_SRC
                    | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            },
        ) else {
            return fail(gpu, LossReason::OutOfMemory);
        };
        let at = Region {
            texture: held.clone(),
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        copy_texture_held(gpu, src, &at, extent);
        copy_texture_held(gpu, &at, dst, extent);
        return destroy_held(gpu, Held::Texture(held));
    }
    let (bw, bh, bytes) = FORMAT_FACTS[format as usize].block;
    // A buffer's depth of a combined format is a 32-bit word, its stencil a byte.
    let aspects: &[(bool, u32)] = match FORMAT_FACTS[format as usize].kind {
        FormatKind::DepthStencil => &[(false, 4), (true, 1)],
        _ => &[(false, bytes)],
    };
    let (cols, rows) = (extent.width.div_ceil(bw), extent.height.div_ceil(bh));
    for &(stencil, bytes) in aspects {
        let size = u64::from(cols * bytes * rows * extent.depth_or_layers);
        let Ok(held) = make_buffer(
            gpu,
            &BufferDesc {
                size,
                usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            },
        ) else {
            return fail(gpu, LossReason::OutOfMemory);
        };
        let at = Place {
            buffer: held.clone(),
            offset: 0,
            row_pitch: cols * bytes,
            rows_per_layer: rows,
        };
        copy_texture_to_buffer_held(
            gpu,
            &Region {
                stencil,
                ..src.clone()
            },
            &at,
            extent,
        );
        copy_buffer_to_texture_held(
            gpu,
            &at,
            &Region {
                stencil,
                ..dst.clone()
            },
            extent,
        );
        destroy_held(gpu, Held::Buffer(held));
    }
}

/// Copies a region of a texture into one of the same format by `blitFramebuffer`, layer by layer:
/// a depth or depth-stencil one, or a float one where blits keep every bit.
fn blit_same<O: OpenGlEs>(gpu: &mut Gl<O>, src: &Region, dst: &Region, extent: Extent) {
    let mask = match FORMAT_FACTS[src.texture.desc.format as usize].kind {
        FormatKind::DepthStencil => gles::DEPTH_BUFFER_BIT | gles::STENCIL_BUFFER_BIT,
        FormatKind::Depth => gles::DEPTH_BUFFER_BIT,
        FormatKind::Stencil => gles::STENCIL_BUFFER_BIT,
        _ => gles::COLOR_BUFFER_BIT,
    };
    open_writes(gpu);
    let (sx, sy, dx, dy, w, h) = (
        src.origin.x as i32,
        src.origin.y as i32,
        dst.origin.x as i32,
        dst.origin.y as i32,
        extent.width as i32,
        extent.height as i32,
    );
    for i in 0..extent.depth_or_layers {
        read_framebuffer(gpu, &src.texture, src.mip, read_layer(src, i));
        let fb = texture_framebuffer(gpu, &dst.texture, dst.mip, read_layer(dst, i));
        gpu.gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, fb);
        gpu.gl.blit_framebuffer(
            sx,
            sy,
            sx + w,
            sy + h,
            dx,
            dy,
            dx + w,
            dy + h,
            mask,
            gles::NEAREST,
        );
        gpu.applied.draw_framebuffer = Some(fb);
    }
}

/// Opens the write masks and the scissor for a clear or a copy, which obey them, and makes the next
/// draw apply its blocks again.
fn open_writes<O: OpenGlEs>(gpu: &mut Gl<O>) {
    let (gl, v) = (&gpu.gl, &mut gpu.values);
    if v.scissor_test {
        gl.disable(gles::SCISSOR_TEST);
        v.scissor_test = false;
    }
    if !v.depth_mask {
        gl.depth_mask(true);
        v.depth_mask = true;
    }
    if v.stencil_write_mask != 0xff {
        gl.stencil_mask(0xff);
        v.stencil_write_mask = 0xff;
    }
    state::open_masks(gl, v);
    gpu.applied.blend = 0;
    gpu.applied.depth_stencil = 0;
    gpu.applied.raster = 0;
}

fn framebuffer<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    key: FramebufferKey,
    views: &[Option<&ViewRef>],
) -> u32 {
    if let Some(fb) = gpu.framebuffers.get(&key) {
        return *fb;
    }
    let gl = &gpu.gl;
    let fb = {
        let fb = platform::one(|n| gl.gen_framebuffers(n)).expect("a framebuffer object");
        gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, fb);
        let mut draw_buffers = [gles::NONE; MAX_COLOR_TARGETS];
        for (i, view) in views.iter().enumerate() {
            let (Some(view), Some((_, mip, layer))) = (view, key[i]) else {
                continue;
            };
            let t = &view.texture;
            let point = if i < MAX_COLOR_TARGETS {
                draw_buffers[i] = gles::COLOR_ATTACHMENT0 + i as u32;
                gles::COLOR_ATTACHMENT0 + i as u32
            } else {
                match FORMAT_FACTS[t.desc.format as usize].kind {
                    FormatKind::DepthStencil => gles::DEPTH_STENCIL_ATTACHMENT,
                    FormatKind::Stencil => gles::STENCIL_ATTACHMENT,
                    _ => gles::DEPTH_ATTACHMENT,
                }
            };
            match (t.storage, t.target) {
                (Storage::Samples(rb), _) => gl.framebuffer_renderbuffer(
                    gles::DRAW_FRAMEBUFFER,
                    point,
                    gles::RENDERBUFFER,
                    rb,
                ),
                (Storage::Texture(raw), gles::TEXTURE_2D) => gl.framebuffer_texture_2d(
                    gles::DRAW_FRAMEBUFFER,
                    point,
                    gles::TEXTURE_2D,
                    raw,
                    mip as i32,
                ),
                (Storage::Texture(raw), gles::TEXTURE_CUBE_MAP) => gl.framebuffer_texture_2d(
                    gles::DRAW_FRAMEBUFFER,
                    point,
                    gles::TEXTURE_CUBE_MAP_POSITIVE_X + layer,
                    raw,
                    mip as i32,
                ),
                (Storage::Texture(raw), _) => gl.framebuffer_texture_layer(
                    gles::DRAW_FRAMEBUFFER,
                    point,
                    raw,
                    mip as i32,
                    layer as i32,
                ),
            }
        }
        let last = draw_buffers
            .iter()
            .rposition(|b| *b != gles::NONE)
            .map_or(0, |i| i + 1);
        gl.draw_buffers(&draw_buffers[..last.max(1)]);
        if gl.check_framebuffer_status(gles::DRAW_FRAMEBUFFER) != gles::FRAMEBUFFER_COMPLETE {
            gpu.lost.get_or_insert(LossReason::Error);
        }
        fb
    };
    gpu.applied.draw_framebuffer = Some(fb);
    gpu.framebuffers.insert(key, fb);
    fb
}

/// A pass without attachments: its own renderbuffer of the pass's size and samples, with no draw
/// buffer, so draws rasterise (for queries and depth-less tests) and write nothing.
fn begin_empty_pass<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    empty: EmptyPass,
    occlusion: Option<QuerySetRef>,
) {
    let gl = &gpu.gl;
    let fb = match gpu.empty_passes.get(&empty) {
        Some(fb) => *fb,
        None => {
            let rb = platform::one(|n| gl.gen_renderbuffers(n)).expect("a renderbuffer object");
            gl.bind_renderbuffer(gles::RENDERBUFFER, rb);
            let (w, h) = (empty.width as i32, empty.height as i32);
            if empty.samples > 1 {
                gl.renderbuffer_storage_multisample(
                    gles::RENDERBUFFER,
                    empty.samples as i32,
                    gles::R8,
                    w,
                    h,
                );
            } else {
                gl.renderbuffer_storage(gles::RENDERBUFFER, gles::R8, w, h);
            }
            let fb = platform::one(|n| gl.gen_framebuffers(n)).expect("a framebuffer object");
            gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, fb);
            gl.framebuffer_renderbuffer(
                gles::DRAW_FRAMEBUFFER,
                gles::COLOR_ATTACHMENT0,
                gles::RENDERBUFFER,
                rb,
            );
            gl.draw_buffers(&[gles::NONE]);
            if gl.check_framebuffer_status(gles::DRAW_FRAMEBUFFER) != gles::FRAMEBUFFER_COMPLETE {
                gpu.lost.get_or_insert(LossReason::Error);
            }
            gpu.applied.draw_framebuffer = Some(fb);
            gpu.empty_passes.insert(empty, fb);
            fb
        }
    };
    let mut discard = Points::default();
    discard.push(gles::COLOR_ATTACHMENT0);
    if gpu.applied.draw_framebuffer != Some(fb) {
        gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, fb);
        gpu.applied.draw_framebuffer = Some(fb);
    }
    gl.invalidate_framebuffer(gles::DRAW_FRAMEBUFFER, discard.get());
    gpu.pass = Some(Pass {
        framebuffer: fb,
        discard,
        occlusion,
        counting: false,
        resolves: Default::default(),
        size: (empty.width, empty.height),
        color_targets: 0,
    });
}

/// The attachment a view to draw into names: its texture, mip, and layer or slice.
fn attachment(view: &ViewRef, slice: u32) -> Attachment {
    let layer = if view.texture.desc.dimension == TextureDimension::D3 {
        slice
    } else {
        view.desc.base_layer
    };
    (view.texture.id, view.desc.base_mip, layer)
}

/// Binds a framebuffer holding one level and layer of `texture` for reading.
fn read_framebuffer<O: OpenGlEs>(gpu: &mut Gl<O>, texture: &TextureRef, mip: u32, layer: u32) {
    let fb = texture_framebuffer(gpu, texture, mip, layer);
    if gpu.applied.read_framebuffer != Some(fb) {
        gpu.gl.bind_framebuffer(gles::READ_FRAMEBUFFER, fb);
        gpu.applied.read_framebuffer = Some(fb);
    }
}

/// The framebuffer holding one level and layer of `texture` alone, made if new; the draw binding
/// stays as it was.
fn texture_framebuffer<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    texture: &TextureRef,
    mip: u32,
    layer: u32,
) -> u32 {
    own_framebuffer(gpu, &[(texture, mip, layer)])
}

/// The framebuffer of the back end's own holding `parts` (each a texture, level and layer; colour
/// ones on targets 0, 1 and on, a depth or stencil one on its point), made if new; the draw binding
/// stays as it was.
fn own_framebuffer<O: OpenGlEs>(gpu: &mut Gl<O>, parts: &[(&TextureRef, u32, u32)]) -> u32 {
    let mut key: FramebufferKey = [None; MAX_COLOR_TARGETS + 1];
    let mut views: [Option<ViewRef>; MAX_COLOR_TARGETS + 1] = Default::default();
    let mut colour = 0;
    for &(texture, mip, layer) in parts {
        let slot = if FORMAT_FACTS[texture.desc.format as usize].depth_stencil {
            MAX_COLOR_TARGETS
        } else {
            colour += 1;
            colour - 1
        };
        key[slot] = Some((texture.id, mip, layer));
        views[slot] = Some(Handle(Rc::new(ViewObj {
            id: 0,
            texture: texture.clone(),
            desc: TextureViewDesc {
                format: texture.desc.format,
                dimension: ViewDimension::D2,
                base_mip: mip,
                mip_count: 1,
                base_layer: layer,
                layer_count: 1,
                stencil: false,
            },
        })));
    }
    let views = views.each_ref().map(Option::as_ref);
    let draw = gpu.applied.draw_framebuffer;
    let fb = framebuffer(gpu, key, &views);
    if gpu.applied.draw_framebuffer != draw {
        gpu.gl
            .bind_framebuffer(gles::DRAW_FRAMEBUFFER, draw.unwrap_or(0));
        gpu.applied.draw_framebuffer = draw;
    }
    fb
}

/// Draws the depth (or with `region.stencil` the stencil) of a region into a new `r32uint` texture
/// of the region's size and layers, which reads back as the stored bytes
/// ([`SlowPath::DepthReadByDraw`]).
fn depth_by_draw<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    region: &Region,
    extent: Extent,
) -> Result<TextureRef, GpuError> {
    fresh(gpu, &region.texture);
    let src = region.texture.clone();
    let Some((scale, kept)) = depth::stored(src.desc.format) else {
        return Err(GpuError::Invalid("webgl: not a depth format"));
    };
    let (kept, scale) = if region.stencil {
        (1, 0)
    } else {
        (kept, scale)
    };
    let kind = match src.target {
        gles::TEXTURE_2D => Fetch::Depth2D,
        gles::TEXTURE_2D_ARRAY => Fetch::DepthArray,
        _ => Fetch::DepthCube,
    };
    // The stencil of a cube sampled through WEBGL_stencil_texturing reads 0 in Chrome 157 on the
    // RTX 3080, so cubes count their stencil bits as contexts without the extension do.
    let bits = region.stencil
        && (!gpu.extensions.contains("WEBGL_stencil_texturing")
            || matches!(kind, Fetch::DepthCube));
    let kind = match kind {
        _ if bits => Fetch::Depth2D,
        Fetch::Depth2D if region.stencil => Fetch::Stencil2D,
        Fetch::DepthArray if region.stencil => Fetch::StencilArray,
        k => k,
    };
    gpu.record_slow_path(SlowPath::DepthReadByDraw);
    let mut scratch = make_texture(
        gpu,
        &TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::R32Uint,
            width: extent.width,
            height: extent.height,
            depth_or_layers: extent.depth_or_layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        },
    )?;
    Rc::get_mut(&mut scratch.0)
        .expect("a texture just made has one handle")
        .read = Some(format::ReadAs {
        format: gles::RGBA_INTEGER,
        ty: gles::UNSIGNED_INT,
        comp_read: 4,
        comp_kept: kept,
        n: 1,
        first: 0,
        swap: false,
        fields: None,
    });
    let at = [region.origin.x as i32, region.origin.y as i32];
    for i in 0..extent.depth_or_layers {
        let layer = region.layer + i;
        let fb = texture_framebuffer(gpu, &scratch, 0, i);
        let size = (extent.width as i32, extent.height as i32);
        if bits {
            let counted = stencil_bits(gpu, &src, region.mip, layer)?;
            fetch(gpu, kind, &counted, 0, [at[0], at[1], 0], 255, fb, size)?;
            destroy_held(gpu, Held::Texture(counted));
        } else {
            fetch(
                gpu,
                kind,
                &src,
                region.mip,
                [at[0], at[1], layer as i32],
                scale,
                fb,
                size,
            )?;
            if scale == UNORM24_SCALE && !gpu.depth24_colliding.is_empty() {
                let at = [region.origin.x, region.origin.y];
                decide_colliding(gpu, (&src, region.mip, layer, at), (&scratch, i), size)?;
            }
        }
    }
    Ok(scratch)
}

/// What a depth of 1 is stored as in 24-bit depth ([`depth::stored`]).
const UNORM24_SCALE: u32 = (1 << 24) - 1;

/// After a read of 24-bit depth by sampling into layer `dst.1` of `dst` (`size` texels), the
/// comparison passes ([`depth::EQUAL`]): for each word the device samples as another's float, a
/// draw with the depth test EQUAL at that word writes it where the source (level, layer and
/// region origin `src`) stores it, the source as the depth attachment. A region off the origin is
/// decided in a colour texture reaching the region's place, the read copied there and back by
/// blits of integer colour, which keep every bit (a blit of the depth itself may sample it).
fn decide_colliding<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    src: (&TextureRef, u32, u32, [u32; 2]),
    dst: (&TextureRef, u32),
    size: (i32, i32),
) -> Result<(), GpuError> {
    let (texture, mip, layer, at) = src;
    let extent = Extent {
        width: size.0 as u32,
        height: size.1 as u32,
        depth_or_layers: 1,
    };
    let region = |texture: &TextureRef, layer, origin| Region {
        texture: texture.clone(),
        mip: 0,
        layer,
        origin,
        stencil: false,
    };
    let placed_at = Origin {
        x: at[0],
        y: at[1],
        z: 0,
    };
    let placed = if at == [0, 0] {
        None
    } else {
        let placed = make_texture(
            gpu,
            &TextureDesc {
                dimension: TextureDimension::D2,
                format: dst.0.desc.format,
                width: at[0] + extent.width,
                height: at[1] + extent.height,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
                cube: false,
                reinterpret: false,
            },
        )?;
        blit_same(
            gpu,
            &region(dst.0, dst.1, Origin::default()),
            &region(&placed, 0, placed_at),
            extent,
        );
        Some(placed)
    };
    let (target, target_layer, rect) = match &placed {
        Some(placed) => (placed, 0, [at[0] as i32, at[1] as i32, size.0, size.1]),
        None => (dst.0, dst.1, [0, 0, size.0, size.1]),
    };
    if gpu.equal_pass.is_none() {
        match depth::build_equal(&gpu.gl) {
            Ok(p) => gpu.equal_pass = Some(p),
            Err(log) => return Err(own_program_failed(gpu, log)),
        }
    }
    let fb = own_framebuffer(gpu, &[(target, 0, target_layer), (texture, mip, layer)]);
    let words = gpu.depth24_colliding.clone();
    for word in words {
        let pass = gpu.equal_pass.as_ref().expect("built above");
        let (program, word_at, depth_at) = (pass.program, pass.word, pass.depth);
        gpu.gl.use_program(program);
        gpu.gl.uniform1ui(word_at, word);
        gpu.gl.uniform1f(depth_at, depth::nearest(word));
        own_equal_draw_into(gpu, fb, rect);
    }
    if let Some(placed) = placed {
        blit_same(
            gpu,
            &region(&placed, 0, placed_at),
            &region(dst.0, dst.1, Origin::default()),
            extent,
        );
        destroy_held(gpu, Held::Texture(placed));
    }
    Ok(())
}

/// Draws a full-screen triangle into `target` (`size` pixels) whose fragments fetch level `mip` of
/// `src` at their position moved by `at` (x, y, layer) and write the fetched value as an unsigned
/// integer: a depth times `scale` (0 keeps a float's bits), or a stencil value. Drawn with GL's own
/// lower-left origin, so rows match on both window-origin routes.
#[allow(clippy::too_many_arguments)]
fn fetch<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    kind: Fetch,
    src: &TextureRef,
    mip: u32,
    at: [i32; 3],
    scale: u32,
    target: u32,
    size: (i32, i32),
) -> Result<(), GpuError> {
    let k = kind as usize;
    if gpu.depth_read[k].is_none() {
        match depth::build(&gpu.gl, kind.shape(), kind.stencil()) {
            Ok(p) => gpu.depth_read[k] = Some(p),
            Err(log) => return Err(own_program_failed(gpu, log)),
        }
    }
    bind_source(gpu, src, mip)?;
    let stencil = kind.stencil();
    aspect(gpu, src, stencil);
    let program = gpu.depth_read[k].as_ref().expect("built above");
    gpu.gl.use_program(program.program);
    if let Some(location) = &program.scale {
        gpu.gl.uniform1ui(*location, scale);
    }
    if let Some(location) = &program.reciprocal {
        gpu.gl.uniform1f(*location, gpu.depth24_reciprocal);
    }
    gpu.gl.uniform3i(program.at, at[0], at[1], at[2]);
    own_draw_into(gpu, target, [0, 0, size.0, size.1]);
    Ok(())
}

/// Binds level `mip` of `src` alone on unit 0 for the back end's own fetches, through a nearest
/// sampler (GLES leaves a depth texture without comparison incomplete under linear filters).
fn bind_source<O: OpenGlEs>(gpu: &mut Gl<O>, src: &TextureRef, mip: u32) -> Result<(), GpuError> {
    let Some(raw) = src.tex() else {
        return Err(unbuilt(gpu, "fetches of multisampled textures"));
    };
    bind_scratch(gpu, src.target, raw);
    let sampler = point_sampler(gpu)?;
    let gl = &gpu.gl;
    if src.levels.get() != (mip, mip) {
        gl.tex_parameteri(src.target, gles::TEXTURE_BASE_LEVEL, mip as i32);
        gl.tex_parameteri(src.target, gles::TEXTURE_MAX_LEVEL, mip as i32);
        src.levels.set((mip, mip));
    }
    gl.bind_sampler(0, sampler);
    // The next draw binds its own sampler on unit 0 again.
    gpu.applied.units[0].1 = Id::MAX;
    Ok(())
}

/// Draws the back end's full-screen triangle with the program in use into `target`, in the
/// viewport `rect` (GL's window coordinates, row 0 the texture's first), with no blending, tests or
/// culling; the next draw applies its own state.
fn own_draw_into<O: OpenGlEs>(gpu: &mut Gl<O>, target: u32, rect: [i32; 4]) {
    gpu.gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, target);
    gpu.applied.draw_framebuffer = Some(target);
    gpu.applied.program = 0;
    open_writes(gpu);
    state::plain(&gpu.gl, &mut gpu.values, rect);
    own_draw(gpu);
}

/// As [`own_draw_into`], for a program writing depth into the depth attachment of
/// `target`: the depth test passes always and writes, in the full depth range; stencil stays.
fn own_depth_draw_into<O: OpenGlEs>(gpu: &mut Gl<O>, target: u32, rect: [i32; 4]) {
    gpu.gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, target);
    gpu.applied.draw_framebuffer = Some(target);
    gpu.applied.program = 0;
    open_writes(gpu);
    state::plain(&gpu.gl, &mut gpu.values, rect);
    let (gl, v) = (&gpu.gl, &mut gpu.values);
    gl.enable(gles::DEPTH_TEST);
    v.depth_test = true;
    if v.depth_func != gles::ALWAYS {
        gl.depth_func(gles::ALWAYS);
        v.depth_func = gles::ALWAYS;
    }
    if v.depth_range != [0.0, 1.0] {
        gl.depth_rangef(0.0, 1.0);
        v.depth_range = [0.0, 1.0];
    }
    own_draw(gpu);
}

/// As [`own_draw_into`], for the comparison pass ([`depth::EQUAL`]): the depth test EQUAL against
/// the depth attachment of `target`, which it does not write, in the full depth range.
fn own_equal_draw_into<O: OpenGlEs>(gpu: &mut Gl<O>, target: u32, rect: [i32; 4]) {
    gpu.gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, target);
    gpu.applied.draw_framebuffer = Some(target);
    gpu.applied.program = 0;
    open_writes(gpu);
    state::plain(&gpu.gl, &mut gpu.values, rect);
    let (gl, v) = (&gpu.gl, &mut gpu.values);
    gl.enable(gles::DEPTH_TEST);
    v.depth_test = true;
    if v.depth_func != gles::EQUAL {
        gl.depth_func(gles::EQUAL);
        v.depth_func = gles::EQUAL;
    }
    gl.depth_mask(false);
    v.depth_mask = false;
    if v.depth_range != [0.0, 1.0] {
        gl.depth_rangef(0.0, 1.0);
        v.depth_range = [0.0, 1.0];
    }
    own_draw(gpu);
}

/// Writes depth or stencil texels, from the host or from the bound pixel unpack buffer, by draws:
/// into an integer texture of the aspect's bytes first, from which a draw writes the depth
/// ([`SlowPath::DepthWrittenByDraw`]) or draws write the stencil bit by bit. GL takes neither
/// aspect of a combined format alone, nor 24-bit depth as Direct3D stores it, from bytes.
fn write_depth<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    dst: &Region,
    extent: Extent,
    texels: Texels<'_>,
    row_pitch: u32,
    rows_per_layer: u32,
) {
    let format = match (dst.stencil, dst.texture.desc.format) {
        (true, _) => Format::R8Uint,
        (false, Format::Depth16Unorm) => Format::R16Uint,
        _ => Format::R32Uint,
    };
    let Ok(scratch) = make_texture(
        gpu,
        &TextureDesc {
            dimension: TextureDimension::D2,
            format,
            width: extent.width,
            height: extent.height,
            depth_or_layers: extent.depth_or_layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::SAMPLE | TextureUsage::COPY_DST,
            cube: false,
            reinterpret: false,
        },
    ) else {
        return fail(gpu, LossReason::OutOfMemory);
    };
    let src = Region {
        texture: scratch.clone(),
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    upload(gpu, &src, extent, texels, row_pitch, rows_per_layer);
    if dst.stencil {
        stencil_by_draw(gpu, &src, dst, extent);
    } else if copy_by_draw(gpu, &src, dst, extent) {
        gpu.record_slow_path(SlowPath::DepthWrittenByDraw);
    } else {
        unbuilt(gpu, "depth written into textures of this format");
    }
    destroy_held(gpu, Held::Texture(scratch));
}

/// Writes the stencil of `dst` from the `r8uint` texels of `src` (2D or 2D array), layer by
/// layer: one draw sets the region's stencil to 0, then one per bit sets that bit where the
/// source has it ([`SlowPath::StencilWrittenByDraw`]). Depth stays.
fn stencil_by_draw<O: OpenGlEs>(gpu: &mut Gl<O>, src: &Region, dst: &Region, extent: Extent) {
    gpu.record_slow_path(SlowPath::StencilWrittenByDraw);
    let k = usize::from(src.texture.target == gles::TEXTURE_2D_ARRAY);
    if gpu.stencil_write[k].is_none() {
        match depth::build_stencil_write(&gpu.gl, k == 1) {
            Ok(p) => gpu.stencil_write[k] = Some(p),
            Err(log) => {
                own_program_failed(gpu, log);
                return;
            }
        }
    }
    if bind_source(gpu, &src.texture, src.mip).is_err() {
        return fail(gpu, LossReason::OutOfMemory);
    }
    let rect = [
        dst.origin.x as i32,
        dst.origin.y as i32,
        extent.width as i32,
        extent.height as i32,
    ];
    for i in 0..extent.depth_or_layers {
        let fb = texture_framebuffer(gpu, &dst.texture, dst.mip, read_layer(dst, i));
        gpu.gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, fb);
        gpu.applied.draw_framebuffer = Some(fb);
        gpu.applied.program = 0;
        open_writes(gpu);
        state::plain(&gpu.gl, &mut gpu.values, rect);
        let program = gpu.stencil_write[k].as_ref().expect("built above");
        let (gl, v) = (&gpu.gl, &mut gpu.values);
        let replace = [gles::KEEP, gles::KEEP, gles::REPLACE];
        gl.use_program(program.program);
        gl.uniform3i(
            program.at,
            src.origin.x as i32 - rect[0],
            src.origin.y as i32 - rect[1],
            (src.layer + i) as i32,
        );
        gl.enable(gles::STENCIL_TEST);
        v.stencil_test = true;
        gl.stencil_op(replace[0], replace[1], replace[2]);
        v.stencil_op = [replace; 2];
        // Bit 0 of the loop is the clear: no fragment discarded, 0 written through every bit.
        for (bit, reference, mask) in
            std::iter::once((0, 0, 0xff)).chain((0..8).map(|b| (1u32 << b, 0xff, 1u8 << b)))
        {
            let program = gpu.stencil_write[k].as_ref().expect("built above");
            gpu.gl.uniform1ui(program.bit, bit);
            gpu.gl.stencil_func(gles::ALWAYS, reference, 0xff);
            gpu.gl.stencil_mask(u32::from(mask));
            gpu.values.stencil_func = [(gles::ALWAYS, reference as u8, 0xff); 2];
            gpu.values.stencil_write_mask = mask;
            own_draw(gpu);
        }
    }
}

/// Draws the back end's full-screen triangle with GL's lower-left window origin.
fn own_draw<O: OpenGlEs>(gpu: &mut Gl<O>) {
    if !gpu.glsl.flip_y {
        let _ = gpu
            .gl
            .clip_control_ext(gles::LOWER_LEFT_EXT, gpu.values.clip_depth);
    }
    gpu.gl.draw_arrays(gles::TRIANGLES, 0, 3);
    if !gpu.glsl.flip_y {
        let _ = gpu
            .gl
            .clip_control_ext(gles::UPPER_LEFT_EXT, gpu.values.clip_depth);
    }
}

/// One of the back end's own programs failed to build: a bug of ours, so the device is lost with
/// GL's log.
fn own_program_failed<O: OpenGlEs>(gpu: &mut Gl<O>, log: String) -> GpuError {
    gpu.last_log = log;
    fail(gpu, LossReason::Error);
    GpuError::DeviceLost
}

/// A nearest-filtering sampler for the back end's own fetches and for textures read only by
/// loads, made at its first use.
fn point_sampler<O: OpenGlEs>(gpu: &mut Gl<O>) -> Result<u32, GpuError> {
    if let Some(s) = gpu.point_sampler {
        return Ok(s);
    }
    let s = {
        let s = platform::one(|n| gpu.gl.gen_samplers(n)).ok_or(GpuError::OutOfMemory)?;
        // A mipmap filter: texel fetches reach only the base level without one.
        gpu.gl.sampler_parameteri(
            s,
            gles::TEXTURE_MIN_FILTER,
            gles::NEAREST_MIPMAP_NEAREST as i32,
        );
        gpu.gl
            .sampler_parameteri(s, gles::TEXTURE_MAG_FILTER, gles::NEAREST as i32);
        s
    };
    gpu.point_sampler = Some(s);
    Ok(s)
}

/// Makes `texture`, bound on the active unit, give its stencil (`stencil`) or its depth to programs
/// (WEBGL_stencil_texturing's mode, which is the texture's, not a view's).
fn aspect<O: OpenGlEs>(gpu: &mut Gl<O>, texture: &TextureObj, stencil: bool) {
    if texture.stencil_mode.get() != stencil {
        let mode = if stencil {
            gles::STENCIL_INDEX
        } else {
            gles::DEPTH_COMPONENT
        };
        gpu.gl.tex_parameteri(
            texture.target,
            gles::DEPTH_STENCIL_TEXTURE_MODE,
            mode as i32,
        );
        texture.stencil_mode.set(stencil);
    }
}

/// Counts the stencil of level `mip` and layer `layer` of `src` into a new `r8unorm` texture of the
/// level's size, without WEBGL_stencil_texturing: eight full-screen draws, each passing the stencil
/// test only where its bit is set and adding the bit's weight by blending with the constant colour,
/// which unorm8 blending sums exactly ([`SlowPath::StencilCopiedByDraw`]).
fn stencil_bits<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    src: &TextureRef,
    mip: u32,
    layer: u32,
) -> Result<TextureRef, GpuError> {
    gpu.record_slow_path(SlowPath::StencilCopiedByDraw);
    if gpu.bits_program.is_none() {
        match depth::build_bits(&gpu.gl) {
            Ok(p) => gpu.bits_program = Some(p),
            Err(log) => return Err(own_program_failed(gpu, log)),
        }
    }
    let (w, h) = (
        (src.desc.width >> mip).max(1),
        (src.desc.height >> mip).max(1),
    );
    let counted = make_texture(
        gpu,
        &TextureDesc {
            dimension: TextureDimension::D2,
            format: Format::R8Unorm,
            width: w,
            height: h,
            depth_or_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::SAMPLE,
            cube: false,
            reinterpret: false,
        },
    )?;
    let fb = own_framebuffer(gpu, &[(&counted, 0, 0), (src, mip, layer)]);
    gpu.gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, fb);
    gpu.gl.use_program(gpu.bits_program.unwrap_or(0));
    gpu.applied.draw_framebuffer = Some(fb);
    gpu.applied.program = 0;
    open_writes(gpu);
    state::plain(&gpu.gl, &mut gpu.values, [0, 0, w as i32, h as i32]);
    let (gl, v) = (&gpu.gl, &mut gpu.values);
    let keep = [gles::KEEP; 3];
    gl.clear_bufferfv(gles::COLOR, 0, &[0.0; 4]);
    let add = state::GlTarget {
        on: true,
        func: [
            gles::CONSTANT_COLOR,
            gles::ONE,
            gles::CONSTANT_COLOR,
            gles::ONE,
        ],
        ..state::GlTarget::OFF
    };
    state::uniform(gl, v, &add);
    gl.enable(gles::STENCIL_TEST);
    v.stencil_test = true;
    gl.stencil_op(keep[0], keep[1], keep[2]);
    v.stencil_op = [keep; 2];
    for bit in 0..8 {
        let weight = f32::from(1u8 << bit) / 255.0;
        let func = (gles::EQUAL, 1u8 << bit, 1u8 << bit);
        gpu.gl
            .stencil_func(func.0, i32::from(func.1), u32::from(func.2));
        gpu.gl.blend_color(weight, weight, weight, weight);
        gpu.values.stencil_func = [func; 2];
        gpu.values.blend_color = [weight; 4];
        own_draw(gpu);
    }
    Ok(counted)
}

/// Binds a texture on unit 0 for an upload or a parameter change. The next draw walks its bindings
/// again, so unit 0 gets back what its program samples there.
fn bind_scratch<O: OpenGlEs>(gpu: &mut Gl<O>, target: u32, raw: u32) {
    if gpu.applied.units.is_empty() {
        gpu.applied.units.push((0, 0));
    }
    gpu.applied.units[0] = (0, 0);
    gpu.applied.sets = [Id::MAX; MAX_BINDING_SETS];
    gpu.gl.active_texture(gles::TEXTURE0);
    gpu.gl.bind_texture(target, raw);
}

/// Writes texels from the host or from a buffer (bound as the pixel unpack buffer) into a region.
/// A region's first texel and slice (layer, or depth slice in 3D), as the host copy counts them.
fn corner(region: &Region) -> [u32; 3] {
    let z = if region.texture.desc.dimension == TextureDimension::D3 {
        region.origin.z
    } else {
        region.layer
    };
    [region.origin.x, region.origin.y, z]
}

/// A copy from or into a texture kept on the host, in blocks (`extent` is in the source's
/// texels): the source's blocks from its host copy, or read back first where it has none
/// ([`SlowPath::HostCopyReadBack`]), then written into the destination as a host write (into
/// both its forms where it keeps a host copy, as texels where it does not).
fn copy_host_kept<O: OpenGlEs>(gpu: &mut Gl<O>, src: &Region, dst: &Region, extent: Extent) {
    let (sbw, sbh, bytes) = FORMAT_FACTS[src.texture.desc.format as usize].block;
    let (dbw, dbh, dst_bytes) = FORMAT_FACTS[dst.texture.desc.format as usize].block;
    if bytes != dst_bytes {
        return invalid(
            gpu,
            format!(
                "a copy from {:?} into {:?}, whose blocks differ in size",
                src.texture.desc.format, dst.texture.desc.format
            ),
        );
    }
    let (cols, rows) = (extent.width.div_ceil(sbw), extent.height.div_ceil(sbh));
    let mut packed = vec![0; (cols * rows * bytes * extent.depth_or_layers) as usize];
    if src.texture.host.is_none() {
        gpu.record_slow_path(SlowPath::HostCopyReadBack);
    }
    if read_texture_now(gpu, src, extent, &mut packed).is_err() {
        unbuilt(
            gpu,
            "copies into textures kept on the host from formats GL cannot read back",
        );
        return;
    }
    let into = Extent {
        width: cols * dbw,
        height: rows * dbh,
        depth_or_layers: extent.depth_or_layers,
    };
    write_texture_held(
        gpu,
        dst,
        into,
        &packed,
        cols * bytes,
        rows,
        WriteMode::Ordered,
    );
}

/// Before the levels of `levels` (by bit) of the host copy of `t` are read: where draws wrote
/// their texels since (signed normalised formats with EXT_render_snorm), each is drawn into a
/// texture of its signed integer twin, which reads back as stored, and written into the host
/// copy ([`SlowPath::HostCopyReadBack`]). A draw reads the smallest value as the one above it,
/// so in a level whose host copy may hold it, the texels are not recovered.
fn host_current<O: OpenGlEs>(gpu: &mut Gl<O>, t: &TextureRef, levels: u32) -> Result<(), GpuError> {
    let drawn = t.drawn.get() & levels;
    if drawn == 0 {
        return Ok(());
    }
    if drawn & t.minimum.get() != 0 {
        return Err(unbuilt(
            gpu,
            "the bits of signed normalised texels drawn into after a write of their smallest value",
        ));
    }
    let Some(h) = &t.host else { return Ok(()) };
    gpu.record_slow_path(SlowPath::HostCopyReadBack);
    let d = t.desc;
    let twin = match d.format {
        Format::R8Snorm => Format::R8Sint,
        Format::Rg8Snorm => Format::Rg8Sint,
        Format::Rgba8Snorm => Format::Rgba8Sint,
        Format::R16Snorm => Format::R16Sint,
        Format::Rg16Snorm => Format::Rg16Sint,
        _ => Format::Rgba16Sint,
    };
    for mip in (0..d.mip_levels).filter(|m| drawn & (1 << m) != 0) {
        let extent = Extent {
            width: (d.width >> mip).max(1),
            height: (d.height >> mip).max(1),
            depth_or_layers: if d.dimension == TextureDimension::D3 {
                (d.depth_or_layers >> mip).max(1)
            } else {
                d.depth_or_layers
            },
        };
        let held = make_texture(
            gpu,
            &TextureDesc {
                dimension: TextureDimension::D2,
                format: twin,
                width: extent.width,
                height: extent.height,
                depth_or_layers: extent.depth_or_layers,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
                cube: false,
                reinterpret: false,
            },
        )?;
        let region = |texture, mip| Region {
            texture,
            mip,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        let (texel, texels) = (
            FORMAT_FACTS[twin as usize].block.2,
            extent.width * extent.height,
        );
        let mut bytes = vec![0; (texels * extent.depth_or_layers * texel) as usize];
        let read = match copy_by_draw(
            gpu,
            &region(t.clone(), mip),
            &region(held.clone(), 0),
            extent,
        ) {
            true => read_texture_now(gpu, &region(held.clone(), 0), extent, &mut bytes),
            false => Err(unbuilt(gpu, "draws from this signed normalised format")),
        };
        destroy_held(gpu, Held::Texture(held));
        read?;
        let size = [extent.width, extent.height, extent.depth_or_layers];
        let row = (extent.width * texel) as usize;
        h.write(&d, mip, [0; 3], size, &bytes, row, extent.height as usize);
    }
    t.drawn.set(t.drawn.get() & !drawn);
    Ok(())
}

/// Uploads a region of a block-compressed texture from its host copy: as blocks natively, else
/// decoded ([`SlowPath::BlockDecoded`]).
fn upload_blocks<O: OpenGlEs>(gpu: &mut Gl<O>, region: &Region, extent: Extent) {
    let t = region.texture.clone();
    let Some(h) = &t.host else { return };
    let level = |size: u32| (size >> region.mip).max(1);
    // The texels the region covers inside the level; blocks may reach past its edge.
    let w = extent.width.min(level(t.desc.width) - region.origin.x);
    let ht = extent.height.min(level(t.desc.height) - region.origin.y);
    let (bw, bh, block_bytes) = FORMAT_FACTS[t.desc.format as usize].block;
    let (wide, high) = (w.div_ceil(bw) as usize, ht.div_ceil(bh) as usize);
    let slices = extent.depth_or_layers as usize;
    let mut blocks = vec![0; wide * high * slices * block_bytes as usize];
    h.read(
        &t.desc,
        region.mip,
        corner(region),
        [w, ht, extent.depth_or_layers],
        &mut blocks,
    );
    let extent = Extent {
        width: w,
        height: ht,
        ..extent
    };
    if let Some(decoded) = t.decoded {
        let texels = host::decode(t.desc.format, &blocks, wide);
        let texel = FORMAT_FACTS[decoded as usize].block.2;
        return upload(
            gpu,
            region,
            extent,
            Texels::Bytes(&texels),
            wide as u32 * bw * texel,
            high as u32 * bh,
        );
    }
    let Some(raw) = t.tex() else { return };
    bind_scratch(gpu, t.target, raw);
    let gl = &gpu.gl;
    let (x, y, level) = (
        region.origin.x as i32,
        region.origin.y as i32,
        region.mip as i32,
    );
    let slice = wide * high * block_bytes as usize;
    match t.target {
        gles::TEXTURE_2D => gl.compressed_tex_sub_image_2d(
            t.target,
            level,
            x,
            y,
            w as i32,
            ht as i32,
            t.gl.internal,
            CompressedPixelUnpack::Bytes(&blocks),
        ),
        gles::TEXTURE_CUBE_MAP => {
            for (i, face) in blocks.chunks_exact(slice).enumerate() {
                gl.compressed_tex_sub_image_2d(
                    gles::TEXTURE_CUBE_MAP_POSITIVE_X + region.layer + i as u32,
                    level,
                    x,
                    y,
                    w as i32,
                    ht as i32,
                    t.gl.internal,
                    CompressedPixelUnpack::Bytes(face),
                );
            }
        }
        _ => gl.compressed_tex_sub_image_3d(
            t.target,
            level,
            x,
            y,
            region.layer as i32,
            w as i32,
            ht as i32,
            slices as i32,
            t.gl.internal,
            CompressedPixelUnpack::Bytes(&blocks),
        ),
    }
}

fn upload<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    region: &Region,
    extent: Extent,
    texels: Texels<'_>,
    row_pitch: u32,
    rows_per_layer: u32,
) {
    let t = region.texture.clone();
    let (_, _, block_bytes) = FORMAT_FACTS[t.decoded.unwrap_or(t.desc.format) as usize].block;
    let Some(raw) = t.tex() else {
        return invalid(gpu, "texels written into a multisampled texture".into());
    };
    bind_scratch(gpu, t.target, raw);
    let gl = &gpu.gl;
    let fmt = t.gl;
    let (x, y, w, h) = (
        region.origin.x as i32,
        region.origin.y as i32,
        extent.width as i32,
        extent.height as i32,
    );
    let layer_bytes = row_pitch as usize * rows_per_layer as usize;
    let from = |i: usize| match texels {
        Texels::Bytes(data) => PixelUnpack::Bytes(&data[i * layer_bytes..]),
        Texels::Offset(offset) => PixelUnpack::Offset(u64::from(offset) + (i * layer_bytes) as u64),
    };
    gl.pixel_storei(gles::UNPACK_ROW_LENGTH, (row_pitch / block_bytes) as i32);
    gl.pixel_storei(gles::UNPACK_IMAGE_HEIGHT, rows_per_layer as i32);
    let level = region.mip as i32;
    match t.target {
        gles::TEXTURE_2D => {
            gl.tex_sub_image_2d(t.target, level, x, y, w, h, fmt.format, fmt.ty, from(0))
        }
        gles::TEXTURE_CUBE_MAP => {
            for i in 0..extent.depth_or_layers {
                gl.tex_sub_image_2d(
                    gles::TEXTURE_CUBE_MAP_POSITIVE_X + region.layer + i,
                    level,
                    x,
                    y,
                    w,
                    h,
                    fmt.format,
                    fmt.ty,
                    from(i as usize),
                );
            }
        }
        _ => {
            let z = if t.target == gles::TEXTURE_3D {
                region.origin.z
            } else {
                region.layer
            };
            gl.tex_sub_image_3d(
                t.target,
                level,
                x,
                y,
                z as i32,
                w,
                h,
                extent.depth_or_layers as i32,
                fmt.format,
                fmt.ty,
                from(0),
            );
        }
    }
}

/// Calls `read` on a region of a texture GL cannot read, drawn into a colour texture it can:
/// depth as colour ([`SlowPath::DepthReadByDraw`]), 16-bit and packed floats as the bits of
/// their unsigned integer twin ([`SlowPath::CopyThroughBuffer`]); None for other textures. Then lets the
/// colour texture go.
fn through_draw<R, O: OpenGlEs>(
    gpu: &mut Gl<O>,
    region: &Region,
    extent: Extent,
    read: impl FnOnce(&mut Gl<O>, &Region) -> Result<R, GpuError>,
) -> Option<Result<R, GpuError>> {
    let format = region.texture.desc.format;
    let scratch = if depth::stored(format).is_some() {
        depth_by_draw(gpu, region, extent)
    } else if let Some(twin) = format::bits_twin(format)
        && region.texture.host.is_none()
    {
        bits_by_draw(gpu, region, extent, twin)
    } else {
        return None;
    };
    Some(scratch.and_then(|scratch| {
        let colour = Region {
            texture: scratch.clone(),
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        let r = read(gpu, &colour);
        destroy_held(gpu, Held::Texture(scratch.clone()));
        r
    }))
}

/// A region's bits drawn into a new 2D texture of `twin`, an integer format of the same bits.
fn bits_by_draw<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    region: &Region,
    extent: Extent,
    twin: Format,
) -> Result<TextureRef, GpuError> {
    let held = make_texture(
        gpu,
        &TextureDesc {
            dimension: TextureDimension::D2,
            format: twin,
            width: extent.width,
            height: extent.height,
            depth_or_layers: extent.depth_or_layers,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::COPY_SRC,
            cube: false,
            reinterpret: false,
        },
    )?;
    let at = Region {
        texture: held.clone(),
        mip: 0,
        layer: 0,
        origin: Origin::default(),
        stencil: false,
    };
    if !copy_by_draw(gpu, region, &at, extent) {
        destroy_held(gpu, Held::Texture(held));
        return Err(unbuilt(gpu, "reading back this format"));
    }
    gpu.record_slow_path(SlowPath::CopyThroughBuffer);
    Ok(held)
}

/// Reads one region of a colour texture into `dst`, rows tightly packed, now.
fn read_texture_now<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    region: &Region,
    extent: Extent,
    dst: &mut [u8],
) -> Result<(), GpuError> {
    if let Some(h) = &region.texture.host {
        host_current(gpu, &region.texture, 1 << region.mip)?;
        let corner = corner(region);
        let size = [extent.width, extent.height, extent.depth_or_layers];
        h.read(&region.texture.desc, region.mip, corner, size, dst);
        return Ok(());
    }
    if let Some(r) = through_draw(gpu, region, extent, |b, colour| {
        read_texture_now(b, colour, extent, dst)
    }) {
        return r;
    }
    fresh(gpu, &region.texture);
    let t = &region.texture;
    let read = t.read.ok_or(GpuError::Invalid(
        "webgl: the format cannot be read back here",
    ))?;
    let texels = (extent.width * extent.height) as usize;
    let mut wide = Vec::new();
    if read.repacks() {
        wide.resize(texels * read.texel_read(), 0);
    }
    if read.converts() {
        gpu.record_slow_path(SlowPath::FormatConverted);
    }
    for i in 0..extent.depth_or_layers as usize {
        let layer = read_layer(region, i as u32);
        read_framebuffer(gpu, t, region.mip, layer);
        let out = &mut dst[i * texels * read.texel_kept()..(i + 1) * texels * read.texel_kept()];
        let into = if wide.is_empty() {
            &mut *out
        } else {
            &mut wide[..]
        };
        gpu.gl.read_pixels(
            region.origin.x as i32,
            region.origin.y as i32,
            extent.width as i32,
            extent.height as i32,
            read.format,
            read.ty,
            PixelPack::Bytes(into),
        );
        if !wide.is_empty() {
            read.repack(&wide, out);
        }
    }
    // readPixels has waited for the GPU; an error it shows leaves the bytes unread.
    let lost = gpu.lost.is_some();
    gl_errors(gpu);
    if !lost && gpu.lost.is_some() {
        return Err(GpuError::DeviceLost);
    }
    Ok(())
}

/// The texture layer, or 3D slice, of layer `i` of a region.
fn read_layer(region: &Region, i: u32) -> u32 {
    if region.texture.desc.dimension == TextureDimension::D3 {
        region.origin.z + i
    } else {
        region.layer + i
    }
}

/// The GL buffer of `buffer` that a copy or read can reach, brought up to date: the data one, else
/// the index one.
fn any_gl<O: OpenGlEs>(gpu: &mut Gl<O>, buffer: &BufferObj) -> u32 {
    let index = buffer.data.is_none();
    refresh(gpu, buffer, Some(index));
    if index { buffer.index } else { buffer.data }.expect("every buffer has a GL buffer")
}

/// Brings `buffer`'s stale range up to date from its other GL buffer, if the stale GL buffer is the
/// index one (`Some(true)`), the data one (`Some(false)`) or either (`None`). Reading back waits
/// for the GPU ([`SlowPath::IndexTwinReadBack`]).
fn refresh<O: OpenGlEs>(gpu: &mut Gl<O>, buffer: &BufferObj, which: Option<bool>) {
    let Some(stale) = buffer.stale.get() else {
        return;
    };
    if which.is_some_and(|i| i != stale.index) {
        return;
    }
    gpu.record_slow_path(SlowPath::IndexTwinReadBack);
    let (from, to) = if stale.index {
        (buffer.data, buffer.index)
    } else {
        (buffer.index, buffer.data)
    };
    let mut bytes = vec![0; (stale.end - stale.start) as usize];
    gpu.gl
        .bind_buffer(gles::COPY_READ_BUFFER, from.unwrap_or(0));
    if !platform::get_buffer_sub_data(&gpu.gl, gles::COPY_READ_BUFFER, stale.start, &mut bytes) {
        read_failed(gpu);
        return;
    }
    gpu.gl.bind_buffer(gles::COPY_WRITE_BUFFER, to.unwrap_or(0));
    gpu.gl
        .buffer_sub_data(gles::COPY_WRITE_BUFFER, stale.start as i64, &bytes);
    buffer.stale.set(None);
    gpu.stale_buffers -= 1;
}

/// Marks bytes `start..end` of `buffer`'s index or data GL buffer stale. Its other GL buffer is up
/// to date there, since every GPU write refreshes its destination first.
fn mark_stale<O: OpenGlEs>(gpu: &mut Gl<O>, buffer: &BufferObj, index: bool, start: u64, end: u64) {
    if buffer.stale.get().is_none() {
        gpu.stale_buffers += 1;
    }
    buffer.stale.set(Some(Stale { index, start, end }));
}

/// Brings up to date every range a draw reads: indices from the index GL buffer, streams and
/// uniform blocks from the data one.
fn refresh_draw<O: OpenGlEs>(gpu: &mut Gl<O>, drawn: &Drawn) {
    if let Some(index) = &drawn.index {
        refresh(gpu, &index.buffer, Some(true));
    }
    for stream in drawn.vertex_buffers.iter().flatten() {
        refresh(gpu, &stream.buffer, Some(false));
    }
    for set in drawn.bindings.iter().flatten() {
        for (_, bound) in &set.entries {
            if let Bound::Uniform(buffer, _, _) = bound {
                refresh(gpu, buffer, Some(false));
            }
        }
    }
}

/// What `program`'s link decided, read at its first draw: `PipelineMiss` when that draw waits for
/// the link (always without KHR_parallel_shader_compile, which alone can ask without waiting).
fn linked<'p, O: OpenGlEs>(gpu: &mut Gl<O>, program: &'p ProgramObj) -> Option<&'p shader::Linked> {
    if let Some(l) = program.linked.get() {
        return Some(l);
    }
    let parallel = gpu.extensions.contains("KHR_parallel_shader_compile");
    if !parallel || !platform::completed(&gpu.gl, program.raw) {
        gpu.record_slow_path(SlowPath::PipelineMiss);
    }
    match shader::finish(&gpu.gl, program) {
        Ok(l) => {
            // Finishing left the program in use; its bindings are walked afresh.
            gpu.applied.program = program.id;
            gpu.applied.sets = [Id::MAX; MAX_BINDING_SETS];
            Some(program.linked.get_or_init(|| l))
        }
        Err(log) => {
            gpu.last_log = log;
            fail(gpu, LossReason::Error);
            None
        }
    }
}

/// The variant of `program` for a vertex layout whose attributes in `key` GL ES cannot read as
/// stored, made at the first draw with it ([`SlowPath::VertexFormatConverted`], counted per draw).
/// None when it could not be made, which has lost the device.
fn variant<O: OpenGlEs>(gpu: &mut Gl<O>, program: &ProgramObj, key: u32) -> Option<Rc<ProgramObj>> {
    gpu.record_slow_path(SlowPath::VertexFormatConverted);
    if let Some((_, v)) = program.variants.borrow().iter().find(|(k, _)| *k == key) {
        return Some(v.clone());
    }
    let id = id(gpu);
    match shader::relink(&gpu.gl, id, program, None, key, gpu.empty_fragment) {
        Ok(v) => {
            let v = Rc::new(v);
            program.variants.borrow_mut().push((key, v.clone()));
            Some(v)
        }
        Err(e) => {
            gpu.last_log = format!("webgl: a program variant for converted vertex formats: {e:?}");
            fail(gpu, LossReason::Error);
            None
        }
    }
}

/// Binds what the program reads from the draw's sets. Err says why the draw cannot run; nothing of
/// it may run then.
fn apply_bindings<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    program: Id,
    linked: &shader::Linked,
    sets: &[Option<&SetRef>; MAX_BINDING_SETS],
) -> Result<(), Unbindable> {
    let ids = sets.map(|s| s.map_or(0, |s| s.id));
    if gpu.applied.program == program && gpu.applied.sets == ids {
        return Ok(());
    }
    // Walked again at the next draw unless this walk completes.
    gpu.applied.sets = [Id::MAX; MAX_BINDING_SETS];
    let find = |(set, binding): Slot| {
        sets.get(set as usize).copied().flatten().and_then(|s| {
            s.entries
                .iter()
                .find(|(b, _)| *b == binding)
                .map(|(_, r)| r)
        })
    };
    if gpu.applied.units.len() < linked.units.len() {
        gpu.applied.units.resize(linked.units.len(), (0, 0));
    }
    // Textures read only by loads get the nearest sampler: GLES leaves integer and depth textures
    // incomplete under the texture's own default filters, and loads ignore filters.
    let point = if linked.units.iter().any(|(_, s, _)| s.is_none()) {
        match point_sampler(gpu) {
            Ok(s) => Some(s),
            Err(_) => return Err(Unbindable::NoSampler),
        }
    } else {
        None
    };
    for (unit, (texture, sampler, level_zero)) in linked.units.iter().enumerate() {
        let Some(Bound::Texture(v)) = find(*texture) else {
            return Err(Unbindable::Missing(*texture));
        };
        // A texture read only by loads has no sampler.
        let sampler = match sampler {
            None => None,
            Some(slot) => match find(*slot) {
                Some(Bound::Sampler(s)) => Some(s),
                _ => return Err(Unbindable::Missing(*slot)),
            },
        };
        // Gathers read level 0 whatever the sampler's minimum level of detail.
        let sampler = sampler.map(|s| match s.level_zero {
            Some(twin) if *level_zero => twin,
            _ => (s.id, s.raw),
        });
        let ids = (v.id, sampler.map_or(POINT_SAMPLER, |s| s.0));
        let t = &v.texture;
        let Some(raw) = t.tex() else {
            return Err(Unbindable::Multisampled(*texture));
        };
        let levels = (v.desc.base_mip, v.desc.base_mip + v.desc.mip_count - 1);
        let other_aspect = FORMAT_FACTS[t.desc.format as usize].depth_stencil
            && t.stencil_mode.get() != v.desc.stencil;
        if gpu.applied.units[unit] != ids || t.levels.get() != levels || other_aspect {
            gpu.gl.active_texture(gles::TEXTURE0 + unit as u32);
            gpu.gl.bind_texture(t.target, raw);
        }
        if other_aspect {
            aspect(gpu, t, v.desc.stencil);
        }
        let gl = &gpu.gl;
        if t.levels.get() != levels {
            gl.tex_parameteri(t.target, gles::TEXTURE_BASE_LEVEL, levels.0 as i32);
            gl.tex_parameteri(t.target, gles::TEXTURE_MAX_LEVEL, levels.1 as i32);
            t.levels.set(levels);
        }
        if let Some((location, held)) = &linked.sizes[unit] {
            let base = v.desc.base_mip;
            let d = &t.desc;
            let depth = match d.dimension {
                TextureDimension::D3 => (d.depth_or_layers >> base).max(1),
                _ => v.desc.layer_count,
            };
            let size = [(d.width >> base).max(1), (d.height >> base).max(1), depth];
            let size = size.map(|s| s as i32);
            if held.get() != size {
                gl.uniform3i(*location, size[0], size[1], size[2]);
                held.set(size);
            }
        }
        if gpu.applied.units[unit].1 != ids.1 {
            gl.bind_sampler(unit as u32, sampler.map(|s| s.1).or(point).unwrap_or(0));
        }
        gpu.applied.units[unit] = ids;
    }
    // The stencil mode and the mip range are the texture's: one draw cannot fetch both aspects of
    // one texture, or read it through two mip ranges.
    for (texture, _, _) in &linked.units {
        if let Some(Bound::Texture(v)) = find(*texture) {
            if FORMAT_FACTS[v.texture.desc.format as usize].depth_stencil
                && v.texture.stencil_mode.get() != v.desc.stencil
            {
                return Err(Unbindable::BothAspects);
            }
            if v.texture.levels.get() != (v.desc.base_mip, v.desc.base_mip + v.desc.mip_count - 1) {
                return Err(Unbindable::TwoMipRanges(*texture));
            }
        }
    }
    if gpu.applied.blocks.len() < linked.blocks.len() {
        gpu.applied.blocks.resize(linked.blocks.len(), (0, 0, 0));
    }
    for (point, slot) in linked.blocks.iter().enumerate() {
        let Some(Bound::Uniform(buffer, offset, size)) = find(*slot) else {
            return Err(Unbindable::Missing(*slot));
        };
        let key = (buffer.id, *offset, *size);
        if gpu.applied.blocks[point] != key {
            gpu.gl.bind_buffer_range(
                gles::UNIFORM_BUFFER,
                point as u32,
                buffer.data.unwrap_or(0),
                *offset as i64,
                *size as i64,
            );
            gpu.applied.blocks[point] = key;
        }
    }
    gpu.applied.sets = ids;
    Ok(())
}

/// Points each attribute at its stream, moved by the draw's base vertex and first instance
/// (WebGL 2 has no draws with bases: [`SlowPath::BaseVertexRebind`]), and per-instance streams
/// on by what `later` instances past the first step through at their rate. Err when a stream's
/// offset would fall below zero.
fn apply_vertices<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    drawn: &Drawn,
    base_vertex: i64,
    first_instance: u32,
    later: u32,
) -> Result<(), ()> {
    let layout = &drawn.vertex_layout.0.1;
    let mut wanted = [None; MAX_VERTEX_STREAMS];
    for (w, a) in wanted.iter_mut().zip(&layout.attributes) {
        let Some(binding) = drawn.vertex_buffers.get(a.stream).and_then(|b| b.as_ref()) else {
            *w = Some((a, None, 0));
            continue;
        };
        let moved = if a.per_instance {
            let stepped = match a.divisor {
                NEVER_ADVANCES => 0,
                n => later / n,
            };
            i64::from(first_instance) + i64::from(stepped)
        } else {
            base_vertex
        } * i64::from(a.stride);
        let offset = binding.offset as i64 + i64::from(a.offset) + moved;
        let offset = u64::try_from(offset).map_err(|_| ())?;
        *w = Some((a, Some(&binding.buffer), offset));
    }
    let n = layout.attributes.len();
    let wanted = &wanted[..n];
    let changed = gpu.applied.layout != drawn.vertex_layout.0.0
        || gpu.applied.attributes.len() != n
        || wanted
            .iter()
            .flatten()
            .zip(&gpu.applied.attributes)
            .any(|((_, b, o), applied)| (b.map_or(0, |b| b.id), *o) != *applied);
    if !changed {
        return Ok(());
    }
    let gl = &gpu.gl;
    let mut enabled = 0u32;
    gpu.applied.attributes.clear();
    for &(a, buffer, offset) in wanted.iter().flatten() {
        gpu.applied
            .attributes
            .push((buffer.map_or(0, |b| b.id), offset));
        let Some(buffer) = buffer else { continue };
        enabled |= 1 << a.location;
        gl.bind_buffer(gles::ARRAY_BUFFER, buffer.data.unwrap_or(0));
        let stride = a.stride as i32;
        if a.integer {
            gl.vertex_attrib_i_pointer(a.location, a.components, a.ty, stride, offset);
        } else {
            gl.vertex_attrib_pointer(a.location, a.components, a.ty, a.normalized, stride, offset);
        }
        gl.vertex_attrib_divisor(a.location, a.divisor);
    }
    let flip = enabled ^ gpu.applied.enabled;
    for location in (0..32).filter(|l| flip & (1 << l) != 0) {
        if enabled & (1 << location) != 0 {
            gl.enable_vertex_attrib_array(location);
        } else {
            gl.disable_vertex_attrib_array(location);
        }
    }
    gpu.applied.enabled = enabled;
    gpu.applied.layout = drawn.vertex_layout.0.0;
    Ok(())
}
/// [`Gpu::draw`], with the draw's instances starting `later` instances past its first
/// instance: `instance_index` and the per-instance streams as the full draw has them there.
/// The bytes of a draw's indices, read back from the index GL buffer (waiting for the GPU).
fn read_indices<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    index: &Index,
    first: u32,
    count: u32,
) -> Result<Vec<u8>, GpuError> {
    let range = index_bytes(index, first, count);
    if range.end > index.buffer.size {
        return Err(GpuError::Invalid(
            "indices past the end of the index buffer",
        ));
    }
    let mut raw = vec![0u8; (range.end - range.start) as usize];
    refresh(gpu, &index.buffer, Some(true));
    let gl_buffer = index.buffer.index.or(index.buffer.data);
    let read = {
        gpu.gl
            .bind_buffer(gles::COPY_READ_BUFFER, gl_buffer.unwrap_or(0));
        platform::get_buffer_sub_data(&gpu.gl, gles::COPY_READ_BUFFER, range.start, &mut raw)
    };
    if !read {
        return Err(read_failed(gpu));
    }
    Ok(raw)
}

/// An indexed draw that must not restart, whose indices may hold GL's restart value (which WebGL
/// cannot turn off): they are read back and, where one is there, drawn rewritten as 32-bit indices
/// from the index ring ([`SlowPath::RestartRewrite`]).
fn draw_unrestarted<O: OpenGlEs>(gpu: &mut Gl<O>, call: &DrawCall<'_, Gl<O>>, drawn: &Drawn) {
    let (
        Some(index),
        DrawRange::Indexed {
            first,
            count,
            base_vertex,
            first_instance,
            instances,
        },
    ) = (&drawn.index, &call.range)
    else {
        return draw_from(gpu, call, drawn, 0);
    };
    let (first, count, base_vertex) = (*first, *count, *base_vertex);
    let (first_instance, instances) = (*first_instance, *instances);
    gpu.record_slow_path(SlowPath::RestartRewrite);
    let Ok(bytes) = read_indices(gpu, index, first, count) else {
        return invalid(gpu, "indices past the end of the index buffer".into());
    };
    let start = index_bytes(index, first, count).start;
    index.buffer.maxima.borrow_mut().wrote(start, &bytes);
    let (widened, shift) = match restart::rewrite(&bytes, index.format) {
        restart::Rewrite::Unneeded => return draw_from(gpu, call, drawn, 0),
        restart::Rewrite::Widened(widened, shift) => (widened, shift),
        restart::Rewrite::Unreachable => {
            unbuilt(
                gpu,
                "a draw whose 32-bit indices hold both 0 and 2^32 - 1 without restart",
            );
            return;
        }
    };
    let Ok(base_vertex) = i32::try_from(i64::from(base_vertex) + i64::from(shift)) else {
        unbuilt(
            gpu,
            "a draw without restart whose lowered 32-bit indices need a base vertex past \
             2^31 - 1",
        );
        return;
    };
    let mut offset = [0];
    let ring = match gpu.write_buffer_ring(RingKind::Indices, &[&widened], 4, &mut offset) {
        Ok(ring) => ring,
        Err(_) => return fail(gpu, LossReason::OutOfMemory),
    };
    let Some(ring_held) = held(&gpu.objects.buffers, ring.0) else {
        return invalid(gpu, GpuError::GONE.into());
    };
    let rewritten = Drawn {
        index: Some(Index {
            buffer: ring_held,
            offset: offset[0],
            format: IndexFormat::U32,
        }),
        ..drawn.clone()
    };
    draw_from(
        gpu,
        &DrawCall {
            index: Some(IndexBinding {
                buffer: &ring,
                offset: offset[0],
                format: IndexFormat::U32,
            }),
            range: DrawRange::Indexed {
                first: 0,
                count,
                base_vertex,
                first_instance,
                instances,
            },
            ..*call
        },
        &rewritten,
        0,
    );
}

fn draw_from<O: OpenGlEs>(gpu: &mut Gl<O>, call: &DrawCall<'_, Gl<O>>, drawn: &Drawn, later: u32) {
    let (base_vertex, first_instance, instances) = match call.range {
        DrawRange::Vertices {
            first_instance,
            instances,
            ..
        } => (0, first_instance, instances),
        DrawRange::Indexed {
            base_vertex,
            first_instance,
            instances,
            ..
        } => (i64::from(base_vertex), first_instance, instances),
        DrawRange::Indirect { .. } | DrawRange::IndexedIndirect { .. } => {
            // INDIRECT is absent: the front reads the arguments back first.
            invalid(gpu, "an indirect draw without INDIRECT".into());
            return;
        }
    };
    if call.viewports.len() != 1 {
        invalid(
            gpu,
            format!("{} viewports in one draw", call.viewports.len()),
        );
        return;
    }
    if call.stencil_reference[0] != call.stencil_reference[1] {
        // STENCIL_PER_FACE is absent: the front draws each face apart.
        invalid(gpu, "front and back stencil references differ".into());
        return;
    }
    if call.immediates.len() > gpu.caps.limits.immediate_bytes as usize {
        invalid(
            gpu,
            format!(
                "{} bytes of immediates past the limit of {}",
                call.immediates.len(),
                gpu.caps.limits.immediate_bytes
            ),
        );
        return;
    }
    let converted = drawn.vertex_layout.0.1.converted;
    let variant = match converted {
        0 => None,
        key => match variant(gpu, &drawn.program, key) {
            Some(v) => Some(v),
            None => return,
        },
    };
    let program: &ProgramObj = variant.as_deref().unwrap_or(&drawn.program);
    let Some(linked) = linked(gpu, program) else {
        return;
    };
    if gpu.stale_buffers > 0 {
        refresh_draw(gpu, drawn);
    }
    if gpu.twins > 0 {
        for set in drawn.bindings.iter().flatten() {
            for (_, bound) in &set.entries {
                if let Bound::Texture(v) = bound {
                    fresh(gpu, &v.texture);
                }
            }
        }
        // Filling a stencil copy draws into framebuffers of its own.
        if let Some(pass) = &gpu.pass
            && gpu.applied.draw_framebuffer != Some(pass.framebuffer)
        {
            gpu.gl
                .bind_framebuffer(gles::DRAW_FRAMEBUFFER, pass.framebuffer);
            gpu.applied.draw_framebuffer = Some(pass.framebuffer);
        }
    }
    if gpu.applied.program != program.id {
        gpu.gl.use_program(program.raw);
        gpu.applied.program = program.id;
        gpu.applied.sets = [u64::MAX; MAX_BINDING_SETS];
        let n = program.stages.0.clip_distances;
        if gpu.values.clip_distances != n {
            state::clip_distances(&gpu.gl, &mut gpu.values, n);
        }
    }
    if gpu.applied.blend != drawn.blend.0.0 {
        // GL blends a second source into one colour target only.
        if drawn.blend.0.1.second_source && gpu.pass.as_ref().is_some_and(|p| p.color_targets > 1) {
            invalid(
                gpu,
                "a second-source blend with more than one colour target".into(),
            );
            return;
        }
        state::blend(
            &gpu.gl,
            &mut gpu.values,
            &drawn.blend.0.1,
            gpu.pass_unwritten,
        );
        gpu.applied.blend = drawn.blend.0.0;
    }
    let (gl, v) = (&gpu.gl, &mut gpu.values);
    let reference = call.stencil_reference[0];
    if gpu.applied.depth_stencil != drawn.depth_stencil.0.0
        || gpu.applied.stencil_reference != reference
    {
        state::depth_stencil(gl, v, &drawn.depth_stencil.0.1, reference);
        gpu.applied.depth_stencil = drawn.depth_stencil.0.0;
        gpu.applied.stencil_reference = reference;
    }
    if gpu.applied.raster != drawn.raster.0.0 {
        state::raster(gl, v, &drawn.raster.0.1);
        gpu.applied.raster = drawn.raster.0.0;
    }
    if let Some(vp) = call.viewports.first() {
        let floats = [f(vp.x), f(vp.y), f(vp.width), f(vp.height)];
        let range = [f(vp.min_depth), f(vp.max_depth)];
        // The interface's bounds (WebGPU's); a front draws a larger viewport in the program.
        let most = gpu.caps.limits.max_texture_2d as f32;
        let edge = |at: f32, size: f32| {
            (0.0..=most).contains(&size) && at >= -2.0 * most && at + size <= 2.0 * most - 1.0
        };
        if !edge(floats[0], floats[2])
            || !edge(floats[1], floats[3])
            || !range.iter().all(|d| (0.0..=1.0).contains(d))
            || range[0] > range[1]
        {
            invalid(
                gpu,
                format!(
                    "the viewport {floats:?} with depths {range:?} past the interface's bounds"
                ),
            );
            return;
        }
        let rect = floats.map(|v| v as i32);
        // FRACTIONAL_VIEWPORT is absent: the front draws such a viewport in the program.
        if floats.iter().zip(&rect).any(|(v, r)| *v != *r as f32) {
            invalid(
                gpu,
                format!("the fractional viewport {floats:?} without FRACTIONAL_VIEWPORT"),
            );
            return;
        }
        if v.viewport != rect {
            gl.viewport(rect[0], rect[1], rect[2], rect[3]);
            v.viewport = rect;
        }
        if v.depth_range != range {
            gl.depth_rangef(range[0], range[1]);
            v.depth_range = range;
        }
    }
    if v.scissor_test
        && let Some(s) = call.scissors.first()
    {
        let rect = gl_rect(s, gpu.pass.as_ref().map_or(NO_PASS, |p| p.size));
        if v.scissor != rect {
            gl.scissor(rect[0], rect[1], rect[2], rect[3]);
            v.scissor = rect;
        }
    }
    let color = call.blend_constant.map(f);
    if v.blend_color != color {
        gl.blend_color(color[0], color[1], color[2], color[3]);
        v.blend_color = color;
    }
    match apply_bindings(
        gpu,
        program.id,
        linked,
        &drawn.bindings.each_ref().map(Option::as_ref),
    ) {
        Ok(()) => {}
        Err(Unbindable::Missing((set, binding))) => {
            invalid(
                gpu,
                format!(
                    "the draw's binding sets hold no fitting resource at set {set} binding \
                 {binding}, which its program reads"
                ),
            );
            return;
        }
        Err(Unbindable::BothAspects) => {
            unbuilt(
                gpu,
                "depth and stencil views of one texture fetched by one draw",
            );
            return;
        }
        Err(Unbindable::NoSampler) => {
            fail(gpu, LossReason::OutOfMemory);
            return;
        }
        Err(Unbindable::Multisampled((set, binding))) => {
            invalid(
                gpu,
                format!(
                    "the draw's program reads the multisampled texture at set {set} binding \
                 {binding}, which needs the absent MULTISAMPLE_READ"
                ),
            );
            return;
        }
        Err(Unbindable::TwoMipRanges((set, binding))) => {
            invalid(
                gpu,
                format!(
                    "the draw reads the texture at set {set} binding {binding} through two mip \
                 ranges, which needs the absent TEXTURE_VIEWS"
                ),
            );
            return;
        }
    }
    if base_vertex != 0 || first_instance != 0 || later != 0 {
        gpu.record_slow_path(SlowPath::BaseVertexRebind);
    }
    if apply_vertices(gpu, drawn, base_vertex, first_instance, later).is_err() {
        unbuilt(gpu, "negative stream offsets after the base vertex");
        return;
    }
    // The streams moved by the bases, so GL's ids lack them: the program adds them back.
    for (uniform, value) in [
        (&linked.first_instance, first_instance + later),
        (&linked.base_vertex, base_vertex as u32),
    ] {
        if let Some((location, held)) = uniform
            && held.get() != value
        {
            gpu.gl.uniform1ui(*location, value);
            held.set(value);
        }
    }
    shader::set_immediates(&gpu.gl, linked, call.immediates);
    let mode = state::mode(call.topology);
    let gl = &gpu.gl;
    match call.range {
        DrawRange::Vertices { first, count, .. } => {
            gl.draw_arrays_instanced(mode, first as i32, count as i32, instances as i32)
        }
        DrawRange::Indexed { first, count, .. } => {
            let Some(index) = &drawn.index else {
                return invalid(gpu, "an indexed draw without an index buffer".into());
            };
            if gpu.applied.index != index.buffer.id {
                gl.bind_buffer(gles::ELEMENT_ARRAY_BUFFER, index.buffer.index.unwrap_or(0));
                gpu.applied.index = index.buffer.id;
            }
            let ty = match index.format {
                IndexFormat::U8 => gles::UNSIGNED_BYTE,
                IndexFormat::U16 => gles::UNSIGNED_SHORT,
                IndexFormat::U32 => gles::UNSIGNED_INT,
            };
            let offset = index.offset
                + u64::from(first) * u64::from(INDEX_FORMATS[index.format as usize].size);
            gl.draw_elements_instanced(mode, count as i32, ty, offset, instances as i32)
        }
        _ => unreachable!("indirect draws returned above"),
    }
}

/// Why a draw's bindings cannot be bound.
enum Unbindable {
    /// The sets hold no fitting resource at a slot the program reads.
    Missing(Slot),
    /// Depth and stencil views of one texture, which WEBGL_stencil_texturing cannot give together.
    BothAspects,
    /// The nearest sampler for loads could not be made.
    NoSampler,
    /// A multisampled texture, which programs cannot read here (`MULTISAMPLE_READ` is absent).
    Multisampled(Slot),
    /// A texture also read through another mip range, which needs the absent `TEXTURE_VIEWS`.
    TwoMipRanges(Slot),
}

/// Where texels written into a texture come from.
#[derive(Clone, Copy)]
enum Texels<'a> {
    Bytes(&'a [u8]),
    /// An offset into the bound pixel unpack buffer.
    Offset(u32),
}

/// Floats travel as bits through the interface.
fn f(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// The area a draw outside a pass (a capture) clips its scissor to.
const NO_PASS: (u32, u32) = (i32::MAX as u32, i32::MAX as u32);

/// The byte range of a buffer that a draw's indices `first..first + count` take.
fn index_bytes(index: &Index, first: u32, count: u32) -> Range<u64> {
    let size = u64::from(INDEX_FORMATS[index.format as usize].size);
    let start = index.offset + u64::from(first) * size;
    start..start + u64::from(count) * size
}

/// A scissor clipped to a pass of `size`, as GL takes it.
fn gl_rect(s: &ScissorRect, size: (u32, u32)) -> [i32; 4] {
    let (x, y) = (s.x.min(size.0), s.y.min(size.1));
    let width = s.width.min(size.0 - x);
    let height = s.height.min(size.1 - y);
    [x, y, width, height].map(|v| v as i32)
}

impl<O: OpenGlEs> Gpu for Gl<O> {
    type Buffer = Buffer;
    type Texture = Texture;
    type TextureView = TextureView;
    type Sampler = Sampler;
    type Shader = Shader;
    type Program = Program;
    type Blend = Blend;
    type DepthStencil = DepthStencil;
    type Raster = Raster;
    type VertexLayout = VertexLayout;
    type BindingSet = BindingSet;
    type QuerySet = QuerySet;
    type Readback = Readback;
    type Fence = Fence<O::Fence>;
    type ResourceTable = NoTable;
    type Surface = Surface;

    fn caps(&self) -> &Caps {
        &self.caps
    }

    fn slow_paths(&mut self) -> &mut SlowPaths {
        &mut self.slow
    }

    fn status(&mut self) -> DeviceStatus {
        match self.lost {
            Some(reason) => DeviceStatus::Lost(reason),
            None => DeviceStatus::Ok,
        }
    }

    fn helpers(&mut self) -> &mut Helpers<Self> {
        &mut self.helpers
    }

    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Buffer, GpuError> {
        let b = make_buffer(self, desc)?;
        self.objects.buffers.insert(b.id, b.clone());
        Ok(Buffer(b.id))
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Texture, GpuError> {
        let t = make_texture(self, desc)?;
        self.objects.textures.insert(t.id, t.clone());
        Ok(Texture(t.id))
    }

    fn create_view(
        &mut self,
        texture: &Texture,
        desc: &TextureViewDesc,
    ) -> Result<TextureView, GpuError> {
        let texture =
            held(&self.objects.textures, texture.0).ok_or(GpuError::Invalid(GpuError::GONE))?;
        let v = make_view(self, &texture, desc)?;
        self.objects.views.insert(v.id, v.clone());
        Ok(TextureView(v.id))
    }

    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Sampler, GpuError> {
        let address = |a| match a {
            AddressMode::Repeat => Ok(gles::REPEAT),
            AddressMode::Mirror => Ok(gles::MIRRORED_REPEAT),
            AddressMode::Clamp => Ok(gles::CLAMP_TO_EDGE),
            AddressMode::Border => Err(GpuError::Absent(Features::BORDER_COLOR)),
            AddressMode::MirrorOnce if self.caps.features.contains(Features::MIRROR_ONCE) => {
                Ok(gles::MIRROR_CLAMP_TO_EDGE_EXT)
            }
            AddressMode::MirrorOnce => Err(GpuError::Absent(Features::MIRROR_ONCE)),
        };
        if desc.anisotropy > self.caps.limits.max_anisotropy {
            return Err(GpuError::Invalid("a sampler's anisotropy past the limit"));
        }
        let wrap = [
            address(desc.address[0])?,
            address(desc.address[1])?,
            address(desc.address[2])?,
        ];
        let min = match (desc.min, desc.mip) {
            (Filter::Nearest, Filter::Nearest) => gles::NEAREST_MIPMAP_NEAREST,
            (Filter::Linear, Filter::Nearest) => gles::LINEAR_MIPMAP_NEAREST,
            (Filter::Nearest, Filter::Linear) => gles::NEAREST_MIPMAP_LINEAR,
            (Filter::Linear, Filter::Linear) => gles::LINEAR_MIPMAP_LINEAR,
        };
        let mag = match desc.mag {
            Filter::Nearest => gles::NEAREST,
            Filter::Linear => gles::LINEAR,
        };
        let gl = &self.gl;
        let make = |min_lod: f32| {
            let raw = platform::one(|n| gl.gen_samplers(n)).ok_or(GpuError::OutOfMemory)?;
            gl.sampler_parameteri(raw, gles::TEXTURE_MIN_FILTER, min as i32);
            gl.sampler_parameteri(raw, gles::TEXTURE_MAG_FILTER, mag as i32);
            for (name, mode) in [
                gles::TEXTURE_WRAP_S,
                gles::TEXTURE_WRAP_T,
                gles::TEXTURE_WRAP_R,
            ]
            .into_iter()
            .zip(wrap)
            {
                gl.sampler_parameteri(raw, name, mode as i32);
            }
            gl.sampler_parameterf(raw, gles::TEXTURE_MIN_LOD, min_lod);
            gl.sampler_parameterf(raw, gles::TEXTURE_MAX_LOD, f(desc.lod_range[1]));
            if let Some(c) = desc.compare {
                gl.sampler_parameteri(
                    raw,
                    gles::TEXTURE_COMPARE_MODE,
                    gles::COMPARE_REF_TO_TEXTURE as i32,
                );
                gl.sampler_parameteri(raw, gles::TEXTURE_COMPARE_FUNC, state::compare(c) as i32);
            }
            // Above 1 only where EXT_texture_filter_anisotropic raised the limit.
            if desc.anisotropy > 1 {
                let a = f32::from(desc.anisotropy);
                gl.sampler_parameterf(raw, gles::TEXTURE_MAX_ANISOTROPY_EXT, a);
            }
            Ok(raw)
        };
        let raw = make(f(desc.lod_range[0]))?;
        let twin = match f(desc.lod_range[0]) > 0.0 {
            true => Some(make(0.0)?),
            false => None,
        };
        let level_zero = twin.map(|t| (id(self), t));
        let number = id(self);
        let made = Handle(Rc::new(SamplerObj {
            id: number,
            raw,
            level_zero,
        }));
        self.objects.samplers.insert(number, made);
        Ok(Sampler(number))
    }

    fn create_shader(&mut self, module: &ShaderModule) -> Result<Shader, GpuError> {
        if module.stage == ShaderStage::Compute {
            return Err(GpuError::Absent(Features::COMPUTE));
        }
        let missing = shader::needs(module).difference(self.caps.features);
        if !missing.is_empty() {
            return Err(GpuError::Absent(missing));
        }
        // GL would refuse the link, which the first draw finds as a lost device.
        if shader::uniform_blocks(module) > self.caps.limits.uniform_buffers_per_stage {
            return Err(GpuError::Invalid(
                "a stage reads more uniform buffers than Limits::uniform_buffers_per_stage",
            ));
        }
        let text = Rc::new(shader::write(module, self.glsl)?);
        if let Some(s) = self.shaders.get(&text) {
            s.handles.set(s.handles.get() + 1);
            return Ok(Shader(s.id));
        }
        let raw = shader::start_compile(&self.gl, text.stage, &text.source)?;
        let s = Handle(Rc::new(shader::ShaderObj {
            id: id(self),
            raw,
            handles: core::cell::Cell::new(1),
            text: text.clone(),
        }));
        self.shaders.insert(text, s.clone());
        self.objects.shaders.insert(s.id, s.clone());
        Ok(Shader(s.id))
    }

    fn create_program(
        &mut self,
        program: &wcontainers_gpu::shader::Program<'_, Self>,
    ) -> Result<Program, GpuError> {
        let wcontainers_gpu::shader::Program::Render { vertex, fragment } = program else {
            return Err(GpuError::Absent(Features::COMPUTE));
        };
        let vertex =
            held(&self.objects.shaders, vertex.0).ok_or(GpuError::Invalid(GpuError::GONE))?;
        let fragment = match fragment {
            Some(f) => {
                Some(held(&self.objects.shaders, f.0).ok_or(GpuError::Invalid(GpuError::GONE))?)
            }
            None => None,
        };
        let pair = (vertex.id, fragment.as_ref().map_or(0, |f| f.id));
        if let Some(p) = self.programs.get(&pair) {
            p.handles.set(p.handles.get() + 1);
            return Ok(Program(p.id));
        }
        let id = id(self);
        let started = shader::link(
            &self.gl,
            id,
            &vertex.0,
            fragment.as_ref().map(|f| &f.0),
            self.empty_fragment,
        )?;
        let p = Handle(Rc::new(started));
        self.programs.insert(pair, p.clone());
        self.objects.programs.insert(p.id, p.clone());
        Ok(Program(p.id))
    }

    fn create_blend(&mut self, desc: &BlendDesc) -> Result<Blend, GpuError> {
        if desc.logic_op.is_some() {
            return Err(GpuError::Absent(Features::LOGIC_OP));
        }
        let used = if desc.same_for_all {
            &desc.targets[..1]
        } else {
            &desc.targets[..]
        };
        if !desc.same_for_all
            && used.iter().any(|t| *t != used[0])
            && !self.caps.features.contains(Features::INDEPENDENT_BLEND)
        {
            return Err(GpuError::Absent(Features::INDEPENDENT_BLEND));
        }
        let second = used.iter().filter_map(|t| t.blend).any(|(c, a)| {
            [c.src, c.dst, a.src, a.dst]
                .into_iter()
                .any(state::second_source)
        });
        if second && !self.caps.features.contains(Features::DUAL_SOURCE_BLEND) {
            return Err(GpuError::Absent(Features::DUAL_SOURCE_BLEND));
        }
        // WebGL refuses the constant colour and the constant alpha as a colour blend's two factors.
        let mixed = used.iter().filter_map(|t| t.blend).any(|(c, _)| {
            let colour = |f| matches!(f, BlendFactor::Constant | BlendFactor::OneMinusConstant);
            let alpha = |f| {
                matches!(
                    f,
                    BlendFactor::ConstantAlpha | BlendFactor::OneMinusConstantAlpha
                )
            };
            (colour(c.src) && alpha(c.dst)) || (alpha(c.src) && colour(c.dst))
        });
        if mixed {
            return Err(GpuError::Absent(Features::BLEND_CONSTANT_ALPHA));
        }
        let number = id(self);
        let made = Handle(Rc::new((number, state::GlBlend::new(desc))));
        self.objects.blends.insert(number, made);
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
        let number = id(self);
        let made = Handle(Rc::new((number, state::GlDepthStencil::new(desc))));
        self.objects.depth_stencils.insert(number, made);
        Ok(DepthStencil(number))
    }

    fn create_raster(&mut self, desc: &RasterDesc) -> Result<Raster, GpuError> {
        let has = |f| self.caps.features.contains(f);
        let absent = [
            (
                desc.fill == FillMode::Line && !has(Features::POLYGON_MODE),
                Features::POLYGON_MODE,
            ),
            (desc.fill == FillMode::Point, Features::POLYGON_MODE_POINT),
            (
                desc.provoking == ProvokingVertex::First
                    && !self
                        .caps
                        .features
                        .contains(Features::PROVOKING_VERTEX_FIRST),
                Features::PROVOKING_VERTEX_FIRST,
            ),
            (
                !desc.depth_clip && !has(Features::DEPTH_CLAMP),
                Features::DEPTH_CLAMP,
            ),
            (
                f(desc.depth_bias_clamp) != 0.0 && !has(Features::DEPTH_BIAS_CLAMP),
                Features::DEPTH_BIAS_CLAMP,
            ),
            (
                f(desc.depth_bias).fract() != 0.0
                    && !self.caps.features.contains(Features::FRACTIONAL_DEPTH_BIAS),
                Features::FRACTIONAL_DEPTH_BIAS,
            ),
            (f(desc.line_width) != 1.0, Features::WIDE_LINES),
            (desc.sample_mask != u32::MAX, Features::SAMPLE_MASK),
            (
                !desc.depth_minus_one && !self.clip_control,
                Features::CLIP_DEPTH_ZERO_ONE,
            ),
        ];
        if let Some((_, feature)) = absent.into_iter().find(|(missing, _)| *missing) {
            return Err(GpuError::Absent(feature));
        }
        let number = id(self);
        let made = Handle(Rc::new((
            number,
            state::GlRaster::new(desc, self.glsl.flip_y, self.clip_control),
        )));
        self.objects.rasters.insert(number, made);
        Ok(Raster(number))
    }

    fn create_vertex_layout(&mut self, desc: &VertexLayoutDesc) -> Result<VertexLayout, GpuError> {
        if desc
            .streams
            .iter()
            .flatten()
            .any(|s| s.stride > self.caps.limits.max_vertex_stride)
        {
            return Err(GpuError::Invalid("webgl: vertex stride above 255"));
        }
        if desc.attributes.iter().flatten().any(|a| {
            desc.streams
                .get(usize::from(a.stream))
                .copied()
                .flatten()
                .is_none()
        }) {
            return Err(GpuError::Invalid(
                "an attribute names a stream the layout lacks",
            ));
        }
        if desc
            .attributes
            .iter()
            .flatten()
            .any(|a| a.location >= self.caps.limits.vertex_attributes.min(16))
        {
            return Err(GpuError::Invalid(
                "an attribute at a location past the limit",
            ));
        }
        match state::GlLayout::new(desc) {
            Some(layout) => {
                let number = id(self);
                self.objects
                    .layouts
                    .insert(number, Handle(Rc::new((number, layout))));
                Ok(VertexLayout(number))
            }
            None => Err(GpuError::Invalid(
                "an attribute names a stream the layout lacks",
            )),
        }
    }

    fn create_binding_set(
        &mut self,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<BindingSet, GpuError> {
        let mut bound = Vec::with_capacity(entries.len());
        for e in entries {
            let r = match &e.resource {
                BindingResource::UniformBuffer(r) => Bound::Uniform(
                    held(&self.objects.buffers, r.buffer.0)
                        .ok_or(GpuError::Invalid(GpuError::GONE))?,
                    r.offset,
                    r.size,
                ),
                BindingResource::Texture(v) => {
                    let v =
                        held(&self.objects.views, v.0).ok_or(GpuError::Invalid(GpuError::GONE))?;
                    // A GL texture is sampled whole in its own target: views to draw into of one
                    // layer, or of a cube's face, cannot be sampled.
                    let t = &v.texture.desc;
                    let layers = match t.dimension {
                        TextureDimension::D3 => 1,
                        _ => t.depth_or_layers,
                    };
                    if v.desc.dimension != t.own_view()
                        || v.desc.base_layer != 0
                        || v.desc.layer_count != layers
                    {
                        return Err(GpuError::Absent(Features::TEXTURE_VIEWS));
                    }
                    Bound::Texture(v)
                }
                BindingResource::Sampler(s) => Bound::Sampler(
                    held(&self.objects.samplers, s.0).ok_or(GpuError::Invalid(GpuError::GONE))?,
                ),
                BindingResource::StorageBuffer { .. } | BindingResource::StorageTexture { .. } => {
                    return Err(GpuError::Absent(
                        Features::FRAGMENT_STORAGE_WRITE | Features::VERTEX_STORAGE_READ,
                    ));
                }
                // WebGL 2 has no buffer textures: fronts read typed buffers another way.
                BindingResource::TexelBuffer { .. } => {
                    return Err(GpuError::Absent(Features::TEXEL_BUFFERS));
                }
                BindingResource::Table(_) => {
                    return Err(GpuError::Absent(Features::RESOURCE_TABLES));
                }
            };
            bound.push((e.binding, r));
        }
        let number = id(self);
        let made = Handle(Rc::new(SetObj {
            id: number,
            entries: bound,
        }));
        self.objects.sets.insert(number, made);
        Ok(BindingSet(number))
    }

    fn binding_layout_key(&self, _set: &BindingSet) -> BindingLayoutKey {
        BindingLayoutKey::default()
    }

    fn create_query_set(&mut self, kind: QueryKind, count: u32) -> Result<QuerySet, GpuError> {
        match kind {
            QueryKind::OcclusionAny => {}
            // GL ES answers only whether any sample passed.
            QueryKind::Occlusion => return Err(GpuError::Absent(Features::OCCLUSION_COUNT)),
            QueryKind::Timestamp if !self.caps.features.contains(Features::TIMESTAMPS) => {
                return Err(GpuError::Absent(Features::TIMESTAMPS));
            }
            QueryKind::Timestamp => {}
        }
        let number = id(self);
        let made = Handle(Rc::new(QuerySetObj {
            kind,
            slots: core::cell::RefCell::new(vec![None; count as usize]),
        }));
        self.objects.query_sets.insert(number, made);
        Ok(QuerySet(number))
    }

    fn destroy(&mut self, object: Object<Self>) {
        let o = &mut self.objects;
        let held = match object {
            Object::Buffer(b) => o.buffers.remove(&b.0).map(Held::Buffer),
            Object::Texture(t) => o.textures.remove(&t.0).map(Held::Texture),
            // A view and a binding set hold no GL object of their own.
            Object::TextureView(v) => {
                o.views.remove(&v.0);
                None
            }
            Object::Sampler(x) => o.samplers.remove(&x.0).map(Held::Sampler),
            // Shaders and programs of equal text are one object: it leaves the table with its last handle.
            Object::Shader(x) => held(&o.shaders, x.0).map(Held::Shader),
            Object::Program(x) => held(&o.programs, x.0).map(Held::Program),
            Object::Blend(x) => {
                o.blends.remove(&x.0);
                None
            }
            Object::DepthStencil(x) => {
                o.depth_stencils.remove(&x.0);
                None
            }
            Object::Raster(x) => {
                o.rasters.remove(&x.0);
                None
            }
            Object::VertexLayout(x) => {
                o.layouts.remove(&x.0);
                None
            }
            Object::BindingSet(x) => {
                o.sets.remove(&x.0);
                None
            }
            Object::QuerySet(x) => o.query_sets.remove(&x.0).map(Held::QuerySet),
            Object::Surface(x) => Some(Held::Surface(x.0)),
            Object::ResourceTable(t) => match t {},
        };
        if let Some(held) = held {
            destroy_held(self, held);
        }
    }

    fn write_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8], mode: WriteMode) {
        match held(&self.objects.buffers, buffer.0) {
            Some(buffer) => write_buffer_held(self, &buffer, offset, data, mode),
            None => invalid(self, GpuError::GONE.into()),
        }
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
        match region_of(self, region) {
            Some(region) => {
                write_texture_held(self, &region, extent, data, row_pitch, rows_per_layer, mode)
            }
            None => invalid(self, GpuError::GONE.into()),
        }
    }

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        let Some(pass) = pass_of(self, pass) else {
            return invalid(self, GpuError::GONE.into());
        };
        if let Some(empty) = pass.empty {
            return begin_empty_pass(self, empty, pass.occlusion.clone());
        }
        let mut key: FramebufferKey = [None; MAX_COLOR_TARGETS + 1];
        let mut views: [Option<&ViewRef>; MAX_COLOR_TARGETS + 1] = [None; MAX_COLOR_TARGETS + 1];
        for (i, c) in pass.colors.iter().enumerate() {
            if let Some(c) = c {
                key[i] = Some(attachment(&c.view, c.depth_slice));
                views[i] = Some(&c.view);
            }
        }
        if let Some(d) = &pass.depth {
            key[MAX_COLOR_TARGETS] = Some(attachment(&d.view, 0));
            views[MAX_COLOR_TARGETS] = Some(&d.view);
        }
        let size = views
            .iter()
            .flatten()
            .fold((u32::MAX, u32::MAX), |(w, h), v| {
                let level = |n: u32| (n >> v.desc.base_mip).max(1);
                (
                    w.min(level(v.texture.desc.width)),
                    h.min(level(v.texture.desc.height)),
                )
            });
        if self.twins > 0 {
            for v in views.iter().flatten() {
                fresh(self, &v.texture);
                written(self, &v.texture);
            }
        }
        for (i, v) in views.iter().enumerate() {
            let Some(v) = v else { continue };
            if v.texture.host.is_none() {
                continue;
            }
            // A pass replacing every texel of a level of one slice leaves no minimum there.
            let (t, mip) = (&v.texture, v.desc.base_mip);
            let replaced = pass
                .colors
                .get(i)
                .and_then(Option::as_ref)
                .is_some_and(|c| !matches!(c.load, LoadOp::Load));
            if replaced && host::slices(&t.desc, mip) == 1 {
                t.minimum.set(t.minimum.get() & !(1 << mip));
            }
            t.drawn.set(t.drawn.get() | 1 << mip);
        }
        // The channels draws leave in each target; slots without a target follow the first.
        let colors = &views[..MAX_COLOR_TARGETS];
        let unwritten = |v: &ViewRef| format::unwritten(v.texture.desc.format);
        let first = colors.iter().flatten().next().map_or(0, |v| unwritten(v));
        let unwritten = if colors.iter().flatten().any(|v| unwritten(v) != 0) {
            if colors.iter().flatten().any(|v| unwritten(v) != first)
                && !self.caps.features.contains(Features::INDEPENDENT_BLEND)
            {
                unbuilt(self, MIXED_CHANNELS);
                return;
            }
            colors.iter().enumerate().fold(0, |bits, (i, v)| {
                let channels = v.map_or(first, unwritten);
                bits | u32::from(channels) << (i * format::CHANNELS)
            })
        } else {
            0
        };
        if self.pass_unwritten != unwritten {
            self.pass_unwritten = unwritten;
            self.applied.blend = 0;
        }
        let fb = framebuffer(self, key, &views);
        let gl = &self.gl;
        let mut start_discard = Points::default();
        let mut end_discard = Points::default();
        if self.applied.draw_framebuffer != Some(fb) {
            gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, fb);
            self.applied.draw_framebuffer = Some(fb);
        }
        let clears = pass
            .colors
            .iter()
            .flatten()
            .any(|c| matches!(c.load, LoadOp::Clear(_)))
            || pass.depth.as_ref().is_some_and(|d| {
                d.depth.is_some_and(|a| matches!(a.load, LoadOp::Clear(_)))
                    || d.stencil
                        .is_some_and(|s| matches!(s.load, LoadOp::Clear(_)))
            });
        if clears {
            // Clears obey the scissor and the write masks.
            open_writes(self);
        }
        let gl = &self.gl;
        // A layer of a 2D array is never invalidated: in Chrome (ANGLE on Direct3D 11) that
        // loses the other layers of the level.
        let whole = |v: &ViewRef| v.texture.target != gles::TEXTURE_2D_ARRAY;
        for (i, c) in pass.colors.iter().enumerate() {
            let Some(c) = c else { continue };
            let point = gles::COLOR_ATTACHMENT0 + i as u32;
            let format = c.view.texture.desc.format;
            // A target lacking channels keeps their values, which a discard would lose.
            let whole = whole(&c.view) && format::unwritten(format) == 0;
            match c.load {
                LoadOp::Clear(value) => match FORMAT_FACTS[format as usize].kind {
                    FormatKind::Uint => gl.clear_bufferuiv(gles::COLOR, i as i32, &value),
                    FormatKind::Sint => {
                        gl.clear_bufferiv(gles::COLOR, i as i32, &value.map(|x| x as i32))
                    }
                    _ => gl.clear_bufferfv(
                        gles::COLOR,
                        i as i32,
                        &format::clear_value(format, value).map(f),
                    ),
                },
                LoadOp::DontCare if whole => start_discard.push(point),
                LoadOp::DontCare | LoadOp::Load => {}
            }
            if c.store == StoreOp::Discard && whole {
                end_discard.push(point);
            }
        }
        if let Some(d) = &pass.depth {
            let depth = d.depth.and_then(|a| match a.load {
                LoadOp::Clear(v) => Some(f(v)),
                _ => None,
            });
            let stencil = d.stencil.and_then(|a| match a.load {
                LoadOp::Clear(v) => Some(i32::from(v)),
                _ => None,
            });
            match (depth, stencil) {
                (Some(depth), Some(stencil)) => {
                    gl.clear_bufferfi(gles::DEPTH_STENCIL, 0, depth, stencil)
                }
                (Some(depth), None) => gl.clear_bufferfv(gles::DEPTH, 0, &[depth]),
                (None, Some(stencil)) => gl.clear_bufferiv(gles::STENCIL, 0, &[stencil]),
                (None, None) => {}
            }
            for (ops, point) in [
                (
                    d.depth.map(|a| (a.load == LoadOp::DontCare, a.store)),
                    gles::DEPTH_ATTACHMENT,
                ),
                (
                    d.stencil.map(|a| (a.load == LoadOp::DontCare, a.store)),
                    gles::STENCIL_ATTACHMENT,
                ),
            ] {
                if let Some((load, store)) = ops
                    && whole(&d.view)
                {
                    if load {
                        start_discard.push(point);
                    }
                    if store == StoreOp::Discard {
                        end_discard.push(point);
                    }
                }
            }
        }
        if start_discard.len > 0 {
            gl.invalidate_framebuffer(gles::DRAW_FRAMEBUFFER, start_discard.get());
        }
        self.pass = Some(Pass {
            framebuffer: fb,
            discard: end_discard,
            occlusion: pass.occlusion.clone(),
            counting: false,
            resolves: core::array::from_fn(|i| {
                let c = pass.colors[i].as_ref()?;
                Some((c.view.texture.clone(), c.resolve.clone()?))
            }),
            size,
            color_targets: pass.colors.iter().flatten().count(),
        });
    }

    fn end_pass(&mut self) {
        if self.pass.as_ref().is_some_and(|p| p.counting) {
            invalid(self, "a pass ended with its occlusion query running".into());
        }
        let Some(pass) = self.pass.take() else {
            return;
        };
        let mut resolved = false;
        for (ms, view) in pass.resolves.iter().flatten() {
            let t = view.texture.clone();
            fresh(self, &t);
            written(self, &t);
            blit_resolve(self, ms, &t, view.desc.base_mip, view.desc.base_layer);
            resolved = true;
        }
        if resolved && pass.discard.len > 0 {
            self.gl
                .bind_framebuffer(gles::DRAW_FRAMEBUFFER, pass.framebuffer);
            self.applied.draw_framebuffer = Some(pass.framebuffer);
        }
        if pass.discard.len > 0 {
            self.gl
                .invalidate_framebuffer(gles::DRAW_FRAMEBUFFER, pass.discard.get())
        }
    }

    fn draw(&mut self, call: &DrawCall<'_, Self>) {
        let Some(drawn) = drawn_of(self, call) else {
            return invalid(self, GpuError::GONE.into());
        };
        if !call.restart
            && let (Some(index), DrawRange::Indexed { first, count, .. }) =
                (&drawn.index, &call.range)
            && index
                .buffer
                .maxima
                .borrow()
                .may_hold(index.format, index_bytes(index, *first, *count))
        {
            return draw_unrestarted(self, call, &drawn);
        }
        draw_from(self, call, &drawn, 0);
    }

    fn begin_query(&mut self, index: u32) {
        let Some(pass) = &self.pass else {
            return invalid(self, "begin_query outside a pass".into());
        };
        let (Some(set), false) = (pass.occlusion.clone(), pass.counting) else {
            return invalid(
                self,
                "begin_query without an occlusion set or with a query running".into(),
            );
        };
        let Some(q) = query_slot(self, &set, index) else {
            return;
        };
        self.gl.begin_query(gles::ANY_SAMPLES_PASSED, q);
        if let Some(p) = &mut self.pass {
            p.counting = true;
        }
    }

    fn end_query(&mut self) {
        match &mut self.pass {
            Some(p) if p.counting => p.counting = false,
            _ => return invalid(self, "end_query with no query running".into()),
        }
        self.gl.end_query(gles::ANY_SAMPLES_PASSED);
    }

    fn copy_buffer(
        &mut self,
        src: &Buffer,
        src_offset: u64,
        dst: &Buffer,
        dst_offset: u64,
        size: u64,
    ) {
        let Some(src) = held(&self.objects.buffers, src.0) else {
            return invalid(self, GpuError::GONE.into());
        };
        let Some(dst) = held(&self.objects.buffers, dst.0) else {
            return invalid(self, GpuError::GONE.into());
        };
        // Both sides are made whole first, so afterwards at most the side no copy reached is stale.
        refresh(self, &src, None);
        refresh(self, &dst, None);
        let pairs = [(src.data, dst.data, false), (src.index, dst.index, true)];
        let mut copied = false;
        let mut missed = None;
        for (s, d, index) in pairs {
            match (s, d) {
                (Some(s), Some(d)) => {
                    let gl = &self.gl;
                    gl.bind_buffer(gles::COPY_READ_BUFFER, s);
                    gl.bind_buffer(gles::COPY_WRITE_BUFFER, d);
                    gl.copy_buffer_sub_data(
                        gles::COPY_READ_BUFFER,
                        gles::COPY_WRITE_BUFFER,
                        src_offset as i64,
                        dst_offset as i64,
                        size as i64,
                    );
                    copied = true;
                }
                (None, Some(_)) => missed = Some(index),
                _ => {}
            }
        }
        if !copied {
            // WebGL copies between GL buffers of one type only: an index-only and a data-only
            // buffer meet on the host.
            self.record_slow_path(SlowPath::HostCopyReadBack);
            let mut bytes = vec![0; size as usize];
            let read = Source::Buffer {
                buffer: src.clone(),
                offset: src_offset,
                size,
            };
            if read_back_now_held(self, &read, &mut bytes).is_ok() {
                write_buffer_held(self, &dst, dst_offset, &bytes, WriteMode::Ordered);
            }
            return;
        }
        if dst.index.is_some() {
            let to = dst_offset..dst_offset + size;
            // Only buffers holding indices are tracked.
            let held =
                src.index.is_none() || src.maxima.borrow().any(src_offset..src_offset + size);
            match held {
                true => dst.maxima.borrow_mut().unknown(to),
                false => dst.maxima.borrow_mut().cleared(to),
            }
        }
        if let Some(index) = missed {
            mark_stale(self, &dst, index, dst_offset, dst_offset + size);
        }
    }

    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        match (region_of(self, src), region_of(self, dst)) {
            (Some(src), Some(dst)) => copy_texture_held(self, &src, &dst, extent),
            _ => invalid(self, GpuError::GONE.into()),
        }
    }

    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        match (place_of(self, src), region_of(self, dst)) {
            (Some(src), Some(dst)) => copy_buffer_to_texture_held(self, &src, &dst, extent),
            _ => invalid(self, GpuError::GONE.into()),
        }
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        match (region_of(self, src), place_of(self, dst)) {
            (Some(src), Some(dst)) => copy_texture_to_buffer_held(self, &src, &dst, extent),
            _ => invalid(self, GpuError::GONE.into()),
        }
    }

    /// WebGL 2 has no buffer clear: one block of the pattern is written from the host into every GL
    /// buffer of `buffer`, then doubled by copies within each ([`SlowPath::PatternClearCopied`]).
    fn clear_buffer(&mut self, buffer: &Buffer, offset: u64, size: u64, pattern: &[u8]) {
        let Some(buffer) = held(&self.objects.buffers, buffer.0) else {
            return invalid(self, GpuError::GONE.into());
        };
        if size == 0 {
            return;
        }
        if !matches!(pattern.len(), 1 | 2 | 4 | 8 | 16)
            || !size.is_multiple_of(pattern.len() as u64)
            || offset + size > buffer.size
        {
            invalid(
                self,
                format!(
                    "a buffer clear of {size} bytes from {offset} with a {}-byte pattern",
                    pattern.len()
                ),
            );
            return;
        }
        self.record_slow_path(SlowPath::PatternClearCopied);
        // A multiple of every pattern length (1 to 16 bytes); `size` is whole patterns.
        const PATTERN_SPAN: usize = 256;
        let mut block = [0u8; PATTERN_SPAN];
        for (b, p) in block.iter_mut().zip(pattern.iter().cycle()) {
            *b = *p;
        }
        let first = size.min(PATTERN_SPAN as u64);
        write_buffer_held(
            self,
            &buffer,
            offset,
            &block[..first as usize],
            WriteMode::Ordered,
        );
        let gl = &self.gl;
        for raw in [buffer.data, buffer.index].into_iter().flatten() {
            let mut done = first;
            gl.bind_buffer(gles::COPY_READ_BUFFER, raw);
            gl.bind_buffer(gles::COPY_WRITE_BUFFER, raw);
            while done < size {
                let n = done.min(size - done);
                gl.copy_buffer_sub_data(
                    gles::COPY_READ_BUFFER,
                    gles::COPY_WRITE_BUFFER,
                    offset as i64,
                    (offset + done) as i64,
                    n as i64,
                );
                done += n;
            }
        }
        if buffer.index.is_some() {
            let range = offset..offset + size;
            match pattern.contains(&0xff) {
                true => buffer.maxima.borrow_mut().unknown(range),
                false => buffer.maxima.borrow_mut().cleared(range),
            }
        }
    }

    /// By `blitFramebuffer`, which averages in the source's stored format; into the destination, or its
    /// twin when the destination is stored as the source's sRGB twin.
    fn resolve(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        format: Format,
    ) {
        let (Some(src), Some(dst)) = (region_of(self, src), region_of(self, dst)) else {
            return invalid(self, GpuError::GONE.into());
        };
        let (s, d) = (&src.texture, &dst.texture);
        if s.desc.samples == 1 || d.desc.samples != 1 {
            return invalid(self, "a resolve from one sample or into several".into());
        }
        if format != s.desc.format {
            // GL resolves only between equal formats, averaging the stored values.
            self.record_slow_path(SlowPath::ResolveInStoredFormat);
        }
        let target = if d.desc.format == s.desc.format {
            d.clone()
        } else {
            match twin_of(self, d, s.desc.format) {
                Ok(t) => t,
                Err(_) => return,
            }
        };
        fresh(self, &target);
        written(self, &target);
        blit_resolve(self, s, &target, dst.mip, dst.layer);
    }

    fn submit(&mut self) -> Self::Fence {
        // GL's errors are read where the back end waits anyway (`gl_errors`): in the browser
        // `getError` waits for the GPU process.
        let sync = {
            let sync = self.gl.fence_sync(gles::SYNC_GPU_COMMANDS_COMPLETE, 0);
            self.gl.flush();
            sync
        };
        if sync.is_none() {
            self.last_log = "no sync object: the context is lost".into();
            fail(self, LossReason::Reset);
        }
        Fence {
            done: Rc::new(Cell::new(sync.is_none())),
            sync,
        }
    }

    fn fence_done(&mut self, fence: &Self::Fence) -> bool {
        if !fence.done.get()
            && let Some(sync) = &fence.sync
        {
            fence.done.set(signaled(&self.gl, sync));
        }
        fence.done.get()
    }

    fn wait_fence(&mut self, fence: &Self::Fence) {
        let Some(sync) = fence.sync.as_ref().filter(|_| !fence.done.get()) else {
            return;
        };
        #[cfg(target_arch = "wasm32")]
        {
            let _ = sync;
            self.gl.finish();
        }
        #[cfg(not(target_arch = "wasm32"))]
        while self
            .gl
            .client_wait_sync(sync, gles::SYNC_FLUSH_COMMANDS_BIT, 1_000_000_000)
            == gles::TIMEOUT_EXPIRED
        {}
        fence.done.set(true);
        gl_errors(self);
    }

    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Self::Readback, GpuError> {
        let src = source_of(self, src).ok_or(GpuError::Invalid(GpuError::GONE))?;
        let r = read_back_held(self, &src)?;
        let number = id(self);
        self.objects.readbacks.insert(number, r);
        Ok(Readback(number))
    }

    fn read_queries(
        &mut self,
        set: &QuerySet,
        first: u32,
        count: u32,
    ) -> Result<Self::Readback, GpuError> {
        let set = held(&self.objects.query_sets, set.0).ok_or(GpuError::Invalid(GpuError::GONE))?;
        let slots = set.slots.borrow();
        let queries = slots
            .get(first as usize..(first + count) as usize)
            .ok_or(GpuError::Invalid("read_queries past the set's end"))?
            .to_vec();
        self.gl.flush();
        let made = Handle(Rc::new(core::cell::RefCell::new(ReadbackObj {
            pbo: None,
            queries,
            timestamps: set.kind == QueryKind::Timestamp,
            size: count as usize * 8,
            sync: None,
            waited: false,
            repack: None,
            host: None,
        })));
        drop(slots);
        let number = id(self);
        self.objects.readbacks.insert(number, made);
        Ok(Readback(number))
    }

    /// Ready once the copy's sync object is signalled; a read back made without one is ready at once and
    /// reads nothing, the device having been lost when it was made.
    fn read_state(&mut self, readback: &Self::Readback) -> ReadState {
        let Some(readback) = held(&self.objects.readbacks, readback.0) else {
            invalid(self, GpuError::GONE.into());
            return ReadState::Ready;
        };
        let r = readback.borrow();
        if r.pbo.is_none() {
            return if queries_available(self, &r.queries) {
                ReadState::Ready
            } else {
                ReadState::Pending
            };
        }
        match &r.sync {
            Some(sync) if !r.waited && !signaled(&self.gl, sync) => ReadState::Pending,
            _ => ReadState::Ready,
        }
    }

    fn wait_read(&mut self, readback: &Self::Readback) {
        let Some(readback) = held(&self.objects.readbacks, readback.0) else {
            return invalid(self, GpuError::GONE.into());
        };
        if readback.borrow().pbo.is_none() {
            // Natively the results arrive while the device thread spins. WebGL makes them available only
            // between the browser's tasks: reading a result waits for it, the OpenGL ES crate's WebGL
            // implementation giving the browser turns until it is there (the page entered the module
            // through `WebAssembly.promising`). A host that gives no turn (no JSPI) leaves a result
            // unavailable, and the device is lost rather than spin for ever.
            #[cfg(target_arch = "wasm32")]
            {
                for query in readback.borrow().queries.iter().flatten() {
                    platform::query_value(&self.gl, **query, gles::QUERY_RESULT);
                }
                if !queries_available(self, &readback.borrow().queries) {
                    if self.lost.is_none() {
                        self.last_log = NO_TURN.into();
                    }
                    fail(self, LossReason::Error);
                }
            }
            while self.status() == DeviceStatus::Ok
                && !queries_available(self, &readback.borrow().queries)
            {}
            return;
        }
        let sync = readback.borrow().sync.clone();
        if let Some(sync) = sync {
            let fence = Fence {
                sync: Some(sync),
                done: Rc::new(Cell::new(false)),
            };
            self.wait_fence(&fence);
            readback.borrow_mut().waited = true;
        }
    }

    fn finish_read(&mut self, readback: Self::Readback, dst: &mut [u8]) {
        let Some(readback) = self.objects.readbacks.remove(&readback.0) else {
            return invalid(self, GpuError::GONE.into());
        };
        if let Some(kept) = readback.borrow_mut().host.take() {
            let n = kept.len().min(dst.len());
            dst[..n].copy_from_slice(&kept[..n]);
            return;
        }
        let Some(pbo) = readback.borrow().pbo else {
            let r = readback.borrow();
            for (q, out) in r.queries.iter().zip(dst.as_chunks_mut::<8>().0) {
                let value = q.as_deref().map_or(0, |&q| {
                    if r.timestamps {
                        let mut value = [0];
                        let _ =
                            self.gl
                                .get_query_objectui64v_ext(q, gles::QUERY_RESULT, &mut value);
                        value[0]
                    } else {
                        u64::from(platform::query_value(&self.gl, q, gles::QUERY_RESULT))
                    }
                });
                *out = value.to_le_bytes();
            }
            drop(r);
            for q in readback.borrow_mut().queries.drain(..).flatten() {
                if let Ok(q) = Rc::try_unwrap(q) {
                    self.gl.delete_queries(&[q]);
                }
            }
            return;
        };
        let r = readback.borrow();
        let mut wide = Vec::new();
        let read = {
            self.gl.bind_buffer(gles::COPY_READ_BUFFER, pbo);
            let read = if let Some(read) = r.repack {
                wide.resize(r.size, 0);
                let ok =
                    platform::get_buffer_sub_data(&self.gl, gles::COPY_READ_BUFFER, 0, &mut wide);
                read.repack(&wide, dst);
                ok
            } else {
                let n = r.size.min(dst.len());
                platform::get_buffer_sub_data(&self.gl, gles::COPY_READ_BUFFER, 0, &mut dst[..n])
            };
            if let Some(sync) = &r.sync {
                self.gl.delete_sync(sync);
            }
            self.gl.delete_buffers(&[pbo]);
            read
        };
        if !read {
            read_failed(self);
        }
    }

    /// A WebGL context draws into one canvas, the one the back end was made on, whatever window the
    /// host names; so a back end has one surface at a time. Natively there is none: the OpenGL ES crate
    /// makes no EGL window surface yet, so every window is refused.
    fn create_surface(&mut self, window: u32) -> Result<Surface, GpuError> {
        if self.surface.is_some() {
            return Err(GpuError::Invalid(
                "webgl: a back end has one surface, its context's canvas",
            ));
        }
        if !self.presents {
            self.last_log = format!("webgl: attaching to window {window}: no window surface");
            return Err(GpuError::Invalid("webgl: the host has no such window"));
        }
        let id = id(self);
        self.surface = Some(SurfaceState { id, frame: None });
        Ok(Surface(id))
    }

    /// `vsync` changes nothing: the browser shows the canvas when it composites the page, at the
    /// display's rate, and presenting never waits.
    fn configure_surface(
        &mut self,
        surface: &Surface,
        format: Format,
        width: u32,
        height: u32,
        _vsync: bool,
    ) -> Result<(), GpuError> {
        if self.surface.as_ref().map(|s| s.id) != Some(surface.0) {
            return Err(GpuError::Invalid("webgl: the surface was destroyed"));
        }
        // BGRA textures are stored as RGBA, so both blit into the RGBA drawing buffer as they are.
        if !matches!(format, Format::Rgba8Unorm | Format::Bgra8Unorm) {
            return Err(GpuError::Invalid(
                "webgl: surfaces take rgba8unorm and bgra8unorm only",
            ));
        }
        let texture = make_texture(
            self,
            &TextureDesc {
                dimension: TextureDimension::D2,
                format,
                width,
                height,
                depth_or_layers: 1,
                mip_levels: 1,
                samples: 1,
                usage: TextureUsage::RENDER | TextureUsage::COPY_SRC | TextureUsage::PRESENT,
                cube: false,
                reinterpret: false,
            },
        )?;
        let view = make_view(
            self,
            &texture,
            &TextureViewDesc {
                format,
                dimension: ViewDimension::D2,
                base_mip: 0,
                mip_count: 1,
                base_layer: 0,
                layer_count: 1,
                stencil: false,
            },
        )?;
        // The frame's view is what `current_texture` hands out, so its number names it.
        self.objects.views.insert(view.id, view.clone());
        let old = self
            .surface
            .as_mut()
            .and_then(|s| s.frame.replace((texture, view)));
        if let Some((t, v)) = old {
            self.objects.views.remove(&v.id);
            destroy_held(self, Held::Texture(t));
        }
        Ok(())
    }

    fn current_texture(&mut self, surface: &Surface) -> Result<TextureView, GpuError> {
        match &self.surface {
            Some(s) if s.id == surface.0 => match &s.frame {
                Some((_, view)) => Ok(TextureView(view.id)),
                None => Err(GpuError::Invalid("webgl: the surface is not configured")),
            },
            _ => Err(GpuError::Invalid("webgl: the surface was destroyed")),
        }
    }

    /// Copies the frame into the drawing buffer (natively the window's back buffer, then swapped),
    /// upside down since GL's row 0 is the bottom of the window, stretched to the drawing buffer's
    /// size where the host's window has another; the browser shows it when the device thread returns
    /// to its event loop.
    fn present(&mut self, surface: &Surface) -> Result<(), GpuError> {
        if let Some(reason) = self.lost {
            return Err(match reason {
                LossReason::OutOfMemory => GpuError::OutOfMemory,
                _ => GpuError::DeviceLost,
            });
        }
        let texture = match &self.surface {
            Some(s) if s.id == surface.0 => match &s.frame {
                Some((t, _)) => t.clone(),
                None => return Err(GpuError::Invalid("webgl: the surface is not configured")),
            },
            _ => return Err(GpuError::Invalid("webgl: the surface was destroyed")),
        };
        read_framebuffer(self, &texture, 0, 0);
        open_writes(self);
        let (w, h) = (texture.desc.width as i32, texture.desc.height as i32);
        // The drawing buffer takes the configured size, as a canvas's does.
        let (dw, dh) = (w, h);
        self.gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, 0);
        self.applied.draw_framebuffer = None;
        self.gl.blit_framebuffer(
            0,
            0,
            w,
            h,
            0,
            dh,
            dw,
            0,
            gles::COLOR_BUFFER_BIT,
            gles::NEAREST,
        );
        self.presented += 1;
        if self.presented.is_power_of_two() {
            platform::report(&self.frame_report());
        }
        Ok(())
    }

    /// Answers inside the call: `readPixels` and `getBufferSubData` wait for the GPU themselves.
    fn read_back_now(
        &mut self,
        src: &ReadSource<'_, Self>,
        dst: &mut [u8],
    ) -> Result<(), GpuError> {
        let src = source_of(self, src).ok_or(GpuError::Invalid(GpuError::GONE))?;
        read_back_now_held(self, &src, dst)
    }

    /// Clears each rectangle by `clearBuffer*` under the scissor, in one pass that loads the view.
    fn clear_rects(
        &mut self,
        view: &TextureView,
        format: Format,
        value: ClearValue,
        rects: &[ScissorRect],
    ) -> Result<(), GpuError> {
        let id = view;
        let view = held(&self.objects.views, view.0).ok_or(GpuError::Invalid(GpuError::GONE))?;
        if rects.is_empty() {
            return Ok(());
        }
        fn keep<V>(has: bool) -> Option<AspectOps<V>> {
            has.then_some(AspectOps {
                load: LoadOp::Load,
                store: StoreOp::Store,
                read_only: false,
            })
        }
        let mut pass = PassDesc {
            colors: Default::default(),
            depth: None,
            empty: None,
            occlusion: None,
        };
        let kind = FORMAT_FACTS[format as usize].kind;
        let (depth, stencil) = match value {
            ClearValue::Color(_) => {
                pass.colors[0] = Some(ColorAttachment {
                    view: id,
                    depth_slice: 0,
                    resolve: None,
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                });
                (None, None)
            }
            ClearValue::DepthStencil { depth, stencil } => {
                let has_depth = matches!(kind, FormatKind::Depth | FormatKind::DepthStencil);
                let has_stencil = matches!(kind, FormatKind::Stencil | FormatKind::DepthStencil);
                let (depth, stencil) =
                    (depth.filter(|_| has_depth), stencil.filter(|_| has_stencil));
                if depth.is_none() && stencil.is_none() {
                    return Ok(());
                }
                pass.depth = Some(DepthAttachment {
                    view: id,
                    depth: keep(has_depth),
                    stencil: keep(has_stencil),
                });
                (depth, stencil)
            }
        };
        self.begin_pass(&pass);
        if self.pass.is_none() {
            return Err(GpuError::DeviceLost);
        }
        // Clears obey the scissor and the write masks.
        open_writes(self);
        let (gl, v) = (&self.gl, &mut self.values);
        gl.enable(gles::SCISSOR_TEST);
        v.scissor_test = true;
        for r in rects {
            let rect = gl_rect(r, self.pass.as_ref().map_or(NO_PASS, |p| p.size));
            if v.scissor != rect {
                gl.scissor(rect[0], rect[1], rect[2], rect[3]);
                v.scissor = rect;
            }
            match (value, kind) {
                (ClearValue::Color(c), FormatKind::Uint) => gl.clear_bufferuiv(gles::COLOR, 0, &c),
                (ClearValue::Color(c), FormatKind::Sint) => {
                    gl.clear_bufferiv(gles::COLOR, 0, &c.map(|x| x as i32))
                }
                (ClearValue::Color(c), _) => gl.clear_bufferfv(
                    gles::COLOR,
                    0,
                    &format::clear_value(view.texture.desc.format, c).map(f),
                ),
                (ClearValue::DepthStencil { .. }, _) => match (depth, stencil) {
                    (Some(d), Some(s)) => gl.clear_bufferfi(gles::DEPTH_STENCIL, 0, f(d), s.into()),
                    (Some(d), None) => gl.clear_bufferfv(gles::DEPTH, 0, &[f(d)]),
                    (None, Some(s)) => gl.clear_bufferiv(gles::STENCIL, 0, &[i32::from(s)]),
                    (None, None) => {}
                },
            }
        }
        self.end_pass();
        Ok(())
    }

    /// Fills the levels by `generateMipmap` where GLES allows it (a format that renders and filters,
    /// every layer, the texels' values in the texture's own format), else by the interface's draws.
    fn generate_mips(
        &mut self,
        texture: &Texture,
        desc: &TextureDesc,
        values: Format,
        mips: Range<u32>,
        layers: Range<u32>,
    ) -> Result<(), GpuError> {
        let texture =
            held(&self.objects.textures, texture.0).ok_or(GpuError::Invalid(GpuError::GONE))?;
        let levels = mips.start.max(1)..mips.end.min(desc.mip_levels);
        let every_layer = layers.start == 0 && layers.end >= desc.depth_or_layers;
        let native = FormatUse::RENDER | FormatUse::FILTER;
        let raw = texture.tex();
        let Some(raw) = raw.filter(|_| {
            values == desc.format
                && every_layer
                && texture.host.is_none()
                && self.caps.formats.uses[values as usize].contains(native)
        }) else {
            return Helpers::generate_mips_by_draws(
                self,
                &Texture(texture.id),
                desc,
                values,
                mips,
                layers,
            );
        };
        if levels.is_empty() {
            return Ok(());
        }
        fresh(self, &texture);
        bind_scratch(self, texture.target, raw);
        let range = (levels.start - 1, levels.end - 1);
        let gl = &self.gl;
        if texture.levels.get() != range {
            gl.tex_parameteri(texture.target, gles::TEXTURE_BASE_LEVEL, range.0 as i32);
            gl.tex_parameteri(texture.target, gles::TEXTURE_MAX_LEVEL, range.1 as i32);
            texture.levels.set(range);
        }
        gl.generate_mipmap(texture.target);
        written(self, &texture);
        Ok(())
    }

    /// Draws `TRIANGLE_FAN` and `LINE_LOOP` as they are. GL's tables give the vertex the raster block's
    /// convention names, which is the one `provoking` names when the front's block follows its API, as
    /// the converted list of the default assumes too. Guest indices go into the index ring once; GL
    /// restarts at the largest value always, so where the call does not restart and one is there they
    /// go widened to 32 bits ([`SlowPath::RestartRewrite`]).
    fn draw_fan_or_loop(
        &mut self,
        call: &DrawCall<'_, Self>,
        kind: FanOrLoop,
        source: IndexSource<'_>,
        _provoking: ProvokingVertex,
        first_instance: u32,
        instances: u32,
    ) -> Result<(), GpuError> {
        let topology = match kind {
            FanOrLoop::TriangleFan => Topology::TriangleFan,
            FanOrLoop::LineLoop => Topology::LineLoop,
        };
        match source {
            IndexSource::Sequential { first, count } => self.draw(&DrawCall {
                topology,
                index: None,
                range: DrawRange::Vertices {
                    first,
                    count,
                    first_instance,
                    instances,
                },
                ..*call
            }),
            IndexSource::Guest {
                indices,
                format,
                base_vertex,
            } => {
                let width = INDEX_FORMATS[format as usize].size as usize;
                let count = indices.len() / width;
                if count == 0 {
                    return Ok(());
                }
                let widened;
                let mut bytes = &indices[..count * width];
                let (mut format, mut base_vertex) = (format, base_vertex);
                if !call.restart {
                    match restart::rewrite(bytes, format) {
                        restart::Rewrite::Unneeded => {}
                        restart::Rewrite::Widened(w, shift) => {
                            self.record_slow_path(SlowPath::RestartRewrite);
                            widened = w;
                            bytes = &widened;
                            format = IndexFormat::U32;
                            base_vertex = i32::try_from(i64::from(base_vertex) + i64::from(shift))
                                .map_err(|_| {
                                    unbuilt(
                                        self,
                                        "a fan or loop without restart whose lowered 32-bit \
                                         indices need a base vertex past 2^31 - 1",
                                    )
                                })?;
                        }
                        restart::Rewrite::Unreachable => {
                            return Err(unbuilt(
                                self,
                                "a fan or loop whose 32-bit indices hold both 0 and 2^32 - 1 \
                                 without restart",
                            ));
                        }
                    }
                }
                let mut offset = [0];
                let ring = self.write_buffer_ring(
                    RingKind::Indices,
                    &[bytes],
                    INDEX_FORMATS[format as usize].size,
                    &mut offset,
                )?;
                self.draw(&DrawCall {
                    topology,
                    index: Some(IndexBinding {
                        buffer: &ring,
                        offset: offset[0],
                        format,
                    }),
                    range: DrawRange::Indexed {
                        first: 0,
                        count: count as u32,
                        base_vertex,
                        first_instance,
                        instances,
                    },
                    ..*call
                });
            }
        }
        Ok(())
    }

    /// Transform feedback: the program linked again with its location outputs as varyings
    /// (interleaved, or separate with more than one target), drawn with `RASTERIZER_DISCARD` unless
    /// it rasterises. GLES 3.0 captures only lists without indices; other draws are captured as
    /// points in list order, one draw per run of consecutive vertices, and drawn again for the picture
    /// (`CaptureExpanded`). A target holding indices too gets its index GL buffer refreshed before an
    /// index read (`IndexTwinReadBack`).
    fn capture_vertices(
        &mut self,
        call: &DrawCall<'_, Self>,
        targets: CaptureTargets<'_, Self>,
        rasterize: bool,
    ) -> Result<(), GpuError> {
        if self.lost.is_some() {
            return Err(GpuError::DeviceLost);
        }
        let max = self.caps.limits.capture_buffers as usize;
        if targets.is_empty() || targets.len() > max || rasterize != self.pass.is_some() {
            return Err(GpuError::Invalid(
                "capture_vertices with no targets, more than capture_buffers, or rasterize not \
                 saying whether a pass is open",
            ));
        }
        let Some(drawn) = drawn_of(self, call) else {
            return Err(GpuError::Invalid(GpuError::GONE));
        };
        let mut held_targets = Vec::with_capacity(targets.len());
        for t in targets {
            held_targets.push(BufferRange {
                buffer: held(&self.objects.buffers, t.buffer.0)
                    .ok_or(GpuError::Invalid(GpuError::GONE))?,
                offset: t.offset,
                size: t.size,
            });
        }
        let targets = &held_targets[..];
        let outputs = &drawn.program.stages.0.outputs;
        let separate = targets.len() > 1;
        if outputs.is_empty()
            || outputs.iter().any(|(_, bytes)| *bytes == 0)
            || (separate && outputs.len() != targets.len())
        {
            return Err(GpuError::Invalid(
                "capture_vertices of a program with no location outputs, outputs that are not \
                 32-bit, or other than one target per output",
            ));
        }
        let native = match (call.topology, &call.range) {
            (Topology::PointList, DrawRange::Vertices { .. }) => Some((gles::POINTS, 1)),
            (Topology::LineList, DrawRange::Vertices { .. }) => Some((gles::LINES, 2)),
            (Topology::TriangleList, DrawRange::Vertices { .. }) => Some((gles::TRIANGLES, 3)),
            _ => None,
        };
        // The vertex numbers in list order, for draws GL cannot capture as they are.
        let mut list = Vec::new();
        let (first_instance, instances, vertices_per_instance) = match (native, &call.range) {
            (
                Some((_, per_primitive)),
                DrawRange::Vertices {
                    count,
                    first_instance,
                    instances,
                    ..
                },
            ) => (*first_instance, *instances, count - count % per_primitive),
            (
                None,
                DrawRange::Vertices {
                    first,
                    count,
                    first_instance,
                    instances,
                },
            ) => {
                let numbers = (*first..first.saturating_add(*count)).map(Some);
                capture::list_order(
                    call.topology,
                    drawn.raster.0.1.provoking(),
                    numbers,
                    &mut list,
                );
                (*first_instance, *instances, list.len() as u32)
            }
            (
                None,
                DrawRange::Indexed {
                    first,
                    count,
                    base_vertex,
                    first_instance,
                    instances,
                },
            ) => {
                let Some(index) = &drawn.index else {
                    return Err(GpuError::Invalid("an indexed draw with no index buffer"));
                };
                let size = INDEX_FORMATS[index.format as usize].size;
                let raw = read_indices(self, index, *first, *count)?;
                // Only strips that restart restart: GL's restart value is a vertex elsewhere.
                let restart = call
                    .restart
                    .then(|| INDEX_FORMATS[index.format as usize].restart_value);
                let mut negative = false;
                let numbers = raw.chunks_exact(size as usize).map(|c| {
                    let mut word = [0; 4];
                    word[..c.len()].copy_from_slice(c);
                    let i = u32::from_le_bytes(word);
                    if Some(i) == restart {
                        return None;
                    }
                    let v = i64::from(i) + i64::from(*base_vertex);
                    negative |= !(0..=i64::from(i32::MAX)).contains(&v);
                    Some(v.clamp(0, i64::from(i32::MAX)) as u32)
                });
                capture::list_order(
                    call.topology,
                    drawn.raster.0.1.provoking(),
                    numbers,
                    &mut list,
                );
                if negative {
                    return Err(unbuilt(
                        self,
                        "capture_vertices reaching below vertex 0 by a negative base vertex, or past \
                         vertex 2^31 - 1",
                    ));
                }
                (*first_instance, *instances, list.len() as u32)
            }
            _ => return Err(GpuError::Absent(Features::INDIRECT)),
        };
        let vertices = u64::from(vertices_per_instance) * u64::from(instances);
        let stride = |n: usize| -> u64 {
            if separate {
                u64::from(outputs[n].1)
            } else {
                outputs.iter().map(|(_, b)| u64::from(*b)).sum()
            }
        };
        if targets.iter().enumerate().any(|(n, t)| {
            t.buffer.data.is_none()
                || t.offset % 4 != 0
                || vertices * stride(n) > t.size
                || t.offset + t.size > t.buffer.size
        }) {
            return Err(GpuError::Invalid(
                "capture_vertices into a buffer without CAPTURE, at an offset not a multiple of 4, \
                 or into a range smaller than the vertices the draw makes",
            ));
        }
        let discard = !rasterize || native.is_none();
        let kind = shader::Capture {
            separate,
            rasterize: !discard,
        };
        let slot = &drawn.program.captures[kind.slot()];
        let capture = match slot.get() {
            Some(c) => c.clone(),
            None => {
                let id = id(self);
                let c = shader::relink(
                    &self.gl,
                    id,
                    &drawn.program,
                    Some(kind),
                    0,
                    self.empty_fragment,
                )?;
                slot.get_or_init(|| Rc::new(c)).clone()
            }
        };
        let capture = Handle(capture);
        if linked(self, &capture).is_none() {
            return Err(GpuError::DeviceLost);
        }
        for t in targets {
            refresh(self, &t.buffer, Some(false));
        }
        let gl = &self.gl;
        if discard {
            // Outside a pass nothing is drawn; the default framebuffer is always complete.
            if !rasterize && self.applied.draw_framebuffer.is_some() {
                gl.bind_framebuffer(gles::DRAW_FRAMEBUFFER, 0);
                self.applied.draw_framebuffer = None;
            }
            gl.enable(gles::RASTERIZER_DISCARD);
        }
        // WebGL refuses a draw while a capture target is bound anywhere else.
        for target in [
            gles::ARRAY_BUFFER,
            gles::COPY_READ_BUFFER,
            gles::COPY_WRITE_BUFFER,
        ] {
            gl.bind_buffer(target, 0);
        }
        for (n, t) in targets.iter().enumerate() {
            gl.bind_buffer_range(
                gles::TRANSFORM_FEEDBACK_BUFFER,
                n as u32,
                t.buffer.data.unwrap_or(0),
                t.offset as i64,
                t.size as i64,
            );
        }
        if self.applied.program != capture.id {
            gl.use_program(capture.raw);
            self.applied.program = capture.id;
            self.applied.sets = [Id::MAX; MAX_BINDING_SETS];
        }
        gl.begin_transform_feedback(native.map_or(gles::POINTS, |(mode, _)| mode));
        // The capture program in the draw's place, with no index buffer.
        let captured = Drawn {
            program: capture.clone(),
            index: None,
            ..drawn.clone()
        };
        if let DrawRange::Vertices {
            first,
            count,
            first_instance,
            instances,
        } = call.range
            && native.is_some()
        {
            draw_from(
                self,
                &DrawCall {
                    index: None,
                    range: DrawRange::Vertices {
                        first,
                        count,
                        first_instance,
                        instances,
                    },
                    ..*call
                },
                &captured,
                0,
            );
        } else {
            self.record_slow_path(SlowPath::CaptureExpanded);
            for later in 0..instances {
                for (first, count) in capture::runs(&list) {
                    let run = DrawCall {
                        index: None,
                        topology: Topology::PointList,
                        range: DrawRange::Vertices {
                            first,
                            count,
                            first_instance,
                            instances: 1,
                        },
                        ..*call
                    };
                    draw_from(self, &run, &captured, later);
                }
            }
        }
        let gl = &self.gl;
        gl.end_transform_feedback();
        if discard {
            gl.disable(gles::RASTERIZER_DISCARD);
        }
        for n in 0..targets.len() {
            gl.bind_buffer_base(gles::TRANSFORM_FEEDBACK_BUFFER, n as u32, 0);
        }
        gl.bind_buffer(gles::TRANSFORM_FEEDBACK_BUFFER, 0);
        for (n, t) in targets.iter().enumerate() {
            if t.buffer.index.is_some() {
                let end = t.offset + vertices * stride(n);
                t.buffer.maxima.borrow_mut().unknown(t.offset..end);
                mark_stale(self, &t.buffer, true, t.offset, end);
            }
        }
        if rasterize && native.is_none() {
            self.draw(call);
        }
        match self.lost {
            Some(_) => Err(GpuError::DeviceLost),
            None => Ok(()),
        }
    }

    /// `queryCounterEXT` with `TIMESTAMP_EXT`, which GL writes between any two commands.
    fn write_timestamp(&mut self, set: &QuerySet, index: u32) -> Result<(), GpuError> {
        let set = held(&self.objects.query_sets, set.0).ok_or(GpuError::Invalid(GpuError::GONE))?;
        if !self.caps.features.contains(Features::TIMESTAMPS) {
            return Err(GpuError::Absent(Features::TIMESTAMPS));
        }
        if set.kind != QueryKind::Timestamp || self.pass.is_some() {
            return Err(GpuError::Invalid(
                "write_timestamp into a set of another kind, or inside a pass",
            ));
        }
        let q = query_slot(self, &set, index).ok_or(GpuError::DeviceLost)?;
        self.gl.query_counter_ext(q, gles::TIMESTAMP_EXT);
        Ok(())
    }

    /// `GPU_DISJOINT_EXT`, which reading clears.
    fn timestamps_disjoint(&mut self) -> Result<bool, GpuError> {
        if !self.caps.features.contains(Features::TIMESTAMPS) {
            return Err(GpuError::Absent(Features::TIMESTAMPS));
        }
        let mut disjoint = [false];
        self.gl.get_booleanv(gles::GPU_DISJOINT_EXT, &mut disjoint);
        Ok(disjoint[0])
    }

    fn dispatch(&mut self, call: &DispatchCall<'_, Self>) -> Result<(), GpuError> {
        let _ = call;
        Err(GpuError::Absent(Features::COMPUTE))
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

/// Writes `data` at `offset` of `buffer`, as `write_buffer` does once the handle is looked up.
fn write_buffer_held<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    buffer: &BufferObj,
    offset: u64,
    data: &[u8],
    mode: WriteMode,
) {
    if offset + data.len() as u64 > buffer.size {
        return invalid(
            gpu,
            format!(
                "a write of {} bytes at {offset} past a buffer of {}",
                data.len(),
                buffer.size
            ),
        );
    }
    if buffer.index.is_some() {
        buffer.maxima.borrow_mut().wrote(offset, data);
    }
    let gl = &gpu.gl;
    for raw in [buffer.data, buffer.index].into_iter().flatten() {
        gl.bind_buffer(gles::COPY_WRITE_BUFFER, raw);
        if mode == WriteMode::Discard {
            // Orphan at the buffer's own size, so later writes above this part still fit.
            gl.buffer_data(
                gles::COPY_WRITE_BUFFER,
                buffer.size as i64,
                None,
                buffer.usage,
            );
        }
        gl.buffer_sub_data(gles::COPY_WRITE_BUFFER, offset as i64, data);
    }
}

fn make_buffer<O: OpenGlEs>(gpu: &mut Gl<O>, desc: &BufferDesc) -> Result<BufferRef, GpuError> {
    let size = i32::try_from(desc.size).map_err(|_| GpuError::OutOfMemory)?;
    let usage = match desc.intent {
        MapIntent::Rarely => gles::STATIC_DRAW,
        MapIntent::Often => gles::DYNAMIC_DRAW,
    };
    let copies = BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::READ_BACK;
    let indices = desc.usage.contains(BufferUsage::INDEX);
    let other = !(desc.usage - BufferUsage::INDEX - copies).is_empty() || !indices;
    if indices && other {
        gpu.record_slow_path(SlowPath::IndexBufferTwin);
    }
    let gl = &gpu.gl;
    let make = |target| {
        let b = platform::one(|n| gl.gen_buffers(n)).ok_or(GpuError::OutOfMemory)?;
        gl.bind_buffer(target, b);
        gl.buffer_data(target, size as i64, None, usage);
        Ok::<_, GpuError>(b)
    };
    let data = other.then(|| make(gles::ARRAY_BUFFER)).transpose()?;
    let index = indices
        .then(|| make(gles::ELEMENT_ARRAY_BUFFER))
        .transpose()?;
    if indices {
        gpu.applied.index = 0;
    }
    Ok(Handle(Rc::new(BufferObj {
        id: id(gpu),
        size: desc.size,
        usage,
        data,
        index,
        stale: Cell::new(None),
        maxima: RefCell::default(),
    })))
}

fn make_texture<O: OpenGlEs>(gpu: &mut Gl<O>, desc: &TextureDesc) -> Result<TextureRef, GpuError> {
    if gpu.caps.formats.uses[desc.format as usize] == FormatUse::empty() {
        return Err(GpuError::Invalid(
            "webgl: the format has no capabilities here",
        ));
    }
    // Block-compressed formats are GL's compressed formats with their extension (not in 3D,
    // which GLES does not compress), else stored decoded.
    let (gl_format, decoded) = match host::compressed(desc.format) {
        Some((internal, ext, _))
            if gpu.extensions.contains(ext) && desc.dimension != TextureDimension::D3 =>
        {
            let class = format::Class::Norm { render: false };
            let (format, ty) = (0, 0);
            let f = format::GlFormat {
                internal,
                format,
                ty,
                class,
            };
            (Some(f), None)
        }
        _ => match host::decoded(desc.format) {
            Some(decoded) => {
                gpu.record_slow_path(SlowPath::BlockDecoded);
                (format::gl_format(decoded), Some(decoded))
            }
            None => (format::gl_format(desc.format), format::widened(desc.format)),
        },
    };
    let gl_format = gl_format.ok_or(GpuError::Invalid(
        "webgl: the format has no capabilities here",
    ))?;
    if desc.samples > 1 {
        return create_samples(gpu, desc, gl_format);
    }
    let target = match desc.own_view() {
        ViewDimension::D2 => gles::TEXTURE_2D,
        ViewDimension::D2Array => gles::TEXTURE_2D_ARRAY,
        ViewDimension::Cube => gles::TEXTURE_CUBE_MAP,
        ViewDimension::D3 => gles::TEXTURE_3D,
        ViewDimension::CubeArray => {
            return Err(GpuError::Invalid("webgl: WebGL 2 has no cube arrays"));
        }
    };
    let raw = platform::one(|n| gpu.gl.gen_textures(n)).ok_or(GpuError::OutOfMemory)?;
    bind_scratch(gpu, target, raw);
    let gl = &gpu.gl;
    let (w, h, d) = (
        desc.width as i32,
        desc.height as i32,
        desc.depth_or_layers as i32,
    );
    let levels = desc.mip_levels as i32;
    match target {
        gles::TEXTURE_2D | gles::TEXTURE_CUBE_MAP => {
            gl.tex_storage_2d(target, levels, gl_format.internal, w, h)
        }
        _ => gl.tex_storage_3d(target, levels, gl_format.internal, w, h, d),
    }
    gl.tex_parameteri(target, gles::TEXTURE_MAX_LEVEL, levels - 1);
    Ok(Handle(Rc::new(TextureObj {
        id: id(gpu),
        storage: Storage::Texture(raw),
        target,
        desc: *desc,
        gl: gl_format,
        read: format::read_as(
            desc.format,
            format::FormatExtensions::found(&gpu.extensions),
        ),
        levels: Cell::new((0, desc.mip_levels - 1)),
        newest: Rc::new(Cell::new(0)),
        side: 1,
        twin: std::cell::OnceCell::new(),
        own: None,
        stencil_mode: Cell::new(false),
        host: host::kept(desc.format).then(|| {
            gpu.record_slow_path(SlowPath::HostCopyKept);
            host::HostCopy::new(desc)
        }),
        decoded,
        drawn: Cell::new(0),
        minimum: Cell::new(0),
    })))
}

fn make_view<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    texture: &TextureRef,
    desc: &TextureViewDesc,
) -> Result<ViewRef, GpuError> {
    let t = &texture.desc;
    let three_d = t.dimension == TextureDimension::D3;
    let to_draw = desc.mip_count == 1 && (desc.layer_count == 1 || three_d);
    let whole = desc.dimension == t.own_view()
        && desc.base_layer == 0
        && desc.layer_count == if three_d { 1 } else { t.depth_or_layers };
    if !to_draw && !whole {
        return Err(GpuError::Absent(Features::TEXTURE_VIEWS));
    }
    if desc.stencil {
        if FORMAT_FACTS[t.format as usize].kind != FormatKind::DepthStencil {
            return Err(GpuError::Invalid(
                "webgl: a stencil view of a texture without stencil",
            ));
        }
        // Natively the texture gives its stencil when a draw binds the view; else a copy in
        // `r8uint` kept in step by draws.
        let texture = if gpu.extensions.contains("WEBGL_stencil_texturing") {
            texture.clone()
        } else {
            stencil_copy_of(gpu, texture)?
        };
        return Ok(Handle(Rc::new(ViewObj {
            id: id(gpu),
            texture,
            desc: *desc,
        })));
    }
    let texture = if desc.format == t.format {
        texture.clone()
    } else if FORMAT_FACTS[desc.format as usize].linear == FORMAT_FACTS[t.format as usize].linear {
        twin_of(gpu, texture, desc.format)?
    } else {
        return Err(GpuError::Absent(Features::FORMAT_REINTERPRET));
    };
    Ok(Handle(Rc::new(ViewObj {
        id: id(gpu),
        texture,
        desc: *desc,
    })))
}

fn write_texture_held<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    region: &Region,
    extent: Extent,
    data: &[u8],
    row_pitch: u32,
    rows_per_layer: u32,
    _mode: WriteMode,
) {
    fresh(gpu, &region.texture);
    written(gpu, &region.texture);
    let t = &region.texture;
    let (corner, size) = (
        corner(region),
        [extent.width, extent.height, extent.depth_or_layers],
    );
    let (pitch, per_layer) = (row_pitch as usize, rows_per_layer as usize);
    let snorm = format::signed_normalised(t.desc.format);
    let level = 1 << region.mip;
    if snorm && host::covers_level(&t.desc, region.mip, corner, size) {
        // Every texel of the level is the host's again.
        t.drawn.set(t.drawn.get() & !level);
        t.minimum.set(t.minimum.get() & !level);
    }
    let wrote_minimum = snorm
        && host::write_holds_minimum(&t.desc, region.mip, corner, size, data, pitch, per_layer);
    if wrote_minimum {
        // Texels drawn since are read back while a draw still reads them exactly.
        if t.minimum.get() & level == 0 {
            let _ = host_current(gpu, t, level);
        }
        t.minimum.set(t.minimum.get() | level);
    }
    if let Some(h) = &region.texture.host {
        h.write(&t.desc, region.mip, corner, size, data, pitch, per_layer);
        if t.minimum.get() & level != 0 && !wrote_minimum && t.drawn.get() & level == 0 {
            // The host copy holds every texel of the level; this write may have replaced its
            // last minimum.
            if !h.holds_minimum(t.desc.format, region.mip) {
                t.minimum.set(t.minimum.get() & !level);
            }
        }
        if host::decoded(region.texture.desc.format).is_some() {
            return upload_blocks(gpu, region, extent);
        }
    }
    if FORMAT_FACTS[region.texture.desc.format as usize].depth_stencil {
        return write_depth(
            gpu,
            region,
            extent,
            Texels::Bytes(data),
            row_pitch,
            rows_per_layer,
        );
    }
    let format = region.texture.desc.format;
    if format::converts_writes(format) {
        // Converted into tightly packed rows of the stored texels.
        gpu.record_slow_path(SlowPath::FormatConverted);
        let row = extent.width as usize * FORMAT_FACTS[format as usize].block.2 as usize;
        let (rows, layers) = (extent.height as usize, extent.depth_or_layers as usize);
        let mut stored = Vec::new();
        for layer in 0..layers {
            for r in 0..rows {
                let at = (layer * rows_per_layer as usize + r) * row_pitch as usize;
                format::stored_texels(format, &data[at..at + row], &mut stored);
            }
        }
        let texel = FORMAT_FACTS[t.decoded.unwrap_or(format) as usize].block.2;
        return upload(
            gpu,
            region,
            extent,
            Texels::Bytes(&stored),
            extent.width * texel,
            extent.height,
        );
    }
    upload(
        gpu,
        region,
        extent,
        Texels::Bytes(data),
        row_pitch,
        rows_per_layer,
    );
}

fn copy_texture_held<O: OpenGlEs>(gpu: &mut Gl<O>, src: &Region, dst: &Region, extent: Extent) {
    // Signed normalised texels drawn since their host copy was whole are drawn on the GPU where
    // they hold no smallest value.
    let level = 1 << src.mip;
    let by_draw = src.texture.drawn.get() & level != 0 && src.texture.minimum.get() & level == 0;
    if (src.texture.host.is_some() && !by_draw) || dst.texture.host.is_some() {
        return copy_host_kept(gpu, src, dst, extent);
    }
    if src.texture.desc.samples > 1 || dst.texture.desc.samples > 1 {
        return copy_samples(gpu, src, dst);
    }
    if within_one_subresource(src, dst, extent) {
        return copy_within(gpu, src, dst, extent);
    }
    let (from, to) = (src.texture.desc.format, dst.texture.desc.format);
    fresh(gpu, &src.texture);
    fresh(gpu, &dst.texture);
    written(gpu, &dst.texture);
    if from == to && FORMAT_FACTS[from as usize].depth_stencil {
        return blit_same(gpu, src, dst, extent);
    }
    if FORMAT_FACTS[from as usize].depth_stencil || FORMAT_FACTS[to as usize].depth_stencil {
        // A colour and a depth format of one typeless group.
        if copy_by_draw(gpu, src, dst, extent) {
            gpu.record_slow_path(if FORMAT_FACTS[to as usize].depth_stencil {
                SlowPath::DepthWrittenByDraw
            } else {
                SlowPath::CopyThroughBuffer
            });
        } else {
            unbuilt(gpu, "copies between these depth and colour formats");
        }
        return;
    }
    let renderable = |f| gpu.caps.formats.uses[f as usize].contains(FormatUse::RENDER);
    // copyTexSubImage may copy floats through arithmetic, which loses NaN payloads and denormals.
    let float = format::float_bits(from);
    if from == to && renderable(from) && float && gpu.float_blits {
        return blit_same(gpu, src, dst, extent);
    }
    // Formats `readPixels` reads are colour-renderable in GL, so copyTexSubImage copies them,
    // also where they are not reported as targets (B4G4R4A4).
    let attachable = renderable(from) || src.texture.read.is_some();
    if from != to || !attachable || float {
        let as_stored = src
            .texture
            .read
            .is_some_and(|r| r.texel_read() == r.texel_kept());
        let half = matches!(
            to,
            Format::R16Float | Format::Rg16Float | Format::Rgba16Float
        );
        if as_stored && FORMAT_FACTS[from as usize].block == FORMAT_FACTS[to as usize].block {
            copy_same_bytes(gpu, src, dst, extent);
        } else if half {
            // A draw writes half floats from 32-bit ones, which lose NaN payloads; the host
            // writes the bits.
            copy_host_kept(gpu, src, dst, extent);
        } else if copy_by_draw(gpu, src, dst, extent) {
            gpu.record_slow_path(SlowPath::CopyThroughBuffer);
        } else if float || src.texture.host.is_some() {
            // 32-bit floats are no targets without EXT_color_buffer_float.
            copy_host_kept(gpu, src, dst, extent);
        } else {
            unbuilt(gpu, "copies of these formats");
        }
        return;
    }
    let d = dst.texture.clone();
    let Some(raw) = d.tex() else {
        return;
    };
    for i in 0..extent.depth_or_layers {
        let layer = read_layer(src, i);
        read_framebuffer(gpu, &src.texture, src.mip, layer);
        bind_scratch(gpu, d.target, raw);
        let (x, y) = (dst.origin.x as i32, dst.origin.y as i32);
        let (sx, sy, w, h) = (
            src.origin.x as i32,
            src.origin.y as i32,
            extent.width as i32,
            extent.height as i32,
        );
        let level = dst.mip as i32;
        let gl = &gpu.gl;
        match d.target {
            gles::TEXTURE_2D => gl.copy_tex_sub_image_2d(d.target, level, x, y, sx, sy, w, h),
            gles::TEXTURE_CUBE_MAP => gl.copy_tex_sub_image_2d(
                gles::TEXTURE_CUBE_MAP_POSITIVE_X + dst.layer + i,
                level,
                x,
                y,
                sx,
                sy,
                w,
                h,
            ),
            _ => gl.copy_tex_sub_image_3d(
                d.target,
                level,
                x,
                y,
                read_layer(dst, i) as i32,
                sx,
                sy,
                w,
                h,
            ),
        }
    }
}

fn copy_texture_to_buffer_held<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    src: &Region,
    dst: &Place,
    extent: Extent,
) {
    if through_draw(gpu, src, extent, |b, colour| {
        copy_texture_to_buffer_held(b, colour, dst, extent);
        Ok(())
    })
    .is_some()
    {
        return;
    }
    let direct = src.texture.read.filter(|r| !r.repacks());
    let (Some(read), Some(data), None) = (direct, dst.buffer.data, &src.texture.host) else {
        // Read on the host (the host copy, or the GPU's texels read back now), then each row of
        // blocks written into the buffer in order: for textures kept on the host, formats
        // `readPixels` reads other than stored, and index-only buffers. Depth arrives here as the
        // colour texture it was drawn into, whose texels keep the depth's own bytes.
        let (bw, bh, bytes) = match (&src.texture.host, src.texture.read) {
            (None, Some(r)) => (1, 1, r.texel_kept() as u32),
            _ => FORMAT_FACTS[src.texture.desc.format as usize].block,
        };
        let (row, rows) = (
            (extent.width.div_ceil(bw) * bytes) as usize,
            extent.height.div_ceil(bh) as usize,
        );
        let mut packed = vec![0; row * rows * extent.depth_or_layers as usize];
        if let Some(h) = &src.texture.host {
            if host_current(gpu, &src.texture, 1 << src.mip).is_err() {
                return;
            }
            let size = [extent.width, extent.height, extent.depth_or_layers];
            h.read(&src.texture.desc, src.mip, corner(src), size, &mut packed);
        } else {
            gpu.record_slow_path(SlowPath::HostCopyReadBack);
            if read_texture_now(gpu, src, extent, &mut packed).is_err() {
                return invalid(
                    gpu,
                    "a copy out of a format that cannot be read back".into(),
                );
            }
        }
        let (pitch, per_layer) = (u64::from(dst.row_pitch), u64::from(dst.rows_per_layer));
        for (i, chunk) in packed.chunks_exact(row).enumerate() {
            let (layer, r) = ((i / rows) as u64, (i % rows) as u64);
            let at = dst.offset + (layer * per_layer + r) * pitch;
            write_buffer_held(gpu, &dst.buffer, at, chunk, WriteMode::Ordered);
        }
        return;
    };
    fresh(gpu, &src.texture);
    refresh(gpu, &dst.buffer, None);
    let layer_bytes = dst.row_pitch as usize * dst.rows_per_layer as usize;
    for i in 0..extent.depth_or_layers {
        let layer = read_layer(src, i);
        read_framebuffer(gpu, &src.texture, src.mip, layer);
        gpu.gl.bind_buffer(gles::PIXEL_PACK_BUFFER, data);
        gpu.gl.pixel_storei(
            gles::PACK_ROW_LENGTH,
            (dst.row_pitch as usize / read.texel_kept()) as i32,
        );
        gpu.gl.read_pixels(
            src.origin.x as i32,
            src.origin.y as i32,
            extent.width as i32,
            extent.height as i32,
            read.format,
            read.ty,
            PixelPack::Offset(dst.offset + (i as usize * layer_bytes) as u64),
        );
        gpu.gl.pixel_storei(gles::PACK_ROW_LENGTH, 0);
        gpu.gl.bind_buffer(gles::PIXEL_PACK_BUFFER, 0);
    }
    if dst.buffer.index.is_some() {
        let end = dst.offset + (layer_bytes * extent.depth_or_layers as usize) as u64;
        dst.buffer.maxima.borrow_mut().unknown(dst.offset..end);
        mark_stale(gpu, &dst.buffer, true, dst.offset, end);
    }
}

fn copy_buffer_to_texture_held<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    src: &Place,
    dst: &Region,
    extent: Extent,
) {
    let data = src.buffer.data.filter(|_| {
        dst.texture.host.is_none() && !format::converts_writes(dst.texture.desc.format)
    });
    let Some(data) = data else {
        // The buffer's bytes read back, then written as a host write: into textures kept on the
        // host or swapped there, and from index-only buffers, which no unpack buffer can be.
        let (bw, bh, bytes) = FORMAT_FACTS[dst.texture.desc.format as usize].block;
        let (row, rows) = (
            u64::from(extent.width.div_ceil(bw) * bytes),
            u64::from(extent.height.div_ceil(bh)),
        );
        let layers = u64::from(extent.depth_or_layers);
        if rows == 0 || layers == 0 {
            return;
        }
        let (pitch, per_layer) = (u64::from(src.row_pitch), u64::from(src.rows_per_layer));
        let size = pitch * (per_layer * (layers - 1) + rows - 1) + row;
        let mut bytes = vec![0; size as usize];
        gpu.record_slow_path(SlowPath::HostCopyReadBack);
        let read = Source::Buffer {
            buffer: src.buffer.clone(),
            offset: src.offset,
            size,
        };
        if read_back_now_held(gpu, &read, &mut bytes).is_ok() {
            let (pitch, per_layer) = (src.row_pitch, src.rows_per_layer);
            write_texture_held(
                gpu,
                dst,
                extent,
                &bytes,
                pitch,
                per_layer,
                WriteMode::Ordered,
            );
        }
        return;
    };
    refresh(gpu, &src.buffer, Some(false));
    fresh(gpu, &dst.texture);
    written(gpu, &dst.texture);
    gpu.gl.bind_buffer(gles::PIXEL_UNPACK_BUFFER, data);
    let texels = Texels::Offset(src.offset as u32);
    if FORMAT_FACTS[dst.texture.desc.format as usize].depth_stencil {
        write_depth(gpu, dst, extent, texels, src.row_pitch, src.rows_per_layer);
    } else {
        upload(gpu, dst, extent, texels, src.row_pitch, src.rows_per_layer);
    }
    gpu.gl.bind_buffer(gles::PIXEL_UNPACK_BUFFER, 0);
}

fn read_back_held<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    src: &Source,
) -> Result<ReadbackRef<O::Fence>, GpuError> {
    if let Source::Texture { region, extent } = src
        && let Some(r) = through_draw(gpu, region, *extent, |b, colour| {
            read_back_held(
                b,
                &Source::Texture {
                    region: colour.clone(),
                    extent: *extent,
                },
            )
        })
    {
        return r;
    }
    if let Source::Texture { region, extent } = src
        && let Some(h) = &region.texture.host
    {
        host_current(gpu, &region.texture, 1 << region.mip)?;
        let (bw, bh, bytes) = FORMAT_FACTS[region.texture.desc.format as usize].block;
        let size = extent.width.div_ceil(bw) * extent.height.div_ceil(bh) * bytes;
        let mut kept = vec![0; (size * extent.depth_or_layers) as usize];
        let corner = corner(region);
        let whole = [extent.width, extent.height, extent.depth_or_layers];
        h.read(&region.texture.desc, region.mip, corner, whole, &mut kept);
        return Ok(Handle(Rc::new(core::cell::RefCell::new(ReadbackObj {
            pbo: None,
            queries: Vec::new(),
            timestamps: false,
            size: kept.len(),
            sync: None,
            waited: false,
            repack: None,
            host: Some(kept),
        }))));
    }
    if let Source::Texture { region, .. } = src {
        fresh(gpu, &region.texture);
    }
    let size = match src {
        Source::Buffer { size, .. } => *size as usize,
        Source::Texture { region, extent } => {
            let read = region.texture.read.ok_or(GpuError::Invalid(
                "webgl: the format cannot be read back here",
            ))?;
            (extent.width * extent.height * extent.depth_or_layers) as usize * read.texel_read()
        }
    };
    // The staging buffer takes its WebGL type from its first target: a pixel pack buffer for
    // texels, and for a buffer the type of the GL buffer it copies from, since WebGL copies only
    // between buffers of one type.
    let target = match src {
        Source::Buffer { buffer, .. } if buffer.data.is_none() => {
            // The vertex array's index binding changes; the next indexed draw rebinds it.
            gpu.applied.index = 0;
            gles::ELEMENT_ARRAY_BUFFER
        }
        Source::Buffer { .. } => gles::COPY_WRITE_BUFFER,
        Source::Texture { .. } => gles::PIXEL_PACK_BUFFER,
    };
    let pbo = {
        let pbo = platform::one(|n| gpu.gl.gen_buffers(n)).ok_or(GpuError::OutOfMemory)?;
        let first = if target == gles::COPY_WRITE_BUFFER {
            gles::ARRAY_BUFFER
        } else {
            target
        };
        gpu.gl.bind_buffer(first, pbo);
        gpu.gl
            .buffer_data(first, size as i64, None, gles::STREAM_READ);
        gpu.gl.bind_buffer(target, pbo);
        pbo
    };
    match src {
        Source::Buffer {
            buffer,
            offset,
            size,
        } => {
            let raw = any_gl(gpu, buffer);
            gpu.gl.bind_buffer(gles::COPY_READ_BUFFER, raw);
            gpu.gl.copy_buffer_sub_data(
                gles::COPY_READ_BUFFER,
                target,
                *offset as i64,
                0,
                *size as i64,
            );
        }
        Source::Texture { region, extent } => {
            let t = &region.texture;
            let read = t.read.expect("checked above");
            let layer_bytes = extent.width * extent.height * read.texel_read() as u32;
            for i in 0..extent.depth_or_layers {
                let layer = read_layer(region, i);
                read_framebuffer(gpu, t, region.mip, layer);
                gpu.gl.read_pixels(
                    region.origin.x as i32,
                    region.origin.y as i32,
                    extent.width as i32,
                    extent.height as i32,
                    read.format,
                    read.ty,
                    PixelPack::Offset(u64::from(i * layer_bytes)),
                );
            }
        }
    }
    let sync = {
        gpu.gl.bind_buffer(gles::PIXEL_PACK_BUFFER, 0);
        gpu.gl.fence_sync(gles::SYNC_GPU_COMMANDS_COMPLETE, 0)
    };
    // Without a sync object nothing says when the copy is done: only a lost context fails here.
    if sync.is_none() {
        fail(gpu, LossReason::Reset);
    }
    let repack = match src {
        Source::Texture { region, .. } => region.texture.read,
        Source::Buffer { .. } => None,
    }
    .filter(|r| r.repacks());
    if repack.is_some_and(format::ReadAs::converts) {
        gpu.record_slow_path(SlowPath::FormatConverted);
    }
    Ok(Handle(Rc::new(core::cell::RefCell::new(ReadbackObj {
        pbo: Some(pbo),
        queries: Vec::new(),
        timestamps: false,
        size,
        sync,
        waited: false,
        repack,
        host: None,
    }))))
}

fn read_back_now_held<O: OpenGlEs>(
    gpu: &mut Gl<O>,
    src: &Source,
    dst: &mut [u8],
) -> Result<(), GpuError> {
    match src {
        Source::Texture { region, extent } => read_texture_now(gpu, region, *extent, dst),
        Source::Buffer {
            buffer,
            offset,
            size,
        } => {
            let raw = any_gl(gpu, buffer);
            gpu.gl.bind_buffer(gles::COPY_READ_BUFFER, raw);
            platform::get_buffer_sub_data(
                &gpu.gl,
                gles::COPY_READ_BUFFER,
                *offset,
                &mut dst[..*size as usize],
            )
            .then_some(())
            .ok_or_else(|| read_failed(gpu))
        }
    }
}

fn destroy_held<O: OpenGlEs>(gpu: &mut Gl<O>, object: Held) {
    if let Held::Surface(surface) = &object {
        if gpu.surface.as_ref().is_some_and(|s| s.id == *surface)
            && let Some((t, v)) = gpu.surface.take().and_then(|s| s.frame)
        {
            gpu.objects.views.remove(&v.id);
            destroy_held(gpu, Held::Texture(t));
        }
        return;
    }
    if let Held::Texture(t) = &object
        && let Some(twin) = t.twin.get()
    {
        gpu.twins -= 1;
        let twin = twin.clone();
        destroy_held(gpu, Held::Texture(twin));
    }
    let gl = &gpu.gl;
    match object {
        Held::Buffer(b) => {
            if b.stale.get().is_some() {
                gpu.stale_buffers -= 1;
            }
            for raw in [b.data, b.index].into_iter().flatten() {
                gl.delete_buffers(&[raw]);
            }
        }
        Held::Texture(t) => {
            gpu.framebuffers.retain(|key, fb| {
                let uses = key.iter().flatten().any(|a| a.0 == t.id);
                if uses {
                    gl.delete_framebuffers(&[*fb]);
                }
                !uses
            });
            if gpu
                .applied
                .draw_framebuffer
                .is_some_and(|fb| !gpu.framebuffers.values().any(|f| *f == fb))
            {
                gpu.applied.draw_framebuffer = None;
            }
            if gpu
                .applied
                .read_framebuffer
                .is_some_and(|fb| !gpu.framebuffers.values().any(|f| *f == fb))
            {
                gpu.applied.read_framebuffer = None;
            }
            match t.storage {
                Storage::Texture(raw) => gl.delete_textures(&[raw]),
                Storage::Samples(rb) => gl.delete_renderbuffers(&[rb]),
            }
        }
        Held::Sampler(s) => {
            gl.delete_samplers(&[s.raw]);
            if let Some((_, twin)) = s.level_zero {
                gl.delete_samplers(&[twin]);
            }
        }
        Held::Shader(s) => {
            s.handles.set(s.handles.get() - 1);
            if s.handles.get() == 0 {
                gpu.shaders.remove(&s.text);
                gpu.objects.shaders.remove(&s.id);
                gl.delete_shader(s.raw);
            }
        }
        Held::Program(p) => {
            p.handles.set(p.handles.get() - 1);
            if p.handles.get() == 0 {
                gpu.programs.remove(&p.pair);
                gpu.objects.programs.remove(&p.id);
                gl.delete_program(p.raw);
                let captures = p.captures.iter().filter_map(|c| c.get());
                for c in captures.chain(core::iter::once(&p.0)) {
                    for (_, v) in c.variants.borrow().iter() {
                        gl.delete_program(v.raw);
                        if gpu.applied.program == v.id {
                            gpu.applied.program = 0;
                        }
                    }
                }
                for c in p.captures.iter().filter_map(|c| c.get()) {
                    gl.delete_program(c.raw);
                    if gpu.applied.program == c.id {
                        gpu.applied.program = 0;
                    }
                }
                if gpu.applied.program == p.id {
                    gpu.applied.program = 0;
                }
            }
        }
        Held::Surface(_) => {}
        Held::QuerySet(set) => {
            // Queries a read back still holds are deleted when it finishes.
            for q in set.slots.borrow_mut().drain(..).flatten() {
                if let Ok(q) = Rc::try_unwrap(q) {
                    gl.delete_queries(&[q]);
                }
            }
        }
    }
}
