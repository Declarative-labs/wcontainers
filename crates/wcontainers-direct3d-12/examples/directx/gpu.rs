//! The one device thread: it owns the back end and the front's device, and runs what COM methods
//! send it, one at a time, while the sender waits for the answer. Between calls it sets the events
//! of fences whose values the GPU has reached, asking the back end without waiting.

use std::num::NonZeroIsize;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::mpsc::{RecvTimeoutError, Sender, sync_channel};
use std::time::Duration;

use wcontainers_direct3d_12::Device;
use wcontainers_gpu_webgpu::WebGpu;
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateEventW, INFINITE, SetEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    GWLP_HINSTANCE, GetWindowLongPtrW, MSG, MsgWaitForMultipleObjects, PM_NOREMOVE,
    PM_QS_SENDMESSAGE, PeekMessageW, QS_SENDMESSAGE,
};

pub type Front = Device<WebGpu>;
type Job = Box<dyn FnOnce(&mut State) + Send>;

/// How often the thread asks the back end about fences while events wait on them.
const FENCE_POLL: Duration = Duration::from_micros(250);

/// What the device thread owns.
pub struct State {
    pub front: Front,
    instance: wgpu::Instance,
    windows: u32,
}

impl State {
    /// Registers the window `hwnd` with the back end as a host window and gives its number.
    pub fn add_window(&mut self, hwnd: isize) -> Result<u32, String> {
        // SAFETY: reads a field of the program's window.
        let instance = unsafe { GetWindowLongPtrW(HWND(hwnd as *mut _), GWLP_HINSTANCE) };
        let mut handle =
            wgpu::rwh::Win32WindowHandle::new(NonZeroIsize::new(hwnd).ok_or("no window")?);
        // A window made without an instance takes the program's own, as Vulkan needs one.
        // SAFETY: asks for the program's module, which lives as long as the process.
        let program = unsafe { GetModuleHandleW(None) }.map_or(0, |m| m.0 as isize);
        handle.hinstance = NonZeroIsize::new(instance).or(NonZeroIsize::new(program));
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(
                wgpu::rwh::WindowsDisplayHandle::new(),
            )),
            raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(handle),
        };
        // SAFETY: the program keeps its window while a swap chain uses it, as DXGI requires.
        let surface =
            unsafe { self.instance.create_surface_unsafe(target) }.map_err(|e| e.to_string())?;
        self.windows += 1;
        self.front.backend().add_window(self.windows, surface);
        Ok(self.windows)
    }
}

static THREAD: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();

const THREAD_NAME: &str = "wcontainers-gpu";

/// Whether the caller is the device thread itself (the GPU driver under the back end).
pub fn on_device_thread() -> bool {
    std::thread::current().name() == Some(THREAD_NAME)
}

/// The back end on wgpu's Vulkan path: in a process whose `d3d12.dll` is this library, wgpu's
/// Direct3D 12 path would load this library as the system's.
fn start() -> Mutex<Sender<Job>> {
    let (tx, rx) = std::sync::mpsc::channel::<Job>();
    std::thread::Builder::new()
        .name(THREAD_NAME.into())
        .spawn(move || {
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            desc.backends = wgpu::Backends::VULKAN;
            let instance = wgpu::Instance::new(desc);
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    ..Default::default()
                }))
                .expect("wcontainers: no Vulkan adapter for the WebGPU back end");
            let (device, queue) =
                pollster::block_on(adapter.request_device(&WebGpu::device_descriptor(&adapter)))
                    .expect("wcontainers: the WebGPU device");
            let mut state = State {
                front: Device::new(WebGpu::new(&adapter, device, queue)),
                instance,
                windows: 0,
            };
            loop {
                let job = if state.front.has_waiters() {
                    match rx.recv_timeout(FENCE_POLL) {
                        Ok(job) => Some(job),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match rx.recv() {
                        Ok(job) => Some(job),
                        Err(_) => break,
                    }
                };
                if let Some(job) = job {
                    job(&mut state);
                }
                for event in state.front.reached_events() {
                    // SAFETY: the event the program gave `SetEventOnCompletion`.
                    let _ = unsafe { SetEvent(HANDLE(event as usize as *mut _)) };
                }
                for w in state.front.take_warnings() {
                    eprintln!("wcontainers: warning: {w}");
                }
            }
        })
        .expect("wcontainers: the device thread");
    Mutex::new(tx)
}

fn send(job: Job) {
    THREAD
        .get_or_init(start)
        .lock()
        .unwrap()
        .send(job)
        .expect("wcontainers: the device thread ended");
}

/// Runs `f` on the device thread with everything it owns and returns its answer.
pub fn call_state<R: Send + 'static>(f: impl FnOnce(&mut State) -> R + Send + 'static) -> R {
    let (tx, rx) = sync_channel(1);
    send(Box::new(move |s| {
        let _ = tx.send(f(s));
    }));
    rx.recv().expect("wcontainers: the device thread ended")
}

thread_local! {
    /// Set by the device thread when a call of [`call_windowed`] is answered.
    static ANSWERED: usize = unsafe { CreateEventW(None, false, false, windows::core::PCWSTR::null()) }
        .expect("wcontainers: an event")
        .0 as usize;
}

/// Runs `f` on the device thread, answering in the meantime what other threads send to this
/// thread's windows: for calls on a swap chain, during which the driver may ask the window's thread
/// (the program's, which is waiting here) before it can go on.
pub fn call_windowed<R: Send + 'static>(f: impl FnOnce(&mut State) -> R + Send + 'static) -> R {
    let event = ANSWERED.with(|e| *e);
    let (tx, rx) = sync_channel(1);
    send(Box::new(move |s| {
        let _ = tx.send(f(s));
        // SAFETY: the calling thread's event, which lives as long as that thread.
        let _ = unsafe { SetEvent(HANDLE(event as *mut _)) };
    }));
    loop {
        if let Ok(answer) = rx.try_recv() {
            return answer;
        }
        let handles = [HANDLE(event as *mut _)];
        let mut msg = MSG::default();
        // SAFETY: waits on this thread's own event, then lets Windows deliver sent messages.
        unsafe {
            MsgWaitForMultipleObjects(Some(&handles), false, INFINITE, QS_SENDMESSAGE);
            let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE | PM_QS_SENDMESSAGE);
        }
    }
}

/// Runs `f` on the device thread with the front's device and returns its answer.
pub fn call<R: Send + 'static>(f: impl FnOnce(&mut Front) -> R + Send + 'static) -> R {
    call_state(move |s| f(&mut s.front))
}
