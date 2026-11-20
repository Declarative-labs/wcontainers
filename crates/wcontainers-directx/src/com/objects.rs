//! The device's children: resources, views, states, input layouts and shaders.

use std::cell::Cell;
use std::rc::Rc;

use wcontainers_direct3d_11::desc;
use windows::Wdk::Graphics::Direct3D::{
    D3DDDIFMT_A8R8G8B8, D3DKMT_CREATEDCFROMMEMORY, D3DKMT_DESTROYDCFROMMEMORY,
    D3DKMTCreateDCFromMemory, D3DKMTDestroyDCFromMemory,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D10::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::{CreateCompatibleDC, DeleteDC, GdiFlush, HDC};
use windows::core::{BOOL, OutRef, Result};

use super::counted::{Held, counted};
use super::sharing::{self, Share};

use wcontainers_direct3d_11::object as front;

use super::*;

/// The device front of a device of ours.
fn device_front(device: &ID3D11Device) -> Shared {
    device
        .cast_object_ref::<Device>()
        .expect("a resource's device is ours")
        .front
        .clone()
}

/// What a buffer and a texture share: their share's handles and keyed mutex.
macro_rules! sharing_methods {
    () => {
        /// The object with its share (an opened resource's, or a made one's).
        pub fn sharing(self, share: Option<Rc<Share>>) -> Self {
            Self {
                share: Locked::new(share),
                ..self
            }
        }

        /// `IDXGIResource::GetSharedHandle`: none for a resource not shared.
        pub(super) fn shared_handle(&self) -> Result<HANDLE> {
            self.share
                .as_ref()
                .map_or(Ok(HANDLE::default()), |s| s.kmt_handle())
        }

        /// `IDXGIResource1::CreateSharedHandle`: only for a resource shared through NT handles.
        pub(super) fn nt_shared_handle(
            &self,
            attributes: *const windows::Win32::Security::SECURITY_ATTRIBUTES,
            name: windows::core::PCWSTR,
        ) -> Result<HANDLE> {
            match self.share.as_ref() {
                Some(s) => s.nt_handle(attributes, name),
                None => Err(windows::Win32::Foundation::E_INVALIDARG.into()),
            }
        }

        fn keyed_mutex(&self) -> bool {
            self.share.as_ref().is_some_and(|s| s.keyed())
        }

        /// `IDXGIKeyedMutex::AcquireSync`, which takes the library's lock itself to let it go
        /// while waiting.
        pub(super) fn acquire_sync(&self, key: u64, milliseconds: u32) -> Result<()> {
            let mut held = lock();
            let share = self.share.as_ref().ok_or(DXGI_ERROR_INVALID_CALL)?.clone();
            let device = self.child.front();
            share.acquire(&mut held, &device, key, milliseconds)
        }

        pub(super) fn release_sync(&self, key: u64) -> Result<()> {
            let share = self.share.as_ref().ok_or(DXGI_ERROR_INVALID_CALL)?;
            share.release(&self.child.front(), key)
        }
    };
}

/// The front resource behind a resource of ours; none for a foreign object.
pub fn resource_of(r: &ID3D11Resource) -> Option<front::Resource<Chosen>> {
    HeldResource::of(r).map(|h| h.front())
}

/// A resource of ours, held with a library reference.
#[derive(Clone)]
pub enum HeldResource {
    Buffer(Held<Buffer_Impl>),
    Texture1D(Held<Texture1D_Impl>),
    Texture2D(Held<Texture2D_Impl>),
    Texture3D(Held<Texture3D_Impl>),
}

impl HeldResource {
    /// None for a foreign object.
    pub fn of(r: &ID3D11Resource) -> Option<Self> {
        if let Ok(b) = r.cast_object_ref::<Buffer>() {
            return Some(HeldResource::Buffer(Held::new(b)));
        }
        if let Ok(t) = r.cast_object_ref::<Texture2D>() {
            return Some(HeldResource::Texture2D(Held::new(t)));
        }
        if let Ok(t) = r.cast_object_ref::<Texture1D>() {
            return Some(HeldResource::Texture1D(Held::new(t)));
        }
        r.cast_object_ref::<Texture3D>()
            .ok()
            .map(|t| HeldResource::Texture3D(Held::new(t)))
    }

    pub fn front(&self) -> front::Resource<Chosen> {
        match self {
            HeldResource::Buffer(b) => front::Resource::Buffer(b.front.clone()),
            HeldResource::Texture1D(t) => front::Resource::Texture(t.front.clone()),
            HeldResource::Texture2D(t) => front::Resource::Texture(t.front.clone()),
            HeldResource::Texture3D(t) => front::Resource::Texture(t.front.clone()),
        }
    }

    /// The resource with a reference of the program's.
    pub fn public(&self) -> ID3D11Resource {
        match self {
            HeldResource::Buffer(b) => b.public::<ID3D11Buffer>().into(),
            HeldResource::Texture1D(t) => t.public::<ID3D11Texture1D>().into(),
            HeldResource::Texture2D(t) => t.public::<ID3D11Texture2D>().into(),
            HeldResource::Texture3D(t) => t.public::<ID3D11Texture3D>().into(),
        }
    }
}

pub struct Buffer {
    pub front: Locked<Rc<front::Buffer<Chosen>>>,
    pub(super) child: Child,
    priority: Cell<u32>,
    share: Locked<Option<Rc<Share>>>,
}

counted!(Buffer => Buffer_Impl {
    main: ID3D11Buffer, ID3D11Buffer_Vtbl;
    dxgi: IDXGIResource1, IDXGIResource1_Vtbl, -2;
    surface: IDXGISurface1, IDXGISurface1_Vtbl, -3;
    d3d10: ID3D10Buffer, ID3D10Buffer_Vtbl, -4, if offers_d3d10;
    keyed: IDXGIKeyedMutex, IDXGIKeyedMutex_Vtbl, -5, if keyed_mutex;
});

impl Buffer {
    /// `IDXGISurface1::GetDC`: buffers are never GDI-compatible.
    pub(super) fn get_dc(&self, _discard: BOOL) -> Result<HDC> {
        Err(DXGI_ERROR_INVALID_CALL.into())
    }

    pub(super) fn release_dc(&self, _dirty: *const RECT) -> Result<()> {
        Err(DXGI_ERROR_INVALID_CALL.into())
    }

    pub fn new(front: Rc<front::Buffer<Chosen>>, device: ID3D11Device) -> Self {
        Buffer {
            front: Locked::new(front),
            child: Child::new(device),
            priority: Cell::new(DXGI_RESOURCE_PRIORITY_NORMAL.0),
            share: Locked::new(None),
        }
    }

    /// A buffer just made: shared if its misc flags say so.
    pub fn made(front: Rc<front::Buffer<Chosen>>, device: ID3D11Device) -> Self {
        let share = sharing::made(
            &device_front(&device),
            &front::Resource::Buffer(front.clone()),
        );
        Buffer::new(front, device).sharing(share)
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_BUFFER_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = convert::buffer_back(&self.front.desc);
        }
    }

    pub(super) fn get_type(&self, out: *mut D3D11_RESOURCE_DIMENSION) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = D3D11_RESOURCE_DIMENSION_BUFFER;
        }
    }

    pub(super) fn set_eviction_priority(&self, p: u32) {
        self.priority.set(p);
    }

    pub(super) fn get_eviction_priority(&self) -> u32 {
        self.priority.get()
    }

    pub(super) fn get_usage(&self) -> Result<DXGI_USAGE> {
        Ok(dxgi_usage(self.front.desc.bind_flags))
    }

    sharing_methods!();

    /// DXGI's surface of a buffer: a row of its bytes of no format.
    pub(super) fn get_desc_dxgi(&self) -> Result<DXGI_SURFACE_DESC> {
        Ok(DXGI_SURFACE_DESC {
            Width: self.front.desc.byte_width,
            Height: 1,
            Format: DXGI_FORMAT(0),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
        })
    }
}

