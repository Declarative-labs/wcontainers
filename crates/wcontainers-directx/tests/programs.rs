//! Our own Direct3D 11 programs (the examples `own_handled`, `own_refused`, `own_edges`,
//! `own_depth_copy`, `own_swap_chain`, `own_volumes`, `own_queries`, `own_interfaces`,
//! `own_indirect`, `own_min_lod`, `own_packed`, `own_nv12`, `own_yuy2`, `own_cases`,
//! `own_level10`, `own_d3d10` and `lifetimes`) run as ordinary programs, once beside the system's d3d11.dll and once with our
//! forwarding d3d11.dll and library beside them, and what each printed compared.
#![cfg(windows)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{fresh, place_libraries, profile};
use wcontainers_directx::com::FEATURES_OFF;

/// Runs example `name` from `dir` (copying it there) with `args`, returning its exit code and output.
fn run_with(name: &str, dir: &Path, args: &[&str]) -> (i32, String) {
    run_env(name, dir, args, &[])
}

/// As [`run_with`], with environment variables `env` set.
fn run_env(name: &str, dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String) {
    let (code, text, _) = run_logged(name, dir, args, env);
    (code, text)
}

/// As [`run_env`], also returning what the program wrote to its error output.
fn run_logged(
    name: &str,
    dir: &Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> (i32, String, String) {
    let exe = dir.join(format!("{name}.exe"));
    std::fs::copy(profile().join(format!("examples/{name}.exe")), &exe).unwrap();
    let out = Command::new(&exe)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(dir)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let errors = String::from_utf8_lossy(&out.stderr).into_owned();
    eprintln!("{name} in {}:\n{text}{errors}", dir.display());
    (out.status.code().unwrap_or(-1), text, errors)
}

fn run(name: &str, dir: &Path) -> (i32, String) {
    run_with(name, dir, &[])
}

/// A directory with nothing but what `name` needs: with ours, the forwarder and the library.
fn place(label: &str, ours: bool) -> PathBuf {
    let dir = fresh(label);
    if ours {
        place_libraries(&dir);
    }
    dir
}

/// The value of the line of `text` starting `key `.
fn line<'a>(text: &'a str, key: &str) -> &'a str {
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix(' '))
        .unwrap_or("")
}

#[test]
fn own_handled_draws_the_same_through_ours() {
    let (code, text) = run("own_handled", &place("handled-system", false));
    assert_eq!((code, text.lines().last()), (0, Some("pass")), "{text}");
    let (code, ours) = run("own_handled", &place("handled-ours", true));
    assert_eq!((code, ours.lines().last()), (0, Some("pass")), "{ours}");
    assert_eq!(line(&text, "adapter"), line(&ours, "adapter"));
    let system = line(&text, "D3D11CreateDevice in").to_ascii_lowercase();
    assert!(system.ends_with("\\system32\\d3d11.dll"), "{system}");
    let ours = line(&ours, "D3D11CreateDevice in").to_ascii_lowercase();
    assert!(
        ours.ends_with("\\handled-ours\\wcontainers_directx.dll"),
        "{ours}"
    );
}

#[test]
fn own_refused_is_refused_through_ours_and_served_by_direct3d() {
    let (code, text) = run("own_refused", &place("refused-system", false));
    assert_eq!(code, 0, "{text}");
    assert_eq!(
        text,
        "CreateTexture2D(keyed mutex) 0x00000000\nCreateQuery(occlusion) 0x00000000\nCreateQuery(timestamp) 0x00000000\n\
         CreateBuffer(structured) 0x00000000\nCreateShaderResourceView(structured) 0x00000000\nCheckMultisampleQualityLevels(4) 0x00000000 1\n\
         CreateTexture2D(4 samples) 0x00000000\nGetDeviceRemovedReason 0x00000000\n"
    );
    let (code, ours) = run("own_refused", &place("refused-ours", true));
    assert_eq!(code, 0, "{ours}");
    // E_NOTIMPL for keyed-mutex sharing; the two queries, structured views, 4 samples and the
    // dual-source draw served as Direct3D serves them.
    assert_eq!(
        ours,
        "CreateTexture2D(keyed mutex) 0x80004001\nCreateQuery(occlusion) 0x00000000\nCreateQuery(timestamp) 0x00000000\n\
         CreateBuffer(structured) 0x00000000\nCreateShaderResourceView(structured) 0x00000000\nCheckMultisampleQualityLevels(4) 0x00000000 1\n\
         CreateTexture2D(4 samples) 0x00000000\nGetDeviceRemovedReason 0x00000000\n"
    );
}

