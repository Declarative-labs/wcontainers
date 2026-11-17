//! The immediate context.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Foundation::{E_INVALIDARG, RECT};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::core::{
    ComObjectInner, ComObjectInterface, IUnknownImpl, Interface, OutRef, Ref, Result, implement,
};

use wcontainers_direct3d_11::desc::{Box3, Viewport};
use wcontainers_direct3d_11::object::Resource;
use wcontainers_direct3d_11::{PIXEL, VERTEX, format};

use super::counted::{Counted, Held};
use super::*;

#[implement(ID3D11DeviceContext)]
pub struct Context {
    front: Locked<Shared>,
    pub(super) child: Child,
    /// The device's pointer to this context, cleared when the context goes.
    slot: Locked<Rc<Cell<*mut c_void>>>,
    /// What is bound, as the program's objects, for the `Get` methods; holding a library reference
    /// on each, as Direct3D's context does.
    bound: RefCell<Bound>,
}

/// The objects and values bound to the context.
struct Bound {
    vs: Option<Held<VertexShader_Impl>>,
    ps: Option<Held<PixelShader_Impl>>,
    /// Per stage (vertex, pixel), per slot.
    constant_buffers: [Vec<Option<Held<Buffer_Impl>>>; 2],
    resources: [Vec<Option<Held<ShaderResourceView_Impl>>>; 2],
    samplers: [Vec<Option<Held<SamplerState_Impl>>>; 2],
    layout: Option<Held<InputLayout_Impl>>,
    /// Per slot: buffer, stride and offset.
    vertex_buffers: Vec<(Option<Held<Buffer_Impl>>, u32, u32)>,
    index: (Option<Held<Buffer_Impl>>, DXGI_FORMAT, u32),
    topology: D3D_PRIMITIVE_TOPOLOGY,
    targets: Vec<Option<Held<RenderTargetView_Impl>>>,
    depth: Option<Held<DepthStencilView_Impl>>,
    blend: (Option<Held<BlendState_Impl>>, [f32; 4], u32),
    depth_stencil: (Option<Held<DepthStencilState_Impl>>, u32),
    raster: Option<Held<RasterizerState_Impl>>,
    viewports: Vec<D3D11_VIEWPORT>,
    scissors: Vec<RECT>,
}

impl Default for Bound {
    fn default() -> Self {
        Bound {
            vs: None,
            ps: None,
            constant_buffers: Default::default(),
            resources: Default::default(),
            samplers: Default::default(),
            layout: None,
            vertex_buffers: Vec::new(),
            index: (None, DXGI_FORMAT(0), 0),
            topology: D3D_PRIMITIVE_TOPOLOGY(0),
            targets: Vec::new(),
            depth: None,
            blend: (None, [1.0; 4], u32::MAX),
            depth_stencil: (None, 0),
            raster: None,
            viewports: Vec::new(),
            scissors: Vec::new(),
        }
    }
}

/// Stores `items` into `slots` from `start`, growing it as needed.
fn store<T: Clone>(slots: &mut Vec<Option<T>>, start: u32, items: Vec<Option<T>>) {
    let end = start as usize + items.len();
    if slots.len() < end {
        slots.resize(end, None);
    }
    for (slot, item) in slots[start as usize..end].iter_mut().zip(items) {
        *slot = item;
    }
}

/// An object bound, with a reference of the program's.
fn public<O: Counted + ComObjectInterface<I>, I: Interface>(held: &Option<Held<O>>) -> Option<I> {
    held.as_ref().map(Held::public)
}

/// Writes `count` slots from `start` of `slots` (unbound past its end) to the program's array `out`.
fn load<O: Counted + ComObjectInterface<I>, I: Interface>(
    slots: &[Option<Held<O>>],
    start: u32,
    count: u32,
    out: *mut Option<I>,
) {
    if out.is_null() {
        return;
    }
    for i in 0..count as usize {
        let item = slots.get(start as usize + i).and_then(public);
        unsafe { out.add(i).write(item) };
    }
}