/// DXGI's usage of a resource of these bind flags.
fn dxgi_usage(bind: u32) -> DXGI_USAGE {
    let mut u = 0;
    if bind & D3D11_BIND_RENDER_TARGET.0 as u32 != 0 {
        u |= DXGI_USAGE_RENDER_TARGET_OUTPUT.0;
    }
    if bind & D3D11_BIND_SHADER_RESOURCE.0 as u32 != 0 {
        u |= DXGI_USAGE_SHADER_INPUT.0;
    }
    if bind & D3D11_BIND_UNORDERED_ACCESS.0 as u32 != 0 {
        u |= DXGI_USAGE_UNORDERED_ACCESS.0;
    }
    DXGI_USAGE(u)
}

/// A texture of one dimension: its interface, description structure, conversion and resource type.
macro_rules! texture {
    ($name:ident, $imp:ident, $iface:ident $(| $also:ident)*, $vtbl:ident, $desc:ident, $back:path, $dim:ident, $iface10:ident, $vtbl10:ident, $at10:expr $(, $surface:ident)?) => {
        pub struct $name {
            pub front: Locked<Rc<front::Texture<Chosen>>>,
            pub(super) child: Child,
            priority: Cell<u32>,
            /// The device context `GetDC` handed out, until `ReleaseDC`.
            dc: Cell<Option<Dc>>,
            share: Locked<Option<Rc<Share>>>,
        }

        counted!($name => $imp {
            main: $iface $(| $also)*, $vtbl;
            dxgi: IDXGIResource1, IDXGIResource1_Vtbl, -2;
            $($surface: IDXGISurface1, IDXGISurface1_Vtbl, -3, if one_subresource;)?
            d3d10: $iface10, $vtbl10, $at10, if offers_d3d10;
            keyed: IDXGIKeyedMutex, IDXGIKeyedMutex_Vtbl, $at10 - 1, if keyed_mutex;
        });

        impl $name {
            pub fn new(front: Rc<front::Texture<Chosen>>, device: ID3D11Device) -> Self {
                $name {
                    front: Locked::new(front),
                    child: Child::new(device),
                    priority: Cell::new(DXGI_RESOURCE_PRIORITY_NORMAL.0),
                    dc: Cell::new(None),
                    share: Locked::new(None),
                }
            }

            /// A texture just made: shared if its misc flags say so.
            pub fn made(front: Rc<front::Texture<Chosen>>, device: ID3D11Device) -> Self {
                let share = sharing::made(
                    &device_front(&device),
                    &front::Resource::Texture(front.clone()),
                );
                $name::new(front, device).sharing(share)
            }

            /// `IDXGISurface1::GetDC`: a memory device context on a bitmap holding the
            /// texels of the texture's one subresource (left undefined with `discard`), written
            /// back at `ReleaseDC`.
            #[allow(dead_code)]
            pub(super) fn get_dc(&self, discard: BOOL) -> Result<HDC> {
                let t: &Rc<front::Texture<Chosen>> = &self.front;
                if t.desc.misc_flags & desc::MISC_GDI_COMPATIBLE == 0 {
                    return Err(DXGI_ERROR_INVALID_CALL.into());
                }
                if let Some(held) = self.dc.take() {
                    self.dc.set(Some(held));
                    return Err(DXGI_ERROR_INVALID_CALL.into());
                }
                let Some(mut dc) = Dc::new(t.desc.width, t.desc.height) else {
                    return Err(windows::Win32::Foundation::E_OUTOFMEMORY.into());
                };
                if !discard.as_bool()
                    && let Err(e) = dc.read(&self.child.front(), t)
                {
                    dc.delete();
                    return Err(hresult(e));
                }
                let handle = dc.dc;
                self.dc.set(Some(dc));
                Ok(handle)
            }

            /// `IDXGISurface1::ReleaseDC`: what GDI drew is written into the texture.
            #[allow(dead_code)]
            pub(super) fn release_dc(&self, _dirty: *const RECT) -> Result<()> {
                let Some(dc) = self.dc.take() else {
                    return Err(DXGI_ERROR_INVALID_CALL.into());
                };
                let written = dc.write(&self.child.front(), &self.front);
                dc.delete();
                written.map_err(hresult)
            }

            pub(super) fn get_desc(&self, out: *mut $desc) {
                if let Some(out) = unsafe { out.as_mut() } {
                    *out = $back(&self.front.desc);
                }
            }

            pub(super) fn get_type(&self, out: *mut D3D11_RESOURCE_DIMENSION) {
                if let Some(out) = unsafe { out.as_mut() } {
                    *out = $dim;
                }
            }

            pub(super) fn set_eviction_priority(&self, p: u32) {
                self.priority.set(p);
            }

            pub(super) fn get_eviction_priority(&self) -> u32 {
                self.priority.get()
            }

            pub(super) fn get_usage(&self) -> Result<DXGI_USAGE> {
                Ok(dxgi_usage(self.front.desc.bind_flags))
            }

            sharing_methods!();

            /// Whether DXGI sees the texture as a surface: one level of one layer or slice.
            fn one_subresource(&self) -> bool {
                let d = &self.front.desc;
                d.mip_levels == 1 && d.array_size == 1 && d.depth <= 1
            }

            #[allow(dead_code)]
            pub(super) fn get_desc_dxgi(&self) -> Result<DXGI_SURFACE_DESC> {
                let d = &self.front.desc;
                Ok(DXGI_SURFACE_DESC {
                    Width: d.width,
                    Height: d.height,
                    Format: DXGI_FORMAT(d.format as i32),
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: d.sample_count,
                        Quality: d.sample_quality,
                    },
                })
            }
        }
    };
}

