// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! Mesa's structures as the glue's C copies them (flat.h): mirrors with C's layout (no bit-field, no union, each
//! object its key), the keys and client memory each names, and the trait's owned structure made from each.

use wcontainers_gallium::Gallium;

use crate::FieldLayout;
use crate::held::{Held, Keys, ObjectKind, each, flag, within};

/// Mesa's `pipe_blend_color` as the glue's C copies it (`struct wc_flat_pipe_blend_color` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlendColor {
    pub color: [f32; 4],
}

impl FlatBlendColor {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::BlendColor> {
        Some(wcontainers_gallium::BlendColor {
            color: self.color,
        })
    }
}

/// Mesa's `pipe_rt_blend_state` as the glue's C copies it (`struct wc_flat_pipe_rt_blend_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatRtBlendState {
    pub blend_enable: u32,
    pub rgb_func: u32,
    pub rgb_src_factor: u32,
    pub rgb_dst_factor: u32,
    pub alpha_func: u32,
    pub alpha_src_factor: u32,
    pub alpha_dst_factor: u32,
    pub colormask: u32,
}

impl FlatRtBlendState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::RtBlendState> {
        Some(wcontainers_gallium::RtBlendState {
            blend_enable: within(self.blend_enable, 1)?,
            rgb_func: within(self.rgb_func, 3)?,
            rgb_src_factor: within(self.rgb_src_factor, 5)?,
            rgb_dst_factor: within(self.rgb_dst_factor, 5)?,
            alpha_func: within(self.alpha_func, 3)?,
            alpha_src_factor: within(self.alpha_src_factor, 5)?,
            alpha_dst_factor: within(self.alpha_dst_factor, 5)?,
            colormask: within(self.colormask, 4)?,
        })
    }
}

/// Mesa's `pipe_blend_state` as the glue's C copies it (`struct wc_flat_pipe_blend_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlendState {
    pub independent_blend_enable: u32,
    pub logicop_enable: u32,
    pub logicop_func: u32,
    pub dither: u32,
    pub alpha_to_coverage: u32,
    pub alpha_to_coverage_dither: u32,
    pub alpha_to_one: u32,
    pub max_rt: u32,
    pub advanced_blend_func: u32,
    pub blend_coherent: u32,
    pub is_yuv: u32,
    pub rt: [FlatRtBlendState; 8],
}

impl FlatBlendState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::BlendState> {
        Some(wcontainers_gallium::BlendState {
            independent_blend_enable: within(self.independent_blend_enable, 1)?,
            logicop_enable: within(self.logicop_enable, 1)?,
            logicop_func: within(self.logicop_func, 4)?,
            dither: within(self.dither, 1)?,
            alpha_to_coverage: within(self.alpha_to_coverage, 1)?,
            alpha_to_coverage_dither: within(self.alpha_to_coverage_dither, 1)?,
            alpha_to_one: within(self.alpha_to_one, 1)?,
            max_rt: within(self.max_rt, 3)?,
            advanced_blend_func: within(self.advanced_blend_func, 4)?,
            blend_coherent: within(self.blend_coherent, 1)?,
            is_yuv: within(self.is_yuv, 1)?,
            rt: each(&self.rt, |element0| element0.owned(held))?,
        })
    }
}

/// Mesa's `pipe_box` as the glue's C copies it (`struct wc_flat_pipe_box` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatPipeBox {
    pub x: i32,
    pub width: i32,
    pub y: i32,
    pub height: i32,
    pub z: i16,
    pub depth: i16,
}

impl FlatPipeBox {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::PipeBox> {
        Some(wcontainers_gallium::PipeBox {
            x: self.x,
            width: self.width,
            y: self.y,
            height: self.height,
            z: self.z,
            depth: self.depth,
        })
    }
}

/// Mesa's `pipe_blit_info.dst` as the glue's C copies it (`struct wc_flat_pipe_blit_info__dst` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlitInfoDst {
    pub resource: usize,
    pub level: u32,
    pub box_: FlatPipeBox,
    pub format: u32,
}

impl FlatBlitInfoDst {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::BlitInfoDst<'a, G::Resource>> {
        Some(wcontainers_gallium::BlitInfoDst {
            resource: held.resource(self.resource)?,
            level: self.level,
            box_: self.box_.owned(held)?,
            format: self.format,
        })
    }
}

/// Mesa's `pipe_blit_info.src` as the glue's C copies it (`struct wc_flat_pipe_blit_info__src` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlitInfoSrc {
    pub resource: usize,
    pub level: u32,
    pub box_: FlatPipeBox,
    pub format: u32,
}

impl FlatBlitInfoSrc {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::BlitInfoSrc<'a, G::Resource>> {
        Some(wcontainers_gallium::BlitInfoSrc {
            resource: held.resource(self.resource)?,
            level: self.level,
            box_: self.box_.owned(held)?,
            format: self.format,
        })
    }
}

/// Mesa's `pipe_scissor_state` as the glue's C copies it (`struct wc_flat_pipe_scissor_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatScissorState {
    pub minx: u32,
    pub miny: u32,
    pub maxx: u32,
    pub maxy: u32,
}

impl FlatScissorState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ScissorState> {
        Some(wcontainers_gallium::ScissorState {
            minx: self.minx,
            miny: self.miny,
            maxx: self.maxx,
            maxy: self.maxy,
        })
    }
}

/// Mesa's `pipe_blit_info` as the glue's C copies it (`struct wc_flat_pipe_blit_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatBlitInfo {
    pub dst: FlatBlitInfoDst,
    pub src: FlatBlitInfoSrc,
    pub mask: u32,
    pub filter: u32,
    pub dst_sample: u8,
    pub sample0_only: u8,
    pub scissor_enable: u8,
    pub scissor: FlatScissorState,
    pub swizzle_enable: u8,
    pub swizzle: [u8; 4],
    pub window_rectangle_include: u8,
    pub num_window_rectangles: u32,
    pub window_rectangles: [FlatScissorState; 8],
    pub render_condition_enable: u8,
    pub alpha_blend: u8,
}

impl FlatBlitInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        self.dst.keys(keys);
        self.src.keys(keys);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::BlitInfo<'a, G::Resource>> {
        Some(wcontainers_gallium::BlitInfo {
            dst: self.dst.owned(held)?,
            src: self.src.owned(held)?,
            mask: self.mask,
            filter: self.filter,
            dst_sample: self.dst_sample,
            sample0_only: flag(self.sample0_only)?,
            scissor_enable: flag(self.scissor_enable)?,
            scissor: self.scissor.owned(held)?,
            swizzle_enable: flag(self.swizzle_enable)?,
            swizzle: self.swizzle,
            window_rectangle_include: flag(self.window_rectangle_include)?,
            num_window_rectangles: self.num_window_rectangles,
            window_rectangles: each(&self.window_rectangles, |element0| element0.owned(held))?,
            render_condition_enable: flag(self.render_condition_enable)?,
            alpha_blend: flag(self.alpha_blend)?,
        })
    }
}

/// Mesa's `pipe_clip_state` as the glue's C copies it (`struct wc_flat_pipe_clip_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatClipState {
    pub ucp: [[f32; 4]; 8],
}

impl FlatClipState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ClipState> {
        Some(wcontainers_gallium::ClipState {
            ucp: self.ucp,
        })
    }
}

/// Mesa's `pipe_compute_state` as the glue's C copies it (`struct wc_flat_pipe_compute_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatComputeState {
    pub static_shared_mem: u32,
}

impl FlatComputeState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ComputeState> {
        Some(wcontainers_gallium::ComputeState {
            static_shared_mem: self.static_shared_mem,
        })
    }
}

/// Mesa's `pipe_constant_buffer` as the glue's C copies it (`struct wc_flat_pipe_constant_buffer` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatConstantBuffer {
    pub buffer: usize,
    pub buffer_offset: u32,
    pub buffer_size: u32,
    pub user_buffer: usize,
}

impl FlatConstantBuffer {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.buffer);
        keys.memory(self.user_buffer, usize::try_from(self.buffer_offset).unwrap_or(usize::MAX), usize::try_from(self.buffer_size).unwrap_or(usize::MAX));
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::ConstantBuffer<'a, G::Resource>> {
        Some(wcontainers_gallium::ConstantBuffer {
            buffer: held.resource(self.buffer)?,
            buffer_offset: self.buffer_offset,
            buffer_size: self.buffer_size,
            user_buffer: held.memory(self.user_buffer, usize::try_from(self.buffer_offset).unwrap_or(usize::MAX), usize::try_from(self.buffer_size).unwrap_or(usize::MAX))?,
        })
    }
}

/// Mesa's `pipe_stencil_state` as the glue's C copies it (`struct wc_flat_pipe_stencil_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatStencilState {
    pub enabled: u32,
    pub func: u32,
    pub fail_op: u32,
    pub zpass_op: u32,
    pub zfail_op: u32,
    pub valuemask: u32,
    pub writemask: u32,
}

impl FlatStencilState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::StencilState> {
        Some(wcontainers_gallium::StencilState {
            enabled: within(self.enabled, 1)?,
            func: within(self.func, 3)?,
            fail_op: within(self.fail_op, 3)?,
            zpass_op: within(self.zpass_op, 3)?,
            zfail_op: within(self.zfail_op, 3)?,
            valuemask: within(self.valuemask, 8)?,
            writemask: within(self.writemask, 8)?,
        })
    }
}

