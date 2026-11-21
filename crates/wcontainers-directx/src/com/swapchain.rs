//! DXGI's factory (the adapter's parent) and swap chains of Direct3D 11 devices, over the front's
//! swap chains (the shared infrastructure's) on the program's windows.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::{
    CloseHandle, DUPLICATE_SAME_ACCESS, DXGI_STATUS_OCCLUDED, DuplicateHandle, E_INVALIDARG,
    E_NOINTERFACE, HANDLE, HWND, RECT, S_OK,
};
use windows::Win32::Graphics::Direct3D11::{D3D11_BIND_SHADER_RESOURCE, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::{CreateSemaphoreW, GetCurrentProcess, ReleaseSemaphore};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, IsIconic, IsWindowVisible};
use windows::core::{
    BOOL, GUID, HRESULT, IUnknown, IUnknownImpl, Interface, OutRef, Ref, Result, implement,
};

use wcontainers_direct3d_11::{Chain, Model, SwapChainDesc};
use wcontainers_gpu::TextureUsage;

use super::counted::{Held, public_count};
use super::*;

/// `IDXGISwapChain::Present`'s largest sync interval.
const MAX_SYNC_INTERVAL: u32 = 4;

/// The frame latency of a swap chain made with a frame latency waitable object, until the
/// program sets another.
const WAITABLE_LATENCY: u32 = 1;

/// Not served: fullscreen swap chains.
const FULLSCREEN: &str = "fullscreen swap chains";

/// Not served: multisampled or stereo swap chains.
const MULTISAMPLED_OR_STEREO: &str = "multisampled or stereo swap chains";

/// Not served: swap chains rotating their back buffers.
const ROTATING: &str = "swap chains rotating their back buffers";

/// Not served: HDR metadata.
const HDR_METADATA: &str = "HDR metadata";

#[implement(IDXGIFactory6)]
pub struct Factory {
    pub(super) private: Private,
    adapter: IDXGIAdapter1,
    window: Cell<isize>,
}

impl Factory {
    pub fn new(adapter: IDXGIAdapter1) -> Self {
        Factory {
            private: Private::default(),
            adapter,
            window: Cell::new(0),
        }
    }
}

/// Our device behind `device`, if it is ours.
fn our_device(device: Ref<IUnknown>) -> Result<ID3D11Device> {
    let device: ID3D11Device = device.ok()?.cast()?;
    device.cast_object_ref::<Device>()?;
    Ok(device)
}

/// The size of the client area of `hwnd`, which a swap chain of size zero takes.
fn client_size(hwnd: HWND) -> (u32, u32) {
    let mut r = RECT::default();
    // SAFETY: writes the window's rectangle into `r`.
    if unsafe { GetClientRect(hwnd, &mut r) }.is_err() {
        return (0, 0);
    }
    (
        (r.right - r.left).max(0) as u32,
        (r.bottom - r.top).max(0) as u32,
    )
}

/// Registers `hwnd` with the device's back end as a host window and gives its number.
fn add_window(front: &Shared, hwnd: HWND) -> Result<u32> {
    static WINDOWS: AtomicU32 = AtomicU32::new(0);
    let id = WINDOWS.fetch_add(1, Ordering::Relaxed) + 1;
    chosen::add_window(&mut front.borrow_mut().backend, id, hwnd)?;
    Ok(id)
}

impl Factory_Impl {
    fn me(&self) -> IDXGIFactory2 {
        self.to_interface::<IDXGIFactory6>().into()
    }

    pub(super) fn get_parent(&self, _riid: *const GUID, parent: *mut *mut c_void) -> Result<()> {
        if let Some(p) = unsafe { parent.as_mut() } {
            *p = std::ptr::null_mut();
        }
        Err(E_NOINTERFACE.into())
    }

    pub(super) fn enum_adapters(&self, index: u32) -> Result<IDXGIAdapter> {
        self.enum_adapters1(index)?.cast()
    }

