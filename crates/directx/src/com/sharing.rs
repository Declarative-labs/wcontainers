//! Resources shared between the process's devices by handle (`GetSharedHandle`,
//! `CreateSharedHandle`, `OpenSharedResource*`) and their keyed mutexes. Each device holds its own
//! copy of a shared resource (its back end shares no memory with another's); a copy written on one
//! device reaches the others through the host when that device flushes or releases the keyed mutex
//! (`SharedThroughHost`), the points Direct3D tells programs to synchronise at.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::{Rc, Weak};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use wcontainers_direct3d_11::WARNING_PREFIX;
use wcontainers_direct3d_11::desc::{self, BufferDesc, SubresourceData, TextureDesc};
use wcontainers_direct3d_11::object::{Buffer, Resource, Texture};
use wcontainers_gpu::SlowPath;
use windows::Win32::Foundation::{
    CloseHandle, CompareObjectHandles, DUPLICATE_SAME_ACCESS, DuplicateHandle, E_INVALIDARG,
    HANDLE, WAIT_ABANDONED, WAIT_TIMEOUT,
};
use windows::Win32::Graphics::Dxgi::DXGI_ERROR_INVALID_CALL;
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::System::Threading::{CreateEventW, GetCurrentProcess, INFINITE};
use windows::core::{Error, HRESULT, PCWSTR, Result};

use parking_lot::ReentrantMutexGuard;

use super::{Chosen, Shared, hresult, missing};

/// The bit DXGI's kernel-mode (KMT) handles carry, by which programs tell them from NT handles.
const KMT_HANDLE_MARK: usize = 0x4000_0000;

/// A multisampled texture opened on another device: no staging copy holds its samples.
const MULTISAMPLED_ACROSS_DEVICES: &str = "a multisampled texture shared between devices";
/// What a resource shared across devices of its kind is made from on another device.
#[derive(Clone, Copy)]
enum Made {
    Buffer(BufferDesc),
    Texture(TextureDesc),
}

/// One device's copy.
struct Copy {
    device: Weak<RefCell<super::Front>>,
    resource: WeakResource,
}

enum WeakResource {
    Buffer(Weak<Buffer<Chosen>>),
    Texture(Weak<Texture<Chosen>>),
}

impl Copy {
    fn live(&self) -> Option<(Shared, Resource<Chosen>)> {
        let resource = match &self.resource {
            WeakResource::Buffer(b) => Resource::Buffer(b.upgrade()?),
            WeakResource::Texture(t) => Resource::Texture(t.upgrade()?),
        };
        Some((self.device.upgrade()?, resource))
    }
}

/// A keyed mutex's state: the key it was last released with and the device holding it.
struct KeyedMutex {
    key: u64,
    holder: Option<Weak<RefCell<super::Front>>>,
    /// Its holder's device went while holding it: every acquire answers `WAIT_ABANDONED`.
    abandoned: bool,
}

/// A shared resource: its copies, the contents last copied between them and its handles.
pub struct Share {
    made: Made,
    /// The handle `GetSharedHandle` answers (none for a resource shared through NT handles).
    kmt: Option<HANDLE>,
    copies: RefCell<Vec<Copy>>,
    /// The contents last copied between devices, per subresource.
    host: RefCell<Option<Vec<Vec<u8>>>>,
    mutex: Option<RefCell<KeyedMutex>>,
    /// This library's own handle of each NT handle's object, and each name given.
    nt: RefCell<Vec<HANDLE>>,
    names: RefCell<Vec<Vec<u16>>>,
}

impl Drop for Share {
    fn drop(&mut self) {
        for h in self.nt.take() {
            let _ = unsafe { CloseHandle(h) };
        }
    }
}

/// The shares, touched only under the library's lock.
struct Shares {
    live: RefCell<Vec<Weak<Share>>>,
    next: Cell<usize>,
}

/// SAFETY: every access is made holding the library's lock (`shares` checks it).
unsafe impl Sync for Shares {}

static SHARES: Shares = Shares {
    live: RefCell::new(Vec::new()),
    next: Cell::new(1),
};

fn shares() -> &'static Shares {
    assert!(super::LOCK.is_owned_by_current_thread());
    &SHARES
}

/// Signalled at every keyed mutex release, counted so a waiter misses none.
static RELEASES: Mutex<u64> = Mutex::new(0);
static RELEASED: Condvar = Condvar::new();

fn weak(r: &Resource<Chosen>) -> WeakResource {
    match r {
        Resource::Buffer(b) => WeakResource::Buffer(Rc::downgrade(b)),
        Resource::Texture(t) => WeakResource::Texture(Rc::downgrade(t)),
    }
}