#[test]
fn deferred_contexts_states_and_resolves_answer_as_direct3d_does() {
    let (code, want) = run("own_deferred", &place("deferred-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_deferred", &place("deferred-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn own_edges_read_and_copy_as_direct3d_does() {
    let (code, want) = run("own_edges", &place("edges-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_edges", &place("edges-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn indirect_draws_read_their_bases_as_direct3d_does() {
    let (code, want) = run("own_indirect", &place("indirect-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_indirect", &place("indirect-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn two_pixel_formats_sample_load_and_copy_as_direct3d_does() {
    let (code, want) = run("own_packed", &place("packed-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_packed", &place("packed-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn yuy2_pixel_pairs_view_write_copy_and_map_as_direct3d_does() {
    let (code, want) = run("own_yuy2", &place("yuy2-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_yuy2", &place("yuy2-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn nv12_planes_view_write_copy_and_map_as_direct3d_does() {
    let (code, want) = run("own_nv12", &place("nv12-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_nv12", &place("nv12-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

/// Runs one case of `own_cases` beside both libraries and compares what each printed: lines
/// naming a filtered result within one unit a channel (Direct3D's filtering tolerance), every
/// other line exactly.
fn case(name: &str) {
    case_env(name, "ours", &[]);
}

/// As [`case`], ours run with environment variables `env` set (in a directory labelled `label`),
/// returning what ours wrote to its error output.
fn case_env(name: &str, label: &str, env: &[(&str, &str)]) -> String {
    let (code, want) = run_with(
        "own_cases",
        &place(&format!("{name}-system"), false),
        &[name],
    );
    assert_eq!(code, 0, "{want}");
    let (code, got, errors) = run_logged(
        "own_cases",
        &place(&format!("{name}-{label}"), true),
        &[name],
        env,
    );
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        if w.starts_with("filtered") {
            assert!(
                within_one(w, g),
                "the first line that differs by more than one\n{w}\n{g}"
            );
        } else {
            assert_eq!(w, g, "the first line that differs");
        }
    }
    assert_eq!(want.lines().count(), got.lines().count());
    errors
}

/// Runs one case of `own_level10` beside both libraries and compares what each printed, line by
/// line. Built on WebGL 2, ours runs without OES_draw_buffers_indexed, as browsers may.
fn level10(name: &str) {
    let label = |side: &str| place(&format!("level10-{name}-{side}"), side == "ours");
    let (code, want) = run_with("own_level10", &label("system"), &[name]);
    assert_eq!(code, 0, "{want}");
    let off: &[(&str, &str)] = if cfg!(feature = "webgl") {
        &[(FEATURES_OFF, "OES_draw_buffers_indexed")]
    } else {
        &[]
    };
    let (code, got) = run_env("own_level10", &label("ours"), &[name], off);
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn targets_blended_and_masked_each_their_own_way_at_10_0_as_direct3d_does() {
    level10("targets");
}

#[test]
fn view_sizes_and_level_counts_at_10_0_as_direct3d_does() {
    level10("resinfo");
}

#[test]
fn an_empty_slots_size_and_level_count_at_10_0_as_direct3d_does() {
    level10("resinfo_empty");
}

#[test]
fn texture_atomics_as_direct3d_does() {
    case("atomics");
}

#[test]
fn typed_storage_stores_as_direct3d_does() {
    case("typed");
}

/// Without `texture-formats-tier1` (natively, the adapter's own format features), the formats
/// WebGPU cannot store into are stored through a buffer of their texels.
#[cfg(not(feature = "webgl"))]
#[test]
fn typed_storage_stores_without_texture_formats_tier1_as_direct3d_does() {
    let tier1 = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
        .as_str()
        .unwrap();
    let errors = case_env("typed", "untiered", &[(FEATURES_OFF, tier1)]);
    let route = wcontainers_gpu::SlowPath::StorageFormatThroughBuffer.reason();
    assert!(errors.contains(route), "{errors}");
}

#[test]
fn a_storage_view_read_by_the_next_draw_on_the_same_targets_as_direct3d_does() {
    case("split");
}

#[test]
fn depth_and_colour_filtered_through_one_sampler_as_direct3d_does() {
    case("depth_filter");
}

#[test]
fn a_depth_buffer_larger_than_its_target_as_direct3d_does() {
    case("deep");
}

#[test]
fn copies_between_multisampled_textures_as_direct3d_does() {
    case("msaa_copy");
}

#[test]
fn stream_output_also_drawn_as_direct3d_does() {
    case("stream");
}

#[test]
fn early_depth_tests_before_storage_writes_as_direct3d_does() {
    case("early");
}

#[test]
fn early_depth_tests_before_storage_writes_writing_depth_as_direct3d_does() {
    case("early_writes");
}

#[test]
fn stencil_views_read_the_stencil_as_direct3d_does() {
    case("stencil");
}

#[test]
fn a_depth_bias_on_lines_and_points_as_direct3d_does() {
    case("biased_lines");
}

#[test]
fn a_depth_bias_on_what_geometry_and_tessellation_stages_rasterise_as_direct3d_does() {
    case("staged_bias");
}

/// The rows of `text`'s lines starting `prefix`, each a row of bytes written in hex.
fn hex_rows(text: &str, prefix: &str) -> Vec<Vec<u8>> {
    text.lines()
        .filter_map(|l| l.strip_prefix(prefix))
        .map(|l| {
            let hex = l.rsplit_once(' ').map_or(l, |(_, h)| h);
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect()
        })
        .collect()
}

/// Lines under `MultisampleEnable` are drawn by the back end's rule, a loss plan 4.12 lists as
/// at most the pixels at a line's ends and along its edges: every pixel real Direct3D covers in
/// all its samples is covered so, and a pixel may differ only where it touches one real
/// Direct3D covers.
#[test]
fn multisampled_lines_differ_from_direct3d_only_at_their_ends_and_edges() {
    let name = "msaa_lines";
    let (code, want) = run_with(
        "own_cases",
        &place(&format!("{name}-system"), false),
        &[name],
    );
    assert_eq!(code, 0, "{want}");
    let (code, got) = run_with("own_cases", &place(&format!("{name}-ours"), true), &[name]);
    assert_eq!(code, 0, "{got}");
    let prefix = "msaa lines: row ";
    let (w, g) = (hex_rows(&want, prefix), hex_rows(&got, prefix));
    assert!(!w.is_empty());
    assert_eq!((w.len(), w[0].len()), (g.len(), g[0].len()));
    // The most samples real Direct3D covers in a pixel: all of them, along a line's middle.
    let full = *w.iter().flatten().max().unwrap();
    let covered = |y: usize, x: usize| w[y][x] != 0;
    let touches = |y: usize, x: usize| {
        (y.saturating_sub(1)..=(y + 1).min(w.len() - 1))
            .any(|ny| (x.saturating_sub(1)..=(x + 1).min(w[0].len() - 1)).any(|nx| covered(ny, nx)))
    };
    for y in 0..w.len() {
        for x in 0..w[0].len() {
            let (a, b) = (w[y][x], g[y][x]);
            if a == full {
                assert_eq!(
                    b, full,
                    "a fully covered pixel at ({x}, {y})\n{want}\n{got}"
                );
            } else if a != b {
                assert!(
                    touches(y, x),
                    "a pixel off the line at ({x}, {y})\n{want}\n{got}"
                );
            }
        }
    }
    let rest = |t: &str| -> Vec<String> {
        t.lines()
            .filter(|l| !l.starts_with(prefix))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(rest(&want), rest(&got));
}

/// Aliased lines (`MultisampleEnable` FALSE) into four samples: every sample of each pixel a
/// line covers is hit, as Direct3D 11.3's functional specification requires (15.14), with no
/// leeway away from the viewport's edges (3.4.3).
#[test]
#[ignore = "WebGPU rasterises lines into a multisampled target by its own multisampled rule; the interface has no aliased-line route"]
fn aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does() {
    case("aliased_msaa_lines");
}

/// The most a channel of an alpha-antialiased line's pixel may differ from real Direct3D's, in
/// steps of 8 bits: the functional specification (3.4.4) leaves the coverage's filter to the
/// implementation, and the RTX 3080 and WARP differ by up to 128 (the shaders track's
/// `antialiased_lines_draw_as_direct3d_does`, which holds the stage helper's route to this).
const ANTIALIASED_LINE_ALLOWANCE: u8 = 64;

/// Alpha-antialiased lines, a list and a strip blended by their coverage: every channel within
/// the allowance of real Direct3D's, alpha (the coverage) included.
#[test]
fn alpha_antialiased_lines_blend_their_coverage_as_direct3d_does() {
    antialiased_lines("aa_lines");
}

/// The same lines as an indexed strip cut by the restart index.
#[test]
#[ignore = "the shaders track's stage helper refuses indexed strips, which removes the device"]
fn an_indexed_alpha_antialiased_strip_blends_its_coverage_as_direct3d_does() {
    antialiased_lines("aa_strip_indexed");
}

/// Runs own case `name`, alpha-antialiased lines, beside both libraries and compares the
/// target's bytes within the allowance and the rest line by line.
fn antialiased_lines(name: &str) {
    let (code, want) = run_with(
        "own_cases",
        &place(&format!("{name}-system"), false),
        &[name],
    );
    assert_eq!(code, 0, "{want}");
    let (code, got) = run_with("own_cases", &place(&format!("{name}-ours"), true), &[name]);
    assert_eq!(code, 0, "{got}");
    let prefix = "aa lines: row ";
    let (w, g) = (hex_rows(&want, prefix), hex_rows(&got, prefix));
    assert!(!w.is_empty());
    assert_eq!(w.len(), g.len(), "{got}");
    assert_eq!(w[0].len(), g[0].len());
    let texel = std::mem::size_of::<u32>();
    let drawn = |rows: &[Vec<u8>]| {
        rows.iter()
            .flat_map(|r| r.chunks(texel))
            .filter(|t| t[..texel - 1].iter().any(|c| *c != 0))
            .count()
    };
    let (drawn_want, drawn_got) = (drawn(&w), drawn(&g));
    assert!(drawn_want > 0 && drawn_got > 0, "{want}\n{got}");
    let mut largest = 0;
    for (y, (wr, gr)) in w.iter().zip(&g).enumerate() {
        for (x, (a, b)) in wr.iter().zip(gr).enumerate() {
            let d = a.abs_diff(*b);
            largest = largest.max(d);
            assert!(
                d <= ANTIALIASED_LINE_ALLOWANCE,
                "byte {x} of row {y}: {a} and {b}\n{want}\n{got}"
            );
        }
    }
    eprintln!(
        "{drawn_want} pixels drawn by direct3d, {drawn_got} by ours, largest difference {largest}"
    );
    let rest = |t: &str| -> Vec<String> {
        t.lines()
            .filter(|l| !l.starts_with(prefix))
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(rest(&want), rest(&got));
}

#[test]
fn minimum_level_clamps_and_both_helper_blocks_as_direct3d_does() {
    let (code, want) = run("own_min_lod", &place("min-lod-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_min_lod", &place("min-lod-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        // A bias the program adds to an implicit level rounds the level apart from the sampler's
        // own on the RTX 3080 (one unit at 4 of 256 texels): within Direct3D's filtering
        // tolerance, one unit a channel. Everything else is exact.
        if w.contains("biased") {
            assert!(
                within_one(w, g),
                "the first line that differs by more than one\n{w}\n{g}"
            );
        } else {
            assert_eq!(w, g, "the first line that differs");
        }
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

/// Whether two printed rows name the same thing and their hex bytes differ by at most one each.
fn within_one(want: &str, got: &str) -> bool {
    let (Some((wn, wh)), Some((gn, gh))) = (want.rsplit_once(' '), got.rsplit_once(' ')) else {
        return want == got;
    };
    let byte = |h: &str, i: usize| i16::from_str_radix(&h[i..i + 2], 16).ok();
    wn == gn
        && wh.len() == gh.len()
        && (0..wh.len())
            .step_by(2)
            .all(|i| match (byte(wh, i), byte(gh, i)) {
                (Some(a), Some(b)) => (a - b).abs() <= 1,
                _ => wh[i..i + 2] == gh[i..i + 2],
            })
}

#[test]
fn one_and_three_dimensional_textures_as_direct3d_does() {
    let (code, want) = run("own_volumes", &place("volumes-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_volumes", &place("volumes-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn queries_answer_as_direct3d_does() {
    let (code, want) = run("own_queries", &place("queries-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_queries", &place("queries-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn interfaces_answer_as_direct3d_does() {
    let (code, want) = run("own_interfaces", &place("interfaces-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_interfaces", &place("interfaces-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn direct3d_10s_device_draws_and_keeps_its_state_as_direct3d_does() {
    let (code, want) = run("own_d3d10", &place("d3d10-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_d3d10", &place("d3d10-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn a_swap_chain_presents_and_resizes_as_direct3d_does() {
    let (code, want) = run("own_swap_chain", &place("swap-chain-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_swap_chain", &place("swap-chain-ours", true));
    assert_eq!(code, 0, "{got}");
    for (w, g) in want.lines().zip(got.lines()) {
        assert_eq!(w, g, "the first line that differs");
    }
    assert_eq!(want.lines().count(), got.lines().count());
}

#[test]
fn depth_with_stencil_copies_as_direct3d_does() {
    let (code, want) = run("own_depth_copy", &place("depth-copy-system", false));
    assert_eq!(code, 0, "{want}");
    let (code, got) = run("own_depth_copy", &place("depth-copy-ours", true));
    assert_eq!(code, 0, "{got}");
    assert_eq!(want, got);
}

/// Seeds and steps of the `lifetimes` runs: `WC_LIFETIMES=<first seed>,<seeds>,<steps>`.
fn lifetime_cases() -> (u64, u64, usize) {
    let v: Vec<u64> = std::env::var("WC_LIFETIMES")
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    match v[..] {
        [first, seeds, steps] => (first, seeds, steps as usize),
        _ => (1, 8, 300),
    }
}

#[test]
fn object_lifetimes_count_as_direct3d_counts_them() {
    let (first, seeds, steps) = lifetime_cases();
    let (system, ours) = (
        place("lifetimes-system", false),
        place("lifetimes-ours", true),
    );
    for seed in first..first + seeds {
        let args = [&seed.to_string()[..], &steps.to_string()[..]];
        let (code, want) = run_with("lifetimes", &system, &args);
        assert_eq!(code, 0, "{want}");
        let (code, got) = run_with("lifetimes", &ours, &args);
        assert_eq!(code, 0, "{got}");
        for (w, g) in want.lines().zip(got.lines()) {
            assert_eq!(w, g, "seed {seed}: the first line that differs");
        }
        assert_eq!(want.lines().count(), got.lines().count(), "seed {seed}");
    }
    eprintln!(
        "lifetimes: seeds {first}..{} of {steps} steps each",
        first + seeds
    );
}

/// `own_handled` with a depth-stencil view of `format`, beside the system's library and ours.
fn handled_with_depth(format: &str) {
    for ours in [false, true] {
        let dir = place(&format!("handled-{format}-{ours}"), ours);
        let (code, text) = run_with("own_handled", &dir, &[format]);
        assert_eq!((code, text.lines().last()), (0, Some("pass")), "{text}");
    }
}

#[test]
fn own_handled_draws_the_same_through_ours_with_32_bit_depth() {
    handled_with_depth("d32");
}

#[test]
fn own_handled_draws_the_same_through_ours_with_depth_and_stencil() {
    handled_with_depth("d24s8");
}
