//! Gallium's state objects as the interface's blocks, and the state st/mesa sets between draws.

use std::ffi::{c_uint, c_void};

use wcontainers_gpu::Gpu;
use wcontainers_nir::mesa::Assembled;
use wcontainers_shaders::stage::{STREAM_BUFFERS, StreamOutput};

use crate::context::{self, Context};
use crate::draw::ShaderId;
use crate::gallium::*;
use crate::resource;
use crate::screen::Screen;
use crate::stream;

/// One shader's NIR, owned by Mesa's compiler through our bridge, the one the driver is linked into.
pub type Nir = wcontainers_nir::mesa::Nir<wcontainers_mesa_compiler_direct::Bridge>;

pub use crate::guest::{DEVICE_LOST, FAILED_CALLS, refuse, refused};

/// Runs one of Gallium's calls into the driver for the context `pipe` (null for the screen's), as
/// [`crate::guest::guest_call`] does for its key.
pub fn guest_call<T>(pipe: *mut pipe_context, dropped: T, call: impl FnOnce() -> T) -> T {
    crate::guest::guest_call(pipe.addr(), dropped, call)
}

/// Forgets a destroyed context's reset, so that one made at its address starts afresh.
pub fn forget_reset(pipe: *mut pipe_context) {
    crate::guest::forget_reset(pipe.addr());
}

pub use crate::states::{
    Blend, COLOR_CHANNELS, DepthStencil, Elements, Raster, Sampler, VERTEX_ALIGNMENT,
};

/// The program stages the driver takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Vertex,
    Fragment,
    Compute,
    Geometry,
    TessControl,
    TessEvaluation,
}

impl Stage {
    pub const ALL: [Stage; 6] = [
        Stage::Vertex,
        Stage::Fragment,
        Stage::Compute,
        Stage::Geometry,
        Stage::TessControl,
        Stage::TessEvaluation,
    ];

    /// The route's stage of a program: geometry and tessellation control programs run as the
    /// stage helper's compute work, which binds their resources in compute's set; an evaluation
    /// program draws the tessellated points as a vertex program does.
    pub fn nir(self) -> wcontainers_nir::mesa::Stage {
        match self {
            Stage::Vertex | Stage::TessEvaluation => wcontainers_nir::mesa::Stage::Vertex,
            Stage::Fragment => wcontainers_nir::mesa::Stage::Fragment,
            Stage::Compute | Stage::Geometry | Stage::TessControl => {
                wcontainers_nir::mesa::Stage::Compute
            }
        }
    }
}

/// One uniform buffer a stage reads: a range of a resource, or bytes the frontend handed over.
#[derive(Default)]
pub struct Constants {
    pub resource: *mut pipe_resource,
    pub offset: u32,
    pub size: u32,
    pub bytes: Vec<u8>,
}

/// One storage buffer a stage binds: a range of a resource, holding a reference on it.
pub struct ShaderBuffer {
    pub resource: *mut pipe_resource,
    pub offset: u32,
    pub size: u32,
    pub writable: bool,
}

impl Default for ShaderBuffer {
    fn default() -> Self {
        ShaderBuffer {
            resource: std::ptr::null_mut(),
            offset: 0,
            size: 0,
            writable: false,
        }
    }
}

/// What one stage has bound.
pub struct StageState<B: Gpu> {
    pub constants: Vec<Constants>,
    pub views: Vec<*mut pipe_sampler_view>,
    pub samplers: Vec<*const Sampler<B>>,
    pub buffers: Vec<ShaderBuffer>,
    /// Gallium's image views, each holding a reference on its resource (null where unbound).
    pub images: Vec<pipe_image_view>,
    pub shader: *const Shader,
}

impl<B: Gpu> Default for StageState<B> {
    fn default() -> Self {
        StageState {
            constants: (0..PIPE_MAX_CONSTANT_BUFFERS)
                .map(|_| Constants::default())
                .collect(),
            views: vec![std::ptr::null_mut(); PIPE_MAX_SHADER_SAMPLER_VIEWS as usize],
            samplers: vec![std::ptr::null(); PIPE_MAX_SAMPLERS as usize],
            buffers: (0..PIPE_MAX_SHADER_BUFFERS)
                .map(|_| ShaderBuffer::default())
                .collect(),
            images: vec![pipe_image_view::default(); PIPE_MAX_SHADER_IMAGES as usize],
            shader: std::ptr::null(),
        }
    }
}

