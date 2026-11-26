//! The WebGL 2 back end of the wcontainers graphics interface, written on glow.
//!
//! The same code draws through Chrome's WebGL 2 when built for WebAssembly and through ANGLE's OpenGL ES
//! 3.0 natively (a test tool only), following WebGL 2's rules on both. Blocks are applied by difference:
//! a block is looked at only when its handle changed, and then only GL values that differ are set. The
//! interface's row 0 is the top: with EXT_clip_control the device draws upper-left; without it, programs
//! negate y and the front face is swapped.

mod caps;
mod capture;
mod convert;
mod depth;
mod format;
mod host;
pub mod platform;
mod restart;
mod shader;
mod state;

/// The GLSL ES 3.00 the back end writes for a shader and the features it needs, for checking
/// shaders outside a context.
pub mod glsl {
    pub use crate::shader::{GlslSettings, ShaderText, needs, write};
}
#[cfg(feature = "trace")]
pub mod trace;

use glow::HasContext;
use shader::{ProgramObj, ShaderObj, Slot};
use state::GlValues;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::rc::Rc;
use wcontainers_gpu::{
    caps::*, draw::*, error::*, format::*, helpers::*, resource::*, shader::*, slow::*, state::*, *,
};

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
pub struct BufferObj {
    id: Id,
    size: u64,
    usage: u32,
    /// The GL buffer for everything but indices, and the one for indices (WebGL never lets one GL
    /// buffer hold both).
    data: Option<glow::Buffer>,
    index: Option<glow::Buffer>,
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
    Texture(glow::Texture),
    Samples(glow::Renderbuffer),
}

#[derive(Debug)]
pub struct TextureObj {
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
    twin: std::cell::OnceCell<Texture>,
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
pub struct ViewObj {
    id: Id,
    texture: Texture,
    desc: TextureViewDesc,
}

#[derive(Debug)]
pub struct SamplerObj {
    id: Id,
    raw: glow::Sampler,
    /// Its twin whose minimum level of detail is 0, for units that gathers read, where its own
    /// minimum is above 0.
    level_zero: Option<(Id, glow::Sampler)>,
}

/// What one binding of a set holds.
#[derive(Debug)]
enum Bound {
    Uniform(Buffer, u64, u64),
    Texture(TextureView),
    Sampler(Sampler),
}

#[derive(Debug)]
pub struct SetObj {
    id: Id,
    entries: Vec<(u32, Bound)>,
}

#[derive(Debug)]
pub struct ReadbackObj {
    /// The pack buffer texels or bytes are copied into; none for query results.
    pbo: Option<glow::Buffer>,
    /// The queries read, in order; none for a query never begun or written, which reads zero.
    queries: Vec<Option<Rc<glow::Query>>>,
    /// The queries are timestamps (64-bit results).
    timestamps: bool,
    size: usize,
    sync: Option<glow::Fence>,
    /// Waited for: WebGL updates a sync object's status only between tasks, so after `finish` it is
    /// known done though the sync object does not say so yet.
    waited: bool,
    /// Texels read wider than they are stored, kept narrow when finished.
    repack: Option<format::ReadAs>,
    /// Texels read from a host copy, ready when made.
    host: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct FenceObj {
    /// None where the context was lost when the fence was made.
    sync: Option<glow::Fence>,
    done: Cell<bool>,
}

/// A handle: a clone names the same object.
#[derive(Debug)]
pub struct Handle<T>(Rc<T>);

impl TextureObj {
    /// The GL texture; none for a multisampled texture.
    fn tex(&self) -> Option<glow::Texture> {
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

pub type Buffer = Handle<BufferObj>;
pub type Texture = Handle<TextureObj>;
pub type TextureView = Handle<ViewObj>;
pub type Sampler = Handle<SamplerObj>;
pub type Shader = Handle<ShaderObj>;
pub type GlProgram = Handle<ProgramObj>;
/// A state block: its id and the GL values it stands for.
pub type Block<T> = Handle<(Id, T)>;
pub type BindingSet = Handle<SetObj>;
pub type Readback = Handle<core::cell::RefCell<ReadbackObj>>;
pub type Fence = Handle<FenceObj>;

/// A query set: each query's GL object, made when the query is first begun or written. A read back
/// holds the objects it reads, so a query begun again before the read finished gets a new one.
#[derive(Debug)]
pub struct QuerySetObj {
    kind: QueryKind,
    slots: core::cell::RefCell<Vec<Option<Rc<glow::Query>>>>,
}

pub type QuerySet = Handle<QuerySetObj>;

/// WebGL 2 has no resource tables, so none is ever made.
#[derive(Clone, Debug)]
pub enum NoTable {}

/// The one surface a back end has: its context's canvas (natively, an EGL window surface).
#[derive(Clone, Debug)]
pub struct Surface {
    id: Id,
}

/// The surface's frame: the texture `current_texture` hands out, copied into the canvas's drawing
/// buffer by `present` (the drawing buffer is not a texture, so nothing can draw into it directly).
#[derive(Debug)]
struct SurfaceState {
    id: Id,
    frame: Option<(Texture, TextureView)>,
}

/// Which mip, layer and slice of which texture an attachment is.
type Attachment = (Id, u32, u32);

/// The attachments of a framebuffer: eight colours and depth-stencil.
type FramebufferKey = [Option<Attachment>; MAX_COLOR_TARGETS + 1];

/// What a running pass must do when it ends.
#[derive(Debug)]
struct Pass {
    framebuffer: glow::Framebuffer,
    discard: Points,
    occlusion: Option<QuerySet>,
    /// An occlusion query is running.
    counting: bool,
    /// Per colour target: the multisampled texture and the view it resolves into at the end.
    resolves: [Option<(Texture, TextureView)>; MAX_COLOR_TARGETS],
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
    draw_framebuffer: Option<glow::Framebuffer>,
    read_framebuffer: Option<glow::Framebuffer>,
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

/// The back end fronts build on: [`Gl`], or with the `record` feature [`Gl`] recording every call
/// into a stream file ([`trace::Traced`]).
#[cfg(not(feature = "record"))]
pub type WebGl = Gl;
#[cfg(feature = "record")]
pub type WebGl = trace::Traced<Gl>;

/// The start of the line the back end prints on standard error at each power of two frames
/// presented: the frames, then every slow path taken as `name=count`, so any program's log gives
/// what a frame costs between two reports.
pub const FRAME_REPORT: &str = "webgl: frames=";

/// What a WebGL 2 back end is made from natively: ANGLE's folder and the extensions to leave unused.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub struct GlConfig {
    pub angle: std::path::PathBuf,
    pub off: Vec<String>,
}

/// What a WebGL 2 back end is made from in a browser: a canvas's context and extensions left unused.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
pub struct GlConfig {
    pub context: web_sys::WebGl2RenderingContext,
    pub off: Vec<String>,
}

impl StandaloneGpu for Gl {
    type Config = GlConfig;
    /// Why ANGLE gave no context, as its loader or EGL said.
    type NewError = String;

    fn new(config: GlConfig) -> Result<Self, String> {
        let off: Vec<&str> = config.off.iter().map(String::as_str).collect();
        #[cfg(not(target_arch = "wasm32"))]
        return Gl::on_angle(&config.angle, &off);
        #[cfg(target_arch = "wasm32")]
        return Ok(Gl::on_canvas(config.context, &off));
    }
}

/// The WebGL 2 back end. Made on the device thread, which owns the GL context.
pub struct Gl {
    gl: glow::Context,
    platform: platform::Platform,
    extensions: BTreeSet<&'static str>,
    renderer: String,
    caps: Caps,
    slow: SlowPaths,
    helpers: Helpers<Gl>,
    lost: Option<LossReason>,
    next_id: Id,
    /// Frames presented, for [`FRAME_REPORT`].
    presented: u64,
    /// How shaders are written; without EXT_clip_control programs negate y and the front face is
    /// swapped.
    glsl: shader::GlslSettings,
    values: GlValues,
    applied: Applied,
    /// The live shaders by their text, so shaders of equal text are one.
    shaders: HashMap<Rc<shader::ShaderText>, Shader>,
    programs: HashMap<(Id, Id), GlProgram>,
    framebuffers: HashMap<FramebufferKey, glow::Framebuffer>,
    /// Framebuffers of passes without attachments, each on an `R8` renderbuffer of its own that no
    /// draw writes (GLES 3.0 has no framebuffer without attachments).
    empty_passes: HashMap<EmptyPass, glow::Framebuffer>,
    empty_fragment: glow::Shader,
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
    bits_program: Option<glow::Program>,
    /// The stencil write programs for 2D and 2D array sources.
    stencil_write: [Option<depth::StencilWrite>; 2],
    /// The programs of copies by draws, by source format, destination format and source target, with
    /// their uniform `at`.
    /// Programs of `copy_by_draw` by formats and source target.
    copy_programs: HashMap<(Format, Format, u32), CopyProgram>,
    /// A nearest-filtering sampler for the back end's own fetches, made at its first use.
    point_sampler: Option<glow::Sampler>,
    /// Whether blits between textures of one float format keep every bit (the probe at creation).
    float_blits: bool,
}

/// A program of `copy_by_draw`, with its uniforms `at` and, into unorm depth, `unit`.
type CopyProgram = (
    glow::Program,
    glow::UniformLocation,
    Option<glow::UniformLocation>,
);

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

/// The name in `off` that takes blits keeping every bit of floats as missing, as where the probe
/// finds they do not.
pub const FLOAT_BLITS: &str = "float_blits";

impl Gl {
    /// Makes the back end on a current GL ES 3.0 or WebGL 2 context. `off` lists WebGL extension names
    /// (and [`FLOAT_BLITS`]) to treat as missing, so tests can take the routes of browsers without them.
    pub fn new(gl: glow::Context, platform: platform::Platform, off: &[&str]) -> Gl {
        let extensions = platform::extensions(&gl, off);
        let renderer = platform::renderer(&gl);
        let mut caps = caps::caps(&gl, &platform, &extensions, &renderer);
        // Timestamps where the extension is present and the GPU writes them (it may give zero bits).
        caps.features.set(
            Features::TIMESTAMPS,
            extensions.contains("EXT_disjoint_timer_query_webgl2")
                && platform.timestamp_bits(&gl) > 0,
        );
        let flip_y = !extensions.contains("EXT_clip_control");
        let noperspective = extensions.contains(platform::NOPERSPECTIVE_INTERPOLATION);
        let clip_depth = if flip_y {
            glow::NEGATIVE_ONE_TO_ONE
        } else {
            platform.clip_control(&gl, glow::UPPER_LEFT, glow::ZERO_TO_ONE);
            glow::ZERO_TO_ONE
        };
        // SAFETY: plain GL calls on the current context.
        let empty_fragment = unsafe {
            gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            // WebGL 2 always restarts strips at the largest index; GLES has it as a switch.
            #[cfg(not(target_arch = "wasm32"))]
            gl.enable(glow::PRIMITIVE_RESTART_FIXED_INDEX);
            let vao = gl.create_vertex_array().expect("a vertex array object");
            gl.bind_vertex_array(Some(vao));
            let s = gl
                .create_shader(glow::FRAGMENT_SHADER)
                .expect("a shader object");
            gl.shader_source(s, "#version 300 es\nvoid main() {}\n");
            gl.compile_shader(s);
            s
        };
        let mut b = Gl {
            gl,
            platform,
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
        };
        match b.probe_depth24() {
            Ok(unorm) => b.caps.features.set(Features::DEPTH24_UNORM, unorm),
            Err(log) => {
                b.own_program_failed(format!("webgl: the 24-bit depth probe: {log}"));
            }
        }
        match b.probe_fractional_bias() {
            Ok(fraction) => b
                .caps
                .features
                .set(Features::FRACTIONAL_DEPTH_BIAS, fraction),
            Err(log) => {
                b.own_program_failed(format!("webgl: the fractional depth bias probe: {log}"));
            }
        }
        b.float_blits = !off.contains(&FLOAT_BLITS) && b.probe_float_blits();
        b
    }

    /// Whether `blitFramebuffer` between textures of one float format keeps every bit (NaN payloads
    /// and signs, denormals), which GL does not promise: special 16-bit and 32-bit floats blitted,
    /// drawn as bits into their unsigned integer twins and read back. False where float textures are
    /// no targets (without EXT_color_buffer_float).
    fn probe_float_blits(&mut self) -> bool {
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
            self.caps.formats.uses[*format as usize].contains(FormatUse::RENDER)
                && self.blit_keeps(*format, *twin, bytes)
        })
    }

