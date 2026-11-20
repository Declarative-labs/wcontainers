//! The driver's `pipe_context`: the state st/mesa sets, kept as the interface's blocks, and the
//! calls that clear, flush and fence.

use std::ffi::{c_uint, c_void};
use std::sync::atomic::{AtomicI32, Ordering};

use wcontainers_gpu::{
    Backend, ClearValue, ColorTarget, EmptyPass, MAX_COLOR_TARGETS, Object, PassTargets,
    PassTracker, ScissorRect, Target, TextureDimension, TextureViewDesc, ViewDimension,
};

use crate::draw::Programs;
use crate::gallium::*;
use crate::resource::{self, Resource};
use crate::screen::{DRIVER_NAME, Screen};
use crate::state::{self, Stage, Stages};
use crate::{blit, draw, present, query, stream, view};

/// Ends the process naming a Gallium entry point the driver does not have yet, so that a case
/// reaching it is a crash with the name in its log, never a call that does nothing.
macro_rules! unbuilt {
    ($context:expr, $($entry:ident),+ $(,)?) => {$(
        $context.$entry = {
            extern "C" fn stub() -> ! {
                eprintln!(
                    "{}: not built: pipe_context::{}",
                    DRIVER_NAME.to_string_lossy(),
                    stringify!($entry)
                );
                std::process::abort()
            }
            // SAFETY: the stub reads no argument and never returns, so the caller's arguments,
            // passed as the C convention passes them, are never read.
            Some(unsafe { std::mem::transmute::<extern "C" fn() -> !, _>(stub) })
        };
    )+};
}

/// What the framebuffer holds: Gallium's state, with references on its textures, and the views
/// the pass tracker draws into.
pub struct Framebuffer<B: Backend> {
    pub state: pipe_framebuffer_state,
    pub views: Vec<B::TextureView>,
}

/// One context: Gallium's struct first, so that a `pipe_context` pointer is one to it.
#[repr(C)]
pub struct Context<B: Backend> {
    base: pipe_context,
    screen: *const Screen<B>,
    pub tracker: PassTracker<B>,
    pub framebuffer: Framebuffer<B>,
    pub stages: Stages<B>,
    pub programs: Programs<B>,
    pub bound: state::Bound<B>,
    pub present: Option<present::Copy<B>>,
    /// Gallium's blitter, made at the first blit.
    pub blitter: *mut blitter_context,
    /// The fragment stage's uniform bytes the blitter saved, alive until it puts them back.
    pub blit_bytes: Vec<u8>,
    /// The occlusion query sets, and the query running in them.
    pub queries: query::Sets<B>,
    pub running: Option<*mut pipe_query>,
    /// The count of primitives streamed that is running.
    pub counting: Option<*mut pipe_query>,
    pub streams: stream::Bound<B>,
}

impl<B: Backend> Context<B> {
    /// # Safety
    /// `context` is one this driver made, alive for `'a`, and used by one thread at a time.
    pub unsafe fn of<'a>(context: *mut pipe_context) -> &'a mut Context<B> {
        // SAFETY: the caller's; `base` is the first field.
        unsafe { &mut *context.cast::<Context<B>>() }
    }

    /// The screen, which outlives every context made on it.
    pub fn screen<'s>(&self) -> &'s Screen<B> {
        // SAFETY: Mesa destroys a screen only after its contexts.
        unsafe { &*self.screen }
    }
}

/// `pipe_screen::context_create`.
pub unsafe extern "C" fn context_create<B: crate::device::HostWindows>(
    screen: *mut pipe_screen,
    private: *mut c_void,
    _: c_uint,
) -> *mut pipe_context {
    // SAFETY: Mesa passes this driver's screen.
    let ours = unsafe { Screen::<B>::of(screen) };
    let mut context = Box::new(Context {
        base: pipe_context::default(),
        screen: ours,
        tracker: PassTracker::default(),
        framebuffer: Framebuffer {
            state: pipe_framebuffer_state::default(),
            views: Vec::new(),
        },
        stages: Stages::default(),
        programs: Programs::default(),
        bound: state::Bound::default(),
        present: None,
        blitter: std::ptr::null_mut(),
        blit_bytes: Vec::new(),
        queries: query::Sets::default(),
        running: None,
        counting: None,
        streams: stream::Bound::default(),
    });
    let base = &mut context.base;
    base.screen = screen;
    base.priv_ = private;
    fill::<B>(base);
    let context = Box::into_raw(context);
    // st/mesa's upload buffers, mapped through this context.
    // SAFETY: the context is complete; the uploaders keep a pointer to it.
    unsafe {
        let base = &raw mut (*context).base;
        (*base).stream_uploader = u_upload_create_default(base);
        (*base).const_uploader = (*base).stream_uploader;
        base
    }
}