/// The share of a resource just made with misc flags `misc` on `device`, or none for one not
/// shared.
pub fn made(device: &Shared, resource: &Resource<Chosen>) -> Option<Rc<Share>> {
    let (misc, made) = match resource {
        Resource::Buffer(b) => (b.desc.misc_flags, Made::Buffer(b.desc)),
        Resource::Texture(t) => (t.desc.misc_flags, Made::Texture(t.desc)),
    };
    if misc & (desc::MISC_SHARED | desc::MISC_SHARED_KEYEDMUTEX) == 0 {
        return None;
    }
    let all = shares();
    let kmt = (misc & desc::MISC_SHARED_NTHANDLE == 0).then(|| {
        let n = all.next.get();
        all.next.set(n + 1);
        HANDLE((KMT_HANDLE_MARK | n) as *mut c_void)
    });
    let share = Rc::new(Share {
        made,
        kmt,
        copies: RefCell::new(vec![Copy {
            device: Rc::downgrade(device),
            resource: weak(resource),
        }]),
        host: RefCell::new(None),
        mutex: (misc & desc::MISC_SHARED_KEYEDMUTEX != 0).then(|| {
            RefCell::new(KeyedMutex {
                key: 0,
                holder: None,
                abandoned: false,
            })
        }),
        nt: RefCell::new(Vec::new()),
        names: RefCell::new(Vec::new()),
    });
    let mut live = all.live.borrow_mut();
    live.retain(|s| s.strong_count() > 0);
    live.push(Rc::downgrade(&share));
    Some(share)
}

fn find(mut found: impl FnMut(&Share) -> bool) -> Option<Rc<Share>> {
    let live = shares().live.borrow();
    live.iter().filter_map(Weak::upgrade).find(|s| found(s))
}

/// The share a KMT handle names.
pub fn by_handle(handle: HANDLE) -> Option<Rc<Share>> {
    find(|s| s.kmt == Some(handle))
}

/// The share whose NT handle's object `handle` is.
pub fn by_nt_handle(handle: HANDLE) -> Option<Rc<Share>> {
    find(|s| {
        s.nt.borrow()
            .iter()
            .any(|&h| unsafe { CompareObjectHandles(h, handle) }.as_bool())
    })
}

/// The share given name `name` by `CreateSharedHandle`.
pub fn by_name(name: &[u16]) -> Option<Rc<Share>> {
    find(|s| s.names.borrow().iter().any(|n| n == name))
}

impl Share {
    /// `IDXGIResource::GetSharedHandle`.
    pub fn kmt_handle(&self) -> Result<HANDLE> {
        self.kmt.ok_or_else(|| E_INVALIDARG.into())
    }