impl<B: Gpu> StageState<B> {
    /// Drops the references the storage buffers and images hold.
    fn release_storage(&mut self) {
        for buffer in &mut self.buffers {
            // SAFETY: each slot holds one reference or none.
            unsafe { resource::reference(&mut buffer.resource, std::ptr::null_mut()) };
        }
        for image in &mut self.images {
            // SAFETY: as above.
            unsafe { resource::reference(&mut image.resource, std::ptr::null_mut()) };
        }
    }
}

/// Every stage's bindings.
pub struct Stages<B: Gpu>(pub [StageState<B>; Stage::ALL.len()]);

impl<B: Gpu> Default for Stages<B> {
    fn default() -> Self {
        Stages(std::array::from_fn(|_| StageState::default()))
    }
}

impl<B: Gpu> Stages<B> {
    pub fn get(&self, stage: Stage) -> &StageState<B> {
        &self.0[stage as usize]
    }

    pub(crate) fn get_mut(&mut self, stage: Stage) -> &mut StageState<B> {
        &mut self.0[stage as usize]
    }

    /// Drops the references every stage's storage holds, with the context.
    pub fn release(&mut self) {
        self.0.iter_mut().for_each(StageState::release_storage);
    }
}

/// A bound vertex stream: a resource holding a reference, and where in it the stream starts.
pub struct VertexBuffer {
    pub resource: *mut pipe_resource,
    pub offset: u32,
}

/// The state set between draws that is not a stage's.
pub struct Bound<B: Gpu> {
    pub blend: *const Blend,
    pub depth_stencil: *const DepthStencil<B>,
    pub raster: *const Raster<B>,
    pub elements: *const Elements<B>,
    pub vertex_buffers: Vec<VertexBuffer>,
    pub blend_color: [u32; COLOR_CHANNELS],
    pub stencil_ref: [u8; 2],
    pub sample_mask: u32,
    pub viewport: pipe_viewport_state,
    pub scissor: pipe_scissor_state,
    /// The control points of a patch (`set_patch_vertices`).
    pub patch_vertices: u8,
}

impl<B: Gpu> Default for Bound<B> {
    fn default() -> Self {
        Bound {
            blend: std::ptr::null(),
            depth_stencil: std::ptr::null(),
            raster: std::ptr::null(),
            elements: std::ptr::null(),
            vertex_buffers: Vec::new(),
            blend_color: Default::default(),
            stencil_ref: [0; 2],
            sample_mask: u32::MAX,
            viewport: pipe_viewport_state::default(),
            scissor: pipe_scissor_state::default(),
            patch_vertices: 0,
        }
    }
}

impl<B: Gpu> Bound<B> {
    /// Drops the references the bound vertex streams hold.
    pub fn release(&mut self) {
        for stream in self.vertex_buffers.drain(..) {
            // SAFETY: each stream holds one reference.
            unsafe { resource::release(stream.resource) };
        }
    }
}

/// A program stage as st/mesa handed it over: its NIR, kept to build each variant from.
pub struct Shader {
    pub stage: Stage,
    /// Its content's key, which what is made of it is kept by.
    pub id: ShaderId,
    pub nir: Nir,
    /// What a vertex or geometry program streams while stream output is bound: the stage helper's
    /// declaration, none, or why it cannot.
    pub streamed: Result<Option<StreamOutput>, &'static str>,
    /// A fragment program that reads where its samples are, converted for the targets' count.
    pub reads_sample_positions: bool,
    /// A fragment program that writes `gl_SampleMask`, converted for the targets' count.
    pub writes_sample_mask: bool,
    /// A fragment program that reads its first target's texels, from a copy of it each draw.
    pub fetches_target: bool,
    /// The image units it declares images at without a format, a bit each, converted for the
    /// bound images' formats.
    pub formatless_images: u64,
    /// The texture units it samples integers at, a bit each.
    pub integer_sampled: u64,
    /// A geometry program writing `gl_Layer`: the location after its outputs, where its
    /// pass-through passes the layer it names to the fragment program.
    pub named_layer: Option<u32>,
    /// A vertex or evaluation program using storage: the geometry programs drawn after it with
    /// no geometry program bound, so that it runs as compute work, which writes storage
    /// (`VertexStorageAsCompute`), by the primitive they take (`Assembled`; an evaluation
    /// program's one at the primitive it makes); an entry is `None` where none could be made.
    pub pass_through: Option<[Option<Box<Shader>>; Assembled::ALL.len()]>,
}