/// Mesa's `pipe_depth_stencil_alpha_state` as the glue's C copies it (`struct wc_flat_pipe_depth_stencil_alpha_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDepthStencilAlphaState {
    pub stencil: [FlatStencilState; 2],
    pub alpha_enabled: u32,
    pub alpha_func: u32,
    pub depth_enabled: u32,
    pub depth_writemask: u32,
    pub depth_func: u32,
    pub depth_bounds_test: u32,
    pub alpha_ref_value: f32,
    pub depth_bounds_min: f64,
    pub depth_bounds_max: f64,
}

impl FlatDepthStencilAlphaState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::DepthStencilAlphaState> {
        Some(wcontainers_gallium::DepthStencilAlphaState {
            stencil: each(&self.stencil, |element0| element0.owned(held))?,
            alpha_enabled: within(self.alpha_enabled, 1)?,
            alpha_func: within(self.alpha_func, 3)?,
            depth_enabled: within(self.depth_enabled, 1)?,
            depth_writemask: within(self.depth_writemask, 1)?,
            depth_func: within(self.depth_func, 3)?,
            depth_bounds_test: within(self.depth_bounds_test, 1)?,
            alpha_ref_value: self.alpha_ref_value,
            depth_bounds_min: self.depth_bounds_min,
            depth_bounds_max: self.depth_bounds_max,
        })
    }
}

/// Mesa's `pipe_draw_indirect_info` as the glue's C copies it (`struct wc_flat_pipe_draw_indirect_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDrawIndirectInfo {
    pub offset: u32,
    pub stride: u32,
    pub draw_count: u32,
    pub indirect_draw_count_offset: u32,
    pub buffer: usize,
    pub indirect_draw_count: usize,
    pub count_from_stream_output: usize,
}

impl FlatDrawIndirectInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.buffer);
        keys.object(ObjectKind::Resource, self.indirect_draw_count);
        keys.object(ObjectKind::StreamOutputTarget, self.count_from_stream_output);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::DrawIndirectInfo<'a, G::Resource, G::StreamOutputTarget>> {
        Some(wcontainers_gallium::DrawIndirectInfo {
            offset: self.offset,
            stride: self.stride,
            draw_count: self.draw_count,
            indirect_draw_count_offset: self.indirect_draw_count_offset,
            buffer: held.resource(self.buffer)?,
            indirect_draw_count: held.resource(self.indirect_draw_count)?,
            count_from_stream_output: held.stream_output_target(self.count_from_stream_output)?,
        })
    }
}

/// Mesa's `pipe_draw_info.index` as the glue's C copies it (`struct wc_flat_pipe_draw_info__index` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDrawInfoIndex {
    pub resource: usize,
    pub user: usize,
}

impl FlatDrawInfoIndex {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
        keys.memory(self.user, 0, keys.index_bytes);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::DrawInfoIndex<'a, G::Resource>> {
        Some(wcontainers_gallium::DrawInfoIndex {
            resource: held.resource(self.resource)?,
            user: held.memory(self.user, 0, held.index_bytes)?,
        })
    }
}

/// Mesa's `pipe_draw_info` as the glue's C copies it (`struct wc_flat_pipe_draw_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDrawInfo {
    pub mode: u32,
    pub index_size: u16,
    pub primitive_restart: u8,
    pub has_user_indices: u8,
    pub index_bounds_valid: u8,
    pub increment_draw_id: u8,
    pub index_bias_varies: u8,
    pub was_line_loop: u8,
    pub _pad: u8,
    pub start_instance: u32,
    pub instance_count: u32,
    pub restart_index: u32,
    pub index: FlatDrawInfoIndex,
    pub min_index: u32,
    pub max_index: u32,
}

impl FlatDrawInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        self.index.keys(keys);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::DrawInfo<'a, G::Resource>> {
        Some(wcontainers_gallium::DrawInfo {
            mode: self.mode,
            index_size: self.index_size,
            primitive_restart: flag(self.primitive_restart)?,
            has_user_indices: flag(self.has_user_indices)?,
            index_bounds_valid: flag(self.index_bounds_valid)?,
            increment_draw_id: flag(self.increment_draw_id)?,
            index_bias_varies: flag(self.index_bias_varies)?,
            was_line_loop: flag(self.was_line_loop)?,
            _pad: within(self._pad, 2)?,
            start_instance: self.start_instance,
            instance_count: self.instance_count,
            restart_index: self.restart_index,
            index: self.index.owned(held)?,
            min_index: self.min_index,
            max_index: self.max_index,
        })
    }
}

/// Mesa's `pipe_draw_start_count_bias` as the glue's C copies it (`struct wc_flat_pipe_draw_start_count_bias` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatDrawStartCountBias {
    pub start: u32,
    pub count: u32,
    pub index_bias: i32,
}

impl FlatDrawStartCountBias {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::DrawStartCountBias> {
        Some(wcontainers_gallium::DrawStartCountBias {
            start: self.start,
            count: self.count,
            index_bias: self.index_bias,
        })
    }
}

/// Mesa's `pipe_reference` as the glue's C copies it (`struct wc_flat_pipe_reference` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatReference {
    pub count: i32,
}

impl FlatReference {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::Reference> {
        Some(wcontainers_gallium::Reference {
            count: self.count,
        })
    }
}

/// Mesa's `pipe_surface` as the glue's C copies it (`struct wc_flat_pipe_surface` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSurface {
    pub reference: FlatReference,
    pub format: u32,
    pub nr_samples: u32,
    pub first_layer: u32,
    pub last_layer: u32,
    pub level: u32,
    pub texture: usize,
}

impl FlatSurface {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.texture);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::Surface<'a, G::Resource>> {
        Some(wcontainers_gallium::Surface {
            reference: self.reference.owned(held)?,
            format: within(self.format, 16)?,
            nr_samples: within(self.nr_samples, 16)?,
            first_layer: within(self.first_layer, 16)?,
            last_layer: within(self.last_layer, 16)?,
            level: self.level,
            texture: held.resource(self.texture)?,
        })
    }
}

/// Mesa's `pipe_framebuffer_state` as the glue's C copies it (`struct wc_flat_pipe_framebuffer_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatFramebufferState {
    pub width: u32,
    pub height: u32,
    pub layers: u16,
    pub samples: u8,
    pub nr_cbufs: u8,
    pub pls_enabled: u8,
    pub downscale_cbufs: u8,
    pub _pad: u8,
    pub viewmask: u8,
    pub cbufs: [FlatSurface; 8],
    pub zsbuf: FlatSurface,
    pub resolve: usize,
}

impl FlatFramebufferState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        for element0 in &self.cbufs {
            element0.keys(keys);
        }
        self.zsbuf.keys(keys);
        keys.object(ObjectKind::Resource, self.resolve);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::FramebufferState<'a, G::Resource>> {
        Some(wcontainers_gallium::FramebufferState {
            width: self.width,
            height: self.height,
            layers: self.layers,
            samples: self.samples,
            nr_cbufs: self.nr_cbufs,
            pls_enabled: flag(self.pls_enabled)?,
            downscale_cbufs: flag(self.downscale_cbufs)?,
            _pad: within(self._pad, 6)?,
            viewmask: self.viewmask,
            cbufs: each(&self.cbufs, |element0| element0.owned(held))?,
            zsbuf: self.zsbuf.owned(held)?,
            resolve: held.resource(self.resolve)?,
        })
    }
}

/// Mesa's `pipe_grid_info` as the glue's C copies it (`struct wc_flat_pipe_grid_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatGridInfo {
    pub variable_shared_mem: u32,
    pub work_dim: u32,
    pub block: [u32; 3],
    pub last_block: [u32; 3],
    pub grid: [u32; 3],
    pub grid_base: [u32; 3],
    pub indirect: usize,
    pub indirect_offset: u32,
    pub indirect_stride: u32,
    pub draw_count: u32,
    pub indirect_draw_count_offset: u32,
    pub indirect_draw_count: usize,
}

impl FlatGridInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.indirect);
        keys.object(ObjectKind::Resource, self.indirect_draw_count);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::GridInfo<'a, G::Resource>> {
        Some(wcontainers_gallium::GridInfo {
            variable_shared_mem: self.variable_shared_mem,
            work_dim: self.work_dim,
            block: self.block,
            last_block: self.last_block,
            grid: self.grid,
            grid_base: self.grid_base,
            indirect: held.resource(self.indirect)?,
            indirect_offset: self.indirect_offset,
            indirect_stride: self.indirect_stride,
            draw_count: self.draw_count,
            indirect_draw_count_offset: self.indirect_draw_count_offset,
            indirect_draw_count: held.resource(self.indirect_draw_count)?,
        })
    }
}

/// Mesa's `pipe_image_view.u.tex` as the glue's C copies it (`struct wc_flat_pipe_image_view__u__tex` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatImageViewUTex {
    pub first_layer: u32,
    pub last_layer: u32,
    pub level: u32,
    pub single_layer_view: u8,
    pub is_2d_view_of_3d: u8,
}

impl FlatImageViewUTex {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ImageViewUTex> {
        Some(wcontainers_gallium::ImageViewUTex {
            first_layer: within(self.first_layer, 16)?,
            last_layer: within(self.last_layer, 16)?,
            level: within(self.level, 8)?,
            single_layer_view: flag(self.single_layer_view)?,
            is_2d_view_of_3d: flag(self.is_2d_view_of_3d)?,
        })
    }
}

