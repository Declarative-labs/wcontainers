//! The driver's `pipe_context`: the state st/mesa sets, kept as the interface's blocks, and the
//! calls that clear, flush and fence.

use std::ffi::{c_uint, c_void};
use std::sync::atomic::{AtomicI32, Ordering};

use wcontainers_gpu::{
    ClearValue, ColorTarget, DeviceStatus, EmptyPass, FORMAT_FACTS, Gpu, MAX_COLOR_TARGETS, Object,
    PassTargets, PassTracker, ScissorRect, Target, TextureDesc, TextureDimension, TextureViewDesc,
    ViewDimension,
};

use crate::draw::Programs;
use crate::gallium::*;
use crate::resource::{self, Resource};
use crate::screen::Screen;
use crate::state::{self, Stage, Stages};
use crate::{blit, draw, present, query, stream, view};

/// Refuses the call once the back end has lost its device: the loss belongs to the work that
/// made it, and every later call of the context would fail.
pub fn end_if_lost<B: Gpu>(backend: &mut B) {
    if let DeviceStatus::Lost(reason) = backend.status() {
        state::refuse(format_args!("the device was lost: {reason:?}"));
    }
}

/// What the framebuffer holds: Gallium's state, with references on its textures, and the views
/// the pass tracker draws into.
pub struct Framebuffer<B: Gpu> {
    pub state: pipe_framebuffer_state,
    pub views: Vec<B::TextureView>,
    /// The samples of each pixel its targets have.
    pub samples: u32,
    /// The twins its targets in another format than their textures' draw into, copied into
    /// before each draw and back after it and after each clear.
    pub twins: Vec<view::Twin<B>>,
}

impl<B: Gpu> Framebuffer<B> {
    /// Whether one of its targets is of `resource`.
    pub fn draws_into(&self, resource: &Resource<B>) -> bool {
        let colors = usize::from(self.state.nr_cbufs);
        self.state.cbufs[..colors]
            .iter()
            .chain(std::iter::once(&self.state.zsbuf))
            .any(|s| std::ptr::eq(s.texture.cast_const(), &resource.base))
    }

    /// Copies each target's twin into its texture, after the work drawing into them (outside a
    /// pass: the open one is closed).
    pub fn copy_back_twins(
        &self,
        tracker: &mut PassTracker<B>,
        backend: &mut B,
    ) -> Result<(), String> {
        if self.twins.is_empty() {
            return Ok(());
        }
        tracker
            .close(backend)
            .map_err(|e| format!("closing the pass for a target's twin: {e:?}"))?;
        for twin in &self.twins {
            twin.copy_back(backend);
        }
        Ok(())
    }

    /// Copies each target's texture into its twin, before work drawing into them (outside a
    /// pass: the open one is closed).
    pub fn copy_into_twins(
        &self,
        tracker: &mut PassTracker<B>,
        backend: &mut B,
    ) -> Result<(), String> {
        if self.twins.is_empty() {
            return Ok(());
        }
        tracker
            .close(backend)
            .map_err(|e| format!("closing the pass for a target's twin: {e:?}"))?;
        for twin in &self.twins {
            twin.copy_in(backend);
        }
        Ok(())
    }
}

/// One context: Gallium's struct first, so that a `pipe_context` pointer is one to it.
#[repr(C)]
pub struct Context<B: Gpu> {
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
    /// The exact sample counts of counted occlusion queries, one running at a time, and whether
    /// it counts the next draw (not while st/mesa draws for itself).
    pub counter: wcontainers_gpu::OcclusionCounter<B>,
    pub counts_samples: bool,
    /// Queries ended whose counts are not yet read back: their reads start at the next flush, so
    /// that a finished context has every answer.
    pub unread: Vec<*mut pipe_query>,
    /// The count of primitives streamed that is running.
    pub counting: Vec<*mut pipe_query>,
    /// The render condition: draws and clears are skipped while the query's answer equals the
    /// value.
    pub condition: Option<(*mut pipe_query, bool)>,
    pub streams: stream::Bound<B>,
}

