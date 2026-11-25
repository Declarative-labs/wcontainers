//! Draws the graphics rig's first three Direct3D 11 scenes, and its pictures of polygon filling, depth
//! bias and depth clipping, through the WebGL 2 back end and checks them against the stored real
//! Direct3D pictures, with each picture's recorded tolerance.
//!
//!   webgl-scenes angle <angle-dir> <out-dir> [--off <extensions>]...
//!       draws on ANGLE's OpenGL ES 3.0 from <angle-dir> (the libEGL.dll and libGLESv2.dll that
//!       angle\build.cmd builds from mozangle), writes the pictures and the capability list to
//!       <out-dir>, and checks them.
//!   webgl-scenes check <dir>
//!       checks the pictures browser/run.mjs wrote from Chrome's WebGL 2 (<scene>[-<change>].rgba).
//!
//! Besides the scenes, the quads scene is drawn with a changed blend factor and a changed depth function,
//! each of which must fail the comparison, and the scenes are drawn by other routes to the same picture
//! or to one made from it (`Change`, `Expected`), which must pass. Exits 1 on any other verdict.

#[path = "common/rig.rs"]
mod rig;
#[path = "common/scenes.rs"]
mod scenes;

use anyhow::{Context, Result, bail};
use scenes::{Change, Expected, Scene, scene};
use std::path::{Path, PathBuf};
use wcontainers_graphics_test::{check, compare, load, picture_from_rows, save};

fn references() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../graphics-test/references/d3d11")
}

fn file(scene: Scene, change: Change) -> String {
    match change {
        Change::None => scene.name().to_owned(),
        c => format!("{}-{}", scene.name(), c.name()),
    }
}

/// Compares one drawn picture with what it must equal (`Change::expected`), given the pictures drawn
/// before it in this run; returns whether the verdict is the one wanted.
fn verdict(
    scene: Scene,
    change: Change,
    texels: &[u8],
    drawn: &[(Scene, Change, Vec<u8>)],
    out: &Path,
) -> Result<bool> {
    let (w, h) = (scene::WIDTH, scene::HEIGHT);
    let picture = |texels: &[u8]| picture_from_rows(w, h, texels, w as usize * 4).into();
    let ours = picture(texels);
    let name = file(scene, change);
    save(&ours, &out.join(format!("{name}.png")))?;
    let reference = format!("{}.png", scene.name());
    let Some(mut allowance) = check::read_tolerances(&references().join(check::TOLERANCES))?
        .remove(&reference)
        .flatten()
    else {
        bail!("{reference} has no allowance: missing, or unrepeatable on real Direct3D");
    };
    let mut theirs = load(&references().join(&reference))?;
    let red = [255, 0, 0, 255];
    let empty = |texels: &[u8]| {
        let px = texels.as_chunks::<4>().0;
        px.iter().all(|p| *p == px[0])
    };
    match change.expected() {
        Expected::Base => {
            let wanted = !empty(texels);
            println!(
                "{} {name}: {}",
                if wanted { "ok  " } else { "FAIL" },
                if wanted {
                    "drawn, for the picture compared with it"
                } else {
                    "nothing drawn"
                }
            );
            return Ok(wanted);
        }
        Expected::Reference => {}
        Expected::CoveredRed => {
            let mut e = theirs.to_rgba8();
            let clear = e.get_pixel(0, 0).0;
            for px in e.as_chunks_mut::<4>().0 {
                if *px != clear {
                    *px = red;
                }
            }
            theirs = e.into();
        }
        Expected::RectRed => {
            let mut e = theirs.to_rgba8();
            let r = scenes::CLEARED;
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    e.get_pixel_mut(x, y).0 = red;
                }
            }
            theirs = e.into();
        }
        Expected::Same(other) => {
            let texels = drawn
                .iter()
                .find(|(s, c, _)| (*s, *c) == (scene, other))
                .map(|(_, _, t)| t)
                .with_context(|| format!("{name} is compared with a picture not drawn first"))?;
            anyhow::ensure!(!empty(texels), "{name} is compared with an empty picture");
            theirs = picture(texels);
            allowance.tolerance = 0.0;
            allowance.pixels = 0;
        }
    }
    let d = compare::compare(&theirs, &ours, allowance.tolerance)?;
    let exact = compare::compare(&theirs, &ours, 0.0)?;
    let pass = d.differing <= allowance.pixels;
    let wanted = pass != change.altered();
    println!(
        "{} {name}: {} of {} pixels beyond {} (allowed {}); exactly equal but {} pixels, largest difference {}{}",
        if wanted { "ok  " } else { "FAIL" },
        d.differing,
        d.pixels,
        allowance.tolerance,
        allowance.pixels,
        exact.differing,
        exact.largest,
        if change.altered() {
            " (a changed scene, which must not pass)"
        } else {
            ""
        },
    );
    Ok(wanted)
}

