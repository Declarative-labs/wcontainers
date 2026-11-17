//! Draws Direct3D 12 scenes: `gt-d3d12 <scene>... | all <hardware|warp> <out-dir> [--debug]` writes
//! each scene's outputs into the directory as `<scene>[-<part>].<png|exr|txt>`; `gt-d3d12 list`
//! prints the scenes' names. With --debug the debug layer checks every call and any error it
//! reports fails the scene.

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use wcontainers_graphics_test::output::Output;
        use wcontainers_graphics_test::{Driver, d3d12};
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        if args == ["list"] {
            for name in d3d12::SCENES {
                println!("{name}");
            }
            return Ok(());
        }
        let debug = args.iter().any(|a| a == "--debug");
        args.retain(|a| a != "--debug");
        let usage = "usage: gt-d3d12 <scene>... | all <hardware|warp> <out-dir> [--debug], or gt-d3d12 list";
        anyhow::ensure!(args.len() >= 3, "{usage}");
        let out = std::path::PathBuf::from(args.pop().unwrap());
        let driver = Driver::parse(&args.pop().unwrap())?;
        let names: Vec<&str> = if args == ["all"] {
            d3d12::SCENES.to_vec()
        } else {
            args.iter().map(String::as_str).collect()
        };
        for name in names {
            anyhow::ensure!(name == "triangle", "no scene {name}; see gt-d3d12 list");
            let picture = d3d12::render_triangle(driver, debug)?;
            println!("adapter {}", picture.adapter);
            println!("{}", Output::picture("", picture.image).save(name, &out)?);
        }
        println!(
            "{}",
            wcontainers_graphics_test::library_paths(&["d3d12.dll", "dxgi.dll"])
        );
        Ok(())
    }
    #[cfg(not(windows))]
    anyhow::bail!("Direct3D 12 runs on Windows only")
}
