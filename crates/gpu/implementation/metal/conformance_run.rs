#![cfg(target_os = "macos")]

wcontainers_gpu_conformance::tests!(<wcontainers_gpu_metal::MetalGpu<wcontainers_metal_direct::DirectMetal> as wcontainers_gpu::StandaloneGpu>::new(wcontainers_gpu_metal::MetalGpuConfig { metal: <wcontainers_metal_direct::DirectMetal as wcontainers_metal::StandaloneMetal>::new(()) }).expect("Metal opens"); StandaloneGpu: yes);
