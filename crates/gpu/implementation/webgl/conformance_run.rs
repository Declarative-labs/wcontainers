#![cfg(windows)]
wcontainers_gpu_conformance::tests!(<wcontainers_gpu_webgl::Gl as wcontainers_gpu::StandaloneGpu>::new(wcontainers_gpu_webgl::GlConfig { angle: std::path::PathBuf::from(r"C:\wc\tools\angle-mozangle-0.7.1"), off: Vec::new() }).expect("ANGLE's OpenGL ES 3.0 context"); StandaloneGpu: yes);
