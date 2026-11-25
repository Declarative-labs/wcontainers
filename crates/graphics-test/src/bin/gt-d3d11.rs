//! Draws Direct3D 11 scenes: `gt-d3d11 <scene>... | all <hardware|warp> <out-dir> [--debug]
//! [--level <level>]` writes each scene's outputs into the directory as
//! `<scene>[-<part>].<png|exr|txt|caps>`, and `<scene>[-<part>].absent` saying why for what the
//! device reports it cannot do; `gt-d3d11 list` prints the scenes' names. The device is asked for
//! every feature level from 11_1 (or `--level`) down to 9_1. With --debug the debug layer checks
//! every call and any error it reports fails the scene.

fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use anyhow::Context;
        use wcontainers_graphics_test::output::Output;
        use wcontainers_graphics_test::{Driver, d3d11};
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        if args == ["list"] {
            for (name, ..) in d3d11::scenes::SCENES {
                println!("{name}");
            }
            return Ok(());
        }
        let debug = args.iter().any(|a| a == "--debug");
        args.retain(|a| a != "--debug");
        let usage = "usage: gt-d3d11 <scene>... | all <hardware|warp> <out-dir> [--debug] [--level <level>], or gt-d3d11 list";
        let mut highest = d3d11::LEVELS.len();
        if let Some(at) = args.iter().position(|a| a == "--level") {
            let level = d3d11::level_named(args.get(at + 1).context(usage)?)?;
            highest = d3d11::LEVELS.iter().position(|(_, l)| *l == level).unwrap() + 1;
            args.drain(at..at + 2);
        }
        let levels: Vec<_> = d3d11::LEVELS[..highest]
            .iter()
            .rev()
            .map(|(_, l)| *l)
            .collect();
        anyhow::ensure!(args.len() >= 3, "{usage}");
        let out = std::path::PathBuf::from(args.pop().unwrap());
        let driver = Driver::parse(&args.pop().unwrap())?;
        let names: Vec<&str> = if args == ["all"] {
            d3d11::scenes::SCENES.iter().map(|(n, ..)| *n).collect()
        } else {
            args.iter().map(String::as_str).collect()
        };
        let gpu = d3d11::create(driver, debug, &levels)
            // A device that could not be made still says where its libraries came from.
            .inspect_err(|_| println!("{}", wcontainers_graphics_test::library_paths()))?;
        println!(
            "adapter {}, feature level {}",
            gpu.adapter()?,
            d3d11::level_name(gpu.level)
        );
        println!("{}", wcontainers_graphics_test::library_paths());
        for name in names {
            let (level, scene) = d3d11::scenes::find(name)
                .with_context(|| format!("no scene {name}; see gt-d3d11 list"))?;
            let outputs = if let Some(why) = gpu.lacks_level(level) {
                vec![Output::absent("", why)]
            } else {
                unsafe { gpu.context.ClearState() };
                // A failing call's reason, from the debug layer when it is on.
                scene(&gpu)
                    .map_err(|e| match gpu.check_debug() {
                        Err(debug) => e.context(format!("{debug:#}")),
                        Ok(()) => e,
                    })
                    .with_context(|| format!("scene {name}"))?
            };
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
