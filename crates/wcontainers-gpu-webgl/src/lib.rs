//! The WebGL 2 back end of the wcontainers graphics interface, written on glow.
//!
//! The same code draws through Chrome's WebGL 2 when built for WebAssembly and through ANGLE's OpenGL ES
//! 3.0 natively (a test tool only), following WebGL 2's rules on both. Blocks are applied by difference:
//! a block is looked at only when its handle changed, and then only GL values that differ are set. The
//! interface's row 0 is the top: with EXT_clip_control the device draws upper-left; without it, programs
//! negate y and the front face is swapped.

mod caps;
mod convert;
mod depth;
mod format;
pub mod platform;
mod shader;
mod state;

use glow::HasContext;
use shader::{ProgramObj, ShaderObj, Slot};
use state::GlValues;
use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use wcontainers_gpu::*;

/// Ids name objects in the state the back end last applied; they are never reused.
type Id = u64;

/// The sampler id a unit records when it holds the back end's nearest sampler, for textures read only
/// by loads.
const POINT_SAMPLER: Id = Id::MAX - 1;

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
}

/// A stale byte range of the index GL buffer (`index`) or of the data one.
#[derive(Clone, Copy, Debug)]
struct Stale {
    index: bool,
    start: u64,
    end: u64,
}

#[derive(Debug)]
pub struct TextureObj {
    id: Id,
    raw: glow::Texture,
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
    pbo: glow::Buffer,
    size: usize,
    sync: Option<glow::Fence>,
    /// Waited for: WebGL updates a sync object's status only between tasks, so after `finish` it is
    /// known done though the sync object does not say so yet.
    waited: bool,
    /// Texels read wider than they are stored, kept narrow when finished.
    repack: Option<format::ReadAs>,
}

#[derive(Debug)]
pub struct FenceObj {
    sync: glow::Fence,
    done: Cell<bool>,
}

/// A handle: a clone names the same object.
#[derive(Debug)]
pub struct Handle<T>(Rc<T>);

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

/// Query sets: not built yet.
#[derive(Clone, Debug)]
pub struct QuerySet;

/// WebGL 2 has no resource tables, so none is ever made.
#[derive(Clone, Debug)]
pub enum NoTable {}

/// The one surface a back end has: its context's canvas (natively, the EGL surface).
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

/// The WebGL 2 back end. Made on the device thread, which owns the GL context.
pub struct WebGl {
    gl: glow::Context,
    platform: platform::Platform,
    extensions: BTreeSet<&'static str>,
    renderer: String,
    caps: Caps,
    slow: SlowPaths,
    helpers: Helpers<WebGl>,
    lost: Option<LossReason>,
    next_id: Id,
    /// No EXT_clip_control: programs negate y and the front face is swapped.
    flip_y: bool,
    values: GlValues,
    applied: Applied,
    programs: HashMap<(Id, Id), GlProgram>,
    framebuffers: HashMap<FramebufferKey, glow::Framebuffer>,
    empty_fragment: glow::Shader,
    pass: Option<Pass>,
    /// How many buffers have a stale range, so draws look for them only when some do.
    stale_buffers: u32,
    /// How many sRGB twins exist, so draws look at their textures only when some do.
    twins: u32,
    /// GL's last compile or link log, for tests and bug reports.
    last_log: String,
    surface: Option<SurfaceState>,
    /// The depth and stencil read programs, by [`Fetch`], made at their first use.
    depth_read: [Option<depth::DepthRead>; 4],
    /// The stencil bit draws' program, made at its first use.
    bits_program: Option<glow::Program>,
    /// The programs of copies by draws, by source format, destination format and source target, with
    /// their uniform `at`.
    copy_programs: HashMap<(Format, Format, u32), (glow::Program, glow::UniformLocation)>,
    /// A nearest-filtering sampler for the back end's own fetches, made at its first use.
    point_sampler: Option<glow::Sampler>,
}

/// What the back end's own fetch draws read: depth or stencil, of a 2D or a 2D array texture.
#[derive(Clone, Copy, Debug)]
enum Fetch {
    Depth2D,
    DepthArray,
    Stencil2D,
    StencilArray,
}

