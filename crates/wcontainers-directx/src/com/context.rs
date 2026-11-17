//! The immediate context.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Foundation::{E_INVALIDARG, RECT};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::core::{
    BOOL, ComObjectInner, ComObjectInterface, IUnknownImpl, Interface, OutRef, PCWSTR, Ref, Result,
    implement,
};

use wcontainers_direct3d_11::desc::{Box3, Viewport};
use wcontainers_direct3d_11::object::Resource;
use wcontainers_direct3d_11::{COMPUTE, PIXEL, UAV_SLOTS, VERTEX, format};

use super::counted::{Counted, Held};
use super::*;

#[implement(ID3D11DeviceContext1, ID3D11Multithread, ID3DUserDefinedAnnotation)]
pub struct Context {
    front: Locked<Shared>,
    pub(super) child: Child,
    /// The device's pointer to this context, cleared when the context goes.
    slot: Locked<Rc<Cell<*mut c_void>>>,
    /// What is bound, as the program's objects, for the `Get` methods; holding a library reference
    /// on each, as Direct3D's context does.
    bound: RefCell<Bound>,
    /// `ID3D11Multithread`: the program's `Enter` calls not yet left, and its protection flag.
    entered: Cell<u32>,
    protected: Cell<bool>,
}

/// The objects and values bound to the context.
struct Bound {
    vs: Option<Held<VertexShader_Impl>>,
    ps: Option<Held<PixelShader_Impl>>,
    cs: Option<Held<ComputeShader_Impl>>,
    /// Per stage (vertex, pixel, compute), per slot.
    constant_buffers: [Vec<Option<Held<Buffer_Impl>>>; 3],
    /// Per stage and slot, the constants bound (first, count); none for the whole buffer.
    constant_ranges: [Vec<Option<(u32, u32)>>; 3],
    resources: [Vec<Option<Held<ShaderResourceView_Impl>>>; 3],
    samplers: [Vec<Option<Held<SamplerState_Impl>>>; 3],
    uavs: [Vec<Option<Held<UnorderedAccessView_Impl>>>; 3],
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
            cs: None,
            constant_buffers: Default::default(),
            constant_ranges: Default::default(),
            resources: Default::default(),
            samplers: Default::default(),
            uavs: Default::default(),
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
            entered: Cell::new(0),
            protected: Cell::new(false),
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
        first: *const u32,
        count: *const u32,
    ) {
        let h = held::<_, Buffer>(list, n);
        let b = fronts(&h, |b| &*b.front);
        // Without either array the buffers are bound whole; without one of them, that half of
        // each slot's range stays as it was.
        let ranges: Vec<_> = if (first.is_null() && count.is_null()) || list.is_null() {
            vec![None; h.len()]
        } else {
            let bound = self.bound.borrow();
            (0..h.len())
                .map(|i| {
                    let was = bound.constant_ranges[stage]
                        .get(start as usize + i)
                        .copied()
                        .flatten()
                        .unwrap_or((0, 4096));
                    let f = unsafe { first.as_ref().map_or(was.0, |_| *first.add(i)) };
                    let c = unsafe { count.as_ref().map_or(was.1, |_| *count.add(i)) };
                    Some((f, c))
                })
                .collect()
        };
        let r = self
            .front
            .borrow_mut()
            .set_constant_ranges(stage, start, &b, &ranges);
        if self.quiet("SetConstantBuffers", r) {
            let mut bound = self.bound.borrow_mut();
            store(&mut bound.constant_buffers[stage], start, h);
            store(&mut bound.constant_ranges[stage], start, ranges);
        }
    }

    /// Writes the ranges of `n` constant buffer slots from `start` to the program's arrays; a whole
    /// buffer, and an empty slot, are the first 4096 constants.
    fn get_constant_ranges(
        &self,
        stage: usize,
        start: u32,
        n: u32,
        first: *mut u32,
        count: *mut u32,
    ) {
        let bound = self.bound.borrow();
        for i in 0..n as usize {
            let slot = start as usize + i;
            let range = match (
                bound.constant_buffers[stage].get(slot),
                bound.constant_ranges[stage].get(slot),
            ) {
                (Some(Some(_)), Some(Some(r))) => *r,
                _ => (0, 4096),
            };
            unsafe {
                if !first.is_null() {
                    *first.add(i) = range.0;
                }
                if !count.is_null() {
                    *count.add(i) = range.1;
                }
            }
        }
    }

    pub(super) fn vs_set_constant_buffers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        self.set_constant_buffers(VERTEX, start, n, list, std::ptr::null(), std::ptr::null());
    }

    pub(super) fn ps_set_constant_buffers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        self.set_constant_buffers(PIXEL, start, n, list, std::ptr::null(), std::ptr::null());
    }

    pub(super) fn vs_set_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
        first: *const u32,
        count: *const u32,
    ) {
        self.set_constant_buffers(VERTEX, start, n, list, first, count);
    }

    pub(super) fn ps_set_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
        first: *const u32,
        count: *const u32,
    ) {
        self.set_constant_buffers(PIXEL, start, n, list, first, count);
    }

    pub(super) fn vs_get_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        out: OutRef<ID3D11Buffer>,
        first: *mut u32,
        count: *mut u32,
    ) {
        load(
            &self.bound.borrow().constant_buffers[VERTEX],
            start,
            n,
            out_array(out),
        );
        self.get_constant_ranges(VERTEX, start, n, first, count);
    }

    pub(super) fn ps_get_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        out: OutRef<ID3D11Buffer>,
        first: *mut u32,
        count: *mut u32,
    ) {
        load(
            &self.bound.borrow().constant_buffers[PIXEL],
            start,
            n,
            out_array(out),
        );
        self.get_constant_ranges(PIXEL, start, n, first, count);
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
            for stage in [VERTEX, PIXEL, COMPUTE] {
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

    pub(super) fn generate_mips(&self, view: Ref<ID3D11ShaderResourceView>) {
        // A view of a buffer has no levels; Direct3D drops the call.
        if let Some(v) = front_of(&view, |v: &ShaderResourceView| &*v.front)
            && let Some(v) = v.texture()
        {
            self.front.borrow_mut().generate_mips(v);
        }
    }

    pub(super) fn begin(&self, a: Ref<ID3D11Asynchronous>) {
        if let Some(q) = query_of(a.as_ref()) {
            self.front.borrow_mut().begin_query(&q);
        }
    }

    pub(super) fn end(&self, a: Ref<ID3D11Asynchronous>) {
        if let Some(q) = query_of(a.as_ref()) {
            self.front.borrow_mut().end_query(&q);
        }
    }

    /// `GetData`: `S_FALSE` until the answer is there; a size other than the query's (or zero) is
    /// invalid.
    pub(super) fn get_data(
        &self,
        a: Ref<ID3D11Asynchronous>,
        data: *mut c_void,
        size: u32,
        flags: u32,
    ) -> Result<()> {
        let q = query_of(a.as_ref()).ok_or(E_INVALIDARG)?;
        let want = wcontainers_direct3d_11::query::data_size(q.kind).unwrap_or(0);
        if (size != 0 && size != want) || (size != 0 && data.is_null()) {
            return Err(E_INVALIDARG.into());
        }
        let flush = flags & D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32 == 0;
        let r = self.front.borrow_mut().query_data(&q, flush);
        match r.map_err(hresult)? {
            None => Err(windows::core::Error::from_hresult(
                windows::Win32::Foundation::S_FALSE,
            )),
            Some(v) => {
                if size != 0 {
                    let bytes = v.to_le_bytes();
                    unsafe {
                        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data.cast(), want as usize)
                    };
                }
                Ok(())
            }
        }
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

    /// Direct3D 11.1's copy: its flags only promise what the program does not overlap, so it is
    /// the plain copy.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn copy_subresource_region1(
        &self,
        dst: Ref<ID3D11Resource>,
        dst_sub: u32,
        x: u32,
        y: u32,
        z: u32,
        src: Ref<ID3D11Resource>,
        src_sub: u32,
        src_box: *const D3D11_BOX,
        _flags: u32,
    ) {
        self.copy_subresource_region(dst, dst_sub, x, y, z, src, src_sub, src_box);
    }

    /// Direct3D 11.1's update: its flags only promise what the program does not overlap.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_subresource1(
        &self,
        dst: Ref<ID3D11Resource>,
        sub: u32,
        dst_box: *const D3D11_BOX,
        data: *const c_void,
        row_pitch: u32,
        depth_pitch: u32,
        _flags: u32,
    ) {
        self.update_subresource(dst, sub, dst_box, data, row_pitch, depth_pitch);
    }

    /// Discards leave contents undefined; keeping them is one of the contents allowed.
    pub(super) fn discard_resource(&self, _resource: Ref<ID3D11Resource>) {}

    pub(super) fn discard_view(&self, _view: Ref<ID3D11View>) {}

    pub(super) fn discard_view1(&self, _view: Ref<ID3D11View>, _rects: *const RECT, _n: u32) {}

    /// `ClearView` of a whole render target view; rectangles and other views are not served.
    pub(super) fn clear_view(
        &self,
        view: Ref<ID3D11View>,
        color: *const f32,
        rects: *const RECT,
        n: u32,
    ) {
        if color.is_null() {
            return;
        }
        let Some(v) = front_of(&view, |v: &RenderTargetView| &*v.front) else {
            return self.unserved("ID3D11DeviceContext1::ClearView of a view not a render target");
        };
        if n != 0 && !rects.is_null() {
            return self.unserved("ID3D11DeviceContext1::ClearView of rectangles");
        }
        let c = unsafe { *color.cast::<[f32; 4]>() };
        self.front.borrow_mut().clear_render_target_view(&v, c);
    }

    /// `ID3D11Multithread::Enter`: the library's lock, held until `Leave`.
    pub(super) fn enter(&self) {
        std::mem::forget(lock());
        self.entered.set(self.entered.get() + 1);
    }

    pub(super) fn leave(&self) {
        if self.entered.get() > 0 {
            self.entered.set(self.entered.get() - 1);
            // SAFETY: this thread holds the lock taken by its `Enter`, and the call's own above it.
            unsafe { unlock() };
        }
    }

    /// Calls are serialised by the library's lock whatever the flag says; it is kept and reported.
    pub(super) fn set_multithread_protected(&self, on: BOOL) -> BOOL {
        self.protected.replace(on.as_bool()).into()
    }

    pub(super) fn get_multithread_protected(&self) -> BOOL {
        self.protected.get().into()
    }

    /// Annotations, answered as Direct3D answers them when no profiling tool is attached.
    pub(super) fn begin_event(&self, _name: &PCWSTR) -> i32 {
        -1
    }

    pub(super) fn end_event(&self) -> i32 {
        -1
    }

    pub(super) fn set_marker(&self, _name: &PCWSTR) {}

    pub(super) fn get_status(&self) -> BOOL {
        false.into()
    }

    pub(super) fn update_subresource(
        &self,
        dst: Ref<ID3D11Resource>,
        sub: u32,
        dst_box: *const D3D11_BOX,
        data: *const c_void,
        row_pitch: u32,
        depth_pitch: u32,
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
            Resource::Texture(t) => {
                let e = t.mip_extent(sub % t.desc.mip_levels);
                let (w, h, z) = b.map_or((e.width, e.height, e.depth_or_layers), |b| {
                    (
                        b.right.saturating_sub(b.left),
                        b.bottom.saturating_sub(b.top),
                        b.back.saturating_sub(b.front),
                    )
                });
                let (row, rows) = format::row_bytes(t.format, w, h);
                if z == 0 || rows == 0 {
                    0
                } else {
                    (depth_pitch * (z - 1) + row_pitch * (rows - 1) + row) as usize
                }
            }
        };
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) };
        let r = self.front.borrow_mut().update_subresource(
            &d,
            sub,
            b.as_ref(),
            bytes,
            row_pitch,
            depth_pitch,
        );
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
        let mapped = resource
            .as_ref()
            .and_then(resource_of)
            .ok_or(E_INVALIDARG.into())
            .and_then(|r| {
                self.front
                    .borrow_mut()
                    .map(&r, sub, kind.0 as u32, flags)
                    .map_err(hresult)
            });
        let Some(out) = (unsafe { out.as_mut() }) else {
            return mapped.map(drop);
        };
        match mapped {
            Ok(m) => {
                *out = D3D11_MAPPED_SUBRESOURCE {
                    pData: m.data.cast(),
                    RowPitch: m.row_pitch,
                    DepthPitch: m.depth_pitch,
                };
                Ok(())
            }
            // A failed map hands out no pointer and leaves the pitches, as Direct3D does.
            Err(e) => {
                out.pData = std::ptr::null_mut();
                Err(e)
            }
        }
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

    pub(super) fn cs_set_shader(
        &self,
        shader: Ref<ID3D11ComputeShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let h = held_one::<_, ComputeShader>(&shader);
        self.front
            .borrow_mut()
            .cs_set_shader(h.as_ref().map(|s| (*s.front).clone()));
        self.bound.borrow_mut().cs = h;
    }

    pub(super) fn cs_get_shader(
        &self,
        out: OutRef<ID3D11ComputeShader>,
        _c: OutRef<ID3D11ClassInstance>,
        count: *mut u32,
    ) {
        no_instances(count);
        let _ = out.write(public(&self.bound.borrow().cs));
    }

    pub(super) fn cs_set_constant_buffers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        self.set_constant_buffers(COMPUTE, start, n, list, std::ptr::null(), std::ptr::null());
    }

    pub(super) fn cs_set_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
        first: *const u32,
        count: *const u32,
    ) {
        self.set_constant_buffers(COMPUTE, start, n, list, first, count);
    }

    pub(super) fn cs_get_constant_buffers(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11Buffer>,
    ) {
        load(
            &self.bound.borrow().constant_buffers[COMPUTE],
            start,
            n,
            out,
        );
    }

    pub(super) fn cs_get_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        out: OutRef<ID3D11Buffer>,
        first: *mut u32,
        count: *mut u32,
    ) {
        load(
            &self.bound.borrow().constant_buffers[COMPUTE],
            start,
            n,
            out_array(out),
        );
        self.get_constant_ranges(COMPUTE, start, n, first, count);
    }

    pub(super) fn cs_set_shader_resources(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.set_shader_resources(COMPUTE, start, n, list);
    }

    pub(super) fn cs_get_shader_resources(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11ShaderResourceView>,
    ) {
        load(&self.bound.borrow().resources[COMPUTE], start, n, out);
    }

    pub(super) fn cs_set_samplers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11SamplerState>,
    ) {
        self.set_samplers(COMPUTE, start, n, list);
    }

    pub(super) fn cs_get_samplers(&self, start: u32, n: u32, out: *mut Option<ID3D11SamplerState>) {
        load(&self.bound.borrow().samplers[COMPUTE], start, n, out);
    }

    /// Binds unordered access views to `stage` from `start`, with the counters' initial counts.
    fn set_uavs(
        &self,
        stage: usize,
        start: u32,
        n: u32,
        list: *const Option<ID3D11UnorderedAccessView>,
        counts: *const u32,
    ) {
        let h = held::<_, UnorderedAccessView>(list, n);
        let v = fronts(&h, |v| &*v.front);
        let counts =
            (!counts.is_null()).then(|| unsafe { std::slice::from_raw_parts(counts, h.len()) });
        let r = self.front.borrow_mut().set_uavs(stage, start, &v, counts);
        if self.quiet("SetUnorderedAccessViews", r) {
            store(&mut self.bound.borrow_mut().uavs[stage], start, h);
        }
    }

    pub(super) fn cs_set_unordered_access_views(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11UnorderedAccessView>,
        counts: *const u32,
    ) {
        self.set_uavs(COMPUTE, start, n, list, counts);
    }

    pub(super) fn cs_get_unordered_access_views(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11UnorderedAccessView>,
    ) {
        load(&self.bound.borrow().uavs[COMPUTE], start, n, out);
    }

    /// Targets (unless `n` is `D3D11_KEEP_RENDER_TARGETS_AND_DEPTH_STENCIL`) and the pixel
    /// stage's unordered access views from `uav_start` (unless `uav_n` is
    /// `D3D11_KEEP_UNORDERED_ACCESS_VIEWS`), the slots outside them unbound.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn om_set_render_targets_and_unordered_access_views(
        &self,
        n: u32,
        targets: *const Option<ID3D11RenderTargetView>,
        depth: Ref<ID3D11DepthStencilView>,
        uav_start: u32,
        uav_n: u32,
        uavs: *const Option<ID3D11UnorderedAccessView>,
        counts: *const u32,
    ) {
        const KEEP: u32 = u32::MAX;
        if n != KEEP {
            self.om_set_render_targets(n, targets, depth);
        }
        if uav_n != KEEP {
            if (uav_start as usize).saturating_add(uav_n as usize) > UAV_SLOTS {
                eprintln!(
                    "wcontainers direct3d 11: OMSetRenderTargetsAndUnorderedAccessViews dropped: slots past the last"
                );
                return;
            }
            let none: [Option<ID3D11UnorderedAccessView>; UAV_SLOTS] = Default::default();
            self.set_uavs(PIXEL, 0, UAV_SLOTS as u32, none.as_ptr(), std::ptr::null());
            if uav_n > 0 {
                self.set_uavs(PIXEL, uav_start, uav_n, uavs, counts);
            }
        }
    }

    pub(super) fn om_get_render_targets_and_unordered_access_views(
        &self,
        n: u32,
        targets: *mut Option<ID3D11RenderTargetView>,
        depth: OutRef<ID3D11DepthStencilView>,
        uav_start: u32,
        uav_n: u32,
        uavs: *mut Option<ID3D11UnorderedAccessView>,
    ) {
        self.om_get_render_targets(n, targets, depth);
        load(&self.bound.borrow().uavs[PIXEL], uav_start, uav_n, uavs);
    }

    pub(super) fn dispatch(&self, x: u32, y: u32, z: u32) {
        self.front.borrow_mut().dispatch([x, y, z], None);
    }

    pub(super) fn dispatch_indirect(&self, args: Ref<ID3D11Buffer>, offset: u32) {
        let Some(b) = front_of(&args, |b: &Buffer| &*b.front) else {
            return;
        };
        self.front.borrow_mut().dispatch([0; 3], Some((&b, offset)));
    }

    pub(super) fn clear_unordered_access_view_uint(
        &self,
        view: Ref<ID3D11UnorderedAccessView>,
        values: *const u32,
    ) {
        let Some(v) = front_of(&view, |v: &UnorderedAccessView| &*v.front) else {
            return;
        };
        if values.is_null() {
            return;
        }
        let values = unsafe { *values.cast::<[u32; 4]>() };
        let r = self.front.borrow_mut().clear_uav(&v, values, false);
        self.quiet("ClearUnorderedAccessViewUint", r);
    }

    pub(super) fn clear_unordered_access_view_float(
        &self,
        view: Ref<ID3D11UnorderedAccessView>,
        values: *const f32,
    ) {
        let Some(v) = front_of(&view, |v: &UnorderedAccessView| &*v.front) else {
            return;
        };
        if values.is_null() {
            return;
        }
        let values = unsafe { *values.cast::<[f32; 4]>() }.map(f32::to_bits);
        let r = self.front.borrow_mut().clear_uav(&v, values, true);
        self.quiet("ClearUnorderedAccessViewFloat", r);
    }

    pub(super) fn copy_structure_count(
        &self,
        dst: Ref<ID3D11Buffer>,
        offset: u32,
        view: Ref<ID3D11UnorderedAccessView>,
    ) {
        let (Some(b), Some(v)) = (
            front_of(&dst, |b: &Buffer| &*b.front),
            front_of(&view, |v: &UnorderedAccessView| &*v.front),
        ) else {
            return;
        };
        let r = self.front.borrow_mut().copy_structure_count(&b, offset, &v);
        self.quiet("CopyStructureCount", r);
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
/// The program's array behind an out parameter the windows crate types as one object.
pub(super) fn out_array<T: Interface>(out: OutRef<T>) -> *mut Option<T> {
    // SAFETY: `OutRef` is a transparent pointer to the first element.
    unsafe { std::mem::transmute(out) }
}

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