impl<B: Gpu> Context<B> {
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
    crate::state::guest_call(std::ptr::null_mut(), std::ptr::null_mut(), move || {
        // SAFETY: Mesa passes this driver's screen.
        let ours = unsafe { Screen::<B>::of(screen) };
        let mut context = Box::new(Context {
            base: pipe_context::default(),
            screen: ours,
            tracker: PassTracker::default(),
            framebuffer: Framebuffer {
                state: pipe_framebuffer_state::default(),
                views: Vec::new(),
                samples: 1,
                twins: Vec::new(),
            },
            stages: Stages::default(),
            programs: Programs::default(),
            bound: state::Bound::default(),
            present: None,
            blitter: std::ptr::null_mut(),
            blit_bytes: Vec::new(),
            queries: query::Sets::default(),
            running: None,
            counter: wcontainers_gpu::OcclusionCounter::default(),
            counts_samples: false,
            unread: Vec::new(),
            counting: Vec::new(),
            condition: None,
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
    })
}

/// Every entry point of the context. `get_query_result_resource` is left empty: st/mesa calls it
/// only for `ARB_query_buffer_object`, which the screen does not report.
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
    c.launch_grid = Some(crate::compute::launch_grid::<B>);
    c.flush_resource = Some(flush_resource);
    c.create_query = Some(query::create_query::<B>);
    c.destroy_query = Some(query::destroy_query::<B>);
    c.begin_query = Some(query::begin_query::<B>);
    c.end_query = Some(query::end_query::<B>);
    c.get_query_result = Some(query::get_query_result::<B>);
    c.set_active_query_state = Some(query::set_active_query_state::<B>);
    c.get_timestamp = Some(query::get_timestamp::<B>);
    c.render_condition = Some(query::render_condition::<B>);
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
    // Mesa's transfer helper takes the maps of multisampled textures and gives the screen's own
    // maps the rest.
    c.texture_map = Some(u_transfer_helper_transfer_map);
    c.texture_unmap = Some(u_transfer_helper_transfer_unmap);
    c.transfer_flush_region = Some(u_transfer_helper_transfer_flush_region);
    c.buffer_subdata = Some(resource::buffer_subdata::<B>);
    c.texture_subdata = Some(resource::texture_subdata::<B>);
    c.get_device_reset_status = Some(state::get_device_reset_status);
    state::fill::<B>(c);
}

unsafe extern "C" fn destroy<B: Gpu>(pipe: *mut pipe_context) {
    crate::state::guest_call(pipe, (), move || {
        state::forget_reset(pipe);
        // SAFETY: Mesa destroys a context it made once, after its last use.
        let mut context = unsafe { Box::from_raw(pipe.cast::<Context<B>>()) };
        if !context.blitter.is_null() {
            // SAFETY: made on this context, destroyed once.
            unsafe { util_blitter_destroy(context.blitter) };
        }
        // SAFETY: the uploader was made with this context.
        unsafe { u_upload_destroy(context.base.stream_uploader) };
        context.bound.release();
        context.stages.release();
        let views = std::mem::take(&mut context.framebuffer.views);
        let screen = context.screen();
        for view in views {
            screen.retire(Object::TextureView(view));
        }
        for twin in std::mem::take(&mut context.framebuffer.twins) {
            twin.release(|object| screen.retire(object));
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
    })
}

/// The framebuffer's surfaces: its colour buffers, then its depth and stencil buffer.
fn surfaces(state: &mut pipe_framebuffer_state) -> impl Iterator<Item = &mut pipe_surface> {
    let colors = usize::from(state.nr_cbufs);
    state.cbufs[..colors]
        .iter_mut()
        .chain(std::iter::once(&mut state.zsbuf))
}

/// The fence `flush` hands out: a reference count and the back end's fence.
pub struct Fence<B: Gpu> {
    count: AtomicI32,
    pub fence: B::Fence,
}

unsafe extern "C" fn flush<B: Gpu>(
    pipe: *mut pipe_context,
    fence: *mut *mut pipe_fence_handle,
    _: c_uint,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        let context = unsafe { Context::<B>::of(pipe) };
        let mut backend = context.screen().backend();
        if let Err(error) = context.tracker.close(&mut backend) {
            state::refuse(format_args!("closing a pass: {error:?}"));
        }
        query::start_reads(context, &mut backend);
        let done = backend.submit();
        end_if_lost(&mut *backend);
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
    })
}

/// `pipe_screen::fence_reference`.
pub unsafe extern "C" fn fence_reference<B: Gpu>(
    _: *mut pipe_screen,
    place: *mut *mut pipe_fence_handle,
    fence: *mut pipe_fence_handle,
) {
    crate::state::guest_call(std::ptr::null_mut(), (), move || {
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
    })
}

/// `pipe_screen::fence_finish`: done, or waited on with any time-out but none.
pub unsafe extern "C" fn fence_finish<B: Gpu>(
    screen: *mut pipe_screen,
    _: *mut pipe_context,
    fence: *mut pipe_fence_handle,
    timeout: u64,
) -> bool {
    crate::state::guest_call(std::ptr::null_mut(), true, move || {
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
    })
}

