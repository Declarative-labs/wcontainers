//! A run's outputs against the stored references, each picture within the allowance recorded for it.
//!
//! The allowance comes from WARP: a renderer passes a picture when it is at least as close to the
//! GPU's picture as WARP is. With `largest` the largest channel difference between the GPU and WARP,
//! the channel tolerance is `largest` capped at 8 steps of an 8-bit channel (2056 of a 16-bit one,
//! 1/128 of a float one), and the pixels allowed beyond it are those WARP has beyond it, counted in
//! each 16 by 16 tile on its own, so that WARP's freedom in one part of a picture allows nothing in
//! another. Texts must match exactly.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

use anyhow::Context;

use crate::compare::{self, Depth, Difference, TILE};

pub const TOLERANCES: &str = "tolerances.txt";

/// How far a picture may differ from its reference.
#[derive(Clone, Debug, PartialEq)]
pub struct Allowance {
    pub tolerance: f64,
    pub pixels: u64,
    /// Pixels allowed beyond the tolerance in each tile that allows any, by tile index.
    pub tiles: BTreeMap<usize, u32>,
}

impl Allowance {
    /// The allowance WARP's picture earns, as the module explains.
    pub fn from_warp(
        gpu: &image::DynamicImage,
        warp: &image::DynamicImage,
    ) -> anyhow::Result<(Allowance, Difference)> {
        let exact = compare::compare(gpu, warp, 0.0)?;
        let cap = match exact.depth {
            Depth::Eight => 8.0,
            Depth::Sixteen => 8.0 * 257.0,
            Depth::Float => 1.0 / 128.0,
        };
        let tolerance = exact.largest.min(cap);
        let beyond = compare::compare(gpu, warp, tolerance)?;
        let tiles = beyond
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, n)| **n > 0)
            .map(|(i, n)| (i, *n))
            .collect();
        Ok((
            Allowance {
                tolerance,
                pixels: beyond.differing,
                tiles,
            },
            exact,
        ))
    }
}

/// One line of a tolerance file for a picture.
pub fn tolerance_line(file: &str, allowance: &Allowance, warp: &Difference) -> String {
    let mut line = format!(
        "{file} {} {} warp {} {}",
        allowance.tolerance, allowance.pixels, warp.differing, warp.largest
    );
    if !allowance.tiles.is_empty() {
        line.push_str(" tiles ");
        let tiles: Vec<String> = allowance
            .tiles
            .iter()
            .map(|(i, n)| format!("{i}:{n}"))
            .collect();
        line.push_str(&tiles.join(","));
    }
    line.push('\n');
    line
}

/// One line of a tolerance file for a text.
pub fn text_line(file: &str, warp_same: bool) -> String {
    let warp = if warp_same { "same" } else { "differs" };
    format!("{file} exact warp {warp}\n")
}

/// Reads a tolerance file: each picture's allowance by file name.
pub fn read_tolerances(path: &Path) -> anyhow::Result<BTreeMap<String, Allowance>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut allowances = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.get(1) == Some(&"exact") {
            continue;
        }
        let [file, tolerance, pixels, ..] = words[..] else {
            anyhow::bail!("bad line in {}: {line}", path.display());
        };
        let mut tiles = BTreeMap::new();
        if let Some(at) = words.iter().position(|w| *w == "tiles") {
            for tile in words
                .get(at + 1)
                .with_context(|| format!("no tiles after `tiles` in {line}"))?
                .split(',')
            {
                let (i, n) = tile
                    .split_once(':')
                    .with_context(|| format!("bad tile {tile} in {line}"))?;
                tiles.insert(i.parse()?, n.parse()?);
            }
        }
        allowances.insert(
            file.to_owned(),
            Allowance {
                tolerance: tolerance.parse()?,
                pixels: pixels.parse()?,
                tiles,
            },
        );
    }
    Ok(allowances)
}

pub struct Verdict {
    pub file: String,
    pub pass: bool,
    pub detail: String,
}

/// Checks every reference in `reference` (except the tolerance file) against the file of the same
/// name in `run`.
pub fn check_dir(reference: &Path, run: &Path) -> anyhow::Result<Vec<Verdict>> {
    let allowances = read_tolerances(&reference.join(TOLERANCES))?;
    let mut files: Vec<String> = std::fs::read_dir(reference)
        .with_context(|| format!("reading {}", reference.display()))?
        .map(|e| Ok(e?.file_name().to_string_lossy().into_owned()))
        .collect::<anyhow::Result<_>>()?;
    files.retain(|f| f != TOLERANCES);
    files.sort();
    let mut verdicts = Vec::new();
    for file in files {
        let ours = run.join(&file);
        let (pass, detail) = if !ours.exists() {
            (false, "missing".to_owned())
        } else if file.ends_with(".txt") {
            check_text(&reference.join(&file), &ours)?
        } else {
            let allowance = allowances
                .get(&file)
                .with_context(|| format!("{file} has no line in {TOLERANCES}"))?;
            match check_picture(&reference.join(&file), &ours, allowance) {
                Ok(verdict) => verdict,
                Err(e) => (false, format!("{e:#}")),
            }
        };
        verdicts.push(Verdict { file, pass, detail });
    }
    Ok(verdicts)
}

