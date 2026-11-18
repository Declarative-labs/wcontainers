//! Draws Direct3D 12 scenes: `gt-d3d12 <scene>... | all <hardware|warp> <out-dir> [--debug]` writes
//! each scene's outputs into the directory as `<scene>[-<part>].<png|exr|txt|caps>`, and
//! `<scene>[-<part>].absent` saying why for what the device reports it cannot do; `gt-d3d12 list`
//! prints the scenes' names. With --debug the debug layer checks every call and any error it
//! reports fails the scene.

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use anyhow::Context;
        use wcontainers_graphics_test::{Driver, d3d12};
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        if args == ["list"] {
            for (name, _) in d3d12::scenes::SCENES {
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
            d3d12::scenes::SCENES.iter().map(|(n, _)| *n).collect()
        } else {
            args.iter().map(String::as_str).collect()
        };
        // A device that could not be made still says where its libraries came from.
        let gpu = d3d12::create(driver, debug)
            .inspect_err(|_| println!("{}", wcontainers_graphics_test::library_paths()))?;
        println!("adapter {}", gpu.adapter);
        println!("{}", wcontainers_graphics_test::library_paths());
        for name in names {
            let scene = d3d12::scenes::find(name)
                .with_context(|| format!("no scene {name}; see gt-d3d12 list"))?;
            let outputs = scene(&gpu).with_context(|| format!("scene {name}"))?;
            gpu.check_debug().with_context(|| format!("scene {name}"))?;
            for output in outputs {
                println!("{}", output.save(name, &out)?);
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    anyhow::bail!("Direct3D 12 runs on Windows only")
}