/// A memory device context on a bitmap of 32-bit texels in our memory, as DXGI makes them
/// (`D3DKMTCreateDCFromMemory`).
struct Dc {
    device: HDC,
    dc: HDC,
    bitmap: HANDLE,
    bits: Box<[u8]>,
    width: u32,
    height: u32,
}

impl Dc {
    fn new(width: u32, height: u32) -> Option<Dc> {
        let mut bits = vec![0u8; width as usize * height as usize * 4].into_boxed_slice();
        let device = unsafe { CreateCompatibleDC(None) };
        if device.is_invalid() {
            return None;
        }
        let mut made = D3DKMT_CREATEDCFROMMEMORY {
            pMemory: bits.as_mut_ptr().cast(),
            Format: D3DDDIFMT_A8R8G8B8,
            Width: width,
            Height: height,
            Pitch: width * 4,
            hDeviceDc: device,
            ..Default::default()
        };
        if unsafe { D3DKMTCreateDCFromMemory(&mut made) }.is_err() {
            unsafe {
                let _ = DeleteDC(device);
            }
            return None;
        }
        Some(Dc {
            device,
            dc: made.hDc,
            bitmap: made.hBitmap,
            bits,
            width,
            height,
        })
    }

    fn pitch(&self) -> usize {
        self.width as usize * 4
    }

