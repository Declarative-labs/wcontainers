//! Draws the triangle natively through wgpu: `gt-wgpu <backend> <out-dir>`, the backend being one of
//! wgpu's names (dx12, vulkan, metal, gl), writes `wgpu-<backend>-triangle.png`.

use wcontainers_graphics_test::{save, webgpu};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [backend, out] = args.as_slice() else {
        anyhow::bail!("usage: gt-wgpu <dx12|vulkan|metal|gl> <out-dir>");
    };
    let backends = wgpu::Backends::from_comma_list(backend);
    anyhow::ensure!(!backends.is_empty(), "unknown backend {backend}");
    let picture = webgpu::render_triangle(backends)?;
    let path = std::path::Path::new(out).join(format!("wgpu-{backend}-triangle.png"));
    save(&picture.image, &path)?;
    println!("triangle on {}: {}", picture.adapter, path.display());
    Ok(())
}