/// A state object of this driver, given to Mesa as a pointer it hands back.
fn object<T>(value: T) -> *mut c_void {
    Box::into_raw(Box::new(value)).cast()
}

/// `None` for a null object, which Mesa keeps where its making was refused.
///
/// # Safety
/// `object` is null or came from [`object`] with a `T`, and is dropped once.
unsafe fn drop_object<T>(object: *mut c_void) -> Option<T> {
    // SAFETY: the caller's.
    (!object.is_null()).then(|| *unsafe { Box::from_raw(object.cast::<T>()) })
}

/// Takes the NIR st/mesa hands over (or Mesa's own converter makes of a TGSI program, which
/// Gallium's helpers hand over) in the shape every variant starts from.
fn create_shader<B: Gpu>(
    pipe: *mut pipe_context,
    ir: pipe_shader_ir,
    program: *const c_void,
    stage: Stage,
) -> *mut c_void {
    // SAFETY: Mesa passes this driver's context, on this driver's screen, and a program; its NIR
    // is the driver's from now on.
    let (screen, driver) = unsafe { ((*pipe).screen, Screen::<B>::of((*pipe).screen)) };
    let raw: *mut c_void = match ir {
        PIPE_SHADER_IR_NIR => program.cast_mut(),
        // SAFETY: a TGSI program's tokens; the converter reads the screen's NIR options.
        PIPE_SHADER_IR_TGSI => unsafe { tgsi_to_nir(program.cast(), screen, false) }.cast(),
        _ => refuse(format_args!("a program neither in NIR nor in TGSI")),
    };
    if raw.is_null() {
        refuse(format_args!("a TGSI program Mesa's converter did not take"));
    }
    // SAFETY: as above.
    let mut nir = unsafe { Nir::from_raw(raw) };
    nir.drop_name();
    nir.uniform_blocks_as_words();
    nir.split_resource_arrays();
    // Every 1D texture is made 2D (`view.rs`).
    nir.textures_1d_as_2d();
    let mut streamed = Ok(None);
    match stage {
        Stage::Vertex => {
            nir.number_vertex_elements();
        }
        Stage::Fragment => nir.fragcolor_to_buffers(PIPE_MAX_COLOR_BUFS),
        Stage::Compute | Stage::Geometry | Stage::TessControl | Stage::TessEvaluation => {}
    }
    // The last stage before the rasterizer streams its outputs.
    if matches!(
        stage,
        Stage::Vertex | Stage::Geometry | Stage::TessEvaluation
    ) {
        nir.stream_point_size_at_a_location();
        streamed = match nir.stream_outputs::<STREAM_BUFFERS>() {
            Some((outputs, _)) if outputs.is_empty() => Ok(None),
            Some((outputs, strides)) => Ok(Some(stream::declaration(&outputs, strides))),
            None => Err("a streamed value that is neither at a location nor the position"),
        };
    }
    let reads_sample_positions = nir.reads_sample_positions();
    let writes_sample_mask = nir.writes_sample_mask();
    let fetches_target = nir.fetches_target();
    let formatless_images = nir.formatless_images();
    let integer_sampled = nir.integer_sampled_units();
    let named_layer = nir.named_layer_location();
    // WebGPU writes no storage from a vertex program, which an evaluation program is drawn as
    // without a geometry program after it; a vertex or evaluation program using storage runs
    // before a pass-through geometry program instead, passing it its point size at a location
    // (the stage helper stages none).
    let pass_through = if matches!(stage, Stage::Vertex | Stage::TessEvaluation) {
        nir.storage_pass_through()
    } else {
        None
    };
    let id = nir
        .key()
        .unwrap_or_else(|e| refuse(format_args!("a program whose content has no key: {e}")));
    driver.shader_made(id);
    let pass_through = pass_through.map(|made| {
        made.map(|geometry| {
            let geometry = geometry?;
            let id = geometry.key().unwrap_or_else(|e| {
                refuse(format_args!(
                    "a pass-through program whose content has no key: {e}"
                ))
            });
            driver.shader_made(id);
            Some(Box::new(Shader {
                stage: Stage::Geometry,
                id,
                named_layer: geometry.named_layer_location(),
                nir: geometry,
                streamed: streamed.clone(),
                reads_sample_positions: false,
                writes_sample_mask: false,
                fetches_target: false,
                formatless_images: 0,
                integer_sampled: 0,
                pass_through: None,
            }))
        })
    });
    object(Shader {
        stage,
        id,
        nir,
        streamed,
        reads_sample_positions,
        writes_sample_mask,
        fetches_target,
        formatless_images,
        integer_sampled,
        named_layer,
        pass_through,
    })
}