    /// Copies the texture's texels into the bitmap, through a staging texture.
    fn read(
        &mut self,
        front: &Shared,
        t: &Rc<front::Texture<Chosen>>,
    ) -> wcontainers_direct3d_11::Result<()> {
        let mut f = front.borrow_mut();
        let staging = f.create_texture(
            &desc::TextureDesc {
                mip_levels: 1,
                usage: desc::usage::STAGING,
                bind_flags: 0,
                cpu_access_flags: desc::cpu::READ,
                misc_flags: 0,
                ..t.desc
            },
            &[],
        )?;
        let staging = front::Resource::Texture(staging);
        f.copy_subresource_region(
            &staging,
            0,
            [0; 3],
            &front::Resource::Texture(t.clone()),
            0,
            None,
        )?;
        let mapped = f.map(&staging, 0, desc::map::READ, 0)?;
        for y in 0..self.height as usize {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    mapped.data.add(y * mapped.row_pitch as usize),
                    self.bits.as_mut_ptr().add(y * self.pitch()),
                    self.pitch(),
                )
            };
        }
        f.unmap(&staging, 0);
        Ok(())
    }

    /// Writes the bitmap's texels into the texture.
    fn write(
        &self,
        front: &Shared,
        t: &Rc<front::Texture<Chosen>>,
    ) -> wcontainers_direct3d_11::Result<()> {
        unsafe { GdiFlush().ok().ok() };
        front.borrow_mut().update_subresource(
            &front::Resource::Texture(t.clone()),
            0,
            None,
            &self.bits,
            self.pitch() as u32,
            0,
        )
    }

    fn delete(self) {
        let gone = D3DKMT_DESTROYDCFROMMEMORY {
            hDc: self.dc,
            hBitmap: self.bitmap,
        };
        unsafe {
            let _ = D3DKMTDestroyDCFromMemory(&gone);
            let _ = DeleteDC(self.device);
        }
    }
}

texture!(
    Texture1D,
    Texture1D_Impl,
    ID3D11Texture1D,
    ID3D11Texture1D_Vtbl,
    D3D11_TEXTURE1D_DESC,
    convert::texture1d_back,
    D3D11_RESOURCE_DIMENSION_TEXTURE1D,
    ID3D10Texture1D,
    ID3D10Texture1D_Vtbl,
    -4,
    surface
);
texture!(
    Texture2D,
    Texture2D_Impl,
    ID3D11Texture2D1 | ID3D11Texture2D,
    ID3D11Texture2D1_Vtbl,
    D3D11_TEXTURE2D_DESC,
    convert::texture2d_back,
    D3D11_RESOURCE_DIMENSION_TEXTURE2D,
    ID3D10Texture2D,
    ID3D10Texture2D_Vtbl,
    -4,
    surface
);
texture!(
    Texture3D,
    Texture3D_Impl,
    ID3D11Texture3D1 | ID3D11Texture3D,
    ID3D11Texture3D1_Vtbl,
    D3D11_TEXTURE3D_DESC,
    convert::texture3d_back,
    D3D11_RESOURCE_DIMENSION_TEXTURE3D,
    ID3D10Texture3D,
    ID3D10Texture3D_Vtbl,
    -4,
    surface
);

