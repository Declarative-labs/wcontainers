//! Draws scenes with Direct3D 11: `gt-d3d11 <scene|all> <hardware|warp> <out-dir>` writes
//! `d3d11-<driver>-<scene>.png` for each scene.

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use wcontainers_graphics_test::{Driver, d3d11, save, scene::Scene};
        let args: Vec<String> = std::env::args().skip(1).collect();
        let [scenes, driver, out] = args.as_slice() else {
            anyhow::bail!("usage: gt-d3d11 <scene|all> <hardware|warp> <out-dir>");
        };
        let driver = Driver::parse(driver)?;
        for scene in Scene::parse_list(scenes)? {
            let picture = d3d11::render(scene, driver)?;
            let path = std::path::Path::new(out).join(format!(
                "d3d11-{}-{}.png",
                driver.name(),
                scene.name()
            ));
            save(&picture.image, &path)?;
            println!(
                "{} on {}: {}",
                scene.name(),
                picture.adapter,
                path.display()
            );
        }
        Ok(())
    }
    #[cfg(not(windows))]
    anyhow::bail!("Direct3D 11 runs on Windows only")
}
