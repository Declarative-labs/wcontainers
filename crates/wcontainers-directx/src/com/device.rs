//! The device: `ID3D11Device3` and `IDXGIDevice3` on one object.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::Win32::Graphics::Dxgi::{
    DXGI_OFFER_RESOURCE_PRIORITY, IDXGIAdapter, IDXGIAdapter1, IDXGIDevice3, IDXGIResource,
};
use windows::core::{
    ComObjectInner, ComObjectInterface, GUID, IUnknownImpl, Interface, OutRef, Ref, Result,
    implement,
};

use wcontainers_direct3d_11::desc::{self, InputElement, SubresourceData, TextureDesc};
use wcontainers_direct3d_11::format;
use wcontainers_gpu::SourceStage;

use super::counted::{Counted, Held};
use super::*;

#[implement(ID3D11Device3, IDXGIDevice3)]
pub struct Device {
    pub front: Locked<Shared>,
    pub(super) private: Private,
    /// The immediate context while it lives, held without a reference (it holds one on the device).
    context: Locked<Rc<Cell<*mut c_void>>>,
    adapter: IDXGIAdapter1,
    flags: u32,
    exception_mode: Cell<u32>,
    /// DXGI's limit of frames queued for presenting, as the program set it.
    frame_latency: Cell<u32>,
    /// The live state objects by description: Direct3D hands out the existing object for a
    /// description equal to a live one's.
    states: States,
}

/// The state objects made, by description, each held with a library reference for the device's
/// life (Direct3D hands out the existing object for a description equal to one it has made).
#[derive(Default)]
struct States {
    blend: RefCell<HashMap<Vec<u32>, Held<BlendState_Impl>>>,
    depth_stencil: RefCell<HashMap<Vec<u32>, Held<DepthStencilState_Impl>>>,
    raster: RefCell<HashMap<Vec<u32>, Held<RasterizerState_Impl>>>,
    sampler: RefCell<HashMap<Vec<u32>, Held<SamplerState_Impl>>>,
}

/// The state object of description `key` in `made`, or a new one from `make`, recorded.
fn shared_state<T, I>(
    made: &RefCell<HashMap<Vec<u32>, Held<T::Outer>>>,
    key: Vec<u32>,
    out: OutRef<I>,
    make: impl FnOnce() -> Result<I>,
) -> Result<()>
where
    T: ComObjectInner,
    T::Outer: Counted + ComObjectInterface<I> + std::any::Any,
    I: Interface + windows::core::Type<I, Default = Option<I>>,
{
    let found = made.borrow().get(&key).map(Held::public);
    if let Some(o) = found {
        return give(out, o);
    }
    let o = make()?;
    let held = Held::new(o.cast_object_ref::<T>()?);
    made.borrow_mut().insert(key, held);
    give(out, o)
}

/// A description's fields as words, for comparing descriptions without their padding.
fn words<T: Copy>(d: &T) -> Vec<u32> {
    assert!(size_of::<T>().is_multiple_of(4));
    let p = (d as *const T).cast::<u32>();
    (0..size_of::<T>() / 4)
        .map(|i| unsafe { p.add(i).read_unaligned() })
        .collect()
}

fn blend_key(d: &D3D11_BLEND_DESC) -> Vec<u32> {
    let mut k = vec![
        d.AlphaToCoverageEnable.0 as u32,
        d.IndependentBlendEnable.0 as u32,
    ];
    for t in &d.RenderTarget {
        k.extend([
            t.BlendEnable.0 as u32,
            t.SrcBlend.0 as u32,
            t.DestBlend.0 as u32,
            t.BlendOp.0 as u32,
            t.SrcBlendAlpha.0 as u32,
            t.DestBlendAlpha.0 as u32,
            t.BlendOpAlpha.0 as u32,
            u32::from(t.RenderTargetWriteMask),
        ]);
    }
    k
}

fn depth_stencil_key(d: &D3D11_DEPTH_STENCIL_DESC) -> Vec<u32> {
    let mut k = vec![
        d.DepthEnable.0 as u32,
        d.DepthWriteMask.0 as u32,
        d.DepthFunc.0 as u32,
        d.StencilEnable.0 as u32,
        u32::from(d.StencilReadMask),
        u32::from(d.StencilWriteMask),
    ];
    for f in [&d.FrontFace, &d.BackFace] {
        k.extend(words(f));
    }
    k
}

/// Makes a device and its immediate context at the first of `levels` served.
pub fn create_device(
    levels: &[u32],
    flags: u32,
) -> Result<(ID3D11Device, ID3D11DeviceContext, u32)> {
    let _lock = lock();
    super::cache::open_once();
    let backend = backend().ok_or(windows::Win32::Graphics::Dxgi::DXGI_ERROR_UNSUPPORTED)?;
    let front = Front::new(backend, levels).map_err(hresult)?;
    let adapter = Adapter::new(wcontainers_gpu::Backend::caps(&front.backend));
    let level = front.feature_level();
    let device: ID3D11Device3 = Device {
        front: Locked::new(Rc::new(RefCell::new(front))),
        private: Private::default(),
        context: Locked::new(Rc::new(Cell::new(std::ptr::null_mut()))),
        adapter: adapter.into(),
        flags,
        exception_mode: Cell::new(0),
        frame_latency: Cell::new(3),
        states: States::default(),
    }
    .into();
    // The object's table of ID3D11Device3 is ours to point elsewhere.
    unsafe {
        let table = device.as_raw() as *mut *const ID3D11Device3_Vtbl;
        *table = patched_table(*table);
    }
    let context = unsafe { device.GetImmediateContext() }?;
    Ok((device.into(), context, level))
}

/// The windows crate's `CheckMultisampleQualityLevels` and `CheckFormatSupport` entries, which
/// write their answer only on success and never check the pointer.
type QualityLevels =
    unsafe extern "system" fn(*mut c_void, DXGI_FORMAT, u32, *mut u32) -> windows::core::HRESULT;
type FormatSupport =
    unsafe extern "system" fn(*mut c_void, DXGI_FORMAT, *mut u32) -> windows::core::HRESULT;
static CHECK_QUALITY_LEVELS: std::sync::OnceLock<QualityLevels> = std::sync::OnceLock::new();
static CHECK_FORMAT_SUPPORT: std::sync::OnceLock<FormatSupport> = std::sync::OnceLock::new();

/// The device's table with `CheckMultisampleQualityLevels` and `CheckFormatSupport` answering as
/// Direct3D does, made once from the first device's.
fn patched_table(original: *const ID3D11Device3_Vtbl) -> *const ID3D11Device3_Vtbl {
    struct Table(ID3D11Device3_Vtbl);
    // SAFETY: a table of function pointers, never written after it is made.
    unsafe impl Sync for Table {}
    unsafe impl Send for Table {}
    static PATCHED: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    &PATCHED
        .get_or_init(|| {
            // SAFETY: the windows crate's table, copied bit for bit.
            let mut table = unsafe { std::ptr::read(original) };
            let device = &mut table.base__.base__.base__;
            let _ = CHECK_QUALITY_LEVELS.set(device.CheckMultisampleQualityLevels);
            device.CheckMultisampleQualityLevels = check_quality_levels;
            let _ = CHECK_FORMAT_SUPPORT.set(device.CheckFormatSupport);
            device.CheckFormatSupport = check_format_support;
            Table(table)
        })
        .0
}

