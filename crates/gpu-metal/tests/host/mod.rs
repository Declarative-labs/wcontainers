//! The host of the shared suite on Metal: a device made through the direct implementation of the Metal trait.

use wcontainers_gpu_metal::MetalGpu;
use wcontainers_metal::StandaloneMetal;
use wcontainers_metal_direct::DirectMetal;

pub struct Native;

impl wcontainers_gpu_test::Host for Native {
    type Backend = MetalGpu<DirectMetal>;

    fn make() -> MetalGpu<DirectMetal> {
        let b = MetalGpu::new(DirectMetal::new(()));
        eprintln!("adapter: {}", wcontainers_gpu::Gpu::caps(&b).adapter.name);
        b
    }

    fn window(_b: &mut MetalGpu<DirectMetal>) -> u32 {
        panic!("this implementation makes no surfaces yet, so no window is opened");
    }

    fn shown(_b: &mut MetalGpu<DirectMetal>, _window: u32) -> Vec<u8> {
        unreachable!("no window was opened")
    }
}