    /// Whether `bytes`, one row of `format`, come back the same after a blit into a second texture.
    fn blit_keeps(&mut self, format: Format, twin: Format, bytes: &[u8]) -> bool {
        let width = bytes.len() as u32 / FORMAT_FACTS[format as usize].block.2;
        let make = |b: &mut Self, format| {
            b.create_texture(&TextureDesc {
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
            })
        };
        let made = [make(self, format), make(self, format), make(self, twin)];
        let kept = match &made {
            [Ok(from), Ok(to), Ok(bits)] => {
                let region = |texture| TextureRegion {
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
                self.write_texture(&region(from), extent, bytes, row, 1, WriteMode::Ordered);
                self.blit_same(&region(from), &region(to), extent);
                let mut got = vec![0; bytes.len()];
                self.copy_by_draw(&region(to), &region(bits), extent)
                    && self
                        .read_texture_now(&region(bits), extent, &mut got)
                        .is_ok()
                    && got == bytes
            }
            _ => false,
        };
        for t in made.into_iter().flatten() {
            self.destroy(Object::Texture(t));
        }
        kept
    }

    /// The 24-bit depth probe (plan section 4.11): a triangle at depth 0.1 drawn into a 1 by 1
    /// `depth24plus` texture without and with a constant bias of one unit, each depth then fetched as
    /// float bits. One 24-bit step between them means 24-bit unsigned normalised storage, one float
    /// step 32-bit float storage; anything else is a bug of the back end's.
    fn probe_depth24(&mut self) -> Result<bool, String> {
        let bits = self.probe_bias(Format::Depth24Plus, 0.1, [0.0, 1.0])?;
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

    /// The fractional bias probe: a triangle at 0.45 of a step above step 1000 of a 1 by 1
    /// `depth16unorm` texture, without and with a constant bias of 0.1 units. True when the bias
    /// takes it to step 1001, as a fractional bias must; false when it stays (GL may round the units
    /// to whole steps: ANGLE on Direct3D 11 does).
    fn probe_fractional_bias(&mut self) -> Result<bool, String> {
        let words = self.probe_bias(Format::Depth16Unorm, 1000.45 / 65535.0, [0.0, 0.1])?;
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
    fn probe_bias(&mut self, format: Format, z: f32, units: [f32; 2]) -> Result<[u32; 2], String> {
        let error = |e: GpuError| format!("{e:?}");
        let (program, uniform) = depth::build_probe(&self.gl)?;
        let texture = |b: &mut Self, format, usage| {
            b.create_texture(&TextureDesc {
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
            })
        };
        let depth = texture(self, format, TextureUsage::RENDER).map_err(error)?;
        let out = texture(self, Format::R32Uint, TextureUsage::RENDER).map_err(error)?;
        let fb = self.texture_framebuffer(&depth, 0, 0);
        let out_fb = self.texture_framebuffer(&out, 0, 0);
        let mut bits = [0u32; 2];
        for (units, bits) in units.into_iter().zip(&mut bits) {
            // SAFETY: plain GL calls; the program and the framebuffer are alive.
            unsafe {
                self.gl.use_program(Some(program));
                let clip = if self.values.clip_depth == glow::ZERO_TO_ONE {
                    z
                } else {
                    2.0 * z - 1.0
                };
                self.gl.uniform_1_f32(Some(&uniform), clip);
                self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb));
                self.applied.draw_framebuffer = Some(fb);
                self.open_writes();
                self.gl.clear_buffer_f32_slice(glow::DEPTH, 0, &[1.0]);
            }
            state::plain(&self.gl, &mut self.values, [0, 0, 1, 1], &self.platform);
            let (gl, v) = (&self.gl, &mut self.values);
            // SAFETY: plain GL state calls, recorded in the values so the next draw sets its own.
            unsafe {
                gl.enable(glow::DEPTH_TEST);
                v.depth_test = true;
                gl.depth_func(glow::ALWAYS);
                v.depth_func = glow::ALWAYS;
                gl.enable(glow::POLYGON_OFFSET_FILL);
                gl.polygon_offset(0.0, units);
                v.polygon_offset = Some((0.0, units, 0.0));
            }
            self.own_draw();
            self.applied.program = 0;
            self.fetch(Fetch::Depth2D, &depth, 0, [0; 3], 0, out_fb, (1, 1))
                .map_err(error)?;
            let mut word = [0u8; 4];
            self.read_texture_now(
                &TextureRegion {
                    texture: &out,
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
        self.destroy(Object::Texture(depth));
        self.destroy(Object::Texture(out));
        // SAFETY: a plain GL call; the program is no longer used.
        unsafe { self.gl.delete_program(program) };
        Ok(bits)
    }

    /// Makes the back end on ANGLE's OpenGL ES 3.0, loaded from `libEGL` in `dir` (on the desktop, the
    /// mozangle build angle\build.cmd makes). A test tool: ANGLE is never shipped.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn on_angle(dir: &std::path::Path, off: &[&str]) -> Result<Gl, String> {
        let (gl, platform) = platform::angle(dir)?;
        Ok(Gl::new(gl, platform, off))
    }

    /// Names a host window (an `HWND` on Windows) that `create_surface` may attach to, as the host's
    /// graphics hook numbers it.
    ///
    /// # Safety
    /// `window` is a native window that outlives every surface made on it.
    #[cfg(not(target_arch = "wasm32"))]
    pub unsafe fn add_window(&mut self, name: u32, window: *mut core::ffi::c_void) {
        // SAFETY: as the caller promises.
        unsafe { self.platform.add_window(name, window) }
    }

    /// Makes the back end on a canvas's WebGL 2 context.
    #[cfg(target_arch = "wasm32")]
    pub fn on_canvas(context: web_sys::WebGl2RenderingContext, off: &[&str]) -> Gl {
        let (gl, platform) = platform::canvas(context);
        Gl::new(gl, platform, off)
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
        let mut all: Vec<String> = self.gl.supported_extensions().iter().cloned().collect();
        all.sort();
        all
    }

    /// GL's log of the last shader or program that failed to build.
    pub fn last_log(&self) -> &str {
        &self.last_log
    }

    fn id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }

    /// Loses the device for an error GL has recorded since the last look: a bug of ours or a lost
    /// context. Called where the back end has just waited for the GPU, so the look costs no wait.
    fn gl_errors(&mut self) {
        // SAFETY: a plain GL query.
        let error = unsafe { self.gl.get_error() };
        let reason = match error {
            glow::NO_ERROR => return,
            glow::OUT_OF_MEMORY => LossReason::OutOfMemory,
            glow::CONTEXT_LOST => LossReason::Reset,
            _ => LossReason::Error,
        };
        if self.lost.is_none() {
            self.last_log = format!("GL error {error:#x}");
        }
        self.fail(reason);
    }

    /// Records an error on GL's side as device loss, which fronts report in their API's words.
    fn fail(&mut self, reason: LossReason) {
        if self.lost.is_none() {
            // The interface carries only the reason's kind; the host's error output gets the why.
            platform::report(&format!(
                "wcontainers webgl: device lost ({reason:?}): {}",
                self.last_log
            ));
        }
        self.lost.get_or_insert(reason);
    }

    /// A read from a GL buffer that gave no bytes: the device is lost, so nothing reads stale bytes as
    /// an answer.
    fn read_failed(&mut self) -> GpuError {
        self.fail(LossReason::Error);
        GpuError::DeviceLost
    }

    /// A call whose route this back end does not have yet: the device is lost with the missing piece in
    /// [`Gl::last_log`], so the front reports device loss instead of drawing wrong.
    fn unbuilt(&mut self, what: &'static str) -> GpuError {
        if self.lost.is_none() {
            self.last_log = format!("webgl: not built yet: {what}");
        }
        self.fail(LossReason::Error);
        GpuError::DeviceLost
    }

    /// A recording call that broke the interface's rules (a bug in a front): the device is lost with
    /// the rule in [`Gl::last_log`], and nothing of the call runs.
    fn invalid(&mut self, rule: String) {
        if self.lost.is_none() {
            self.last_log = format!("webgl: invalid: {rule}");
        }
        self.fail(LossReason::Error);
    }

    /// The sRGB twin of `texture` in `format`, made at its first use: a second GL texture of the same
    /// shape, which [`Gl::fresh`] keeps holding the same bytes.
    /// The GL query object for query `index` of `set`, a new one when a read back still holds the
    /// last. None (the device lost) past the set's end or when GL makes none.
    fn query_slot(&mut self, set: &QuerySet, index: u32) -> Option<glow::Query> {
        let mut slots = set.slots.borrow_mut();
        let Some(slot) = slots.get_mut(index as usize) else {
            drop(slots);
            self.invalid(format!("query {index} past the set's end"));
            return None;
        };
        if slot.as_ref().is_none_or(|q| Rc::strong_count(q) > 1) {
            // SAFETY: a plain GL call.
            match unsafe { self.gl.create_query() } {
                Ok(q) => *slot = Some(Rc::new(q)),
                Err(_) => {
                    drop(slots);
                    self.fail(LossReason::OutOfMemory);
                    return None;
                }
            }
        }
        slot.as_deref().copied()
    }

    /// Whether every query's result is available.
    fn queries_available(&self, queries: &[Option<Rc<glow::Query>>]) -> bool {
        queries.iter().flatten().all(|q| {
            // SAFETY: a plain query of a live query object.
            unsafe {
                self.gl
                    .get_query_parameter_u32(**q, glow::QUERY_RESULT_AVAILABLE)
                    != 0
            }
        })
    }

    /// A multisampled texture: a renderbuffer of the format and sample count.
    fn create_samples(
        &mut self,
        desc: &TextureDesc,
        gl_format: format::GlFormat,
    ) -> Result<Texture, GpuError> {
        if !self.caps.formats.takes_samples(desc.format, desc.samples)
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
        let gl = &self.gl;
        // SAFETY: plain GL calls; the renderbuffer binding is used by nothing else.
        let rb = unsafe {
            let rb = gl
                .create_renderbuffer()
                .map_err(|_| GpuError::OutOfMemory)?;
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(rb));
            gl.renderbuffer_storage_multisample(
                glow::RENDERBUFFER,
                desc.samples as i32,
                gl_format.internal,
                desc.width as i32,
                desc.height as i32,
            );
            rb
        };
        Ok(Handle(Rc::new(TextureObj {
            id: self.id(),
            storage: Storage::Samples(rb),
            target: glow::RENDERBUFFER,
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
    fn blit_resolve(&mut self, src: &Texture, dst: &Texture, mip: u32, layer: u32) {
        let mask = match FORMAT_FACTS[src.desc.format as usize].kind {
            FormatKind::DepthStencil => glow::DEPTH_BUFFER_BIT | glow::STENCIL_BUFFER_BIT,
            FormatKind::Depth => glow::DEPTH_BUFFER_BIT,
            FormatKind::Stencil => glow::STENCIL_BUFFER_BIT,
            _ => glow::COLOR_BUFFER_BIT,
        };
        self.open_writes();
        self.read_framebuffer(src, 0, 0);
        let fb = self.texture_framebuffer(dst, mip, layer);
        let (w, h) = (src.desc.width as i32, src.desc.height as i32);
        // SAFETY: plain GL calls; both framebuffers hold one image of the same format and size.
        unsafe {
            self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb));
            self.gl
                .blit_framebuffer(0, 0, w, h, 0, 0, w, h, mask, glow::NEAREST);
        }
        self.applied.draw_framebuffer = Some(fb);
    }

    /// A whole copy between multisampled textures of one format and sample count: GL ES 3.0 blits
    /// into no multisampled framebuffer and programs cannot read samples here, so the source is
    /// resolved and drawn into every sample of the destination
    /// ([`SlowPath::MultisampleCopyResolved`], a listed loss).
    fn copy_samples(&mut self, src: &TextureRegion<'_, Self>, dst: &TextureRegion<'_, Self>) {
        let (s, d) = (src.texture, dst.texture);
        if s.desc.samples != d.desc.samples
            || s.desc.width != d.desc.width
            || s.desc.height != d.desc.height
            || FORMAT_FACTS[s.desc.format as usize].block
                != FORMAT_FACTS[d.desc.format as usize].block
        {
            return self.invalid(
                "a copy of a multisampled texture other than whole into one of its sample count"
                    .into(),
            );
        }
        match FORMAT_FACTS[s.desc.format as usize].kind {
            FormatKind::Uint | FormatKind::Sint => {
                return self.invalid(
                    "a copy of integer samples, which cannot be averaged and need the absent \
                     MULTISAMPLE_READ"
                        .into(),
                );
            }
            FormatKind::Depth | FormatKind::DepthStencil | FormatKind::Stencil => {
                self.unbuilt("copies of multisampled depth and stencil");
                return;
            }
            _ => {}
        }
        self.record_slow_path(SlowPath::MultisampleCopyResolved);
        let resolved = match self.create_texture(&TextureDesc {
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::SAMPLE,
            ..s.desc
        }) {
            Ok(t) => t,
            Err(_) => return self.fail(LossReason::OutOfMemory),
        };
        self.blit_resolve(s, &resolved, 0, 0);
        let whole = Extent {
            width: s.desc.width,
            height: s.desc.height,
            depth_or_layers: 1,
        };
        let from = TextureRegion {
            texture: &resolved,
            ..*src
        };
        if !self.copy_by_draw(&from, dst, whole) {
            self.unbuilt("copies of multisampled textures of this format");
        }
        self.destroy(Object::Texture(resolved));
    }

    fn twin_of(&mut self, texture: &Texture, format: Format) -> Result<Texture, GpuError> {
        if let Some(t) = texture.twin.get() {
            return Ok(t.clone());
        }
        let readable = |f: Format| {
            host::kept(f)
                || format::read_as(f, format::FormatExtensions::found(&self.extensions))
                    .is_some_and(|r| r.texel_read() == r.texel_kept())
        };
        if !readable(texture.desc.format) || !readable(format) {
            return Err(self.unbuilt("sRGB twins of formats read back wider than stored"));
        }
        let mut twin = self.create_texture(&TextureDesc {
            format,
            ..texture.desc
        })?;
        let t = Rc::get_mut(&mut twin.0).expect("a texture just made has one handle");
        t.newest = texture.newest.clone();
        t.side = 2;
        t.own = Some(Rc::downgrade(&texture.0));
        if texture.newest.get() == 0 {
            texture.newest.set(1);
        }
        self.twins += 1;
        let _ = texture.twin.set(twin.clone());
        Ok(twin)
    }

    /// The stencil copy of `texture`, made at its first stencil view without WEBGL_stencil_texturing:
    /// an `r8uint` texture of the same shape, which [`Gl::fresh`] fills from the stencil by draws
    /// whenever the stencil changed since.
    fn stencil_copy_of(&mut self, texture: &Texture) -> Result<Texture, GpuError> {
        if let Some(t) = texture.twin.get() {
            return Ok(t.clone());
        }
        let mut copy = self.create_texture(&TextureDesc {
            format: Format::R8Uint,
            samples: 1,
            usage: TextureUsage::RENDER | TextureUsage::SAMPLE,
            reinterpret: false,
            ..texture.desc
        })?;
        let t = Rc::get_mut(&mut copy.0).expect("a texture just made has one handle");
        t.newest = texture.newest.clone();
        t.side = 2;
        t.own = Some(Rc::downgrade(&texture.0));
        texture.newest.set(1);
        self.twins += 1;
        let _ = texture.twin.set(copy.clone());
        Ok(copy)
    }

    /// Fills every level and layer of `copy` with the stencil of `src`, counted by draws.
    fn fill_stencil_copy(&mut self, src: &Texture, copy: &Texture) -> Result<(), GpuError> {
        let d = src.desc;
        for mip in 0..d.mip_levels {
            let size = ((d.width >> mip).max(1), (d.height >> mip).max(1));
            for layer in 0..d.depth_or_layers {
                let counted = self.stencil_bits(src, mip, layer)?;
                let fb = self.texture_framebuffer(copy, mip, layer);
                let size = (size.0 as i32, size.1 as i32);
                self.fetch(Fetch::Depth2D, &counted, 0, [0; 3], 255, fb, size)?;
                self.destroy(Object::Texture(counted));
            }
        }
        Ok(())
    }

    /// Before `t` is read or written: copies its twin's texels into it when the twin holds newer ones.
    fn fresh(&mut self, t: &Texture) {
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
                let _ = self.fill_stencil_copy(&other, t);
            } else {
                self.copy_whole(&other, t);
            }
        }
        t.newest.set(0);
    }

    /// After `t` was written: its twin no longer holds the same texels, and the next draw binds again.
    fn written(&mut self, t: &Texture) {
        if t.side == 2 || t.twin.get().is_some() {
            t.newest.set(t.side);
            self.applied.sets = [Id::MAX; MAX_BINDING_SETS];
        }
    }

    /// Copies every level and layer of `from` into `to`, a texture of the same shape whose texels have
    /// the same bytes: on the GPU, or for textures kept on the host from host copy to host copy, then
    /// uploaded.
    fn copy_whole(&mut self, from: &Texture, to: &Texture) {
        let d = from.desc;
        let kept = from.host.as_ref().zip(to.host.as_ref());
        if let Some((a, b)) = kept {
            self.record_slow_path(SlowPath::HostCopyKept);
            b.copy_from(a);
        }
        for mip in 0..d.mip_levels {
            let region = |texture| TextureRegion {
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
                self.upload_blocks(&region(to), extent);
            } else {
                self.copy_same_bytes(&region(from), &region(to), extent);
            }
        }
    }

    /// Copies a region between textures whose texels have the same bytes, the source's read by
    /// `readPixels` as stored: by a draw where one carries both formats and they are not floats,
    /// which stays on the GPU, else through a pixel buffer, whose unpack ANGLE waits for on the CPU
    /// ([`SlowPath::CopyThroughBuffer`] either way).
    fn copy_same_bytes(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let float = [src, dst]
            .iter()
            .any(|r| format::float_bits(r.texture.desc.format));
        if !float && self.copy_by_draw(src, dst, extent) {
            self.record_slow_path(SlowPath::CopyThroughBuffer);
        } else {
            self.copy_through_pbo(src, dst, extent);
        }
    }

    /// Copies a region between textures whose texels have the same bytes, the source's read by
    /// `readPixels` as stored: read into a pixel pack buffer, then written from it as the destination's
    /// texels, all on the GPU ([`SlowPath::CopyThroughBuffer`]).
    fn copy_through_pbo(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.record_slow_path(SlowPath::CopyThroughBuffer);
        let read = src
            .texture
            .read
            .expect("the caller checked the source is read as stored");
        let texel = read.texel_kept() as u32;
        let layer_bytes = extent.width * extent.height * texel;
        // SAFETY: plain GL calls on the current context.
        let pbo = unsafe {
            let Ok(pbo) = self.gl.create_buffer() else {
                self.fail(LossReason::OutOfMemory);
                return;
            };
            self.gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(pbo));
            self.gl.buffer_data_size(
                glow::PIXEL_PACK_BUFFER,
                (layer_bytes * extent.depth_or_layers) as i32,
                glow::STREAM_COPY,
            );
            pbo
        };
        for i in 0..extent.depth_or_layers {
            let layer = self.read_layer(src, i);
            self.read_framebuffer(src.texture, src.mip, layer);
            // SAFETY: the pack buffer is bound and holds every layer's rows.
            unsafe {
                self.gl.read_pixels(
                    src.origin.x as i32,
                    src.origin.y as i32,
                    extent.width as i32,
                    extent.height as i32,
                    read.format,
                    read.ty,
                    glow::PixelPackData::BufferOffset(i * layer_bytes),
                );
            }
        }
        // SAFETY: plain GL calls; the buffer moves from packing to unpacking.
        unsafe {
            self.gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(pbo));
        }
        self.upload(
            dst,
            extent,
            Texels::Offset(0),
            extent.width * texel,
            extent.height,
        );
        // SAFETY: plain GL calls; GL keeps the buffer for the upload already issued.
        unsafe {
            self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);
            self.gl.delete_buffer(pbo);
        }
    }

    /// Copies a region between colour formats of one typeless group by a draw per layer that fetches
    /// each source texel and writes its bits as the destination's value ([`SlowPath::CopyThroughBuffer`],
    /// convert.rs). False when no such draw carries the two formats or the destination cannot be
    /// drawn into.
    fn copy_by_draw(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> bool {
        let (from, to) = (src.texture.desc.format, dst.texture.desc.format);
        let sampler = match src.texture.target {
            glow::TEXTURE_2D => "2D",
            glow::TEXTURE_2D_ARRAY => "2DArray",
            glow::TEXTURE_3D => "3D",
            _ => "Cube",
        };
        let drawn = if FORMAT_FACTS[to as usize].depth_stencil {
            FormatUse::DEPTH_STENCIL
        } else {
            FormatUse::RENDER
        };
        if !self.caps.formats.uses[to as usize].contains(drawn) {
            return false;
        }
        let key = (from, to, src.texture.target);
        if !self.copy_programs.contains_key(&key) {
            let Some(source) = convert::program(from, to, sampler) else {
                return false;
            };
            let built = depth::link(&self.gl, &source).and_then(|p| {
                // SAFETY: plain queries of a linked program.
                let (at, unit) = unsafe {
                    (
                        self.gl.get_uniform_location(p, "at"),
                        self.gl.get_uniform_location(p, "unit"),
                    )
                };
                at.map(|at| (p, at, unit))
                    .ok_or_else(|| "the copy program has no uniform at".to_string())
            });
            match built {
                Ok(p) => {
                    self.copy_programs.insert(key, p);
                }
                Err(log) => {
                    self.own_program_failed(log);
                    return true;
                }
            }
        }
        if self.bind_source(src.texture, src.mip).is_err() {
            self.fail(LossReason::OutOfMemory);
            return true;
        }
        if FORMAT_FACTS[from as usize].depth_stencil {
            self.aspect(src.texture, false);
        }
        let rect = [
            dst.origin.x as i32,
            dst.origin.y as i32,
            extent.width as i32,
            extent.height as i32,
        ];
        for i in 0..extent.depth_or_layers {
            let fb = self.texture_framebuffer(dst.texture, dst.mip, self.read_layer(dst, i));
            let (program, at, unit) = &self.copy_programs[&key];
            // SAFETY: plain GL calls; the program is alive.
            unsafe {
                self.gl.use_program(Some(*program));
                self.gl.uniform_3_i32(
                    Some(at),
                    src.origin.x as i32 - rect[0],
                    src.origin.y as i32 - rect[1],
                    self.read_layer(src, i) as i32,
                );
                if let Some(u) = convert::depth_unit(to) {
                    self.gl.uniform_2_f32(unit.as_ref(), u[0], u[1]);
                }
            }
            if FORMAT_FACTS[to as usize].depth_stencil {
                self.own_depth_draw_into(fb, rect);
            } else {
                self.own_draw_into(fb, rect);
            }
        }
        true
    }

    /// Whether a copy reads and writes one subresource (layers of a level that both regions cover,
    /// or one level of a 3D texture, whose slices GL copies into as one image), which every GL
    /// route refuses as a feedback loop.
    fn within_one_subresource(
        &self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> bool {
        let (s, d, n) = (src.layer, dst.layer, extent.depth_or_layers);
        Rc::ptr_eq(&src.texture.0, &dst.texture.0)
            && src.mip == dst.mip
            && (src.texture.desc.dimension == TextureDimension::D3 || (s < d + n && d < s + n))
    }

    /// A copy within one subresource, through a buffer holding the source region
    /// ([`SlowPath::CopyWithinTexture`]): a copy into the buffer, then one out of it, aspect by
    /// aspect for a format with depth and stencil. Formats GL does not read back (16-bit floats)
    /// are held in a 2D texture instead.
    fn copy_within(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.record_slow_path(SlowPath::CopyWithinTexture);
        let format = src.texture.desc.format;
        if !self.caps.formats.uses[format as usize].contains(FormatUse::COPY_OUT) {
            let d = &src.texture.desc;
            let Ok(held) = self.create_texture(&TextureDesc {
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
            }) else {
                return self.fail(LossReason::OutOfMemory);
            };
            let at = TextureRegion {
                texture: &held,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            };
            self.copy_texture(src, &at, extent);
            self.copy_texture(&at, dst, extent);
            return self.destroy(Object::Texture(held));
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
            let Ok(held) = self.create_buffer(&BufferDesc {
                size,
                usage: BufferUsage::COPY_SRC | BufferUsage::COPY_DST,
                intent: MapIntent::Rarely,
            }) else {
                return self.fail(LossReason::OutOfMemory);
            };
            let at = CopyBufferTexture {
                buffer: &held,
                offset: 0,
                row_pitch: cols * bytes,
                rows_per_layer: rows,
            };
            self.copy_texture_to_buffer(&TextureRegion { stencil, ..*src }, &at, extent);
            self.copy_buffer_to_texture(&at, &TextureRegion { stencil, ..*dst }, extent);
            self.destroy(Object::Buffer(held));
        }
    }

    /// Copies a region of a texture into one of the same format by `blitFramebuffer`, layer by layer:
    /// a depth or depth-stencil one, or a float one where blits keep every bit.
    fn blit_same(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let mask = match FORMAT_FACTS[src.texture.desc.format as usize].kind {
            FormatKind::DepthStencil => glow::DEPTH_BUFFER_BIT | glow::STENCIL_BUFFER_BIT,
            FormatKind::Depth => glow::DEPTH_BUFFER_BIT,
            FormatKind::Stencil => glow::STENCIL_BUFFER_BIT,
            _ => glow::COLOR_BUFFER_BIT,
        };
        self.open_writes();
        let (sx, sy, dx, dy, w, h) = (
            src.origin.x as i32,
            src.origin.y as i32,
            dst.origin.x as i32,
            dst.origin.y as i32,
            extent.width as i32,
            extent.height as i32,
        );
        for i in 0..extent.depth_or_layers {
            self.read_framebuffer(src.texture, src.mip, self.read_layer(src, i));
            let fb = self.texture_framebuffer(dst.texture, dst.mip, self.read_layer(dst, i));
            // SAFETY: plain GL calls; both framebuffers hold one level and layer of a texture of the
            // same format.
            unsafe {
                self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb));
                self.gl.blit_framebuffer(
                    sx,
                    sy,
                    sx + w,
                    sy + h,
                    dx,
                    dy,
                    dx + w,
                    dy + h,
                    mask,
                    glow::NEAREST,
                );
            }
            self.applied.draw_framebuffer = Some(fb);
        }
    }