/// Direct3D 11.3's descriptions of textures: no layout but the undefined one is served.
impl Texture2D {
    pub(super) fn get_desc1(&self, out: *mut D3D11_TEXTURE2D_DESC1) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = unsafe { convert::same_fields(&convert::texture2d_back(&self.front.desc)) };
        }
    }
}

impl Texture3D {
    pub(super) fn get_desc1(&self, out: *mut D3D11_TEXTURE3D_DESC1) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = unsafe { convert::same_fields(&convert::texture3d_back(&self.front.desc)) };
        }
    }
}

/// A view: the front view and the resource it views (held with a library reference).
pub struct RenderTargetView {
    pub front: Locked<Rc<front::View<Chosen>>>,
    pub(super) resource: HeldResource,
    pub(super) child: Child,
    /// The description of a view of a buffer, as the program gave it.
    pub(super) buffer: Option<D3D11_RENDER_TARGET_VIEW_DESC>,
}

counted!(RenderTargetView => RenderTargetView_Impl {
    main: ID3D11RenderTargetView1 | ID3D11RenderTargetView, ID3D11RenderTargetView1_Vtbl;
    d3d10: ID3D10RenderTargetView, ID3D10RenderTargetView_Vtbl, -2;
});

impl RenderTargetView {
    pub fn new(
        front: Rc<front::View<Chosen>>,
        resource: HeldResource,
        device: ID3D11Device,
    ) -> Self {
        RenderTargetView {
            front: Locked::new(front),
            resource,
            child: Child::new(device),
            buffer: None,
        }
    }

    fn desc(&self) -> D3D11_RENDER_TARGET_VIEW_DESC {
        self.buffer
            .unwrap_or_else(|| convert::rtv_back(&self.front.desc))
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_RENDER_TARGET_VIEW_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc();
        }
    }

    pub(super) fn get_desc1(&self, out: *mut D3D11_RENDER_TARGET_VIEW_DESC1) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = unsafe { convert::same_fields(&self.desc()) };
        }
    }

    pub(super) fn get_resource(&self, out: OutRef<ID3D11Resource>) {
        let _ = out.write(Some(self.resource.public()));
    }
}

/// A view: the front view and the resource it views (held with a library reference).
pub struct DepthStencilView {
    pub front: Locked<Rc<front::View<Chosen>>>,
    pub(super) resource: HeldResource,
    pub(super) child: Child,
}

counted!(DepthStencilView => DepthStencilView_Impl {
    main: ID3D11DepthStencilView, ID3D11DepthStencilView_Vtbl;
    d3d10: ID3D10DepthStencilView, ID3D10DepthStencilView_Vtbl, -2;
});

impl DepthStencilView {
    pub fn new(
        front: Rc<front::View<Chosen>>,
        resource: HeldResource,
        device: ID3D11Device,
    ) -> Self {
        DepthStencilView {
            front: Locked::new(front),
            resource,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_DEPTH_STENCIL_VIEW_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = convert::dsv_back(&self.front.desc);
        }
    }

    pub(super) fn get_resource(&self, out: OutRef<ID3D11Resource>) {
        let _ = out.write(Some(self.resource.public()));
    }
}

/// A view: the front view, its description and the resource it views (held with a library
/// reference).
pub struct ShaderResourceView {
    pub front: Locked<Rc<front::ShaderView<Chosen>>>,
    pub(super) desc: D3D11_SHADER_RESOURCE_VIEW_DESC,
    pub(super) resource: HeldResource,
    pub(super) child: Child,
}

counted!(ShaderResourceView => ShaderResourceView_Impl {
    main: ID3D11ShaderResourceView1 | ID3D11ShaderResourceView, ID3D11ShaderResourceView1_Vtbl;
    d3d10: ID3D10ShaderResourceView1, ID3D10ShaderResourceView1_Vtbl, -2, if offers_d3d10;
});

impl ShaderResourceView {
    pub fn new(
        front: Rc<front::ShaderView<Chosen>>,
        desc: D3D11_SHADER_RESOURCE_VIEW_DESC,
        resource: HeldResource,
        device: ID3D11Device,
    ) -> Self {
        ShaderResourceView {
            front: Locked::new(front),
            desc,
            resource,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_SHADER_RESOURCE_VIEW_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }

    pub(super) fn get_desc1(&self, out: *mut D3D11_SHADER_RESOURCE_VIEW_DESC1) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = unsafe { convert::same_fields(&self.desc) };
        }
    }

    pub(super) fn get_resource(&self, out: OutRef<ID3D11Resource>) {
        let _ = out.write(Some(self.resource.public()));
    }
}