    /// A new object for the one adapter at each call, as DXGI's enumerations give.
    pub(super) fn enum_adapters1(&self, index: u32) -> Result<IDXGIAdapter1> {
        if index != 0 {
            return Err(DXGI_ERROR_NOT_FOUND.into());
        }
        let adapter = self.adapter.cast_object_ref::<Adapter>()?;
        Ok(adapter.another(self.to_interface()).into())
    }

    /// The debug layer the flag asks for is not served, so no flag holds.
    pub(super) fn get_creation_flags(&self) -> DXGI_CREATE_FACTORY_FLAGS {
        DXGI_CREATE_FACTORY_FLAGS(0)
    }

    /// The one adapter, by its identifier.
    pub(super) fn enum_adapter_by_luid(
        &self,
        luid: &windows::Win32::Foundation::LUID,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        let Some(p) = (unsafe { out.as_mut() }) else {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        };
        *p = std::ptr::null_mut();
        let ours = unsafe { self.adapter.GetDesc1()? }.AdapterLuid;
        if (ours.LowPart, ours.HighPart) != (luid.LowPart, luid.HighPart) {
            return Err(DXGI_ERROR_NOT_FOUND.into());
        }
        let adapter = self.enum_adapters1(0)?;
        unsafe { adapter.query(riid, out).ok() }
    }

    /// The one adapter, whatever the preference.
    pub(super) fn enum_adapter_by_gpu_preference(
        &self,
        index: u32,
        _preference: DXGI_GPU_PREFERENCE,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        if let Some(p) = unsafe { out.as_mut() } {
            *p = std::ptr::null_mut();
        }
        let adapter = self.enum_adapters1(index)?;
        unsafe { adapter.query(riid, out).ok() }
    }