    /// Opens the write masks and the scissor for a clear or a copy, which obey them, and makes the next
    /// draw apply its blocks again.
    fn open_writes(&mut self) {
        let (gl, v) = (&self.gl, &mut self.values);
        // SAFETY: plain GL state calls.
        unsafe {
            if v.scissor_test {
                gl.disable(glow::SCISSOR_TEST);
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
        }
        state::open_masks(gl, v);
        self.applied.blend = 0;
        self.applied.depth_stencil = 0;
        self.applied.raster = 0;
    }

    fn framebuffer(
        &mut self,
        key: FramebufferKey,
        views: &[Option<&TextureView>],
    ) -> glow::Framebuffer {
        if let Some(fb) = self.framebuffers.get(&key) {
            return *fb;
        }
        let gl = &self.gl;
        // SAFETY: plain GL calls on the current context; the views' textures are alive.
        let fb = unsafe {
            let fb = gl.create_framebuffer().expect("a framebuffer object");
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb));
            let mut draw_buffers = [glow::NONE; MAX_COLOR_TARGETS];
            for (i, view) in views.iter().enumerate() {
                let (Some(view), Some((_, mip, layer))) = (view, key[i]) else {
                    continue;
                };
                let t = &view.texture;
                let point = if i < MAX_COLOR_TARGETS {
                    draw_buffers[i] = glow::COLOR_ATTACHMENT0 + i as u32;
                    glow::COLOR_ATTACHMENT0 + i as u32
                } else {
                    match FORMAT_FACTS[t.desc.format as usize].kind {
                        FormatKind::DepthStencil => glow::DEPTH_STENCIL_ATTACHMENT,
                        FormatKind::Stencil => glow::STENCIL_ATTACHMENT,
                        _ => glow::DEPTH_ATTACHMENT,
                    }
                };
                match (t.storage, t.target) {
                    (Storage::Samples(rb), _) => gl.framebuffer_renderbuffer(
                        glow::DRAW_FRAMEBUFFER,
                        point,
                        glow::RENDERBUFFER,
                        Some(rb),
                    ),
                    (Storage::Texture(raw), glow::TEXTURE_2D) => gl.framebuffer_texture_2d(
                        glow::DRAW_FRAMEBUFFER,
                        point,
                        glow::TEXTURE_2D,
                        Some(raw),
                        mip as i32,
                    ),
                    (Storage::Texture(raw), glow::TEXTURE_CUBE_MAP) => gl.framebuffer_texture_2d(
                        glow::DRAW_FRAMEBUFFER,
                        point,
                        glow::TEXTURE_CUBE_MAP_POSITIVE_X + layer,
                        Some(raw),
                        mip as i32,
                    ),
                    (Storage::Texture(raw), _) => gl.framebuffer_texture_layer(
                        glow::DRAW_FRAMEBUFFER,
                        point,
                        Some(raw),
                        mip as i32,
                        layer as i32,
                    ),
                }
            }
            let last = draw_buffers
                .iter()
                .rposition(|b| *b != glow::NONE)
                .map_or(0, |i| i + 1);
            gl.draw_buffers(&draw_buffers[..last.max(1)]);
            if gl.check_framebuffer_status(glow::DRAW_FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                self.lost.get_or_insert(LossReason::Error);
            }
            fb
        };
        self.applied.draw_framebuffer = Some(fb);
        self.framebuffers.insert(key, fb);
        fb
    }

