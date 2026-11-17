//! The device: `ID3D11Device` and `IDXGIDevice` on one object.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIAdapter1, IDXGIDevice};
use windows::core::{
    ComObjectInner, ComObjectInterface, GUID, IUnknownImpl, Interface, OutRef, Ref, Result,
    implement,
};

use wcontainers_direct3d_11::desc::{self, InputElement, SubresourceData};
use wcontainers_direct3d_11::format;
use wcontainers_gpu::SourceStage;

use super::counted::{Counted, Held};
use super::*;

#[implement(ID3D11Device, IDXGIDevice)]
pub struct Device {
    pub front: Locked<Shared>,
    pub(super) private: Private,
    /// The immediate context while it lives, held without a reference (it holds one on the device).
    context: Locked<Rc<Cell<*mut c_void>>>,
    adapter: IDXGIAdapter1,
    flags: u32,
    exception_mode: Cell<u32>,
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
    let backend = backend().ok_or(windows::Win32::Graphics::Dxgi::DXGI_ERROR_UNSUPPORTED)?;
    let front = Front::new(backend, levels).map_err(hresult)?;
    let adapter = Adapter::new(wcontainers_gpu::Backend::caps(&front.backend));
    let level = front.feature_level();
    let device: ID3D11Device = Device {
        front: Locked::new(Rc::new(RefCell::new(front))),
        private: Private::default(),
        context: Locked::new(Rc::new(Cell::new(std::ptr::null_mut()))),
        adapter: adapter.into(),
        flags,
        exception_mode: Cell::new(0),
        states: States::default(),
    }
    .into();
    let context = unsafe { device.GetImmediateContext() }?;
    Ok((device, context, level))
}

/// Writes `object` to an out parameter, or reports that the call only checks (a null out pointer).
fn give<I: windows::core::Type<I, Default = Option<I>>>(out: OutRef<I>, object: I) -> Result<()> {
    if out.is_null() {
        return Err(windows::Win32::Foundation::S_FALSE.into());
    }
    out.write(Some(object))
}

impl Device_Impl {
    fn me(&self) -> ID3D11Device {
        self.to_interface()
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
        let bytes = unsafe { init.as_ref() }
            .filter(|i| !i.pSysMem.is_null())
            .map(|i| unsafe {
                std::slice::from_raw_parts(i.pSysMem.cast::<u8>(), d.byte_width as usize)
            });
        let front = self
            .front
            .borrow_mut()
            .create_buffer(&d, bytes)
            .map_err(hresult)?;
        give(out, Buffer::new(front, self.me()).into())
    }

    pub(super) fn create_texture2_d(
        &self,
        desc: *const D3D11_TEXTURE2D_DESC,
        init: *const D3D11_SUBRESOURCE_DATA,
        out: OutRef<ID3D11Texture2D>,
    ) -> Result<()> {
        let d = convert::texture2d(unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?);
        let mut data = Vec::new();
        if !init.is_null() {
            // The description is checked before the program's array of initial data is read by it.
            let (fmt, mips) = self
                .front
                .borrow()
                .check_texture_2d(&d, true)
                .map_err(hresult)?;
            for i in 0..mips * d.array_size {
                let s = unsafe { &*init.add(i as usize) };
                let mip = i % mips;
                let (w, h) = ((d.width >> mip).max(1), (d.height >> mip).max(1));
                let (row, rows) = format::row_bytes(fmt, w, h);
                let len = (s.SysMemPitch * (rows - 1) + row) as usize;
                if s.pSysMem.is_null() {
                    return Err(E_INVALIDARG.into());
                }
                data.push(SubresourceData {
                    bytes: unsafe { std::slice::from_raw_parts(s.pSysMem.cast::<u8>(), len) },
                    row_pitch: s.SysMemPitch,
                    slice_pitch: s.SysMemSlicePitch,
                });
            }
        }
        let front = self
            .front
            .borrow_mut()
            .create_texture_2d(&d, &data)
            .map_err(hresult)?;
        give(out, Texture2D::new(front, self.me()).into())
    }

    /// The front texture behind a resource, refusing buffers (no buffer views yet) and foreign objects.
    fn texture_of(
        resource: &Ref<ID3D11Resource>,
    ) -> Result<Rc<wcontainers_direct3d_11::object::Texture2D<Chosen>>> {
        match resource.as_ref().and_then(resource_of) {
            Some(wcontainers_direct3d_11::object::Resource::Texture2D(t)) => Ok(t),
            Some(_) => Err(missing("buffer views")),
            None => Err(E_INVALIDARG.into()),
        }
    }

    pub(super) fn create_shader_resource_view(
        &self,
        resource: Ref<ID3D11Resource>,
        desc: *const D3D11_SHADER_RESOURCE_VIEW_DESC,
        out: OutRef<ID3D11ShaderResourceView>,
    ) -> Result<()> {
        let texture = Self::texture_of(&resource)?;
        let d = unsafe { desc.as_ref() }.map(convert::srv).transpose()?;
        let view = self
            .front
            .borrow_mut()
            .create_view(&texture, d.as_ref(), desc::bind::SHADER_RESOURCE)
            .map_err(hresult)?;
        let r = resource
            .as_ref()
            .and_then(HeldResource::of)
            .ok_or(E_INVALIDARG)?;
        give(out, ShaderResourceView::new(view, r, self.me()).into())
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
        shared_state::<BlendState, _>(&self.states.blend, blend_key(d), out, || {
            let state = self
                .front
                .borrow_mut()
                .create_blend_state(&convert::blend(d))
                .map_err(hresult)?;
            Ok(BlendState::new(state, *d, self.me()).into())
        })
    }

    pub(super) fn create_depth_stencil_state(
        &self,
        desc: *const D3D11_DEPTH_STENCIL_DESC,
        out: OutRef<ID3D11DepthStencilState>,
    ) -> Result<()> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
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
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        shared_state::<SamplerState, _>(&self.states.sampler, words(d), out, || {
            let state = self
                .front
                .borrow_mut()
                .create_sampler_state(&convert::sampler(d))
                .map_err(hresult)?;
            Ok(SamplerState::new(state, *d, self.me()).into())
        })
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
        let raw = self.context.get();
        let context: ID3D11DeviceContext = if raw.is_null() {
            let c: ID3D11DeviceContext =
                Context::new(self.front.clone(), self.me(), self.context.clone()).into();
            self.context.set(c.as_raw());
            c
        } else {
            unsafe { ID3D11DeviceContext::from_raw_borrowed(&raw) }
                .cloned()
                .expect("the context lives")
        };
        let _ = out.write(Some(context));
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

    pub(super) fn get_adapter(&self) -> Result<IDXGIAdapter> {
        self.adapter.cast()
    }
}
