//! The immediate context and deferred contexts.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Foundation::{E_INVALIDARG, RECT};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::Win32::Graphics::Dxgi::DXGI_ERROR_INVALID_CALL;
use windows::core::{
    BOOL, ComObjectInner, ComObjectInterface, IUnknownImpl, Interface, OutRef, PCWSTR, Ref, Result,
    implement,
};

use wcontainers_direct3d_11::desc::{Box3, Viewport, map};
use wcontainers_direct3d_11::object::Resource;
use wcontainers_direct3d_11::query::Answer;
use wcontainers_direct3d_11::{
    COLOR_COMPONENTS, COMPUTE, Color, DOMAIN, GEOMETRY, HULL, PIXEL, Rect, STAGES, UAV_SLOTS,
    VERTEX, format,
};

/// Stream output target slots.
const SO_SLOTS: usize = D3D11_SO_BUFFER_SLOT_COUNT as usize;

use super::counted::{Counted, Held};
use super::*;

/// `D3D11_ERROR_DEFERRED_CONTEXT_MAP_WITHOUT_INITIAL_DISCARD`, which the windows crate lacks.
const MAP_WITHOUT_INITIAL_DISCARD: windows::core::HRESULT =
    windows::core::HRESULT(0x887C_0004_u32 as i32);

/// A deferred context, whose table refuses `ID3D11Multithread` as Direct3D's deferred contexts do.
pub(super) fn deferred_context(front: Shared, device: ID3D11Device) -> ID3D11DeviceContext3 {
    let c: ID3D11DeviceContext3 = Context::deferred(front, device).into();
    // The object's table of ID3D11DeviceContext3 is ours to point elsewhere.
    unsafe {
        let table = c.as_raw() as *mut *const ID3D11DeviceContext3_Vtbl;
        *table = deferred_table(*table);
    }
    c
}

type QueryInterface = unsafe extern "system" fn(
    *mut c_void,
    *const windows::core::GUID,
    *mut *mut c_void,
) -> windows::core::HRESULT;
static QUERY_INTERFACE: std::sync::OnceLock<QueryInterface> = std::sync::OnceLock::new();

/// The context's table with `QueryInterface` refusing `ID3D11Multithread`, made once.
fn deferred_table(original: *const ID3D11DeviceContext3_Vtbl) -> *const ID3D11DeviceContext3_Vtbl {
    struct Table(ID3D11DeviceContext3_Vtbl);
    // SAFETY: a table of function pointers, never written after it is made.
    unsafe impl Sync for Table {}
    unsafe impl Send for Table {}
    static PATCHED: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    &PATCHED
        .get_or_init(|| {
            // SAFETY: the windows crate's table, copied bit for bit.
            let mut table = unsafe { std::ptr::read(original) };
            let unknown = &mut table.base__.base__.base__.base__.base__;
            let _ = QUERY_INTERFACE.set(unknown.QueryInterface);
            unknown.QueryInterface = query_deferred;
            Table(table)
        })
        .0
}

unsafe extern "system" fn query_deferred(
    this: *mut c_void,
    iid: *const windows::core::GUID,
    out: *mut *mut c_void,
) -> windows::core::HRESULT {
    if !iid.is_null() && unsafe { *iid } == ID3D11Multithread::IID {
        if !out.is_null() {
            unsafe { *out = std::ptr::null_mut() };
        }
        return windows::Win32::Foundation::E_NOINTERFACE;
    }
    let entry = QUERY_INTERFACE.get().expect("the table was patched");
    unsafe { entry(this, iid, out) }
}

/// A call a deferred context recorded, run on the device's front when its command list is
/// executed (as often as it is executed).
pub type Command = Box<dyn Fn(&mut Front)>;

#[implement(ID3D11DeviceContext3, ID3D11Multithread, ID3DUserDefinedAnnotation)]
pub struct Context {
    front: Locked<Shared>,
    pub(super) child: Child,
    /// The device's pointer to this context, cleared when the context goes.
    slot: Locked<Rc<Cell<*mut c_void>>>,
    /// For a deferred context, what it recorded since its last command list; none for the
    /// immediate context.
    recorded: Option<Locked<RefCell<Vec<Command>>>>,
    /// A deferred context's mapped subresources (by resource address and subresource): the bytes
    /// the program writes, written into the resource when the command list runs.
    staged: RefCell<HashMap<(usize, u32), Staging>>,
    /// What is bound and the state object, which the immediate context's are the device's: they
    /// last while the device does, through any number of COM contexts made for it.
    kept: Rc<Kept>,
    /// The default state, which the getters of the API whose behaviour is not active read.
    empty: RefCell<Bound>,
    /// Whether the call running came through Direct3D 10's device.
    pub(super) calling10: Cell<bool>,
}

/// Whether `h` holds the object at `address`.
fn holds<O: Counted>(h: &Option<Held<O>>, address: usize) -> bool {
    h.as_ref().is_some_and(|h| h.address() == address)
}

/// The slots of `list` holding the object at `address`.
fn slots<O: Counted>(list: &[Option<Held<O>>], address: usize) -> Vec<u32> {
    (0..list.len() as u32)
        .filter(|&i| holds(&list[i as usize], address))
        .collect()
}

/// A context's lasting state.
#[derive(Default)]
pub struct Kept {
    /// What is bound, as the program's objects, for the `Get` methods; holding a library reference
    /// on each, as Direct3D's context does.
    bound: RefCell<Bound>,
    /// The immediate context's state object, once the program swaps one in; a library reference,
    /// as Direct3D's context holds none of the program's on it.
    state: RefCell<Option<Held<DeviceContextState_Impl>>>,
    /// `ID3D11Multithread`: the program's `Enter` calls not yet left, and its protection flag.
    entered: Cell<u32>,
    protected: Cell<bool>,
}

impl Kept {
    /// Whether the active context state emulates Direct3D 10; not while a swap changes it.
    pub(super) fn emulates10(&self) -> bool {
        self.state
            .try_borrow()
            .is_ok_and(|s| s.as_ref().is_some_and(|s| s.emulates10))
    }
}

/// A context state object (`CreateDeviceContextState`): the bindings it holds while another is
/// in the immediate context.
pub struct DeviceContextState {
    bound: Locked<RefCell<Bound>>,
    /// Whether it emulates Direct3D 10's device (made for `ID3D10Device` or `ID3D10Device1`).
    emulates10: bool,
    pub(super) child: Child,
}

super::counted::counted!(DeviceContextState => DeviceContextState_Impl { main: ID3DDeviceContextState, ID3DDeviceContextState_Vtbl });

impl DeviceContextState {
    pub fn new(device: ID3D11Device, emulates10: bool) -> Self {
        DeviceContextState {
            bound: Locked::new(RefCell::new(Bound::default())),
            emulates10,
            child: Child::new(device),
        }
    }
}

/// The objects and values bound to the context.
struct Bound {
    vs: Option<Held<VertexShader_Impl>>,
    ps: Option<Held<PixelShader_Impl>>,
    cs: Option<Held<ComputeShader_Impl>>,
    gs: Option<Held<GeometryShader_Impl>>,
    hs: Option<Held<HullShader_Impl>>,
    ds: Option<Held<DomainShader_Impl>>,
    /// The stream output targets.
    so_targets: [Option<Held<Buffer_Impl>>; SO_SLOTS],
    /// Per stage (vertex, pixel, compute, geometry, hull, domain), per slot.
    constant_buffers: [Vec<Option<Held<Buffer_Impl>>>; STAGES],
    /// Per stage and slot, the constants bound (first, count); none for the whole buffer.
    constant_ranges: [Vec<Option<(u32, u32)>>; STAGES],
    resources: [Vec<Option<Held<ShaderResourceView_Impl>>>; STAGES],
    samplers: [Vec<Option<Held<SamplerState_Impl>>>; STAGES],
    uavs: [Vec<Option<Held<UnorderedAccessView_Impl>>>; COMPUTE + 1],
    layout: Option<Held<InputLayout_Impl>>,
    /// Per slot: buffer, stride and offset.
    vertex_buffers: Vec<(Option<Held<Buffer_Impl>>, u32, u32)>,
    index: (Option<Held<Buffer_Impl>>, DXGI_FORMAT, u32),
    topology: D3D_PRIMITIVE_TOPOLOGY,
    targets: Vec<Option<Held<RenderTargetView_Impl>>>,
    depth: Option<Held<DepthStencilView_Impl>>,
    blend: (Option<Held<BlendState_Impl>>, Color, u32),
    depth_stencil: (Option<Held<DepthStencilState_Impl>>, u32),
    raster: Option<Held<RasterizerState_Impl>>,
    viewports: Vec<D3D11_VIEWPORT>,
    scissors: Vec<RECT>,
    predication: (Option<Held<Predicate_Impl>>, bool),
}