/// An unordered access view: the front view, its description and the resource it views.
pub struct UnorderedAccessView {
    pub front: Locked<Rc<front::ShaderView<Chosen>>>,
    desc: D3D11_UNORDERED_ACCESS_VIEW_DESC,
    pub(super) resource: HeldResource,
    pub(super) child: Child,
}

counted!(UnorderedAccessView => UnorderedAccessView_Impl {
    main: ID3D11UnorderedAccessView1 | ID3D11UnorderedAccessView, ID3D11UnorderedAccessView1_Vtbl
});

impl UnorderedAccessView {
    pub fn new(
        front: Rc<front::ShaderView<Chosen>>,
        desc: D3D11_UNORDERED_ACCESS_VIEW_DESC,
        resource: HeldResource,
        device: ID3D11Device,
    ) -> Self {
        UnorderedAccessView {
            front: Locked::new(front),
            desc,
            resource,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_UNORDERED_ACCESS_VIEW_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }

    pub(super) fn get_desc1(&self, out: *mut D3D11_UNORDERED_ACCESS_VIEW_DESC1) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = unsafe { convert::same_fields(&self.desc) };
        }
    }

    pub(super) fn get_resource(&self, out: OutRef<ID3D11Resource>) {
        let _ = out.write(Some(self.resource.public()));
    }
}

/// A state object and the description it was made from, which `GetDesc` returns.
pub struct SamplerState {
    pub front: Locked<Rc<front::Sampler<Chosen>>>,
    desc: D3D11_SAMPLER_DESC,
    pub(super) child: Child,
}

counted!(SamplerState => SamplerState_Impl {
    main: ID3D11SamplerState, ID3D11SamplerState_Vtbl;
    d3d10: ID3D10SamplerState, ID3D10SamplerState_Vtbl, -2;
});

impl SamplerState {
    pub fn new(
        front: Rc<front::Sampler<Chosen>>,
        desc: D3D11_SAMPLER_DESC,
        device: ID3D11Device,
    ) -> Self {
        SamplerState {
            front: Locked::new(front),
            desc,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_SAMPLER_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }
}

/// A state object and the description it was made from, which `GetDesc` returns.
pub struct BlendState {
    pub front: Locked<Rc<front::Blend<Chosen>>>,
    desc: D3D11_BLEND_DESC,
    logic: convert::LogicOps,
    pub(super) child: Child,
}

counted!(BlendState => BlendState_Impl {
    main: ID3D11BlendState1 | ID3D11BlendState, ID3D11BlendState1_Vtbl;
    d3d10: ID3D10BlendState1, ID3D10BlendState1_Vtbl, -2;
});

impl BlendState {
    pub fn new(
        front: Rc<front::Blend<Chosen>>,
        desc: D3D11_BLEND_DESC,
        logic: convert::LogicOps,
        device: ID3D11Device,
    ) -> Self {
        BlendState {
            front: Locked::new(front),
            desc,
            logic,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_BLEND_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }

    pub(super) fn get_desc1(&self, out: *mut D3D11_BLEND_DESC1) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = convert::blend_joined(&self.desc, &self.logic);
        }
    }
}

/// A state object and the description it was made from, which `GetDesc` returns.
pub struct DepthStencilState {
    pub front: Locked<Rc<front::DepthStencil<Chosen>>>,
    desc: D3D11_DEPTH_STENCIL_DESC,
    pub(super) child: Child,
}

counted!(DepthStencilState => DepthStencilState_Impl {
    main: ID3D11DepthStencilState, ID3D11DepthStencilState_Vtbl;
    d3d10: ID3D10DepthStencilState, ID3D10DepthStencilState_Vtbl, -2;
});

impl DepthStencilState {
    pub fn new(
        front: Rc<front::DepthStencil<Chosen>>,
        desc: D3D11_DEPTH_STENCIL_DESC,
        device: ID3D11Device,
    ) -> Self {
        DepthStencilState {
            front: Locked::new(front),
            desc,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_DEPTH_STENCIL_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }
}

/// A state object and the description it was made from, which `GetDesc` returns.
pub struct RasterizerState {
    pub front: Locked<Rc<front::Rasterizer<Chosen>>>,
    desc: D3D11_RASTERIZER_DESC,
    pub(super) child: Child,
}

counted!(RasterizerState => RasterizerState_Impl {
    main: ID3D11RasterizerState2 | ID3D11RasterizerState1 | ID3D11RasterizerState, ID3D11RasterizerState2_Vtbl;
    d3d10: ID3D10RasterizerState, ID3D10RasterizerState_Vtbl, -2;
});

impl RasterizerState {
    pub fn new(
        front: Rc<front::Rasterizer<Chosen>>,
        desc: D3D11_RASTERIZER_DESC,
        device: ID3D11Device,
    ) -> Self {
        RasterizerState {
            front: Locked::new(front),
            desc,
            child: Child::new(device),
        }
    }

