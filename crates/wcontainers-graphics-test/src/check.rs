//! A run's outputs against the stored references, each picture within the allowance recorded for it.
//!
//! The allowance comes from WARP: a renderer passes a picture when it is at least as close to the
//! GPU's picture as WARP is. With `largest` the largest channel difference between the GPU and WARP,
//! the channel tolerance is `largest` capped at 8 steps of an 8-bit channel (2056 of a 16-bit one,
//! 1/128 of a float one), and the pixels allowed beyond it are those WARP has beyond it, counted in
//! each 16 by 16 tile on its own, so that WARP's freedom in one part of a picture allows nothing in
//! another. Texts must match exactly. Capability answers (`.caps`) may report less than the GPU, line
//! by line, but never more: a less capable implementation's part is then absent, not failed, as
//! is a part the scene left out with a note (`.absent`) saying what the device lacks. A real program's frames have no WARP picture; their allowance
//! comes from repeated runs on the GPU instead (`Allowance::from_repeats`), which also give the
//! stray pixels real Direct3D draws differently in places that move from run to run; a program one of whose
//! frames had more than a twentieth of its tiles changed from run to run has all its frames
//! recorded as unrepeatable, and they are not compared.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

use anyhow::Context;

use crate::compare::{self, Depth, Difference, TILE};
use crate::output::ABSENT;

pub const TOLERANCES: &str = "tolerances.txt";

/// How far a picture may differ from its reference.
#[derive(Clone, Debug, PartialEq)]
pub struct Allowance {
    pub tolerance: f64,
    pub pixels: u64,
    /// Pixels allowed beyond the tolerance in each tile that allows any, by tile index.
    pub tiles: BTreeMap<usize, u32>,
    /// Pixels allowed beyond the tolerance and beyond their tiles' allowances, anywhere.
    pub stray: u64,
}

impl Allowance {
    /// The allowance WARP's picture earns, as the module explains.
    pub fn from_warp(
        gpu: &image::DynamicImage,
        warp: &image::DynamicImage,
    ) -> anyhow::Result<(Allowance, Difference)> {
        let exact = compare::compare(gpu, warp, 0.0)?;
        let tolerance = exact.largest.min(cap(exact.depth));
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
                stray: 0,
            },
            exact,
        ))
    }

    /// The allowance of a program's frame recorded on several runs of real Direct3D: the channel
    /// tolerance is the cap, and a tile where any other run differs from the first beyond it is not
    /// compared (the program draws it differently each time), nor one that meets a rectangle
    /// (x, y, width, height) in `ignored`, where the program shows what each implementation
    /// answers differently (a GPU memory readout). Stray pixels: the few pixels real Direct3D draws
    /// differently (edges of dense meshes under multisampling) can fall in other tiles on every
    /// run, so each other run is held to the tiles the rest allow, and the most pixels one run had
    /// beyond those is allowed anywhere.
    pub fn from_repeats(
        first: &image::DynamicImage,
        others: &[image::DynamicImage],
        ignored: &[[u32; 4]],
    ) -> anyhow::Result<Allowance> {
        let tolerance = cap(compare::Depth::of(first)?);
        let columns = Difference::tile_columns(first.width()) as usize;
        let count = compare::compare(first, first, 0.0)?.tiles.len();
        let size = |i: usize| {
            let (x, y) = ((i % columns) as u32 * TILE, (i / columns) as u32 * TILE);
            (
                x,
                y,
                TILE.min(first.width() - x),
                TILE.min(first.height() - y),
            )
        };
        let left_out: Vec<bool> = (0..count)
            .map(|i| {
                let (x, y, w, h) = size(i);
                ignored
                    .iter()
                    .any(|&[rx, ry, rw, rh]| x < rx + rw && rx < x + w && y < ry + rh && ry < y + h)
            })
            .collect();
        let runs = others
            .iter()
            .map(|other| Ok(compare::compare(first, other, tolerance)?.tiles))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let changed_by = |skip: Option<usize>, i: usize| {
            runs.iter()
                .enumerate()
                .any(|(k, run)| Some(k) != skip && run[i] > 0)
        };
        let stray = (0..runs.len())
            .map(|k| {
                (0..count)
                    .filter(|&i| !left_out[i] && !changed_by(Some(k), i))
                    .map(|i| u64::from(runs[k][i]))
                    .sum::<u64>()
            })
            .max()
            .unwrap_or(0);
        let tiles: BTreeMap<usize, u32> = (0..count)
            .filter(|&i| left_out[i] || changed_by(None, i))
            .map(|i| {
                let (_, _, w, h) = size(i);
                (i, w * h)
            })
            .collect();
        Ok(Allowance {
            tolerance,
            pixels: tiles.values().map(|&n| u64::from(n)).sum(),
            tiles,
            stray,
        })
    }
}