/// A graphics stage's program: its NIR, or its TGSI tokens.
fn create_stage<B: Gpu>(
    pipe: *mut pipe_context,
    state: *const pipe_shader_state,
    stage: Stage,
) -> *mut c_void {
    // SAFETY: Mesa passes a shader state; its union holds what its type names.
    let state = unsafe { &*state };
    let program: *const c_void = if state.type_ == PIPE_SHADER_IR_NIR {
        // SAFETY: as above.
        unsafe { state.ir.nir }.cast()
    } else {
        state.tokens.cast()
    };
    create_shader::<B>(pipe, state.type_, program, stage)
}

unsafe extern "C" fn create_vs_state<B: Gpu>(
    pipe: *mut pipe_context,
    state: *const pipe_shader_state,
) -> *mut c_void {
    crate::state::guest_call(pipe, std::ptr::null_mut(), move || {
        create_stage::<B>(pipe, state, Stage::Vertex)
    })
}

unsafe extern "C" fn create_fs_state<B: Gpu>(
    pipe: *mut pipe_context,
    state: *const pipe_shader_state,
) -> *mut c_void {
    crate::state::guest_call(pipe, std::ptr::null_mut(), move || {
        create_stage::<B>(pipe, state, Stage::Fragment)
    })
}

unsafe extern "C" fn create_gs_state<B: Gpu>(
    pipe: *mut pipe_context,
    state: *const pipe_shader_state,
) -> *mut c_void {
    crate::state::guest_call(pipe, std::ptr::null_mut(), move || {
        create_stage::<B>(pipe, state, Stage::Geometry)
    })
}

unsafe extern "C" fn bind_gs_state<B: Gpu>(pipe: *mut pipe_context, state: *mut c_void) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        unsafe { Context::<B>::of(pipe) }
            .stages
            .get_mut(Stage::Geometry)
            .shader = state.cast();
    })
}

unsafe extern "C" fn create_tcs_state<B: Gpu>(
    pipe: *mut pipe_context,
    state: *const pipe_shader_state,
) -> *mut c_void {
    crate::state::guest_call(pipe, std::ptr::null_mut(), move || {
        create_stage::<B>(pipe, state, Stage::TessControl)
    })
}

unsafe extern "C" fn bind_tcs_state<B: Gpu>(pipe: *mut pipe_context, state: *mut c_void) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        unsafe { Context::<B>::of(pipe) }
            .stages
            .get_mut(Stage::TessControl)
            .shader = state.cast();
    })
}

unsafe extern "C" fn create_tes_state<B: Gpu>(
    pipe: *mut pipe_context,
    state: *const pipe_shader_state,
) -> *mut c_void {
    crate::state::guest_call(pipe, std::ptr::null_mut(), move || {
        create_stage::<B>(pipe, state, Stage::TessEvaluation)
    })
}

unsafe extern "C" fn bind_tes_state<B: Gpu>(pipe: *mut pipe_context, state: *mut c_void) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        unsafe { Context::<B>::of(pipe) }
            .stages
            .get_mut(Stage::TessEvaluation)
            .shader = state.cast();
    })
}

unsafe extern "C" fn create_compute_state<B: Gpu>(
    pipe: *mut pipe_context,
    state: *const pipe_compute_state,
) -> *mut c_void {
    crate::state::guest_call(pipe, std::ptr::null_mut(), move || {
        // SAFETY: Mesa passes a compute state.
        let state = unsafe { &*state };
        create_shader::<B>(pipe, state.ir_type, state.prog, Stage::Compute)
    })
}

unsafe extern "C" fn bind_compute_state<B: Gpu>(pipe: *mut pipe_context, state: *mut c_void) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        unsafe { Context::<B>::of(pipe) }
            .stages
            .get_mut(Stage::Compute)
            .shader = state.cast();
    })
}

unsafe extern "C" fn bind_vs_state<B: Gpu>(pipe: *mut pipe_context, state: *mut c_void) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        unsafe { Context::<B>::of(pipe) }
            .stages
            .get_mut(Stage::Vertex)
            .shader = state.cast();
    })
}

unsafe extern "C" fn bind_fs_state<B: Gpu>(pipe: *mut pipe_context, state: *mut c_void) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        unsafe { Context::<B>::of(pipe) }
            .stages
            .get_mut(Stage::Fragment)
            .shader = state.cast();
    })
}