/// Compares one of the rig's pictures (None where the back end lacks its feature) with the stored
/// Direct3D one, within its recorded allowance; returns whether it passes.
fn rig_verdict(output: &rig::Output, bytes: Option<&[u8]>, out: &Path) -> Result<bool> {
    let name = output.name;
    let Some(bytes) = bytes.filter(|b| !b.is_empty()) else {
        println!("skip {name}: absent {:?}", output.needs);
        return Ok(true);
    };
    let (w, h) = (output.width, output.height);
    let reference = format!("{name}.{}", if output.depth { "exr" } else { "png" });
    let Some(allowance) = check::read_tolerances(&references().join(check::TOLERANCES))?
        .remove(&reference)
        .flatten()
    else {
        bail!("{reference} has no allowance");
    };
    let theirs = load(&references().join(&reference))?;
    let (beyond, exact, largest) = if output.depth {
        let theirs = theirs.to_rgba32f().into_raw();
        let ours: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        anyhow::ensure!(ours.len() == theirs.len(), "{name}: the sizes differ");
        let texel = |d: f64| {
            let mut n = 0;
            for (a, b) in ours.chunks(4).zip(theirs.chunks(4)) {
                n += u64::from(a.iter().zip(b).any(|(a, b)| f64::from((a - b).abs()) > d));
            }
            n
        };
        // The first texels beyond the allowance, for the log.
        let w = w as usize;
        for (i, (a, b)) in ours
            .chunks(4)
            .zip(theirs.chunks(4))
            .enumerate()
            .filter(|(_, (a, b))| {
                a.iter()
                    .zip(*b)
                    .any(|(a, b)| f64::from((a - b).abs()) > allowance.tolerance)
            })
            .take(3)
        {
            println!(
                "     {name} ({}, {}): ours {a:?}, Direct3D {b:?}",
                i % w,
                i / w
            );
        }
        let largest = ours
            .iter()
            .zip(&theirs)
            .map(|(a, b)| f64::from((a - b).abs()))
            .fold(0.0, f64::max);
        (texel(allowance.tolerance), texel(0.0), largest)
    } else {
        let ours = picture_from_rows(w, h, bytes, w as usize * 4).into();
        save(&ours, &out.join(format!("{name}.png")))?;
        let d = compare::compare(&theirs, &ours, allowance.tolerance)?;
        let e = compare::compare(&theirs, &ours, 0.0)?;
        (d.differing, e.differing, e.largest)
    };
    let pass = beyond <= allowance.pixels;
    println!(
        "{} {name}: {beyond} of {} pixels beyond {} (allowed {}); exactly equal but {exact} pixels, largest difference {largest}",
        if pass { "ok  " } else { "FAIL" },
        w * h,
        allowance.tolerance,
        allowance.pixels,
    );
    Ok(pass)
}

fn angle(dir: &Path, out: &Path, off: &[&str]) -> Result<bool> {
    use wcontainers_gpu::Gpu;
    let mut gl = wcontainers_gpu_webgl::Gl::on_angle(dir, off).map_err(anyhow::Error::msg)?;
    std::fs::create_dir_all(out)?;
    println!("renderer: {}", gl.renderer());
    let used: Vec<_> = gl.extensions().collect();
    let report = format!(
        "renderer: {}\nextensions used: {}\nswitched off: {}\ncontext extensions: {}\nexact float blits: {}\n{}",
        gl.renderer(),
        used.join(" "),
        off.join(" "),
        gl.context_extensions().join(" "),
        gl.exact_float_blits(),
        scenes::caps_report(gl.caps()),
    );
    std::fs::write(out.join("caps-angle.txt"), report)?;
    let mut all = true;
    let mut drawn = Vec::new();
    for (scene, change) in Change::PICTURES {
        let texels = scenes::draw(&mut gl, scene, change)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("{} (GL log: {})", file(scene, change), gl.last_log()))?;
        all &= verdict(scene, change, &texels, &drawn, out)?;
        drawn.push((scene, change, texels));
    }
    for o in &rig::OUTPUTS {
        let bytes = rig::draw(&mut gl, o)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("{} (GL log: {})", o.name, gl.last_log()))?;
        all &= rig_verdict(o, bytes.as_deref(), out)?;
    }
    Ok(all)
}

fn check_browser(dir: &Path) -> Result<bool> {
    let mut all = true;
    let mut drawn = Vec::new();
    for (scene, change) in Change::PICTURES {
        let raw = dir.join(format!("{}.rgba", file(scene, change)));
        let texels = std::fs::read(&raw).with_context(|| format!("reading {}", raw.display()))?;
        all &= verdict(scene, change, &texels, &drawn, dir)?;
        drawn.push((scene, change, texels));
    }
    for o in &rig::OUTPUTS {
        let raw = dir.join(format!("rig-{}.rgba", o.name));
        let bytes = std::fs::read(&raw).with_context(|| format!("reading {}", raw.display()))?;
        all &= rig_verdict(o, Some(&bytes), dir)?;
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
                    ("--off", Some(names)) => off.extend(names.split_whitespace()),
                    _ => bail!("unknown argument {flag}"),
                }
            }
            angle(Path::new(dir), Path::new(out), &off)?
        }
        ["check", dir] => check_browser(Path::new(dir))?,
        _ => bail!(
            "usage: webgl-scenes angle <angle-dir> <out-dir> [--off <extensions>]... | check <dir>"
        ),
    };
    println!("{}", if all { "all as expected" } else { "FAILED" });
    std::process::exit(if all { 0 } else { 1 });
}