impl Default for Bound {
    fn default() -> Self {
        Bound {
            vs: None,
            ps: None,
            cs: None,
            gs: None,
            hs: None,
            ds: None,
            so_targets: Default::default(),
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
            blend: (None, [1.0; COLOR_COMPONENTS], u32::MAX),
            depth_stencil: (None, 0),
            raster: None,
            viewports: Vec::new(),
            scissors: Vec::new(),
            predication: (None, false),
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

/// A deferred context's copy of a mapped subresource, laid out at its own pitches.
struct Staging {
    bytes: Vec<u8>,
    row_pitch: u32,
    depth_pitch: u32,
    /// Bytes of a row and rows of a slice, and slices, of the subresource.
    rows: (u32, u32, u32),
}

impl Context {
    pub(super) fn new(
        front: Shared,
        device: ID3D11Device,
        slot: Rc<Cell<*mut c_void>>,
        kept: Rc<Kept>,
    ) -> Self {
        Context {
            front: Locked::new(front),
            child: Child::new(device),
            slot: Locked::new(slot),
            recorded: None,
            staged: RefCell::new(HashMap::new()),
            kept,
            empty: RefCell::new(Bound::default()),
            calling10: Cell::new(false),
        }
    }

    /// A deferred context: its command lists start from the default state.
    pub(super) fn deferred(front: Shared, device: ID3D11Device) -> Self {
        let mut c = Context::new(
            front,
            device,
            Rc::new(Cell::new(std::ptr::null_mut())),
            Rc::default(),
        );
        c.recorded = Some(Locked::new(RefCell::new(Vec::new())));
        c.run("ClearState", |f| {
            f.clear_state();
            Ok(())
        });
        c
    }

    fn is_deferred(&self) -> bool {
        self.recorded.is_some()
    }

    /// Runs a front call now on the immediate context, or records it on a deferred one. True if it
    /// ran (or was recorded); an invalid call is dropped and one the front cannot serve removes the
    /// device, when it runs.
    fn run(
        &self,
        what: &'static str,
        call: impl Fn(&mut Front) -> wcontainers_direct3d_11::Result<()> + 'static,
    ) -> bool {
        match &self.recorded {
            None => {
                let mut front = self.front.borrow_mut();
                let r = call(&mut front);
                settle(&mut front, what, r)
            }
            Some(recorded) => {
                recorded.borrow_mut().push(Box::new(move |front| {
                    let r = call(front);
                    settle(front, what, r);
                }));
                true
            }
        }
    }
}

/// The end of a front call that has no result: an invalid one is dropped, as Direct3D drops it
/// (its debug layer reports it); one the front cannot serve removes the device. True if it ran.
fn settle(front: &mut Front, what: &'static str, r: wcontainers_direct3d_11::Result<()>) -> bool {
    match r {
        Ok(()) => true,
        Err(wcontainers_direct3d_11::Error::InvalidArg(why)) => {
            eprintln!("wcontainers direct3d 11: {what} dropped: {why}");
            false
        }
        Err(e) => {
            front.lose(e);
            false
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

    pub(super) fn vs_set_shader(
        &self,
        shader: Ref<ID3D11VertexShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let h = held_one::<_, VertexShader>(&shader);
        let s = h.as_ref().map(|s| (*s.front).clone());
        self.run("VSSetShader", move |f| {
            f.vs_set_shader(s.clone());
            Ok(())
        });
        self.kept.bound.borrow_mut().vs = h;
    }

    pub(super) fn gs_set_shader(
        &self,
        shader: Ref<ID3D11GeometryShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let h = held_one::<_, GeometryShader>(&shader);
        let s = h.as_ref().map(|s| (*s.front).clone());
        self.run("GSSetShader", move |f| {
            f.gs_set_shader(s.clone());
            Ok(())
        });
        self.kept.bound.borrow_mut().gs = h;
    }

    pub(super) fn gs_get_shader(
        &self,
        out: OutRef<ID3D11GeometryShader>,
        _c: OutRef<ID3D11ClassInstance>,
        count: *mut u32,
    ) {
        no_instances(count);
        let _ = out.write(public(&self.seen().gs));
    }

    pub(super) fn gs_set_shader_resources(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.set_shader_resources(GEOMETRY, start, n, list);
    }

    pub(super) fn gs_get_shader_resources(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11ShaderResourceView>,
    ) {
        load(&self.seen().resources[GEOMETRY], start, n, out);
    }

    pub(super) fn hs_set_shader(
        &self,
        shader: Ref<ID3D11HullShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let h = held_one::<_, HullShader>(&shader);
        let s = h.as_ref().map(|s| (*s.front).clone());
        self.run("HSSetShader", move |f| {
            f.hs_set_shader(s.clone());
            Ok(())
        });
        self.kept.bound.borrow_mut().hs = h;
    }

    pub(super) fn hs_get_shader(
        &self,
        out: OutRef<ID3D11HullShader>,
        _c: OutRef<ID3D11ClassInstance>,
        count: *mut u32,
    ) {
        no_instances(count);
        let _ = out.write(public(&self.seen().hs));
    }

    pub(super) fn ds_set_shader(
        &self,
        shader: Ref<ID3D11DomainShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let h = held_one::<_, DomainShader>(&shader);
        let s = h.as_ref().map(|s| (*s.front).clone());
        self.run("DSSetShader", move |f| {
            f.ds_set_shader(s.clone());
            Ok(())
        });
        self.kept.bound.borrow_mut().ds = h;
    }

    pub(super) fn ds_get_shader(
        &self,
        out: OutRef<ID3D11DomainShader>,
        _c: OutRef<ID3D11ClassInstance>,
        count: *mut u32,
    ) {
        no_instances(count);
        let _ = out.write(public(&self.seen().ds));
    }

    pub(super) fn hs_set_shader_resources(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.set_shader_resources(HULL, start, n, list);
    }

    pub(super) fn hs_get_shader_resources(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11ShaderResourceView>,
    ) {
        load(&self.seen().resources[HULL], start, n, out);
    }

    pub(super) fn ds_set_shader_resources(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11ShaderResourceView>,
    ) {
        self.set_shader_resources(DOMAIN, start, n, list);
    }

    pub(super) fn ds_get_shader_resources(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11ShaderResourceView>,
    ) {
        load(&self.seen().resources[DOMAIN], start, n, out);
    }

    /// `SOSetTargets`: the slots past the buffers given are unbound; an offset of -1 appends where
    /// the buffer was filled to, and no offsets start each buffer from its beginning.
    pub(super) fn so_set_targets(
        &self,
        n: u32,
        list: *const Option<ID3D11Buffer>,
        offsets: *const u32,
    ) {
        if n as usize > SO_SLOTS {
            self.run("SOSetTargets", |_| {
                Err(wcontainers_direct3d_11::Error::InvalidArg(
                    "more than four stream output targets",
                ))
            });
            return;
        }
        let h = held::<_, Buffer>(list, n);
        let b = fronts(&h, |b| &*b.front);
        let o: Vec<u32> = if offsets.is_null() {
            vec![0; h.len()]
        } else {
            (0..h.len()).map(|i| unsafe { *offsets.add(i) }).collect()
        };
        if self.run("SOSetTargets", move |f| f.so_set_targets(&b, &o)) {
            let mut targets: [Option<Held<Buffer_Impl>>; SO_SLOTS] = Default::default();
            for (t, h) in targets.iter_mut().zip(h) {
                *t = h;
            }
            let mut bound = self.kept.bound.borrow_mut();
            bound.so_targets = targets;
        }
    }

    /// The stream output targets' offsets for Direct3D 10's `SOGetTargets`: whatever offset a
    /// target was bound at, Direct3D 10 answers all ones for it, and zero for an empty slot.
    pub(super) fn so_get_offsets(&self, n: u32, out: *mut u32) {
        let seen = self.seen();
        for i in 0..n as usize {
            let bound = seen.so_targets.get(i).is_some_and(Option::is_some);
            unsafe { out.add(i).write(if bound { u32::MAX } else { 0 }) };
        }
    }

    /// The state the getters read: a getter of the API whose behaviour is not active (Direct3D
    /// 10's or 11's, by the context state) sees the default state, as Direct3D's do.
    fn seen(&self) -> std::cell::Ref<'_, Bound> {
        if self.emulates10() != self.calling10.get() {
            self.empty.borrow()
        } else {
            self.kept.bound.borrow()
        }
    }

    /// Whether a context state emulating Direct3D 10 is active on this context: Direct3D 11's
    /// calls on it then do nothing, and Direct3D 10's device's run here.
    pub(super) fn emulates10(&self) -> bool {
        self.kept.emulates10()
    }

    /// Empties every slot holding the object at `address`, which the program released for the
    /// last time, as if the program had bound nothing there (Direct3D 10's device holds no
    /// reference on what it binds). Rasterizer, blend and depth-stencil states stay bound: the
    /// device keeps those (Wine's test_state_refcounting reads a released rasterizer state back
    /// from real Direct3D 10, and a released sampler as none).
    pub(super) fn unbind_released(&self, address: usize) {
        let was = self.calling10.replace(true);
        let found = {
            let b = self.kept.bound.borrow();
            [
                holds(&b.vs, address),
                holds(&b.ps, address),
                holds(&b.gs, address),
                holds(&b.layout, address),
                holds(&b.index.0, address),
                holds(&b.predication.0, address),
            ]
        };
        let [vs, ps, gs, layout, index, predicate] = found;
        if vs {
            self.vs_set_shader(Ref::default(), std::ptr::null(), 0);
        }
        if ps {
            self.ps_set_shader(Ref::default(), std::ptr::null(), 0);
        }
        if gs {
            self.gs_set_shader(Ref::default(), std::ptr::null(), 0);
        }
        if layout {
            self.ia_set_input_layout(Ref::default());
        }
        if index {
            self.ia_set_index_buffer(Ref::default(), DXGI_FORMAT::default(), 0);
        }
        if predicate {
            let value = self.kept.bound.borrow().predication.1;
            self.set_predication(Ref::default(), value.into());
        }
        let stages = self.kept.bound.borrow().resources.len();
        for stage in 0..stages {
            let found = slots(&self.kept.bound.borrow().resources[stage], address);
            for slot in found {
                self.set_shader_resources(stage, slot, 1, &None);
            }
            let found = slots(&self.kept.bound.borrow().samplers[stage], address);
            for slot in found {
                self.set_samplers(stage, slot, 1, &None);
            }
            let found = slots(&self.kept.bound.borrow().constant_buffers[stage], address);
            for slot in found {
                self.set_constant_buffers(
                    stage,
                    slot,
                    1,
                    &None,
                    std::ptr::null(),
                    std::ptr::null(),
                );
            }
        }
        let vertex: Vec<u32> = {
            let b = self.kept.bound.borrow();
            let buffers: Vec<_> = b.vertex_buffers.iter().map(|v| v.0.clone()).collect();
            slots(&buffers, address)
        };
        for slot in vertex {
            self.ia_set_vertex_buffers(slot, 1, &None, &0, &0);
        }
        let targets = {
            let b = self.kept.bound.borrow();
            let hit = !slots(&b.targets, address).is_empty() || holds(&b.depth, address);
            hit.then(|| {
                let list: Vec<Option<ID3D11RenderTargetView>> = b
                    .targets
                    .iter()
                    .map(|t| {
                        t.as_ref()
                            .filter(|h| h.address() != address)
                            .map(|h| h.public())
                    })
                    .collect();
                let depth: Option<ID3D11DepthStencilView> = b
                    .depth
                    .as_ref()
                    .filter(|h| h.address() != address)
                    .map(|h| h.public());
                (list, depth)
            })
        };
        if let Some((list, depth)) = targets {
            self.om_set_render_targets(list.len() as u32, list.as_ptr(), Ref::from(&depth));
        }
        self.calling10.set(was);
    }

    pub(super) fn so_get_targets(&self, n: u32, out: *mut Option<ID3D11Buffer>) {
        load(&self.seen().so_targets, 0, n, out);
    }

    pub(super) fn draw_auto(&self) {
        self.run("DrawAuto", move |f| {
            f.draw_auto();
            Ok(())
        });
    }

    pub(super) fn ps_set_shader(
        &self,
        shader: Ref<ID3D11PixelShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let h = held_one::<_, PixelShader>(&shader);
        let s = h.as_ref().map(|s| (*s.front).clone());
        self.run("PSSetShader", move |f| {
            f.ps_set_shader(s.clone());
            Ok(())
        });
        self.kept.bound.borrow_mut().ps = h;
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
        // Without either array the buffers are bound whole; with one of them alone the call is
        // invalid and dropped, as Direct3D drops it.
        if first.is_null() != count.is_null() && !list.is_null() {
            self.run("SetConstantBuffers1", |_| {
                Err(wcontainers_direct3d_11::Error::InvalidArg(
                    "constant ranges with one of their two arrays",
                ))
            });
            return;
        }
        let h = held::<_, Buffer>(list, n);
        let b = fronts(&h, |b| &*b.front);
        let ranges: Vec<_> = if first.is_null() || list.is_null() {
            vec![None; h.len()]
        } else {
            (0..h.len())
                .map(|i| unsafe { Some((*first.add(i), *count.add(i))) })
                .collect()
        };
        let r = ranges.clone();
        let kept = self.run("SetConstantBuffers", move |f| {
            f.set_constant_ranges(stage, start, &b, &r)
        });
        if kept {
            let mut bound = self.kept.bound.borrow_mut();
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
        let bound = self.seen();
        for i in 0..n as usize {
            let slot = start as usize + i;
            let range = match (
                bound.constant_buffers[stage].get(slot),
                bound.constant_ranges[stage].get(slot),
            ) {
                (Some(Some(_)), Some(Some(r))) => *r,
                _ => (0, D3D11_REQ_CONSTANT_BUFFER_ELEMENT_COUNT),
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
            &self.seen().constant_buffers[VERTEX],
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
            &self.seen().constant_buffers[PIXEL],
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
        if self.run("SetShaderResources", move |f| {
            f.set_shader_resources(stage, start, &v)
        }) {
            store(&mut self.kept.bound.borrow_mut().resources[stage], start, h);
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
        let kept = self.run("SetSamplers", move |f| f.set_samplers(stage, start, &s));
        if kept {
            store(&mut self.kept.bound.borrow_mut().samplers[stage], start, h);
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

    /// Forgets the shader resources the front unbound: views of a resource now bound as a target or
    /// for unordered access. A deferred context keeps what it was given (the front unbinds them
    /// when its commands run).
    fn forget_unbound_resources(&self) {
        let mut gone = Vec::new();
        if self.is_deferred() {
            // Direct3D's rule on the deferred context's own bindings: no view of a subresource
            // bound as a target or for unordered access is bound for reading.
            let mut bound = self.kept.bound.borrow_mut();
            let Bound {
                resources,
                uavs,
                targets,
                depth,
                ..
            } = &mut *bound;
            let targets: Vec<_> = targets
                .iter()
                .flatten()
                .map(|t| (*t.front).clone())
                .chain(depth.iter().map(|d| (*d.front).clone()))
                .collect();
            for stage in [VERTEX, PIXEL, COMPUTE, GEOMETRY, HULL, DOMAIN] {
                let writer = if stage == COMPUTE { COMPUTE } else { PIXEL };
                for h in resources[stage].iter_mut() {
                    let written = h.as_ref().is_some_and(|v| {
                        let v = &**v.front;
                        v.texture()
                            .is_some_and(|t| targets.iter().any(|x| x.overlaps(t)))
                            || uavs[writer].iter().flatten().any(|u| u.front.overlaps(v))
                    });
                    if written {
                        gone.push(h.take());
                    }
                }
            }
            drop(bound);
            drop(gone);
            return;
        }
        {
            let front = self.front.borrow();
            let mut bound = self.kept.bound.borrow_mut();
            for stage in [VERTEX, PIXEL, COMPUTE, GEOMETRY, HULL, DOMAIN] {
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
        let l = h.as_ref().map(|l| (*l.front).clone());
        self.run("IASetInputLayout", move |f| {
            f.ia_set_input_layout(l.clone());
            Ok(())
        });
        self.kept.bound.borrow_mut().layout = h;
    }

    pub(super) fn ia_set_primitive_topology(&self, topology: D3D_PRIMITIVE_TOPOLOGY) {
        self.run("IASetPrimitiveTopology", move |f| {
            f.ia_set_primitive_topology(topology.0 as u32);
            Ok(())
        });
        self.kept.bound.borrow_mut().topology = topology;
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
        if self.run("IASetVertexBuffers", move |f| {
            f.ia_set_vertex_buffers(start, &slots)
        }) {
            let mut bound = self.kept.bound.borrow_mut();
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
        let b = h.as_ref().map(|b| (*b.front).clone());
        if self.run("IASetIndexBuffer", move |f| {
            f.ia_set_index_buffer(b.clone(), format.0 as u32, offset)
        }) {
            self.kept.bound.borrow_mut().index = (h, format, offset);
        }
    }

    /// `OMSetRenderTargets`: the targets, with every pixel stage unordered access view unbound,
    /// as `OMSetRenderTargetsAndUnorderedAccessViews` binding none.
    pub(super) fn om_set_render_targets(
        &self,
        n: u32,
        list: *const Option<ID3D11RenderTargetView>,
        depth: Ref<ID3D11DepthStencilView>,
    ) {
        self.om_set_render_targets_and_unordered_access_views(
            n,
            list,
            depth,
            n,
            0,
            std::ptr::null(),
            std::ptr::null(),
        );
    }

    fn set_targets(
        &self,
        n: u32,
        list: *const Option<ID3D11RenderTargetView>,
        depth: Ref<ID3D11DepthStencilView>,
    ) {
        let h = held::<_, RenderTargetView>(list, n);
        let targets = fronts(&h, |v| &*v.front);
        let hd = held_one::<_, DepthStencilView>(&depth);
        let d = hd.as_ref().map(|v| (*v.front).clone());
        if self.run("OMSetRenderTargets", move |f| {
            f.om_set_render_targets(&targets, d.clone())
        }) {
            let old = {
                let mut bound = self.kept.bound.borrow_mut();
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
        let f = (!factor.is_null()).then(|| unsafe { *factor.cast::<Color>() });
        let s = h.as_ref().map(|s| (*s.front).clone());
        self.run("OMSetBlendState", move |front| {
            front.om_set_blend_state(s.clone(), f, mask);
            Ok(())
        });
        self.kept.bound.borrow_mut().blend = (h, f.unwrap_or([1.0; COLOR_COMPONENTS]), mask);
    }

    pub(super) fn om_set_depth_stencil_state(
        &self,
        state: Ref<ID3D11DepthStencilState>,
        reference: u32,
    ) {
        let h = held_one::<_, DepthStencilState>(&state);
        let s = h.as_ref().map(|s| (*s.front).clone());
        self.run("OMSetDepthStencilState", move |f| {
            f.om_set_depth_stencil_state(s.clone(), reference);
            Ok(())
        });
        self.kept.bound.borrow_mut().depth_stencil = (h, reference);
    }

    pub(super) fn rs_set_state(&self, state: Ref<ID3D11RasterizerState>) {
        let h = held_one::<_, RasterizerState>(&state);
        let s = h.as_ref().map(|s| (*s.front).clone());
        self.run("RSSetState", move |f| {
            f.rs_set_state(s.clone());
            Ok(())
        });
        self.kept.bound.borrow_mut().raster = h;
    }

    pub(super) fn rs_set_viewports(&self, n: u32, viewports: *const D3D11_VIEWPORT) {
        let list = if viewports.is_null() {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(viewports, n as usize) }
        };
        self.set_viewports(list.to_vec());
    }

    fn set_viewports(&self, list: Vec<D3D11_VIEWPORT>) {
        // More viewports than a pipeline has is invalid: Direct3D keeps the bound ones.
        if list.len() > D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE as usize {
            return;
        }
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
        self.run("RSSetViewports", move |f| {
            f.rs_set_viewports(&v);
            Ok(())
        });
        self.kept.bound.borrow_mut().viewports = list;
    }

    pub(super) fn rs_set_scissor_rects(&self, n: u32, rects: *const RECT) {
        let list = if rects.is_null() {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(rects, n as usize) }
        };
        self.set_scissor_rects(list.to_vec());
    }

    fn set_scissor_rects(&self, list: Vec<RECT>) {
        let r: Vec<Rect> = list
            .iter()
            .map(|r| [r.left, r.top, r.right, r.bottom])
            .collect();
        self.run("RSSetScissorRects", move |f| {
            f.rs_set_scissor_rects(&r);
            Ok(())
        });
        self.kept.bound.borrow_mut().scissors = list;
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
        let c = unsafe { *color.cast::<Color>() };
        self.run("ClearRenderTargetView", move |f| {
            f.clear_render_target_view(&v, c);
            Ok(())
        });
    }

    pub(super) fn generate_mips(&self, view: Ref<ID3D11ShaderResourceView>) {
        // A view of a buffer has no levels; Direct3D drops the call.
        if let Some(v) = front_of(&view, |v: &ShaderResourceView| &*v.front)
            && let Some(v) = v.texture().cloned()
        {
            self.run("GenerateMips", move |f| {
                f.generate_mips(&v);
                Ok(())
            });
        }
    }

    /// `SetResourceMinLOD`: the clamp lives on the texture, read by each later draw that samples
    /// it. Direct3D takes it only on the immediate context and only for textures made with
    /// `D3D11_RESOURCE_MISC_RESOURCE_CLAMP`; it drops the rest.
    pub(super) fn set_resource_min_lod(&self, r: Ref<ID3D11Resource>, lod: f32) {
        if self.is_deferred() {
            return;
        }
        if let Some(Resource::Texture(t)) = r.as_ref().and_then(resource_of)
            && t.desc.misc_flags & wcontainers_direct3d_11::desc::MISC_RESOURCE_CLAMP != 0
        {
            t.min_lod.set(lod);
        }
    }

    pub(super) fn get_resource_min_lod(&self, r: Ref<ID3D11Resource>) -> f32 {
        match r.as_ref().and_then(resource_of) {
            Some(Resource::Texture(t)) => t.min_lod.get(),
            _ => 0.0,
        }
    }

    pub(super) fn set_predication(&self, p: Ref<ID3D11Predicate>, value: windows::core::BOOL) {
        let h = held_one::<_, Predicate>(&p);
        let q = h.as_ref().map(|p| p.front.clone());
        let value = value.as_bool();
        self.run("SetPredication", move |f| {
            f.set_predication(q.clone(), value);
            Ok(())
        });
        self.kept.bound.borrow_mut().predication = (h, value);
    }

    pub(super) fn get_predication(
        &self,
        out: OutRef<ID3D11Predicate>,
        value: *mut windows::core::BOOL,
    ) {
        let bound = self.seen();
        let _ = out.write(public(&bound.predication.0));
        if let Some(v) = unsafe { value.as_mut() } {
            *v = bound.predication.1.into();
        }
    }

    pub(super) fn begin(&self, a: Ref<ID3D11Asynchronous>) {
        if let Some(q) = query_of(a.as_ref()) {
            self.run("Begin", move |f| {
                f.begin_query(&q);
                Ok(())
            });
        }
    }

    pub(super) fn end(&self, a: Ref<ID3D11Asynchronous>) {
        if let Some(q) = query_of(a.as_ref()) {
            self.run("End", move |f| {
                f.end_query(&q);
                Ok(())
            });
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
        if self.is_deferred() {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
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
                    let mut bytes = [0u8; size_of::<Answer>()];
                    for (b, w) in bytes.chunks_mut(size_of::<u64>()).zip(v) {
                        b.copy_from_slice(&w.to_le_bytes());
                    }
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
        self.run("ClearDepthStencilView", move |f| {
            f.clear_depth_stencil_view(&v, flags, depth, stencil);
            Ok(())
        });
    }

    pub(super) fn draw(&self, count: u32, start: u32) {
        self.run("Draw", move |f| {
            f.draw(count, start);
            Ok(())
        });
    }

    pub(super) fn draw_indexed(&self, count: u32, start: u32, base: i32) {
        self.run("DrawIndexed", move |f| {
            f.draw_indexed(count, start, base);
            Ok(())
        });
    }

    pub(super) fn draw_instanced(
        &self,
        count: u32,
        instances: u32,
        start: u32,
        first_instance: u32,
    ) {
        self.run("DrawInstanced", move |f| {
            f.draw_instanced(count, instances, start, first_instance);
            Ok(())
        });
    }

    pub(super) fn draw_indexed_instanced(
        &self,
        count: u32,
        instances: u32,
        start: u32,
        base: i32,
        first_instance: u32,
    ) {
        self.run("DrawIndexedInstanced", move |f| {
            f.draw_indexed_instanced(count, instances, start, base, first_instance);
            Ok(())
        });
    }

    pub(super) fn copy_resource(&self, dst: Ref<ID3D11Resource>, src: Ref<ID3D11Resource>) {
        let (Some(d), Some(s)) = (
            dst.as_ref().and_then(resource_of),
            src.as_ref().and_then(resource_of),
        ) else {
            return;
        };
        self.run("CopyResource", move |f| f.copy_resource(&d, &s));
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
        self.run("CopySubresourceRegion", move |f| {
            f.copy_subresource_region(&d, dst_sub, [x, y, z], &s, src_sub, b.as_ref())
        });
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

    pub(super) fn resolve_subresource(
        &self,
        dst: Ref<ID3D11Resource>,
        dst_sub: u32,
        src: Ref<ID3D11Resource>,
        src_sub: u32,
        format: DXGI_FORMAT,
    ) {
        let (Some(d), Some(s)) = (
            dst.as_ref().and_then(resource_of),
            src.as_ref().and_then(resource_of),
        ) else {
            return;
        };
        self.run("ResolveSubresource", move |f| {
            f.resolve_subresource(&d, dst_sub, &s, src_sub, format.0 as u32)
        });
    }

    /// Discards leave contents undefined; keeping them is one of the contents allowed.
    pub(super) fn discard_resource(&self, _resource: Ref<ID3D11Resource>) {}

    pub(super) fn discard_view(&self, _view: Ref<ID3D11View>) {}

    pub(super) fn discard_view1(&self, _view: Ref<ID3D11View>, _rects: *const RECT, _n: u32) {}

    /// `ClearView` of a render target, storage or depth view, whole or its rectangles; video
    /// views are not served.
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
        let c = unsafe { *color.cast::<Color>() };
        let rects: Vec<Rect> = if n == 0 || rects.is_null() {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(rects, n as usize) }
                .iter()
                .map(|r| [r.left, r.top, r.right, r.bottom])
                .collect()
        };
        if let Some(v) = front_of(&view, |v: &UnorderedAccessView| &*v.front) {
            self.run(CLEAR_VIEW, move |f| f.clear_view_uav(&v, c, &rects));
            return;
        }
        if let Some(v) = front_of(&view, |v: &DepthStencilView| &*v.front) {
            self.run(CLEAR_VIEW, move |f| f.clear_view_depth(&v, c[0], &rects));
            return;
        }
        let Some(v) = front_of(&view, |v: &RenderTargetView| &*v.front) else {
            return self.unserved(CLEAR_VIEW_VIDEO);
        };
        self.run(CLEAR_VIEW, move |f| {
            if rects.is_empty() {
                f.clear_render_target_view(&v, c);
                Ok(())
            } else {
                f.clear_view_rects(&v, c, &rects)
            }
        });
    }

    /// `ID3D11Multithread::Enter`: the library's lock, held until `Leave`.
    pub(super) fn enter(&self) {
        std::mem::forget(lock());
        let entered = &self.kept.entered;
        entered.set(entered.get() + 1);
    }

    pub(super) fn leave(&self) {
        let entered = &self.kept.entered;
        if entered.get() > 0 {
            entered.set(entered.get() - 1);
            // SAFETY: this thread holds the lock taken by its `Enter`, and the call's own above it.
            unsafe { unlock() };
        }
    }

    /// Calls are serialised by the library's lock whatever the flag says; it is kept and reported.
    pub(super) fn set_multithread_protected(&self, on: BOOL) -> BOOL {
        self.kept.protected.replace(on.as_bool()).into()
    }

    pub(super) fn get_multithread_protected(&self) -> BOOL {
        self.kept.protected.get().into()
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

    pub(super) fn is_annotation_enabled(&self) -> BOOL {
        self.get_status()
    }

    pub(super) fn set_marker_int(&self, _name: &PCWSTR, _data: i32) {}

    pub(super) fn begin_event_int(&self, _name: &PCWSTR, _data: i32) {}

    /// `Flush1`: one queue runs every kind of work; an event given is signalled once the GPU has
    /// done what was flushed.
    pub(super) fn flush1(
        &self,
        _kind: D3D11_CONTEXT_TYPE,
        event: windows::Win32::Foundation::HANDLE,
    ) {
        if self.is_deferred() {
            return;
        }
        if event.is_invalid() {
            self.front.borrow_mut().flush();
            super::sharing::flushed(&self.front);
        } else {
            self.front.borrow_mut().finish();
            super::sharing::flushed(&self.front);
            let _ = unsafe { windows::Win32::System::Threading::SetEvent(event) };
        }
    }

    /// No protected content is served, so protection is never on.
    pub(super) fn get_hardware_protection_state(&self, out: *mut BOOL) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = false.into();
        }
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
                let (row, rows) = format::subresource_rows(t.desc.format, t.format, w, h);
                if z == 0 || rows == 0 {
                    0
                } else {
                    (depth_pitch * (z - 1) + row_pitch * (rows - 1) + row) as usize
                }
            }
        };
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) };
        if self.is_deferred() {
            let bytes = bytes.to_vec();
            self.run("UpdateSubresource", move |f| {
                f.update_subresource(&d, sub, b.as_ref(), &bytes, row_pitch, depth_pitch)
            });
        } else {
            let mut front = self.front.borrow_mut();
            let r = front.update_subresource(&d, sub, b.as_ref(), bytes, row_pitch, depth_pitch);
            settle(&mut front, "UpdateSubresource", r);
        }
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
                if self.is_deferred() {
                    self.map_deferred(&r, sub, kind.0 as u32)
                } else {
                    self.front
                        .borrow_mut()
                        .map(&r, sub, kind.0 as u32, flags)
                        .map_err(hresult)
                }
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
        let Some(r) = resource.as_ref().and_then(resource_of) else {
            return;
        };
        if !self.is_deferred() {
            self.front.borrow_mut().unmap(&r, sub);
            return;
        }
        // The bytes written go into the resource, through a map of the immediate context, when
        // the command list runs.
        let Some((bytes, row_pitch, depth_pitch, (row, rows, slices))) = self
            .staged
            .borrow()
            .get(&(address(&r), sub))
            .map(|s| (s.bytes.clone(), s.row_pitch, s.depth_pitch, s.rows))
        else {
            return;
        };
        self.run("Unmap", move |f| {
            let m = f.map(&r, sub, map::WRITE_DISCARD, 0)?;
            for z in 0..slices {
                for y in 0..rows {
                    let from = (z * depth_pitch + y * row_pitch) as usize;
                    let to = (z * m.depth_pitch + y * m.row_pitch) as usize;
                    // SAFETY: the map hands out its subresource's rows at its pitches.
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            bytes[from..][..row as usize].as_ptr(),
                            m.data.add(to),
                            row as usize,
                        )
                    };
                }
            }
            f.unmap(&r, sub);
            Ok(())
        });
    }

    /// A deferred context's `Map`: of dynamic resources only, discarding (or, after a discard
    /// in the same command list, not overwriting) into bytes of the context's own.
    fn map_deferred(
        &self,
        r: &Resource<Chosen>,
        sub: u32,
        kind: u32,
    ) -> Result<wcontainers_direct3d_11::Mapped> {
        let (usage, rows) = match r {
            Resource::Buffer(b) if sub == 0 => (b.desc.usage, (b.desc.byte_width, 1, 1)),
            Resource::Texture(t) if sub < t.desc.mip_levels * t.desc.array_size => {
                let e = t.mip_extent(sub % t.desc.mip_levels);
                let (row, rows, _) = t.packed(sub % t.desc.mip_levels);
                (t.desc.usage, (row, rows, e.depth_or_layers))
            }
            _ => return Err(E_INVALIDARG.into()),
        };
        if usage != wcontainers_direct3d_11::desc::usage::DYNAMIC {
            return Err(E_INVALIDARG.into());
        }
        let mut staged = self.staged.borrow_mut();
        let key = (address(r), sub);
        let s = match kind {
            map::WRITE_DISCARD => staged.entry(key).insert_entry(Staging {
                bytes: vec![0; (rows.0 * rows.1 * rows.2) as usize],
                row_pitch: rows.0,
                depth_pitch: rows.0 * rows.1,
                rows,
            }),
            map::WRITE_NO_OVERWRITE => match staged.entry(key) {
                std::collections::hash_map::Entry::Occupied(s) => s,
                std::collections::hash_map::Entry::Vacant(_) => {
                    return Err(MAP_WITHOUT_INITIAL_DISCARD.into());
                }
            },
            _ => return Err(E_INVALIDARG.into()),
        };
        let s = s.into_mut();
        Ok(wcontainers_direct3d_11::Mapped {
            data: s.bytes.as_mut_ptr(),
            row_pitch: s.row_pitch,
            depth_pitch: s.depth_pitch,
        })
    }

    pub(super) fn clear_state(&self) {
        self.run("ClearState", |f| {
            f.clear_state();
            Ok(())
        });
        // The old bindings are released after the borrow ends: their drops may reach the context.
        let old = std::mem::take(&mut *self.kept.bound.borrow_mut());
        drop(old);
    }

    /// A deferred context's flush does nothing: its work goes when its command list runs.
    pub(super) fn flush(&self) {
        if !self.is_deferred() {
            self.front.borrow_mut().flush();
            super::sharing::flushed(&self.front);
        }
    }

    pub(super) fn cs_set_shader(
        &self,
        shader: Ref<ID3D11ComputeShader>,
        _c: *const Option<ID3D11ClassInstance>,
        _n: u32,
    ) {
        let h = held_one::<_, ComputeShader>(&shader);
        let s = h.as_ref().map(|s| (*s.front).clone());
        self.run("CSSetShader", move |f| {
            f.cs_set_shader(s.clone());
            Ok(())
        });
        self.kept.bound.borrow_mut().cs = h;
    }

    pub(super) fn cs_get_shader(
        &self,
        out: OutRef<ID3D11ComputeShader>,
        _c: OutRef<ID3D11ClassInstance>,
        count: *mut u32,
    ) {
        no_instances(count);
        let _ = out.write(public(&self.seen().cs));
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
        load(&self.seen().constant_buffers[COMPUTE], start, n, out);
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
            &self.seen().constant_buffers[COMPUTE],
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
        load(&self.seen().resources[COMPUTE], start, n, out);
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
        load(&self.seen().samplers[COMPUTE], start, n, out);
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
        let counts = (!counts.is_null())
            .then(|| unsafe { std::slice::from_raw_parts(counts, h.len()) }.to_vec());
        if self.run("SetUnorderedAccessViews", move |f| {
            f.set_uavs(stage, start, &v, counts.as_deref())
        }) {
            store(&mut self.kept.bound.borrow_mut().uavs[stage], start, h);
            self.forget_unbound_resources();
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
        load(&self.seen().uavs[COMPUTE], start, n, out);
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
            self.set_targets(n, targets, depth);
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
        load(&self.seen().uavs[PIXEL], uav_start, uav_n, uavs);
    }

    pub(super) fn dispatch(&self, x: u32, y: u32, z: u32) {
        self.run("Dispatch", move |f| {
            f.dispatch([x, y, z], None);
            Ok(())
        });
    }

    pub(super) fn draw_instanced_indirect(&self, args: Ref<ID3D11Buffer>, offset: u32) {
        self.draw_indirect("DrawInstancedIndirect", args, offset, false);
    }

    pub(super) fn draw_indexed_instanced_indirect(&self, args: Ref<ID3D11Buffer>, offset: u32) {
        self.draw_indirect("DrawIndexedInstancedIndirect", args, offset, true);
    }

    fn draw_indirect(
        &self,
        what: &'static str,
        args: Ref<ID3D11Buffer>,
        offset: u32,
        indexed: bool,
    ) {
        let Some(b) = front_of(&args, |b: &Buffer| &*b.front) else {
            return;
        };
        self.run(what, move |f| f.draw_indirect(&b, offset, indexed));
    }

    pub(super) fn dispatch_indirect(&self, args: Ref<ID3D11Buffer>, offset: u32) {
        let Some(b) = front_of(&args, |b: &Buffer| &*b.front) else {
            return;
        };
        self.run("DispatchIndirect", move |f| {
            f.dispatch(Default::default(), Some((&b, offset)));
            Ok(())
        });
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
        let values = unsafe { *values.cast::<[u32; COLOR_COMPONENTS]>() };
        self.run("ClearUnorderedAccessViewUint", move |f| {
            f.clear_uav(&v, values, false)
        });
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
        let values = unsafe { *values.cast::<Color>() }.map(f32::to_bits);
        self.run("ClearUnorderedAccessViewFloat", move |f| {
            f.clear_uav(&v, values, true)
        });
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
        self.run("CopyStructureCount", move |f| {
            f.copy_structure_count(&b, offset, &v)
        });
    }

    pub(super) fn vs_get_shader(
        &self,
        out: OutRef<ID3D11VertexShader>,
        _c: OutRef<ID3D11ClassInstance>,
        count: *mut u32,
    ) {
        no_instances(count);
        let _ = out.write(public(&self.seen().vs));
    }

    pub(super) fn ps_get_shader(
        &self,
        out: OutRef<ID3D11PixelShader>,
        _c: OutRef<ID3D11ClassInstance>,
        count: *mut u32,
    ) {
        no_instances(count);
        let _ = out.write(public(&self.seen().ps));
    }

    pub(super) fn vs_get_constant_buffers(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11Buffer>,
    ) {
        load(&self.seen().constant_buffers[VERTEX], start, n, out);
    }

    pub(super) fn ps_get_constant_buffers(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11Buffer>,
    ) {
        load(&self.seen().constant_buffers[PIXEL], start, n, out);
    }

    pub(super) fn vs_get_shader_resources(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11ShaderResourceView>,
    ) {
        load(&self.seen().resources[VERTEX], start, n, out);
    }

    pub(super) fn ps_get_shader_resources(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11ShaderResourceView>,
    ) {
        load(&self.seen().resources[PIXEL], start, n, out);
    }

    pub(super) fn vs_get_samplers(&self, start: u32, n: u32, out: *mut Option<ID3D11SamplerState>) {
        load(&self.seen().samplers[VERTEX], start, n, out);
    }

    pub(super) fn ps_get_samplers(&self, start: u32, n: u32, out: *mut Option<ID3D11SamplerState>) {
        load(&self.seen().samplers[PIXEL], start, n, out);
    }

    pub(super) fn ia_get_input_layout(&self, out: OutRef<ID3D11InputLayout>) {
        let _ = out.write(public(&self.seen().layout));
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
        let bound = self.seen();
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
            let bound = self.seen();
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
            *out = self.seen().topology;
        }
    }

    pub(super) fn om_get_render_targets(
        &self,
        n: u32,
        targets: *mut Option<ID3D11RenderTargetView>,
        depth: OutRef<ID3D11DepthStencilView>,
    ) {
        let bound = self.seen();
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
            let bound = self.seen();
            (public(&bound.blend.0), bound.blend.1, bound.blend.2)
        };
        let _ = out.write(b);
        unsafe {
            if !factor.is_null() {
                factor.cast::<Color>().write(f);
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
            let bound = self.seen();
            (public(&bound.depth_stencil.0), bound.depth_stencil.1)
        };
        let _ = out.write(d);
        if let Some(reference) = unsafe { reference.as_mut() } {
            *reference = r;
        }
    }

    pub(super) fn rs_get_state(&self, out: OutRef<ID3D11RasterizerState>) {
        let _ = out.write(public(&self.seen().raster));
    }

    pub(super) fn rs_get_viewports(&self, count: *mut u32, out: *mut D3D11_VIEWPORT) {
        get_list(&self.seen().viewports, count, out);
    }

    pub(super) fn rs_get_scissor_rects(&self, count: *mut u32, out: *mut RECT) {
        get_list(&self.seen().scissors, count, out);
    }

    /// A call on a stage or feature the front does not serve, which is served while it binds nothing.
    pub(super) fn gs_set_constant_buffers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        self.set_constant_buffers(GEOMETRY, start, n, list, std::ptr::null(), std::ptr::null());
    }

    pub(super) fn gs_set_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
        first: *const u32,
        count: *const u32,
    ) {
        self.set_constant_buffers(GEOMETRY, start, n, list, first, count);
    }

    pub(super) fn gs_get_constant_buffers(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11Buffer>,
    ) {
        load(&self.seen().constant_buffers[GEOMETRY], start, n, out);
    }

    pub(super) fn gs_get_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        out: OutRef<ID3D11Buffer>,
        first: *mut u32,
        count: *mut u32,
    ) {
        load(
            &self.seen().constant_buffers[GEOMETRY],
            start,
            n,
            out_array(out),
        );
        self.get_constant_ranges(GEOMETRY, start, n, first, count);
    }

    pub(super) fn gs_set_samplers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11SamplerState>,
    ) {
        self.set_samplers(GEOMETRY, start, n, list);
    }

    pub(super) fn gs_get_samplers(&self, start: u32, n: u32, out: *mut Option<ID3D11SamplerState>) {
        load(&self.seen().samplers[GEOMETRY], start, n, out);
    }

    pub(super) fn hs_set_constant_buffers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        self.set_constant_buffers(HULL, start, n, list, std::ptr::null(), std::ptr::null());
    }

    pub(super) fn hs_set_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
        first: *const u32,
        count: *const u32,
    ) {
        self.set_constant_buffers(HULL, start, n, list, first, count);
    }

    pub(super) fn hs_get_constant_buffers(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11Buffer>,
    ) {
        load(&self.seen().constant_buffers[HULL], start, n, out);
    }

    pub(super) fn hs_get_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        out: OutRef<ID3D11Buffer>,
        first: *mut u32,
        count: *mut u32,
    ) {
        load(
            &self.seen().constant_buffers[HULL],
            start,
            n,
            out_array(out),
        );
        self.get_constant_ranges(HULL, start, n, first, count);
    }

