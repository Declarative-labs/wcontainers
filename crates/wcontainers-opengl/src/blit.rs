//! Blits: Gallium's blitter, drawing through the driver's own draws with the state it saves and
//! puts back.

use wcontainers_gpu::Backend;

use crate::context::Context;
use crate::gallium::*;
use crate::resource;
use crate::state::{Stage, refuse};

/// The fragment stage's uniform buffer the blitter uses (`cb_slot`), saved with its bytes.
fn save_constants<B: Backend>(context: &mut Context<B>, blitter: &mut blitter_context) {
    let slot = blitter.cb_slot as usize;
    let constants = &context.stages.get(Stage::Fragment).constants[slot];
    context.blit_bytes.clone_from(&constants.bytes);
    let saved = &mut blitter.saved_fs_constant_buffer;
    // SAFETY: the slot's resource is null or live; the blitter's copy takes its own reference.
    unsafe { resource::reference(&mut saved.buffer, constants.resource) };
    if context.blit_bytes.is_empty() {
        saved.user_buffer = std::ptr::null();
        saved.buffer_offset = constants.offset;
        saved.buffer_size = constants.size;
    } else {
        saved.user_buffer = context.blit_bytes.as_ptr().cast();
        saved.buffer_offset = 0;
        saved.buffer_size = context.blit_bytes.len() as u32;
    }
}

/// Everything the blitter changes, saved in it so that it puts it back after its draws.
fn save<B: Backend>(context: &mut Context<B>, blitter: &mut blitter_context) {
    let bound = &context.bound;
    blitter.saved_blend_state = bound.blend.cast_mut().cast();
    blitter.saved_dsa_state = bound.depth_stencil.cast_mut().cast();
    blitter.saved_rs_state = bound.raster.cast_mut().cast();
    blitter.saved_velem_state = bound.elements.cast_mut().cast();
    blitter.saved_vs = context.stages.get(Stage::Vertex).shader.cast_mut().cast();
    blitter.saved_fs = context.stages.get(Stage::Fragment).shader.cast_mut().cast();
    blitter.saved_gs = std::ptr::null_mut();
    blitter.saved_tcs = std::ptr::null_mut();
    blitter.saved_tes = std::ptr::null_mut();
    blitter.saved_ms = std::ptr::null_mut();
    blitter.saved_stencil_ref = pipe_stencil_ref {
        ref_value: bound.stencil_ref,
    };
    blitter.saved_viewport = bound.viewport;
    blitter.saved_scissor = bound.scissor;
    blitter.is_sample_mask_saved = true;
    blitter.saved_sample_mask = bound.sample_mask;
    blitter.saved_min_samples = 1;
    blitter.saved_fb_state.nr_cbufs = 0;
    // SAFETY: both framebuffers are complete; the copy takes its own references.
    unsafe { util_copy_framebuffer_state(&mut blitter.saved_fb_state, &context.framebuffer.state) };
    let fragment = context.stages.get(Stage::Fragment);
    let samplers = blitter.saved_sampler_states.len();
    blitter.saved_num_sampler_states = samplers as u32;
    for (saved, &sampler) in blitter
        .saved_sampler_states
        .iter_mut()
        .zip(&fragment.samplers)
    {
        *saved = sampler.cast_mut().cast();
    }
    let views = blitter.saved_sampler_views.len();
    blitter.saved_num_sampler_views = views as u32;
    for (saved, &view) in blitter.saved_sampler_views.iter_mut().zip(&fragment.views) {
        // SAFETY: the views are null or live; the blitter's copy takes its own reference.
        unsafe { crate::view::reference(saved, view) };
    }
    let streams = &context.bound.vertex_buffers;
    blitter.saved_num_vb = streams.len() as u32;
    for (saved, stream) in blitter.saved_vertex_buffers.iter_mut().zip(streams) {
        saved.is_user_buffer = false;
        saved.buffer_offset = stream.offset;
        // SAFETY: the stream's resource is live; the blitter's copy takes its own reference.
        unsafe { resource::reference(&mut saved.buffer.resource, stream.resource) };
    }
    blitter.saved_num_so_targets = 0;
    blitter.saved_render_cond_query = std::ptr::null_mut();
    blitter.saved_num_window_rectangles = 0;
    save_constants(context, blitter);
}

/// `pipe_context::blit`.
pub unsafe extern "C" fn blit<B: Backend>(pipe: *mut pipe_context, info: *const pipe_blit_info) {
    // SAFETY: Mesa passes this driver's context and a blit.
    let context = unsafe { Context::<B>::of(pipe) };
    if context.blitter.is_null() {
        // SAFETY: the context is complete; the blitter keeps a pointer to it.
        context.blitter = unsafe { util_blitter_create(pipe) };
    }
    let blitter = context.blitter;
    // SAFETY: the blitter was made on this context.
    if !unsafe { util_blitter_is_blit_supported(blitter, info) } {
        refuse("a blit Gallium's blitter does not make");
    }
    // SAFETY: as above; the blitter is used by this context's thread only.
    save(context, unsafe { &mut *blitter });
    // SAFETY: as above; every state the blitter changes is saved.
    unsafe { util_blitter_blit(blitter, info, std::ptr::null_mut()) };
}
