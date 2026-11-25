//! Blits: Gallium's blitter, drawing through the driver's own draws with the state it saves and
//! puts back.

use std::ffi::c_uint;

use wcontainers_gpu::{
    Extent, FORMAT_FACTS, Format, FormatKind, FormatUse, Gpu, Origin, SlowPath, TextureDesc,
    TextureDimension, TextureRegion,
};

use crate::context::Context;
use crate::gallium::*;
use crate::resource::{self, Resource};
use crate::state::{Stage, refuse};

/// The fragment stage's uniform buffer the blitter uses (`cb_slot`), saved with its bytes.
fn save_constants<B: Gpu>(context: &mut Context<B>, blitter: &mut blitter_context) {
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
fn save<B: Gpu>(context: &mut Context<B>, blitter: &mut blitter_context) {
    // The render condition, which the blitter lifts for a blit that ignores it; every mode waits.
    let (query, condition) = context.condition.unwrap_or((std::ptr::null_mut(), false));
    blitter.saved_render_cond_query = query;
    blitter.saved_render_cond_cond = condition;
    blitter.saved_render_cond_mode = PIPE_RENDER_COND_WAIT;
    let bound = &context.bound;
    blitter.saved_blend_state = bound.blend.cast_mut().cast();
    blitter.saved_dsa_state = bound.depth_stencil.cast_mut().cast();
    blitter.saved_rs_state = bound.raster.cast_mut().cast();
    blitter.saved_velem_state = bound.elements.cast_mut().cast();
    blitter.saved_vs = context.stages.get(Stage::Vertex).shader.cast_mut().cast();
    blitter.saved_fs = context.stages.get(Stage::Fragment).shader.cast_mut().cast();
    blitter.saved_gs = context.stages.get(Stage::Geometry).shader.cast_mut().cast();
    blitter.saved_tcs = context
        .stages
        .get(Stage::TessControl)
        .shader
        .cast_mut()
        .cast();
    blitter.saved_tes = context
        .stages
        .get(Stage::TessEvaluation)
        .shader
        .cast_mut()
        .cast();
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

/// Resolves a whole multisampled level's layer into a single-sampled one of its size and format
/// by the back end's own resolve, where the blit is that and nothing more; false for any other.
fn whole_resolve<B: Gpu>(
    context: &mut Context<B>,
    info: &pipe_blit_info,
    from: &Resource<B>,
    to: &Resource<B>,
) -> bool {
    let (src, dst) = (&info.src, &info.dst);
    let (Some((from_texture, _)), Some((to_texture, _)), Some(format)) = (
        from.texture(),
        to.texture(),
        crate::format::format(src.format),
    ) else {
        return false;
    };
    let whole = |area: &pipe_box, resource: &Resource<B>, level: c_uint| {
        let side = |size: u32| (size >> level).max(1);
        area.x == 0
            && area.y == 0
            && area.depth == 1
            && area.width as u32 == side(resource.base.width0)
            && area.height as u32 == side(resource.base.height0)
    };
    // The host resolves only formats it averages; Gallium's blitter takes the rest (integers, one
    // sample of each pixel, as OpenGL resolves them).
    let resolves =
        context.screen().caps().formats.uses[format as usize].contains(FormatUse::RESOLVE);
    let plain = resolves
        && from.base.nr_samples > 1
        && to.base.nr_samples <= 1
        && src.format == dst.format
        && src.format == from.base.format()
        && dst.format == to.base.format()
        && info.mask == PIPE_MASK_RGBA
        && !info.scissor_enable
        && !info.swizzle_enable
        && !info.alpha_blend
        && info.num_window_rectangles == 0
        && (src.box_.width, src.box_.height) == (dst.box_.width, dst.box_.height)
        && whole(&src.box_, from, src.level)
        && whole(&dst.box_, to, dst.level);
    if !plain {
        return false;
    }
    let region = |texture, level, layer| TextureRegion {
        texture,
        mip: level,
        layer,
        origin: Origin::default(),
        stencil: false,
    };
    let mut backend = context.screen().backend();
    if let Err(error) = context.tracker.close(&mut backend) {
        refuse(format!("closing a pass: {error:?}"));
    }
    let layer = src.box_.z as u32;
    backend.resolve(
        &region(from_texture, src.level, layer),
        &region(to_texture, dst.level, dst.box_.z as u32),
        format,
    );
    true
}

/// Copies texels unchanged where the blit is a copy of their bits and nothing more: one format
/// for both sides, whose texels are as large as each texture's, unscaled, unmasked and
/// single-sampled, and either integer or seen in another format than a texture's own and not sRGB
/// (st/mesa's image copies between formats of one size, seen as one canonical format, unsigned
/// integer or unsigned normalised, which a blit carries bit for bit); false for any other. The
/// host copies between formats of one texel size, which no view of a texture in another format is
/// needed for.
fn bit_copy<B: Gpu>(
    context: &mut Context<B>,
    info: &pipe_blit_info,
    from: &Resource<B>,
    to: &Resource<B>,
) -> bool {
    let (src, dst) = (&info.src, &info.dst);
    let (Some((from_texture, from_desc)), Some((to_texture, to_desc)), Some(format)) = (
        from.texture(),
        to.texture(),
        crate::format::format(src.format),
    ) else {
        return false;
    };
    let one_texel = |f: Format| {
        let (width, height, bytes) = FORMAT_FACTS[f as usize].block;
        width == 1
            && height == 1
            && bytes == FORMAT_FACTS[format as usize].block.2
            && !FORMAT_FACTS[f as usize].depth_stencil
    };
    let (a, b) = (&src.box_, &dst.box_);
    let reinterprets = src.format != from.base.format() || dst.format != to.base.format();
    let exact = matches!(
        FORMAT_FACTS[format as usize].kind,
        FormatKind::Uint | FormatKind::Sint
    ) || reinterprets && FORMAT_FACTS[format as usize].linear == format;
    let plain = src.format == dst.format
        && exact
        && one_texel(from_desc.format)
        && one_texel(to_desc.format)
        && from_desc.samples <= 1
        && to_desc.samples <= 1
        && info.mask == PIPE_MASK_RGBA
        && !info.scissor_enable
        && !info.swizzle_enable
        && !info.alpha_blend
        && info.num_window_rectangles == 0
        && a.width > 0
        && a.height > 0
        && a.depth > 0
        && (a.width, a.height, a.depth) == (b.width, b.height, b.depth);
    if !plain {
        return false;
    }
    let mut backend = context.screen().backend();
    if let Err(error) = context.tracker.close(&mut backend) {
        refuse(format!("closing a pass: {error:?}"));
    }
    let corner = |texture, desc: &TextureDesc, level, at: &pipe_box| {
        crate::view::corner::<B>(
            texture,
            desc.dimension == TextureDimension::D3,
            level,
            at.x as u32,
            at.y as u32,
            at.z as u32,
        )
    };
    backend.copy_texture(
        &corner(from_texture, from_desc, src.level, a),
        &corner(to_texture, to_desc, dst.level, b),
        Extent {
            width: a.width as u32,
            height: a.height as u32,
            depth_or_layers: a.depth as u32,
        },
    );
    true
}

/// `pipe_context::blit`.
pub unsafe extern "C" fn blit<B: Gpu>(pipe: *mut pipe_context, info: *const pipe_blit_info) {
    crate::state::guest_call(pipe, (), move || {
        // SAFETY: Mesa passes this driver's context and a blit.
        let context = unsafe { Context::<B>::of(pipe) };
        if context.blitter.is_null() {
            // SAFETY: the context is complete; the blitter keeps a pointer to it.
            context.blitter = unsafe { util_blitter_create(pipe) };
        }
        // SAFETY: as above.
        if unsafe { (*info).render_condition_enable } && !crate::query::condition_holds(context) {
            return;
        }
        // SAFETY: as above; the blit names two resources of this driver.
        let (info, from, to) = unsafe {
            (
                &*info,
                Resource::<B>::of((*info).src.resource),
                Resource::<B>::of((*info).dst.resource),
            )
        };
        if !from.layers().is_empty() || !to.layers().is_empty() {
            by_layers(pipe, info, from, to);
            return;
        }
        if whole_resolve(context, info, from, to) || bit_copy(context, info, from, to) {
            return;
        }
        let blitter = context.blitter;
        let mut info = *info;
        if info.mask & PIPE_MASK_S != 0 {
            stencil_by_bits(context, &info);
            info.mask &= !PIPE_MASK_S;
            if info.mask == 0 {
                return;
            }
        }
        // SAFETY: the blitter was made on this context.
        if !unsafe { util_blitter_is_blit_supported(blitter, &info) } {
            refuse(format_args!(
                "a blit Gallium's blitter does not make (Gallium's formats {} to {}, of textures in {} \
                 and {}, mask {:#x})",
                info.src.format,
                info.dst.format,
                from.base.format(),
                to.base.format(),
                info.mask
            ));
        }
        // SAFETY: as above; the blitter is used by this context's thread only.
        save(context, unsafe { &mut *blitter });
        // SAFETY: as above; every state the blitter changes is saved.
        unsafe { util_blitter_blit(blitter, &info, std::ptr::null_mut()) };
    })
}

/// A blit from or into a texture held a layer a resource, as a blit a layer between the layers'
/// own resources.
fn by_layers<B: Gpu>(
    pipe: *mut pipe_context,
    info: &pipe_blit_info,
    from: &Resource<B>,
    to: &Resource<B>,
) {
    let (src, dst) = (info.src.box_, info.dst.box_);
    if src.depth != dst.depth {
        refuse(format_args!(
            "a blit scaling the layers of a multisampled array"
        ));
    }
    for i in 0..src.depth {
        let mut one = *info;
        for (side, resource, z) in [
            (&mut one.src, from, src.z + i),
            (&mut one.dst, to, dst.z + i),
        ] {
            side.box_.depth = 1;
            side.box_.z = z;
            if let Some(&layer) = resource.layers().get(z as usize) {
                side.resource = layer;
                side.box_.z = 0;
            }
        }
        // SAFETY: the context Mesa passed; the blit names two live resources of this driver.
        unsafe { blit::<B>(pipe, &one) };
    }
}

/// A blit's stencil, which no program writes on the host (no stencil export): the destination's
/// stencil cleared within the blit, then each bit of it set by a draw keeping the fragments whose
/// source has it (Gallium's blitter, `util_blitter_stencil_fallback`).
fn stencil_by_bits<B: Gpu>(context: &mut Context<B>, info: &pipe_blit_info) {
    let blitter = context.blitter;
    let (src, dst) = (&info.src, &info.dst);
    let mut surface = pipe_surface::default();
    // SAFETY: the blit's destination is a live texture of this driver; the template is filled.
    unsafe {
        util_blitter_default_dst_texture(&mut surface, dst.resource, dst.level, dst.box_.z as u32)
    };
    let b = &dst.box_;
    let (mut x0, mut y0, mut x1, mut y1) = (b.x, b.y, b.x + b.width, b.y + b.height);
    if info.scissor_enable {
        let s = &info.scissor;
        x0 = x0.max(s.minx as i32);
        y0 = y0.max(s.miny as i32);
        x1 = x1.min(s.maxx as i32);
        y1 = y1.min(s.maxy as i32);
    }
    if x1 > x0 && y1 > y0 {
        // SAFETY: as above; the blitter is used by this context's thread only.
        save(context, unsafe { &mut *blitter });
        // SAFETY: as above; every state the blitter changes is saved.
        unsafe {
            util_blitter_clear_depth_stencil(
                blitter,
                &mut surface,
                PIPE_CLEAR_STENCIL,
                0.0,
                0,
                x0 as u32,
                y0 as u32,
                (x1 - x0) as u32,
                (y1 - y0) as u32,
            )
        };
    }
    let scissor = if info.scissor_enable {
        &info.scissor as *const pipe_scissor_state
    } else {
        std::ptr::null()
    };
    // One draw a bit (and sample) writes the stencil, as for a stencil copy the host makes whole.
    context
        .screen()
        .backend()
        .record_slow_path(SlowPath::StencilWrittenByDraw);
    // SAFETY: as above.
    save(context, unsafe { &mut *blitter });
    // SAFETY: as above; both resources are live textures of this driver.
    unsafe {
        util_blitter_stencil_fallback(
            blitter,
            dst.resource,
            dst.level,
            &dst.box_,
            src.resource,
            src.level,
            &src.box_,
            scissor,
        )
    };
}