/// The f32 bits of a depth value.
fn depth_bits(depth: f64) -> u32 {
    (depth as f32).to_bits()
}

/// The stencil bits of a clear value.
fn stencil_bits(stencil: c_uint) -> u8 {
    (stencil & c_uint::from(u8::MAX)) as u8
}

unsafe extern "C" fn clear<B: Gpu>(
    pipe: *mut pipe_context,
    buffers: c_uint,
    _: u32,
    _: u8,
    scissor: *const pipe_scissor_state,
    color: *const pipe_color_union,
    depth: f64,
    stencil: c_uint,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and a colour.
        let (context, color) = unsafe { (Context::<B>::of(pipe), (*color).ui) };
        if !query::condition_holds(context) {
            return;
        }
        if !scissor.is_null() {
            state::refuse(format_args!("a scissored clear (the screen reports none)"));
        }
        let mut backend = context.screen().backend();
        let tracker = &mut context.tracker;
        let state = &context.framebuffer.state;
        let colors = usize::from(state.nr_cbufs);
        let depth = (buffers & PIPE_CLEAR_DEPTH != 0).then(|| depth_bits(depth));
        let stencil = (buffers & PIPE_CLEAR_STENCIL != 0).then(|| stencil_bits(stencil));
        // A layered framebuffer is cleared on every layer, a layer at a time.
        let layers = layers(state);
        let set = tracker.targets().cloned();
        let mut made = Vec::new();
        let mut done = Ok(());
        for layer in 0..layers {
            if layer > 0 {
                let mut twins = Vec::new();
                let (targets, views, _) = layer_targets(&mut *backend, state, layer, &mut twins);
                if !twins.is_empty() {
                    crate::state::refuse(format_args!(
                        "a clear of a layered target drawn through a twin (not built)"
                    ));
                }
                made.extend(views);
                done = done.and_then(|()| tracker.set_targets(&mut backend, targets));
            }
            for target in 0..colors {
                if buffers & (PIPE_CLEAR_COLOR0 << target) != 0 {
                    done = done.and_then(|()| tracker.clear_color(&mut backend, target, color));
                }
            }
            if depth.is_some() || stencil.is_some() {
                done =
                    done.and_then(|()| tracker.clear_depth_stencil(&mut backend, depth, stencil));
            }
        }
        if layers > 1 {
            if let Some(set) = set {
                done = done.and_then(|()| tracker.set_targets(&mut backend, set));
            }
            for view in made {
                backend.destroy(Object::TextureView(view));
            }
        }
        if let Err(error) = done {
            state::refuse(format_args!("a clear: {error:?}"));
        }
        if let Err(error) = context
            .framebuffer
            .copy_back_twins(&mut context.tracker, &mut backend)
        {
            state::refuse(format_args!("a clear: {error}"));
        }
    })
}

/// Clears a rectangle of one surface, outside any pass.
#[allow(
    clippy::too_many_arguments,
    reason = "one clear of Gallium's, unpacked"
)]
fn clear_surface<B: Gpu>(
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
        .and_then(|()| surface_view::<B>(backend, surface, surface.first_layer()).ok_or(NO_VIEW));
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
        state::refuse(format_args!("a clear: {error:?}"));
    }
}

/// Why a surface had no view.
const NO_VIEW: wcontainers_gpu::GpuError =
    wcontainers_gpu::GpuError::Invalid("a surface of no texture or of a format the device lacks");

#[allow(clippy::too_many_arguments, reason = "Gallium's signature")]
unsafe extern "C" fn clear_render_target<B: Gpu>(
    pipe: *mut pipe_context,
    surface: *mut pipe_surface,
    color: *const pipe_color_union,
    x: c_uint,
    y: c_uint,
    width: c_uint,
    height: c_uint,
    conditional: bool,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context, a surface and a colour.
        let (context, surface, color) = unsafe { (Context::<B>::of(pipe), &*surface, (*color).ui) };
        if conditional && !query::condition_holds(context) {
            return;
        }
        clear_surface(
            context,
            surface,
            ClearValue::Color(color),
            x,
            y,
            width,
            height,
        );
    })
}

#[allow(clippy::too_many_arguments, reason = "Gallium's signature")]
unsafe extern "C" fn clear_depth_stencil<B: Gpu>(
    pipe: *mut pipe_context,
    surface: *mut pipe_surface,
    flags: c_uint,
    depth: f64,
    stencil: c_uint,
    x: c_uint,
    y: c_uint,
    width: c_uint,
    height: c_uint,
    conditional: bool,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and a surface.
        let (context, surface) = unsafe { (Context::<B>::of(pipe), &*surface) };
        if conditional && !query::condition_holds(context) {
            return;
        }
        let value = ClearValue::DepthStencil {
            depth: (flags & PIPE_CLEAR_DEPTH != 0).then(|| depth_bits(depth)),
            stencil: (flags & PIPE_CLEAR_STENCIL != 0).then(|| stencil_bits(stencil)),
        };
        clear_surface(context, surface, value, x, y, width, height);
    })
}