/// Mesa's `pipe_image_view.u.buf` as the glue's C copies it (`struct wc_flat_pipe_image_view__u__buf` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatImageViewUBuf {
    pub offset: u32,
    pub size: u32,
}

impl FlatImageViewUBuf {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ImageViewUBuf> {
        Some(wcontainers_gallium::ImageViewUBuf {
            offset: self.offset,
            size: self.size,
        })
    }
}

/// Mesa's `pipe_tex2d_from_buf` as the glue's C copies it (`struct wc_flat_pipe_tex2d_from_buf` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatTex2dFromBuf {
    pub offset: u32,
    pub row_stride: u16,
    pub width: u16,
    pub height: u16,
}

impl FlatTex2dFromBuf {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::Tex2dFromBuf> {
        Some(wcontainers_gallium::Tex2dFromBuf {
            offset: self.offset,
            row_stride: self.row_stride,
            width: self.width,
            height: self.height,
        })
    }
}

/// Mesa's `pipe_image_view.u` as the glue's C copies it (`struct wc_flat_pipe_image_view__u` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatImageViewU {
    pub tex: FlatImageViewUTex,
    pub buf: FlatImageViewUBuf,
    pub tex2d_from_buf: FlatTex2dFromBuf,
}

impl FlatImageViewU {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::ImageViewU> {
        Some(wcontainers_gallium::ImageViewU {
            tex: self.tex.owned(held)?,
            buf: self.buf.owned(held)?,
            tex2d_from_buf: self.tex2d_from_buf.owned(held)?,
        })
    }
}

/// Mesa's `pipe_image_view` as the glue's C copies it (`struct wc_flat_pipe_image_view` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatImageView {
    pub resource: usize,
    pub format: u32,
    pub access: u16,
    pub shader_access: u16,
    pub u: FlatImageViewU,
}

impl FlatImageView {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::ImageView<'a, G::Resource>> {
        Some(wcontainers_gallium::ImageView {
            resource: held.resource(self.resource)?,
            format: self.format,
            access: self.access,
            shader_access: self.shader_access,
            u: self.u.owned(held)?,
        })
    }
}

/// Mesa's `pipe_poly_stipple` as the glue's C copies it (`struct wc_flat_pipe_poly_stipple` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatPolyStipple {
    pub stipple: [u32; 32],
}

impl FlatPolyStipple {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::PolyStipple> {
        Some(wcontainers_gallium::PolyStipple {
            stipple: self.stipple,
        })
    }
}

/// Mesa's `pipe_rasterizer_state` as the glue's C copies it (`struct wc_flat_pipe_rasterizer_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatRasterizerState {
    pub flatshade: u32,
    pub light_twoside: u32,
    pub clamp_vertex_color: u32,
    pub clamp_fragment_color: u32,
    pub front_ccw: u32,
    pub cull_face: u32,
    pub fill_front: u32,
    pub fill_back: u32,
    pub offset_point: u32,
    pub offset_line: u32,
    pub offset_tri: u32,
    pub scissor: u32,
    pub poly_smooth: u32,
    pub poly_stipple_enable: u32,
    pub point_smooth: u32,
    pub sprite_coord_mode: u32,
    pub point_quad_rasterization: u32,
    pub point_line_tri_clip: u32,
    pub point_size_per_vertex: u32,
    pub multisample: u32,
    pub no_ms_sample_mask_out: u32,
    pub force_persample_interp: u32,
    pub line_smooth: u32,
    pub line_stipple_enable: u32,
    pub line_last_pixel: u32,
    pub line_rectangular: u32,
    pub conservative_raster_mode: u32,
    pub flatshade_first: u32,
    pub half_pixel_center: u32,
    pub bottom_edge_rule: u32,
    pub subpixel_precision_x: u32,
    pub subpixel_precision_y: u32,
    pub rasterizer_discard: u32,
    pub rasterization_stream: u32,
    pub tile_raster_order_fixed: u32,
    pub tile_raster_order_increasing_x: u32,
    pub tile_raster_order_increasing_y: u32,
    pub depth_clip_near: u32,
    pub depth_clip_far: u32,
    pub depth_clamp: u32,
    pub clip_halfz: u32,
    pub offset_units_unscaled: u32,
    pub unclamped_fragment_depth_values: u32,
    pub clip_plane_enable: u32,
    pub representative_fragment_test: u32,
    pub line_stipple_factor: u32,
    pub line_stipple_pattern: u32,
    pub sprite_coord_enable: u16,
    pub line_width: f32,
    pub point_size: f32,
    pub offset_units: f32,
    pub offset_scale: f32,
    pub offset_clamp: f32,
    pub conservative_raster_dilate: f32,
}

impl FlatRasterizerState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::RasterizerState> {
        Some(wcontainers_gallium::RasterizerState {
            flatshade: within(self.flatshade, 1)?,
            light_twoside: within(self.light_twoside, 1)?,
            clamp_vertex_color: within(self.clamp_vertex_color, 1)?,
            clamp_fragment_color: within(self.clamp_fragment_color, 1)?,
            front_ccw: within(self.front_ccw, 1)?,
            cull_face: within(self.cull_face, 2)?,
            fill_front: within(self.fill_front, 2)?,
            fill_back: within(self.fill_back, 2)?,
            offset_point: within(self.offset_point, 1)?,
            offset_line: within(self.offset_line, 1)?,
            offset_tri: within(self.offset_tri, 1)?,
            scissor: within(self.scissor, 1)?,
            poly_smooth: within(self.poly_smooth, 1)?,
            poly_stipple_enable: within(self.poly_stipple_enable, 1)?,
            point_smooth: within(self.point_smooth, 1)?,
            sprite_coord_mode: within(self.sprite_coord_mode, 1)?,
            point_quad_rasterization: within(self.point_quad_rasterization, 1)?,
            point_line_tri_clip: within(self.point_line_tri_clip, 1)?,
            point_size_per_vertex: within(self.point_size_per_vertex, 1)?,
            multisample: within(self.multisample, 1)?,
            no_ms_sample_mask_out: within(self.no_ms_sample_mask_out, 1)?,
            force_persample_interp: within(self.force_persample_interp, 1)?,
            line_smooth: within(self.line_smooth, 1)?,
            line_stipple_enable: within(self.line_stipple_enable, 1)?,
            line_last_pixel: within(self.line_last_pixel, 1)?,
            line_rectangular: within(self.line_rectangular, 1)?,
            conservative_raster_mode: within(self.conservative_raster_mode, 2)?,
            flatshade_first: within(self.flatshade_first, 1)?,
            half_pixel_center: within(self.half_pixel_center, 1)?,
            bottom_edge_rule: within(self.bottom_edge_rule, 1)?,
            subpixel_precision_x: within(self.subpixel_precision_x, 4)?,
            subpixel_precision_y: within(self.subpixel_precision_y, 4)?,
            rasterizer_discard: within(self.rasterizer_discard, 1)?,
            rasterization_stream: within(self.rasterization_stream, 2)?,
            tile_raster_order_fixed: within(self.tile_raster_order_fixed, 1)?,
            tile_raster_order_increasing_x: within(self.tile_raster_order_increasing_x, 1)?,
            tile_raster_order_increasing_y: within(self.tile_raster_order_increasing_y, 1)?,
            depth_clip_near: within(self.depth_clip_near, 1)?,
            depth_clip_far: within(self.depth_clip_far, 1)?,
            depth_clamp: within(self.depth_clamp, 1)?,
            clip_halfz: within(self.clip_halfz, 1)?,
            offset_units_unscaled: within(self.offset_units_unscaled, 1)?,
            unclamped_fragment_depth_values: within(self.unclamped_fragment_depth_values, 1)?,
            clip_plane_enable: within(self.clip_plane_enable, 8)?,
            representative_fragment_test: within(self.representative_fragment_test, 1)?,
            line_stipple_factor: within(self.line_stipple_factor, 8)?,
            line_stipple_pattern: within(self.line_stipple_pattern, 16)?,
            sprite_coord_enable: self.sprite_coord_enable,
            line_width: self.line_width,
            point_size: self.point_size,
            offset_units: self.offset_units,
            offset_scale: self.offset_scale,
            offset_clamp: self.offset_clamp,
            conservative_raster_dilate: self.conservative_raster_dilate,
        })
    }
}

/// Mesa's `pipe_resource` as the glue's C copies it (`struct wc_flat_pipe_resource` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatResourceTemplate {
    pub width0: u32,
    pub height0: u32,
    pub depth0: u16,
    pub array_size: u16,
    pub format: u32,
    pub target: u32,
    pub last_level: u8,
    pub nr_samples: u8,
    pub nr_storage_samples: u8,
    pub nr_sparse_levels: u8,
    pub compression_rate: u32,
    pub usage: u32,
    pub bind: u32,
    pub flags: u32,
}

impl FlatResourceTemplate {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ResourceTemplate> {
        Some(wcontainers_gallium::ResourceTemplate {
            width0: self.width0,
            height0: self.height0,
            depth0: self.depth0,
            array_size: self.array_size,
            format: within(self.format, 16)?,
            target: within(self.target, 8)?,
            last_level: self.last_level,
            nr_samples: self.nr_samples,
            nr_storage_samples: self.nr_storage_samples,
            nr_sparse_levels: self.nr_sparse_levels,
            compression_rate: within(self.compression_rate, 4)?,
            usage: within(self.usage, 4)?,
            bind: self.bind,
            flags: self.flags,
        })
    }
}