/// Every entry point of the context.
#[allow(
    clippy::missing_transmute_annotations,
    reason = "each stub takes the type of the field it fills, which the macro does not name"
)]
fn fill<B: crate::device::HostWindows>(c: &mut pipe_context) {
    c.destroy = Some(destroy::<B>);
    c.draw_vbo = Some(draw::draw_vbo::<B>);
    c.blit = Some(blit::blit::<B>);
    c.flush = Some(flush::<B>);
    c.clear = Some(clear::<B>);
    c.clear_render_target = Some(clear_render_target::<B>);
    c.clear_depth_stencil = Some(clear_depth_stencil::<B>);
    c.set_framebuffer_state = Some(set_framebuffer_state::<B>);
    c.texture_barrier = Some(texture_barrier::<B>);
    c.memory_barrier = Some(texture_barrier::<B>);
    c.flush_resource = Some(flush_resource);
    c.create_query = Some(query::create_query::<B>);
    c.destroy_query = Some(query::destroy_query::<B>);
    c.begin_query = Some(query::begin_query::<B>);
    c.end_query = Some(query::end_query::<B>);
    c.get_query_result = Some(query::get_query_result::<B>);
    c.set_active_query_state = Some(query::set_active_query_state::<B>);
    c.set_polygon_stipple = Some(set_polygon_stipple);
    c.create_stream_output_target = Some(stream::create_stream_output_target::<B>);
    c.stream_output_target_destroy = Some(stream::stream_output_target_destroy::<B>);
    c.set_stream_output_targets = Some(stream::set_stream_output_targets::<B>);
    c.resource_copy_region = Some(view::resource_copy_region::<B>);
    c.generate_mipmap = Some(view::generate_mipmap::<B>);
    c.create_sampler_view = Some(view::create_sampler_view::<B>);
    c.sampler_view_destroy = Some(view::sampler_view_destroy::<B>);
    c.sampler_view_release = Some(view::sampler_view_release);
    c.resource_release = Some(resource::resource_release);
    c.buffer_map = Some(resource::buffer_map::<B>);
    c.buffer_unmap = Some(resource::buffer_unmap::<B>);
    c.texture_map = Some(resource::texture_map::<B>);
    c.texture_unmap = Some(resource::texture_unmap::<B>);
    c.transfer_flush_region = Some(resource::transfer_flush_region::<B>);
    c.buffer_subdata = Some(resource::buffer_subdata::<B>);
    c.texture_subdata = Some(resource::texture_subdata::<B>);
    state::fill::<B>(c);
    unbuilt!(
        c,
        get_query_result_resource,
        render_condition,
        create_gs_state,
        bind_gs_state,
        delete_gs_state,
        create_tcs_state,
        bind_tcs_state,
        delete_tcs_state,
        create_tes_state,
        bind_tes_state,
        delete_tes_state,
        create_compute_state,
        bind_compute_state,
        delete_compute_state,
        launch_grid,
    );
}

unsafe extern "C" fn destroy<B: Backend>(pipe: *mut pipe_context) {
    // SAFETY: Mesa destroys a context it made once, after its last use.
    let mut context = unsafe { Box::from_raw(pipe.cast::<Context<B>>()) };
    if !context.blitter.is_null() {
        // SAFETY: made on this context, destroyed once.
        unsafe { util_blitter_destroy(context.blitter) };
    }
    // SAFETY: the uploader was made with this context.
    unsafe { u_upload_destroy(context.base.stream_uploader) };
    context.bound.release();
    let views = std::mem::take(&mut context.framebuffer.views);
    let screen = context.screen();
    for view in views {
        screen.retire(Object::TextureView(view));
    }
    for surface in surfaces(&mut context.framebuffer.state) {
        // SAFETY: the framebuffer holds a reference on each texture.
        unsafe { resource::release(surface.texture) };
    }
    context.programs.release(screen);
    context.queries.release(screen);
    context.streams.release(screen);
    if let Some(copy) = context.present.take() {
        copy.release(screen);
    }
}

