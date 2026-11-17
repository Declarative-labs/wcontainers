//! Draws Direct3D 11 scenes: `gt-d3d11 <scene>... | all <hardware|warp> <out-dir> [--debug]` writes
//! each scene's outputs into the directory as `<scene>[-<part>].<png|exr|txt>`; `gt-d3d11 list`
//! prints the scenes' names. With --debug the debug layer checks every call and any error it
//! reports fails the scene.

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use anyhow::Context;
        use wcontainers_graphics_test::{Driver, d3d11};
        use windows::Win32::Graphics::Direct3D::{D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1};
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        if args == ["list"] {
            for (name, _) in d3d11::scenes::SCENES {
                println!("{name}");
            }
            return Ok(());
        }
        let debug = args.iter().any(|a| a == "--debug");
        args.retain(|a| a != "--debug");
        let usage = "usage: gt-d3d11 <scene>... | all <hardware|warp> <out-dir> [--debug], or gt-d3d11 list";
        anyhow::ensure!(args.len() >= 3, "{usage}");
        let out = std::path::PathBuf::from(args.pop().unwrap());
        let driver = Driver::parse(&args.pop().unwrap())?;
        let names: Vec<&str> = if args == ["all"] {
            d3d11::scenes::SCENES.iter().map(|(n, _)| *n).collect()
        } else {
            args.iter().map(String::as_str).collect()
        };
        let gpu = d3d11::create(
            driver,
            debug,
            &[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0],
        )?;
        println!("adapter {}", gpu.adapter()?);
        println!("{}", wcontainers_graphics_test::library_paths());
        for name in names {
            let scene = d3d11::scenes::find(name)
                .with_context(|| format!("no scene {name}; see gt-d3d11 list"))?;
            unsafe { gpu.context.ClearState() };
            let outputs = scene(&gpu).with_context(|| format!("scene {name}"))?;
            gpu.check_debug().with_context(|| format!("scene {name}"))?;
            for output in outputs {
                println!("{}", output.save(name, &out)?);
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    anyhow::bail!("Direct3D 11 runs on Windows only")
}
