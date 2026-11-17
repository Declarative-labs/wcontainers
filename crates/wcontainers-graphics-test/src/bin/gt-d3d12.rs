//! Draws the triangle with Direct3D 12: `gt-d3d12 <hardware|warp> <out-dir>` writes
//! `d3d12-<driver>-triangle.png`.

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use wcontainers_graphics_test::{Driver, d3d12, save};
        let args: Vec<String> = std::env::args().skip(1).collect();
        let [driver, out] = args.as_slice() else {
            anyhow::bail!("usage: gt-d3d12 <hardware|warp> <out-dir>");
        };
        let driver = Driver::parse(driver)?;
        let picture = d3d12::render_triangle(driver)?;
        let path = std::path::Path::new(out).join(format!("d3d12-{}-triangle.png", driver.name()));
        save(&picture.image, &path)?;
        println!("triangle on {}: {}", picture.adapter, path.display());
        Ok(())
    }
    #[cfg(not(windows))]
    anyhow::bail!("Direct3D 12 runs on Windows only")
}