/// The framebuffer's surfaces: its colour buffers, then its depth and stencil buffer.
fn surfaces(state: &mut pipe_framebuffer_state) -> impl Iterator<Item = &mut pipe_surface> {
    let colors = usize::from(state.nr_cbufs);
    state.cbufs[..colors]
        .iter_mut()
        .chain(std::iter::once(&mut state.zsbuf))
}

/// The fence `flush` hands out: a reference count and the back end's fence.
pub struct Fence<B: Backend> {
    count: AtomicI32,
    pub fence: B::Fence,
}

unsafe extern "C" fn flush<B: Backend>(
    pipe: *mut pipe_context,
    fence: *mut *mut pipe_fence_handle,
    _: c_uint,
) {
    // SAFETY: Mesa passes this driver's context.
    let context = unsafe { Context::<B>::of(pipe) };
    let mut backend = context.screen().backend();
    if let Err(error) = context.tracker.close(&mut backend) {
        eprintln!(
            "{}: closing a pass: {error:?}",
            DRIVER_NAME.to_string_lossy()
        );
    }
    let done = backend.submit();
    if !fence.is_null() {
        let made = Box::into_raw(Box::new(Fence::<B> {
            count: AtomicI32::new(1),
            fence: done,
        }));
        // SAFETY: Mesa gives a place for one fence reference, empty or holding an old one.
        unsafe { fence_reference::<B>(context.screen.cast_mut().cast(), fence, made.cast()) };
        // The place took its own reference; the one made with the fence goes.
        // SAFETY: made above, with one reference besides the place's.
        unsafe { (*made).count.fetch_sub(1, Ordering::AcqRel) };
    }
}

/// `pipe_screen::fence_reference`.
pub unsafe extern "C" fn fence_reference<B: Backend>(
    _: *mut pipe_screen,
    place: *mut *mut pipe_fence_handle,
    fence: *mut pipe_fence_handle,
) {
    // SAFETY: Mesa passes a place and a fence of this driver, or null.
    unsafe {
        let new = fence.cast::<Fence<B>>();
        if !new.is_null() {
            (*new).count.fetch_add(1, Ordering::Relaxed);
        }
        let old = std::mem::replace(&mut *place, fence).cast::<Fence<B>>();
        if !old.is_null() && (*old).count.fetch_sub(1, Ordering::AcqRel) == 1 {
            drop(Box::from_raw(old));
        }
    }
}

/// `pipe_screen::fence_finish`: done, or waited on with any time-out but none.
pub unsafe extern "C" fn fence_finish<B: Backend>(
    screen: *mut pipe_screen,
    _: *mut pipe_context,
    fence: *mut pipe_fence_handle,
    timeout: u64,
) -> bool {
    // SAFETY: Mesa passes this driver's screen and one of its fences.
    let (screen, fence) = unsafe { (Screen::<B>::of(screen), &*fence.cast::<Fence<B>>()) };
    let mut backend = screen.backend();
    if backend.fence_done(&fence.fence) {
        return true;
    }
    if timeout == 0 {
        return false;
    }
    backend.wait_fence(&fence.fence);
    true
}

/// The f32 bits of a depth value.
fn depth_bits(depth: f64) -> u32 {
    (depth as f32).to_bits()
}

/// The stencil bits of a clear value.
fn stencil_bits(stencil: c_uint) -> u8 {
    (stencil & c_uint::from(u8::MAX)) as u8
}

unsafe extern "C" fn clear<B: Backend>(
    pipe: *mut pipe_context,
    buffers: c_uint,
    _: u32,
    _: u8,
    scissor: *const pipe_scissor_state,
    color: *const pipe_color_union,
    depth: f64,
    stencil: c_uint,
) {
    // SAFETY: Mesa passes this driver's context and a colour.
    let (context, color) = unsafe { (Context::<B>::of(pipe), (*color).ui) };
    if !scissor.is_null() {
        eprintln!(
            "{}: refused: a scissored clear (the screen reports none)",
            DRIVER_NAME.to_string_lossy()
        );
        std::process::abort();
    }
    let mut backend = context.screen().backend();
    let tracker = &mut context.tracker;
    let colors = usize::from(context.framebuffer.state.nr_cbufs);
    let mut done = Ok(());
    for target in 0..colors {
        if buffers & (PIPE_CLEAR_COLOR0 << target) != 0 {
            done = done.and_then(|()| tracker.clear_color(&mut backend, target, color));
        }
    }
    let depth = (buffers & PIPE_CLEAR_DEPTH != 0).then(|| depth_bits(depth));
    let stencil = (buffers & PIPE_CLEAR_STENCIL != 0).then(|| stencil_bits(stencil));
    if depth.is_some() || stencil.is_some() {
        done = done.and_then(|()| tracker.clear_depth_stencil(&mut backend, depth, stencil));
    }
    if let Err(error) = done {
        eprintln!("{}: a clear: {error:?}", DRIVER_NAME.to_string_lossy());
    }
}