/// A view to draw into of layer `layer` of one surface's texture, with the format it is seen in.
pub fn surface_view<B: Gpu>(
    backend: &mut B,
    surface: &pipe_surface,
    layer: u32,
) -> Option<(B::TextureView, wcontainers_gpu::Format)> {
    if surface.texture.is_null() {
        return None;
    }
    // SAFETY: a surface holds a live texture.
    let (resource, layer) = unsafe { Resource::<B>::of(surface.texture) }.layer(layer);
    let (texture, desc) = resource.texture()?;
    let format = crate::format::format(surface.format())?;
    if through_twin(backend, format, desc) {
        crate::state::refuse(format_args!(
            "a target in {format:?} of a texture in {:?}, which the host views only in its own \
             format (not built)",
            desc.format
        ));
    }
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
                base_layer: if three_d { 0 } else { layer },
                layer_count: 1,
                stencil: false,
            },
        )
        .ok()?;
    Some((view, format))
}

/// Whether a colour target in `format` of a texture `desc` describes is drawn into through a twin:
/// the host views a texture only in its own format or its sRGB twin unless it reinterprets.
fn through_twin<B: Gpu>(backend: &B, format: wcontainers_gpu::Format, desc: &TextureDesc) -> bool {
    let reinterprets = desc.reinterpret
        && backend
            .caps()
            .features
            .contains(wcontainers_gpu::Features::FORMAT_REINTERPRET);
    let colour = !FORMAT_FACTS[format as usize].depth_stencil
        && !FORMAT_FACTS[desc.format as usize].depth_stencil;
    colour
        && FORMAT_FACTS[format as usize].linear != FORMAT_FACTS[desc.format as usize].linear
        && !reinterprets
}

/// The pass tracker's target of layer `run` of one surface (its first layer and on), with the view
/// made for it; a target in another format than its texture's draws into a twin, added to
/// `twins`.
fn target<B: Gpu>(
    backend: &mut B,
    surface: &pipe_surface,
    run: u32,
    twins: &mut Vec<view::Twin<B>>,
) -> Option<Target<B>> {
    let first = surface.first_layer() + run;
    // SAFETY: a surface holds a live texture.
    let (resource, layer) = unsafe { Resource::<B>::of(surface.texture) }.layer(first);
    let (texture, desc) = resource.texture()?;
    let size = |n: u32| (n >> surface.level).max(1);
    let format = crate::format::format(surface.format())?;
    if through_twin(backend, format, desc) {
        let twin = view::Twin::of_target(backend, texture, desc, format, surface.level, layer)
            .unwrap_or_else(|why| state::refuse(format_args!("{why} (not built)")));
        let view = backend
            .create_view(
                twin.texture(),
                &TextureViewDesc {
                    format,
                    dimension: ViewDimension::D2,
                    base_mip: 0,
                    mip_count: 1,
                    base_layer: 0,
                    layer_count: 1,
                    stencil: false,
                },
            )
            .ok()?;
        let target = Target {
            view,
            texture: twin.texture().clone(),
            format,
            mip: 0,
            layer: 0,
            depth_slice: 0,
            width: size(desc.width),
            height: size(desc.height),
            samples: 1,
        };
        twins.push(twin);
        return Some(target);
    }
    let (view, format) = surface_view(backend, surface, first)?;
    let three_d = desc.dimension == TextureDimension::D3;
    Some(Target {
        view,
        texture: texture.clone(),
        format,
        mip: surface.level,
        layer: if three_d { 0 } else { layer },
        depth_slice: if three_d { layer } else { 0 },
        width: size(desc.width),
        height: size(desc.height),
        samples: desc.samples,
    })
}