/// `CheckMultisampleQualityLevels`: `E_INVALIDARG` for a null out pointer, and the count zeroed
/// for a sample count it has no levels for (`E_FAIL`), as Direct3D answers.
unsafe extern "system" fn check_quality_levels(
    this: *mut c_void,
    format: DXGI_FORMAT,
    samples: u32,
    out: *mut u32,
) -> windows::core::HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let mut n = 0;
    let entry = CHECK_QUALITY_LEVELS.get().expect("the table was patched");
    let hr = unsafe { entry(this, format, samples, &mut n) };
    if hr.is_ok() || hr == windows::Win32::Foundation::E_FAIL {
        unsafe { *out = n };
    }
    hr
}

/// Writes `object` to an out parameter, or reports that the call only checks (a null out pointer).
/// Makes an object through the call `make` serving its Direct3D 11 interface `I` and hands it out
/// as its 11.3 interface `J`, which every such object of ours offers. A null `out` asks only
/// whether the call would succeed.
fn as_one<I, J>(out: OutRef<J>, make: impl FnOnce(OutRef<I>) -> Result<()>) -> Result<()>
where
    I: Interface + windows::core::Type<I, Default = Option<I>>,
    J: Interface + windows::core::Type<J, Default = Option<J>>,
{
    if out.is_null() {
        // SAFETY: `OutRef` is a transparent pointer; a null one asks for nothing back.
        return make(unsafe {
            std::mem::transmute::<*mut c_void, OutRef<I>>(std::ptr::null_mut())
        });
    }
    let mut made: Option<I> = None;
    make(OutRef::from(&mut made))?;
    out.write(made.map(|m| m.cast()).transpose()?)
}

fn give<I: windows::core::Type<I, Default = Option<I>>>(out: OutRef<I>, object: I) -> Result<()> {
    if out.is_null() {
        return Err(windows::Win32::Foundation::S_FALSE.into());
    }
    out.write(Some(object))
}

/// Whether queries of `kind` are predicates: occlusion and stream output overflow predicates.
fn is_predicate(kind: D3D11_QUERY) -> bool {
    kind == D3D11_QUERY_OCCLUSION_PREDICATE
        || (D3D11_QUERY_SO_OVERFLOW_PREDICATE.0..=D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM3.0)
            .contains(&kind.0)
            && (kind.0 - D3D11_QUERY_SO_OVERFLOW_PREDICATE.0) % 2 == 0
}

/// The interface identifiers of `ID3D10Device` and `ID3D10Device1`, as Direct3D 10's headers give
/// them.
const ID3D10_DEVICE: windows::core::GUID =
    windows::core::GUID::from_u128(0x9b7e4c0f_342c_4106_a19f_4f2704f689f0);
const ID3D10_DEVICE1: windows::core::GUID =
    windows::core::GUID::from_u128(0x9b7e4c8f_342c_4106_a19f_4f2704f689f0);

impl Device_Impl {
    fn me(&self) -> ID3D11Device {
        self.to_interface::<ID3D11Device3>().into()
    }