impl Context {
    pub(super) fn new(front: Shared, device: ID3D11Device, slot: Rc<Cell<*mut c_void>>) -> Self {
        Context {
            front: Locked::new(front),
            child: Child::new(device),
            slot: Locked::new(slot),
            bound: RefCell::new(Bound::default()),
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let _lock = lock();
        self.slot.set(std::ptr::null_mut());
        unsafe { ManuallyDrop::drop(&mut self.child.device) };
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

/// Our object of type `T` behind a COM object; none for a foreign one.
fn ours<I: Interface, T>(object: Option<&I>) -> Option<&T::Outer>
where
    T: ComObjectInner,
    T::Outer: Counted + std::any::Any,
{
    object.and_then(|i| i.cast_object_ref::<T>().ok())
}

/// Library references to the objects of ours among `count` COM objects of type `T`, a foreign
/// object counting as none.
fn held<I: Interface, T>(list: *const Option<I>, count: u32) -> Vec<Option<Held<T::Outer>>>
where
    T: ComObjectInner,
    T::Outer: Counted + std::any::Any,
{
    items(list, count)
        .iter()
        .map(|i| ours::<I, T>(i.as_ref()).map(Held::new))
        .collect()
}

fn held_one<I: Interface, T>(object: &Ref<I>) -> Option<Held<T::Outer>>
where
    T: ComObjectInner,
    T::Outer: Counted + std::any::Any,
{
    ours::<I, T>(object.as_ref()).map(Held::new)
}

/// The front objects behind held objects.
fn fronts<O: Counted, F: Clone>(
    list: &[Option<Held<O>>],
    get: impl Fn(&O) -> &F,
) -> Vec<Option<F>> {
    list.iter()
        .map(|h| h.as_ref().map(|h| get(h).clone()))
        .collect()
}

fn front_of<I: Interface, T, F: Clone>(object: &Ref<I>, get: impl Fn(&T) -> &F) -> Option<F>
where
    T: ComObjectInner,
    T::Outer: Counted + std::any::Any,
{
    ours::<I, T>(object.as_ref()).map(|o| get(o.get_impl()).clone())
}

impl Context {
    pub(super) fn call(&self) -> Call<'_> {
        Call::new(&self.front)
    }

    pub(super) fn unserved(&self, what: &'static str) {
        missing_void(what);
        self.front
            .borrow_mut()
            .lose(wcontainers_direct3d_11::Error::NotImplemented(what));
    }

    /// The end of a front call that has no result: an invalid one is dropped, as Direct3D drops it
    /// (its debug layer reports it); one the front cannot serve removes the device. True if it ran.
    fn quiet(&self, what: &'static str, r: wcontainers_direct3d_11::Result<()>) -> bool {
        match r {
            Ok(()) => true,
            Err(wcontainers_direct3d_11::Error::InvalidArg(why)) => {
                eprintln!("wcontainers direct3d 11: {what} dropped: {why}");
                false
            }
            Err(e) => {
                self.front.borrow_mut().lose(e);
                false
            }
        }
    }

    pub(super) fn vs_set_shader(
        &self,
        shader: Ref<ID3D11VertexShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let h = held_one::<_, VertexShader>(&shader);
        self.front
            .borrow_mut()
            .vs_set_shader(h.as_ref().map(|s| (*s.front).clone()));
        self.bound.borrow_mut().vs = h;
    }

    pub(super) fn ps_set_shader(
        &self,
        shader: Ref<ID3D11PixelShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let h = held_one::<_, PixelShader>(&shader);
        self.front
            .borrow_mut()
            .ps_set_shader(h.as_ref().map(|s| (*s.front).clone()));
        self.bound.borrow_mut().ps = h;
    }

    fn set_constant_buffers(
        &self,
        stage: usize,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        let h = held::<_, Buffer>(list, n);
        let b = fronts(&h, |b| &*b.front);
        let r = self
            .front
            .borrow_mut()
            .set_constant_buffers(stage, start, &b);
        if self.quiet("SetConstantBuffers", r) {
            store(
                &mut self.bound.borrow_mut().constant_buffers[stage],
                start,
                h,
            );
        }
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
        let h = held::<_, ShaderResourceView>(list, n);
        let v = fronts(&h, |v| &*v.front);
        let r = self
            .front
            .borrow_mut()
            .set_shader_resources(stage, start, &v);
        if self.quiet("SetShaderResources", r) {
            store(&mut self.bound.borrow_mut().resources[stage], start, h);
            self.forget_unbound_resources();
        }
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
        let h = held::<_, SamplerState>(list, n);
        let s = fronts(&h, |s| &*s.front);
        let r = self.front.borrow_mut().set_samplers(stage, start, &s);
        if self.quiet("SetSamplers", r) {
            store(&mut self.bound.borrow_mut().samplers[stage], start, h);
        }
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

    /// Forgets the shader resources the front unbound: views of a resource now bound as a target.
    fn forget_unbound_resources(&self) {
        let mut gone = Vec::new();
        {
            let front = self.front.borrow();
            let mut bound = self.bound.borrow_mut();
            for stage in [VERTEX, PIXEL] {
                for (slot, h) in bound.resources[stage].iter_mut().enumerate() {
                    if h.is_some() && !front.shader_resource_bound(stage, slot as u32) {
                        gone.push(h.take());
                    }
                }
            }
        }
        drop(gone);
    }

    pub(super) fn ia_set_input_layout(&self, layout: Ref<ID3D11InputLayout>) {
        let h = held_one::<_, InputLayout>(&layout);
        self.front
            .borrow_mut()
            .ia_set_input_layout(h.as_ref().map(|l| (*l.front).clone()));
        self.bound.borrow_mut().layout = h;
    }

    pub(super) fn ia_set_primitive_topology(&self, topology: D3D_PRIMITIVE_TOPOLOGY) {
        self.front
            .borrow_mut()
            .ia_set_primitive_topology(topology.0 as u32);
        self.bound.borrow_mut().topology = topology;
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
        let h = held::<_, Buffer>(list, n);
        let buffers = fronts(&h, |b| &*b.front);
        let strides = unsafe { std::slice::from_raw_parts(strides, n as usize) };
        let offsets = unsafe { std::slice::from_raw_parts(offsets, n as usize) };
        let slots: Vec<_> = buffers
            .into_iter()
            .zip(strides.iter().zip(offsets))
            .map(|(b, (&s, &o))| b.map(|b| (b, s, o)))
            .collect();
        let r = self.front.borrow_mut().ia_set_vertex_buffers(start, &slots);
        if self.quiet("IASetVertexBuffers", r) {
            let mut bound = self.bound.borrow_mut();
            let end = (start + n) as usize;
            if bound.vertex_buffers.len() < end {
                bound.vertex_buffers.resize(end, (None, 0, 0));
            }
            for (i, b) in h.into_iter().enumerate() {
                bound.vertex_buffers[start as usize + i] = (b, strides[i], offsets[i]);
            }
        }
    }

    pub(super) fn ia_set_index_buffer(
        &self,
        buffer: Ref<ID3D11Buffer>,
        format: DXGI_FORMAT,
        offset: u32,
    ) {
        let h = held_one::<_, Buffer>(&buffer);
        let r = self.front.borrow_mut().ia_set_index_buffer(
            h.as_ref().map(|b| (*b.front).clone()),
            format.0 as u32,
            offset,
        );
        if self.quiet("IASetIndexBuffer", r) {
            self.bound.borrow_mut().index = (h, format, offset);
        }
    }

    pub(super) fn om_set_render_targets(
        &self,
        n: u32,
        list: *const Option<ID3D11RenderTargetView>,
        depth: Ref<ID3D11DepthStencilView>,
    ) {
        let h = held::<_, RenderTargetView>(list, n);
        let targets = fronts(&h, |v| &*v.front);
        let hd = held_one::<_, DepthStencilView>(&depth);
        let d = hd.as_ref().map(|v| (*v.front).clone());
        let r = self.front.borrow_mut().om_set_render_targets(&targets, d);
        if self.quiet("OMSetRenderTargets", r) {
            let old = {
                let mut bound = self.bound.borrow_mut();
                (
                    std::mem::replace(&mut bound.targets, h),
                    std::mem::replace(&mut bound.depth, hd),
                )
            };
            drop(old);
            self.forget_unbound_resources();
        }
    }

    pub(super) fn om_set_blend_state(
        &self,
        state: Ref<ID3D11BlendState>,
        factor: *const f32,
        mask: u32,
    ) {
        let h = held_one::<_, BlendState>(&state);
        let f = (!factor.is_null()).then(|| unsafe { *factor.cast::<[f32; 4]>() });
        self.front
            .borrow_mut()
            .om_set_blend_state(h.as_ref().map(|s| (*s.front).clone()), f, mask);
        self.bound.borrow_mut().blend = (h, f.unwrap_or([1.0; 4]), mask);
    }

    pub(super) fn om_set_depth_stencil_state(
        &self,
        state: Ref<ID3D11DepthStencilState>,
        reference: u32,
    ) {
        let h = held_one::<_, DepthStencilState>(&state);
        self.front
            .borrow_mut()
            .om_set_depth_stencil_state(h.as_ref().map(|s| (*s.front).clone()), reference);
        self.bound.borrow_mut().depth_stencil = (h, reference);
    }

    pub(super) fn rs_set_state(&self, state: Ref<ID3D11RasterizerState>) {
        let h = held_one::<_, RasterizerState>(&state);
        self.front
            .borrow_mut()
            .rs_set_state(h.as_ref().map(|s| (*s.front).clone()));
        self.bound.borrow_mut().raster = h;
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
        self.bound.borrow_mut().viewports = list.to_vec();
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
        self.bound.borrow_mut().scissors = list.to_vec();
    }

    pub(super) fn clear_render_target_view(
        &self,
        view: Ref<ID3D11RenderTargetView>,
        color: *const f32,
    ) {
        let Some(v) = front_of(&view, |v: &RenderTargetView| &*v.front) else {
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
        let Some(v) = front_of(&view, |v: &DepthStencilView| &*v.front) else {
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
        // The old bindings are released after the borrow ends: their drops may reach the context.
        let old = std::mem::take(&mut *self.bound.borrow_mut());
        drop(old);
    }

    pub(super) fn flush(&self) {
        self.front.borrow_mut().flush();
    }

    pub(super) fn vs_get_shader(
        &self,
        out: OutRef<ID3D11VertexShader>,
        _c: OutRef<ID3D11ClassInstance>,
        count: *mut u32,
    ) {
        no_instances(count);
        let _ = out.write(public(&self.bound.borrow().vs));
    }

    pub(super) fn ps_get_shader(
        &self,
        out: OutRef<ID3D11PixelShader>,
        _c: OutRef<ID3D11ClassInstance>,
        count: *mut u32,
    ) {
        no_instances(count);
        let _ = out.write(public(&self.bound.borrow().ps));
    }

    pub(super) fn vs_get_constant_buffers(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11Buffer>,
    ) {
        load(&self.bound.borrow().constant_buffers[VERTEX], start, n, out);
    }

    pub(super) fn ps_get_constant_buffers(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11Buffer>,
    ) {
        load(&self.bound.borrow().constant_buffers[PIXEL], start, n, out);
    }

    pub(super) fn vs_get_shader_resources(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11ShaderResourceView>,
    ) {
        load(&self.bound.borrow().resources[VERTEX], start, n, out);
    }

    pub(super) fn ps_get_shader_resources(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11ShaderResourceView>,
    ) {
        load(&self.bound.borrow().resources[PIXEL], start, n, out);
    }

    pub(super) fn vs_get_samplers(&self, start: u32, n: u32, out: *mut Option<ID3D11SamplerState>) {
        load(&self.bound.borrow().samplers[VERTEX], start, n, out);
    }

    pub(super) fn ps_get_samplers(&self, start: u32, n: u32, out: *mut Option<ID3D11SamplerState>) {
        load(&self.bound.borrow().samplers[PIXEL], start, n, out);
    }

    pub(super) fn ia_get_input_layout(&self, out: OutRef<ID3D11InputLayout>) {
        let _ = out.write(public(&self.bound.borrow().layout));
    }

    pub(super) fn ia_get_vertex_buffers(
        &self,
        start: u32,
        n: u32,
        buffers: OutRef<ID3D11Buffer>,
        strides: *mut u32,
        offsets: *mut u32,
    ) {
        // The out reference is the program's array of `n` buffers.
        let buffers: *mut Option<ID3D11Buffer> = unsafe { std::mem::transmute(buffers) };
        let bound = self.bound.borrow();
        for i in 0..n as usize {
            let (b, stride, offset) = bound
                .vertex_buffers
                .get(start as usize + i)
                .map_or((None, 0, 0), |(b, s, o)| (public(b), *s, *o));
            unsafe {
                if !buffers.is_null() {
                    buffers.add(i).write(b);
                }
                if !strides.is_null() {
                    strides.add(i).write(stride);
                }
                if !offsets.is_null() {
                    offsets.add(i).write(offset);
                }
            }
        }
    }

    pub(super) fn ia_get_index_buffer(
        &self,
        out: OutRef<ID3D11Buffer>,
        format: *mut DXGI_FORMAT,
        offset: *mut u32,
    ) {
        let (b, f, o) = {
            let bound = self.bound.borrow();
            (public(&bound.index.0), bound.index.1, bound.index.2)
        };
        let _ = out.write(b);
        unsafe {
            if let Some(format) = format.as_mut() {
                *format = f;
            }
            if let Some(offset) = offset.as_mut() {
                *offset = o;
            }
        }
    }

    pub(super) fn ia_get_primitive_topology(&self, out: *mut D3D_PRIMITIVE_TOPOLOGY) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.bound.borrow().topology;
        }
    }

    pub(super) fn om_get_render_targets(
        &self,
        n: u32,
        targets: *mut Option<ID3D11RenderTargetView>,
        depth: OutRef<ID3D11DepthStencilView>,
    ) {
        let bound = self.bound.borrow();
        load(&bound.targets, 0, n, targets);
        let _ = depth.write(public(&bound.depth));
    }

    pub(super) fn om_get_blend_state(
        &self,
        out: OutRef<ID3D11BlendState>,
        factor: *mut f32,
        mask: *mut u32,
    ) {
        let (b, f, m) = {
            let bound = self.bound.borrow();
            (public(&bound.blend.0), bound.blend.1, bound.blend.2)
        };
        let _ = out.write(b);
        unsafe {
            if !factor.is_null() {
                factor.cast::<[f32; 4]>().write(f);
            }
            if let Some(mask) = mask.as_mut() {
                *mask = m;
            }
        }
    }

    pub(super) fn om_get_depth_stencil_state(
        &self,
        out: OutRef<ID3D11DepthStencilState>,
        reference: *mut u32,
    ) {
        let (d, r) = {
            let bound = self.bound.borrow();
            (public(&bound.depth_stencil.0), bound.depth_stencil.1)
        };
        let _ = out.write(d);
        if let Some(reference) = unsafe { reference.as_mut() } {
            *reference = r;
        }
    }

    pub(super) fn rs_get_state(&self, out: OutRef<ID3D11RasterizerState>) {
        let _ = out.write(public(&self.bound.borrow().raster));
    }

    pub(super) fn rs_get_viewports(&self, count: *mut u32, out: *mut D3D11_VIEWPORT) {
        get_list(&self.bound.borrow().viewports, count, out);
    }

    pub(super) fn rs_get_scissor_rects(&self, count: *mut u32, out: *mut RECT) {
        get_list(&self.bound.borrow().scissors, count, out);
    }

    /// A call on a stage or feature the front does not serve, which is served while it binds nothing.
    pub(super) fn only_unbinding(&self, what: &'static str, binds: bool) {
        if binds {
            self.unserved(what);
        }
    }

    pub(super) fn get_type(&self) -> D3D11_DEVICE_CONTEXT_TYPE {
        D3D11_DEVICE_CONTEXT_IMMEDIATE
    }

    pub(super) fn get_context_flags(&self) -> u32 {
        0
    }
}

/// Whether any of `count` items of a COM array argument is an object.
pub(super) fn binds<T>(list: *const Option<T>, count: u32) -> bool {
    items(list, count).iter().any(Option::is_some)
}

/// Writes `count` empty slots to the program's array `out` (a stage or feature nothing is bound to).
pub(super) fn nothing_bound<T>(out: *mut Option<T>, count: u32) {
    if !out.is_null() {
        for i in 0..count as usize {
            unsafe { out.add(i).write(None) };
        }
    }
}

/// The class instances of a bound shader: none.
pub(super) fn no_instances(count: *mut u32) {
    if let Some(count) = unsafe { count.as_mut() } {
        *count = 0;
    }
}

/// `RSGetViewports` and `RSGetScissorRects`: the bound count when `out` is null, else up to `*count`
/// items, the slots past the bound ones zeroed.
fn get_list<T: Copy + Default>(list: &[T], count: *mut u32, out: *mut T) {
    let Some(count) = (unsafe { count.as_mut() }) else {
        return;
    };
    if out.is_null() {
        *count = list.len() as u32;
        return;
    }
    for i in 0..*count as usize {
        unsafe { out.add(i).write(list.get(i).copied().unwrap_or_default()) };
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