impl WebGl {
    /// Makes the back end on a current GL ES 3.0 or WebGL 2 context. `off` lists WebGL extension names
    /// to treat as missing, so tests can take the routes of browsers without them.
    pub fn new(gl: glow::Context, platform: platform::Platform, off: &[&str]) -> WebGl {
        let extensions = platform::extensions(&gl, off);
        let renderer = platform::renderer(&gl);
        let caps = caps::caps(&gl, &extensions, &renderer);
        let flip_y = !extensions.contains("EXT_clip_control");
        let clip_depth = if flip_y {
            platform::NEGATIVE_ONE_TO_ONE
        } else {
            platform.clip_control(&gl, platform::UPPER_LEFT, platform::ZERO_TO_ONE);
            platform::ZERO_TO_ONE
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
        let mut b = WebGl {
            gl,
            platform,
            extensions,
            renderer,
            caps,
            slow: SlowPaths::default(),
            helpers: Helpers::default(),
            lost: None,
            next_id: 1,
            flip_y,
            values: GlValues::initial(clip_depth),
            applied: Applied::new(),
            programs: HashMap::new(),
            framebuffers: HashMap::new(),
            empty_fragment,
            pass: None,
            stale_buffers: 0,
            twins: 0,
            last_log: String::new(),
            surface: None,
            depth_read: [None, None, None, None],
            bits_program: None,
            copy_programs: HashMap::new(),
            point_sampler: None,
        };
        match b.probe_depth24() {
            Ok(unorm) => b.caps.features.set(Features::DEPTH24_UNORM, unorm),
            Err(log) => {
                b.own_program_failed(format!("webgl: the 24-bit depth probe: {log}"));
            }
        }
        b
    }

    /// The 24-bit depth probe (plan section 4.11): a triangle at depth 0.1 drawn into a 1 by 1
    /// `depth24plus` texture without and with a constant bias of one unit, each depth then fetched as
    /// float bits. One 24-bit step between them means 24-bit unsigned normalised storage, one float
    /// step 32-bit float storage; anything else is a bug of the back end's.
    fn probe_depth24(&mut self) -> Result<bool, String> {
        let error = |e: GpuError| format!("{e:?}");
        let (program, z) = depth::build_probe(&self.gl)?;
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
        let depth = texture(self, Format::Depth24Plus, TextureUsage::RENDER).map_err(error)?;
        let out = texture(self, Format::R32Uint, TextureUsage::RENDER).map_err(error)?;
        let fb = self.texture_framebuffer(&depth, 0, 0);
        let out_fb = self.texture_framebuffer(&out, 0, 0);
        let mut bits = [0u32; 2];
        for (units, bits) in [0.0f32, 1.0].into_iter().zip(&mut bits) {
            // SAFETY: plain GL calls; the program and the framebuffer are alive.
            unsafe {
                self.gl.use_program(Some(program));
                // Depth 0.1 in either clip depth convention.
                let clip = if self.values.clip_depth == platform::ZERO_TO_ONE {
                    0.1
                } else {
                    -0.8
                };
                self.gl.uniform_1_f32(Some(&z), clip);
                self.gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(fb));
                self.applied.draw_framebuffer = Some(fb);
                self.open_writes();
                self.gl.clear_buffer_f32_slice(glow::DEPTH, 0, &[1.0]);
            }
            state::plain(&self.gl, &mut self.values, [0, 0, 1, 1]);
            let (gl, v) = (&self.gl, &mut self.values);
            // SAFETY: plain GL state calls, recorded in the values so the next draw sets its own.
            unsafe {
                gl.enable(glow::DEPTH_TEST);
                v.depth_test = true;
                gl.depth_func(glow::ALWAYS);
                v.depth_func = glow::ALWAYS;
                gl.enable(glow::POLYGON_OFFSET_FILL);
                gl.polygon_offset(0.0, units);
                v.polygon_offset = Some((0.0, units));
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

    /// Makes the back end on ANGLE's OpenGL ES 3.0, loaded from `libEGL` in `dir` (on the desktop, the
    /// mozangle build angle\build.cmd makes). A test tool: ANGLE is never shipped.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn on_angle(dir: &std::path::Path, off: &[&str]) -> Result<WebGl, String> {
        let (gl, platform) = platform::angle(dir)?;
        Ok(WebGl::new(gl, platform, off))
    }

    /// Makes the back end on a canvas's WebGL 2 context.
    #[cfg(target_arch = "wasm32")]
    pub fn on_canvas(context: web_sys::WebGl2RenderingContext, off: &[&str]) -> WebGl {
        let (gl, platform) = platform::canvas(context);
        WebGl::new(gl, platform, off)
    }

    /// The renderer GL reports, which names the GPU.
    pub fn renderer(&self) -> &str {
        &self.renderer
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

    /// Records an error on GL's side as device loss, which fronts report in their API's words.
    fn fail(&mut self, reason: LossReason) {
        self.lost.get_or_insert(reason);
    }

    /// A read from a GL buffer that gave no bytes: the device is lost, so nothing reads stale bytes as
    /// an answer.
    fn read_failed(&mut self) -> GpuError {
        self.fail(LossReason::Error);
        GpuError::DeviceLost
    }

    /// A call whose route this back end does not have yet: the device is lost with the missing piece in
    /// [`WebGl::last_log`], so the front reports device loss instead of drawing wrong.
    fn unbuilt(&mut self, what: &'static str) -> GpuError {
        if self.lost.is_none() {
            self.last_log = format!("webgl: not built yet: {what}");
        }
        self.fail(LossReason::Error);
        GpuError::DeviceLost
    }

    /// A recording call that broke the interface's rules (a bug in a front): the device is lost with
    /// the rule in [`WebGl::last_log`], and nothing of the call runs.
    fn invalid(&mut self, rule: String) {
        if self.lost.is_none() {
            self.last_log = format!("webgl: invalid: {rule}");
        }
        self.fail(LossReason::Error);
    }

    /// The sRGB twin of `texture` in `format`, made at its first use: a second GL texture of the same
    /// shape, which [`WebGl::fresh`] keeps holding the same bytes.
    fn twin_of(&mut self, texture: &Texture, format: Format) -> Result<Texture, GpuError> {
        if let Some(t) = texture.twin.get() {
            return Ok(t.clone());
        }
        let readable = |f: Format| {
            format::read_as(f, format::FormatExtensions::found(&self.extensions))
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
    /// an `r8uint` texture of the same shape, which [`WebGl::fresh`] fills from the stencil by draws
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
            if other.desc.format.is_depth_stencil() {
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
    /// the same bytes.
    fn copy_whole(&mut self, from: &Texture, to: &Texture) {
        let d = from.desc;
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
            self.copy_through_pbo(&region(from), &region(to), extent);
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
        self.slow.hit(SlowPath::CopyThroughBuffer);
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
    /// convert.rs). False when no such draw carries the two formats, the source is a cube or the
    /// destination cannot be drawn into.
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
            _ => return false,
        };
        if !self.caps.formats.get(to).contains(FormatUse::RENDER) {
            return false;
        }
        let key = (from, to, src.texture.target);
        if !self.copy_programs.contains_key(&key) {
            let Some(source) = convert::program(from, to, sampler) else {
                return false;
            };
            let built = depth::link(&self.gl, &source).and_then(|p| {
                // SAFETY: a plain query of a linked program.
                unsafe { self.gl.get_uniform_location(p, "at") }
                    .map(|at| (p, at))
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
        self.slow.hit(SlowPath::CopyThroughBuffer);
        if self.bind_source(src.texture, src.mip).is_err() {
            self.fail(LossReason::OutOfMemory);
            return true;
        }
        let rect = [
            dst.origin.x as i32,
            dst.origin.y as i32,
            extent.width as i32,
            extent.height as i32,
        ];
        for i in 0..extent.depth_or_layers {
            let fb = self.texture_framebuffer(dst.texture, dst.mip, self.read_layer(dst, i));
            let (program, at) = &self.copy_programs[&key];
            // SAFETY: plain GL calls; the program is alive.
            unsafe {
                self.gl.use_program(Some(*program));
                self.gl.uniform_3_i32(
                    Some(at),
                    src.origin.x as i32 - rect[0],
                    src.origin.y as i32 - rect[1],
                    self.read_layer(src, i) as i32,
                );
            }
            self.own_draw_into(fb, rect);
        }
        true
    }

    /// Copies a region of a depth or depth-stencil texture into one of the same format by
    /// `blitFramebuffer`, layer by layer.
    fn blit_depth(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &TextureRegion<'_, Self>,
        extent: Extent,
    ) {
        let mask = match src.texture.desc.format.kind() {
            FormatKind::DepthStencil => glow::DEPTH_BUFFER_BIT | glow::STENCIL_BUFFER_BIT,
            _ => glow::DEPTH_BUFFER_BIT,
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
            if v.color_mask != [true; 4] {
                gl.color_mask(true, true, true, true);
                v.color_mask = [true; 4];
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
        self.applied.blend = 0;
        self.applied.depth_stencil = 0;
        self.applied.raster = 0;
    }

    /// The texels last presented, read from the default framebuffer, rows top first: for hosts that
    /// cannot see the window otherwise (natively, the EGL surface is off-screen).
    pub fn read_presented(&mut self) -> Vec<u8> {
        let Some((t, _)) = self.surface.as_ref().and_then(|s| s.frame.as_ref()) else {
            return Vec::new();
        };
        let (w, h) = (t.desc.width as usize, t.desc.height as usize);
        let mut texels = vec![0; w * h * 4];
        // SAFETY: plain GL calls; the read framebuffer binding is put back.
        unsafe {
            self.gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
            self.gl.read_pixels(
                0,
                0,
                w as i32,
                h as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut texels)),
            );
            self.gl
                .bind_framebuffer(glow::READ_FRAMEBUFFER, self.applied.read_framebuffer);
        }
        // GL's row 0 is the bottom of the window.
        texels
            .chunks_exact(w * 4)
            .rev()
            .flatten()
            .copied()
            .collect()
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
                    match t.desc.format.kind() {
                        FormatKind::DepthStencil => glow::DEPTH_STENCIL_ATTACHMENT,
                        FormatKind::Stencil => glow::STENCIL_ATTACHMENT,
                        _ => glow::DEPTH_ATTACHMENT,
                    }
                };
                match t.target {
                    glow::TEXTURE_2D => gl.framebuffer_texture_2d(
                        glow::DRAW_FRAMEBUFFER,
                        point,
                        glow::TEXTURE_2D,
                        Some(t.raw),
                        mip as i32,
                    ),
                    glow::TEXTURE_CUBE_MAP => gl.framebuffer_texture_2d(
                        glow::DRAW_FRAMEBUFFER,
                        point,
                        glow::TEXTURE_CUBE_MAP_POSITIVE_X + layer,
                        Some(t.raw),
                        mip as i32,
                    ),
                    _ => gl.framebuffer_texture_layer(
                        glow::DRAW_FRAMEBUFFER,
                        point,
                        Some(t.raw),
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
            let slot = if texture.desc.format.is_depth_stencil() {
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
            _ => return Err(self.unbuilt("depth read backs of cube textures")),
        };
        let bits = region.stencil && !self.extensions.contains("WEBGL_stencil_texturing");
        let kind = match kind {
            _ if bits => Fetch::Depth2D,
            Fetch::Depth2D if region.stencil => Fetch::Stencil2D,
            Fetch::DepthArray if region.stencil => Fetch::StencilArray,
            k => k,
        };
        self.slow.hit(SlowPath::DepthReadByDraw);
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
            match depth::build(
                &self.gl,
                matches!(kind, Fetch::DepthArray | Fetch::StencilArray),
                matches!(kind, Fetch::Stencil2D | Fetch::StencilArray),
            ) {
                Ok(p) => self.depth_read[k] = Some(p),
                Err(log) => return Err(self.own_program_failed(log)),
            }
        }
        self.bind_source(src, mip)?;
        let stencil = matches!(kind, Fetch::Stencil2D | Fetch::StencilArray);
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
        self.bind_scratch(src.target, src.raw);
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
        state::plain(&self.gl, &mut self.values, rect);
        self.own_draw();
    }

    /// Draws the back end's full-screen triangle with GL's lower-left window origin.
    fn own_draw(&mut self) {
        if !self.flip_y {
            self.platform
                .clip_control(&self.gl, platform::LOWER_LEFT, self.values.clip_depth);
        }
        // SAFETY: the caller bound the program, the target and the state.
        unsafe { self.gl.draw_arrays(glow::TRIANGLES, 0, 3) };
        if !self.flip_y {
            self.platform
                .clip_control(&self.gl, platform::UPPER_LEFT, self.values.clip_depth);
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
                depth::STENCIL_INDEX
            } else {
                depth::DEPTH_COMPONENT
            };
            // SAFETY: a plain GL call on the bound texture.
            unsafe {
                self.gl.tex_parameter_i32(
                    texture.target,
                    depth::DEPTH_STENCIL_TEXTURE_MODE,
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
        self.slow.hit(SlowPath::StencilCopiedByDraw);
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
        state::plain(&self.gl, &mut self.values, [0, 0, w as i32, h as i32]);
        let (gl, v) = (&self.gl, &mut self.values);
        let keep = [glow::KEEP; 3];
        // SAFETY: plain GL state calls, recorded in the values so the next draw sets its own.
        unsafe {
            gl.clear_buffer_f32_slice(glow::COLOR, 0, &[0.0; 4]);
            gl.enable(glow::BLEND);
            v.blend = true;
            gl.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);
            v.blend_equation = [glow::FUNC_ADD; 2];
            let func = [
                glow::CONSTANT_COLOR,
                glow::ONE,
                glow::CONSTANT_COLOR,
                glow::ONE,
            ];
            gl.blend_func_separate(func[0], func[1], func[2], func[3]);
            v.blend_func = func;
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
    fn upload(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        texels: Texels<'_>,
        row_pitch: u32,
        rows_per_layer: u32,
    ) {
        let t = region.texture.clone();
        let (_, _, block_bytes) = t.desc.format.block();
        self.bind_scratch(t.target, t.raw);
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

    /// Reads one region of a colour texture into `dst`, rows tightly packed, now.
    /// Calls `read` on a region whose depth was drawn into a colour texture, when the region is depth
    /// (None otherwise), then lets the colour texture go.
    fn through_depth<R>(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        read: impl FnOnce(&mut Self, &TextureRegion<'_, Self>) -> Result<R, GpuError>,
    ) -> Option<Result<R, GpuError>> {
        depth::stored(region.texture.desc.format)?;
        Some(self.depth_by_draw(region, extent).and_then(|scratch| {
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

    fn read_texture_now(
        &mut self,
        region: &TextureRegion<'_, Self>,
        extent: Extent,
        dst: &mut [u8],
    ) -> Result<(), GpuError> {
        if let Some(r) = self.through_depth(region, extent, |b, colour| {
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
        if read.texel_read() != read.texel_kept() {
            wide.resize(texels * read.texel_read(), 0);
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
        self.slow.hit(SlowPath::IndexTwinReadBack);
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
            self.slow.hit(SlowPath::PipelineMiss);
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
        let point = if linked.units.iter().any(|(_, s)| s.is_none()) {
            match self.point_sampler() {
                Ok(s) => Some(s),
                Err(_) => return Err(Unbindable::NoSampler),
            }
        } else {
            None
        };
        for (unit, (texture, sampler)) in linked.units.iter().enumerate() {
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
            let ids = (v.id, sampler.map_or(POINT_SAMPLER, |s| s.id));
            let t = &v.texture;
            let levels = (v.desc.base_mip, v.desc.base_mip + v.desc.mip_count - 1);
            let aspect = t.desc.format.is_depth_stencil() && t.stencil_mode.get() != v.desc.stencil;
            if self.applied.units[unit] != ids || t.levels.get() != levels || aspect {
                // SAFETY: plain GL calls; the texture is alive.
                unsafe {
                    self.gl.active_texture(glow::TEXTURE0 + unit as u32);
                    self.gl.bind_texture(t.target, Some(t.raw));
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
            if self.applied.units[unit].1 != ids.1 {
                // SAFETY: a plain GL call; the sampler is alive.
                unsafe { gl.bind_sampler(unit as u32, sampler.map(|s| s.raw).or(point)) };
            }
            self.applied.units[unit] = ids;
        }
        // The stencil mode is the texture's: one draw cannot fetch both aspects of one texture.
        for (texture, _) in &linked.units {
            if let Some(Bound::Texture(v)) = find(*texture)
                && v.texture.desc.format.is_depth_stencil()
                && v.texture.stencil_mode.get() != v.desc.stencil
            {
                return Err(Unbindable::BothAspects);
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
    /// (WebGL 2 has no draws with bases: [`SlowPath::BaseVertexRebind`]). Err when a stream's offset
    /// would fall below zero.
    fn apply_vertices(
        &mut self,
        call: &DrawCall<'_, Self>,
        base_vertex: i64,
        first_instance: u32,
    ) -> Result<(), ()> {
        let layout = &call.vertex_layout.0.1;
        let mut wanted = [None; MAX_VERTEX_STREAMS];
        for (w, a) in wanted.iter_mut().zip(&layout.attributes) {
            let Some(binding) = call.vertex_buffers.get(a.stream).and_then(|b| b.as_ref()) else {
                *w = Some((a, None, 0));
                continue;
            };
            let moved = if a.per_instance {
                i64::from(first_instance)
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

impl Backend for WebGl {
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
        let size = i32::try_from(desc.size).map_err(|_| GpuError::OutOfMemory)?;
        let usage = match desc.intent {
            MapIntent::Rarely => glow::STATIC_DRAW,
            MapIntent::Often => glow::DYNAMIC_DRAW,
        };
        let copies = BufferUsage::COPY_SRC | BufferUsage::COPY_DST | BufferUsage::READ_BACK;
        let indices = desc.usage.contains(BufferUsage::INDEX);
        let other = !(desc.usage - BufferUsage::INDEX - copies).is_empty() || !indices;
        if indices && other {
            self.slow.hit(SlowPath::IndexBufferTwin);
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
        })))
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<Texture, GpuError> {
        let gl_format = format::gl_format(desc.format)
            .filter(|_| self.caps.formats.get(desc.format) != FormatUse::empty())
            .ok_or(GpuError::Invalid(
                "webgl: the format has no capabilities here",
            ))?;
        if desc.samples > 1 {
            return Err(GpuError::Invalid(
                "webgl: the caps give every format one sample",
            ));
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
            raw,
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
        })))
    }

    fn create_view(
        &mut self,
        texture: &Texture,
        desc: &TextureViewDesc,
    ) -> Result<TextureView, GpuError> {
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
            if t.format.kind() != FormatKind::DepthStencil {
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
        } else if desc.format.linear() == t.format.linear() {
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
        let address = |a| match a {
            AddressMode::Repeat => Ok(glow::REPEAT),
            AddressMode::Mirror => Ok(glow::MIRRORED_REPEAT),
            AddressMode::Clamp => Ok(glow::CLAMP_TO_EDGE),
            AddressMode::Border => Err(GpuError::Absent(Features::BORDER_COLOR)),
            AddressMode::MirrorOnce => Err(GpuError::Absent(Features::MIRROR_ONCE)),
        };
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
        let raw = unsafe {
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
            gl.sampler_parameter_f32(raw, glow::TEXTURE_MIN_LOD, f(desc.lod_range[0]));
            gl.sampler_parameter_f32(raw, glow::TEXTURE_MAX_LOD, f(desc.lod_range[1]));
            if let Some(c) = desc.compare {
                gl.sampler_parameter_i32(
                    raw,
                    glow::TEXTURE_COMPARE_MODE,
                    glow::COMPARE_REF_TO_TEXTURE as i32,
                );
                gl.sampler_parameter_i32(raw, glow::TEXTURE_COMPARE_FUNC, state::compare(c) as i32);
            }
            if desc.anisotropy > 1 && self.extensions.contains("EXT_texture_filter_anisotropic") {
                let a = desc.anisotropy.min(self.caps.limits.max_anisotropy);
                gl.sampler_parameter_f32(raw, glow::TEXTURE_MAX_ANISOTROPY_EXT, f32::from(a));
            }
            raw
        };
        Ok(Handle(Rc::new(SamplerObj { id: self.id(), raw })))
    }

    fn create_shader(&mut self, module: &ShaderModule) -> Result<Shader, GpuError> {
        if module.stage == ShaderStage::Compute {
            return Err(GpuError::Absent(Features::COMPUTE));
        }
        let id = self.id();
        let shader = shader::compile(&self.gl, id, module, self.flip_y)?;
        Ok(Handle(Rc::new(shader)))
    }

    fn create_program(&mut self, program: &Program<'_, Self>) -> Result<GlProgram, GpuError> {
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
        if desc.logic_op.is_some() {
            return Err(GpuError::Absent(Features::LOGIC_OP));
        }
        let used = if desc.same_for_all {
            &desc.targets[..1]
        } else {
            &desc.targets[..]
        };
        if !desc.same_for_all && used.iter().any(|t| *t != used[0]) {
            return Err(GpuError::Absent(Features::INDEPENDENT_BLEND));
        }
        let second = used.iter().filter_map(|t| t.blend).any(|(c, a)| {
            [c.src, c.dst, a.src, a.dst]
                .into_iter()
                .any(state::second_source)
        });
        if second {
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
        let absent = [
            (desc.fill != FillMode::Solid, Features::POLYGON_MODE),
            (
                desc.provoking == ProvokingVertex::First
                    && !self.caps.has(Features::PROVOKING_VERTEX_FIRST),
                Features::PROVOKING_VERTEX_FIRST,
            ),
            (!desc.depth_clip, Features::DEPTH_CLAMP),
            (f(desc.depth_bias_clamp) != 0.0, Features::DEPTH_BIAS_CLAMP),
            (f(desc.line_width) != 1.0, Features::WIDE_LINES),
            (desc.sample_mask != u32::MAX, Features::SAMPLE_MASK),
            (
                !desc.depth_minus_one && self.flip_y,
                Features::CLIP_DEPTH_ZERO_ONE,
            ),
        ];
        if let Some((_, feature)) = absent.into_iter().find(|(missing, _)| *missing) {
            return Err(GpuError::Absent(feature));
        }
        Ok(Handle(Rc::new((
            self.id(),
            state::GlRaster::new(desc, self.flip_y),
        ))))
    }

    fn create_vertex_layout(
        &mut self,
        desc: &VertexLayoutDesc,
    ) -> Result<Block<state::GlLayout>, GpuError> {
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
        match state::GlLayout::new(desc) {
            Some(layout) => Ok(Handle(Rc::new((self.id(), layout)))),
            None => Err(self.unbuilt("vertex formats GL ES cannot read as stored")),
        }
    }

    fn create_binding_set(
        &mut self,
        entries: &[BindingEntry<'_, Self>],
    ) -> Result<BindingSet, GpuError> {
        let mut bound = Vec::with_capacity(entries.len());
        for e in entries {
            let r = match &e.resource {
                BindingResource::UniformBuffer(r) => {
                    Bound::Uniform(r.buffer.clone(), r.offset, r.size)
                }
                BindingResource::Texture(v) => Bound::Texture((*v).clone()),
                BindingResource::Sampler(s) => Bound::Sampler((*s).clone()),
                BindingResource::StorageBuffer { .. } | BindingResource::StorageTexture { .. } => {
                    return Err(GpuError::Absent(
                        Features::FRAGMENT_STORAGE_WRITE | Features::VERTEX_STORAGE_READ,
                    ));
                }
                BindingResource::TexelBuffer { .. } => {
                    return Err(GpuError::Invalid(
                        "webgl: texel buffers are read as textures by the front",
                    ));
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

    fn create_query_set(&mut self, _kind: QueryKind, _count: u32) -> Result<QuerySet, GpuError> {
        Err(self.unbuilt("queries"))
    }

    fn destroy(&mut self, object: Object<Self>) {
        if let Object::Surface(surface) = &object {
            if self.surface.as_ref().is_some_and(|s| s.id == surface.id)
                && let Some((t, v)) = self.surface.take().and_then(|s| s.frame)
            {
                self.destroy(Object::TextureView(v));
                self.destroy(Object::Texture(t));
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
                    gl.delete_texture(t.raw);
                }
                Object::Sampler(s) => gl.delete_sampler(s.raw),
                Object::Shader(s) => gl.delete_shader(s.raw),
                Object::Program(p) => {
                    p.handles.set(p.handles.get() - 1);
                    if p.handles.get() == 0 {
                        self.programs.remove(&p.pair);
                        gl.delete_program(p.raw);
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
                | Object::QuerySet(_)
                | Object::Surface(_) => {}
                Object::ResourceTable(t) => match t {},
            }
        }
    }

    fn write_buffer(&mut self, buffer: &Buffer, offset: u64, data: &[u8], mode: WriteMode) {
        if offset + data.len() as u64 > buffer.size {
            self.fail(LossReason::Error);
            return;
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
        self.fresh(region.texture);
        self.written(region.texture);
        self.upload(
            region,
            extent,
            Texels::Bytes(data),
            row_pitch,
            rows_per_layer,
        );
    }

    fn begin_pass(&mut self, pass: &PassDesc<'_, Self>) {
        if pass.empty.is_some()
            || pass.occlusion.is_some()
            || pass.colors.iter().flatten().any(|c| c.resolve.is_some())
        {
            self.unbuilt("empty passes, occlusion queries and resolves");
            return;
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
        if self.twins > 0 {
            for v in views.iter().flatten() {
                self.fresh(&v.texture);
                self.written(&v.texture);
            }
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
            for (i, c) in pass.colors.iter().enumerate() {
                let Some(c) = c else { continue };
                let point = glow::COLOR_ATTACHMENT0 + i as u32;
                match c.load {
                    LoadOp::Clear(value) => match c.view.texture.desc.format.kind() {
                        FormatKind::Uint => {
                            gl.clear_buffer_u32_slice(glow::COLOR, i as u32, &value)
                        }
                        FormatKind::Sint => gl.clear_buffer_i32_slice(
                            glow::COLOR,
                            i as u32,
                            &value.map(|x| x as i32),
                        ),
                        _ => gl.clear_buffer_f32_slice(glow::COLOR, i as u32, &value.map(f)),
                    },
                    LoadOp::DontCare => start_discard.push(point),
                    LoadOp::Load => {}
                }
                if c.store == StoreOp::Discard {
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
                    if let Some((load, store)) = ops {
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
        });
    }

    fn end_pass(&mut self) {
        if let Some(pass) = self.pass.take()
            && pass.discard.len > 0
        {
            // SAFETY: the pass's framebuffer is still bound.
            unsafe {
                self.gl
                    .invalidate_framebuffer(glow::DRAW_FRAMEBUFFER, pass.discard.get())
            };
        }
    }

    fn draw(&mut self, call: &DrawCall<'_, Self>) {
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
                self.fail(LossReason::Error);
                return;
            }
        };
        let program = call.program;
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
        }
        let (gl, v) = (&self.gl, &mut self.values);
        if self.applied.blend != call.blend.0.0 {
            state::blend(gl, v, &call.blend.0.1);
            self.applied.blend = call.blend.0.0;
        }
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
                let rect = [
                    f(vp.x) as i32,
                    f(vp.y) as i32,
                    f(vp.width) as i32,
                    f(vp.height) as i32,
                ];
                if v.viewport != rect {
                    gl.viewport(rect[0], rect[1], rect[2], rect[3]);
                    v.viewport = rect;
                }
                let range = [f(vp.min_depth), f(vp.max_depth)];
                if v.depth_range != range {
                    gl.depth_range_f32(range[0], range[1]);
                    v.depth_range = range;
                }
            }
            if v.scissor_test
                && let Some(s) = call.scissors.first()
            {
                let rect = [s.x as i32, s.y as i32, s.width as i32, s.height as i32];
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
        }
        if base_vertex != 0 || first_instance != 0 {
            self.slow.hit(SlowPath::BaseVertexRebind);
        }
        if self
            .apply_vertices(call, base_vertex, first_instance)
            .is_err()
        {
            self.unbuilt("negative stream offsets after the base vertex");
            return;
        }
        // The streams moved by the bases, so GL's ids lack them: the program adds them back.
        for (uniform, value) in [
            (&linked.first_instance, first_instance),
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
                    self.fail(LossReason::Error);
                    return;
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
                let offset = index.offset + u64::from(first) * u64::from(index.format.size());
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

    fn begin_query(&mut self, _index: u32) {
        self.unbuilt("queries");
    }

    fn end_query(&mut self) {
        self.unbuilt("queries");
    }

    fn copy_buffer(
        &mut self,
        src: &Buffer,
        src_offset: u64,
        dst: &Buffer,
        dst_offset: u64,
        size: u64,
    ) {
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
            self.unbuilt("copies between index-only and data-only buffers");
            return;
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
        let (from, to) = (src.texture.desc.format, dst.texture.desc.format);
        self.fresh(src.texture);
        self.fresh(dst.texture);
        self.written(dst.texture);
        if from.is_depth_stencil() || to.is_depth_stencil() {
            if from != to {
                self.unbuilt("copies between depth and colour formats");
            } else {
                self.blit_depth(src, dst, extent);
            }
            return;
        }
        let renderable = |f| self.caps.formats.get(f).contains(FormatUse::RENDER);
        if from != to || !renderable(from) {
            let as_stored = src
                .texture
                .read
                .is_some_and(|r| r.texel_read() == r.texel_kept());
            if as_stored && from.block() == to.block() {
                self.copy_through_pbo(src, dst, extent);
            } else if !self.copy_by_draw(src, dst, extent) {
                self.unbuilt("copies of these formats");
            }
            return;
        }
        let d = dst.texture.clone();
        for i in 0..extent.depth_or_layers {
            let layer = self.read_layer(src, i);
            self.read_framebuffer(src.texture, src.mip, layer);
            self.bind_scratch(d.target, d.raw);
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
        let Some(data) = src.buffer.data else {
            self.unbuilt("copies into textures from index-only buffers");
            return;
        };
        self.refresh(src.buffer, Some(false));
        self.fresh(dst.texture);
        self.written(dst.texture);
        // SAFETY: a plain GL call; the buffer holds other data.
        unsafe { self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(data)) };
        self.upload(
            dst,
            extent,
            Texels::Offset(src.offset as u32),
            src.row_pitch,
            src.rows_per_layer,
        );
        // SAFETY: a plain GL call; host uploads need no unpack buffer bound.
        unsafe { self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None) };
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &TextureRegion<'_, Self>,
        dst: &CopyBufferTexture<'_, Self>,
        extent: Extent,
    ) {
        if self
            .through_depth(src, extent, |b, colour| {
                b.copy_texture_to_buffer(colour, dst, extent);
                Ok(())
            })
            .is_some()
        {
            return;
        }
        self.fresh(src.texture);
        let read = src
            .texture
            .read
            .filter(|r| r.texel_read() == r.texel_kept());
        let (Some(read), Some(data)) = (read, dst.buffer.data) else {
            self.unbuilt(
                "copies into buffers of formats read wider than stored, and into index-only buffers",
            );
            return;
        };
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
            self.mark_stale(dst.buffer, true, dst.offset, end);
        }
    }

    /// WebGL 2 has no buffer clear: one block of the pattern is written from the host into every GL
    /// buffer of `buffer`, then doubled by copies within each ([`SlowPath::PatternClearCopied`]).
    fn clear_buffer(&mut self, buffer: &Buffer, offset: u64, size: u64, pattern: &[u8]) {
        if size == 0 {
            return;
        }
        if pattern.is_empty() || offset + size > buffer.size {
            self.fail(LossReason::Error);
            return;
        }
        self.slow.hit(SlowPath::PatternClearCopied);
        // A multiple of every pattern length (1 to 16 bytes); `size` is whole patterns.
        const BLOCK: usize = 256;
        let mut block = [0u8; BLOCK];
        for (b, p) in block.iter_mut().zip(pattern.iter().cycle()) {
            *b = *p;
        }
        let first = size.min(BLOCK as u64);
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
    }

    fn resolve(
        &mut self,
        _src: &TextureRegion<'_, Self>,
        _dst: &TextureRegion<'_, Self>,
        _format: Format,
    ) {
        self.unbuilt("resolves");
    }

    fn submit(&mut self) -> Fence {
        let gl = &self.gl;
        // SAFETY: plain GL calls.
        let sync = unsafe {
            // An error GL reports here is a bug of ours or a lost context: the device is lost.
            match gl.get_error() {
                glow::NO_ERROR => {}
                glow::OUT_OF_MEMORY => self.lost = self.lost.or(Some(LossReason::OutOfMemory)),
                glow::CONTEXT_LOST => self.lost = self.lost.or(Some(LossReason::Reset)),
                _ => self.lost = self.lost.or(Some(LossReason::Error)),
            }
            let sync = gl
                .fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0)
                .expect("a sync object");
            gl.flush();
            sync
        };
        Handle(Rc::new(FenceObj {
            sync,
            done: Cell::new(false),
        }))
    }

    fn fence_done(&mut self, fence: &Fence) -> bool {
        if !fence.done.get() {
            // SAFETY: a plain query of a live sync object.
            let status = unsafe { self.gl.get_sync_status(fence.sync) };
            fence.done.set(status == glow::SIGNALED);
        }
        fence.done.get()
    }

    fn wait_fence(&mut self, fence: &Fence) {
        if fence.done.get() {
            return;
        }
        // SAFETY: plain GL calls. WebGL cannot wait on a sync object (its longest wait is zero), and
        // finish returns only when every earlier command has completed.
        unsafe {
            #[cfg(target_arch = "wasm32")]
            self.gl.finish();
            #[cfg(not(target_arch = "wasm32"))]
            while self
                .gl
                .client_wait_sync(fence.sync, glow::SYNC_FLUSH_COMMANDS_BIT, 1_000_000_000)
                == glow::TIMEOUT_EXPIRED
            {}
        }
        fence.done.set(true);
    }

    fn read_back(&mut self, src: &ReadSource<'_, Self>) -> Result<Readback, GpuError> {
        if let ReadSource::Texture { region, extent } = src
            && let Some(r) = self.through_depth(region, *extent, |b, colour| {
                b.read_back(&ReadSource::Texture {
                    region: TextureRegion { ..*colour },
                    extent: *extent,
                })
            })
        {
            return r;
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
        .filter(|r| r.texel_read() != r.texel_kept());
        Ok(Handle(Rc::new(core::cell::RefCell::new(ReadbackObj {
            pbo,
            size,
            sync,
            waited: false,
            repack,
        }))))
    }

    fn read_queries(
        &mut self,
        _set: &QuerySet,
        _first: u32,
        _count: u32,
    ) -> Result<Readback, GpuError> {
        Err(self.unbuilt("queries"))
    }

    /// Ready once the copy's sync object is signalled; a read back made without one is ready at once and
    /// reads nothing, the device having been lost when it was made.
    fn read_state(&mut self, readback: &Readback) -> ReadState {
        let r = readback.borrow();
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
        let sync = readback.borrow().sync;
        if let Some(sync) = sync {
            let fence = Handle(Rc::new(FenceObj {
                sync,
                done: Cell::new(false),
            }));
            self.wait_fence(&fence);
            readback.borrow_mut().waited = true;
        }
    }

    fn finish_read(&mut self, readback: Readback, dst: &mut [u8]) {
        let r = readback.borrow();
        let mut wide = Vec::new();
        // SAFETY: plain GL calls; reading the pack buffer waits for the copy into it.
        let read = unsafe {
            self.gl.bind_buffer(glow::COPY_READ_BUFFER, Some(r.pbo));
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
            self.gl.delete_buffer(r.pbo);
            read
        };
        if !read {
            self.read_failed();
        }
    }

    /// A WebGL context draws into one canvas, the one the back end was made on, whatever window the
    /// host names; so a back end has one surface at a time.
    fn create_surface(&mut self, _window: u32) -> Result<Surface, GpuError> {
        if self.surface.is_some() {
            return Err(GpuError::Invalid(
                "webgl: a back end has one surface, its context's canvas",
            ));
        }
        let id = self.id();
        self.surface = Some(SurfaceState { id, frame: None });
        Ok(Surface { id })
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
        if self.surface.as_ref().map(|s| s.id) != Some(surface.id) {
            return Err(GpuError::Invalid("webgl: the surface was destroyed"));
        }
        if format != Format::Rgba8Unorm {
            return Err(GpuError::Invalid("webgl: surfaces take rgba8unorm only"));
        }
        if let Err(e) = self.platform.resize(&self.gl, width, height) {
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
        match &self.surface {
            Some(s) if s.id == surface.id => match &s.frame {
                Some((_, view)) => Ok(view.clone()),
                None => Err(GpuError::Invalid("webgl: the surface is not configured")),
            },
            _ => Err(GpuError::Invalid("webgl: the surface was destroyed")),
        }
    }

    /// Copies the frame into the drawing buffer, upside down since GL's row 0 is the bottom of the
    /// window; the browser shows it when the device thread returns to its event loop.
    fn present(&mut self, surface: &Surface) -> Result<(), GpuError> {
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
                h,
                w,
                0,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
        }
        Ok(())
    }

    /// Answers inside the call: `readPixels` and `getBufferSubData` wait for the GPU themselves.
    fn read_back_now(
        &mut self,
        src: &ReadSource<'_, Self>,
        dst: &mut [u8],
    ) -> Result<(), GpuError> {
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

    /// Draws `TRIANGLE_FAN` and `LINE_LOOP` as they are. GL's tables give the vertex the raster block's
    /// convention names, which is the one `provoking` names when the front's block follows its API, as
    /// the converted list of the default assumes too. Guest indices go into the index ring once; GL
    /// restarts at the largest value always, and a front whose API turns restart off rewrites them, as
    /// for every draw.
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
                let width = format.size() as usize;
                let count = indices.len() / width;
                if count == 0 {
                    return Ok(());
                }
                let mut offset = [0];
                let ring = self.write_buffer_ring(
                    RingKind::Indices,
                    &[&indices[..count * width]],
                    width as u32,
                    &mut offset,
                )?;
                self.draw(&DrawCall {
                    topology,
                    restart: true,
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
}