    pub(super) fn hs_set_samplers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11SamplerState>,
    ) {
        self.set_samplers(HULL, start, n, list);
    }

    pub(super) fn hs_get_samplers(&self, start: u32, n: u32, out: *mut Option<ID3D11SamplerState>) {
        load(&self.seen().samplers[HULL], start, n, out);
    }

    pub(super) fn ds_set_constant_buffers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
    ) {
        self.set_constant_buffers(DOMAIN, start, n, list, std::ptr::null(), std::ptr::null());
    }

    pub(super) fn ds_set_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11Buffer>,
        first: *const u32,
        count: *const u32,
    ) {
        self.set_constant_buffers(DOMAIN, start, n, list, first, count);
    }

    pub(super) fn ds_get_constant_buffers(
        &self,
        start: u32,
        n: u32,
        out: *mut Option<ID3D11Buffer>,
    ) {
        load(&self.seen().constant_buffers[DOMAIN], start, n, out);
    }

    pub(super) fn ds_get_constant_buffers1(
        &self,
        start: u32,
        n: u32,
        out: OutRef<ID3D11Buffer>,
        first: *mut u32,
        count: *mut u32,
    ) {
        load(
            &self.seen().constant_buffers[DOMAIN],
            start,
            n,
            out_array(out),
        );
        self.get_constant_ranges(DOMAIN, start, n, first, count);
    }

    pub(super) fn ds_set_samplers(
        &self,
        start: u32,
        n: u32,
        list: *const Option<ID3D11SamplerState>,
    ) {
        self.set_samplers(DOMAIN, start, n, list);
    }

    pub(super) fn ds_get_samplers(&self, start: u32, n: u32, out: *mut Option<ID3D11SamplerState>) {
        load(&self.seen().samplers[DOMAIN], start, n, out);
    }

    pub(super) fn get_type(&self) -> D3D11_DEVICE_CONTEXT_TYPE {
        if self.is_deferred() {
            D3D11_DEVICE_CONTEXT_DEFERRED
        } else {
            D3D11_DEVICE_CONTEXT_IMMEDIATE
        }
    }

    pub(super) fn get_context_flags(&self) -> u32 {
        0
    }

    /// `SwapDeviceContextState`: the immediate context's bindings go into the state object in it
    /// (one holding the device's first state, the first time) and `state`'s are bound. A
    /// deferred context swaps nothing and hands back none.
    pub(super) fn swap_device_context_state(
        &self,
        state: Ref<ID3DDeviceContextState>,
        previous: OutRef<ID3DDeviceContextState>,
    ) {
        let incoming = ours::<_, DeviceContextState>(state.as_ref()).map(Held::new);
        let Some(incoming) = incoming.filter(|_| !self.is_deferred()) else {
            let _ = previous.write(if self.is_deferred() {
                None
            } else {
                self.kept.state.borrow().as_ref().map(Held::public)
            });
            return;
        };
        let current = self.kept.state.borrow_mut().take().unwrap_or_else(|| {
            let first: ID3DDeviceContextState =
                DeviceContextState::new(self.child.device().clone(), false).into();
            Held::new(ours::<_, DeviceContextState>(Some(&first)).expect("ours"))
        });
        {
            let mut bound = self.kept.bound.borrow_mut();
            std::mem::swap(&mut *bound, &mut *current.bound.borrow_mut());
            std::mem::swap(&mut *bound, &mut *incoming.bound.borrow_mut());
        }
        *self.kept.state.borrow_mut() = Some(incoming);
        let _ = previous.write(Some(current.public()));
        self.run("ClearState", |f| {
            f.clear_state();
            Ok(())
        });
        self.reapply();
    }

    /// `FinishCommandList`: what the deferred context recorded becomes a command list. Its next
    /// list starts from the default state, or from the state it has now if `restore` is true.
    pub(super) fn finish_command_list(
        &self,
        restore: BOOL,
        out: OutRef<ID3D11CommandList>,
    ) -> Result<()> {
        let Some(recorded) = &self.recorded else {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        };
        let commands = std::mem::take(&mut *recorded.borrow_mut());
        self.staged.borrow_mut().clear();
        self.after_list(restore.as_bool());
        // Without an out pointer the recorded calls are dropped all the same. A list is made only
        // to be handed out: its device reference is given back by its last release.
        if out.is_null() {
            return Err(windows::Win32::Foundation::E_UNEXPECTED.into());
        }
        let list = CommandList::new(Rc::new(commands), self.child.device().clone());
        out.write(Some(list.into()))
    }

    /// `ExecuteCommandList`: the list's calls run (on a deferred context, are recorded), from the
    /// default state; then the state is the one before, or the default if `restore` is false.
    pub(super) fn execute_command_list(&self, list: Ref<ID3D11CommandList>, restore: BOOL) {
        let Some(commands) = front_of(&list, |l: &CommandList| &*l.commands) else {
            return;
        };
        match &self.recorded {
            None => {
                let mut front = self.front.borrow_mut();
                for c in commands.iter() {
                    c(&mut front);
                }
            }
            Some(recorded) => recorded.borrow_mut().push(Box::new(move |front| {
                for c in commands.iter() {
                    c(front);
                }
            })),
        }
        self.after_list(restore.as_bool());
    }

    /// The state after a command list is made or run: the context's own again if `restore`,
    /// otherwise the default.
    fn after_list(&self, restore: bool) {
        self.run("ClearState", |f| {
            f.clear_state();
            Ok(())
        });
        if restore {
            self.reapply();
        } else {
            let old = std::mem::take(&mut *self.kept.bound.borrow_mut());
            drop(old);
        }
    }

    /// Binds again everything the context holds, on a front whose state was cleared.
    fn reapply(&self) {
        let (viewports, scissors) = {
            let b = self.kept.bound.borrow();
            let vs = b.vs.as_ref().map(|s| (*s.front).clone());
            let ps = b.ps.as_ref().map(|s| (*s.front).clone());
            let cs = b.cs.as_ref().map(|s| (*s.front).clone());
            let gs = b.gs.as_ref().map(|s| (*s.front).clone());
            let hs = b.hs.as_ref().map(|s| (*s.front).clone());
            let ds = b.ds.as_ref().map(|s| (*s.front).clone());
            self.run("VSSetShader", move |f| {
                f.vs_set_shader(vs.clone());
                f.ps_set_shader(ps.clone());
                f.cs_set_shader(cs.clone());
                f.gs_set_shader(gs.clone());
                f.hs_set_shader(hs.clone());
                f.ds_set_shader(ds.clone());
                Ok(())
            });
            let targets = fronts(&b.so_targets, |t| &*t.front);
            self.run("SOSetTargets", move |f| {
                f.so_set_targets(&targets, &[u32::MAX; SO_SLOTS])
            });
            let layout = b.layout.as_ref().map(|l| (*l.front).clone());
            let topology = b.topology.0 as u32;
            let slots: Vec<_> = b
                .vertex_buffers
                .iter()
                .map(|(h, stride, offset)| {
                    h.as_ref().map(|h| ((*h.front).clone(), *stride, *offset))
                })
                .collect();
            let index = b.index.0.as_ref().map(|h| (*h.front).clone());
            let (format, offset) = (b.index.1.0 as u32, b.index.2);
            self.run("IASetVertexBuffers", move |f| {
                f.ia_set_input_layout(layout.clone());
                f.ia_set_primitive_topology(topology);
                f.ia_set_vertex_buffers(0, &slots)?;
                f.ia_set_index_buffer(index.clone(), format, offset)
            });
            let targets = fronts(&b.targets, |v| &*v.front);
            let depth = b.depth.as_ref().map(|v| (*v.front).clone());
            self.run("OMSetRenderTargets", move |f| {
                f.om_set_render_targets(&targets, depth.clone())
            });
            for stage in [VERTEX, PIXEL, COMPUTE, GEOMETRY, HULL, DOMAIN] {
                let buffers = fronts(&b.constant_buffers[stage], |c| &*c.front);
                let ranges: Vec<_> = (0..buffers.len())
                    .map(|i| b.constant_ranges[stage].get(i).copied().flatten())
                    .collect();
                let views = fronts(&b.resources[stage], |v| &*v.front);
                let samplers = fronts(&b.samplers[stage], |s| &*s.front);
                let uavs = b
                    .uavs
                    .get(stage)
                    .map_or(Vec::new(), |u| fronts(u, |u| &*u.front));
                self.run("SetShaderResources", move |f| {
                    if stage == PIXEL || stage == COMPUTE {
                        f.set_uavs(stage, 0, &uavs, None)?;
                    }
                    f.set_constant_ranges(stage, 0, &buffers, &ranges)?;
                    f.set_shader_resources(stage, 0, &views)?;
                    f.set_samplers(stage, 0, &samplers)
                });
            }
            let blend = b.blend.0.as_ref().map(|s| (*s.front).clone());
            let (factor, mask) = (b.blend.1, b.blend.2);
            let depth_stencil = b.depth_stencil.0.as_ref().map(|s| (*s.front).clone());
            let reference = b.depth_stencil.1;
            let raster = b.raster.as_ref().map(|s| (*s.front).clone());
            let predicate = b.predication.0.as_ref().map(|p| p.front.clone());
            let value = b.predication.1;
            self.run("SetPredication", move |f| {
                f.set_predication(predicate.clone(), value);
                Ok(())
            });
            self.run("OMSetBlendState", move |f| {
                f.om_set_blend_state(blend.clone(), Some(factor), mask);
                f.om_set_depth_stencil_state(depth_stencil.clone(), reference);
                f.rs_set_state(raster.clone());
                Ok(())
            });
            (b.viewports.clone(), b.scissors.clone())
        };
        self.set_viewports(viewports);
        self.set_scissor_rects(scissors);
    }
}

/// The program's array behind an out parameter the windows crate types as one object.
pub(super) fn out_array<T: Interface>(out: OutRef<T>) -> *mut Option<T> {
    // SAFETY: `OutRef` is a transparent pointer to the first element.
    unsafe { std::mem::transmute(out) }
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
    // Entries past those bound read zero; the count becomes the number written that are bound.
    for i in 0..*count as usize {
        unsafe { out.add(i).write(list.get(i).copied().unwrap_or_default()) };
    }
    *count = (*count).min(list.len() as u32);
}

/// A resource's address, which names it among mapped subresources.
fn address(r: &Resource<Chosen>) -> usize {
    match r {
        Resource::Buffer(b) => Rc::as_ptr(b) as usize,
        Resource::Texture(t) => Rc::as_ptr(t) as usize,
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

const CLEAR_VIEW: &str = "ClearView";
const CLEAR_VIEW_VIDEO: &str =
    "ID3D11DeviceContext1::ClearView of a view not of a target, storage or depth (a video view)";
