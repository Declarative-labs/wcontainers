//! The immediate context.

use std::cell::Cell;
use std::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Foundation::{E_INVALIDARG, RECT};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::core::{IUnknownImpl, Interface, Ref, Result, implement};

use wcontainers_direct3d_11::desc::{Box3, Viewport};
use wcontainers_direct3d_11::object::Resource;
use wcontainers_direct3d_11::{PIXEL, VERTEX, format};

use super::*;

#[implement(ID3D11DeviceContext)]
pub struct Context {
    front: Shared,
    pub(super) child: Child,
    /// The device's pointer to this context, cleared when the context goes.
    slot: Rc<Cell<*mut c_void>>,
}

impl Context {
    pub(super) fn new(front: Shared, device: ID3D11Device, slot: Rc<Cell<*mut c_void>>) -> Self {
        Context {
            front,
            child: Child::new(device),
            slot,
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        self.slot.set(std::ptr::null_mut());
    }
}

/// The items of a COM array argument: `count` pointers from `items`, or none if it is null.
fn items<'a, T>(items: *const Option<T>, count: u32) -> &'a [Option<T>] {
    if items.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(items, count as usize) }
    }
}

/// The front objects behind `count` COM objects of type `O`, a foreign object counting as none.
fn fronts<I: Interface, O, F: Clone>(
    list: *const Option<I>,
    count: u32,
    get: impl Fn(&O) -> F,
) -> Vec<Option<F>>
where
    O: windows::core::ComObjectInner,
    O::Outer: std::any::Any + windows::core::IUnknownImpl<Impl = O>,
{
    items(list, count)
        .iter()
        .map(|i| {
            i.as_ref()
                .and_then(|i| i.cast_object_ref::<O>().ok())
                .map(|o| get(o.get_impl()))
        })
        .collect()
}

fn front_of<I: Interface, O, F>(object: &Ref<I>, get: impl Fn(&O) -> F) -> Option<F>
where
    O: windows::core::ComObjectInner,
    O::Outer: std::any::Any + windows::core::IUnknownImpl<Impl = O>,
{
    object
        .as_ref()
        .and_then(|i| i.cast_object_ref::<O>().ok())
        .map(|o| get(o.get_impl()))
}

impl Context {
    pub(super) fn unserved(&self, what: &'static str) {
        missing_void(what);
        self.front.borrow_mut().removed = true;
    }

    /// Runs a front call whose error Direct3D cannot report, warning instead.
    fn quiet(&self, what: &'static str, r: wcontainers_direct3d_11::Result<()>) {
        if let Err(e) = r {
            eprintln!("wcontainers direct3d 11: {what}: {e:?}");
        }
    }

    pub(super) fn vs_set_shader(
        &self,
        shader: Ref<ID3D11VertexShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let s = front_of(&shader, |s: &VertexShader| s.front.clone());
        self.front.borrow_mut().vs_set_shader(s);
    }

    pub(super) fn ps_set_shader(
        &self,
        shader: Ref<ID3D11PixelShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let s = front_of(&shader, |s: &PixelShader| s.front.clone());
        self.front.borrow_mut().ps_set_shader(s);
    }

    fn set_constant_buffers(
        &self,
        stage: usize,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        let b = fronts(list, n, |b: &Buffer| b.front.clone());
        let r = self
            .front
            .borrow_mut()
            .set_constant_buffers(stage, start, &b);
        self.quiet("SetConstantBuffers", r);
    }