unsafe extern "C" fn delete_shader_state<B: Gpu>(pipe: *mut pipe_context, state: *mut c_void) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and a shader it made, deleted once.
        let (context, shader) = unsafe { (Context::<B>::of(pipe), drop_object::<Shader>(state)) };
        let Some(shader) = shader else { return };
        let screen = context.screen();
        let pass_through = shader.pass_through.iter().flatten().flatten().map(|p| p.id);
        for id in std::iter::once(shader.id).chain(pass_through) {
            if !screen.shader_deleted(&id) {
                context.programs.deleted(screen, id);
            }
        }
    })
}

unsafe extern "C" fn set_constant_buffer<B: Gpu>(
    pipe: *mut pipe_context,
    shader: mesa_shader_stage,
    index: c_uint,
    buffer: *const pipe_constant_buffer,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and a buffer or null.
        let context = unsafe { Context::<B>::of(pipe) };
        let Some(stage) = context::stage(shader) else {
            return;
        };
        let Some(slot) = context
            .stages
            .get_mut(stage)
            .constants
            .get_mut(index as usize)
        else {
            // Unbinding a slot past those the screen reports leaves nothing to drop.
            if buffer.is_null() {
                return;
            }
            refuse(format_args!(
                "a uniform buffer past those the screen reports"
            ));
        };
        let mut held = std::mem::replace(&mut slot.resource, std::ptr::null_mut());
        slot.bytes.clear();
        if !buffer.is_null() {
            // SAFETY: as above.
            let buffer = unsafe { &*buffer };
            slot.offset = buffer.buffer_offset;
            slot.size = buffer.buffer_size;
            if buffer.user_buffer.is_null() {
                // SAFETY: the buffer is a live resource; the slot takes its own reference.
                unsafe { resource::reference(&mut slot.resource, buffer.buffer) };
            } else {
                // SAFETY: the frontend's bytes live for the call; the driver keeps a copy.
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        buffer
                            .user_buffer
                            .cast::<u8>()
                            .add(buffer.buffer_offset as usize),
                        buffer.buffer_size as usize,
                    )
                };
                slot.bytes.extend_from_slice(bytes);
                slot.offset = 0;
            }
        }
        // SAFETY: the reference the slot held goes.
        unsafe { resource::reference(&mut held, std::ptr::null_mut()) };
    })
}

unsafe extern "C" fn set_sampler_views<B: Gpu>(
    pipe: *mut pipe_context,
    shader: mesa_shader_stage,
    start: c_uint,
    count: c_uint,
    trailing: c_uint,
    views: *mut *mut pipe_sampler_view,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and `count` views, or null.
        let context = unsafe { Context::<B>::of(pipe) };
        let Some(stage) = context::stage(shader) else {
            return;
        };
        let slots = &mut context.stages.get_mut(stage).views;
        for i in 0..(count + trailing) as usize {
            let view = if views.is_null() || i >= count as usize {
                std::ptr::null_mut()
            } else {
                // SAFETY: as above.
                unsafe { *views.add(i) }
            };
            // SAFETY: the views are live; the slot takes its own reference.
            unsafe { crate::view::reference(&mut slots[start as usize + i], view) };
        }
    })
}

unsafe extern "C" fn set_shader_buffers<B: Gpu>(
    pipe: *mut pipe_context,
    shader: mesa_shader_stage,
    start: c_uint,
    count: c_uint,
    buffers: *const pipe_shader_buffer,
    writable: c_uint,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and `count` buffers, or null.
        let context = unsafe { Context::<B>::of(pipe) };
        let Some(stage) = context::stage(shader) else {
            return;
        };
        let slots = &mut context.stages.get_mut(stage).buffers;
        for i in 0..count as usize {
            let given = if buffers.is_null() {
                None
            } else {
                // SAFETY: as above.
                Some(unsafe { &*buffers.add(i) })
            };
            let Some(slot) = slots.get_mut(start as usize + i) else {
                refuse(format_args!(
                    "a storage buffer past those the screen reports"
                ));
            };
            let resource = given.map_or(std::ptr::null_mut(), |b| b.buffer);
            // SAFETY: the buffer is null or live; the slot's reference moves to it.
            unsafe { resource::reference(&mut slot.resource, resource) };
            slot.offset = given.map_or(0, |b| b.buffer_offset);
            slot.size = given.map_or(0, |b| b.buffer_size);
            slot.writable = writable & (1 << i) != 0;
        }
    })
}