    pub(super) fn get_desc(&self, out: *mut D3D11_RASTERIZER_DESC) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = self.desc;
        }
    }

    pub(super) fn get_desc1(&self, out: *mut D3D11_RASTERIZER_DESC1) {
        let d = &self.desc;
        if let Some(out) = unsafe { out.as_mut() } {
            *out = D3D11_RASTERIZER_DESC1 {
                FillMode: d.FillMode,
                CullMode: d.CullMode,
                FrontCounterClockwise: d.FrontCounterClockwise,
                DepthBias: d.DepthBias,
                DepthBiasClamp: d.DepthBiasClamp,
                SlopeScaledDepthBias: d.SlopeScaledDepthBias,
                DepthClipEnable: d.DepthClipEnable,
                ScissorEnable: d.ScissorEnable,
                MultisampleEnable: d.MultisampleEnable,
                AntialiasedLineEnable: d.AntialiasedLineEnable,
                ForcedSampleCount: 0,
            };
        }
    }

    /// No conservative rasterization is served, so it is always off.
    pub(super) fn get_desc2(&self, out: *mut D3D11_RASTERIZER_DESC2) {
        let mut d = D3D11_RASTERIZER_DESC1::default();
        self.get_desc1(&mut d);
        if let Some(out) = unsafe { out.as_mut() } {
            *out = unsafe { convert::same_fields(&d) };
        }
    }
}

pub struct InputLayout {
    pub front: Locked<Rc<front::InputLayout>>,
    pub(super) child: Child,
}

counted!(InputLayout => InputLayout_Impl {
    main: ID3D11InputLayout, ID3D11InputLayout_Vtbl;
    d3d10: ID3D10InputLayout, ID3D10InputLayout_Vtbl, -2;
});

impl InputLayout {
    pub fn new(front: Rc<front::InputLayout>, device: ID3D11Device) -> Self {
        InputLayout {
            front: Locked::new(front),
            child: Child::new(device),
        }
    }
}

/// A deferred context's command list: the calls it recorded, run by `ExecuteCommandList`.
pub struct CommandList {
    pub commands: Locked<Rc<Vec<super::context::Command>>>,
    pub(super) child: Child,
}

counted!(CommandList => CommandList_Impl { main: ID3D11CommandList, ID3D11CommandList_Vtbl });

impl CommandList {
    pub fn new(commands: Rc<Vec<super::context::Command>>, device: ID3D11Device) -> Self {
        CommandList {
            commands: Locked::new(commands),
            child: Child::new(device),
        }
    }

    pub(super) fn get_context_flags(&self) -> u32 {
        0
    }
}

pub struct VertexShader {
    pub front: Locked<Rc<front::Shader<Chosen>>>,
    pub(super) child: Child,
}

counted!(VertexShader => VertexShader_Impl {
    main: ID3D11VertexShader, ID3D11VertexShader_Vtbl;
    d3d10: ID3D10VertexShader, ID3D10VertexShader_Vtbl, -2;
});

impl VertexShader {
    pub fn new(front: Rc<front::Shader<Chosen>>, device: ID3D11Device) -> Self {
        VertexShader {
            front: Locked::new(front),
            child: Child::new(device),
        }
    }
}

pub struct PixelShader {
    pub front: Locked<Rc<front::Shader<Chosen>>>,
    pub(super) child: Child,
}

counted!(PixelShader => PixelShader_Impl {
    main: ID3D11PixelShader, ID3D11PixelShader_Vtbl;
    d3d10: ID3D10PixelShader, ID3D10PixelShader_Vtbl, -2;
});

impl PixelShader {
    pub fn new(front: Rc<front::Shader<Chosen>>, device: ID3D11Device) -> Self {
        PixelShader {
            front: Locked::new(front),
            child: Child::new(device),
        }
    }
}

pub struct GeometryShader {
    pub front: Locked<Rc<front::Geometry<Chosen>>>,
    pub(super) child: Child,
}