    /// Presenting without waiting for the display (tearing) is not offered: browsers pace frames.
    /// Another feature or a size other than the answer's is `DXGI_ERROR_INVALID_CALL`, as DXGI
    /// answers.
    pub(super) fn check_feature_support(
        &self,
        feature: DXGI_FEATURE,
        data: *mut c_void,
        size: u32,
    ) -> Result<()> {
        if feature != DXGI_FEATURE_PRESENT_ALLOW_TEARING
            || data.is_null()
            || size as usize != size_of::<BOOL>()
        {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
        unsafe { *data.cast::<BOOL>() = false.into() };
        Ok(())
    }

    pub(super) fn is_current(&self) -> BOOL {
        true.into()
    }

    pub(super) fn is_windowed_stereo_enabled(&self) -> BOOL {
        false.into()
    }

    /// Kept for `GetWindowAssociation`; the keys it would watch (Alt+Enter) change nothing, since
    /// swap chains are windowed only.
    pub(super) fn make_window_association(&self, hwnd: HWND, _flags: DXGI_MWA_FLAGS) -> Result<()> {
        self.window.set(hwnd.0 as isize);
        Ok(())
    }

    pub(super) fn get_window_association(&self) -> Result<HWND> {
        Ok(HWND(self.window.get() as *mut c_void))
    }

    pub(super) fn create_swap_chain(
        &self,
        device: Ref<IUnknown>,
        desc: *const DXGI_SWAP_CHAIN_DESC,
        out: OutRef<IDXGISwapChain>,
    ) -> HRESULT {
        let made = (|| {
            let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
            let desc1 = DXGI_SWAP_CHAIN_DESC1 {
                Width: d.BufferDesc.Width,
                Height: d.BufferDesc.Height,
                Format: d.BufferDesc.Format,
                Stereo: false.into(),
                SampleDesc: d.SampleDesc,
                BufferUsage: d.BufferUsage,
                BufferCount: d.BufferCount,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: d.SwapEffect,
                AlphaMode: DXGI_ALPHA_MODE_UNSPECIFIED,
                Flags: d.Flags,
            };
            self.make(device, d.OutputWindow, &desc1, d.Windowed.as_bool())?
                .cast::<IDXGISwapChain>()
        })();
        match made {
            Ok(chain) => out.write(Some(chain)).map_or_else(|e| e.code(), |_| S_OK),
            Err(e) => e.code(),
        }
    }

    pub(super) fn create_swap_chain_for_hwnd(
        &self,
        device: Ref<IUnknown>,
        hwnd: HWND,
        desc: *const DXGI_SWAP_CHAIN_DESC1,
        fullscreen: *const DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
        _restrict: Ref<IDXGIOutput>,
    ) -> Result<IDXGISwapChain1> {
        let d = unsafe { desc.as_ref() }.ok_or(E_INVALIDARG)?;
        let windowed = unsafe { fullscreen.as_ref() }.is_none_or(|f| f.Windowed.as_bool());
        self.make(device, hwnd, d, windowed)
    }

    fn make(
        &self,
        device: Ref<IUnknown>,
        hwnd: HWND,
        d: &DXGI_SWAP_CHAIN_DESC1,
        windowed: bool,
    ) -> Result<IDXGISwapChain1> {
        let device = our_device(device).map_err(|_| Error::from(E_INVALIDARG))?;
        if hwnd.is_invalid() {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
        if !windowed {
            return Err(missing(FULLSCREEN));
        }
        if d.SampleDesc.Count != 1 || d.Stereo.as_bool() {
            return Err(missing(MULTISAMPLED_OR_STEREO));
        }
        let model = Model::from_dxgi(d.SwapEffect.0 as u32).ok_or(E_INVALIDARG)?;
        let (mut width, mut height) = (d.Width, d.Height);
        if width == 0 || height == 0 {
            let (w, h) = client_size(hwnd);
            width = if width == 0 { w.max(1) } else { width };
            height = if height == 0 { h.max(1) } else { height };
        }
        let shader_input = d.BufferUsage.0 & DXGI_USAGE_SHADER_INPUT.0 != 0;
        let desc = SwapChainDesc {
            width,
            height,
            format: d.Format.0 as u32,
            buffers: d.BufferCount,
            model,
            usage: if shader_input {
                TextureUsage::SAMPLE
            } else {
                TextureUsage::empty()
            },
        };
        let ours = device.cast_object_ref::<Device>()?;
        let front: Shared = (*ours.front).clone();
        let window = add_window(&front, hwnd)?;
        let bind = if shader_input {
            D3D11_BIND_SHADER_RESOURCE.0 as u32
        } else {
            0
        };
        let waitable = d.Flags & DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32 != 0;
        // DXGI gives the blit models no waitable object.
        if waitable && !model.is_flip() {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
        let mut chain = front
            .borrow_mut()
            .create_swap_chain(window, &desc, bind, waitable)
            .map_err(chain_hresult)?;
        let latency_object = if waitable {
            chain
                .set_max_latency(WAITABLE_LATENCY)
                .map_err(chain_hresult)?;
            // SAFETY: a semaphore of the process's own, counting the frames the program may begin.
            let s = unsafe { CreateSemaphoreW(None, chain.max_latency() as i32, i32::MAX, None) }?;
            Some(LatencyObject(s))
        } else {
            None
        };
        let mut kept = *d;
        kept.Width = width;
        kept.Height = height;
        Ok(IDXGISwapChain4::from(SwapChain {
            private: Private::default(),
            front: Locked::new(front),
            chain: Locked::new(RefCell::new(Some(chain))),
            buffers: Locked::new(RefCell::new(Vec::new())),
            device,
            factory: self.me(),
            hwnd: hwnd.0 as isize,
            desc: Cell::new(kept),
            latency_object,
        })
        .into())
    }
}

#[implement(IDXGISwapChain4)]
pub struct SwapChain {
    pub(super) private: Private,
    front: Locked<Shared>,
    chain: Locked<RefCell<Option<Chain<Chosen>>>>,
    /// The back buffers handed out, each with a library reference, made at its first `GetBuffer`.
    buffers: Locked<RefCell<Vec<Option<Held<Texture2D_Impl>>>>>,
    device: ID3D11Device,
    factory: IDXGIFactory2,
    hwnd: isize,
    desc: Cell<DXGI_SWAP_CHAIN_DESC1>,
    /// The frame latency waitable object of a swap chain made with one.
    latency_object: Option<LatencyObject>,
}

/// A frame latency waitable object: a semaphore counting the frames the program may begin, made
/// with the swap chain's latency and released by each frame retired.
struct LatencyObject(HANDLE);

impl Drop for LatencyObject {
    fn drop(&mut self) {
        // SAFETY: the semaphore is ours; the program closes its own duplicates.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

impl Drop for SwapChain {
    fn drop(&mut self) {
        let _lock = lock();
        self.buffers.borrow_mut().clear();
        if let Some(chain) = self.chain.borrow_mut().take() {
            self.front.borrow_mut().destroy_chain(chain);
        }
    }
}

/// The HRESULT of a front error from a swap chain call.
fn chain_hresult(e: wcontainers_direct3d_11::Error) -> Error {
    match e {
        wcontainers_direct3d_11::Error::InvalidCall(_) => DXGI_ERROR_INVALID_CALL.into(),
        e => hresult(e),
    }
}

impl SwapChain_Impl {
    pub(super) fn get_parent(&self, riid: *const GUID, parent: *mut *mut c_void) -> Result<()> {
        unsafe { self.factory.query(riid, parent).ok() }
    }

    pub(super) fn get_device(&self, riid: *const GUID, out: *mut *mut c_void) -> Result<()> {
        unsafe { self.device.query(riid, out).ok() }
    }

    pub(super) fn present(&self, sync_interval: u32, flags: DXGI_PRESENT) -> HRESULT {
        if sync_interval > MAX_SYNC_INTERVAL {
            return DXGI_ERROR_INVALID_CALL;
        }
        // A window nobody can see takes no frames, as DXGI answers.
        let hwnd = HWND(self.hwnd as *mut c_void);
        // SAFETY: asks about the program's window.
        let hidden = unsafe { !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() };
        let test = flags.0 & DXGI_PRESENT_TEST.0 != 0;
        if hidden {
            // An occluded frame releases the waitable object as a shown one does.
            if let (Some(o), false) = (&self.latency_object, test) {
                // SAFETY: releases our semaphore by the frame.
                let _ = unsafe { ReleaseSemaphore(o.0, 1, None) };
            }
            return DXGI_STATUS_OCCLUDED;
        }
        if test {
            return S_OK;
        }
        let mut chain = self.chain.borrow_mut();
        let Some(chain) = chain.as_mut() else {
            return DXGI_ERROR_INVALID_CALL;
        };
        match self.front.borrow_mut().present(chain, sync_interval) {
            Ok(retired) => {
                if let (Some(o), 1..) = (&self.latency_object, retired) {
                    // SAFETY: releases our semaphore by the frames retired.
                    let _ = unsafe { ReleaseSemaphore(o.0, retired as i32, None) };
                }
                S_OK
            }
            Err(e) => chain_hresult(e).code(),
        }
    }

    pub(super) fn present1(
        &self,
        sync_interval: u32,
        flags: DXGI_PRESENT,
        _parameters: *const DXGI_PRESENT_PARAMETERS,
    ) -> HRESULT {
        self.present(sync_interval, flags)
    }

    pub(super) fn get_buffer(
        &self,
        index: u32,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        let chain = self.chain.borrow();
        let front = chain
            .as_ref()
            .and_then(|c| c.buffer(index))
            .ok_or(DXGI_ERROR_INVALID_CALL)?
            .clone();
        let mut buffers = self.buffers.borrow_mut();
        if buffers.len() <= index as usize {
            buffers.resize_with(index as usize + 1, || None);
        }
        let texture: ID3D11Texture2D = match &buffers[index as usize] {
            Some(held) => held.public(),
            None => {
                let t: ID3D11Texture2D = Texture2D::new(front, self.device.clone()).into();
                buffers[index as usize] = Some(Held::new(t.cast_object_ref::<Texture2D>()?));
                t
            }
        };
        unsafe { texture.query(riid, out).ok() }
    }

    pub(super) fn set_fullscreen_state(
        &self,
        fullscreen: BOOL,
        _target: Ref<IDXGIOutput>,
    ) -> Result<()> {
        if fullscreen.as_bool() {
            return Err(missing(FULLSCREEN));
        }
        Ok(())
    }

    pub(super) fn get_fullscreen_state(
        &self,
        fullscreen: *mut BOOL,
        target: OutRef<IDXGIOutput>,
    ) -> Result<()> {
        if let Some(f) = unsafe { fullscreen.as_mut() } {
            *f = false.into();
        }
        if !target.is_null() {
            target.write(None)?;
        }
        Ok(())
    }

    /// The back buffers are presented unrotated; asking for that changes nothing.
    pub(super) fn set_rotation(&self, rotation: DXGI_MODE_ROTATION) -> Result<()> {
        if rotation != DXGI_MODE_ROTATION_IDENTITY {
            return Err(missing(ROTATING));
        }
        Ok(())
    }

    pub(super) fn get_rotation(&self) -> Result<DXGI_MODE_ROTATION> {
        Ok(DXGI_MODE_ROTATION_IDENTITY)
    }

    pub(super) fn get_desc1(&self) -> Result<DXGI_SWAP_CHAIN_DESC1> {
        Ok(self.desc.get())
    }

    pub(super) fn get_desc(&self) -> Result<DXGI_SWAP_CHAIN_DESC> {
        let d = self.desc.get();
        Ok(DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: d.Width,
                Height: d.Height,
                RefreshRate: DXGI_RATIONAL::default(),
                Format: d.Format,
                ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
                Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
            },
            SampleDesc: d.SampleDesc,
            BufferUsage: d.BufferUsage,
            BufferCount: d.BufferCount,
            OutputWindow: HWND(self.hwnd as *mut c_void),
            Windowed: true.into(),
            SwapEffect: d.SwapEffect,
            Flags: d.Flags,
        })
    }

    pub(super) fn get_hwnd(&self) -> Result<HWND> {
        Ok(HWND(self.hwnd as *mut c_void))
    }

    pub(super) fn get_last_present_count(&self) -> Result<u32> {
        let chain = self.chain.borrow();
        Ok(chain.as_ref().map_or(0, |c| c.present_count() as u32))
    }

    /// Refused while the program holds a back buffer, as DXGI refuses it.
    pub(super) fn resize_buffers(
        &self,
        count: u32,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        flags: &DXGI_SWAP_CHAIN_FLAG,
    ) -> Result<()> {
        let mut buffers = self.buffers.borrow_mut();
        if buffers.iter().flatten().any(|b| public_count(&**b) > 0) {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
        buffers.clear();
        drop(buffers);
        let (mut w, mut h) = (width, height);
        if w == 0 || h == 0 {
            let (cw, ch) = client_size(HWND(self.hwnd as *mut c_void));
            w = if w == 0 { cw.max(1) } else { w };
            h = if h == 0 { ch.max(1) } else { h };
        }
        let mut chain = self.chain.borrow_mut();
        let chain = chain.as_mut().ok_or(DXGI_ERROR_INVALID_CALL)?;
        self.front
            .borrow_mut()
            .resize_buffers(chain, count, w, h, format.0 as u32)
            .map_err(chain_hresult)?;
        let mut d = self.desc.get();
        let now = chain.desc();
        d.Width = now.width;
        d.Height = now.height;
        d.Format = DXGI_FORMAT(now.format as i32);
        d.BufferCount = now.buffers;
        d.Flags = flags.0 as u32;
        self.desc.set(d);
        Ok(())
    }

    /// Taken only by swap chains made with a waitable object, as DXGI takes it.
    pub(super) fn set_maximum_frame_latency(&self, frames: u32) -> Result<()> {
        if self.desc.get().Flags & DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32 == 0
        {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
        let mut chain = self.chain.borrow_mut();
        let chain = chain.as_mut().ok_or(DXGI_ERROR_INVALID_CALL)?;
        let before = chain.max_latency();
        chain.set_max_latency(frames).map_err(chain_hresult)?;
        // A larger latency lets the program begin as many more frames at once; a smaller one is
        // kept by `Present`, which waits until fewer frames than it are unretired.
        if let (Some(o), Some(more @ 1..)) = (&self.latency_object, frames.checked_sub(before)) {
            // SAFETY: releases our semaphore by the frames the latency grew.
            let _ = unsafe { ReleaseSemaphore(o.0, more as i32, None) };
        }
        Ok(())
    }

    /// A handle of the swap chain's frame latency waitable object, which the program closes; a
    /// null handle for a swap chain made without one, as DXGI gives.
    pub(super) fn get_frame_latency_waitable_object(&self) -> HANDLE {
        let Some(o) = &self.latency_object else {
            return HANDLE::default();
        };
        let mut handle = HANDLE::default();
        // SAFETY: duplicates our semaphore's handle within the process for the program.
        let process = unsafe { GetCurrentProcess() };
        match unsafe {
            DuplicateHandle(
                process,
                o.0,
                process,
                &mut handle,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        } {
            Ok(()) => handle,
            Err(_) => HANDLE::default(),
        }
    }

    pub(super) fn get_maximum_frame_latency(&self) -> Result<u32> {
        if self.desc.get().Flags & DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32 == 0
        {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        }
        let chain = self.chain.borrow();
        Ok(chain.as_ref().map_or(1, |c| c.max_latency()))
    }

    pub(super) fn get_current_back_buffer_index(&self) -> u32 {
        let chain = self.chain.borrow();
        chain.as_ref().map_or(0, |c| c.current_index())
    }

    /// The present bit for the colour space the back buffers' format presents in (sRGB, or linear
    /// light from 16-bit float buffers), as on an SDR display.
    pub(super) fn check_color_space_support(&self, space: DXGI_COLOR_SPACE_TYPE) -> Result<u32> {
        let format = self.desc.get().Format.0 as u32;
        Ok(
            if wcontainers_directx_graphics_infrastructure::presents_in(format, space.0 as u32) {
                DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT.0 as u32
            } else {
                0
            },
        )
    }

    pub(super) fn set_color_space1(&self, space: DXGI_COLOR_SPACE_TYPE) -> Result<()> {
        if self.check_color_space_support(space)? == 0 {
            return Err(E_INVALIDARG.into());
        }
        Ok(())
    }

    /// `ResizeBuffers` with Direct3D 12's node masks and queues, which a Direct3D 11 device has none of.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn resize_buffers1(
        &self,
        count: u32,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        flags: &DXGI_SWAP_CHAIN_FLAG,
        masks: *const u32,
        queues: *const Option<IUnknown>,
    ) -> Result<()> {
        if !masks.is_null() || !queues.is_null() {
            return Err(E_INVALIDARG.into());
        }
        self.resize_buffers(count, width, height, format, flags)
    }

    /// Only the absence of HDR metadata is taken: the output is an SDR display.
    pub(super) fn set_hdr_meta_data(
        &self,
        kind: DXGI_HDR_METADATA_TYPE,
        _size: u32,
        _data: *const c_void,
    ) -> Result<()> {
        if kind != DXGI_HDR_METADATA_TYPE_NONE {
            return Err(missing(HDR_METADATA));
        }
        Ok(())
    }
}