unsafe extern "C" fn set_shader_images<B: Gpu>(
    pipe: *mut pipe_context,
    shader: mesa_shader_stage,
    start: c_uint,
    count: c_uint,
    trailing: c_uint,
    images: *const pipe_image_view,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and `count` images, or null.
        let context = unsafe { Context::<B>::of(pipe) };
        let Some(stage) = context::stage(shader) else {
            return;
        };
        let slots = &mut context.stages.get_mut(stage).images;
        for i in 0..(count + trailing) as usize {
            let given = if images.is_null() || i >= count as usize {
                pipe_image_view::default()
            } else {
                // SAFETY: as above.
                unsafe { *images.add(i) }
            };
            let Some(slot) = slots.get_mut(start as usize + i) else {
                refuse(format_args!("an image past those the screen reports"));
            };
            let mut held = std::mem::replace(&mut slot.resource, std::ptr::null_mut());
            *slot = pipe_image_view {
                resource: std::ptr::null_mut(),
                ..given
            };
            // SAFETY: the image's resource is null or live; the slot takes its own reference and
            // gives back the one it held.
            unsafe {
                resource::reference(&mut slot.resource, given.resource);
                resource::reference(&mut held, std::ptr::null_mut());
            }
        }
    })
}

unsafe extern "C" fn set_vertex_buffers<B: Gpu>(
    pipe: *mut pipe_context,
    count: c_uint,
    buffers: *const pipe_vertex_buffer,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and `count` buffers.
        let (context, buffers) = unsafe {
            (
                Context::<B>::of(pipe),
                std::slice::from_raw_parts(buffers, count as usize),
            )
        };
        let mut old = std::mem::take(&mut context.bound.vertex_buffers).into_iter();
        for buffer in buffers {
            if buffer.is_user_buffer {
                refuse(format_args!(
                    "a vertex stream in client memory (the screen reports none)"
                ));
            }
            // SAFETY: not a user buffer, so a resource.
            let resource = unsafe { buffer.buffer.resource };
            // Each slot holds its own reference, kept where the slot is bound to the same resource
            // again, as Gallium's helpers do.
            let mut held = old.next().map_or(std::ptr::null_mut(), |p| p.resource);
            // SAFETY: both are null or live resources; the slot's reference moves to the new one.
            unsafe { resource::reference(&mut held, resource) };
            context.bound.vertex_buffers.push(VertexBuffer {
                resource,
                offset: buffer.buffer_offset,
            });
        }
        for previous in old {
            // SAFETY: each slot held one reference.
            unsafe { resource::release(previous.resource) };
        }
    })
}

/// Fills the context's state entry points.
pub fn fill<B: Gpu>(c: &mut pipe_context) {
    c.create_vs_state = Some(create_vs_state::<B>);
    c.bind_vs_state = Some(bind_vs_state::<B>);
    c.delete_vs_state = Some(delete_shader_state::<B>);
    c.create_fs_state = Some(create_fs_state::<B>);
    c.bind_fs_state = Some(bind_fs_state::<B>);
    c.delete_fs_state = Some(delete_shader_state::<B>);
    c.create_gs_state = Some(create_gs_state::<B>);
    c.bind_gs_state = Some(bind_gs_state::<B>);
    c.delete_gs_state = Some(delete_shader_state::<B>);
    c.create_tcs_state = Some(create_tcs_state::<B>);
    c.bind_tcs_state = Some(bind_tcs_state::<B>);
    c.delete_tcs_state = Some(delete_shader_state::<B>);
    c.create_tes_state = Some(create_tes_state::<B>);
    c.bind_tes_state = Some(bind_tes_state::<B>);
    c.delete_tes_state = Some(delete_shader_state::<B>);
    // The host's samples sit at the standard positions (WebGPU's, Vulkan's and Direct3D's).
    c.get_sample_position = Some(u_default_get_sample_position);
    c.set_constant_buffer = Some(set_constant_buffer::<B>);
    c.set_sampler_views = Some(set_sampler_views::<B>);
    c.set_vertex_buffers = Some(set_vertex_buffers::<B>);
    c.set_shader_buffers = Some(set_shader_buffers::<B>);
    c.set_shader_images = Some(set_shader_images::<B>);
    c.create_compute_state = Some(create_compute_state::<B>);
    c.bind_compute_state = Some(bind_compute_state::<B>);
    c.delete_compute_state = Some(delete_shader_state::<B>);
}