counted!(GeometryShader => GeometryShader_Impl {
    main: ID3D11GeometryShader, ID3D11GeometryShader_Vtbl;
    d3d10: ID3D10GeometryShader, ID3D10GeometryShader_Vtbl, -2;
});

impl GeometryShader {
    pub fn new(front: Rc<front::Geometry<Chosen>>, device: ID3D11Device) -> Self {
        GeometryShader {
            front: Locked::new(front),
            child: Child::new(device),
        }
    }
}

/// A hull or domain shader: the stage's slots and its conversion.
macro_rules! stage_shader {
    ($name:ident, $imp:ident, $iface:ident, $vtbl:ident) => {
        pub struct $name {
            pub front: Locked<Rc<front::Shader<Chosen>>>,
            pub(super) child: Child,
        }

        counted!($name => $imp { main: $iface, $vtbl });

        impl $name {
            pub fn new(front: Rc<front::Shader<Chosen>>, device: ID3D11Device) -> Self {
                $name {
                    front: Locked::new(front),
                    child: Child::new(device),
                }
            }
        }
    };
}

stage_shader!(
    HullShader,
    HullShader_Impl,
    ID3D11HullShader,
    ID3D11HullShader_Vtbl
);
stage_shader!(
    DomainShader,
    DomainShader_Impl,
    ID3D11DomainShader,
    ID3D11DomainShader_Vtbl
);

pub struct ComputeShader {
    pub front: Locked<Rc<front::Shader<Chosen>>>,
    pub(super) child: Child,
}

counted!(ComputeShader => ComputeShader_Impl { main: ID3D11ComputeShader, ID3D11ComputeShader_Vtbl });

impl ComputeShader {
    pub fn new(front: Rc<front::Shader<Chosen>>, device: ID3D11Device) -> Self {
        ComputeShader {
            front: Locked::new(front),
            child: Child::new(device),
        }
    }
}

/// A query or predicate: its interfaces (Direct3D 11's and 10's), and whether `QueryInterface` answers `ID3D11Predicate`.
macro_rules! asynchronous {
    ($name:ident, $imp:ident, $iface:ident $(| $also:ident)*, $vtbl:ident, $iface10:ident, $vtbl10:ident) => {
        pub struct $name {
            pub front: Locked<Rc<wcontainers_direct3d_11::query::Query<Chosen>>>,
            pub(super) child: Child,
        }

        counted!($name => $imp {
            main: $iface $(| $also)*, $vtbl;
            d3d10: $iface10, $vtbl10, -2;
        });

        impl $name {
            pub fn new(
                front: Rc<wcontainers_direct3d_11::query::Query<Chosen>>,
                device: ID3D11Device,
            ) -> Self {
                $name {
                    front: Locked::new(front),
                    child: Child::new(device),
                }
            }

            pub(super) fn get_desc(&self, out: *mut D3D11_QUERY_DESC) {
                if let Some(out) = unsafe { out.as_mut() } {
                    *out = D3D11_QUERY_DESC {
                        Query: D3D11_QUERY(self.front.kind as i32),
                        MiscFlags: self.front.misc,
                    };
                }
            }

            pub(super) fn get_data_size(&self) -> u32 {
                wcontainers_direct3d_11::query::data_size(self.front.kind).unwrap_or(0)
            }
        }
    };
}

asynchronous!(
    Query,
    Query_Impl,
    ID3D11Query1 | ID3D11Query,
    ID3D11Query1_Vtbl,
    ID3D10Query,
    ID3D10Query_Vtbl
);

impl Query {
    /// A query of all the work: one queue runs every kind.
    pub(super) fn get_desc1(&self, out: *mut D3D11_QUERY_DESC1) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = D3D11_QUERY_DESC1 {
                Query: D3D11_QUERY(self.front.kind as i32),
                MiscFlags: self.front.misc,
                ContextType: D3D11_CONTEXT_TYPE_ALL,
            };
        }
    }
}
asynchronous!(
    Predicate,
    Predicate_Impl,
    ID3D11Predicate,
    ID3D11Predicate_Vtbl,
    ID3D10Predicate,
    ID3D10Predicate_Vtbl
);

/// The front query behind a query or predicate of ours.
pub fn query_of(
    a: Option<&ID3D11Asynchronous>,
) -> Option<Rc<wcontainers_direct3d_11::query::Query<Chosen>>> {
    let a = a?;
    if let Ok(q) = a.cast_object_ref::<Query>() {
        return Some(q.front.clone());
    }
    a.cast_object_ref::<Predicate>()
        .ok()
        .map(|p| p.front.clone())
}
