//! The one device thread: it owns the back end and the front's device, and runs what COM methods
//! send it, one at a time, while the sender waits for the answer.

use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, sync_channel};

use wcontainers_direct3d_12::Device;
use wcontainers_gpu_webgpu::WebGpu;

pub type Front = Device<WebGpu>;
type Job = Box<dyn FnOnce(&mut Front) + Send>;

static THREAD: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();

/// The back end on wgpu's Vulkan path: in a process whose `d3d12.dll` is this library, wgpu's
/// Direct3D 12 path would load this library as the system's.
fn start() -> Mutex<Sender<Job>> {
    let (tx, rx) = std::sync::mpsc::channel::<Job>();
    std::thread::Builder::new()
        .name("wcontainers-gpu".into())
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
            let mut front = Device::new(WebGpu::new(&adapter, device, queue));
            for job in rx {
                job(&mut front);
                for w in front.take_warnings() {
                    eprintln!("wcontainers: warning: {w}");
                }
            }
        })
        .expect("wcontainers: the device thread");
    Mutex::new(tx)
}

/// Runs `f` on the device thread and returns its answer.
pub fn call<R: Send + 'static>(f: impl FnOnce(&mut Front) -> R + Send + 'static) -> R {
    let (tx, rx) = sync_channel(1);
    let job: Job = Box::new(move |d| {
        let _ = tx.send(f(d));
    });
    THREAD
        .get_or_init(start)
        .lock()
        .unwrap()
        .send(job)
        .expect("wcontainers: the device thread ended");
    rx.recv().expect("wcontainers: the device thread ended")
}