    /// A pass without attachments: its own renderbuffer of the pass's size and samples, with no draw
    /// buffer, so draws rasterise (for queries and depth-less tests) and write nothing.
    fn begin_empty_pass(&mut self, empty: EmptyPass, occlusion: Option<QuerySet>) {
        let gl = &self.gl;
        let fb = match self.empty_passes.get(&empty) {
            Some(fb) => *fb,
            // SAFETY: plain GL calls on the current context.
            None => unsafe {
                let rb = gl.create_renderbuffer().expect("a renderbuffer object");
                gl.bind_renderbuffer(glow::RENDERBUFFER, Some(rb));
                let (w, h) = (empty.width as i32, empty.height as i32);
                if empty.samples > 1 {
                    gl.renderbuffer_storage_multisample(
                        glow::RENDERBUFFER,
                        empty.samples as i32,
                        glow::R8,
                        w,
                        h,
                    );
                } else {
                    gl.renderbuffer_storage(glow::RENDERBUFFER, glow::R8, w, h);
                }
                let fb = gl.create_framebuffer().expect("a framebuffer object");
                gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb));
                gl.framebuffer_renderbuffer(
                    glow::DRAW_FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::RENDERBUFFER,
                    Some(rb),
                );
                gl.draw_buffers(&[glow::NONE]);
                if gl.check_framebuffer_status(glow::DRAW_FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE
                {
                    self.lost.get_or_insert(LossReason::Error);
                }
                self.applied.draw_framebuffer = Some(fb);
                self.empty_passes.insert(empty, fb);
                fb
            },
        };
        let mut discard = Points::default();
        discard.push(glow::COLOR_ATTACHMENT0);
        // SAFETY: plain GL calls on a live framebuffer.
        unsafe {
            if self.applied.draw_framebuffer != Some(fb) {
                gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb));
                self.applied.draw_framebuffer = Some(fb);
            }
            gl.invalidate_framebuffer(glow::DRAW_FRAMEBUFFER, discard.get());
        }
        self.pass = Some(Pass {
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
    fn attachment(view: &TextureView, slice: u32) -> Attachment {
        let layer = if view.texture.desc.dimension == TextureDimension::D3 {
            slice
        } else {
            view.desc.base_layer
        };
        (view.texture.id, view.desc.base_mip, layer)
    }

    /// Binds a framebuffer holding one level and layer of `texture` for reading.
    fn read_framebuffer(&mut self, texture: &Texture, mip: u32, layer: u32) {
        let fb = self.texture_framebuffer(texture, mip, layer);
        if self.applied.read_framebuffer != Some(fb) {
            // SAFETY: a plain GL call.
            unsafe { self.gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fb)) };
            self.applied.read_framebuffer = Some(fb);
        }
    }

    /// The framebuffer holding one level and layer of `texture` alone, made if new; the draw binding
    /// stays as it was.
    fn texture_framebuffer(
        &mut self,
        texture: &Texture,
        mip: u32,
        layer: u32,
    ) -> glow::Framebuffer {
        self.own_framebuffer(&[(texture, mip, layer)])
    }

    /// The framebuffer of the back end's own holding `parts` (each a texture, level and layer; colour
    /// ones on targets 0, 1 and on, a depth or stencil one on its point), made if new; the draw binding
    /// stays as it was.
    fn own_framebuffer(&mut self, parts: &[(&Texture, u32, u32)]) -> glow::Framebuffer {
        let mut key: FramebufferKey = [None; MAX_COLOR_TARGETS + 1];
        let mut views: [Option<TextureView>; MAX_COLOR_TARGETS + 1] = Default::default();
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
        let draw = self.applied.draw_framebuffer;
        let fb = self.framebuffer(key, &views);
        if self.applied.draw_framebuffer != draw {
            // SAFETY: a plain GL call; the draw binding is put back as it was.
            unsafe { self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, draw) };
            self.applied.draw_framebuffer = draw;
        }
        fb
    }

    /// Draws the depth (or with `region.stencil` the stencil) of a region into a new `r32uint` texture
    /// of the region's size and layers, which reads back as the stored bytes
    /// ([`SlowPath::DepthReadByDraw`]).
    fn depth_by_draw(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
    ) -> Result<Texture, GpuError> {
        self.fresh(region.texture);
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
            glow::TEXTURE_2D => Fetch::Depth2D,
            glow::TEXTURE_2D_ARRAY => Fetch::DepthArray,
            _ => Fetch::DepthCube,
        };
        // The stencil of a cube sampled through WEBGL_stencil_texturing reads 0 in Chrome 157 on the
        // RTX 3080, so cubes count their stencil bits as contexts without the extension do.
        let bits = region.stencil
            && (!self.extensions.contains("WEBGL_stencil_texturing")
                || matches!(kind, Fetch::DepthCube));
        let kind = match kind {
            _ if bits => Fetch::Depth2D,
            Fetch::Depth2D if region.stencil => Fetch::Stencil2D,
            Fetch::DepthArray if region.stencil => Fetch::StencilArray,
            k => k,
        };
        self.record_slow_path(SlowPath::DepthReadByDraw);
        let mut scratch = self.create_texture(&TextureDesc {
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
        })?;
        Rc::get_mut(&mut scratch.0)
            .expect("a texture just made has one handle")
            .read = Some(format::ReadAs {
            format: glow::RGBA_INTEGER,
            ty: glow::UNSIGNED_INT,
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
            let fb = self.texture_framebuffer(&scratch, 0, i);
            let size = (extent.width as i32, extent.height as i32);
            if bits {
                let counted = self.stencil_bits(&src, region.mip, layer)?;
                self.fetch(kind, &counted, 0, [at[0], at[1], 0], 255, fb, size)?;
                self.destroy(Object::Texture(counted));
            } else {
                self.fetch(
                    kind,
                    &src,
                    region.mip,
                    [at[0], at[1], layer as i32],
                    scale,
                    fb,
                    size,
                )?;
            }
        }
        Ok(scratch)
    }

    /// Draws a full-screen triangle into `target` (`size` pixels) whose fragments fetch level `mip` of
    /// `src` at their position moved by `at` (x, y, layer) and write the fetched value as an unsigned
    /// integer: a depth times `scale` (0 keeps a float's bits), or a stencil value. Drawn with GL's own
    /// lower-left origin, so rows match on both window-origin routes.
    #[allow(clippy::too_many_arguments)]
    fn fetch(
        &mut self,
        kind: Fetch,
        src: &Texture,
        mip: u32,
        at: [i32; 3],
        scale: u32,
        target: glow::Framebuffer,
        size: (i32, i32),
    ) -> Result<(), GpuError> {
        let k = kind as usize;
        if self.depth_read[k].is_none() {
            match depth::build(&self.gl, kind.shape(), kind.stencil()) {
                Ok(p) => self.depth_read[k] = Some(p),
                Err(log) => return Err(self.own_program_failed(log)),
            }
        }
        self.bind_source(src, mip)?;
        let stencil = kind.stencil();
        self.aspect(src, stencil);
        let program = self.depth_read[k].as_ref().expect("built above");
        // SAFETY: plain GL calls; the program is alive.
        unsafe {
            self.gl.use_program(Some(program.program));
            if let Some(location) = &program.scale {
                self.gl.uniform_1_u32(Some(location), scale);
            }
            self.gl
                .uniform_3_i32(Some(&program.at), at[0], at[1], at[2]);
        }
        self.own_draw_into(target, [0, 0, size.0, size.1]);
        Ok(())
    }

    /// Binds level `mip` of `src` alone on unit 0 for the back end's own fetches, through a nearest
    /// sampler (GLES leaves a depth texture without comparison incomplete under linear filters).
    fn bind_source(&mut self, src: &Texture, mip: u32) -> Result<(), GpuError> {
        let Some(raw) = src.tex() else {
            return Err(self.unbuilt("fetches of multisampled textures"));
        };
        self.bind_scratch(src.target, raw);
        let sampler = self.point_sampler()?;
        let gl = &self.gl;
        // SAFETY: plain GL calls on the bound texture.
        unsafe {
            if src.levels.get() != (mip, mip) {
                gl.tex_parameter_i32(src.target, glow::TEXTURE_BASE_LEVEL, mip as i32);
                gl.tex_parameter_i32(src.target, glow::TEXTURE_MAX_LEVEL, mip as i32);
                src.levels.set((mip, mip));
            }
            gl.bind_sampler(0, Some(sampler));
        }
        // The next draw binds its own sampler on unit 0 again.
        self.applied.units[0].1 = Id::MAX;
        Ok(())
    }

    /// Draws the back end's full-screen triangle with the program in use into `target`, in the
    /// viewport `rect` (GL's window coordinates, row 0 the texture's first), with no blending, tests or
    /// culling; the next draw applies its own state.
    fn own_draw_into(&mut self, target: glow::Framebuffer, rect: [i32; 4]) {
        // SAFETY: a plain GL call; the framebuffer is alive.
        unsafe {
            self.gl
                .bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(target))
        };
        self.applied.draw_framebuffer = Some(target);
        self.applied.program = 0;
        self.open_writes();
        state::plain(&self.gl, &mut self.values, rect, &self.platform);
        self.own_draw();
    }

    /// As [`Gl::own_draw_into`], for a program writing depth into the depth attachment of
    /// `target`: the depth test passes always and writes, in the full depth range; stencil stays.
    fn own_depth_draw_into(&mut self, target: glow::Framebuffer, rect: [i32; 4]) {
        // SAFETY: a plain GL call; the framebuffer is alive.
        unsafe {
            self.gl
                .bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(target))
        };
        self.applied.draw_framebuffer = Some(target);
        self.applied.program = 0;
        self.open_writes();
        state::plain(&self.gl, &mut self.values, rect, &self.platform);
        let (gl, v) = (&self.gl, &mut self.values);
        // SAFETY: plain GL state calls, recorded in the values so the next draw sets its own.
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            v.depth_test = true;
            if v.depth_func != glow::ALWAYS {
                gl.depth_func(glow::ALWAYS);
                v.depth_func = glow::ALWAYS;
            }
            if v.depth_range != [0.0, 1.0] {
                gl.depth_range_f32(0.0, 1.0);
                v.depth_range = [0.0, 1.0];
            }
        }
        self.own_draw();
    }

    /// Writes depth or stencil texels, from the host or from the bound pixel unpack buffer, by draws:
    /// into an integer texture of the aspect's bytes first, from which a draw writes the depth
    /// ([`SlowPath::DepthWrittenByDraw`]) or draws write the stencil bit by bit. GL takes neither
    /// aspect of a combined format alone, nor 24-bit depth as Direct3D stores it, from bytes.
    fn write_depth(
        &mut self,
        dst: &TextureRegion<'_, Self>,
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
        let Ok(scratch) = self.create_texture(&TextureDesc {
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
        }) else {
            return self.fail(LossReason::OutOfMemory);
        };
        let src = TextureRegion {
            texture: &scratch,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        self.upload(&src, extent, texels, row_pitch, rows_per_layer);
        if dst.stencil {
            self.stencil_by_draw(&src, dst, extent);
        } else if self.copy_by_draw(&src, dst, extent) {
            self.record_slow_path(SlowPath::DepthWrittenByDraw);
        } else {
            self.unbuilt("depth written into textures of this format");
        }
        self.destroy(Object::Texture(scratch));
    }

    /// Writes the stencil of `dst` from the `r8uint` texels of `src` (2D or 2D array), layer by
    /// layer: one draw sets the region's stencil to 0, then one per bit sets that bit where the
    /// source has it ([`SlowPath::StencilWrittenByDraw`]). Depth stays.
    fn stencil_by_draw(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.record_slow_path(SlowPath::StencilWrittenByDraw);
        let k = usize::from(src.texture.target == glow::TEXTURE_2D_ARRAY);
        if self.stencil_write[k].is_none() {
            match depth::build_stencil_write(&self.gl, k == 1) {
                Ok(p) => self.stencil_write[k] = Some(p),
                Err(log) => {
                    self.own_program_failed(log);
                    return;
                }
            }
        }
        if self.bind_source(src.texture, src.mip).is_err() {
            return self.fail(LossReason::OutOfMemory);
        }
        let rect = [
            dst.origin.x as i32,
            dst.origin.y as i32,
            extent.width as i32,
            extent.height as i32,
        ];
        for i in 0..extent.depth_or_layers {
            let fb = self.texture_framebuffer(dst.texture, dst.mip, self.read_layer(dst, i));
            // SAFETY: a plain GL call; the framebuffer is alive.
            unsafe { self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb)) };
            self.applied.draw_framebuffer = Some(fb);
            self.applied.program = 0;
            self.open_writes();
            state::plain(&self.gl, &mut self.values, rect, &self.platform);
            let program = self.stencil_write[k].as_ref().expect("built above");
            let (gl, v) = (&self.gl, &mut self.values);
            let replace = [glow::KEEP, glow::KEEP, glow::REPLACE];
            // SAFETY: plain GL calls, recorded in the values so the next draw sets its own.
            unsafe {
                gl.use_program(Some(program.program));
                gl.uniform_3_i32(
                    Some(&program.at),
                    src.origin.x as i32 - rect[0],
                    src.origin.y as i32 - rect[1],
                    (src.layer + i) as i32,
                );
                gl.enable(glow::STENCIL_TEST);
                v.stencil_test = true;
                gl.stencil_op(replace[0], replace[1], replace[2]);
                v.stencil_op = [replace; 2];
            }
            // Bit 0 of the loop is the clear: no fragment discarded, 0 written through every bit.
            for (bit, reference, mask) in
                std::iter::once((0, 0, 0xff)).chain((0..8).map(|b| (1u32 << b, 0xff, 1u8 << b)))
            {
                let program = self.stencil_write[k].as_ref().expect("built above");
                // SAFETY: plain GL state calls.
                unsafe {
                    self.gl.uniform_1_u32(Some(&program.bit), bit);
                    self.gl.stencil_func(glow::ALWAYS, reference, 0xff);
                    self.gl.stencil_mask(u32::from(mask));
                }
                self.values.stencil_func = [(glow::ALWAYS, reference as u8, 0xff); 2];
                self.values.stencil_write_mask = mask;
                self.own_draw();
            }
        }
    }

    /// Draws the back end's full-screen triangle with GL's lower-left window origin.
    fn own_draw(&mut self) {
        if !self.glsl.flip_y {
            self.platform
                .clip_control(&self.gl, glow::LOWER_LEFT, self.values.clip_depth);
        }
        // SAFETY: the caller bound the program, the target and the state.
        unsafe { self.gl.draw_arrays(glow::TRIANGLES, 0, 3) };
        if !self.glsl.flip_y {
            self.platform
                .clip_control(&self.gl, glow::UPPER_LEFT, self.values.clip_depth);
        }
    }

    /// One of the back end's own programs failed to build: a bug of ours, so the device is lost with
    /// GL's log.
    fn own_program_failed(&mut self, log: String) -> GpuError {
        self.last_log = log;
        self.fail(LossReason::Error);
        GpuError::DeviceLost
    }

    /// A nearest-filtering sampler for the back end's own fetches and for textures read only by
    /// loads, made at its first use.
    fn point_sampler(&mut self) -> Result<glow::Sampler, GpuError> {
        if let Some(s) = self.point_sampler {
            return Ok(s);
        }
        // SAFETY: plain GL calls on the current context.
        let s = unsafe {
            let s = self
                .gl
                .create_sampler()
                .map_err(|_| GpuError::OutOfMemory)?;
            // A mipmap filter: texel fetches reach only the base level without one.
            self.gl.sampler_parameter_i32(
                s,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST_MIPMAP_NEAREST as i32,
            );
            self.gl
                .sampler_parameter_i32(s, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
            s
        };
        self.point_sampler = Some(s);
        Ok(s)
    }

    /// Makes `texture`, bound on the active unit, give its stencil (`stencil`) or its depth to programs
    /// (WEBGL_stencil_texturing's mode, which is the texture's, not a view's).
    fn aspect(&mut self, texture: &TextureObj, stencil: bool) {
        if texture.stencil_mode.get() != stencil {
            let mode = if stencil {
                glow::STENCIL_INDEX
            } else {
                glow::DEPTH_COMPONENT
            };
            // SAFETY: a plain GL call on the bound texture.
            unsafe {
                self.gl.tex_parameter_i32(
                    texture.target,
                    glow::DEPTH_STENCIL_TEXTURE_MODE,
                    mode as i32,
                )
            };
            texture.stencil_mode.set(stencil);
        }
    }

    /// Counts the stencil of level `mip` and layer `layer` of `src` into a new `r8unorm` texture of the
    /// level's size, without WEBGL_stencil_texturing: eight full-screen draws, each passing the stencil
    /// test only where its bit is set and adding the bit's weight by blending with the constant colour,
    /// which unorm8 blending sums exactly ([`SlowPath::StencilCopiedByDraw`]).
    fn stencil_bits(&mut self, src: &Texture, mip: u32, layer: u32) -> Result<Texture, GpuError> {
        self.record_slow_path(SlowPath::StencilCopiedByDraw);
        if self.bits_program.is_none() {
            match depth::build_bits(&self.gl) {
                Ok(p) => self.bits_program = Some(p),
                Err(log) => return Err(self.own_program_failed(log)),
            }
        }
        let (w, h) = (
            (src.desc.width >> mip).max(1),
            (src.desc.height >> mip).max(1),
        );
        let counted = self.create_texture(&TextureDesc {
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
        })?;
        let fb = self.own_framebuffer(&[(&counted, 0, 0), (src, mip, layer)]);
        // SAFETY: plain GL calls; the framebuffer holds the counted texture and the source's stencil.
        unsafe {
            self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb));
            self.gl.use_program(self.bits_program);
        }
        self.applied.draw_framebuffer = Some(fb);
        self.applied.program = 0;
        self.open_writes();
        state::plain(
            &self.gl,
            &mut self.values,
            [0, 0, w as i32, h as i32],
            &self.platform,
        );
        let (gl, v) = (&self.gl, &mut self.values);
        let keep = [glow::KEEP; 3];
        // SAFETY: plain GL state calls, recorded in the values so the next draw sets its own.
        unsafe {
            gl.clear_buffer_f32_slice(glow::COLOR, 0, &[0.0; 4]);
            let add = state::GlTarget {
                on: true,
                func: [
                    glow::CONSTANT_COLOR,
                    glow::ONE,
                    glow::CONSTANT_COLOR,
                    glow::ONE,
                ],
                ..state::GlTarget::OFF
            };
            state::uniform(gl, v, &add);
            gl.enable(glow::STENCIL_TEST);
            v.stencil_test = true;
            gl.stencil_op(keep[0], keep[1], keep[2]);
            v.stencil_op = [keep; 2];
        }
        for bit in 0..8 {
            let weight = f32::from(1u8 << bit) / 255.0;
            let func = (glow::EQUAL, 1u8 << bit, 1u8 << bit);
            // SAFETY: plain GL state calls.
            unsafe {
                self.gl
                    .stencil_func(func.0, i32::from(func.1), u32::from(func.2));
                self.gl.blend_color(weight, weight, weight, weight);
            }
            self.values.stencil_func = [func; 2];
            self.values.blend_color = [weight; 4];
            self.own_draw();
        }
        Ok(counted)
    }

    /// Binds a texture on unit 0 for an upload or a parameter change. The next draw walks its bindings
    /// again, so unit 0 gets back what its program samples there.
    fn bind_scratch(&mut self, target: u32, raw: glow::Texture) {
        if self.applied.units.is_empty() {
            self.applied.units.push((0, 0));
        }
        self.applied.units[0] = (0, 0);
        self.applied.sets = [Id::MAX; MAX_BINDING_SETS];
        // SAFETY: plain GL calls on the current context.
        unsafe {
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(target, Some(raw));
        }
    }

    /// Writes texels from the host or from a buffer (bound as the pixel unpack buffer) into a region.
    /// A region's first texel and slice (layer, or depth slice in 3D), as the host copy counts them.
    fn corner(region: &TextureRegion<'_, Self>) -> [u32; 3] {
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
    fn copy_host_kept(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let (sbw, sbh, bytes) = FORMAT_FACTS[src.texture.desc.format as usize].block;
        let (dbw, dbh, dst_bytes) = FORMAT_FACTS[dst.texture.desc.format as usize].block;
        if bytes != dst_bytes {
            return self.invalid(format!(
                "a copy from {:?} into {:?}, whose blocks differ in size",
                src.texture.desc.format, dst.texture.desc.format
            ));
        }
        let (cols, rows) = (extent.width.div_ceil(sbw), extent.height.div_ceil(sbh));
        let mut packed = vec![0; (cols * rows * bytes * extent.depth_or_layers) as usize];
        if src.texture.host.is_none() {
            self.record_slow_path(SlowPath::HostCopyReadBack);
        }
        if self.read_texture_now(src, extent, &mut packed).is_err() {
            self.unbuilt("copies into textures kept on the host from formats GL cannot read back");
            return;
        }
        let into = Extent {
            width: cols * dbw,
            height: rows * dbh,
            depth_or_layers: extent.depth_or_layers,
        };
        self.write_texture(dst, into, &packed, cols * bytes, rows, WriteMode::Ordered);
    }

    /// Before the levels of `levels` (by bit) of the host copy of `t` are read: where draws wrote
    /// their texels since (signed normalised formats with EXT_render_snorm), each is drawn into a
    /// texture of its signed integer twin, which reads back as stored, and written into the host
    /// copy ([`SlowPath::HostCopyReadBack`]). A draw reads the smallest value as the one above it,
    /// so in a level whose host copy may hold it, the texels are not recovered.
    fn host_current(&mut self, t: &Texture, levels: u32) -> Result<(), GpuError> {
        let drawn = t.drawn.get() & levels;
        if drawn == 0 {
            return Ok(());
        }
        if drawn & t.minimum.get() != 0 {
            return Err(self.unbuilt(
                "the bits of signed normalised texels drawn into after a write of their smallest value",
            ));
        }
        let Some(h) = &t.host else { return Ok(()) };
        self.record_slow_path(SlowPath::HostCopyReadBack);
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
            let held = self.create_texture(&TextureDesc {
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
            })?;
            let region = |texture, mip| TextureRegion {
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
            let read = match self.copy_by_draw(&region(t, mip), &region(&held, 0), extent) {
                true => self.read_texture_now(&region(&held, 0), extent, &mut bytes),
                false => Err(self.unbuilt("draws from this signed normalised format")),
            };
            self.destroy(Object::Texture(held));
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
    fn upload_blocks(&mut self, region: &TextureRegion<'_, Self>, extent: Extent) {
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
            Self::corner(region),
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
            return self.upload(
                region,
                extent,
                Texels::Bytes(&texels),
                wide as u32 * bw * texel,
                high as u32 * bh,
            );
        }
        let Some(raw) = t.tex() else { return };
        self.bind_scratch(t.target, raw);
        let gl = &self.gl;
        let (x, y, level) = (
            region.origin.x as i32,
            region.origin.y as i32,
            region.mip as i32,
        );
        let slice = wide * high * block_bytes as usize;
        // SAFETY: plain GL calls; `blocks` holds every block of the region, tightly packed.
        unsafe {
            match t.target {
                glow::TEXTURE_2D => gl.compressed_tex_sub_image_2d(
                    t.target,
                    level,
                    x,
                    y,
                    w as i32,
                    ht as i32,
                    t.gl.internal,
                    glow::CompressedPixelUnpackData::Slice(&blocks),
                ),
                glow::TEXTURE_CUBE_MAP => {
                    for (i, face) in blocks.chunks_exact(slice).enumerate() {
                        gl.compressed_tex_sub_image_2d(
                            glow::TEXTURE_CUBE_MAP_POSITIVE_X + region.layer + i as u32,
                            level,
                            x,
                            y,
                            w as i32,
                            ht as i32,
                            t.gl.internal,
                            glow::CompressedPixelUnpackData::Slice(face),
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
                    glow::CompressedPixelUnpackData::Slice(&blocks),
                ),
            }
        }
    }

    fn upload(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        texels: Texels<'_>,
        row_pitch: u32,
        rows_per_layer: u32,
    ) {
        let t = region.texture.clone();
        let (_, _, block_bytes) = FORMAT_FACTS[t.decoded.unwrap_or(t.desc.format) as usize].block;
        let Some(raw) = t.tex() else {
            return self.invalid("texels written into a multisampled texture".into());
        };
        self.bind_scratch(t.target, raw);
        let gl = &self.gl;
        let fmt = t.gl;
        let (x, y, w, h) = (
            region.origin.x as i32,
            region.origin.y as i32,
            extent.width as i32,
            extent.height as i32,
        );
        let layer_bytes = row_pitch as usize * rows_per_layer as usize;
        let from = |i: usize| match texels {
            Texels::Bytes(data) => glow::PixelUnpackData::Slice(Some(&data[i * layer_bytes..])),
            Texels::Offset(offset) => {
                glow::PixelUnpackData::BufferOffset(offset + (i * layer_bytes) as u32)
            }
        };
        // SAFETY: plain GL calls; the source holds every row the region reads, by the interface's rule.
        unsafe {
            gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, (row_pitch / block_bytes) as i32);
            gl.pixel_store_i32(glow::UNPACK_IMAGE_HEIGHT, rows_per_layer as i32);
            let level = region.mip as i32;
            match t.target {
                glow::TEXTURE_2D => {
                    gl.tex_sub_image_2d(t.target, level, x, y, w, h, fmt.format, fmt.ty, from(0))
                }
                glow::TEXTURE_CUBE_MAP => {
                    for i in 0..extent.depth_or_layers {
                        gl.tex_sub_image_2d(
                            glow::TEXTURE_CUBE_MAP_POSITIVE_X + region.layer + i,
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
                    let z = if t.target == glow::TEXTURE_3D {
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
    }

    /// Calls `read` on a region of a texture GL cannot read, drawn into a colour texture it can:
    /// depth as colour ([`SlowPath::DepthReadByDraw`]), 16-bit and packed floats as the bits of
    /// their unsigned integer twin ([`SlowPath::CopyThroughBuffer`]); None for other textures. Then lets the
    /// colour texture go.
    fn through_draw<R>(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        read: impl FnOnce(&mut Self, &TextureRegion<'_, Self>) -> Result<R, GpuError>,
    ) -> Option<Result<R, GpuError>> {
        let format = region.texture.desc.format;
        let scratch = if depth::stored(format).is_some() {
            self.depth_by_draw(region, extent)
        } else if let Some(twin) = format::bits_twin(format)
            && region.texture.host.is_none()
        {
            self.bits_by_draw(region, extent, twin)
        } else {
            return None;
        };
        Some(scratch.and_then(|scratch| {
            let colour = TextureRegion {
                texture: &scratch,
                mip: 0,
                layer: 0,
                origin: Origin::default(),
                stencil: false,
            };
            let r = read(self, &colour);
            self.destroy(Object::Texture(scratch.clone()));
            r
        }))
    }

    /// A region's bits drawn into a new 2D texture of `twin`, an integer format of the same bits.
    fn bits_by_draw(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        twin: Format,
    ) -> Result<Texture, GpuError> {
        let held = self.create_texture(&TextureDesc {
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
        })?;
        let at = TextureRegion {
            texture: &held,
            mip: 0,
            layer: 0,
            origin: Origin::default(),
            stencil: false,
        };
        if !self.copy_by_draw(region, &at, extent) {
            self.destroy(Object::Texture(held));
            return Err(self.unbuilt("reading back this format"));
        }
        self.record_slow_path(SlowPath::CopyThroughBuffer);
        Ok(held)
    }

    /// Reads one region of a colour texture into `dst`, rows tightly packed, now.
    fn read_texture_now(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        dst: &mut [u8],
    ) -> Result<(), GpuError> {
        if let Some(h) = &region.texture.host {
            self.host_current(region.texture, 1 << region.mip)?;
            let corner = Self::corner(region);
            let size = [extent.width, extent.height, extent.depth_or_layers];
            h.read(&region.texture.desc, region.mip, corner, size, dst);
            return Ok(());
        }
        if let Some(r) = self.through_draw(region, extent, |b, colour| {
            b.read_texture_now(colour, extent, dst)
        }) {
            return r;
        }
        self.fresh(region.texture);
        let t = region.texture;
        let read = t.read.ok_or(GpuError::Invalid(
            "webgl: the format cannot be read back here",
        ))?;
        let texels = (extent.width * extent.height) as usize;
        let mut wide = Vec::new();
        if read.repacks() {
            wide.resize(texels * read.texel_read(), 0);
        }
        if read.converts() {
            self.record_slow_path(SlowPath::FormatConverted);
        }
        for i in 0..extent.depth_or_layers as usize {
            let layer = self.read_layer(region, i as u32);
            self.read_framebuffer(t, region.mip, layer);
            let out =
                &mut dst[i * texels * read.texel_kept()..(i + 1) * texels * read.texel_kept()];
            let into = if wide.is_empty() {
                &mut *out
            } else {
                &mut wide[..]
            };
            // SAFETY: the read framebuffer holds the region's level and layer; `into` fits the rows.
            unsafe {
                self.gl.read_pixels(
                    region.origin.x as i32,
                    region.origin.y as i32,
                    extent.width as i32,
                    extent.height as i32,
                    read.format,
                    read.ty,
                    glow::PixelPackData::Slice(Some(into)),
                );
            }
            if !wide.is_empty() {
                read.repack(&wide, out);
            }
        }
        // readPixels has waited for the GPU; an error it shows leaves the bytes unread.
        let lost = self.lost.is_some();
        self.gl_errors();
        if !lost && self.lost.is_some() {
            return Err(GpuError::DeviceLost);
        }
        Ok(())
    }

    /// The texture layer, or 3D slice, of layer `i` of a region.
    fn read_layer(&self, region: &TextureRegion<'_, Self>, i: u32) -> u32 {
        if region.texture.desc.dimension == TextureDimension::D3 {
            region.origin.z + i
        } else {
            region.layer + i
        }
    }

    /// The GL buffer of `buffer` that a copy or read can reach, brought up to date: the data one, else
    /// the index one.
    fn any_gl(&mut self, buffer: &BufferObj) -> glow::Buffer {
        let index = buffer.data.is_none();
        self.refresh(buffer, Some(index));
        if index { buffer.index } else { buffer.data }.expect("every buffer has a GL buffer")
    }

    /// Brings `buffer`'s stale range up to date from its other GL buffer, if the stale GL buffer is the
    /// index one (`Some(true)`), the data one (`Some(false)`) or either (`None`). Reading back waits
    /// for the GPU ([`SlowPath::IndexTwinReadBack`]).
    fn refresh(&mut self, buffer: &BufferObj, which: Option<bool>) {
        let Some(stale) = buffer.stale.get() else {
            return;
        };
        if which.is_some_and(|i| i != stale.index) {
            return;
        }
        self.record_slow_path(SlowPath::IndexTwinReadBack);
        let (from, to) = if stale.index {
            (buffer.data, buffer.index)
        } else {
            (buffer.index, buffer.data)
        };
        let mut bytes = vec![0; (stale.end - stale.start) as usize];
        // SAFETY: plain GL calls; the copy targets take buffers of either WebGL type.
        unsafe {
            self.gl.bind_buffer(glow::COPY_READ_BUFFER, from);
            if !platform::get_buffer_sub_data(
                &self.gl,
                glow::COPY_READ_BUFFER,
                stale.start as i32,
                &mut bytes,
            ) {
                self.read_failed();
                return;
            }
            self.gl.bind_buffer(glow::COPY_WRITE_BUFFER, to);
            self.gl
                .buffer_sub_data_u8_slice(glow::COPY_WRITE_BUFFER, stale.start as i32, &bytes);
        }
        buffer.stale.set(None);
        self.stale_buffers -= 1;
    }

    /// Marks bytes `start..end` of `buffer`'s index or data GL buffer stale. Its other GL buffer is up
    /// to date there, since every GPU write refreshes its destination first.
    fn mark_stale(&mut self, buffer: &BufferObj, index: bool, start: u64, end: u64) {
        if buffer.stale.get().is_none() {
            self.stale_buffers += 1;
        }
        buffer.stale.set(Some(Stale { index, start, end }));
    }

    /// Brings up to date every range a draw reads: indices from the index GL buffer, streams and
    /// uniform blocks from the data one.
    fn refresh_draw(&mut self, call: &DrawCall<'_, Self>) {
        if let Some(index) = &call.index {
            self.refresh(index.buffer, Some(true));
        }
        for stream in call.vertex_buffers.iter().flatten() {
            self.refresh(stream.buffer, Some(false));
        }
        for set in call.bindings.iter().flatten() {
            for (_, bound) in &set.entries {
                if let Bound::Uniform(buffer, _, _) = bound {
                    self.refresh(buffer, Some(false));
                }
            }
        }
    }

    /// What `program`'s link decided, read at its first draw: `PipelineMiss` when that draw waits for
    /// the link (always without KHR_parallel_shader_compile, which alone can ask without waiting).
    fn linked<'p>(&mut self, program: &'p ProgramObj) -> Option<&'p shader::Linked> {
        if let Some(l) = program.linked.get() {
            return Some(l);
        }
        let parallel = self.extensions.contains("KHR_parallel_shader_compile");
        // SAFETY: a plain query of a live program.
        if !parallel || !unsafe { self.gl.get_program_completion_status(program.raw) } {
            self.record_slow_path(SlowPath::PipelineMiss);
        }
        match shader::finish(&self.gl, program) {
            Ok(l) => {
                // Finishing left the program in use; its bindings are walked afresh.
                self.applied.program = program.id;
                self.applied.sets = [Id::MAX; MAX_BINDING_SETS];
                Some(program.linked.get_or_init(|| l))
            }
            Err(log) => {
                self.last_log = log;
                self.fail(LossReason::Error);
                None
            }
        }
    }

    /// The variant of `program` for a vertex layout whose attributes in `key` GL ES cannot read as
    /// stored, made at the first draw with it ([`SlowPath::VertexFormatConverted`], counted per draw).
    /// None when it could not be made, which has lost the device.
    fn variant(&mut self, program: &ProgramObj, key: u32) -> Option<Rc<ProgramObj>> {
        self.record_slow_path(SlowPath::VertexFormatConverted);
        if let Some((_, v)) = program.variants.borrow().iter().find(|(k, _)| *k == key) {
            return Some(v.clone());
        }
        let id = self.id();
        match shader::relink(&self.gl, id, program, None, key, self.empty_fragment) {
            Ok(v) => {
                let v = Rc::new(v);
                program.variants.borrow_mut().push((key, v.clone()));
                Some(v)
            }
            Err(e) => {
                self.last_log =
                    format!("webgl: a program variant for converted vertex formats: {e:?}");
                self.fail(LossReason::Error);
                None
            }
        }
    }

    /// Binds what the program reads from the draw's sets. Err says why the draw cannot run; nothing of
    /// it may run then.
    fn apply_bindings(
        &mut self,
        program: Id,
        linked: &shader::Linked,
        sets: &[Option<&BindingSet>; MAX_BINDING_SETS],
    ) -> Result<(), Unbindable> {
        let ids = sets.map(|s| s.map_or(0, |s| s.id));
        if self.applied.program == program && self.applied.sets == ids {
            return Ok(());
        }
        // Walked again at the next draw unless this walk completes.
        self.applied.sets = [Id::MAX; MAX_BINDING_SETS];
        let find = |(set, binding): Slot| {
            sets.get(set as usize).copied().flatten().and_then(|s| {
                s.entries
                    .iter()
                    .find(|(b, _)| *b == binding)
                    .map(|(_, r)| r)
            })
        };
        if self.applied.units.len() < linked.units.len() {
            self.applied.units.resize(linked.units.len(), (0, 0));
        }
        // Textures read only by loads get the nearest sampler: GLES leaves integer and depth textures
        // incomplete under the texture's own default filters, and loads ignore filters.
        let point = if linked.units.iter().any(|(_, s, _)| s.is_none()) {
            match self.point_sampler() {
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
            let aspect = FORMAT_FACTS[t.desc.format as usize].depth_stencil
                && t.stencil_mode.get() != v.desc.stencil;
            if self.applied.units[unit] != ids || t.levels.get() != levels || aspect {
                // SAFETY: plain GL calls; the texture is alive.
                unsafe {
                    self.gl.active_texture(glow::TEXTURE0 + unit as u32);
                    self.gl.bind_texture(t.target, Some(raw));
                }
            }
            if aspect {
                self.aspect(t, v.desc.stencil);
            }
            let gl = &self.gl;
            // SAFETY: plain GL calls; the texture and sampler are alive.
            unsafe {
                if t.levels.get() != levels {
                    gl.tex_parameter_i32(t.target, glow::TEXTURE_BASE_LEVEL, levels.0 as i32);
                    gl.tex_parameter_i32(t.target, glow::TEXTURE_MAX_LEVEL, levels.1 as i32);
                    t.levels.set(levels);
                }
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
                    // SAFETY: the program is in use.
                    unsafe { gl.uniform_3_i32(Some(location), size[0], size[1], size[2]) };
                    held.set(size);
                }
            }
            if self.applied.units[unit].1 != ids.1 {
                // SAFETY: a plain GL call; the sampler is alive.
                unsafe { gl.bind_sampler(unit as u32, sampler.map(|s| s.1).or(point)) };
            }
            self.applied.units[unit] = ids;
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
                if v.texture.levels.get()
                    != (v.desc.base_mip, v.desc.base_mip + v.desc.mip_count - 1)
                {
                    return Err(Unbindable::TwoMipRanges(*texture));
                }
            }
        }
        if self.applied.blocks.len() < linked.blocks.len() {
            self.applied.blocks.resize(linked.blocks.len(), (0, 0, 0));
        }
        for (point, slot) in linked.blocks.iter().enumerate() {
            let Some(Bound::Uniform(buffer, offset, size)) = find(*slot) else {
                return Err(Unbindable::Missing(*slot));
            };
            let key = (buffer.id, *offset, *size);
            if self.applied.blocks[point] != key {
                // SAFETY: a plain GL call; the buffer is alive.
                unsafe {
                    self.gl.bind_buffer_range(
                        glow::UNIFORM_BUFFER,
                        point as u32,
                        buffer.data,
                        *offset as i32,
                        *size as i32,
                    )
                };
                self.applied.blocks[point] = key;
            }
        }
        self.applied.sets = ids;
        Ok(())
    }

    /// Points each attribute at its stream, moved by the draw's base vertex and first instance
    /// (WebGL 2 has no draws with bases: [`SlowPath::BaseVertexRebind`]), and per-instance streams
    /// on by what `later` instances past the first step through at their rate. Err when a stream's
    /// offset would fall below zero.
    fn apply_vertices(
        &mut self,
        call: &DrawCall<'_, Self>,
        base_vertex: i64,
        first_instance: u32,
        later: u32,
    ) -> Result<(), ()> {
        let layout = &call.vertex_layout.0.1;
        let mut wanted = [None; MAX_VERTEX_STREAMS];
        for (w, a) in wanted.iter_mut().zip(&layout.attributes) {
            let Some(binding) = call.vertex_buffers.get(a.stream).and_then(|b| b.as_ref()) else {
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
            *w = Some((a, Some(binding.buffer), offset));
        }
        let n = layout.attributes.len();
        let wanted = &wanted[..n];
        let changed = self.applied.layout != call.vertex_layout.0.0
            || self.applied.attributes.len() != n
            || wanted
                .iter()
                .flatten()
                .zip(&self.applied.attributes)
                .any(|((_, b, o), applied)| (b.map_or(0, |b| b.id), *o) != *applied);
        if !changed {
            return Ok(());
        }
        let gl = &self.gl;
        let mut enabled = 0u32;
        self.applied.attributes.clear();
        for &(a, buffer, offset) in wanted.iter().flatten() {
            self.applied
                .attributes
                .push((buffer.map_or(0, |b| b.id), offset));
            let Some(buffer) = buffer else { continue };
            enabled |= 1 << a.location;
            // SAFETY: plain GL calls; the buffer is alive and holds other data.
            unsafe {
                gl.bind_buffer(glow::ARRAY_BUFFER, buffer.data);
                let stride = a.stride as i32;
                if a.integer {
                    gl.vertex_attrib_pointer_i32(
                        a.location,
                        a.components,
                        a.ty,
                        stride,
                        offset as i32,
                    );
                } else {
                    gl.vertex_attrib_pointer_f32(
                        a.location,
                        a.components,
                        a.ty,
                        a.normalized,
                        stride,
                        offset as i32,
                    );
                }
                gl.vertex_attrib_divisor(a.location, a.divisor);
            }
        }
        let flip = enabled ^ self.applied.enabled;
        for location in (0..32).filter(|l| flip & (1 << l) != 0) {
            // SAFETY: plain GL calls.
            unsafe {
                if enabled & (1 << location) != 0 {
                    gl.enable_vertex_attrib_array(location);
                } else {
                    gl.disable_vertex_attrib_array(location);
                }
            }
        }
        self.applied.enabled = enabled;
        self.applied.layout = call.vertex_layout.0.0;
        Ok(())
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
fn index_bytes(index: &IndexBinding<'_, Gl>, first: u32, count: u32) -> Range<u64> {
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

impl Gl {
    /// [`Gpu::draw`], with the draw's instances starting `later` instances past its first
    /// instance: `instance_index` and the per-instance streams as the full draw has them there.
    /// The bytes of a draw's indices, read back from the index GL buffer (waiting for the GPU).
    fn read_indices(
        &mut self,
        index: &IndexBinding<'_, Self>,
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
        self.refresh(index.buffer, Some(true));
        let gl_buffer = index.buffer.index.or(index.buffer.data);
        // SAFETY: plain GL calls; the copy target takes buffers of either WebGL type, and the range
        // was checked above.
        let read = unsafe {
            self.gl.bind_buffer(glow::COPY_READ_BUFFER, gl_buffer);
            platform::get_buffer_sub_data(
                &self.gl,
                glow::COPY_READ_BUFFER,
                range.start as i32,
                &mut raw,
            )
        };
        if !read {
            return Err(self.read_failed());
        }
        Ok(raw)
    }

    /// An indexed draw that must not restart, whose indices may hold GL's restart value (which WebGL
    /// cannot turn off): they are read back and, where one is there, drawn rewritten as 32-bit indices
    /// from the index ring ([`SlowPath::RestartRewrite`]).
    fn draw_unrestarted(&mut self, call: &DrawCall<'_, Self>) {
        let (
            Some(index),
            DrawRange::Indexed {
                first,
                count,
                base_vertex,
                first_instance,
                instances,
            },
        ) = (&call.index, &call.range)
        else {
            return self.draw_from(call, 0);
        };
        let (first, count, base_vertex) = (*first, *count, *base_vertex);
        let (first_instance, instances) = (*first_instance, *instances);
        self.record_slow_path(SlowPath::RestartRewrite);
        let Ok(bytes) = self.read_indices(index, first, count) else {
            return self.invalid("indices past the end of the index buffer".into());
        };
        let start = index_bytes(index, first, count).start;
        index.buffer.maxima.borrow_mut().wrote(start, &bytes);
        let (widened, shift) = match restart::rewrite(&bytes, index.format) {
            restart::Rewrite::Unneeded => return self.draw_from(call, 0),
            restart::Rewrite::Widened(widened, shift) => (widened, shift),
            restart::Rewrite::Unreachable => {
                self.unbuilt(
                    "a draw whose 32-bit indices hold both 0 and 2^32 - 1 without restart",
                );
                return;
            }
        };
        let Ok(base_vertex) = i32::try_from(i64::from(base_vertex) + i64::from(shift)) else {
            self.unbuilt(
                "a draw without restart whose lowered 32-bit indices need a base vertex past \
                 2^31 - 1",
            );
            return;
        };
        let mut offset = [0];
        let ring = match self.write_buffer_ring(RingKind::Indices, &[&widened], 4, &mut offset) {
            Ok(ring) => ring,
            Err(_) => return self.fail(LossReason::OutOfMemory),
        };
        self.draw_from(
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
            0,
        );
    }

    fn draw_from(&mut self, call: &DrawCall<'_, Self>, later: u32) {
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
                self.invalid("an indirect draw without INDIRECT".into());
                return;
            }
        };
        if call.viewports.len() != 1 {
            self.invalid(format!("{} viewports in one draw", call.viewports.len()));
            return;
        }
        if call.stencil_reference[0] != call.stencil_reference[1] {
            // STENCIL_PER_FACE is absent: the front draws each face apart.
            self.invalid("front and back stencil references differ".into());
            return;
        }
        if call.immediates.len() > self.caps.limits.immediate_bytes as usize {
            self.invalid(format!(
                "{} bytes of immediates past the limit of {}",
                call.immediates.len(),
                self.caps.limits.immediate_bytes
            ));
            return;
        }
        let converted = call.vertex_layout.0.1.converted;
        let variant = match converted {
            0 => None,
            key => match self.variant(call.program, key) {
                Some(v) => Some(v),
                None => return,
            },
        };
        let program: &ProgramObj = variant.as_deref().unwrap_or(call.program);
        let Some(linked) = self.linked(program) else {
            return;
        };
        if self.stale_buffers > 0 {
            self.refresh_draw(call);
        }
        if self.twins > 0 {
            for set in call.bindings.iter().flatten() {
                for (_, bound) in &set.entries {
                    if let Bound::Texture(v) = bound {
                        self.fresh(&v.texture);
                    }
                }
            }
            // Filling a stencil copy draws into framebuffers of its own.
            if let Some(pass) = &self.pass
                && self.applied.draw_framebuffer != Some(pass.framebuffer)
            {
                // SAFETY: a plain GL call; the pass's framebuffer is alive.
                unsafe {
                    self.gl
                        .bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(pass.framebuffer))
                };
                self.applied.draw_framebuffer = Some(pass.framebuffer);
            }
        }
        if self.applied.program != program.id {
            // SAFETY: a plain GL call; the program is alive.
            unsafe { self.gl.use_program(Some(program.raw)) };
            self.applied.program = program.id;
            self.applied.sets = [u64::MAX; MAX_BINDING_SETS];
            let n = program.stages.0.clip_distances;
            if self.values.clip_distances != n {
                state::clip_distances(&self.gl, &mut self.values, n);
            }
        }
        if self.applied.blend != call.blend.0.0 {
            // GL blends a second source into one colour target only.
            if call.blend.0.1.second_source
                && self.pass.as_ref().is_some_and(|p| p.color_targets > 1)
            {
                self.invalid("a second-source blend with more than one colour target".into());
                return;
            }
            state::blend(
                &self.gl,
                &mut self.values,
                &call.blend.0.1,
                &self.platform,
                self.pass_unwritten,
            );
            self.applied.blend = call.blend.0.0;
        }
        let (gl, v) = (&self.gl, &mut self.values);
        let reference = call.stencil_reference[0];
        if self.applied.depth_stencil != call.depth_stencil.0.0
            || self.applied.stencil_reference != reference
        {
            state::depth_stencil(gl, v, &call.depth_stencil.0.1, reference);
            self.applied.depth_stencil = call.depth_stencil.0.0;
            self.applied.stencil_reference = reference;
        }
        if self.applied.raster != call.raster.0.0 {
            state::raster(gl, v, &call.raster.0.1, &self.platform);
            self.applied.raster = call.raster.0.0;
        }
        // SAFETY: plain GL state calls.
        unsafe {
            if let Some(vp) = call.viewports.first() {
                let floats = [f(vp.x), f(vp.y), f(vp.width), f(vp.height)];
                let range = [f(vp.min_depth), f(vp.max_depth)];
                // The interface's bounds (WebGPU's); a front draws a larger viewport in the program.
                let most = self.caps.limits.max_texture_2d as f32;
                let edge = |at: f32, size: f32| {
                    (0.0..=most).contains(&size)
                        && at >= -2.0 * most
                        && at + size <= 2.0 * most - 1.0
                };
                if !edge(floats[0], floats[2])
                    || !edge(floats[1], floats[3])
                    || !range.iter().all(|d| (0.0..=1.0).contains(d))
                    || range[0] > range[1]
                {
                    self.invalid(format!(
                        "the viewport {floats:?} with depths {range:?} past the interface's bounds"
                    ));
                    return;
                }
                let rect = floats.map(|v| v as i32);
                // FRACTIONAL_VIEWPORT is absent: the front draws such a viewport in the program.
                if floats.iter().zip(&rect).any(|(v, r)| *v != *r as f32) {
                    self.invalid(format!(
                        "the fractional viewport {floats:?} without FRACTIONAL_VIEWPORT"
                    ));
                    return;
                }
                if v.viewport != rect {
                    gl.viewport(rect[0], rect[1], rect[2], rect[3]);
                    v.viewport = rect;
                }
                if v.depth_range != range {
                    gl.depth_range_f32(range[0], range[1]);
                    v.depth_range = range;
                }
            }
            if v.scissor_test
                && let Some(s) = call.scissors.first()
            {
                let rect = gl_rect(s, self.pass.as_ref().map_or(NO_PASS, |p| p.size));
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
        }
        match self.apply_bindings(program.id, linked, &call.bindings) {
            Ok(()) => {}
            Err(Unbindable::Missing((set, binding))) => {
                self.invalid(format!(
                    "the draw's binding sets hold no fitting resource at set {set} binding \
                     {binding}, which its program reads"
                ));
                return;
            }
            Err(Unbindable::BothAspects) => {
                self.unbuilt("depth and stencil views of one texture fetched by one draw");
                return;
            }
            Err(Unbindable::NoSampler) => {
                self.fail(LossReason::OutOfMemory);
                return;
            }
            Err(Unbindable::Multisampled((set, binding))) => {
                self.invalid(format!(
                    "the draw's program reads the multisampled texture at set {set} binding \
                     {binding}, which needs the absent MULTISAMPLE_READ"
                ));
                return;
            }
            Err(Unbindable::TwoMipRanges((set, binding))) => {
                self.invalid(format!(
                    "the draw reads the texture at set {set} binding {binding} through two mip \
                     ranges, which needs the absent TEXTURE_VIEWS"
                ));
                return;
            }
        }
        if base_vertex != 0 || first_instance != 0 || later != 0 {
            self.record_slow_path(SlowPath::BaseVertexRebind);
        }
        if self
            .apply_vertices(call, base_vertex, first_instance, later)
            .is_err()
        {
            self.unbuilt("negative stream offsets after the base vertex");
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
                // SAFETY: the program is in use.
                unsafe { self.gl.uniform_1_u32(Some(location), value) };
                held.set(value);
            }
        }
        shader::set_immediates(&self.gl, linked, call.immediates);
        let mode = state::mode(call.topology);
        let gl = &self.gl;
        match call.range {
            DrawRange::Vertices { first, count, .. } => {
                // SAFETY: the draw's state is applied.
                unsafe {
                    gl.draw_arrays_instanced(mode, first as i32, count as i32, instances as i32)
                };
            }
            DrawRange::Indexed { first, count, .. } => {
                let Some(index) = &call.index else {
                    return self.invalid("an indexed draw without an index buffer".into());
                };
                if self.applied.index != index.buffer.id {
                    // SAFETY: the buffer's index GL buffer is alive.
                    unsafe { gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, index.buffer.index) };
                    self.applied.index = index.buffer.id;
                }
                let ty = match index.format {
                    IndexFormat::U8 => glow::UNSIGNED_BYTE,
                    IndexFormat::U16 => glow::UNSIGNED_SHORT,
                    IndexFormat::U32 => glow::UNSIGNED_INT,
                };
                let offset = index.offset
                    + u64::from(first) * u64::from(INDEX_FORMATS[index.format as usize].size);
                // SAFETY: the draw's state and index buffer are applied.
                unsafe {
                    gl.draw_elements_instanced(
                        mode,
                        count as i32,
                        ty,
                        offset as i32,
                        instances as i32,
                    )
                };
            }
            _ => unreachable!("indirect draws returned above"),
        }
    }
}

impl Gpu for Gl {
    type Buffer = Buffer;
    type Texture = Texture;
    type TextureView = TextureView;
    type Sampler = Sampler;
    type Shader = Shader;
    type Program = GlProgram;
    type Blend = Block<state::GlBlend>;
    type DepthStencil = Block<state::GlDepthStencil>;
    type Raster = Block<state::GlRaster>;
    type VertexLayout = Block<state::GlLayout>;
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
        self.platform.current();
        &mut self.slow
    }

    fn status(&mut self) -> DeviceStatus {
        self.platform.current();
        if let Some(reason) = self.platform.lost() {
            self.fail(reason);
        }
        match self.lost {
            Some(reason) => DeviceStatus::Lost(reason),
            None => DeviceStatus::Ok,
        }
    }

    fn helpers(&mut self) -> &mut Helpers<Self> {
        self.platform.current();
        &mut self.helpers
    }

    fn create_buffer(&mut self, desc: &BufferDesc) -> Result<Buffer, GpuError> {
        self.platform.current();
        let size = i32::try_from(desc.size).map_err(|_| GpuError::OutOfMemory)?;
        let usage = match desc.intent {
            MapIntent::Rarely => glow::STATIC_DRAW,
            MapIntent::Often => glow::DYNAMIC_DRAW,
        };
        let copies = BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::READ_BACK;
        let indices = desc.usage.contains(BufferUsage::INDEX);
        let other = !(desc.usage - BufferUsage::INDEX - copies).is_empty() || !indices;
        if indices && other {
            self.record_slow_path(SlowPath::IndexBufferTwin);
        }
        let gl = &self.gl;
        // SAFETY: plain GL calls. Each GL buffer is first bound to the target that fixes its WebGL
        // type, with the vertex array the back end keeps bound, whose index binding is redone at the
        // next indexed draw.
        let make = |target| unsafe {
            let b = gl.create_buffer().map_err(|_| GpuError::OutOfMemory)?;
            gl.bind_buffer(target, Some(b));
            gl.buffer_data_size(target, size, usage);
            Ok::<_, GpuError>(b)
        };
        let data = other.then(|| make(glow::ARRAY_BUFFER)).transpose()?;
        let index = indices
            .then(|| make(glow::ELEMENT_ARRAY_BUFFER))
            .transpose()?;
        if indices {
            self.applied.index = 0;
        }
        Ok(Handle(Rc::new(BufferObj {
            id: self.id(),
            size: desc.size,
            usage,
            data,
            index,
            stale: Cell::new(None),
            maxima: RefCell::default(),
        })))
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Texture, GpuError> {
        self.platform.current();
        if self.caps.formats.uses[desc.format as usize] == FormatUse::empty() {
            return Err(GpuError::Invalid(
                "webgl: the format has no capabilities here",
            ));
        }
        // Block-compressed formats are GL's compressed formats with their extension (not in 3D,
        // which GLES does not compress), else stored decoded.
        let (gl_format, decoded) = match host::compressed(desc.format) {
            Some((internal, ext, _))
                if self.extensions.contains(ext) && desc.dimension != TextureDimension::D3 =>
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
                    self.record_slow_path(SlowPath::BlockDecoded);
                    (format::gl_format(decoded), Some(decoded))
                }
                None => (format::gl_format(desc.format), format::widened(desc.format)),
            },
        };
        let gl_format = gl_format.ok_or(GpuError::Invalid(
            "webgl: the format has no capabilities here",
        ))?;
        if desc.samples > 1 {
            return self.create_samples(desc, gl_format);
        }
        let target = match desc.own_view() {
            ViewDimension::D2 => glow::TEXTURE_2D,
            ViewDimension::D2Array => glow::TEXTURE_2D_ARRAY,
            ViewDimension::Cube => glow::TEXTURE_CUBE_MAP,
            ViewDimension::D3 => glow::TEXTURE_3D,
            ViewDimension::CubeArray => {
                return Err(GpuError::Invalid("webgl: WebGL 2 has no cube arrays"));
            }
        };
        // SAFETY: a plain GL call on the current context.
        let raw = unsafe { self.gl.create_texture() }.map_err(|_| GpuError::OutOfMemory)?;
        self.bind_scratch(target, raw);
        let gl = &self.gl;
        // SAFETY: plain GL calls on the bound texture.
        unsafe {
            let (w, h, d) = (
                desc.width as i32,
                desc.height as i32,
                desc.depth_or_layers as i32,
            );
            let levels = desc.mip_levels as i32;
            match target {
                glow::TEXTURE_2D | glow::TEXTURE_CUBE_MAP => {
                    gl.tex_storage_2d(target, levels, gl_format.internal, w, h)
                }
                _ => gl.tex_storage_3d(target, levels, gl_format.internal, w, h, d),
            }
            gl.tex_parameter_i32(target, glow::TEXTURE_MAX_LEVEL, levels - 1);
        }
        Ok(Handle(Rc::new(TextureObj {
            id: self.id(),
            storage: Storage::Texture(raw),
            target,
            desc: *desc,
            gl: gl_format,
            read: format::read_as(
                desc.format,
                format::FormatExtensions::found(&self.extensions),
            ),
            levels: Cell::new((0, desc.mip_levels - 1)),
            newest: Rc::new(Cell::new(0)),
            side: 1,
            twin: std::cell::OnceCell::new(),
            own: None,
            stencil_mode: Cell::new(false),
            host: host::kept(desc.format).then(|| {
                self.record_slow_path(SlowPath::HostCopyKept);
                host::HostCopy::new(desc)
            }),
            decoded,
            drawn: Cell::new(0),
            minimum: Cell::new(0),
        })))
    }

    fn create_view(
        &mut self,
        texture: &Texture,
        desc: &TextureViewDesc,
    ) -> Result<TextureView, GpuError> {
        self.platform.current();
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
            let texture = if self.extensions.contains("WEBGL_stencil_texturing") {
                texture.clone()
            } else {
                self.stencil_copy_of(texture)?
            };
            return Ok(Handle(Rc::new(ViewObj {
                id: self.id(),
                texture,
                desc: *desc,
            })));
        }
        let texture = if desc.format == t.format {
            texture.clone()
        } else if FORMAT_FACTS[desc.format as usize].linear
            == FORMAT_FACTS[t.format as usize].linear
        {
            self.twin_of(texture, desc.format)?
        } else {
            return Err(GpuError::Absent(Features::FORMAT_REINTERPRET));
        };
        Ok(Handle(Rc::new(ViewObj {
            id: self.id(),
            texture,
            desc: *desc,
        })))
    }

    fn create_sampler(&mut self, desc: &SamplerDesc) -> Result<Sampler, GpuError> {
        self.platform.current();
        let address = |a| match a {
            AddressMode::Repeat => Ok(glow::REPEAT),
            AddressMode::Mirror => Ok(glow::MIRRORED_REPEAT),
            AddressMode::Clamp => Ok(glow::CLAMP_TO_EDGE),
            AddressMode::Border => Err(GpuError::Absent(Features::BORDER_COLOR)),
            AddressMode::MirrorOnce if self.caps.features.contains(Features::MIRROR_ONCE) => {
                Ok(glow::MIRROR_CLAMP_TO_EDGE)
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
            (Filter::Nearest, Filter::Nearest) => glow::NEAREST_MIPMAP_NEAREST,
            (Filter::Linear, Filter::Nearest) => glow::LINEAR_MIPMAP_NEAREST,
            (Filter::Nearest, Filter::Linear) => glow::NEAREST_MIPMAP_LINEAR,
            (Filter::Linear, Filter::Linear) => glow::LINEAR_MIPMAP_LINEAR,
        };
        let mag = match desc.mag {
            Filter::Nearest => glow::NEAREST,
            Filter::Linear => glow::LINEAR,
        };
        let gl = &self.gl;
        // SAFETY: plain GL calls on the current context.
        let make = |min_lod: f32| unsafe {
            let raw = gl.create_sampler().map_err(|_| GpuError::OutOfMemory)?;
            gl.sampler_parameter_i32(raw, glow::TEXTURE_MIN_FILTER, min as i32);
            gl.sampler_parameter_i32(raw, glow::TEXTURE_MAG_FILTER, mag as i32);
            for (name, mode) in [
                glow::TEXTURE_WRAP_S,
                glow::TEXTURE_WRAP_T,
                glow::TEXTURE_WRAP_R,
            ]
            .into_iter()
            .zip(wrap)
            {
                gl.sampler_parameter_i32(raw, name, mode as i32);
            }
            gl.sampler_parameter_f32(raw, glow::TEXTURE_MIN_LOD, min_lod);
            gl.sampler_parameter_f32(raw, glow::TEXTURE_MAX_LOD, f(desc.lod_range[1]));
            if let Some(c) = desc.compare {
                gl.sampler_parameter_i32(
                    raw,
                    glow::TEXTURE_COMPARE_MODE,
                    glow::COMPARE_REF_TO_TEXTURE as i32,
                );
                gl.sampler_parameter_i32(raw, glow::TEXTURE_COMPARE_FUNC, state::compare(c) as i32);
            }
            // Above 1 only where EXT_texture_filter_anisotropic raised the limit.
            if desc.anisotropy > 1 {
                let a = f32::from(desc.anisotropy);
                gl.sampler_parameter_f32(raw, glow::TEXTURE_MAX_ANISOTROPY_EXT, a);
            }
            Ok(raw)
        };
        let raw = make(f(desc.lod_range[0]))?;
        let twin = match f(desc.lod_range[0]) > 0.0 {
            true => Some(make(0.0)?),
            false => None,
        };
        let level_zero = twin.map(|t| (self.id(), t));
        Ok(Handle(Rc::new(SamplerObj {
            id: self.id(),
            raw,
            level_zero,
        })))
    }

    fn create_shader(&mut self, module: &ShaderModule) -> Result<Shader, GpuError> {
        self.platform.current();
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
            return Ok(s.clone());
        }
        let raw = shader::start_compile(&self.gl, text.stage, &text.source)?;
        let s = Handle(Rc::new(shader::ShaderObj {
            id: self.id(),
            raw,
            handles: core::cell::Cell::new(1),
            text: text.clone(),
        }));
        self.shaders.insert(text, s.clone());
        Ok(s)
    }

    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<GlProgram, GpuError> {
        self.platform.current();
        let Program::Render { vertex, fragment } = program else {
            return Err(GpuError::Absent(Features::COMPUTE));
        };
        let pair = (vertex.id, fragment.map_or(0, |f| f.id));
        if let Some(p) = self.programs.get(&pair) {
            p.handles.set(p.handles.get() + 1);
            return Ok(p.clone());
        }
        let id = self.id();
        let started = shader::link(
            &self.gl,
            id,
            &vertex.0,
            fragment.map(|f| &f.0),
            self.empty_fragment,
        )?;
        let p = Handle(Rc::new(started));
        self.programs.insert(pair, p.clone());
        Ok(p)
    }

    fn create_blend(&mut self, desc: &BlendDesc) -> Result<Block<state::GlBlend>, GpuError> {
        self.platform.current();
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
        Ok(Handle(Rc::new((self.id(), state::GlBlend::new(desc)))))
    }

    fn create_depth_stencil(
        &mut self,
        desc: &DepthStencilDesc,
    ) -> Result<Block<state::GlDepthStencil>, GpuError> {
        self.platform.current();
        if desc.depth_bounds.is_some() {
            return Err(GpuError::Absent(Features::DEPTH_BOUNDS));
        }
        if desc.stencil_read_mask[0] != desc.stencil_read_mask[1]
            || desc.stencil_write_mask[0] != desc.stencil_write_mask[1]
        {
            return Err(GpuError::Absent(Features::STENCIL_PER_FACE));
        }
        Ok(Handle(Rc::new((
            self.id(),
            state::GlDepthStencil::new(desc),
        ))))
    }

    fn create_raster(&mut self, desc: &RasterDesc) -> Result<Block<state::GlRaster>, GpuError> {
        self.platform.current();
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
                !desc.depth_minus_one && self.glsl.flip_y,
                Features::CLIP_DEPTH_ZERO_ONE,
            ),
        ];
        if let Some((_, feature)) = absent.into_iter().find(|(missing, _)| *missing) {
            return Err(GpuError::Absent(feature));
        }
        Ok(Handle(Rc::new((
            self.id(),
            state::GlRaster::new(desc, self.glsl.flip_y),
        ))))
    }

    fn create_vertex_layout(
        &mut self,
        desc: &VertexLayoutDesc,
    ) -> Result<Block<state::GlLayout>, GpuError> {
        self.platform.current();
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
            Some(layout) => Ok(Handle(Rc::new((self.id(), layout)))),
            None => Err(GpuError::Invalid(
                "an attribute names a stream the layout lacks",
            )),
        }
    }

    fn create_binding_set(
        &mut self,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<BindingSet, GpuError> {
        self.platform.current();
        let mut bound = Vec::with_capacity(entries.len());
        for e in entries {
            let r = match &e.resource {
                BindingResource::UniformBuffer(r) => {
                    Bound::Uniform(r.buffer.clone(), r.offset, r.size)
                }
                BindingResource::Texture(v) => {
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
                    Bound::Texture((*v).clone())
                }
                BindingResource::Sampler(s) => Bound::Sampler((*s).clone()),
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
        Ok(Handle(Rc::new(SetObj {
            id: self.id(),
            entries: bound,
        })))
    }

    fn binding_layout_key(&self, _set: &BindingSet) -> BindingLayoutKey {
        BindingLayoutKey::default()
    }

    fn create_query_set(&mut self, kind: QueryKind, count: u32) -> Result<QuerySet, GpuError> {
        self.platform.current();
        match kind {
            QueryKind::OcclusionAny => {}
            // GL ES answers only whether any sample passed.
            QueryKind::Occlusion => return Err(GpuError::Absent(Features::OCCLUSION_COUNT)),
            QueryKind::Timestamp if !self.caps.features.contains(Features::TIMESTAMPS) => {
                return Err(GpuError::Absent(Features::TIMESTAMPS));
            }
            QueryKind::Timestamp => {}
        }
        Ok(Handle(Rc::new(QuerySetObj {
            kind,
            slots: core::cell::RefCell::new(vec![None; count as usize]),
        })))
    }

    fn destroy(&mut self, object: Object<Self>) {
        self.platform.current();
        if let Object::Surface(surface) = &object {
            if self.surface.as_ref().is_some_and(|s| s.id == surface.id) {
                if let Some((t, v)) = self.surface.take().and_then(|s| s.frame) {
                    self.destroy(Object::TextureView(v));
                    self.destroy(Object::Texture(t));
                }
                self.platform.detach();
            }
            return;
        }
        if let Object::Texture(t) = &object
            && let Some(twin) = t.twin.get()
        {
            self.twins -= 1;
            let twin = twin.clone();
            self.destroy(Object::Texture(twin));
        }
        let gl = &self.gl;
        // SAFETY: GL keeps an object alive for commands already issued that use it.
        unsafe {
            match object {
                Object::Buffer(b) => {
                    if b.stale.get().is_some() {
                        self.stale_buffers -= 1;
                    }
                    for raw in [b.data, b.index].into_iter().flatten() {
                        gl.delete_buffer(raw);
                    }
                }
                Object::Texture(t) => {
                    self.framebuffers.retain(|key, fb| {
                        let uses = key.iter().flatten().any(|a| a.0 == t.id);
                        if uses {
                            gl.delete_framebuffer(*fb);
                        }
                        !uses
                    });
                    if self
                        .applied
                        .draw_framebuffer
                        .is_some_and(|fb| !self.framebuffers.values().any(|f| *f == fb))
                    {
                        self.applied.draw_framebuffer = None;
                    }
                    if self
                        .applied
                        .read_framebuffer
                        .is_some_and(|fb| !self.framebuffers.values().any(|f| *f == fb))
                    {
                        self.applied.read_framebuffer = None;
                    }
                    match t.storage {
                        Storage::Texture(raw) => gl.delete_texture(raw),
                        Storage::Samples(rb) => gl.delete_renderbuffer(rb),
                    }
                }
                Object::Sampler(s) => {
                    gl.delete_sampler(s.raw);
                    if let Some((_, twin)) = s.level_zero {
                        gl.delete_sampler(twin);
                    }
                }
                Object::Shader(s) => {
                    s.handles.set(s.handles.get() - 1);
                    if s.handles.get() == 0 {
                        self.shaders.remove(&s.text);
                        gl.delete_shader(s.raw);
                    }
                }
                Object::Program(p) => {
                    p.handles.set(p.handles.get() - 1);
                    if p.handles.get() == 0 {
                        self.programs.remove(&p.pair);
                        gl.delete_program(p.raw);
                        let captures = p.captures.iter().filter_map(|c| c.get());
                        for c in captures.chain(core::iter::once(&p.0)) {
                            for (_, v) in c.variants.borrow().iter() {
                                gl.delete_program(v.raw);
                                if self.applied.program == v.id {
                                    self.applied.program = 0;
                                }
                            }
                        }
                        for c in p.captures.iter().filter_map(|c| c.get()) {
                            gl.delete_program(c.raw);
                            if self.applied.program == c.id {
                                self.applied.program = 0;
                            }
                        }
                        if self.applied.program == p.id {
                            self.applied.program = 0;
                        }
                    }
                }
                Object::Blend(_)
                | Object::DepthStencil(_)
                | Object::Raster(_)
                | Object::VertexLayout(_)
                | Object::TextureView(_)
                | Object::BindingSet(_)
                | Object::Surface(_) => {}
                Object::QuerySet(set) => {
                    // Queries a read back still holds are deleted when it finishes.
                    for q in set.slots.borrow_mut().drain(..).flatten() {
                        if let Ok(q) = Rc::try_unwrap(q) {
                            gl.delete_query(q);
                        }
                    }
                }
                Object::ResourceTable(t) => match t {},
            }
        }
    }

    fn write_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8], mode: WriteMode) {
        self.platform.current();
        if offset + data.len() as u64 > buffer.size {
            return self.invalid(format!(
                "a write of {} bytes at {offset} past a buffer of {}",
                data.len(),
                buffer.size
            ));
        }
        if buffer.index.is_some() {
            buffer.maxima.borrow_mut().wrote(offset, data);
        }
        let gl = &self.gl;
        for raw in [buffer.data, buffer.index].into_iter().flatten() {
            // SAFETY: plain GL calls; the copy targets take buffers of either WebGL type.
            unsafe {
                gl.bind_buffer(glow::COPY_WRITE_BUFFER, Some(raw));
                if mode == WriteMode::Discard {
                    // Orphan at the buffer's own size, so later writes above this part still fit.
                    gl.buffer_data_size(glow::COPY_WRITE_BUFFER, buffer.size as i32, buffer.usage);
                }
                gl.buffer_sub_data_u8_slice(glow::COPY_WRITE_BUFFER, offset as i32, data);
            }
        }
    }

    fn write_texture(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        data: &[u8],
        row_pitch: u32,
        rows_per_layer: u32,
        _mode: WriteMode,
    ) {
        self.platform.current();
        self.fresh(region.texture);
        self.written(region.texture);
        let t = region.texture;
        let (corner, size) = (
            Self::corner(region),
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
                let _ = self.host_current(t, level);
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
                return self.upload_blocks(region, extent);
            }
        }
        if FORMAT_FACTS[region.texture.desc.format as usize].depth_stencil {
            return self.write_depth(
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
            self.record_slow_path(SlowPath::FormatConverted);
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
            return self.upload(
                region,
                extent,
                Texels::Bytes(&stored),
                extent.width * texel,
                extent.height,
            );
        }
        self.upload(
            region,
            extent,
            Texels::Bytes(data),
            row_pitch,
            rows_per_layer,
        );
    }

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        self.platform.current();
        if let Some(empty) = pass.empty {
            return self.begin_empty_pass(empty, pass.occlusion.cloned());
        }
        let mut key: FramebufferKey = [None; MAX_COLOR_TARGETS + 1];
        let mut views: [Option<&TextureView>; MAX_COLOR_TARGETS + 1] =
            [None; MAX_COLOR_TARGETS + 1];
        for (i, c) in pass.colors.iter().enumerate() {
            if let Some(c) = c {
                key[i] = Some(Self::attachment(c.view, c.depth_slice));
                views[i] = Some(c.view);
            }
        }
        if let Some(d) = &pass.depth {
            key[MAX_COLOR_TARGETS] = Some(Self::attachment(d.view, 0));
            views[MAX_COLOR_TARGETS] = Some(d.view);
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
                self.fresh(&v.texture);
                self.written(&v.texture);
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
        let unwritten = |v: &TextureView| format::unwritten(v.texture.desc.format);
        let first = colors.iter().flatten().next().map_or(0, |v| unwritten(v));
        let unwritten = if colors.iter().flatten().any(|v| unwritten(v) != 0) {
            if colors.iter().flatten().any(|v| unwritten(v) != first)
                && !self.caps.features.contains(Features::INDEPENDENT_BLEND)
            {
                self.unbuilt(MIXED_CHANNELS);
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
        let fb = self.framebuffer(key, &views);
        let gl = &self.gl;
        let mut start_discard = Points::default();
        let mut end_discard = Points::default();
        // SAFETY: plain GL calls on the bound framebuffer.
        unsafe {
            if self.applied.draw_framebuffer != Some(fb) {
                gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb));
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
                self.open_writes();
            }
            let gl = &self.gl;
            // A layer of a 2D array is never invalidated: in Chrome (ANGLE on Direct3D 11) that
            // loses the other layers of the level.
            let whole = |v: &TextureView| v.texture.target != glow::TEXTURE_2D_ARRAY;
            for (i, c) in pass.colors.iter().enumerate() {
                let Some(c) = c else { continue };
                let point = glow::COLOR_ATTACHMENT0 + i as u32;
                let format = c.view.texture.desc.format;
                // A target lacking channels keeps their values, which a discard would lose.
                let whole = whole(c.view) && format::unwritten(format) == 0;
                match c.load {
                    LoadOp::Clear(value) => match FORMAT_FACTS[format as usize].kind {
                        FormatKind::Uint => {
                            gl.clear_buffer_u32_slice(glow::COLOR, i as u32, &value)
                        }
                        FormatKind::Sint => gl.clear_buffer_i32_slice(
                            glow::COLOR,
                            i as u32,
                            &value.map(|x| x as i32),
                        ),
                        _ => gl.clear_buffer_f32_slice(
                            glow::COLOR,
                            i as u32,
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
                        gl.clear_buffer_depth_stencil(glow::DEPTH_STENCIL, 0, depth, stencil)
                    }
                    (Some(depth), None) => gl.clear_buffer_f32_slice(glow::DEPTH, 0, &[depth]),
                    (None, Some(stencil)) => {
                        gl.clear_buffer_i32_slice(glow::STENCIL, 0, &[stencil])
                    }
                    (None, None) => {}
                }
                for (ops, point) in [
                    (
                        d.depth.map(|a| (a.load == LoadOp::DontCare, a.store)),
                        glow::DEPTH_ATTACHMENT,
                    ),
                    (
                        d.stencil.map(|a| (a.load == LoadOp::DontCare, a.store)),
                        glow::STENCIL_ATTACHMENT,
                    ),
                ] {
                    if let Some((load, store)) = ops
                        && whole(d.view)
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
                gl.invalidate_framebuffer(glow::DRAW_FRAMEBUFFER, start_discard.get());
            }
        }
        self.pass = Some(Pass {
            framebuffer: fb,
            discard: end_discard,
            occlusion: pass.occlusion.cloned(),
            counting: false,
            resolves: core::array::from_fn(|i| {
                let c = pass.colors[i].as_ref()?;
                Some((c.view.texture.clone(), c.resolve?.clone()))
            }),
            size,
            color_targets: pass.colors.iter().flatten().count(),
        });
    }

    fn end_pass(&mut self) {
        self.platform.current();
        if self.pass.as_ref().is_some_and(|p| p.counting) {
            self.invalid("a pass ended with its occlusion query running".into());
        }
        let Some(pass) = self.pass.take() else {
            return;
        };
        let mut resolved = false;
        for (ms, view) in pass.resolves.iter().flatten() {
            let t = view.texture.clone();
            self.fresh(&t);
            self.written(&t);
            self.blit_resolve(ms, &t, view.desc.base_mip, view.desc.base_layer);
            resolved = true;
        }
        if resolved && pass.discard.len > 0 {
            // SAFETY: a plain GL call; the discard below names the pass's framebuffer.
            unsafe {
                self.gl
                    .bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(pass.framebuffer))
            };
            self.applied.draw_framebuffer = Some(pass.framebuffer);
        }
        if pass.discard.len > 0 {
            // SAFETY: the pass's framebuffer is still bound.
            unsafe {
                self.gl
                    .invalidate_framebuffer(glow::DRAW_FRAMEBUFFER, pass.discard.get())
            };
        }
    }

    fn draw(&mut self, call: &DrawCall<'_, Self>) {
        self.platform.current();
        if !call.restart
            && let (Some(index), DrawRange::Indexed { first, count, .. }) =
                (&call.index, &call.range)
            && index
                .buffer
                .maxima
                .borrow()
                .may_hold(index.format, index_bytes(index, *first, *count))
        {
            return self.draw_unrestarted(call);
        }
        self.draw_from(call, 0);
    }

    fn begin_query(&mut self, index: u32) {
        self.platform.current();
        let Some(pass) = &self.pass else {
            return self.invalid("begin_query outside a pass".into());
        };
        let (Some(set), false) = (pass.occlusion.clone(), pass.counting) else {
            return self
                .invalid("begin_query without an occlusion set or with a query running".into());
        };
        let Some(q) = self.query_slot(&set, index) else {
            return;
        };
        // SAFETY: a plain GL call; no other occlusion query runs.
        unsafe { self.gl.begin_query(glow::ANY_SAMPLES_PASSED, q) };
        if let Some(p) = &mut self.pass {
            p.counting = true;
        }
    }

    fn end_query(&mut self) {
        self.platform.current();
        match &mut self.pass {
            Some(p) if p.counting => p.counting = false,
            _ => return self.invalid("end_query with no query running".into()),
        }
        // SAFETY: a plain GL call ending the running query.
        unsafe { self.gl.end_query(glow::ANY_SAMPLES_PASSED) };
    }

    fn copy_buffer(
        &mut self,
        src: &Buffer,
        src_offset: u64,
        dst: &Buffer,
        dst_offset: u64,
        size: u64,
    ) {
        self.platform.current();
        // Both sides are made whole first, so afterwards at most the side no copy reached is stale.
        self.refresh(src, None);
        self.refresh(dst, None);
        let pairs = [(src.data, dst.data, false), (src.index, dst.index, true)];
        let mut copied = false;
        let mut missed = None;
        for (s, d, index) in pairs {
            match (s, d) {
                (Some(s), Some(d)) => {
                    let gl = &self.gl;
                    // SAFETY: plain GL calls between GL buffers of one WebGL type.
                    unsafe {
                        gl.bind_buffer(glow::COPY_READ_BUFFER, Some(s));
                        gl.bind_buffer(glow::COPY_WRITE_BUFFER, Some(d));
                        gl.copy_buffer_sub_data(
                            glow::COPY_READ_BUFFER,
                            glow::COPY_WRITE_BUFFER,
                            src_offset as i32,
                            dst_offset as i32,
                            size as i32,
                        );
                    }
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
            let read = ReadSource::Buffer {
                buffer: src,
                offset: src_offset,
                size,
            };
            if self.read_back_now(&read, &mut bytes).is_ok() {
                self.write_buffer(dst, dst_offset, &bytes, WriteMode::Ordered);
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
            self.mark_stale(dst, index, dst_offset, dst_offset + size);
        }
    }

    fn copy_texture(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.platform.current();
        // Signed normalised texels drawn since their host copy was whole are drawn on the GPU where
        // they hold no smallest value.
        let level = 1 << src.mip;
        let by_draw =
            src.texture.drawn.get() & level != 0 && src.texture.minimum.get() & level == 0;
        if (src.texture.host.is_some() && !by_draw) || dst.texture.host.is_some() {
            return self.copy_host_kept(src, dst, extent);
        }
        if src.texture.desc.samples > 1 || dst.texture.desc.samples > 1 {
            return self.copy_samples(src, dst);
        }
        if self.within_one_subresource(src, dst, extent) {
            return self.copy_within(src, dst, extent);
        }
        let (from, to) = (src.texture.desc.format, dst.texture.desc.format);
        self.fresh(src.texture);
        self.fresh(dst.texture);
        self.written(dst.texture);
        if from == to && FORMAT_FACTS[from as usize].depth_stencil {
            return self.blit_same(src, dst, extent);
        }
        if FORMAT_FACTS[from as usize].depth_stencil || FORMAT_FACTS[to as usize].depth_stencil {
            // A colour and a depth format of one typeless group.
            if self.copy_by_draw(src, dst, extent) {
                self.record_slow_path(if FORMAT_FACTS[to as usize].depth_stencil {
                    SlowPath::DepthWrittenByDraw
                } else {
                    SlowPath::CopyThroughBuffer
                });
            } else {
                self.unbuilt("copies between these depth and colour formats");
            }
            return;
        }
        let renderable = |f| self.caps.formats.uses[f as usize].contains(FormatUse::RENDER);
        // copyTexSubImage may copy floats through arithmetic, which loses NaN payloads and denormals.
        let float = format::float_bits(from);
        if from == to && renderable(from) && float && self.float_blits {
            return self.blit_same(src, dst, extent);
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
                self.copy_same_bytes(src, dst, extent);
            } else if half {
                // A draw writes half floats from 32-bit ones, which lose NaN payloads; the host
                // writes the bits.
                self.copy_host_kept(src, dst, extent);
            } else if self.copy_by_draw(src, dst, extent) {
                self.record_slow_path(SlowPath::CopyThroughBuffer);
            } else if float || src.texture.host.is_some() {
                // 32-bit floats are no targets without EXT_color_buffer_float.
                self.copy_host_kept(src, dst, extent);
            } else {
                self.unbuilt("copies of these formats");
            }
            return;
        }
        let d = dst.texture.clone();
        let Some(raw) = d.tex() else {
            return;
        };
        for i in 0..extent.depth_or_layers {
            let layer = self.read_layer(src, i);
            self.read_framebuffer(src.texture, src.mip, layer);
            self.bind_scratch(d.target, raw);
            let (x, y) = (dst.origin.x as i32, dst.origin.y as i32);
            let (sx, sy, w, h) = (
                src.origin.x as i32,
                src.origin.y as i32,
                extent.width as i32,
                extent.height as i32,
            );
            let level = dst.mip as i32;
            let gl = &self.gl;
            // SAFETY: plain GL calls; the read framebuffer holds the source level and layer.
            unsafe {
                match d.target {
                    glow::TEXTURE_2D => {
                        gl.copy_tex_sub_image_2d(d.target, level, x, y, sx, sy, w, h)
                    }
                    glow::TEXTURE_CUBE_MAP => gl.copy_tex_sub_image_2d(
                        glow::TEXTURE_CUBE_MAP_POSITIVE_X + dst.layer + i,
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
                        self.read_layer(dst, i) as i32,
                        sx,
                        sy,
                        w,
                        h,
                    ),
                }
            }
        }
    }

    fn copy_buffer_to_texture(
        &mut self,
        src: &CopyBufferTexture<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        self.platform.current();
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
            self.record_slow_path(SlowPath::HostCopyReadBack);
            let read = ReadSource::Buffer {
                buffer: src.buffer,
                offset: src.offset,
                size,
            };
            if self.read_back_now(&read, &mut bytes).is_ok() {
                let (pitch, per_layer) = (src.row_pitch, src.rows_per_layer);
                self.write_texture(dst, extent, &bytes, pitch, per_layer, WriteMode::Ordered);
            }
            return;
        };
        self.refresh(src.buffer, Some(false));
        self.fresh(dst.texture);
        self.written(dst.texture);
        // SAFETY: a plain GL call; the buffer holds other data.
        unsafe { self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(data)) };
        let texels = Texels::Offset(src.offset as u32);
        if FORMAT_FACTS[dst.texture.desc.format as usize].depth_stencil {
            self.write_depth(dst, extent, texels, src.row_pitch, src.rows_per_layer);
        } else {
            self.upload(dst, extent, texels, src.row_pitch, src.rows_per_layer);
        }
        // SAFETY: a plain GL call; host uploads need no unpack buffer bound.
        unsafe { self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None) };
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        self.platform.current();
        if self
            .through_draw(src, extent, |b, colour| {
                b.copy_texture_to_buffer(colour, dst, extent);
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
                if self.host_current(src.texture, 1 << src.mip).is_err() {
                    return;
                }
                let size = [extent.width, extent.height, extent.depth_or_layers];
                h.read(
                    &src.texture.desc,
                    src.mip,
                    Self::corner(src),
                    size,
                    &mut packed,
                );
            } else {
                self.record_slow_path(SlowPath::HostCopyReadBack);
                if self.read_texture_now(src, extent, &mut packed).is_err() {
                    return self.invalid("a copy out of a format that cannot be read back".into());
                }
            }
            let (pitch, per_layer) = (u64::from(dst.row_pitch), u64::from(dst.rows_per_layer));
            for (i, chunk) in packed.chunks_exact(row).enumerate() {
                let (layer, r) = ((i / rows) as u64, (i % rows) as u64);
                let at = dst.offset + (layer * per_layer + r) * pitch;
                self.write_buffer(dst.buffer, at, chunk, WriteMode::Ordered);
            }
            return;
        };
        self.fresh(src.texture);
        self.refresh(dst.buffer, None);
        let layer_bytes = dst.row_pitch as usize * dst.rows_per_layer as usize;
        for i in 0..extent.depth_or_layers {
            let layer = self.read_layer(src, i);
            self.read_framebuffer(src.texture, src.mip, layer);
            // SAFETY: plain GL calls; the buffer holds other data and every row of the region.
            unsafe {
                self.gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(data));
                self.gl.pixel_store_i32(
                    glow::PACK_ROW_LENGTH,
                    (dst.row_pitch as usize / read.texel_kept()) as i32,
                );
                self.gl.read_pixels(
                    src.origin.x as i32,
                    src.origin.y as i32,
                    extent.width as i32,
                    extent.height as i32,
                    read.format,
                    read.ty,
                    glow::PixelPackData::BufferOffset(
                        (dst.offset as usize + i as usize * layer_bytes) as u32,
                    ),
                );
                self.gl.pixel_store_i32(glow::PACK_ROW_LENGTH, 0);
                self.gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            }
        }
        if dst.buffer.index.is_some() {
            let end = dst.offset + (layer_bytes * extent.depth_or_layers as usize) as u64;
            dst.buffer.maxima.borrow_mut().unknown(dst.offset..end);
            self.mark_stale(dst.buffer, true, dst.offset, end);
        }
    }

    /// WebGL 2 has no buffer clear: one block of the pattern is written from the host into every GL
    /// buffer of `buffer`, then doubled by copies within each ([`SlowPath::PatternClearCopied`]).
    fn clear_buffer(&mut self, buffer: &Buffer, offset: u64, size: u64, pattern: &[u8]) {
        self.platform.current();
        if size == 0 {
            return;
        }
        if !matches!(pattern.len(), 1 | 2 | 4 | 8 | 16)
            || !size.is_multiple_of(pattern.len() as u64)
            || offset + size > buffer.size
        {
            self.invalid(format!(
                "a buffer clear of {size} bytes from {offset} with a {}-byte pattern",
                pattern.len()
            ));
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
        self.write_buffer(buffer, offset, &block[..first as usize], WriteMode::Ordered);
        let gl = &self.gl;
        for raw in [buffer.data, buffer.index].into_iter().flatten() {
            let mut done = first;
            // SAFETY: plain GL calls; the ranges lie in the buffer and never overlap, as WebGL requires
            // of a copy within one buffer.
            unsafe {
                gl.bind_buffer(glow::COPY_READ_BUFFER, Some(raw));
                gl.bind_buffer(glow::COPY_WRITE_BUFFER, Some(raw));
                while done < size {
                    let n = done.min(size - done);
                    gl.copy_buffer_sub_data(
                        glow::COPY_READ_BUFFER,
                        glow::COPY_WRITE_BUFFER,
                        offset as i32,
                        (offset + done) as i32,
                        n as i32,
                    );
                    done += n;
                }
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
        self.platform.current();
        let (s, d) = (src.texture, dst.texture);
        if s.desc.samples == 1 || d.desc.samples != 1 {
            return self.invalid("a resolve from one sample or into several".into());
        }
        if format != s.desc.format {
            // GL resolves only between equal formats, averaging the stored values.
            self.record_slow_path(SlowPath::ResolveInStoredFormat);
        }
        let target = if d.desc.format == s.desc.format {
            d.clone()
        } else {
            match self.twin_of(d, s.desc.format) {
                Ok(t) => t,
                Err(_) => return,
            }
        };
        self.fresh(&target);
        self.written(&target);
        self.blit_resolve(s, &target, dst.mip, dst.layer);
    }

    fn submit(&mut self) -> Fence {
        self.platform.current();
        // GL's errors are read where the back end waits anyway (`gl_errors`): in the browser
        // `getError` waits for the GPU process. A lost context is seen here without waiting.
        if let Some(reason) = self.platform.lost() {
            self.fail(reason);
        }
        // SAFETY: plain GL calls.
        let sync = unsafe {
            let sync = self.gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0).ok();
            self.gl.flush();
            sync
        };
        if sync.is_none() {
            self.last_log = "no sync object: the context is lost".into();
            self.fail(LossReason::Reset);
        }
        Handle(Rc::new(FenceObj {
            done: Cell::new(sync.is_none()),
            sync,
        }))
    }

    fn fence_done(&mut self, fence: &Fence) -> bool {
        self.platform.current();
        if !fence.done.get()
            && let Some(sync) = fence.sync
        {
            // SAFETY: a plain query of a live sync object.
            let status = unsafe { self.gl.get_sync_status(sync) };
            fence.done.set(status == glow::SIGNALED);
        }
        fence.done.get()
    }

    fn wait_fence(&mut self, fence: &Fence) {
        self.platform.current();
        let Some(sync) = fence.sync.filter(|_| !fence.done.get()) else {
            return;
        };
        // SAFETY: plain GL calls. WebGL cannot wait on a sync object (its longest wait is zero), and
        // finish returns only when every earlier command has completed.
        unsafe {
            #[cfg(target_arch = "wasm32")]
            {
                let _ = sync;
                self.gl.finish();
            }
            #[cfg(not(target_arch = "wasm32"))]
            while self
                .gl
                .client_wait_sync(sync, glow::SYNC_FLUSH_COMMANDS_BIT, 1_000_000_000)
                == glow::TIMEOUT_EXPIRED
            {}
        }
        fence.done.set(true);
        self.gl_errors();
    }

    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Readback, GpuError> {
        self.platform.current();
        if let ReadSource::Texture { region, extent } = src
            && let Some(r) = self.through_draw(region, *extent, |b, colour| {
                b.read_back(&ReadSource::Texture {
                    region: TextureRegion { ..*colour },
                    extent: *extent,
                })
            })
        {
            return r;
        }
        if let ReadSource::Texture { region, extent } = src
            && let Some(h) = &region.texture.host
        {
            self.host_current(region.texture, 1 << region.mip)?;
            let (bw, bh, bytes) = FORMAT_FACTS[region.texture.desc.format as usize].block;
            let size = extent.width.div_ceil(bw) * extent.height.div_ceil(bh) * bytes;
            let mut kept = vec![0; (size * extent.depth_or_layers) as usize];
            let corner = Self::corner(region);
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
        if let ReadSource::Texture { region, .. } = src {
            self.fresh(region.texture);
        }
        let size = match src {
            ReadSource::Buffer { size, .. } => *size as usize,
            ReadSource::Texture { region, extent } => {
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
            ReadSource::Buffer { buffer, .. } if buffer.data.is_none() => {
                // The vertex array's index binding changes; the next indexed draw rebinds it.
                self.applied.index = 0;
                glow::ELEMENT_ARRAY_BUFFER
            }
            ReadSource::Buffer { .. } => glow::COPY_WRITE_BUFFER,
            ReadSource::Texture { .. } => glow::PIXEL_PACK_BUFFER,
        };
        // SAFETY: plain GL calls; the pack buffer is unbound again before returning.
        let pbo = unsafe {
            let pbo = self.gl.create_buffer().map_err(|_| GpuError::OutOfMemory)?;
            let first = if target == glow::COPY_WRITE_BUFFER {
                glow::ARRAY_BUFFER
            } else {
                target
            };
            self.gl.bind_buffer(first, Some(pbo));
            self.gl
                .buffer_data_size(first, size as i32, glow::STREAM_READ);
            self.gl.bind_buffer(target, Some(pbo));
            pbo
        };
        match src {
            ReadSource::Buffer {
                buffer,
                offset,
                size,
            } => {
                let raw = self.any_gl(buffer);
                // SAFETY: plain GL calls between GL buffers of one WebGL type.
                unsafe {
                    self.gl.bind_buffer(glow::COPY_READ_BUFFER, Some(raw));
                    self.gl.copy_buffer_sub_data(
                        glow::COPY_READ_BUFFER,
                        target,
                        *offset as i32,
                        0,
                        *size as i32,
                    );
                }
            }
            ReadSource::Texture { region, extent } => {
                let t = region.texture;
                let read = t.read.expect("checked above");
                let layer_bytes = extent.width * extent.height * read.texel_read() as u32;
                for i in 0..extent.depth_or_layers {
                    let layer = self.read_layer(region, i);
                    self.read_framebuffer(t, region.mip, layer);
                    // SAFETY: the pack buffer holds every layer's rows.
                    unsafe {
                        self.gl.read_pixels(
                            region.origin.x as i32,
                            region.origin.y as i32,
                            extent.width as i32,
                            extent.height as i32,
                            read.format,
                            read.ty,
                            glow::PixelPackData::BufferOffset(i * layer_bytes),
                        );
                    }
                }
            }
        }
        // SAFETY: plain GL calls.
        let sync = unsafe {
            self.gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
            self.gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0).ok()
        };
        // Without a sync object nothing says when the copy is done: only a lost context fails here.
        if sync.is_none() {
            self.fail(LossReason::Reset);
        }
        let repack = match src {
            ReadSource::Texture { region, .. } => region.texture.read,
            ReadSource::Buffer { .. } => None,
        }
        .filter(|r| r.repacks());
        if repack.is_some_and(format::ReadAs::converts) {
            self.record_slow_path(SlowPath::FormatConverted);
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

    fn read_queries(
        &mut self,
        set: &QuerySet,
        first: u32,
        count: u32,
    ) -> Result<Readback, GpuError> {
        self.platform.current();
        let slots = set.slots.borrow();
        let queries = slots
            .get(first as usize..(first + count) as usize)
            .ok_or(GpuError::Invalid("read_queries past the set's end"))?
            .to_vec();
        // SAFETY: a plain GL call; results arrive only once the commands reach the GPU.
        unsafe { self.gl.flush() };
        Ok(Handle(Rc::new(core::cell::RefCell::new(ReadbackObj {
            pbo: None,
            queries,
            timestamps: set.kind == QueryKind::Timestamp,
            size: count as usize * 8,
            sync: None,
            waited: false,
            repack: None,
            host: None,
        }))))
    }

    /// Ready once the copy's sync object is signalled; a read back made without one is ready at once and
    /// reads nothing, the device having been lost when it was made.
    fn read_state(&mut self, readback: &Readback) -> ReadState {
        self.platform.current();
        let r = readback.borrow();
        if r.pbo.is_none() {
            return if self.queries_available(&r.queries) {
                ReadState::Ready
            } else {
                ReadState::Pending
            };
        }
        match r.sync {
            // SAFETY: a plain query of a live sync object.
            Some(sync)
                if !r.waited && unsafe { self.gl.get_sync_status(sync) } != glow::SIGNALED =>
            {
                ReadState::Pending
            }
            _ => ReadState::Ready,
        }
    }

    fn wait_read(&mut self, readback: &Readback) {
        self.platform.current();
        if readback.borrow().pbo.is_none() {
            // WebGL makes results available only between tasks, so the device thread gives way
            // until they are (natively they arrive while it spins).
            while self.status() == DeviceStatus::Ok
                && !self.queries_available(&readback.borrow().queries)
            {
                self.platform.next_task();
            }
            return;
        }
        let sync = readback.borrow().sync;
        if let Some(sync) = sync {
            let fence = Handle(Rc::new(FenceObj {
                sync: Some(sync),
                done: Cell::new(false),
            }));
            self.wait_fence(&fence);
            readback.borrow_mut().waited = true;
        }
    }

    fn finish_read(&mut self, readback: Readback, dst: &mut [u8]) {
        self.platform.current();
        if let Some(kept) = readback.borrow_mut().host.take() {
            let n = kept.len().min(dst.len());
            dst[..n].copy_from_slice(&kept[..n]);
            return;
        }
        let Some(pbo) = readback.borrow().pbo else {
            let r = readback.borrow();
            for (q, out) in r.queries.iter().zip(dst.as_chunks_mut::<8>().0) {
                let value = q.as_deref().map_or(0, |&q| {
                    // SAFETY: plain queries of a live query object, whose result is available.
                    unsafe {
                        if r.timestamps {
                            self.gl.get_query_parameter_u64(q, glow::QUERY_RESULT)
                        } else {
                            u64::from(self.gl.get_query_parameter_u32(q, glow::QUERY_RESULT))
                        }
                    }
                });
                *out = value.to_le_bytes();
            }
            drop(r);
            for q in readback.borrow_mut().queries.drain(..).flatten() {
                if let Ok(q) = Rc::try_unwrap(q) {
                    // SAFETY: no query set holds it any more.
                    unsafe { self.gl.delete_query(q) };
                }
            }
            return;
        };
        let r = readback.borrow();
        let mut wide = Vec::new();
        // SAFETY: plain GL calls; reading the pack buffer waits for the copy into it.
        let read = unsafe {
            self.gl.bind_buffer(glow::COPY_READ_BUFFER, Some(pbo));
            let read = if let Some(read) = r.repack {
                wide.resize(r.size, 0);
                let ok =
                    platform::get_buffer_sub_data(&self.gl, glow::COPY_READ_BUFFER, 0, &mut wide);
                read.repack(&wide, dst);
                ok
            } else {
                let n = r.size.min(dst.len());
                platform::get_buffer_sub_data(&self.gl, glow::COPY_READ_BUFFER, 0, &mut dst[..n])
            };
            if let Some(sync) = r.sync {
                self.gl.delete_sync(sync);
            }
            self.gl.delete_buffer(pbo);
            read
        };
        if !read {
            self.read_failed();
        }
    }

    /// A WebGL context draws into one canvas, the one the back end was made on, whatever window the
    /// host names; so a back end has one surface at a time. Natively the surface is an EGL window
    /// surface on a window the host named with `add_window`.
    fn create_surface(&mut self, window: u32) -> Result<Surface, GpuError> {
        self.platform.current();
        if self.surface.is_some() {
            return Err(GpuError::Invalid(
                "webgl: a back end has one surface, its context's canvas",
            ));
        }
        if let Err(e) = self.platform.attach(window) {
            self.last_log = format!("webgl: attaching to window {window}: {e}");
            return Err(GpuError::Invalid("webgl: the host has no such window"));
        }
        let id = self.id();
        self.surface = Some(SurfaceState { id, frame: None });
        Ok(Surface { id })
    }

    /// In the browser `vsync` changes nothing: the browser shows the canvas when it composites the
    /// page, at the display's rate, and presenting never waits. Natively it is the swap interval.
    fn configure_surface(
        &mut self,
        surface: &Surface,
        format: Format,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<(), GpuError> {
        self.platform.current();
        if self.surface.as_ref().map(|s| s.id) != Some(surface.id) {
            return Err(GpuError::Invalid("webgl: the surface was destroyed"));
        }
        // BGRA textures are stored as RGBA, so both blit into the RGBA drawing buffer as they are.
        if !matches!(format, Format::Rgba8Unorm | Format::Bgra8Unorm) {
            return Err(GpuError::Invalid(
                "webgl: surfaces take rgba8unorm and bgra8unorm only",
            ));
        }
        if let Err(e) = self.platform.resize(&self.gl, width, height, vsync) {
            self.last_log = format!("webgl: resizing the surface: {e}");
            return Err(GpuError::OutOfMemory);
        }
        let texture = self.create_texture(&TextureDesc {
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
        })?;
        let view = self.create_view(
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
        let old = self
            .surface
            .as_mut()
            .and_then(|s| s.frame.replace((texture, view)));
        if let Some((t, v)) = old {
            self.destroy(Object::TextureView(v));
            self.destroy(Object::Texture(t));
        }
        Ok(())
    }

    fn current_texture(&mut self, surface: &Surface) -> Result<TextureView, GpuError> {
        self.platform.current();
        match &self.surface {
            Some(s) if s.id == surface.id => match &s.frame {
                Some((_, view)) => Ok(view.clone()),
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
        self.platform.current();
        if let Some(reason) = self.lost {
            return Err(match reason {
                LossReason::OutOfMemory => GpuError::OutOfMemory,
                _ => GpuError::DeviceLost,
            });
        }
        let texture = match &self.surface {
            Some(s) if s.id == surface.id => match &s.frame {
                Some((t, _)) => t.clone(),
                None => return Err(GpuError::Invalid("webgl: the surface is not configured")),
            },
            _ => return Err(GpuError::Invalid("webgl: the surface was destroyed")),
        };
        self.read_framebuffer(&texture, 0, 0);
        self.open_writes();
        let (w, h) = (texture.desc.width as i32, texture.desc.height as i32);
        let (dw, dh) = self.platform.drawable_size(&self.gl);
        // SAFETY: plain GL calls; the frame's framebuffer is bound for reading.
        unsafe {
            self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
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
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
        }
        if let Err(e) = self.platform.swap() {
            self.last_log = format!("webgl: presenting: {e}");
            self.fail(LossReason::Removed);
            return Err(GpuError::DeviceLost);
        }
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
        self.platform.current();
        match src {
            ReadSource::Texture { region, extent } => self.read_texture_now(region, *extent, dst),
            ReadSource::Buffer {
                buffer,
                offset,
                size,
            } => {
                let raw = self.any_gl(buffer);
                // SAFETY: plain GL calls; the copy target takes buffers of either WebGL type.
                unsafe {
                    self.gl.bind_buffer(glow::COPY_READ_BUFFER, Some(raw));
                    platform::get_buffer_sub_data(
                        &self.gl,
                        glow::COPY_READ_BUFFER,
                        *offset as i32,
                        &mut dst[..*size as usize],
                    )
                }
                .then_some(())
                .ok_or_else(|| self.read_failed())
            }
        }
    }

    /// Clears each rectangle by `clearBuffer*` under the scissor, in one pass that loads the view.
    fn clear_rects(
        &mut self,
        view: &TextureView,
        format: Format,
        value: ClearValue,
        rects: &[ScissorRect],
    ) -> Result<(), GpuError> {
        self.platform.current();
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
                    view,
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
                    view,
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
        self.open_writes();
        let (gl, v) = (&self.gl, &mut self.values);
        // SAFETY: plain GL calls on the pass's framebuffer.
        unsafe {
            gl.enable(glow::SCISSOR_TEST);
            v.scissor_test = true;
            for r in rects {
                let rect = gl_rect(r, self.pass.as_ref().map_or(NO_PASS, |p| p.size));
                if v.scissor != rect {
                    gl.scissor(rect[0], rect[1], rect[2], rect[3]);
                    v.scissor = rect;
                }
                match (value, kind) {
                    (ClearValue::Color(c), FormatKind::Uint) => {
                        gl.clear_buffer_u32_slice(glow::COLOR, 0, &c)
                    }
                    (ClearValue::Color(c), FormatKind::Sint) => {
                        gl.clear_buffer_i32_slice(glow::COLOR, 0, &c.map(|x| x as i32))
                    }
                    (ClearValue::Color(c), _) => gl.clear_buffer_f32_slice(
                        glow::COLOR,
                        0,
                        &format::clear_value(view.texture.desc.format, c).map(f),
                    ),
                    (ClearValue::DepthStencil { .. }, _) => match (depth, stencil) {
                        (Some(d), Some(s)) => {
                            gl.clear_buffer_depth_stencil(glow::DEPTH_STENCIL, 0, f(d), s.into())
                        }
                        (Some(d), None) => gl.clear_buffer_f32_slice(glow::DEPTH, 0, &[f(d)]),
                        (None, Some(s)) => {
                            gl.clear_buffer_i32_slice(glow::STENCIL, 0, &[i32::from(s)])
                        }
                        (None, None) => {}
                    },
                }
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
        self.platform.current();
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
            return Helpers::generate_mips_by_draws(self, texture, desc, values, mips, layers);
        };
        if levels.is_empty() {
            return Ok(());
        }
        self.fresh(texture);
        self.bind_scratch(texture.target, raw);
        let range = (levels.start - 1, levels.end - 1);
        let gl = &self.gl;
        // SAFETY: plain GL calls on the bound texture.
        unsafe {
            if texture.levels.get() != range {
                gl.tex_parameter_i32(texture.target, glow::TEXTURE_BASE_LEVEL, range.0 as i32);
                gl.tex_parameter_i32(texture.target, glow::TEXTURE_MAX_LEVEL, range.1 as i32);
                texture.levels.set(range);
            }
            gl.generate_mipmap(texture.target);
        }
        self.written(texture);
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
        self.platform.current();
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
                                    self.unbuilt(
                                        "a fan or loop without restart whose lowered 32-bit \
                                         indices need a base vertex past 2^31 - 1",
                                    )
                                })?;
                        }
                        restart::Rewrite::Unreachable => {
                            return Err(self.unbuilt(
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
        self.platform.current();
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
        let outputs = &call.program.stages.0.outputs;
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
            (Topology::PointList, DrawRange::Vertices { .. }) => Some((glow::POINTS, 1)),
            (Topology::LineList, DrawRange::Vertices { .. }) => Some((glow::LINES, 2)),
            (Topology::TriangleList, DrawRange::Vertices { .. }) => Some((glow::TRIANGLES, 3)),
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
                    call.raster.0.1.provoking(),
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
                let Some(index) = &call.index else {
                    return Err(GpuError::Invalid("an indexed draw with no index buffer"));
                };
                let size = INDEX_FORMATS[index.format as usize].size;
                let raw = self.read_indices(index, *first, *count)?;
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
                    call.raster.0.1.provoking(),
                    numbers,
                    &mut list,
                );
                if negative {
                    return Err(self.unbuilt(
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
        let slot = &call.program.captures[kind.slot()];
        let capture = match slot.get() {
            Some(c) => c.clone(),
            None => {
                let id = self.id();
                let c = shader::relink(
                    &self.gl,
                    id,
                    call.program,
                    Some(kind),
                    0,
                    self.empty_fragment,
                )?;
                slot.get_or_init(|| Rc::new(c)).clone()
            }
        };
        let capture = Handle(capture);
        if self.linked(&capture).is_none() {
            return Err(GpuError::DeviceLost);
        }
        for t in targets {
            self.refresh(t.buffer, Some(false));
        }
        let gl = &self.gl;
        // SAFETY: plain GL calls; the targets' data GL buffers are alive and hold the ranges.
        unsafe {
            if discard {
                // Outside a pass nothing is drawn; the default framebuffer is always complete.
                if !rasterize && self.applied.draw_framebuffer.is_some() {
                    gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None);
                    self.applied.draw_framebuffer = None;
                }
                gl.enable(glow::RASTERIZER_DISCARD);
            }
            // WebGL refuses a draw while a capture target is bound anywhere else.
            for target in [
                glow::ARRAY_BUFFER,
                glow::COPY_READ_BUFFER,
                glow::COPY_WRITE_BUFFER,
            ] {
                gl.bind_buffer(target, None);
            }
            for (n, t) in targets.iter().enumerate() {
                gl.bind_buffer_range(
                    glow::TRANSFORM_FEEDBACK_BUFFER,
                    n as u32,
                    t.buffer.data,
                    t.offset as i32,
                    t.size as i32,
                );
            }
            if self.applied.program != capture.id {
                gl.use_program(Some(capture.raw));
                self.applied.program = capture.id;
                self.applied.sets = [Id::MAX; MAX_BINDING_SETS];
            }
            gl.begin_transform_feedback(native.map_or(glow::POINTS, |(mode, _)| mode));
        }
        if let DrawRange::Vertices {
            first,
            count,
            first_instance,
            instances,
        } = call.range
            && native.is_some()
        {
            self.draw(&DrawCall {
                program: &capture,
                index: None,
                range: DrawRange::Vertices {
                    first,
                    count,
                    first_instance,
                    instances,
                },
                ..*call
            });
        } else {
            self.record_slow_path(SlowPath::CaptureExpanded);
            for later in 0..instances {
                for (first, count) in capture::runs(&list) {
                    let run = DrawCall {
                        program: &capture,
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
                    self.draw_from(&run, later);
                }
            }
        }
        let gl = &self.gl;
        // SAFETY: plain GL calls ending what was begun above.
        unsafe {
            gl.end_transform_feedback();
            if discard {
                gl.disable(glow::RASTERIZER_DISCARD);
            }
            for n in 0..targets.len() {
                gl.bind_buffer_base(glow::TRANSFORM_FEEDBACK_BUFFER, n as u32, None);
            }
            gl.bind_buffer(glow::TRANSFORM_FEEDBACK_BUFFER, None);
        }
        for (n, t) in targets.iter().enumerate() {
            if t.buffer.index.is_some() {
                let end = t.offset + vertices * stride(n);
                t.buffer.maxima.borrow_mut().unknown(t.offset..end);
                self.mark_stale(t.buffer, true, t.offset, end);
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
        self.platform.current();
        if !self.caps.features.contains(Features::TIMESTAMPS) {
            return Err(GpuError::Absent(Features::TIMESTAMPS));
        }
        if set.kind != QueryKind::Timestamp || self.pass.is_some() {
            return Err(GpuError::Invalid(
                "write_timestamp into a set of another kind, or inside a pass",
            ));
        }
        let q = self.query_slot(set, index).ok_or(GpuError::DeviceLost)?;
        // SAFETY: a plain GL call the extension defines.
        unsafe { self.gl.query_counter(q, glow::TIMESTAMP) };
        Ok(())
    }

    /// `GPU_DISJOINT_EXT`, which reading clears.
    fn timestamps_disjoint(&mut self) -> Result<bool, GpuError> {
        self.platform.current();
        if !self.caps.features.contains(Features::TIMESTAMPS) {
            return Err(GpuError::Absent(Features::TIMESTAMPS));
        }
        // SAFETY: a plain query the extension defines.
        Ok(unsafe { self.gl.get_parameter_bool(platform::GPU_DISJOINT) })
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