fn check_picture(
    reference: &Path,
    ours: &Path,
    allowance: &Allowance,
) -> anyhow::Result<(bool, String)> {
    let reference = crate::load(reference)?;
    let d = compare::compare(&reference, &crate::load(ours)?, allowance.tolerance)?;
    let mut detail = format!(
        "{} of {} pixels beyond {} (WARP {}), largest difference {}",
        d.differing, d.pixels, allowance.tolerance, allowance.pixels, d.largest
    );
    if let Some((x, y)) = d.first {
        write!(detail, ", first at {x},{y}")?;
    }
    let over = tiles_over(&d, allowance);
    if let Some(&(i, n, allowed)) = over.iter().max_by_key(|(_, n, allowed)| n - allowed) {
        let columns = Difference::tile_columns(reference.width()) as usize;
        write!(
            detail,
            "; {} tiles of {TILE} by {TILE} beyond WARP's count, the worst at {},{}: {n} pixels, WARP {allowed}",
            over.len(),
            i % columns * TILE as usize,
            i / columns * TILE as usize
        )?;
    }
    Ok((over.is_empty(), detail))
}

/// The tiles where `d` has more pixels beyond the tolerance than the allowance: index, count, allowed.
fn tiles_over(d: &Difference, allowance: &Allowance) -> Vec<(usize, u32, u32)> {
    d.tiles
        .iter()
        .enumerate()
        .map(|(i, &n)| (i, n, allowance.tiles.get(&i).copied().unwrap_or(0)))
        .filter(|(_, n, allowed)| n > allowed)
        .collect()
}

fn check_text(reference: &Path, ours: &Path) -> anyhow::Result<(bool, String)> {
    let want = std::fs::read_to_string(reference)?;
    let got = std::fs::read_to_string(ours)?;
    if want == got {
        return Ok((true, "same".to_owned()));
    }
    if want.lines().eq(got.lines()) {
        return Ok((false, "same lines, different line endings".to_owned()));
    }
    let line = want
        .lines()
        .zip(got.lines())
        .position(|(a, b)| a != b)
        .unwrap_or(want.lines().count().min(got.lines().count()));
    Ok((
        false,
        format!(
            "line {}: wanted `{}`, got `{}`",
            line + 1,
            want.lines().nth(line).unwrap_or("<end>"),
            got.lines().nth(line).unwrap_or("<end>")
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};

    #[test]
    fn warp_allowance_caps_the_tolerance() {
        let gpu = RgbaImage::from_pixel(4, 1, Rgba([100, 100, 100, 255]));
        let mut warp = gpu.clone();
        warp.put_pixel(0, 0, Rgba([101, 100, 100, 255]));
        warp.put_pixel(1, 0, Rgba([140, 100, 100, 255]));
        let (allowance, exact) =
            Allowance::from_warp(&DynamicImage::from(gpu), &DynamicImage::from(warp)).unwrap();
        assert_eq!((exact.differing, exact.largest), (2, 40.0));
        assert_eq!(
            allowance,
            Allowance {
                tolerance: 8.0,
                pixels: 1,
                tiles: BTreeMap::from([(0, 1)]),
            }
        );
    }

    #[test]
    fn allowance_holds_per_tile() {
        let gpu = RgbaImage::from_pixel(32, 16, Rgba([100, 100, 100, 255]));
        let mut warp = gpu.clone();
        warp.put_pixel(0, 0, Rgba([140, 100, 100, 255]));
        warp.put_pixel(1, 0, Rgba([140, 100, 100, 255]));
        let gpu = DynamicImage::from(gpu);
        let (allowance, exact) = Allowance::from_warp(&gpu, &DynamicImage::from(warp)).unwrap();
        let line = tolerance_line("a.png", &allowance, &exact);
        assert_eq!(line, "a.png 8 2 warp 2 40 tiles 0:2\n");
        // Two pixels off in the second tile: as many as WARP has in all, but none allowed there.
        let mut ours = gpu.to_rgba8();
        ours.put_pixel(20, 3, Rgba([0, 100, 100, 255]));
        ours.put_pixel(21, 3, Rgba([0, 100, 100, 255]));
        let d = compare::compare(&gpu, &DynamicImage::from(ours), allowance.tolerance).unwrap();
        assert_eq!(tiles_over(&d, &allowance), vec![(1, 2, 0)]);
    }
}