/// Clears a rectangle of one surface, outside any pass.
#[allow(
    clippy::too_many_arguments,
    reason = "one clear of Gallium's, unpacked"
)]
fn clear_surface<B: Backend>(
    context: &mut Context<B>,
    surface: &pipe_surface,
    value: ClearValue,
    x: c_uint,
    y: c_uint,
    width: c_uint,
    height: c_uint,
) {
    let mut guard = context.screen().backend();
    let backend = &mut *guard;
    let made = context
        .tracker
        .close(backend)
        .and_then(|()| surface_view::<B>(backend, surface).ok_or(NO_VIEW));
    let result = made.and_then(|(view, format)| {
        let rect = ScissorRect {
            x,
            y,
            width,
            height,
        };
        let cleared = backend.clear_rects(&view, format, value, &[rect]);
        backend.destroy(Object::TextureView(view));
        cleared
    });
    if let Err(error) = result {
        eprintln!("{}: a clear: {error:?}", DRIVER_NAME.to_string_lossy());
    }
}

/// Why a surface had no view.
const NO_VIEW: wcontainers_gpu::GpuError =
    wcontainers_gpu::GpuError::Invalid("a surface of no texture or of a format the device lacks");

#[allow(clippy::too_many_arguments, reason = "Gallium's signature")]
unsafe extern "C" fn clear_render_target<B: Backend>(
    pipe: *mut pipe_context,
    surface: *mut pipe_surface,
    color: *const pipe_color_union,
    x: c_uint,
    y: c_uint,
    width: c_uint,
    height: c_uint,
    _: bool,
) {
    // SAFETY: Mesa passes this driver's context, a surface and a colour.
    let (context, surface, color) = unsafe { (Context::<B>::of(pipe), &*surface, (*color).ui) };
    clear_surface(
        context,
        surface,
        ClearValue::Color(color),
        x,
        y,
        width,
        height,
    );
}

#[allow(clippy::too_many_arguments, reason = "Gallium's signature")]
unsafe extern "C" fn clear_depth_stencil<B: Backend>(
    pipe: *mut pipe_context,
    surface: *mut pipe_surface,
    flags: c_uint,
    depth: f64,
    stencil: c_uint,
    x: c_uint,
    y: c_uint,
    width: c_uint,
    height: c_uint,
    _: bool,
) {
    // SAFETY: Mesa passes this driver's context and a surface.
    let (context, surface) = unsafe { (Context::<B>::of(pipe), &*surface) };
    let value = ClearValue::DepthStencil {
        depth: (flags & PIPE_CLEAR_DEPTH != 0).then(|| depth_bits(depth)),
        stencil: (flags & PIPE_CLEAR_STENCIL != 0).then(|| stencil_bits(stencil)),
    };
    clear_surface(context, surface, value, x, y, width, height);
}

/// A view to draw into of one surface, with the format it is seen in.
pub fn surface_view<B: Backend>(
    backend: &mut B,
    surface: &pipe_surface,
) -> Option<(B::TextureView, wcontainers_gpu::Format)> {
    if surface.texture.is_null() {
        return None;
    }
    // SAFETY: a surface holds a live texture.
    let resource = unsafe { Resource::<B>::of(surface.texture) };
    let (texture, desc) = resource.texture()?;
    let format = crate::format::format(surface.format())?;
    let three_d = desc.dimension == TextureDimension::D3;
    let view = backend
        .create_view(
            texture,
            &TextureViewDesc {
                format,
                dimension: if three_d {
                    ViewDimension::D3
                } else {
                    ViewDimension::D2
                },
                base_mip: surface.level,
                mip_count: 1,
                base_layer: if three_d { 0 } else { surface.first_layer() },
                layer_count: 1,
                stencil: false,
            },
        )
        .ok()?;
    Some((view, format))
}