/// Mesa's `pipe_sampler_state` as the glue's C copies it (`struct wc_flat_pipe_sampler_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerState {
    pub wrap_s: u32,
    pub wrap_t: u32,
    pub wrap_r: u32,
    pub min_img_filter: u32,
    pub min_mip_filter: u32,
    pub mag_img_filter: u32,
    pub compare_mode: u32,
    pub compare_func: u32,
    pub unnormalized_coords: u32,
    pub max_anisotropy: u32,
    pub seamless_cube_map: u32,
    pub border_color_is_integer: u32,
    pub reduction_mode: u32,
    pub pad: u32,
    pub lod_bias: f32,
    pub min_lod: f32,
    pub max_lod: f32,
    pub border_color: [u32; 4],
    pub border_color_format: u32,
}

impl FlatSamplerState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::SamplerState> {
        Some(wcontainers_gallium::SamplerState {
            wrap_s: within(self.wrap_s, 3)?,
            wrap_t: within(self.wrap_t, 3)?,
            wrap_r: within(self.wrap_r, 3)?,
            min_img_filter: within(self.min_img_filter, 1)?,
            min_mip_filter: within(self.min_mip_filter, 2)?,
            mag_img_filter: within(self.mag_img_filter, 1)?,
            compare_mode: within(self.compare_mode, 1)?,
            compare_func: within(self.compare_func, 3)?,
            unnormalized_coords: within(self.unnormalized_coords, 1)?,
            max_anisotropy: within(self.max_anisotropy, 5)?,
            seamless_cube_map: within(self.seamless_cube_map, 1)?,
            border_color_is_integer: within(self.border_color_is_integer, 1)?,
            reduction_mode: within(self.reduction_mode, 2)?,
            pad: within(self.pad, 5)?,
            lod_bias: self.lod_bias,
            min_lod: self.min_lod,
            max_lod: self.max_lod,
            border_color: self.border_color,
            border_color_format: self.border_color_format,
        })
    }
}

/// Mesa's `pipe_sampler_view.u.tex` as the glue's C copies it (`struct wc_flat_pipe_sampler_view__u__tex` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerViewUTex {
    pub first_layer: u32,
    pub last_layer: u32,
    pub first_level: u32,
    pub last_level: u32,
    pub min_lod_clamp: f32,
}

impl FlatSamplerViewUTex {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::SamplerViewUTex> {
        Some(wcontainers_gallium::SamplerViewUTex {
            first_layer: within(self.first_layer, 16)?,
            last_layer: within(self.last_layer, 16)?,
            first_level: within(self.first_level, 8)?,
            last_level: within(self.last_level, 8)?,
            min_lod_clamp: self.min_lod_clamp,
        })
    }
}

/// Mesa's `pipe_sampler_view.u.buf` as the glue's C copies it (`struct wc_flat_pipe_sampler_view__u__buf` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerViewUBuf {
    pub offset: u32,
    pub size: u32,
}

impl FlatSamplerViewUBuf {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::SamplerViewUBuf> {
        Some(wcontainers_gallium::SamplerViewUBuf {
            offset: self.offset,
            size: self.size,
        })
    }
}

/// Mesa's `pipe_sampler_view.u` as the glue's C copies it (`struct wc_flat_pipe_sampler_view__u` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerViewU {
    pub tex: FlatSamplerViewUTex,
    pub buf: FlatSamplerViewUBuf,
    pub tex2d_from_buf: FlatTex2dFromBuf,
}

impl FlatSamplerViewU {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::SamplerViewU> {
        Some(wcontainers_gallium::SamplerViewU {
            tex: self.tex.owned(held)?,
            buf: self.buf.owned(held)?,
            tex2d_from_buf: self.tex2d_from_buf.owned(held)?,
        })
    }
}

/// Mesa's `pipe_sampler_view` as the glue's C copies it (`struct wc_flat_pipe_sampler_view` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatSamplerViewTemplate {
    pub format: u32,
    pub astc_decode_format: u32,
    pub is_tex2d_from_buf: u8,
    pub target: u32,
    pub swizzle_r: u32,
    pub swizzle_g: u32,
    pub swizzle_b: u32,
    pub swizzle_a: u32,
    pub u: FlatSamplerViewU,
}

impl FlatSamplerViewTemplate {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::SamplerViewTemplate> {
        Some(wcontainers_gallium::SamplerViewTemplate {
            format: within(self.format, 12)?,
            astc_decode_format: within(self.astc_decode_format, 2)?,
            is_tex2d_from_buf: flag(self.is_tex2d_from_buf)?,
            target: within(self.target, 5)?,
            swizzle_r: within(self.swizzle_r, 3)?,
            swizzle_g: within(self.swizzle_g, 3)?,
            swizzle_b: within(self.swizzle_b, 3)?,
            swizzle_a: within(self.swizzle_a, 3)?,
            u: self.u.owned(held)?,
        })
    }
}

/// Mesa's `pipe_shader_buffer` as the glue's C copies it (`struct wc_flat_pipe_shader_buffer` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatShaderBuffer {
    pub buffer: usize,
    pub buffer_offset: u32,
    pub buffer_size: u32,
}

impl FlatShaderBuffer {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.buffer);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::ShaderBuffer<'a, G::Resource>> {
        Some(wcontainers_gallium::ShaderBuffer {
            buffer: held.resource(self.buffer)?,
            buffer_offset: self.buffer_offset,
            buffer_size: self.buffer_size,
        })
    }
}

/// Mesa's `pipe_stream_output` as the glue's C copies it (`struct wc_flat_pipe_stream_output` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatStreamOutput {
    pub register_index: u32,
    pub start_component: u32,
    pub num_components: u32,
    pub output_buffer: u32,
    pub dst_offset: u32,
    pub stream: u32,
}

impl FlatStreamOutput {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::StreamOutput> {
        Some(wcontainers_gallium::StreamOutput {
            register_index: within(self.register_index, 6)?,
            start_component: within(self.start_component, 2)?,
            num_components: within(self.num_components, 3)?,
            output_buffer: within(self.output_buffer, 3)?,
            dst_offset: within(self.dst_offset, 16)?,
            stream: within(self.stream, 2)?,
        })
    }
}

/// Mesa's `pipe_stream_output_info` as the glue's C copies it (`struct wc_flat_pipe_stream_output_info` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatStreamOutputInfo {
    pub num_outputs: u32,
    pub stride: [u16; 4],
    pub output: [FlatStreamOutput; 128],
}

impl FlatStreamOutputInfo {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::StreamOutputInfo> {
        Some(wcontainers_gallium::StreamOutputInfo {
            num_outputs: self.num_outputs,
            stride: self.stride,
            output: each(&self.output, |element0| element0.owned(held))?,
        })
    }
}

/// Mesa's `pipe_shader_state` as the glue's C copies it (`struct wc_flat_pipe_shader_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatShaderState {
    pub stream_output: FlatStreamOutputInfo,
    pub report_compile_error: u8,
}

impl FlatShaderState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, held: &Held<G>) -> Option<wcontainers_gallium::ShaderState> {
        Some(wcontainers_gallium::ShaderState {
            stream_output: self.stream_output.owned(held)?,
            report_compile_error: flag(self.report_compile_error)?,
        })
    }
}

/// Mesa's `pipe_stencil_ref` as the glue's C copies it (`struct wc_flat_pipe_stencil_ref` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatStencilRef {
    pub ref_value: [u8; 2],
}

impl FlatStencilRef {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::StencilRef> {
        Some(wcontainers_gallium::StencilRef {
            ref_value: self.ref_value,
        })
    }
}

/// Mesa's `pipe_vertex_buffer.buffer` as the glue's C copies it (`struct wc_flat_pipe_vertex_buffer__buffer` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatVertexBufferBuffer {
    pub resource: usize,
}

impl FlatVertexBufferBuffer {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        keys.object(ObjectKind::Resource, self.resource);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::VertexBufferBuffer<'a, G::Resource>> {
        Some(wcontainers_gallium::VertexBufferBuffer {
            resource: held.resource(self.resource)?,
        })
    }
}

/// Mesa's `pipe_vertex_buffer` as the glue's C copies it (`struct wc_flat_pipe_vertex_buffer` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatVertexBuffer {
    pub is_user_buffer: u8,
    pub buffer_offset: u32,
    pub buffer: FlatVertexBufferBuffer,
}

impl FlatVertexBuffer {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, keys: &mut Keys) {
        self.buffer.keys(keys);
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<'a, G: Gallium>(&self, held: &'a Held<G>) -> Option<wcontainers_gallium::VertexBuffer<'a, G::Resource>> {
        Some(wcontainers_gallium::VertexBuffer {
            is_user_buffer: flag(self.is_user_buffer)?,
            buffer_offset: self.buffer_offset,
            buffer: self.buffer.owned(held)?,
        })
    }
}

/// Mesa's `pipe_vertex_element` as the glue's C copies it (`struct wc_flat_pipe_vertex_element` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatVertexElement {
    pub src_offset: u16,
    pub vertex_buffer_index: u8,
    pub dual_slot: u8,
    pub src_format: u8,
    pub src_stride: u32,
    pub instance_divisor: u32,
}