    pub(super) fn call(&self) -> Call<'_> {
        Call::new(&self.front)
    }

    pub(super) fn unserved(&self, what: &'static str) {
        missing_void(what);
        self.front
            .borrow_mut()
            .lose(wcontainers_direct3d_11::Error::NotImplemented(what));
    }

    pub(super) fn create_buffer(
        &self,
        desc: *const D3D11_BUFFER_DESC,
        init: *const D3D11_SUBRESOURCE_DATA,
        out: OutRef<ID3D11Buffer>,
    ) -> Result<()> {
        let d = convert::buffer(unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?);
        // Initial data without bytes is invalid, as Direct3D answers.
        let bytes = match unsafe { init.as_ref() } {
            Some(i) if i.pSysMem.is_null() => return Err(E_INVALIDARG.into()),
            Some(i) => Some(unsafe {
                std::slice::from_raw_parts(i.pSysMem.cast::<u8>(), d.byte_width as usize)
            }),
            None => None,
        };
        let front = self
            .front
            .borrow_mut()
            .create_buffer(&d, bytes)
            .map_err(hresult)?;
        give(out, Buffer::new(front, self.me()).into())
    }

    /// The front texture of a description and the program's initial data.
    fn texture(
        &self,
        d: TextureDesc,
        init: *const D3D11_SUBRESOURCE_DATA,
    ) -> Result<Rc<wcontainers_direct3d_11::object::Texture<Chosen>>> {
        let mut data = Vec::new();
        if !init.is_null() {
            // The description is checked before the program's array of initial data is read by it.
            let (fmt, mips) = self
                .front
                .borrow()
                .check_texture(&d, true)
                .map_err(hresult)?;
            for i in 0..mips * d.array_size {
                let s = unsafe { &*init.add(i as usize) };
                let mip = i % mips;
                let (w, h) = ((d.width >> mip).max(1), (d.height >> mip).max(1));
                let slices = (d.depth >> mip).max(1);
                let (row, rows) = format::row_bytes(fmt, w, h);
                let len = s.SysMemSlicePitch * (slices - 1) + s.SysMemPitch * (rows - 1) + row;
                if s.pSysMem.is_null() {
                    return Err(E_INVALIDARG.into());
                }
                data.push(SubresourceData {
                    bytes: unsafe {
                        std::slice::from_raw_parts(s.pSysMem.cast::<u8>(), len as usize)
                    },
                    row_pitch: s.SysMemPitch,
                    slice_pitch: s.SysMemSlicePitch,
                });
            }
        }
        self.front
            .borrow_mut()
            .create_texture(&d, &data)
            .map_err(hresult)
    }

    pub(super) fn create_texture1_d(
        &self,
        desc: *const D3D11_TEXTURE1D_DESC,
        init: *const D3D11_SUBRESOURCE_DATA,
        out: OutRef<ID3D11Texture1D>,
    ) -> Result<()> {
        let d = convert::texture1d(unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?);
        let front = self.texture(d, init)?;
        give(out, Texture1D::new(front, self.me()).into())
    }

    pub(super) fn create_texture2_d(
        &self,
        desc: *const D3D11_TEXTURE2D_DESC,
        init: *const D3D11_SUBRESOURCE_DATA,
        out: OutRef<ID3D11Texture2D>,
    ) -> Result<()> {
        let d = convert::texture2d(unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?);
        let front = self.texture(d, init)?;
        give(out, Texture2D::new(front, self.me()).into())
    }

    pub(super) fn create_texture3_d(
        &self,
        desc: *const D3D11_TEXTURE3D_DESC,
        init: *const D3D11_SUBRESOURCE_DATA,
        out: OutRef<ID3D11Texture3D>,
    ) -> Result<()> {
        let d = convert::texture3d(unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?);
        let front = self.texture(d, init)?;
        give(out, Texture3D::new(front, self.me()).into())
    }

    /// `CreateTexture2D1`: only the undefined layout is served (no other is reported).
    pub(super) fn create_texture2_d1(
        &self,
        desc: *const D3D11_TEXTURE2D_DESC1,
        init: *const D3D11_SUBRESOURCE_DATA,
        out: OutRef<ID3D11Texture2D1>,
    ) -> Result<()> {
        let d1 = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        if d1.TextureLayout != D3D11_TEXTURE_LAYOUT_UNDEFINED {
            return Err(E_INVALIDARG.into());
        }
        let d = convert::texture2d(&unsafe { convert::same_fields(d1) });
        let front = self.texture(d, init)?;
        give(out, Texture2D::new(front, self.me()).into())
    }

    pub(super) fn create_texture3_d1(
        &self,
        desc: *const D3D11_TEXTURE3D_DESC1,
        init: *const D3D11_SUBRESOURCE_DATA,
        out: OutRef<ID3D11Texture3D1>,
    ) -> Result<()> {
        let d1 = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        if d1.TextureLayout != D3D11_TEXTURE_LAYOUT_UNDEFINED {
            return Err(E_INVALIDARG.into());
        }
        let d = convert::texture3d(&unsafe { convert::same_fields(d1) });
        let front = self.texture(d, init)?;
        give(out, Texture3D::new(front, self.me()).into())
    }

    pub(super) fn create_shader_resource_view1(
        &self,
        resource: Ref<ID3D11Resource>,
        desc: *const D3D11_SHADER_RESOURCE_VIEW_DESC1,
        out: OutRef<ID3D11ShaderResourceView1>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.map(convert::srv_of1).transpose()?;
        let d = d.as_ref().map_or(std::ptr::null(), |d| d as *const _);
        as_one(out, |o| self.create_shader_resource_view(resource, d, o))
    }

    pub(super) fn create_render_target_view1(
        &self,
        resource: Ref<ID3D11Resource>,
        desc: *const D3D11_RENDER_TARGET_VIEW_DESC1,
        out: OutRef<ID3D11RenderTargetView1>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.map(convert::rtv_of1).transpose()?;
        let d = d.as_ref().map_or(std::ptr::null(), |d| d as *const _);
        as_one(out, |o| self.create_render_target_view(resource, d, o))
    }

    pub(super) fn create_unordered_access_view1(
        &self,
        resource: Ref<ID3D11Resource>,
        desc: *const D3D11_UNORDERED_ACCESS_VIEW_DESC1,
        out: OutRef<ID3D11UnorderedAccessView1>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.map(convert::uav_of1).transpose()?;
        let d = d.as_ref().map_or(std::ptr::null(), |d| d as *const _);
        as_one(out, |o| self.create_unordered_access_view(resource, d, o))
    }

    /// `CreateQuery1`: one queue runs every kind of work, so only queries of all of it are served;
    /// predicates are made by `CreateQuery` and `CreatePredicate`.
    pub(super) fn create_query1(
        &self,
        desc: *const D3D11_QUERY_DESC1,
        out: OutRef<ID3D11Query1>,
    ) -> Result<()> {
        let d1 = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        if d1.ContextType != D3D11_CONTEXT_TYPE_ALL {
            return Err(missing("queries of one context type"));
        }
        if is_predicate(d1.Query) {
            return Err(missing("predicates made by CreateQuery1"));
        }
        let d = D3D11_QUERY_DESC {
            Query: d1.Query,
            MiscFlags: d1.MiscFlags,
        };
        as_one(out, |o| self.create_query(&d, o))
    }

    /// The front texture behind a resource a target view names, refusing buffers (render target
    /// views of buffers are not served) and foreign objects.
    fn texture_of(
        resource: &Ref<ID3D11Resource>,
    ) -> Result<Rc<wcontainers_direct3d_11::object::Texture<Chosen>>> {
        match resource.as_ref().and_then(resource_of) {
            Some(wcontainers_direct3d_11::object::Resource::Texture(t)) => Ok(t),
            Some(_) => Err(missing("target views of buffers")),
            None => Err(E_INVALIDARG.into()),
        }
    }

    pub(super) fn create_shader_resource_view(
        &self,
        resource: Ref<ID3D11Resource>,
        desc: *const D3D11_SHADER_RESOURCE_VIEW_DESC,
        out: OutRef<ID3D11ShaderResourceView>,
    ) -> Result<()> {
        use windows::Win32::Graphics::Direct3D::{
            D3D11_SRV_DIMENSION_BUFFER, D3D11_SRV_DIMENSION_BUFFEREX,
        };
        let r = resource
            .as_ref()
            .and_then(HeldResource::of)
            .ok_or(E_INVALIDARG)?;
        let given = unsafe { desc.as_ref() };
        let (view, d) = match r.front() {
            wcontainers_direct3d_11::object::Resource::Buffer(b) => {
                let d = match given {
                    Some(d)
                        if d.ViewDimension == D3D11_SRV_DIMENSION_BUFFER
                            || d.ViewDimension == D3D11_SRV_DIMENSION_BUFFEREX =>
                    {
                        *d
                    }
                    // Without a description a structured buffer is viewed whole.
                    None if b.desc.misc_flags & desc::MISC_STRUCTURED != 0 => {
                        let mut d = D3D11_SHADER_RESOURCE_VIEW_DESC {
                            ViewDimension: D3D11_SRV_DIMENSION_BUFFER,
                            ..Default::default()
                        };
                        d.Anonymous.Buffer.Anonymous2.NumElements =
                            b.desc.byte_width / b.desc.structure_byte_stride.max(1);
                        d
                    }
                    _ => return Err(E_INVALIDARG.into()),
                };
                // BUFFER and BUFFEREX lay out their first element and count alike; only BUFFEREX
                // has flags (raw).
                let (first, count, raw) = unsafe {
                    if d.ViewDimension == D3D11_SRV_DIMENSION_BUFFEREX {
                        let e = d.Anonymous.BufferEx;
                        (e.FirstElement, e.NumElements, e.Flags & 1 != 0)
                    } else {
                        let e = d.Anonymous.Buffer;
                        (e.Anonymous1.FirstElement, e.Anonymous2.NumElements, false)
                    }
                };
                let view = self
                    .front
                    .borrow_mut()
                    .create_buffer_srv(&b, d.Format.0 as u32, first, count, raw)
                    .map_err(hresult)?;
                (view, d)
            }
            wcontainers_direct3d_11::object::Resource::Texture(t) => {
                let v = given.map(convert::srv).transpose()?;
                let view = self
                    .front
                    .borrow_mut()
                    .create_view(&t, v.as_ref(), desc::bind::SHADER_RESOURCE)
                    .map_err(hresult)?;
                let d = convert::srv_back(&view.desc);
                (
                    Rc::new(wcontainers_direct3d_11::object::ShaderView::Texture(view)),
                    d,
                )
            }
        };
        give(out, ShaderResourceView::new(view, d, r, self.me()).into())
    }

    pub(super) fn create_render_target_view(
        &self,
        resource: Ref<ID3D11Resource>,
        desc: *const D3D11_RENDER_TARGET_VIEW_DESC,
        out: OutRef<ID3D11RenderTargetView>,
    ) -> Result<()> {
        let texture = Self::texture_of(&resource)?;
        let d = unsafe { desc.as_ref() }.map(convert::rtv).transpose()?;
        let view = self
            .front
            .borrow_mut()
            .create_view(&texture, d.as_ref(), desc::bind::RENDER_TARGET)
            .map_err(hresult)?;
        let r = resource
            .as_ref()
            .and_then(HeldResource::of)
            .ok_or(E_INVALIDARG)?;
        give(out, RenderTargetView::new(view, r, self.me()).into())
    }

    pub(super) fn create_unordered_access_view(
        &self,
        resource: Ref<ID3D11Resource>,
        desc: *const D3D11_UNORDERED_ACCESS_VIEW_DESC,
        out: OutRef<ID3D11UnorderedAccessView>,
    ) -> Result<()> {
        let r = resource
            .as_ref()
            .and_then(HeldResource::of)
            .ok_or(E_INVALIDARG)?;
        let given = unsafe { desc.as_ref() };
        let (view, d) = match r.front() {
            wcontainers_direct3d_11::object::Resource::Buffer(b) => {
                let structured = b.desc.misc_flags & desc::MISC_STRUCTURED != 0;
                let d = match given {
                    Some(d) if d.ViewDimension == D3D11_UAV_DIMENSION_BUFFER => *d,
                    // Without a description a structured buffer is viewed whole.
                    None if structured => {
                        let mut d = D3D11_UNORDERED_ACCESS_VIEW_DESC {
                            ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
                            ..Default::default()
                        };
                        d.Anonymous.Buffer.NumElements =
                            b.desc.byte_width / b.desc.structure_byte_stride.max(1);
                        d
                    }
                    _ => return Err(E_INVALIDARG.into()),
                };
                let e = unsafe { d.Anonymous.Buffer };
                let view = self
                    .front
                    .borrow_mut()
                    .create_buffer_uav(
                        &b,
                        d.Format.0 as u32,
                        e.FirstElement,
                        e.NumElements,
                        e.Flags,
                    )
                    .map_err(hresult)?;
                (view, d)
            }
            wcontainers_direct3d_11::object::Resource::Texture(t) => {
                let v = given.map(convert::uav).transpose()?;
                let view = self
                    .front
                    .borrow_mut()
                    .create_texture_uav(&t, v.as_ref())
                    .map_err(hresult)?;
                let d = match &*view {
                    wcontainers_direct3d_11::object::ShaderView::Texture(v) => {
                        convert::uav_back(&v.desc)
                    }
                    wcontainers_direct3d_11::object::ShaderView::Buffer(_) => unreachable!(),
                };
                (view, d)
            }
        };
        give(out, UnorderedAccessView::new(view, d, r, self.me()).into())
    }

    pub(super) fn create_depth_stencil_view(
        &self,
        resource: Ref<ID3D11Resource>,
        desc: *const D3D11_DEPTH_STENCIL_VIEW_DESC,
        out: OutRef<ID3D11DepthStencilView>,
    ) -> Result<()> {
        let texture = Self::texture_of(&resource)?;
        let d = unsafe { desc.as_ref() }.map(convert::dsv).transpose()?;
        let view = self
            .front
            .borrow_mut()
            .create_view(&texture, d.as_ref(), desc::bind::DEPTH_STENCIL)
            .map_err(hresult)?;
        let r = resource
            .as_ref()
            .and_then(HeldResource::of)
            .ok_or(E_INVALIDARG)?;
        give(out, DepthStencilView::new(view, r, self.me()).into())
    }

    pub(super) fn create_input_layout(
        &self,
        elements: *const D3D11_INPUT_ELEMENT_DESC,
        count: u32,
        bytecode: *const c_void,
        length: usize,
        out: OutRef<ID3D11InputLayout>,
    ) -> Result<()> {
        if elements.is_null() || bytecode.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let list = unsafe { std::slice::from_raw_parts(elements, count as usize) };
        let mut front_elements = Vec::with_capacity(list.len());
        for e in list {
            let semantic = unsafe { e.SemanticName.to_string() }.map_err(|_| E_INVALIDARG)?;
            front_elements.push(InputElement {
                semantic,
                semantic_index: e.SemanticIndex,
                format: e.Format.0 as u32,
                slot: e.InputSlot,
                offset: e.AlignedByteOffset,
                per_instance: e.InputSlotClass == D3D11_INPUT_PER_INSTANCE_DATA,
                step_rate: e.InstanceDataStepRate,
            });
        }
        let code = unsafe { std::slice::from_raw_parts(bytecode.cast::<u8>(), length) };
        let layout = self
            .front
            .borrow_mut()
            .create_input_layout(&front_elements, code)
            .map_err(hresult)?;
        give(out, InputLayout::new(layout, self.me()).into())
    }

    pub(super) fn create_vertex_shader(
        &self,
        bytecode: *const c_void,
        length: usize,
        linkage: Ref<ID3D11ClassLinkage>,
        out: OutRef<ID3D11VertexShader>,
    ) -> Result<()> {
        if bytecode.is_null() || !linkage.is_null() {
            return Err(if bytecode.is_null() {
                E_INVALIDARG.into()
            } else {
                missing("class linkage")
            });
        }
        let code = unsafe { std::slice::from_raw_parts(bytecode.cast::<u8>(), length) };
        let shader = self
            .front
            .borrow_mut()
            .create_shader(code, SourceStage::Vertex)
            .map_err(hresult)?;
        give(out, VertexShader::new(shader, self.me()).into())
    }

    pub(super) fn create_compute_shader(
        &self,
        bytecode: *const c_void,
        length: usize,
        linkage: Ref<ID3D11ClassLinkage>,
        out: OutRef<ID3D11ComputeShader>,
    ) -> Result<()> {
        if bytecode.is_null() || !linkage.is_null() {
            return Err(if bytecode.is_null() {
                E_INVALIDARG.into()
            } else {
                missing("class linkage")
            });
        }
        // 10_x takes shader model 4 compute shaders where raw and structured buffers come with
        // them (`D3D10_X_HARDWARE_OPTIONS`); the front checks the model against the level.
        if self.front.borrow().feature_level() < 0xa000 || !self.compute() {
            return Err(missing(
                "compute shaders without the back end's compute programs",
            ));
        }
        let code = unsafe { std::slice::from_raw_parts(bytecode.cast::<u8>(), length) };
        let shader = self
            .front
            .borrow_mut()
            .create_shader(code, SourceStage::Compute)
            .map_err(hresult)?;
        give(out, ComputeShader::new(shader, self.me()).into())
    }

    /// A shader of a stage this library does not draw yet: what Direct3D refuses (a null pointer,
    /// another stage, a model the feature level does not take) is refused as it refuses it; the rest
    /// is not served.
    fn unserved_shader(
        &self,
        bytecode: *const c_void,
        length: usize,
        stage: SourceStage,
        what: &'static str,
    ) -> Result<()> {
        if bytecode.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let code = unsafe { std::slice::from_raw_parts(bytecode.cast::<u8>(), length) };
        let level = self.front.borrow().feature_level();
        if level < 0xa000
            || wcontainers_direct3d_11::shader::stage(code).ok() != Some(stage)
            || wcontainers_direct3d_11::shader::check_model(code, level).is_err()
        {
            return Err(E_INVALIDARG.into());
        }
        Err(missing(what))
    }

    pub(super) fn create_geometry_shader(
        &self,
        bytecode: *const c_void,
        length: usize,
        linkage: Ref<ID3D11ClassLinkage>,
        out: OutRef<ID3D11GeometryShader>,
    ) -> Result<()> {
        self.geometry_shader(bytecode, length, None, linkage, out)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn create_geometry_shader_with_stream_output(
        &self,
        bytecode: *const c_void,
        length: usize,
        entries: *const D3D11_SO_DECLARATION_ENTRY,
        count: u32,
        strides: *const u32,
        stride_count: u32,
        rasterized: u32,
        linkage: Ref<ID3D11ClassLinkage>,
        out: OutRef<ID3D11GeometryShader>,
    ) -> Result<()> {
        let entries = (!entries.is_null()).then(|| {
            unsafe { std::slice::from_raw_parts(entries, count as usize) }
                .iter()
                .map(|e| wcontainers_direct3d_11::stream::DeclEntry {
                    stream: e.Stream,
                    semantic: (!e.SemanticName.is_null()).then(|| {
                        String::from_utf8_lossy(unsafe { e.SemanticName.as_bytes() }).into_owned()
                    }),
                    semantic_index: e.SemanticIndex,
                    start_component: e.StartComponent,
                    component_count: e.ComponentCount,
                    slot: e.OutputSlot,
                })
                .collect::<Vec<_>>()
        });
        let strides = (!strides.is_null()).then(|| {
            unsafe { std::slice::from_raw_parts(strides, stride_count as usize) }.to_vec()
        });
        self.geometry_shader(
            bytecode,
            length,
            Some((entries, strides, rasterized)),
            linkage,
            out,
        )
    }

    /// A geometry shader, with the stream output declaration given (entries and strides, each
    /// `None` where the program gave no array, and the rasterized stream).
    #[allow(clippy::type_complexity)]
    fn geometry_shader(
        &self,
        bytecode: *const c_void,
        length: usize,
        so: Option<(
            Option<Vec<wcontainers_direct3d_11::stream::DeclEntry>>,
            Option<Vec<u32>>,
            u32,
        )>,
        linkage: Ref<ID3D11ClassLinkage>,
        out: OutRef<ID3D11GeometryShader>,
    ) -> Result<()> {
        if bytecode.is_null() || !linkage.is_null() {
            return Err(if bytecode.is_null() {
                E_INVALIDARG.into()
            } else {
                missing("class linkage")
            });
        }
        if self.front.borrow().feature_level() < 0xa000 {
            return Err(E_INVALIDARG.into());
        }
        let code = unsafe { std::slice::from_raw_parts(bytecode.cast::<u8>(), length) };
        let so = so
            .as_ref()
            .map(|(e, s, r)| (e.as_deref(), s.as_deref(), *r));
        let shader = self
            .front
            .borrow_mut()
            .create_geometry_shader(code, so)
            .map_err(hresult)?;
        give(out, GeometryShader::new(shader, self.me()).into())
    }

    pub(super) fn create_hull_shader(
        &self,
        bytecode: *const c_void,
        length: usize,
        linkage: Ref<ID3D11ClassLinkage>,
        out: OutRef<ID3D11HullShader>,
    ) -> Result<()> {
        let shader = self.tessellation_shader(bytecode, length, linkage, SourceStage::Hull)?;
        give(out, HullShader::new(shader, self.me()).into())
    }

    pub(super) fn create_domain_shader(
        &self,
        bytecode: *const c_void,
        length: usize,
        linkage: Ref<ID3D11ClassLinkage>,
        out: OutRef<ID3D11DomainShader>,
    ) -> Result<()> {
        let shader = self.tessellation_shader(bytecode, length, linkage, SourceStage::Domain)?;
        give(out, DomainShader::new(shader, self.me()).into())
    }

    /// A hull or domain shader: drawn through the stage helper's compute programs, so absent
    /// without the back end's.
    fn tessellation_shader(
        &self,
        bytecode: *const c_void,
        length: usize,
        linkage: Ref<ID3D11ClassLinkage>,
        stage: SourceStage,
    ) -> Result<Rc<wcontainers_direct3d_11::object::Shader<Chosen>>> {
        if !linkage.is_null() {
            return Err(missing("class linkage"));
        }
        if !self.compute() {
            let what = "hull and domain shaders without the back end's compute programs";
            return Err(self
                .unserved_shader(bytecode, length, stage, what)
                .expect_err("refused"));
        }
        if bytecode.is_null() || self.front.borrow().feature_level() < 0xb000 {
            return Err(E_INVALIDARG.into());
        }
        let code = unsafe { std::slice::from_raw_parts(bytecode.cast::<u8>(), length) };
        self.front
            .borrow_mut()
            .create_shader(code, stage)
            .map_err(hresult)
    }

    pub(super) fn create_pixel_shader(
        &self,
        bytecode: *const c_void,
        length: usize,
        linkage: Ref<ID3D11ClassLinkage>,
        out: OutRef<ID3D11PixelShader>,
    ) -> Result<()> {
        if bytecode.is_null() || !linkage.is_null() {
            return Err(if bytecode.is_null() {
                E_INVALIDARG.into()
            } else {
                missing("class linkage")
            });
        }
        let code = unsafe { std::slice::from_raw_parts(bytecode.cast::<u8>(), length) };
        let shader = self
            .front
            .borrow_mut()
            .create_shader(code, SourceStage::Pixel)
            .map_err(hresult)?;
        give(out, PixelShader::new(shader, self.me()).into())
    }

    pub(super) fn create_blend_state(
        &self,
        desc: *const D3D11_BLEND_DESC,
        out: OutRef<ID3D11BlendState>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        self.blend_state(d, [(false.into(), D3D11_LOGIC_OP_NOOP); 8], out)
    }

    pub(super) fn create_blend_state1(
        &self,
        desc: *const D3D11_BLEND_DESC1,
        out: OutRef<ID3D11BlendState1>,
    ) -> Result<()> {
        let (d, logic) = convert::blend_split(unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?);
        if logic.iter().any(|l| l.0.as_bool()) {
            return Err(missing("logic operations"));
        }
        self.blend_state(&d, logic, out)
    }

    fn blend_state<I>(
        &self,
        d: &D3D11_BLEND_DESC,
        logic: convert::LogicOps,
        out: OutRef<I>,
    ) -> Result<()>
    where
        BlendState_Impl: ComObjectInterface<I>,
        I: Interface + windows::core::Type<I, Default = Option<I>> + From<BlendState>,
    {
        let d = &convert::blend_stored(d);
        let mut key = blend_key(d);
        key.extend(logic.iter().flat_map(|l| [l.0.0 as u32, l.1.0 as u32]));
        shared_state::<BlendState, _>(&self.states.blend, key, out, || {
            let state = self
                .front
                .borrow_mut()
                .create_blend_state(&convert::blend(d))
                .map_err(hresult)?;
            Ok(BlendState::new(state, *d, logic, self.me()).into())
        })
    }

    pub(super) fn create_depth_stencil_state(
        &self,
        desc: *const D3D11_DEPTH_STENCIL_DESC,
        out: OutRef<ID3D11DepthStencilState>,
    ) -> Result<()> {
        let d = &convert::depth_stencil_stored(unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?);
        shared_state::<DepthStencilState, _>(
            &self.states.depth_stencil,
            depth_stencil_key(d),
            out,
            || {
                let state = self
                    .front
                    .borrow_mut()
                    .create_depth_stencil_state(&convert::depth_stencil(d)?)
                    .map_err(hresult)?;
                Ok(DepthStencilState::new(state, *d, self.me()).into())
            },
        )
    }

    pub(super) fn create_rasterizer_state(
        &self,
        desc: *const D3D11_RASTERIZER_DESC,
        out: OutRef<ID3D11RasterizerState>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        self.rasterizer_state(d, out)
    }

    pub(super) fn create_rasterizer_state1(
        &self,
        desc: *const D3D11_RASTERIZER_DESC1,
        out: OutRef<ID3D11RasterizerState1>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        self.rasterizer_state1(d, out)
    }

    pub(super) fn create_rasterizer_state2(
        &self,
        desc: *const D3D11_RASTERIZER_DESC2,
        out: OutRef<ID3D11RasterizerState2>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        self.rasterizer_state1(&convert::rasterizer_of2(d)?, out)
    }

    fn rasterizer_state1<I>(&self, d: &D3D11_RASTERIZER_DESC1, out: OutRef<I>) -> Result<()>
    where
        RasterizerState_Impl: ComObjectInterface<I>,
        I: Interface + windows::core::Type<I, Default = Option<I>> + From<RasterizerState>,
    {
        if d.ForcedSampleCount != 0 {
            return Err(missing("forced sample counts"));
        }
        self.rasterizer_state(&convert::rasterizer_of1(d), out)
    }

    fn rasterizer_state<I>(&self, d: &D3D11_RASTERIZER_DESC, out: OutRef<I>) -> Result<()>
    where
        RasterizerState_Impl: ComObjectInterface<I>,
        I: Interface + windows::core::Type<I, Default = Option<I>> + From<RasterizerState>,
    {
        shared_state::<RasterizerState, _>(&self.states.raster, words(d), out, || {
            let state = self
                .front
                .borrow_mut()
                .create_rasterizer_state(&convert::rasterizer(d))
                .map_err(hresult)?;
            Ok(RasterizerState::new(state, *d, self.me()).into())
        })
    }

    pub(super) fn create_sampler_state(
        &self,
        desc: *const D3D11_SAMPLER_DESC,
        out: OutRef<ID3D11SamplerState>,
    ) -> Result<()> {
        let d = &convert::sampler_stored(unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?);
        shared_state::<SamplerState, _>(&self.states.sampler, words(d), out, || {
            let state = self
                .front
                .borrow_mut()
                .create_sampler_state(&convert::sampler(d))
                .map_err(hresult)?;
            Ok(SamplerState::new(state, *d, self.me()).into())
        })
    }

    pub(super) fn create_query(
        &self,
        desc: *const D3D11_QUERY_DESC,
        out: OutRef<ID3D11Query>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        let front = self
            .front
            .borrow_mut()
            .create_query(d.Query.0 as u32, d.MiscFlags)
            .map_err(hresult)?;
        // Predicates are queries too; `QueryInterface` gives them as `ID3D11Predicate`.
        if is_predicate(d.Query) {
            let p: ID3D11Predicate = Predicate::new(front, self.me()).into();
            return give(out, p.cast()?);
        }
        give(out, Query::new(front, self.me()).into())
    }

    pub(super) fn create_predicate(
        &self,
        desc: *const D3D11_QUERY_DESC,
        out: OutRef<ID3D11Predicate>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        // The per-stream overflow predicates are predicates, but only made as queries.
        if !matches!(
            d.Query,
            D3D11_QUERY_OCCLUSION_PREDICATE | D3D11_QUERY_SO_OVERFLOW_PREDICATE
        ) {
            return Err(E_INVALIDARG.into());
        }
        let front = self
            .front
            .borrow_mut()
            .create_query(d.Query.0 as u32, d.MiscFlags)
            .map_err(hresult)?;
        give(out, Predicate::new(front, self.me()).into())
    }

    pub(super) fn check_format_support(&self, format: DXGI_FORMAT) -> Result<u32> {
        match self.front.borrow().format_support(format.0 as u32) {
            // Direct3D's answer for a format the device does not support at all.
            0 => Err(windows::Win32::Foundation::E_FAIL.into()),
            bits => Ok(bits),
        }
    }

    /// `CheckMultisampleQualityLevels1`: tiled resources are not reported, so none of their
    /// levels exist.
    pub(super) fn check_multisample_quality_levels1(
        &self,
        format: DXGI_FORMAT,
        samples: u32,
        flags: u32,
    ) -> Result<u32> {
        match flags {
            0 => self.check_multisample_quality_levels(format, samples),
            f if f == D3D11_CHECK_MULTISAMPLE_QUALITY_LEVELS_TILED_RESOURCE.0 as u32 => Ok(0),
            _ => Err(E_INVALIDARG.into()),
        }
    }

    pub(super) fn check_multisample_quality_levels(
        &self,
        format: DXGI_FORMAT,
        samples: u32,
    ) -> Result<u32> {
        // Values that name no DXGI format.
        let f = format.0 as u32;
        if !matches!(f, 0..=132 | 189..=191) {
            return Err(E_INVALIDARG.into());
        }
        if samples == 0 || samples > 32 {
            return Err(windows::Win32::Foundation::E_FAIL.into());
        }
        Ok(self
            .front
            .borrow()
            .multisample_quality_levels(format.0 as u32, samples))
    }

    /// Each feature's answer: what is served, and absent (zeros, Direct3D's "not supported") for
    /// the rest. A size other than the feature's structure is `E_INVALIDARG`, as Direct3D answers.
    pub(super) fn check_feature_support(
        &self,
        feature: D3D11_FEATURE,
        data: *mut c_void,
        size: u32,
    ) -> Result<()> {
        let want = match feature {
            D3D11_FEATURE_THREADING => size_of::<D3D11_FEATURE_DATA_THREADING>(),
            D3D11_FEATURE_DOUBLES => size_of::<D3D11_FEATURE_DATA_DOUBLES>(),
            D3D11_FEATURE_FORMAT_SUPPORT => size_of::<D3D11_FEATURE_DATA_FORMAT_SUPPORT>(),
            D3D11_FEATURE_FORMAT_SUPPORT2 => size_of::<D3D11_FEATURE_DATA_FORMAT_SUPPORT2>(),
            D3D11_FEATURE_D3D10_X_HARDWARE_OPTIONS => {
                size_of::<D3D11_FEATURE_DATA_D3D10_X_HARDWARE_OPTIONS>()
            }
            D3D11_FEATURE_D3D11_OPTIONS => size_of::<D3D11_FEATURE_DATA_D3D11_OPTIONS>(),
            D3D11_FEATURE_ARCHITECTURE_INFO => size_of::<D3D11_FEATURE_DATA_ARCHITECTURE_INFO>(),
            D3D11_FEATURE_D3D9_OPTIONS => size_of::<D3D11_FEATURE_DATA_D3D9_OPTIONS>(),
            D3D11_FEATURE_SHADER_MIN_PRECISION_SUPPORT => {
                size_of::<D3D11_FEATURE_DATA_SHADER_MIN_PRECISION_SUPPORT>()
            }
            D3D11_FEATURE_D3D9_SHADOW_SUPPORT => {
                size_of::<D3D11_FEATURE_DATA_D3D9_SHADOW_SUPPORT>()
            }
            D3D11_FEATURE_D3D11_OPTIONS1 => size_of::<D3D11_FEATURE_DATA_D3D11_OPTIONS1>(),
            D3D11_FEATURE_D3D9_SIMPLE_INSTANCING_SUPPORT => {
                size_of::<D3D11_FEATURE_DATA_D3D9_SIMPLE_INSTANCING_SUPPORT>()
            }
            D3D11_FEATURE_MARKER_SUPPORT => size_of::<D3D11_FEATURE_DATA_MARKER_SUPPORT>(),
            D3D11_FEATURE_D3D9_OPTIONS1 => size_of::<D3D11_FEATURE_DATA_D3D9_OPTIONS1>(),
            D3D11_FEATURE_D3D11_OPTIONS2 => size_of::<D3D11_FEATURE_DATA_D3D11_OPTIONS2>(),
            D3D11_FEATURE_D3D11_OPTIONS3 => size_of::<D3D11_FEATURE_DATA_D3D11_OPTIONS3>(),
            D3D11_FEATURE_GPU_VIRTUAL_ADDRESS_SUPPORT => {
                size_of::<D3D11_FEATURE_DATA_GPU_VIRTUAL_ADDRESS_SUPPORT>()
            }
            D3D11_FEATURE_D3D11_OPTIONS4 => size_of::<D3D11_FEATURE_DATA_D3D11_OPTIONS4>(),
            D3D11_FEATURE_SHADER_CACHE => size_of::<D3D11_FEATURE_DATA_SHADER_CACHE>(),
            D3D11_FEATURE_D3D11_OPTIONS5 => size_of::<D3D11_FEATURE_DATA_D3D11_OPTIONS5>(),
            D3D11_FEATURE_DISPLAYABLE => size_of::<D3D11_FEATURE_DATA_DISPLAYABLE>(),
            _ => return Err(E_INVALIDARG.into()),
        };
        if data.is_null() || size as usize != want {
            return Err(E_INVALIDARG.into());
        }
        if feature == D3D11_FEATURE_FORMAT_SUPPORT {
            let d = unsafe { &mut *data.cast::<D3D11_FEATURE_DATA_FORMAT_SUPPORT>() };
            // A format the device lacks answers no uses with the failure.
            let support = self.check_format_support(d.InFormat);
            d.OutFormatSupport = *support.as_ref().unwrap_or(&0);
            return support.map(|_| ());
        }
        if feature == D3D11_FEATURE_FORMAT_SUPPORT2 {
            let d = unsafe { &mut *data.cast::<D3D11_FEATURE_DATA_FORMAT_SUPPORT2>() };
            self.check_format_support(d.InFormat)?;
            d.OutFormatSupport2 = 0;
            return Ok(());
        }
        unsafe { std::ptr::write_bytes(data.cast::<u8>(), 0, want) };
        // From 10_0 every device has textures of any size and instancing; constant buffers bind at
        // offsets (`*SetConstantBuffers1`); the rest is absent.
        match feature {
            D3D11_FEATURE_D3D11_OPTIONS => unsafe {
                (*data.cast::<D3D11_FEATURE_DATA_D3D11_OPTIONS>()).ConstantBufferOffsetting =
                    true.into();
            },
            D3D11_FEATURE_D3D9_OPTIONS => unsafe {
                (*data.cast::<D3D11_FEATURE_DATA_D3D9_OPTIONS>()).FullNonPow2TextureSupport =
                    true.into();
            },
            D3D11_FEATURE_D3D10_X_HARDWARE_OPTIONS if self.compute() => unsafe {
                (*data.cast::<D3D11_FEATURE_DATA_D3D10_X_HARDWARE_OPTIONS>())
                    .ComputeShaders_Plus_RawAndStructuredBuffers_Via_Shader_4_x = true.into();
            },
            D3D11_FEATURE_D3D9_SIMPLE_INSTANCING_SUPPORT => unsafe {
                (*data.cast::<D3D11_FEATURE_DATA_D3D9_SIMPLE_INSTANCING_SUPPORT>())
                    .SimpleInstancingSupported = true.into();
            },
            _ => {}
        }
        Ok(())
    }

    /// The back end runs compute programs.
    fn compute(&self) -> bool {
        wcontainers_gpu::Backend::caps(&self.front.borrow().backend)
            .has(wcontainers_gpu::Features::COMPUTE)
    }

    pub(super) fn get_feature_level(&self) -> D3D_FEATURE_LEVEL {
        D3D_FEATURE_LEVEL(self.front.borrow().feature_level() as i32)
    }

    pub(super) fn get_creation_flags(&self) -> u32 {
        self.flags
    }

    pub(super) fn get_device_removed_reason(&self) -> Result<()> {
        removal(self.front.borrow_mut().poll_removed())
    }

    pub(super) fn get_immediate_context(&self, out: OutRef<ID3D11DeviceContext>) {
        let _ = out.write(Some(self.immediate().into()));
    }

    pub(super) fn get_immediate_context1(&self, out: OutRef<ID3D11DeviceContext1>) {
        let _ = out.write(Some(self.immediate().into()));
    }

    pub(super) fn get_immediate_context2(&self, out: OutRef<ID3D11DeviceContext2>) {
        let _ = out.write(Some(self.immediate().into()));
    }

    pub(super) fn get_immediate_context3(&self, out: OutRef<ID3D11DeviceContext3>) {
        let _ = out.write(Some(self.immediate()));
    }

    /// `CreateDeferredContext`: a context recording command lists; it takes no flags.
    fn deferred(&self, flags: u32) -> Result<ID3D11DeviceContext3> {
        if flags != 0 {
            return Err(E_INVALIDARG.into());
        }
        self.deferred_allowed()?;
        Ok(super::context::deferred_context(
            self.front.clone(),
            self.me(),
        ))
    }

    pub(super) fn create_deferred_context(
        &self,
        flags: u32,
        out: OutRef<ID3D11DeviceContext>,
    ) -> Result<()> {
        out.write(Some(self.deferred(flags)?.into()))
    }

    pub(super) fn create_deferred_context1(
        &self,
        flags: u32,
        out: OutRef<ID3D11DeviceContext1>,
    ) -> Result<()> {
        out.write(Some(self.deferred(flags)?.into()))
    }

    pub(super) fn create_deferred_context2(
        &self,
        flags: u32,
        out: OutRef<ID3D11DeviceContext2>,
    ) -> Result<()> {
        out.write(Some(self.deferred(flags)?.into()))
    }

    pub(super) fn create_deferred_context3(
        &self,
        flags: u32,
        out: OutRef<ID3D11DeviceContext3>,
    ) -> Result<()> {
        out.write(Some(self.deferred(flags)?))
    }

    /// A device made single-threaded has no deferred contexts (`DXGI_ERROR_INVALID_CALL`).
    fn deferred_allowed(&self) -> Result<()> {
        if self.flags & D3D11_CREATE_DEVICE_SINGLETHREADED.0 != 0 {
            return Err(windows::Win32::Graphics::Dxgi::DXGI_ERROR_INVALID_CALL.into());
        }
        Ok(())
    }

    /// `CreateDeviceContextState`: a state object for this device's own interface, at the first
    /// of `levels` the device has. Direct3D 10's interfaces (a device emulating Direct3D 10) are
    /// not served.
    #[allow(clippy::too_many_arguments, reason = "the COM method's own arguments")]
    pub(super) fn create_device_context_state(
        &self,
        _flags: u32,
        levels: *const D3D_FEATURE_LEVEL,
        count: u32,
        _sdk_version: u32,
        emulated: *const windows::core::GUID,
        chosen: *mut D3D_FEATURE_LEVEL,
        out: OutRef<ID3DDeviceContextState>,
    ) -> Result<()> {
        let chosen = unsafe { chosen.as_mut() };
        let level = (!levels.is_null() && count > 0)
            .then(|| unsafe { std::slice::from_raw_parts(levels, count as usize) })
            .and_then(|l| {
                let top = self.front.borrow().feature_level();
                l.iter()
                    .copied()
                    .find(|l| (0x9100..=top).contains(&(l.0 as u32)))
            });
        let Some(level) = level else {
            if let Some(c) = chosen {
                *c = D3D_FEATURE_LEVEL(0);
            }
            return Err(E_INVALIDARG.into());
        };
        let emulated = unsafe { emulated.as_ref() }.ok_or(E_INVALIDARG)?;
        if [ID3D10_DEVICE, ID3D10_DEVICE1].contains(emulated) {
            return Err(missing("context states emulating Direct3D 10"));
        }
        if ![ID3D11Device::IID, ID3D11Device1::IID].contains(emulated) {
            return Err(E_INVALIDARG.into());
        }
        if let Some(c) = chosen {
            *c = level;
        }
        give(
            out,
            super::context::DeviceContextState::new(self.me()).into(),
        )
    }

    /// The immediate context, made at the first call.
    fn immediate(&self) -> ID3D11DeviceContext3 {
        let raw = self.context.get();
        if raw.is_null() {
            let c: ID3D11DeviceContext3 =
                Context::new(self.front.clone(), self.me(), self.context.clone()).into();
            self.context.set(c.as_raw());
            c
        } else {
            unsafe { ID3D11DeviceContext3::from_raw_borrowed(&raw) }
                .cloned()
                .expect("the context lives")
        }
    }

    /// DXGI's frame latency, kept and reported; presents queue at most wgpu's two frames whatever
    /// it is.
    pub(super) fn set_maximum_frame_latency(&self, n: u32) -> Result<()> {
        if n > 16 {
            return Err(windows::Win32::Graphics::Dxgi::DXGI_ERROR_INVALID_CALL.into());
        }
        self.frame_latency.set(n);
        Ok(())
    }

    pub(super) fn get_maximum_frame_latency(&self) -> Result<u32> {
        Ok(self.frame_latency.get())
    }

    pub(super) fn set_exception_mode(&self, flags: u32) -> Result<()> {
        self.exception_mode.set(flags);
        Ok(())
    }

    pub(super) fn get_exception_mode(&self) -> u32 {
        self.exception_mode.get()
    }

    pub(super) fn get_parent(&self, riid: *const GUID, parent: *mut *mut c_void) -> Result<()> {
        unsafe { self.adapter.query(riid, parent).ok() }
    }

    /// `OfferResources`: the contents are kept, which DXGI allows (it may discard them).
    pub(super) fn offer_resources(
        &self,
        count: u32,
        resources: *const Option<IDXGIResource>,
        priority: DXGI_OFFER_RESOURCE_PRIORITY,
    ) -> Result<()> {
        if !(1..=3).contains(&priority.0) || !all_given(count, resources) {
            return Err(E_INVALIDARG.into());
        }
        Ok(())
    }

    /// `ReclaimResources`: offered resources were never discarded.
    pub(super) fn reclaim_resources(
        &self,
        count: u32,
        resources: *const Option<IDXGIResource>,
        discarded: *mut windows::core::BOOL,
    ) -> Result<()> {
        if !all_given(count, resources) {
            return Err(E_INVALIDARG.into());
        }
        if !discarded.is_null() {
            for i in 0..count as usize {
                unsafe { *discarded.add(i) = false.into() };
            }
        }
        Ok(())
    }

    /// `Trim`: the front keeps no temporary memory a hint could free.
    pub(super) fn trim(&self) {}

    pub(super) fn get_adapter(&self) -> Result<IDXGIAdapter> {
        self.adapter.cast()
    }
}

/// Whether `resources` holds `count` resources, none of them null.
fn all_given(count: u32, resources: *const Option<IDXGIResource>) -> bool {
    count > 0
        && !resources.is_null()
        && unsafe { std::slice::from_raw_parts(resources, count as usize) }
            .iter()
            .all(Option::is_some)
}

/// `CheckFormatSupport`: `E_INVALIDARG` for a null out pointer, and no uses written when the
/// format is not supported, as Direct3D answers.
unsafe extern "system" fn check_format_support(
    this: *mut c_void,
    format: DXGI_FORMAT,
    out: *mut u32,
) -> windows::core::HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let mut uses = 0;
    let entry = CHECK_FORMAT_SUPPORT.get().expect("the table was patched");
    let hr = unsafe { entry(this, format, &mut uses) };
    unsafe { *out = if hr.is_ok() { uses } else { 0 } };
    hr
}