    /// `IDXGIResource1::CreateSharedHandle`: an NT handle of an object of this library's naming
    /// the share, under `name` if given.
    pub fn nt_handle(
        &self,
        attributes: *const SECURITY_ATTRIBUTES,
        name: PCWSTR,
    ) -> Result<HANDLE> {
        if self.kmt.is_some() {
            return Err(E_INVALIDARG.into());
        }
        let attributes = (!attributes.is_null()).then_some(attributes);
        let handle = unsafe { CreateEventW(attributes, true, false, name) }?;
        let mut own = HANDLE::default();
        let process = unsafe { GetCurrentProcess() };
        if let Err(e) = unsafe {
            DuplicateHandle(
                process,
                handle,
                process,
                &mut own,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        } {
            let _ = unsafe { CloseHandle(handle) };
            return Err(e);
        }
        self.nt.borrow_mut().push(own);
        if !name.is_null() {
            self.names
                .borrow_mut()
                .push(unsafe { name.as_wide() }.to_vec());
        }
        Ok(handle)
    }

    /// The share's resource on `device`: its own copy, made (and filled from the newest) at the
    /// first open there.
    pub fn on(&self, device: &Shared) -> Result<Resource<Chosen>> {
        let mine = Rc::downgrade(device);
        let found = self
            .copies
            .borrow()
            .iter()
            .filter(|c| c.device.ptr_eq(&mine))
            .find_map(Copy::live);
        if let Some((_, r)) = found {
            return Ok(r);
        }
        if let Made::Texture(d) = self.made
            && d.sample_count > 1
        {
            return Err(missing(MULTISAMPLED_ACROSS_DEVICES));
        }
        let made = {
            let mut f = device.borrow_mut();
            match &self.made {
                Made::Buffer(d) => f.create_buffer(d, None).map(Resource::Buffer),
                Made::Texture(d) => f.create_texture(d, &[]).map(Resource::Texture),
            }
            .map_err(hresult)?
        };
        let newest = self.copies.borrow().iter().find_map(Copy::live);
        self.copies.borrow_mut().push(Copy {
            device: mine,
            resource: weak(&made),
        });
        if let Some((from, source)) = newest {
            let contents = read(&from, &source).map_err(hresult)?;
            write(device, &made, &contents).map_err(hresult)?;
            *self.host.borrow_mut() = Some(contents);
        }
        Ok(made)
    }

    /// The copy on `device` was handed over (flushed or released): where other devices hold
    /// copies and its contents changed, they are copied into theirs.
    fn hand_over(&self, device: &Shared) -> Result<()> {
        let mine = Rc::downgrade(device);
        let copies: Vec<_> = self.copies.borrow().iter().filter_map(Copy::live).collect();
        let Some((_, source)) = copies.iter().find(|(d, _)| Rc::ptr_eq(d, device)) else {
            return Ok(());
        };
        if copies.iter().all(|(d, _)| Rc::downgrade(d).ptr_eq(&mine)) {
            return Ok(());
        }
        device
            .borrow_mut()
            .count_slow_path(SlowPath::SharedThroughHost);
        let contents = read(device, source).map_err(hresult)?;
        if self.host.borrow().as_ref() == Some(&contents) {
            return Ok(());
        }
        for (d, r) in copies.iter().filter(|(d, _)| !Rc::ptr_eq(d, device)) {
            write(d, r, &contents).map_err(hresult)?;
        }
        *self.host.borrow_mut() = Some(contents);
        Ok(())
    }

    /// Whether the resource has a keyed mutex.
    pub fn keyed(&self) -> bool {
        self.mutex.is_some()
    }

    /// One try of `AcquireSync` by `device`: none while it must wait.
    fn try_acquire(&self, device: &Shared, key: u64) -> Option<Result<()>> {
        let mut m = self.mutex.as_ref()?.borrow_mut();
        let mine = Rc::downgrade(device);
        if m.abandoned {
            return Some(Err(Error::from_hresult(HRESULT(WAIT_ABANDONED.0 as i32))));
        }
        match &m.holder {
            Some(h) if h.ptr_eq(&mine) => Some(Err(DXGI_ERROR_INVALID_CALL.into())),
            Some(h) if h.strong_count() == 0 => {
                m.abandoned = true;
                Some(Err(Error::from_hresult(HRESULT(WAIT_ABANDONED.0 as i32))))
            }
            Some(_) => None,
            None if m.key == key => {
                m.holder = Some(mine);
                Some(Ok(()))
            }
            None => None,
        }
    }

    /// `IDXGIKeyedMutex::AcquireSync` by `device`, waiting up to `milliseconds` with the library's
    /// lock, `held` once by the caller, let go.
    pub fn acquire(
        &self,
        held: &mut ReentrantMutexGuard<'static, ()>,
        device: &Shared,
        key: u64,
        milliseconds: u32,
    ) -> Result<()> {
        let deadline = (milliseconds != INFINITE)
            .then(|| Instant::now() + Duration::from_millis(milliseconds.into()));
        loop {
            let seen = *RELEASES.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(answer) = self.try_acquire(device, key) {
                return answer;
            }
            let left = deadline.map(|d| d.saturating_duration_since(Instant::now()));
            if left == Some(Duration::ZERO) {
                return Err(Error::from_hresult(HRESULT(WAIT_TIMEOUT.0 as i32)));
            }
            ReentrantMutexGuard::unlocked(held, || {
                let n = RELEASES.lock().unwrap_or_else(|e| e.into_inner());
                let wait = |n: &mut u64| *n == seen;
                match left {
                    Some(left) => drop(RELEASED.wait_timeout_while(n, left, wait)),
                    None => drop(RELEASED.wait_while(n, wait)),
                }
            });
        }
    }

    /// `IDXGIKeyedMutex::ReleaseSync` by `device`: what it wrote reaches the other devices first.
    pub fn release(&self, device: &Shared, key: u64) -> Result<()> {
        let Some(mutex) = &self.mutex else {
            return Err(DXGI_ERROR_INVALID_CALL.into());
        };
        {
            let m = mutex.borrow();
            let mine = Rc::downgrade(device);
            if m.abandoned || !m.holder.as_ref().is_some_and(|h| h.ptr_eq(&mine)) {
                return Err(DXGI_ERROR_INVALID_CALL.into());
            }
        }
        self.hand_over(device)?;
        {
            let mut m = mutex.borrow_mut();
            m.key = key;
            m.holder = None;
        }
        *RELEASES.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        RELEASED.notify_all();
        Ok(())
    }
}

/// `device` flushed: each share it holds a copy of is handed over.
pub fn flushed(device: &Shared) {
    let mine = Rc::downgrade(device);
    let held: Vec<Rc<Share>> = {
        let live = shares().live.borrow();
        live.iter()
            .filter_map(Weak::upgrade)
            .filter(|s| s.copies.borrow().iter().any(|c| c.device.ptr_eq(&mine)))
            .collect()
    };
    for s in held {
        if let Err(e) = s.hand_over(device) {
            eprintln!(
                "{WARNING_PREFIX}: a shared resource was not copied to its other devices: {e}"
            );
        }
    }
}

/// Each subresource's bytes, rows tightly packed, read back through a staging copy.
fn read(device: &Shared, r: &Resource<Chosen>) -> wcontainers_direct3d_11::Result<Vec<Vec<u8>>> {
    let mut f = device.borrow_mut();
    let mut out = Vec::new();
    match r {
        Resource::Buffer(b) => {
            let staging = Resource::Buffer(f.create_buffer(&staging_buffer(&b.desc), None)?);
            f.copy_resource(&staging, r)?;
            let mapped = f.map(&staging, 0, desc::map::READ, 0)?;
            let len = b.desc.byte_width as usize;
            out.push(mapped.data[..len].iter().map(Cell::get).collect());
            f.unmap(&staging, 0);
        }
        Resource::Texture(t) => {
            let d = TextureDesc {
                usage: desc::usage::STAGING,
                bind_flags: 0,
                cpu_access_flags: desc::cpu::READ,
                misc_flags: 0,
                ..t.desc
            };
            let staging = Resource::Texture(f.create_texture(&d, &[])?);
            f.copy_resource(&staging, r)?;
            for sub in 0..t.desc.mip_levels * t.desc.array_size {
                let (row, rows, slices) = layout(t, sub);
                let mapped = f.map(&staging, sub, desc::map::READ, 0)?;
                let mut bytes = Vec::with_capacity((row * rows * slices) as usize);
                for slice in 0..slices {
                    for y in 0..rows {
                        let at = slice * mapped.depth_pitch + y * mapped.row_pitch;
                        let from = &mapped.data[at as usize..][..row as usize];
                        bytes.extend(from.iter().map(Cell::get));
                    }
                }
                f.unmap(&staging, sub);
                out.push(bytes);
            }
        }
    }
    Ok(out)
}

/// Writes contents [`read`] gave into `r`, through a staging copy made with them.
fn write(
    device: &Shared,
    r: &Resource<Chosen>,
    contents: &[Vec<u8>],
) -> wcontainers_direct3d_11::Result<()> {
    let mut f = device.borrow_mut();
    let staging = match r {
        Resource::Buffer(b) => {
            Resource::Buffer(f.create_buffer(&staging_buffer(&b.desc), Some(&contents[0]))?)
        }
        Resource::Texture(t) => {
            let data: Vec<_> = contents
                .iter()
                .enumerate()
                .map(|(sub, bytes)| {
                    let (row, rows, _) = layout(t, sub as u32);
                    SubresourceData {
                        bytes,
                        row_pitch: row,
                        slice_pitch: row * rows,
                    }
                })
                .collect();
            let d = TextureDesc {
                usage: desc::usage::STAGING,
                bind_flags: 0,
                cpu_access_flags: desc::cpu::WRITE,
                misc_flags: 0,
                ..t.desc
            };
            Resource::Texture(f.create_texture(&d, &data)?)
        }
    };
    f.copy_resource(r, &staging)
}

fn staging_buffer(d: &BufferDesc) -> BufferDesc {
    BufferDesc {
        usage: desc::usage::STAGING,
        bind_flags: 0,
        cpu_access_flags: desc::cpu::READ | desc::cpu::WRITE,
        misc_flags: 0,
        structure_byte_stride: 0,
        ..*d
    }
}

/// Bytes a row, rows and slices of subresource `sub`.
fn layout(t: &Texture<Chosen>, sub: u32) -> (u32, u32, u32) {
    let mip = sub % t.desc.mip_levels;
    let d = &t.desc;
    let (w, h) = ((d.width >> mip).max(1), (d.height >> mip).max(1));
    let (row, rows) = wcontainers_direct3d_11::format::subresource_rows(d.format, t.format, w, h);
    (row, rows, (d.depth >> mip).max(1))
}