/// The pass tracker's targets of layer `run` of each surface of `state`, the views made for them
/// and their samples; targets in another format than their textures' draw into twins, added to
/// `twins`.
pub fn layer_targets<B: Gpu>(
    backend: &mut B,
    state: &pipe_framebuffer_state,
    run: u32,
    twins: &mut Vec<view::Twin<B>>,
) -> (PassTargets<B>, Vec<B::TextureView>, u32) {
    let mut targets = PassTargets {
        colors: [const { None }; MAX_COLOR_TARGETS],
        depth: None,
        empty: None,
    };
    let mut views = Vec::new();
    let mut samples = u32::from(state.samples).max(1);
    let colors = usize::from(state.nr_cbufs);
    for (slot, surface) in targets.colors.iter_mut().zip(&state.cbufs[..colors]) {
        if surface.texture.is_null() {
            continue;
        }
        let Some(made) = target(backend, surface, run, twins) else {
            continue;
        };
        views.push(made.view.clone());
        samples = made.samples;
        *slot = Some(ColorTarget {
            target: made,
            resolve: None,
        });
    }
    if !state.zsbuf.texture.is_null()
        && let Some(made) = target(backend, &state.zsbuf, run, twins)
    {
        views.push(made.view.clone());
        samples = made.samples;
        targets.depth = Some(made);
    }
    if views.is_empty() && state.width > 0 && state.height > 0 {
        targets.empty = Some(EmptyPass {
            width: state.width,
            height: state.height,
            samples,
        });
    }
    (targets, views, samples)
}

/// The layers of a layered framebuffer, which a draw naming layers and a clear go through one at
/// a time: the fewest its targets have (OpenGL leaves a layer past any target's undefined), or
/// its own count without targets; one where it is not layered.
pub fn layers(state: &pipe_framebuffer_state) -> u32 {
    let colors = usize::from(state.nr_cbufs);
    state.cbufs[..colors]
        .iter()
        .chain(std::iter::once(&state.zsbuf))
        .filter(|s| !s.texture.is_null())
        .map(|s| s.last_layer().saturating_sub(s.first_layer()) + 1)
        .min()
        .unwrap_or_else(|| u32::from(state.layers))
        .max(1)
}

unsafe extern "C" fn set_framebuffer_state<B: Gpu>(
    pipe: *mut pipe_context,
    state: *const pipe_framebuffer_state,
) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and a framebuffer.
        let (context, mut state) = unsafe { (Context::<B>::of(pipe), *state) };
        for surface in surfaces(&mut state) {
            let texture = std::mem::replace(&mut surface.texture, std::ptr::null_mut());
            // SAFETY: the surfaces' textures are live; the copy takes its own references.
            unsafe { resource::reference(&mut surface.texture, texture) };
        }
        let old = std::mem::replace(&mut context.framebuffer.state, state);
        let screen = context.screen();
        let mut guard = screen.backend();
        let backend = &mut *guard;
        // The old twins were copied back after their work.
        for twin in std::mem::take(&mut context.framebuffer.twins) {
            twin.release(|object| backend.destroy(object));
        }
        let mut twins = Vec::new();
        let (targets, views, samples) = layer_targets(backend, &state, 0, &mut twins);
        context.framebuffer.samples = samples;
        context.framebuffer.twins = twins;
        if let Err(error) = context
            .framebuffer
            .copy_into_twins(&mut context.tracker, backend)
        {
            state::refuse(format_args!("framebuffer: {error}"));
        }
        if let Err(error) = context.tracker.set_targets(backend, targets) {
            state::refuse(format_args!("framebuffer: {error:?}"));
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
    })
}

/// Later draws see what earlier ones wrote, to targets or storage: the open pass ends (the host
/// orders passes and dispatches after the ones before).
unsafe extern "C" fn texture_barrier<B: Gpu>(pipe: *mut pipe_context, _: c_uint) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context.
        let context = unsafe { Context::<B>::of(pipe) };
        let mut backend = context.screen().backend();
        if let Err(error) = context.tracker.close(&mut backend) {
            state::refuse(format_args!("closing a pass: {error:?}"));
        }
    })
}

/// A resource about to be shown or shared needs nothing more: every write is already ordered.
extern "C" fn flush_resource(_: *mut pipe_context, _: *mut pipe_resource) {}

/// The stipple pattern applies only to a rasterizer state that stipples, which is refused by name.
extern "C" fn set_polygon_stipple(_: *mut pipe_context, _: *const pipe_poly_stipple) {}

/// The stage slot a Gallium stage's state is kept in, for the stages the driver takes.
pub fn stage(stage: mesa_shader_stage) -> Option<Stage> {
    match stage {
        MESA_SHADER_VERTEX => Some(Stage::Vertex),
        MESA_SHADER_FRAGMENT => Some(Stage::Fragment),
        MESA_SHADER_COMPUTE => Some(Stage::Compute),
        MESA_SHADER_GEOMETRY => Some(Stage::Geometry),
        MESA_SHADER_TESS_CTRL => Some(Stage::TessControl),
        MESA_SHADER_TESS_EVAL => Some(Stage::TessEvaluation),
        _ => None,
    }
}