/// The largest channel tolerance an allowance has: 8 steps of an 8-bit channel, 2056 of a 16-bit
/// one, 1/128 of a float one.
fn cap(depth: Depth) -> f64 {
    match depth {
        Depth::Eight => 8.0,
        Depth::Sixteen => 8.0 * 257.0,
        Depth::Float => 1.0 / 128.0,
    }
}

/// One line of a tolerance file for a picture.
pub fn tolerance_line(file: &str, allowance: &Allowance, warp: &Difference) -> String {
    let mut line = format!(
        "{file} {} {} warp {} {}",
        allowance.tolerance, allowance.pixels, warp.differing, warp.largest
    );
    if allowance.stray > 0 {
        write!(line, " stray {}", allowance.stray).unwrap();
    }
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

/// Whether a frame drew so differently from run to run on real Direct3D (more than a twentieth of
/// its tiles) that comparing what is left of it would test little.
pub fn unrepeatable(allowance: &Allowance, tiles: usize) -> bool {
    allowance.tiles.len() * 20 > tiles
}

/// One line of a tolerance file for a frame that is not compared.
pub fn unrepeatable_line(file: &str) -> String {
    format!("{file} unrepeatable\n")
}

/// One line of a tolerance file for a text or capability answers.
pub fn text_line(file: &str, warp_same: bool) -> String {
    let warp = if warp_same { "same" } else { "differs" };
    let rule = if file.ends_with(".caps") {
        "at-most"
    } else {
        "exact"
    };
    format!("{file} {rule} warp {warp}\n")
}

/// Reads a tolerance file: each picture's allowance by file name, none for an unrepeatable frame.
pub fn read_tolerances(path: &Path) -> anyhow::Result<BTreeMap<String, Option<Allowance>>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut allowances = BTreeMap::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let words: Vec<&str> = line.split_whitespace().collect();
        if matches!(words.get(1), Some(&"exact" | &"at-most")) {
            continue;
        }
        if let [file, "unrepeatable"] = words[..] {
            allowances.insert(file.to_owned(), None);
            continue;
        }
        let [file, tolerance, pixels, ..] = words[..] else {
            anyhow::bail!("bad line in {}: {line}", path.display());
        };
        let stray = match words.iter().position(|w| *w == "stray") {
            Some(at) => words
                .get(at + 1)
                .with_context(|| format!("no count after `stray` in {line}"))?
                .parse()?,
            None => 0,
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
            Some(Allowance {
                tolerance: tolerance.parse()?,
                pixels: pixels.parse()?,
                tiles,
                stray,
            }),
        );
    }
    Ok(allowances)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Pass,
    Fail,
    /// The implementation reports what the output needs as unsupported.
    Absent,
}

impl State {
    fn of(pass: bool) -> State {
        if pass { State::Pass } else { State::Fail }
    }
}

pub struct Verdict {
    pub file: String,
    pub state: State,
    pub detail: String,
}

/// The note of an absent part that covers `file`: its own (`<scene>-<part>.absent`) or its
/// scene's (`<scene>.absent`).
fn absent_note(run: &Path, file: &str) -> Option<String> {
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    let scene = file.split(['-', '.']).next().unwrap_or(file);
    [stem, scene]
        .iter()
        .find_map(|s| std::fs::read_to_string(run.join(format!("{s}.{ABSENT}"))).ok())
        .map(|why| format!("needs {}", why.trim()))
}