    pub(super) fn vs_set_constant_buffers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        self.set_constant_buffers(VERTEX, start, n, list);
    }

    pub(super) fn ps_set_constant_buffers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        self.set_constant_buffers(PIXEL, start, n, list);
    }

    fn set_shader_resources(
        &self,
        stage: usize,
        start: u32,
        n: u32,
        list: *const Option<ID3D11ShaderResourceView>,
    ) {
        let v = fronts(list, n, |v: &ShaderResourceView| v.front.clone());
        let r = self
            .front
            .borrow_mut()
            .set_shader_resources(stage, start, &v);
        self.quiet("SetShaderResources", r);
    }

    pub(super) fn vs_set_shader_resources(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.set_shader_resources(VERTEX, start, n, list);
    }

    pub(super) fn ps_set_shader_resources(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.set_shader_resources(PIXEL, start, n, list);
    }

    fn set_samplers(
        &self,
        stage: usize,
        start: u32,
        n: u32,
        list: *const Option<ID3D11SamplerState>,
    ) {
        let s = fronts(list, n, |s: &SamplerState| s.front.clone());
        let r = self.front.borrow_mut().set_samplers(stage, start, &s);
        self.quiet("SetSamplers", r);
    }

    pub(super) fn vs_set_samplers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11SamplerState>,
    ) {
        self.set_samplers(VERTEX, start, n, list);
    }

    pub(super) fn ps_set_samplers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11SamplerState>,
    ) {
        self.set_samplers(PIXEL, start, n, list);
    }

    pub(super) fn ia_set_input_layout(&self, layout: Ref<ID3D11InputLayout>) {
        let l = front_of(&layout, |l: &InputLayout| l.front.clone());
        self.front.borrow_mut().ia_set_input_layout(l);
    }

    pub(super) fn ia_set_primitive_topology(&self, topology: D3D_PRIMITIVE_TOPOLOGY) {
        self.front
            .borrow_mut()
            .ia_set_primitive_topology(topology.0 as u32);
    }

    pub(super) fn ia_set_vertex_buffers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
        strides: *const u32,
        offsets: *const u32,
    ) {
        if list.is_null() || strides.is_null() || offsets.is_null() {
            return;
        }
        let buffers = fronts(list, n, |b: &Buffer| b.front.clone());
        let strides = unsafe { std::slice::from_raw_parts(strides, n as usize) };
        let offsets = unsafe { std::slice::from_raw_parts(offsets, n as usize) };
        let slots: Vec<_> = buffers
            .into_iter()
            .zip(strides.iter().zip(offsets))
            .map(|(b, (&s, &o))| b.map(|b| (b, s, o)))
            .collect();
        let r = self.front.borrow_mut().ia_set_vertex_buffers(start, &slots);
        self.quiet("IASetVertexBuffers", r);
    }

    pub(super) fn ia_set_index_buffer(
        &self,
        buffer: Ref<ID3D11Buffer>,
        format: DXGI_FORMAT,
        offset: u32,
    ) {
        let b = front_of(&buffer, |b: &Buffer| b.front.clone());
        let r = self
            .front
            .borrow_mut()
            .ia_set_index_buffer(b, format.0 as u32, offset);
        self.quiet("IASetIndexBuffer", r);
    }

    pub(super) fn om_set_render_targets(
        &self,
        n: u32,
        list: *const Option<ID3D11RenderTargetView>,
        depth: Ref<ID3D11DepthStencilView>,
    ) {
        let targets = fronts(list, n, |v: &RenderTargetView| v.front.clone());
        let d = front_of(&depth, |v: &DepthStencilView| v.front.clone());
        let r = self.front.borrow_mut().om_set_render_targets(&targets, d);
        self.quiet("OMSetRenderTargets", r);
    }

    pub(super) fn om_set_blend_state(
        &self,
        state: Ref<ID3D11BlendState>,
        factor: *const f32,
        mask: u32,
    ) {
        let s = front_of(&state, |s: &BlendState| s.front.clone());
        let f = (!factor.is_null()).then(|| unsafe { *factor.cast::<[f32; 4]>() });
        self.front.borrow_mut().om_set_blend_state(s, f, mask);
    }

    pub(super) fn om_set_depth_stencil_state(
        &self,
        state: Ref<ID3D11DepthStencilState>,
        reference: u32,
    ) {
        let s = front_of(&state, |s: &DepthStencilState| s.front.clone());
        self.front
            .borrow_mut()
            .om_set_depth_stencil_state(s, reference);
    }

    pub(super) fn rs_set_state(&self, state: Ref<ID3D11RasterizerState>) {
        let s = front_of(&state, |s: &RasterizerState| s.front.clone());
        self.front.borrow_mut().rs_set_state(s);
    }

    pub(super) fn rs_set_viewports(&self, n: u32, viewports: *const D3D11_VIEWPORT) {
        let list = if viewports.is_null() {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(viewports, n as usize) }
        };
        let v: Vec<Viewport> = list
            .iter()
            .map(|v| Viewport {
                x: v.TopLeftX,
                y: v.TopLeftY,
                width: v.Width,
                height: v.Height,
                min_depth: v.MinDepth,
                max_depth: v.MaxDepth,
            })
            .collect();
        self.front.borrow_mut().rs_set_viewports(&v);
    }

    pub(super) fn rs_set_scissor_rects(&self, n: u32, rects: *const RECT) {
        let list = if rects.is_null() {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(rects, n as usize) }
        };
        let r: Vec<[i32; 4]> = list
            .iter()
            .map(|r| [r.left, r.top, r.right, r.bottom])
            .collect();
        self.front.borrow_mut().rs_set_scissor_rects(&r);
    }

    pub(super) fn clear_render_target_view(
        &self,
        view: Ref<ID3D11RenderTargetView>,
        color: *const f32,
    ) {
        let Some(v) = front_of(&view, |v: &RenderTargetView| v.front.clone()) else {
            return;
        };
        if color.is_null() {
            return;
        }
        let c = unsafe { *color.cast::<[f32; 4]>() };
        self.front.borrow_mut().clear_render_target_view(&v, c);
    }

    pub(super) fn clear_depth_stencil_view(
        &self,
        view: Ref<ID3D11DepthStencilView>,
        flags: u32,
        depth: f32,
        stencil: u8,
    ) {
        let Some(v) = front_of(&view, |v: &DepthStencilView| v.front.clone()) else {
            return;
        };
        self.front
            .borrow_mut()
            .clear_depth_stencil_view(&v, flags, depth, stencil);
    }

    pub(super) fn draw(&self, count: u32, start: u32) {
        self.front.borrow_mut().draw(count, start);
    }

    pub(super) fn draw_indexed(&self, count: u32, start: u32, base: i32) {
        self.front.borrow_mut().draw_indexed(count, start, base);
    }

    pub(super) fn draw_instanced(
        &self,
        count: u32,
        instances: u32,
        start: u32,
        first_instance: u32,
    ) {
        self.front
            .borrow_mut()
            .draw_instanced(count, instances, start, first_instance);
    }

    pub(super) fn draw_indexed_instanced(
        &self,
        count: u32,
        instances: u32,
        start: u32,
        base: i32,
        first_instance: u32,
    ) {
        self.front.borrow_mut().draw_indexed_instanced(
            count,
            instances,
            start,
            base,
            first_instance,
        );
    }

    pub(super) fn copy_resource(&self, dst: Ref<ID3D11Resource>, src: Ref<ID3D11Resource>) {
        let (Some(d), Some(s)) = (
            dst.as_ref().and_then(resource_of),
            src.as_ref().and_then(resource_of),
        ) else {
            return;
        };
        let r = self.front.borrow_mut().copy_resource(&d, &s);
        self.quiet("CopyResource", r);
    }

    #[allow(clippy::too_many_arguments, reason = "the COM method's own arguments")]
    pub(super) fn copy_subresource_region(
        &self,
        dst: Ref<ID3D11Resource>,
        dst_sub: u32,
        x: u32,
        y: u32,
        z: u32,
        src: Ref<ID3D11Resource>,
        src_sub: u32,
        src_box: *const D3D11_BOX,
    ) {
        let (Some(d), Some(s)) = (
            dst.as_ref().and_then(resource_of),
            src.as_ref().and_then(resource_of),
        ) else {
            return;
        };
        let b = unsafe { src_box.as_ref() }.map(box3);
        let r = self.front.borrow_mut().copy_subresource_region(
            &d,
            dst_sub,
            [x, y, z],
            &s,
            src_sub,
            b.as_ref(),
        );
        self.quiet("CopySubresourceRegion", r);
    }

    pub(super) fn update_subresource(
        &self,
        dst: Ref<ID3D11Resource>,
        sub: u32,
        dst_box: *const D3D11_BOX,
        data: *const c_void,
        row_pitch: u32,
        _depth_pitch: u32,
    ) {
        let Some(d) = dst.as_ref().and_then(resource_of) else {
            return;
        };
        if data.is_null() {
            return;
        }
        let b = unsafe { dst_box.as_ref() }.map(box3);
        let len = match &d {
            Resource::Buffer(buf) => {
                b.map_or(buf.desc.byte_width, |b| b.right.saturating_sub(b.left)) as usize
            }
            Resource::Texture2D(t) => {
                let mip = sub % t.desc.mip_levels;
                let (w, h) = t.mip_size(mip);
                let (w, h) = b.map_or((w, h), |b| {
                    (
                        b.right.saturating_sub(b.left),
                        b.bottom.saturating_sub(b.top),
                    )
                });
                let (row, rows) = format::row_bytes(t.format, w, h);
                (row_pitch * rows.saturating_sub(1) + row) as usize
            }
        };
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) };
        let r = self
            .front
            .borrow_mut()
            .update_subresource(&d, sub, b.as_ref(), bytes, row_pitch);
        self.quiet("UpdateSubresource", r);
    }

    pub(super) fn map(
        &self,
        resource: Ref<ID3D11Resource>,
        sub: u32,
        kind: D3D11_MAP,
        flags: u32,
        out: *mut D3D11_MAPPED_SUBRESOURCE,
    ) -> Result<()> {
        let r = resource
            .as_ref()
            .and_then(resource_of)
            .ok_or(E_INVALIDARG)?;
        let mapped = self
            .front
            .borrow_mut()
            .map(&r, sub, kind.0 as u32, flags)
            .map_err(hresult)?;
        if let Some(out) = unsafe { out.as_mut() } {
            *out = D3D11_MAPPED_SUBRESOURCE {
                pData: mapped.data.cast(),
                RowPitch: mapped.row_pitch,
                DepthPitch: mapped.depth_pitch,
            };
        }
        Ok(())
    }

    pub(super) fn unmap(&self, resource: Ref<ID3D11Resource>, sub: u32) {
        if let Some(r) = resource.as_ref().and_then(resource_of) {
            self.front.borrow_mut().unmap(&r, sub);
        }
    }

    pub(super) fn clear_state(&self) {
        self.front.borrow_mut().clear_state();
    }

    pub(super) fn flush(&self) {
        self.front.borrow_mut().flush();
    }

    pub(super) fn get_type(&self) -> D3D11_DEVICE_CONTEXT_TYPE {
        D3D11_DEVICE_CONTEXT_IMMEDIATE
    }

    pub(super) fn get_context_flags(&self) -> u32 {
        0
    }
}

fn box3(b: &D3D11_BOX) -> Box3 {
    Box3 {
        left: b.left,
        top: b.top,
        front: b.front,
        right: b.right,
        bottom: b.bottom,
        back: b.back,
    }
}