impl FlatVertexElement {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::VertexElement> {
        Some(wcontainers_gallium::VertexElement {
            src_offset: self.src_offset,
            vertex_buffer_index: within(self.vertex_buffer_index, 7)?,
            dual_slot: flag(self.dual_slot)?,
            src_format: self.src_format,
            src_stride: self.src_stride,
            instance_divisor: self.instance_divisor,
        })
    }
}

/// Mesa's `pipe_viewport_state` as the glue's C copies it (`struct wc_flat_pipe_viewport_state` of flat.h).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FlatViewportState {
    pub scale: [f32; 3],
    pub translate: [f32; 3],
    pub swizzle_x: u32,
    pub swizzle_y: u32,
    pub swizzle_z: u32,
    pub swizzle_w: u32,
}

impl FlatViewportState {
    /// Lists the objects and the client memory this copy names.
    pub fn keys(&self, _keys: &mut Keys) {
    }

    /// The trait's owned structure, each object a handle `held` holds; None where a value is out of its range or names
    /// an object not held.
    pub fn owned<G: Gallium>(&self, _held: &Held<G>) -> Option<wcontainers_gallium::ViewportState> {
        Some(wcontainers_gallium::ViewportState {
            scale: self.scale,
            translate: self.translate,
            swizzle_x: within(self.swizzle_x, 8)?,
            swizzle_y: within(self.swizzle_y, 8)?,
            swizzle_z: within(self.swizzle_z, 8)?,
            swizzle_w: within(self.swizzle_w, 8)?,
        })
    }
}

/// Each flat form's index in flat.c's `wc_flat_size` and its size in Rust.
pub const SIZES: &[(u32, usize)] = &[
    (0, core::mem::size_of::<FlatBlendColor>()),
    (1, core::mem::size_of::<FlatBlendState>()),
    (2, core::mem::size_of::<FlatBlitInfo>()),
    (3, core::mem::size_of::<FlatBlitInfoDst>()),
    (4, core::mem::size_of::<FlatBlitInfoSrc>()),
    (5, core::mem::size_of::<FlatPipeBox>()),
    (6, core::mem::size_of::<FlatClipState>()),
    (7, core::mem::size_of::<FlatComputeState>()),
    (8, core::mem::size_of::<FlatConstantBuffer>()),
    (9, core::mem::size_of::<FlatDepthStencilAlphaState>()),
    (10, core::mem::size_of::<FlatDrawIndirectInfo>()),
    (11, core::mem::size_of::<FlatDrawInfo>()),
    (12, core::mem::size_of::<FlatDrawInfoIndex>()),
    (13, core::mem::size_of::<FlatDrawStartCountBias>()),
    (14, core::mem::size_of::<FlatFramebufferState>()),
    (15, core::mem::size_of::<FlatGridInfo>()),
    (16, core::mem::size_of::<FlatImageView>()),
    (17, core::mem::size_of::<FlatImageViewU>()),
    (18, core::mem::size_of::<FlatImageViewUBuf>()),
    (19, core::mem::size_of::<FlatImageViewUTex>()),
    (20, core::mem::size_of::<FlatPolyStipple>()),
    (21, core::mem::size_of::<FlatRasterizerState>()),
    (22, core::mem::size_of::<FlatReference>()),
    (23, core::mem::size_of::<FlatResourceTemplate>()),
    (24, core::mem::size_of::<FlatRtBlendState>()),
    (25, core::mem::size_of::<FlatSamplerState>()),
    (26, core::mem::size_of::<FlatSamplerViewTemplate>()),
    (27, core::mem::size_of::<FlatSamplerViewU>()),
    (28, core::mem::size_of::<FlatSamplerViewUBuf>()),
    (29, core::mem::size_of::<FlatSamplerViewUTex>()),
    (30, core::mem::size_of::<FlatScissorState>()),
    (31, core::mem::size_of::<FlatShaderBuffer>()),
    (32, core::mem::size_of::<FlatShaderState>()),
    (33, core::mem::size_of::<FlatStencilRef>()),
    (34, core::mem::size_of::<FlatStencilState>()),
    (35, core::mem::size_of::<FlatStreamOutput>()),
    (36, core::mem::size_of::<FlatStreamOutputInfo>()),
    (37, core::mem::size_of::<FlatSurface>()),
    (38, core::mem::size_of::<FlatTex2dFromBuf>()),
    (39, core::mem::size_of::<FlatVertexBuffer>()),
    (40, core::mem::size_of::<FlatVertexBufferBuffer>()),
    (41, core::mem::size_of::<FlatVertexElement>()),
    (42, core::mem::size_of::<FlatViewportState>()),
];