/// The pass tracker's target of one surface, with the view made for it.
fn target<B: Backend>(backend: &mut B, surface: &pipe_surface) -> Option<Target<B>> {
    // SAFETY: a surface holds a live texture.
    let resource = unsafe { Resource::<B>::of(surface.texture) };
    let (texture, desc) = resource.texture()?;
    let (view, format) = surface_view(backend, surface)?;
    let three_d = desc.dimension == TextureDimension::D3;
    let size = |n: u32| (n >> surface.level).max(1);
    Some(Target {
        view,
        texture: texture.clone(),
        format,
        mip: surface.level,
        layer: if three_d { 0 } else { surface.first_layer() },
        depth_slice: if three_d { surface.first_layer() } else { 0 },
        width: size(desc.width),
        height: size(desc.height),
        samples: desc.samples,
    })
}

unsafe extern "C" fn set_framebuffer_state<B: Backend>(
    pipe: *mut pipe_context,
    state: *const pipe_framebuffer_state,
) {
    // SAFETY: Mesa passes this driver's context and a framebuffer.
    let (context, mut state) = unsafe { (Context::<B>::of(pipe), *state) };
    for surface in surfaces(&mut state) {
        let texture = std::mem::replace(&mut surface.texture, std::ptr::null_mut());
        // SAFETY: the surfaces' textures are live; the copy takes its own references.
        unsafe { resource::reference(&mut surface.texture, texture) };
    }
    let old = std::mem::replace(&mut context.framebuffer.state, state);
    let mut targets = PassTargets {
        colors: [const { None }; MAX_COLOR_TARGETS],
        depth: None,
        empty: None,
    };
    let screen = context.screen();
    let mut guard = screen.backend();
    let backend = &mut *guard;
    let mut views = Vec::new();
    let colors = usize::from(state.nr_cbufs);
    for (slot, surface) in targets.colors.iter_mut().zip(&state.cbufs[..colors]) {
        if surface.texture.is_null() {
            continue;
        }
        let Some(made) = target(backend, surface) else {
            continue;
        };
        views.push(made.view.clone());
        *slot = Some(ColorTarget {
            target: made,
            resolve: None,
        });
    }
    if !state.zsbuf.texture.is_null()
        && let Some(made) = target(backend, &state.zsbuf)
    {
        views.push(made.view.clone());
        targets.depth = Some(made);
    }
    if views.is_empty() && state.width > 0 && state.height > 0 {
        targets.empty = Some(EmptyPass {
            width: state.width,
            height: state.height,
            samples: u32::from(state.samples).max(1),
        });
    }
    if let Err(error) = context.tracker.set_targets(backend, targets) {
        eprintln!("{}: framebuffer: {error:?}", DRIVER_NAME.to_string_lossy());
    }
    drop(guard);
    for view in std::mem::replace(&mut context.framebuffer.views, views) {
        screen.retire(Object::TextureView(view));
    }
    let mut old = old;
    for surface in surfaces(&mut old) {
        // SAFETY: the old copy held a reference on each texture.
        unsafe { resource::release(surface.texture) };
    }
}

/// Later draws see what earlier ones wrote: the open pass ends.
unsafe extern "C" fn texture_barrier<B: Backend>(pipe: *mut pipe_context, _: c_uint) {
    // SAFETY: Mesa passes this driver's context.
    let context = unsafe { Context::<B>::of(pipe) };
    let mut backend = context.screen().backend();
    if let Err(error) = context.tracker.close(&mut backend) {
        eprintln!(
            "{}: closing a pass: {error:?}",
            DRIVER_NAME.to_string_lossy()
        );
    }
}

/// A resource about to be shown or shared needs nothing more: every write is already ordered.
unsafe extern "C" fn flush_resource(_: *mut pipe_context, _: *mut pipe_resource) {}

/// The stipple pattern applies only to a rasterizer state that stipples, which is refused by name.
unsafe extern "C" fn set_polygon_stipple(_: *mut pipe_context, _: *const pipe_poly_stipple) {}

/// The stage slot a Gallium stage's state is kept in, for the stages the driver takes.
pub fn stage(stage: mesa_shader_stage) -> Option<Stage> {
    match stage {
        MESA_SHADER_VERTEX => Some(Stage::Vertex),
        MESA_SHADER_FRAGMENT => Some(Stage::Fragment),
        _ => None,
    }
}
