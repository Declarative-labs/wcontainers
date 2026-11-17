//! Draws the graphics rig's first three Direct3D 11 scenes through the WebGL 2 back end and checks them
//! against the stored real Direct3D pictures, with each picture's recorded tolerance.
//!
//!   webgl-scenes angle <angle-dir> <out-dir> [--off <extension>]...
//!       draws on ANGLE's OpenGL ES 3.0 from <angle-dir> (Chrome's own libEGL.dll and libGLESv2.dll),
//!       writes the pictures and the capability list to <out-dir>, and checks them.
//!   webgl-scenes check <dir>
//!       checks the pictures browser/run.mjs wrote from Chrome's WebGL 2 (<scene>[-<change>].rgba).
//!
//! Besides the scenes, the quads scene is drawn with a changed blend factor and a changed depth function,
//! each of which must fail the comparison. Exits 1 if a scene fails or a changed one passes.

#[path = "common/scenes.rs"]
mod scenes;

use anyhow::{Context, Result, bail};
use scenes::{Change, Scene, scene};
use std::path::{Path, PathBuf};
use wcontainers_graphics_test::{check, compare, load, picture_from_rows, save};

fn references() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../wcontainers-graphics-test/references/d3d11")
}

fn file(scene: Scene, change: Change) -> String {
    match change {
        Change::None => scene.name().to_owned(),
        c => format!("{}-{}", scene.name(), c.name()),
    }
}

/// Every picture the run draws: the three scenes, then the changed quads.
fn pictures() -> Vec<(Scene, Change)> {
    let mut all: Vec<_> = Scene::ALL.iter().map(|s| (*s, Change::None)).collect();
    all.extend(Change::ALTERED.iter().map(|c| (Scene::Quads, *c)));
    all
}

/// Compares one drawn picture with its scene's reference; returns whether the verdict is the one wanted.
fn verdict(scene: Scene, change: Change, texels: Vec<u8>, out: &Path) -> Result<bool> {
    let (w, h) = (scene::WIDTH, scene::HEIGHT);
    let ours = picture_from_rows(w, h, &texels, w as usize * 4).into();
    let name = file(scene, change);
    save(&ours, &out.join(format!("{name}.png")))?;
    let reference = format!("{}.png", scene.name());
    let allowance = check::read_tolerances(&references().join(check::TOLERANCES))?[&reference];
    let theirs = load(&references().join(&reference))?;
    let d = compare::compare(&theirs, &ours, allowance.tolerance)?;
    let exact = compare::compare(&theirs, &ours, 0.0)?;
    let pass = d.differing <= allowance.pixels;
    let wanted = pass == (change == Change::None);
    println!(
        "{} {name}: {} of {} pixels beyond {} (allowed {}); exactly equal but {} pixels, largest difference {}{}",
        if wanted { "ok  " } else { "FAIL" },
        d.differing,
        d.pixels,
        allowance.tolerance,
        allowance.pixels,
        exact.differing,
        exact.largest,
        if change == Change::None {
            ""
        } else {
            " (a changed scene, which must not pass)"
        },
    );
    Ok(wanted)
}

fn angle(dir: &Path, out: &Path, off: &[&str]) -> Result<bool> {
    use wcontainers_gpu::Backend;
    let mut gl = wcontainers_gpu_webgl::WebGl::on_angle(dir, off).map_err(anyhow::Error::msg)?;
    std::fs::create_dir_all(out)?;
    println!("renderer: {}", gl.renderer());
    let used: Vec<_> = gl.extensions().collect();
    let report = format!(
        "renderer: {}\nextensions used: {}\nswitched off: {}\ncontext extensions: {}\n{}",
        gl.renderer(),
        used.join(" "),
        off.join(" "),
        gl.context_extensions().join(" "),
        scenes::caps_report(gl.caps()),
    );
    std::fs::write(out.join("caps-angle.txt"), report)?;
    let mut all = true;
    for (scene, change) in pictures() {
        let texels = scenes::draw(&mut gl, scene, change)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("{} (GL log: {})", file(scene, change), gl.last_log()))?;
        all &= verdict(scene, change, texels, out)?;
    }
    Ok(all)
}

fn check_browser(dir: &Path) -> Result<bool> {
    let mut all = true;
    for (scene, change) in pictures() {
        let raw = dir.join(format!("{}.rgba", file(scene, change)));
        let texels = std::fs::read(&raw).with_context(|| format!("reading {}", raw.display()))?;
        all &= verdict(scene, change, texels, dir)?;
    }
    Ok(all)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let all = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["angle", dir, out, ref rest @ ..] => {
            let mut off = Vec::new();
            let mut rest = rest.iter();
            while let Some(flag) = rest.next() {
                match (*flag, rest.next()) {
                    ("--off", Some(name)) => off.push(*name),
                    _ => bail!("unknown argument {flag}"),
                }
            }
            angle(Path::new(dir), Path::new(out), &off)?
        }
        ["check", dir] => check_browser(Path::new(dir))?,
        _ => bail!(
            "usage: webgl-scenes angle <angle-dir> <out-dir> [--off <extension>]... | check <dir>"
        ),
    };
    println!("{}", if all { "all as expected" } else { "FAILED" });
    std::process::exit(if all { 0 } else { 1 });
}