/// Every field of every flat form, in flat.c's `wc_flat_layout` order: its Mesa structure, its name, its offset and size
/// in the Rust mirror, its width if Mesa declares it a bit-field, and whether its size in Mesa is its size here (a
/// number, a flag, an object, client memory or a color; not a structure, flattened, nor a bit-field), or at least its
/// size there (an enumeration, which Mesa may pack).
pub const FIELDS: &[FieldLayout] = &[
    FieldLayout { structure: "pipe_blend_color", field: "color", flat_offset: core::mem::offset_of!(FlatBlendColor, color), flat_size: core::mem::size_of::<[f32; 4]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "independent_blend_enable", flat_offset: core::mem::offset_of!(FlatBlendState, independent_blend_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "logicop_enable", flat_offset: core::mem::offset_of!(FlatBlendState, logicop_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "logicop_func", flat_offset: core::mem::offset_of!(FlatBlendState, logicop_func), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "dither", flat_offset: core::mem::offset_of!(FlatBlendState, dither), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "alpha_to_coverage", flat_offset: core::mem::offset_of!(FlatBlendState, alpha_to_coverage), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "alpha_to_coverage_dither", flat_offset: core::mem::offset_of!(FlatBlendState, alpha_to_coverage_dither), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "alpha_to_one", flat_offset: core::mem::offset_of!(FlatBlendState, alpha_to_one), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "max_rt", flat_offset: core::mem::offset_of!(FlatBlendState, max_rt), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "advanced_blend_func", flat_offset: core::mem::offset_of!(FlatBlendState, advanced_blend_func), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "blend_coherent", flat_offset: core::mem::offset_of!(FlatBlendState, blend_coherent), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "is_yuv", flat_offset: core::mem::offset_of!(FlatBlendState, is_yuv), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blend_state", field: "rt", flat_offset: core::mem::offset_of!(FlatBlendState, rt), flat_size: core::mem::size_of::<[FlatRtBlendState; 8]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "dst", flat_offset: core::mem::offset_of!(FlatBlitInfo, dst), flat_size: core::mem::size_of::<FlatBlitInfoDst>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "src", flat_offset: core::mem::offset_of!(FlatBlitInfo, src), flat_size: core::mem::size_of::<FlatBlitInfoSrc>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "mask", flat_offset: core::mem::offset_of!(FlatBlitInfo, mask), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "filter", flat_offset: core::mem::offset_of!(FlatBlitInfo, filter), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "dst_sample", flat_offset: core::mem::offset_of!(FlatBlitInfo, dst_sample), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "sample0_only", flat_offset: core::mem::offset_of!(FlatBlitInfo, sample0_only), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "scissor_enable", flat_offset: core::mem::offset_of!(FlatBlitInfo, scissor_enable), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "scissor", flat_offset: core::mem::offset_of!(FlatBlitInfo, scissor), flat_size: core::mem::size_of::<FlatScissorState>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "swizzle_enable", flat_offset: core::mem::offset_of!(FlatBlitInfo, swizzle_enable), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "swizzle", flat_offset: core::mem::offset_of!(FlatBlitInfo, swizzle), flat_size: core::mem::size_of::<[u8; 4]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "window_rectangle_include", flat_offset: core::mem::offset_of!(FlatBlitInfo, window_rectangle_include), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "num_window_rectangles", flat_offset: core::mem::offset_of!(FlatBlitInfo, num_window_rectangles), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "window_rectangles", flat_offset: core::mem::offset_of!(FlatBlitInfo, window_rectangles), flat_size: core::mem::size_of::<[FlatScissorState; 8]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "render_condition_enable", flat_offset: core::mem::offset_of!(FlatBlitInfo, render_condition_enable), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info", field: "alpha_blend", flat_offset: core::mem::offset_of!(FlatBlitInfo, alpha_blend), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.dst", field: "resource", flat_offset: core::mem::offset_of!(FlatBlitInfoDst, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.dst", field: "level", flat_offset: core::mem::offset_of!(FlatBlitInfoDst, level), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.dst", field: "box", flat_offset: core::mem::offset_of!(FlatBlitInfoDst, box_), flat_size: core::mem::size_of::<FlatPipeBox>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info.dst", field: "format", flat_offset: core::mem::offset_of!(FlatBlitInfoDst, format), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_blit_info.src", field: "resource", flat_offset: core::mem::offset_of!(FlatBlitInfoSrc, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.src", field: "level", flat_offset: core::mem::offset_of!(FlatBlitInfoSrc, level), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_blit_info.src", field: "box", flat_offset: core::mem::offset_of!(FlatBlitInfoSrc, box_), flat_size: core::mem::size_of::<FlatPipeBox>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_blit_info.src", field: "format", flat_offset: core::mem::offset_of!(FlatBlitInfoSrc, format), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_box", field: "x", flat_offset: core::mem::offset_of!(FlatPipeBox, x), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "width", flat_offset: core::mem::offset_of!(FlatPipeBox, width), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "y", flat_offset: core::mem::offset_of!(FlatPipeBox, y), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "height", flat_offset: core::mem::offset_of!(FlatPipeBox, height), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "z", flat_offset: core::mem::offset_of!(FlatPipeBox, z), flat_size: core::mem::size_of::<i16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_box", field: "depth", flat_offset: core::mem::offset_of!(FlatPipeBox, depth), flat_size: core::mem::size_of::<i16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_clip_state", field: "ucp", flat_offset: core::mem::offset_of!(FlatClipState, ucp), flat_size: core::mem::size_of::<[[f32; 4]; 8]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_compute_state", field: "static_shared_mem", flat_offset: core::mem::offset_of!(FlatComputeState, static_shared_mem), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_constant_buffer", field: "buffer", flat_offset: core::mem::offset_of!(FlatConstantBuffer, buffer), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_constant_buffer", field: "buffer_offset", flat_offset: core::mem::offset_of!(FlatConstantBuffer, buffer_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_constant_buffer", field: "buffer_size", flat_offset: core::mem::offset_of!(FlatConstantBuffer, buffer_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_constant_buffer", field: "user_buffer", flat_offset: core::mem::offset_of!(FlatConstantBuffer, user_buffer), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "stencil", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, stencil), flat_size: core::mem::size_of::<[FlatStencilState; 2]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "alpha_enabled", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, alpha_enabled), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "alpha_func", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, alpha_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_enabled", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_enabled), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_writemask", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_writemask), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_func", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_bounds_test", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_bounds_test), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "alpha_ref_value", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, alpha_ref_value), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_bounds_min", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_bounds_min), flat_size: core::mem::size_of::<f64>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_depth_stencil_alpha_state", field: "depth_bounds_max", flat_offset: core::mem::offset_of!(FlatDepthStencilAlphaState, depth_bounds_max), flat_size: core::mem::size_of::<f64>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "offset", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "stride", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, stride), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "draw_count", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, draw_count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "indirect_draw_count_offset", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, indirect_draw_count_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "buffer", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, buffer), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "indirect_draw_count", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, indirect_draw_count), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_indirect_info", field: "count_from_stream_output", flat_offset: core::mem::offset_of!(FlatDrawIndirectInfo, count_from_stream_output), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "mode", flat_offset: core::mem::offset_of!(FlatDrawInfo, mode), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_draw_info", field: "index_size", flat_offset: core::mem::offset_of!(FlatDrawInfo, index_size), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "primitive_restart", flat_offset: core::mem::offset_of!(FlatDrawInfo, primitive_restart), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "has_user_indices", flat_offset: core::mem::offset_of!(FlatDrawInfo, has_user_indices), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "index_bounds_valid", flat_offset: core::mem::offset_of!(FlatDrawInfo, index_bounds_valid), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "increment_draw_id", flat_offset: core::mem::offset_of!(FlatDrawInfo, increment_draw_id), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "index_bias_varies", flat_offset: core::mem::offset_of!(FlatDrawInfo, index_bias_varies), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "was_line_loop", flat_offset: core::mem::offset_of!(FlatDrawInfo, was_line_loop), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "_pad", flat_offset: core::mem::offset_of!(FlatDrawInfo, _pad), flat_size: core::mem::size_of::<u8>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "start_instance", flat_offset: core::mem::offset_of!(FlatDrawInfo, start_instance), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "instance_count", flat_offset: core::mem::offset_of!(FlatDrawInfo, instance_count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "restart_index", flat_offset: core::mem::offset_of!(FlatDrawInfo, restart_index), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "index", flat_offset: core::mem::offset_of!(FlatDrawInfo, index), flat_size: core::mem::size_of::<FlatDrawInfoIndex>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "min_index", flat_offset: core::mem::offset_of!(FlatDrawInfo, min_index), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info", field: "max_index", flat_offset: core::mem::offset_of!(FlatDrawInfo, max_index), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info.index", field: "resource", flat_offset: core::mem::offset_of!(FlatDrawInfoIndex, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_info.index", field: "user", flat_offset: core::mem::offset_of!(FlatDrawInfoIndex, user), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_start_count_bias", field: "start", flat_offset: core::mem::offset_of!(FlatDrawStartCountBias, start), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_start_count_bias", field: "count", flat_offset: core::mem::offset_of!(FlatDrawStartCountBias, count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_draw_start_count_bias", field: "index_bias", flat_offset: core::mem::offset_of!(FlatDrawStartCountBias, index_bias), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "width", flat_offset: core::mem::offset_of!(FlatFramebufferState, width), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "height", flat_offset: core::mem::offset_of!(FlatFramebufferState, height), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "layers", flat_offset: core::mem::offset_of!(FlatFramebufferState, layers), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "samples", flat_offset: core::mem::offset_of!(FlatFramebufferState, samples), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "nr_cbufs", flat_offset: core::mem::offset_of!(FlatFramebufferState, nr_cbufs), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "pls_enabled", flat_offset: core::mem::offset_of!(FlatFramebufferState, pls_enabled), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "downscale_cbufs", flat_offset: core::mem::offset_of!(FlatFramebufferState, downscale_cbufs), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "_pad", flat_offset: core::mem::offset_of!(FlatFramebufferState, _pad), flat_size: core::mem::size_of::<u8>(), bits: 6, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "viewmask", flat_offset: core::mem::offset_of!(FlatFramebufferState, viewmask), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "cbufs", flat_offset: core::mem::offset_of!(FlatFramebufferState, cbufs), flat_size: core::mem::size_of::<[FlatSurface; 8]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "zsbuf", flat_offset: core::mem::offset_of!(FlatFramebufferState, zsbuf), flat_size: core::mem::size_of::<FlatSurface>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_framebuffer_state", field: "resolve", flat_offset: core::mem::offset_of!(FlatFramebufferState, resolve), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "variable_shared_mem", flat_offset: core::mem::offset_of!(FlatGridInfo, variable_shared_mem), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "work_dim", flat_offset: core::mem::offset_of!(FlatGridInfo, work_dim), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "block", flat_offset: core::mem::offset_of!(FlatGridInfo, block), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "last_block", flat_offset: core::mem::offset_of!(FlatGridInfo, last_block), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "grid", flat_offset: core::mem::offset_of!(FlatGridInfo, grid), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "grid_base", flat_offset: core::mem::offset_of!(FlatGridInfo, grid_base), flat_size: core::mem::size_of::<[u32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect_offset", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect_stride", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect_stride), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "draw_count", flat_offset: core::mem::offset_of!(FlatGridInfo, draw_count), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect_draw_count_offset", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect_draw_count_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_grid_info", field: "indirect_draw_count", flat_offset: core::mem::offset_of!(FlatGridInfo, indirect_draw_count), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view", field: "resource", flat_offset: core::mem::offset_of!(FlatImageView, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view", field: "format", flat_offset: core::mem::offset_of!(FlatImageView, format), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_image_view", field: "access", flat_offset: core::mem::offset_of!(FlatImageView, access), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view", field: "shader_access", flat_offset: core::mem::offset_of!(FlatImageView, shader_access), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view", field: "u", flat_offset: core::mem::offset_of!(FlatImageView, u), flat_size: core::mem::size_of::<FlatImageViewU>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u", field: "tex", flat_offset: core::mem::offset_of!(FlatImageViewU, tex), flat_size: core::mem::size_of::<FlatImageViewUTex>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u", field: "buf", flat_offset: core::mem::offset_of!(FlatImageViewU, buf), flat_size: core::mem::size_of::<FlatImageViewUBuf>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u", field: "tex2d_from_buf", flat_offset: core::mem::offset_of!(FlatImageViewU, tex2d_from_buf), flat_size: core::mem::size_of::<FlatTex2dFromBuf>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.buf", field: "offset", flat_offset: core::mem::offset_of!(FlatImageViewUBuf, offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.buf", field: "size", flat_offset: core::mem::offset_of!(FlatImageViewUBuf, size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "first_layer", flat_offset: core::mem::offset_of!(FlatImageViewUTex, first_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "last_layer", flat_offset: core::mem::offset_of!(FlatImageViewUTex, last_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "level", flat_offset: core::mem::offset_of!(FlatImageViewUTex, level), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "single_layer_view", flat_offset: core::mem::offset_of!(FlatImageViewUTex, single_layer_view), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_image_view.u.tex", field: "is_2d_view_of_3d", flat_offset: core::mem::offset_of!(FlatImageViewUTex, is_2d_view_of_3d), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_poly_stipple", field: "stipple", flat_offset: core::mem::offset_of!(FlatPolyStipple, stipple), flat_size: core::mem::size_of::<[u32; 32]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "flatshade", flat_offset: core::mem::offset_of!(FlatRasterizerState, flatshade), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "light_twoside", flat_offset: core::mem::offset_of!(FlatRasterizerState, light_twoside), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "clamp_vertex_color", flat_offset: core::mem::offset_of!(FlatRasterizerState, clamp_vertex_color), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "clamp_fragment_color", flat_offset: core::mem::offset_of!(FlatRasterizerState, clamp_fragment_color), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "front_ccw", flat_offset: core::mem::offset_of!(FlatRasterizerState, front_ccw), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "cull_face", flat_offset: core::mem::offset_of!(FlatRasterizerState, cull_face), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "fill_front", flat_offset: core::mem::offset_of!(FlatRasterizerState, fill_front), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "fill_back", flat_offset: core::mem::offset_of!(FlatRasterizerState, fill_back), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_point", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_point), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_line", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_line), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_tri", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_tri), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "scissor", flat_offset: core::mem::offset_of!(FlatRasterizerState, scissor), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "poly_smooth", flat_offset: core::mem::offset_of!(FlatRasterizerState, poly_smooth), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "poly_stipple_enable", flat_offset: core::mem::offset_of!(FlatRasterizerState, poly_stipple_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_smooth", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_smooth), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "sprite_coord_mode", flat_offset: core::mem::offset_of!(FlatRasterizerState, sprite_coord_mode), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_quad_rasterization", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_quad_rasterization), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_line_tri_clip", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_line_tri_clip), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_size_per_vertex", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_size_per_vertex), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "multisample", flat_offset: core::mem::offset_of!(FlatRasterizerState, multisample), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "no_ms_sample_mask_out", flat_offset: core::mem::offset_of!(FlatRasterizerState, no_ms_sample_mask_out), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "force_persample_interp", flat_offset: core::mem::offset_of!(FlatRasterizerState, force_persample_interp), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_smooth", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_smooth), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_stipple_enable", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_stipple_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_last_pixel", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_last_pixel), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_rectangular", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_rectangular), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "conservative_raster_mode", flat_offset: core::mem::offset_of!(FlatRasterizerState, conservative_raster_mode), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "flatshade_first", flat_offset: core::mem::offset_of!(FlatRasterizerState, flatshade_first), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "half_pixel_center", flat_offset: core::mem::offset_of!(FlatRasterizerState, half_pixel_center), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "bottom_edge_rule", flat_offset: core::mem::offset_of!(FlatRasterizerState, bottom_edge_rule), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "subpixel_precision_x", flat_offset: core::mem::offset_of!(FlatRasterizerState, subpixel_precision_x), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "subpixel_precision_y", flat_offset: core::mem::offset_of!(FlatRasterizerState, subpixel_precision_y), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "rasterizer_discard", flat_offset: core::mem::offset_of!(FlatRasterizerState, rasterizer_discard), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "rasterization_stream", flat_offset: core::mem::offset_of!(FlatRasterizerState, rasterization_stream), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "tile_raster_order_fixed", flat_offset: core::mem::offset_of!(FlatRasterizerState, tile_raster_order_fixed), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "tile_raster_order_increasing_x", flat_offset: core::mem::offset_of!(FlatRasterizerState, tile_raster_order_increasing_x), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "tile_raster_order_increasing_y", flat_offset: core::mem::offset_of!(FlatRasterizerState, tile_raster_order_increasing_y), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "depth_clip_near", flat_offset: core::mem::offset_of!(FlatRasterizerState, depth_clip_near), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "depth_clip_far", flat_offset: core::mem::offset_of!(FlatRasterizerState, depth_clip_far), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "depth_clamp", flat_offset: core::mem::offset_of!(FlatRasterizerState, depth_clamp), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "clip_halfz", flat_offset: core::mem::offset_of!(FlatRasterizerState, clip_halfz), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_units_unscaled", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_units_unscaled), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "unclamped_fragment_depth_values", flat_offset: core::mem::offset_of!(FlatRasterizerState, unclamped_fragment_depth_values), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "clip_plane_enable", flat_offset: core::mem::offset_of!(FlatRasterizerState, clip_plane_enable), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "representative_fragment_test", flat_offset: core::mem::offset_of!(FlatRasterizerState, representative_fragment_test), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_stipple_factor", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_stipple_factor), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_stipple_pattern", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_stipple_pattern), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "sprite_coord_enable", flat_offset: core::mem::offset_of!(FlatRasterizerState, sprite_coord_enable), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "line_width", flat_offset: core::mem::offset_of!(FlatRasterizerState, line_width), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "point_size", flat_offset: core::mem::offset_of!(FlatRasterizerState, point_size), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_units", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_units), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_scale", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_scale), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "offset_clamp", flat_offset: core::mem::offset_of!(FlatRasterizerState, offset_clamp), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rasterizer_state", field: "conservative_raster_dilate", flat_offset: core::mem::offset_of!(FlatRasterizerState, conservative_raster_dilate), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_reference", field: "count", flat_offset: core::mem::offset_of!(FlatReference, count), flat_size: core::mem::size_of::<i32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "width0", flat_offset: core::mem::offset_of!(FlatResourceTemplate, width0), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "height0", flat_offset: core::mem::offset_of!(FlatResourceTemplate, height0), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "depth0", flat_offset: core::mem::offset_of!(FlatResourceTemplate, depth0), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "array_size", flat_offset: core::mem::offset_of!(FlatResourceTemplate, array_size), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "format", flat_offset: core::mem::offset_of!(FlatResourceTemplate, format), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_resource", field: "target", flat_offset: core::mem::offset_of!(FlatResourceTemplate, target), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_resource", field: "last_level", flat_offset: core::mem::offset_of!(FlatResourceTemplate, last_level), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "nr_samples", flat_offset: core::mem::offset_of!(FlatResourceTemplate, nr_samples), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "nr_storage_samples", flat_offset: core::mem::offset_of!(FlatResourceTemplate, nr_storage_samples), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "nr_sparse_levels", flat_offset: core::mem::offset_of!(FlatResourceTemplate, nr_sparse_levels), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "compression_rate", flat_offset: core::mem::offset_of!(FlatResourceTemplate, compression_rate), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "usage", flat_offset: core::mem::offset_of!(FlatResourceTemplate, usage), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_resource", field: "bind", flat_offset: core::mem::offset_of!(FlatResourceTemplate, bind), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_resource", field: "flags", flat_offset: core::mem::offset_of!(FlatResourceTemplate, flags), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "blend_enable", flat_offset: core::mem::offset_of!(FlatRtBlendState, blend_enable), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "rgb_func", flat_offset: core::mem::offset_of!(FlatRtBlendState, rgb_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "rgb_src_factor", flat_offset: core::mem::offset_of!(FlatRtBlendState, rgb_src_factor), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "rgb_dst_factor", flat_offset: core::mem::offset_of!(FlatRtBlendState, rgb_dst_factor), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "alpha_func", flat_offset: core::mem::offset_of!(FlatRtBlendState, alpha_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "alpha_src_factor", flat_offset: core::mem::offset_of!(FlatRtBlendState, alpha_src_factor), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "alpha_dst_factor", flat_offset: core::mem::offset_of!(FlatRtBlendState, alpha_dst_factor), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_rt_blend_state", field: "colormask", flat_offset: core::mem::offset_of!(FlatRtBlendState, colormask), flat_size: core::mem::size_of::<u32>(), bits: 4, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "wrap_s", flat_offset: core::mem::offset_of!(FlatSamplerState, wrap_s), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "wrap_t", flat_offset: core::mem::offset_of!(FlatSamplerState, wrap_t), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "wrap_r", flat_offset: core::mem::offset_of!(FlatSamplerState, wrap_r), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "min_img_filter", flat_offset: core::mem::offset_of!(FlatSamplerState, min_img_filter), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "min_mip_filter", flat_offset: core::mem::offset_of!(FlatSamplerState, min_mip_filter), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "mag_img_filter", flat_offset: core::mem::offset_of!(FlatSamplerState, mag_img_filter), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "compare_mode", flat_offset: core::mem::offset_of!(FlatSamplerState, compare_mode), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "compare_func", flat_offset: core::mem::offset_of!(FlatSamplerState, compare_func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "unnormalized_coords", flat_offset: core::mem::offset_of!(FlatSamplerState, unnormalized_coords), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "max_anisotropy", flat_offset: core::mem::offset_of!(FlatSamplerState, max_anisotropy), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "seamless_cube_map", flat_offset: core::mem::offset_of!(FlatSamplerState, seamless_cube_map), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "border_color_is_integer", flat_offset: core::mem::offset_of!(FlatSamplerState, border_color_is_integer), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "reduction_mode", flat_offset: core::mem::offset_of!(FlatSamplerState, reduction_mode), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "pad", flat_offset: core::mem::offset_of!(FlatSamplerState, pad), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "lod_bias", flat_offset: core::mem::offset_of!(FlatSamplerState, lod_bias), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "min_lod", flat_offset: core::mem::offset_of!(FlatSamplerState, min_lod), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "max_lod", flat_offset: core::mem::offset_of!(FlatSamplerState, max_lod), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "border_color", flat_offset: core::mem::offset_of!(FlatSamplerState, border_color), flat_size: core::mem::size_of::<[u32; 4]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_state", field: "border_color_format", flat_offset: core::mem::offset_of!(FlatSamplerState, border_color_format), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: true },
    FieldLayout { structure: "pipe_sampler_view", field: "format", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, format), flat_size: core::mem::size_of::<u32>(), bits: 12, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_sampler_view", field: "astc_decode_format", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, astc_decode_format), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "is_tex2d_from_buf", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, is_tex2d_from_buf), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "target", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, target), flat_size: core::mem::size_of::<u32>(), bits: 5, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_sampler_view", field: "swizzle_r", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, swizzle_r), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "swizzle_g", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, swizzle_g), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "swizzle_b", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, swizzle_b), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "swizzle_a", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, swizzle_a), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view", field: "u", flat_offset: core::mem::offset_of!(FlatSamplerViewTemplate, u), flat_size: core::mem::size_of::<FlatSamplerViewU>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u", field: "tex", flat_offset: core::mem::offset_of!(FlatSamplerViewU, tex), flat_size: core::mem::size_of::<FlatSamplerViewUTex>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u", field: "buf", flat_offset: core::mem::offset_of!(FlatSamplerViewU, buf), flat_size: core::mem::size_of::<FlatSamplerViewUBuf>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u", field: "tex2d_from_buf", flat_offset: core::mem::offset_of!(FlatSamplerViewU, tex2d_from_buf), flat_size: core::mem::size_of::<FlatTex2dFromBuf>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.buf", field: "offset", flat_offset: core::mem::offset_of!(FlatSamplerViewUBuf, offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.buf", field: "size", flat_offset: core::mem::offset_of!(FlatSamplerViewUBuf, size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "first_layer", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, first_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "last_layer", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, last_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "first_level", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, first_level), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "last_level", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, last_level), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_sampler_view.u.tex", field: "min_lod_clamp", flat_offset: core::mem::offset_of!(FlatSamplerViewUTex, min_lod_clamp), flat_size: core::mem::size_of::<f32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_scissor_state", field: "minx", flat_offset: core::mem::offset_of!(FlatScissorState, minx), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_scissor_state", field: "miny", flat_offset: core::mem::offset_of!(FlatScissorState, miny), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_scissor_state", field: "maxx", flat_offset: core::mem::offset_of!(FlatScissorState, maxx), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_scissor_state", field: "maxy", flat_offset: core::mem::offset_of!(FlatScissorState, maxy), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_buffer", field: "buffer", flat_offset: core::mem::offset_of!(FlatShaderBuffer, buffer), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_buffer", field: "buffer_offset", flat_offset: core::mem::offset_of!(FlatShaderBuffer, buffer_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_buffer", field: "buffer_size", flat_offset: core::mem::offset_of!(FlatShaderBuffer, buffer_size), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_shader_state", field: "stream_output", flat_offset: core::mem::offset_of!(FlatShaderState, stream_output), flat_size: core::mem::size_of::<FlatStreamOutputInfo>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_shader_state", field: "report_compile_error", flat_offset: core::mem::offset_of!(FlatShaderState, report_compile_error), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_stencil_ref", field: "ref_value", flat_offset: core::mem::offset_of!(FlatStencilRef, ref_value), flat_size: core::mem::size_of::<[u8; 2]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "enabled", flat_offset: core::mem::offset_of!(FlatStencilState, enabled), flat_size: core::mem::size_of::<u32>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "func", flat_offset: core::mem::offset_of!(FlatStencilState, func), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "fail_op", flat_offset: core::mem::offset_of!(FlatStencilState, fail_op), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "zpass_op", flat_offset: core::mem::offset_of!(FlatStencilState, zpass_op), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "zfail_op", flat_offset: core::mem::offset_of!(FlatStencilState, zfail_op), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "valuemask", flat_offset: core::mem::offset_of!(FlatStencilState, valuemask), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stencil_state", field: "writemask", flat_offset: core::mem::offset_of!(FlatStencilState, writemask), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "register_index", flat_offset: core::mem::offset_of!(FlatStreamOutput, register_index), flat_size: core::mem::size_of::<u32>(), bits: 6, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "start_component", flat_offset: core::mem::offset_of!(FlatStreamOutput, start_component), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "num_components", flat_offset: core::mem::offset_of!(FlatStreamOutput, num_components), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "output_buffer", flat_offset: core::mem::offset_of!(FlatStreamOutput, output_buffer), flat_size: core::mem::size_of::<u32>(), bits: 3, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "dst_offset", flat_offset: core::mem::offset_of!(FlatStreamOutput, dst_offset), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output", field: "stream", flat_offset: core::mem::offset_of!(FlatStreamOutput, stream), flat_size: core::mem::size_of::<u32>(), bits: 2, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_stream_output_info", field: "num_outputs", flat_offset: core::mem::offset_of!(FlatStreamOutputInfo, num_outputs), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_stream_output_info", field: "stride", flat_offset: core::mem::offset_of!(FlatStreamOutputInfo, stride), flat_size: core::mem::size_of::<[u16; 4]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_stream_output_info", field: "output", flat_offset: core::mem::offset_of!(FlatStreamOutputInfo, output), flat_size: core::mem::size_of::<[FlatStreamOutput; 128]>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "reference", flat_offset: core::mem::offset_of!(FlatSurface, reference), flat_size: core::mem::size_of::<FlatReference>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "format", flat_offset: core::mem::offset_of!(FlatSurface, format), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_surface", field: "nr_samples", flat_offset: core::mem::offset_of!(FlatSurface, nr_samples), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "first_layer", flat_offset: core::mem::offset_of!(FlatSurface, first_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "last_layer", flat_offset: core::mem::offset_of!(FlatSurface, last_layer), flat_size: core::mem::size_of::<u32>(), bits: 16, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "level", flat_offset: core::mem::offset_of!(FlatSurface, level), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_surface", field: "texture", flat_offset: core::mem::offset_of!(FlatSurface, texture), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_tex2d_from_buf", field: "offset", flat_offset: core::mem::offset_of!(FlatTex2dFromBuf, offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_tex2d_from_buf", field: "row_stride", flat_offset: core::mem::offset_of!(FlatTex2dFromBuf, row_stride), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_tex2d_from_buf", field: "width", flat_offset: core::mem::offset_of!(FlatTex2dFromBuf, width), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_tex2d_from_buf", field: "height", flat_offset: core::mem::offset_of!(FlatTex2dFromBuf, height), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_buffer", field: "is_user_buffer", flat_offset: core::mem::offset_of!(FlatVertexBuffer, is_user_buffer), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_buffer", field: "buffer_offset", flat_offset: core::mem::offset_of!(FlatVertexBuffer, buffer_offset), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_buffer", field: "buffer", flat_offset: core::mem::offset_of!(FlatVertexBuffer, buffer), flat_size: core::mem::size_of::<FlatVertexBufferBuffer>(), bits: 0, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_vertex_buffer.buffer", field: "resource", flat_offset: core::mem::offset_of!(FlatVertexBufferBuffer, resource), flat_size: core::mem::size_of::<usize>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "src_offset", flat_offset: core::mem::offset_of!(FlatVertexElement, src_offset), flat_size: core::mem::size_of::<u16>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "vertex_buffer_index", flat_offset: core::mem::offset_of!(FlatVertexElement, vertex_buffer_index), flat_size: core::mem::size_of::<u8>(), bits: 7, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "dual_slot", flat_offset: core::mem::offset_of!(FlatVertexElement, dual_slot), flat_size: core::mem::size_of::<u8>(), bits: 1, same_size: false, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "src_format", flat_offset: core::mem::offset_of!(FlatVertexElement, src_format), flat_size: core::mem::size_of::<u8>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "src_stride", flat_offset: core::mem::offset_of!(FlatVertexElement, src_stride), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_vertex_element", field: "instance_divisor", flat_offset: core::mem::offset_of!(FlatVertexElement, instance_divisor), flat_size: core::mem::size_of::<u32>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_viewport_state", field: "scale", flat_offset: core::mem::offset_of!(FlatViewportState, scale), flat_size: core::mem::size_of::<[f32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_viewport_state", field: "translate", flat_offset: core::mem::offset_of!(FlatViewportState, translate), flat_size: core::mem::size_of::<[f32; 3]>(), bits: 0, same_size: true, at_least: false },
    FieldLayout { structure: "pipe_viewport_state", field: "swizzle_x", flat_offset: core::mem::offset_of!(FlatViewportState, swizzle_x), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_viewport_state", field: "swizzle_y", flat_offset: core::mem::offset_of!(FlatViewportState, swizzle_y), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_viewport_state", field: "swizzle_z", flat_offset: core::mem::offset_of!(FlatViewportState, swizzle_z), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
    FieldLayout { structure: "pipe_viewport_state", field: "swizzle_w", flat_offset: core::mem::offset_of!(FlatViewportState, swizzle_w), flat_size: core::mem::size_of::<u32>(), bits: 8, same_size: false, at_least: true },
];

/// The fields of Mesa's structures the flat forms do not carry (`FIELDS_LEFT_OUT` of the generator, each with its reason
/// in interface.rs): with FIELDS, every field of each structure.
pub const LEFT_OUT: &[(&str, &str)] = &[
    ("pipe_compute_state", "ir_type"),
    ("pipe_compute_state", "prog"),
    ("pipe_grid_info", "num_globals"),
    ("pipe_grid_info", "globals"),
    ("pipe_resource", "__cl_space"),
    ("pipe_resource", "reference"),
    ("pipe_resource", "clientptr_heap"),
    ("pipe_resource", "next"),
    ("pipe_resource", "screen"),
    ("pipe_sampler_view", "reference"),
    ("pipe_sampler_view", "texture"),
    ("pipe_sampler_view", "context"),
    ("pipe_shader_state", "type"),
    ("pipe_shader_state", "tokens"),
    ("pipe_shader_state", "ir"),
    ("pipe_shader_state", "error_message"),
    ("pipe_vertex_buffer.buffer", "user"),
];