/// Checks every reference in `reference` (except the tolerance file) against the file of the same
/// name in `run`, and fails each output in `run` (a file `is_output` accepts) that has no reference.
pub fn check_dir(
    reference: &Path,
    run: &Path,
    is_output: impl Fn(&str) -> bool,
) -> anyhow::Result<Vec<Verdict>> {
    let allowances = read_tolerances(&reference.join(TOLERANCES))?;
    let names = |dir: &Path| -> anyhow::Result<Vec<String>> {
        std::fs::read_dir(dir)
            .with_context(|| format!("reading {}", dir.display()))?
            .map(|e| Ok(e?.file_name().to_string_lossy().into_owned()))
            .collect()
    };
    let mut files = names(reference)?;
    files.retain(|f| f != TOLERANCES);
    files.sort();
    let mut verdicts = Vec::new();
    for file in &files {
        let ours = run.join(file);
        let (state, detail) = if !ours.exists() {
            match absent_note(run, file) {
                Some(why) => (State::Absent, why),
                None => (State::Fail, "missing".to_owned()),
            }
        } else if file.ends_with(".txt") {
            check_text(&reference.join(file), &ours)?
        } else if file.ends_with(".caps") {
            check_caps(&reference.join(file), &ours)?
        } else {
            match allowances
                .get(file)
                .with_context(|| format!("{file} has no line in {TOLERANCES}"))?
            {
                None => (
                    State::Pass,
                    "not compared: drew differently on each run of real Direct3D".to_owned(),
                ),
                Some(allowance) => match check_picture(&reference.join(file), &ours, allowance) {
                    Ok((pass, detail)) => (State::of(pass), detail),
                    Err(e) => (State::Fail, format!("{e:#}")),
                },
            }
        };
        verdicts.push(Verdict {
            file: file.clone(),
            state,
            detail,
        });
    }
    let mut extra = names(run)?;
    extra.retain(|f| is_output(f) && !files.contains(f));
    extra.sort();
    for file in extra {
        verdicts.push(Verdict {
            file,
            state: State::Fail,
            detail: format!(
                "written, but {} has no reference of it",
                reference.display()
            ),
        });
    }
    // A note of absence must name a scene or part that has references.
    let mut notes = names(run)?;
    notes.retain(|f| {
        f.strip_suffix(&format!(".{ABSENT}")).is_some_and(|stem| {
            !files.iter().any(|r| {
                r.rsplit_once('.').is_some_and(|(s, _)| s == stem)
                    || r.split(['-', '.']).next() == Some(stem)
            })
        })
    });
    notes.sort();
    for file in notes {
        verdicts.push(Verdict {
            file,
            state: State::Fail,
            detail: format!(
                "a part reported absent that {} has no reference of",
                reference.display()
            ),
        });
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
        "{} of {} pixels beyond {} (allowed {}), largest difference {}",
        d.differing, d.pixels, allowance.tolerance, allowance.pixels, d.largest
    );
    if let Some((x, y)) = d.first {
        write!(detail, ", first at {x},{y}")?;
    }
    let over = tiles_over(&d, allowance);
    let excess: u64 = over
        .iter()
        .map(|(_, n, allowed)| u64::from(n - allowed))
        .sum();
    if allowance.stray > 0 {
        write!(
            detail,
            "; {excess} beyond their tiles' allowance (stray allowed {})",
            allowance.stray
        )?;
    }
    if excess > allowance.stray
        && let Some(&(i, n, allowed)) = over.iter().max_by_key(|(_, n, allowed)| n - allowed)
    {
        let columns = Difference::tile_columns(reference.width()) as usize;
        write!(
            detail,
            "; {} tiles of {TILE} by {TILE} beyond their allowance, the worst at {},{}: {n} pixels, allowed {allowed}",
            over.len(),
            i % columns * TILE as usize,
            i / columns * TILE as usize
        )?;
    }
    Ok((excess <= allowance.stray, detail))
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

fn check_text(reference: &Path, ours: &Path) -> anyhow::Result<(State, String)> {
    let want = std::fs::read_to_string(reference)?;
    let got = std::fs::read_to_string(ours)?;
    if want == got {
        return Ok((State::Pass, "same".to_owned()));
    }
    if want.lines().eq(got.lines()) {
        return Ok((State::Fail, "same lines, different line endings".to_owned()));
    }
    let line = first_difference(&want, &got);
    Ok((State::Fail, line_detail("", &want, &got, line)))
}

fn first_difference(want: &str, got: &str) -> usize {
    want.lines()
        .zip(got.lines())
        .position(|(a, b)| a != b)
        .unwrap_or(want.lines().count().min(got.lines().count()))
}

fn line_detail(what: &str, want: &str, got: &str, line: usize) -> String {
    format!(
        "line {}{what}: wanted `{}`, got `{}`",
        line + 1,
        want.lines().nth(line).unwrap_or("<end>"),
        got.lines().nth(line).unwrap_or("<end>")
    )
}

/// A capability answer's order: no, refused and false lowest, then numbers and feature levels
/// (`11_0` above `10_1`); None for words that are not answers.
fn rank(word: &str) -> Option<u64> {
    match word {
        "false" | "refused" => return Some(0),
        "true" => return Some(1),
        _ => {}
    }
    if let Some((major, minor)) = word.split_once('_') {
        let (major, minor): (u64, u64) = (major.parse().ok()?, minor.parse().ok()?);
        return Some(major << 8 | minor);
    }
    word.parse().ok()
}

/// Capability answers: each line as the GPU's, word by word, where an answer may be lower than the
/// GPU's (absent) but not higher (fail).
fn check_caps(reference: &Path, ours: &Path) -> anyhow::Result<(State, String)> {
    let want = std::fs::read_to_string(reference)?;
    let got = std::fs::read_to_string(ours)?;
    if want.lines().eq(got.lines()) {
        return Ok((State::Pass, "same".to_owned()));
    }
    let (mut first_less, mut less) = (None, 0);
    for (i, (w, g)) in want.lines().zip(got.lines()).enumerate() {
        let (ws, gs): (Vec<&str>, Vec<&str>) = (
            w.split_whitespace().collect(),
            g.split_whitespace().collect(),
        );
        let mut lower = false;
        for k in 0..ws.len().max(gs.len()) {
            let (a, b) = (ws.get(k).copied(), gs.get(k).copied());
            if a == b {
                continue;
            }
            match (a.and_then(rank), b.and_then(rank)) {
                (Some(a), Some(b)) if b < a => lower = true,
                (Some(_), Some(_)) => {
                    return Ok((
                        State::Fail,
                        line_detail(", more than the GPU", &want, &got, i),
                    ));
                }
                _ => return Ok((State::Fail, line_detail("", &want, &got, i))),
            }
        }
        if lower {
            less += 1;
            first_less.get_or_insert(i);
        }
    }
    if want.lines().count() != got.lines().count() {
        let line = first_difference(&want, &got);
        return Ok((State::Fail, line_detail("", &want, &got, line)));
    }
    let i = first_less.unwrap_or(0);
    Ok((
        State::Absent,
        format!(
            "{less} lines report less than the GPU, the first {}",
            line_detail("", &want, &got, i)
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
                stray: 0,
            }
        );
    }

    #[test]
    fn repeats_leave_out_changing_tiles() {
        let first = RgbaImage::from_pixel(40, 16, Rgba([100, 100, 100, 255]));
        let mut second = first.clone();
        second.put_pixel(35, 2, Rgba([200, 100, 100, 255]));
        second.put_pixel(3, 2, Rgba([105, 100, 100, 255]));
        let first = DynamicImage::from(first);
        let allowance =
            Allowance::from_repeats(&first, &[DynamicImage::from(second.clone())], &[]).unwrap();
        assert_eq!(allowance.tolerance, 8.0);
        assert_eq!(allowance.tiles, BTreeMap::from([(2, 8 * 16)]));
        assert_eq!(allowance.pixels, 128);
        // One run alone: nothing else allows its tile, so its pixel may fall anywhere.
        assert_eq!(allowance.stray, 1);
        let allowance =
            Allowance::from_repeats(&first, &[DynamicImage::from(second)], &[[15, 0, 2, 1]])
                .unwrap();
        assert_eq!(
            allowance.tiles,
            BTreeMap::from([(0, 256), (1, 256), (2, 8 * 16)])
        );
    }

    #[test]
    fn stray_pixels_are_those_outside_other_runs_tiles() {
        let first = RgbaImage::from_pixel(64, 16, Rgba([100, 100, 100, 255]));
        let off = |pixels: &[(u32, u32)]| {
            let mut run = first.clone();
            for &(x, y) in pixels {
                run.put_pixel(x, y, Rgba([200, 100, 100, 255]));
            }
            DynamicImage::from(run)
        };
        // Tile 0 is ignored; tile 1 changes in both runs; tile 2 in the first run only, by two.
        let runs = [
            off(&[(0, 0), (1, 0), (2, 0), (16, 0), (32, 0), (33, 0)]),
            off(&[(0, 1), (17, 1)]),
        ];
        let allowance =
            Allowance::from_repeats(&DynamicImage::from(first.clone()), &runs, &[[0, 0, 4, 4]])
                .unwrap();
        assert_eq!(
            allowance.tiles.keys().copied().collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(allowance.stray, 2);
        // Pixels inside ignored or commonly changed tiles never count as stray.
        let runs = [off(&[(0, 0), (16, 0)]), off(&[(1, 1), (17, 1)])];
        let allowance =
            Allowance::from_repeats(&DynamicImage::from(first), &runs, &[[0, 0, 4, 4]]).unwrap();
        assert_eq!(allowance.stray, 0);
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

    #[test]
    fn stray_pixels_anywhere() {
        let root = Path::new(env!("OUT_DIR")).join("stray_pixels_anywhere");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let reference = RgbaImage::from_pixel(48, 16, Rgba([100, 100, 100, 255]));
        reference.save(root.join("reference.png")).unwrap();
        let allowance = Allowance {
            tolerance: 8.0,
            pixels: 256,
            tiles: BTreeMap::from([(0, 256)]),
            stray: 2,
        };
        let line = tolerance_line(
            "a.png",
            &allowance,
            &compare::compare(
                &DynamicImage::from(reference.clone()),
                &DynamicImage::from(reference.clone()),
                0.0,
            )
            .unwrap(),
        );
        assert_eq!(line, "a.png 8 256 warp 0 0 stray 2 tiles 0:256\n");
        let path = root.join(TOLERANCES);
        std::fs::write(&path, &line).unwrap();
        assert_eq!(
            read_tolerances(&path).unwrap()["a.png"],
            Some(allowance.clone())
        );
        // Anything in the allowed tile, and two stray pixels in two other tiles, pass; a third fails.
        let mut ours = reference.clone();
        for (x, y) in [(0, 0), (1, 1), (2, 2), (20, 3), (40, 5)] {
            ours.put_pixel(x, y, Rgba([0, 100, 100, 255]));
        }
        ours.save(root.join("ours.png")).unwrap();
        let check = || {
            check_picture(
                &root.join("reference.png"),
                &root.join("ours.png"),
                &allowance,
            )
            .unwrap()
            .0
        };
        assert!(check());
        ours.put_pixel(41, 5, Rgba([0, 100, 100, 255]));
        ours.save(root.join("ours.png")).unwrap();
        assert!(!check());
    }

    #[test]
    fn outputs_without_reference_fail() {
        let root = Path::new(env!("OUT_DIR")).join("outputs_without_reference_fail");
        let (reference, run) = (root.join("reference"), root.join("run"));
        let _ = std::fs::remove_dir_all(&root);
        for dir in [&reference, &run] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("a-text.txt"), "same\n").unwrap();
        }
        std::fs::write(reference.join(TOLERANCES), "a-text.txt exact warp same\n").unwrap();
        std::fs::write(run.join("a-new.txt"), "unseen\n").unwrap();
        std::fs::write(run.join("a.log"), "not an output\n").unwrap();
        let verdicts = check_dir(&reference, &run, |f| f.ends_with(".txt")).unwrap();
        let seen: Vec<(&str, State)> = verdicts.iter().map(|v| (&*v.file, v.state)).collect();
        assert_eq!(
            seen,
            [("a-text.txt", State::Pass), ("a-new.txt", State::Fail)]
        );
    }

    #[test]
    fn absent_parts_and_lesser_answers() {
        let root = Path::new(env!("OUT_DIR")).join("absent_parts_and_lesser_answers");
        let (reference, run) = (root.join("reference"), root.join("run"));
        let _ = std::fs::remove_dir_all(&root);
        for dir in [&reference, &run] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let caps = "level 11_1\nr8 render target 1 blendable 1\nreduction true\n";
        std::fs::write(reference.join("a-caps.caps"), caps).unwrap();
        std::fs::write(reference.join("a-sums.txt"), "1\n").unwrap();
        std::fs::write(reference.join("b-x.txt"), "1\n").unwrap();
        std::fs::write(reference.join("b-y.txt"), "1\n").unwrap();
        std::fs::write(reference.join(TOLERANCES), "").unwrap();
        let lesser = "level 10_0\nr8 render target 1 blendable 0\nreduction true\n";
        std::fs::write(run.join("a-caps.caps"), lesser).unwrap();
        std::fs::write(run.join("a-sums.absent"), "needs sums\n").unwrap();
        std::fs::write(run.join("b.absent"), "needs level 11_0\n").unwrap();
        std::fs::write(run.join("c.absent"), "no such scene\n").unwrap();
        let states = |run: &Path| -> Vec<(String, State)> {
            check_dir(&reference, run, |f| {
                f.ends_with(".txt") || f.ends_with(".caps")
            })
            .unwrap()
            .into_iter()
            .map(|v| (v.file, v.state))
            .collect()
        };
        let want = [
            ("a-caps.caps", State::Absent),
            ("a-sums.txt", State::Absent),
            ("b-x.txt", State::Absent),
            ("b-y.txt", State::Absent),
            ("c.absent", State::Fail),
        ]
        .map(|(f, s)| (f.to_owned(), s));
        assert_eq!(states(&run), want);
        // More than the GPU, or a changed word, fails.
        for wrong in [
            "level 12_0\n",
            "level 11_1\nr8 render target 2 blendable 1\n",
        ] {
            let body = format!("{wrong}{}", &caps[wrong.len().min(caps.len())..]);
            std::fs::write(run.join("a-caps.caps"), body).unwrap();
            assert_eq!(states(&run)[0].1, State::Fail, "{wrong}");
        }
        std::fs::write(run.join("a-caps.caps"), caps.replace("r8", "r16")).unwrap();
        assert_eq!(states(&run)[0].1, State::Fail);
    }
}
