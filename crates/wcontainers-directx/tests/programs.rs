//! Our own Direct3D 11 programs (the examples `own_handled`, `own_refused`, `own_edges`,
//! `own_depth_copy`, `own_swap_chain`, `own_volumes`, `own_queries`, `own_interfaces`,
//! `own_indirect`, `own_min_lod`, `own_packed`, `own_nv12`, `own_yuy2`, `own_cases`,
//! `own_level10` and `lifetimes`) run as ordinary programs, once beside the system's d3d11.dll and once with our
//! forwarding d3d11.dll and library beside them, and what each printed compared.
#![cfg(windows)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{fresh, place_libraries, profile};

/// Runs example `name` from `dir` (copying it there) with `args`, returning its exit code and output.
fn run_with(name: &str, dir: &Path, args: &[&str]) -> (i32, String) {
    run_env(name, dir, args, &[])
}

/// As [`run_with`], with environment variables `env` set.
fn run_env(name: &str, dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String) {
    let exe = dir.join(format!("{name}.exe"));
    std::fs::copy(profile().join(format!("examples/{name}.exe")), &exe).unwrap();
    let out = Command::new(&exe)
        .args(args)
        .envs(env.iter().copied())
        .current_dir(dir)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!(
        "{name} in {}:\n{text}{}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
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
    let (code, want) = run_with(
        "own_cases",
        &place(&format!("{name}-system"), false),
        &[name],
    );
    assert_eq!(code, 0, "{want}");
    let (code, got) = run_with("own_cases", &place(&format!("{name}-ours"), true), &[name]);
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
}

/// Runs one case of `own_level10` beside both libraries and compares what each printed, line by
/// line. Built on WebGL 2, ours runs without OES_draw_buffers_indexed, as browsers may.
fn level10(name: &str) {
    let label = |side: &str| place(&format!("level10-{name}-{side}"), side == "ours");
    let (code, want) = run_with("own_level10", &label("system"), &[name]);
    assert_eq!(code, 0, "{want}");
    let off: &[(&str, &str)] = if cfg!(feature = "webgl") {
        &[("WEBGL_OFF", "OES_draw_buffers_indexed")]
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
#[ignore = "known failure: an empty slot's size reads 1 by 1 (its stand-in's), where Direct3D gives 0"]
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
#[ignore = "known failure: early depth-stencil tests before storage writes are not served \
            (EarlyTestsInShader, shaders track); the front refuses the draw"]
fn early_depth_tests_with_storage_writes_as_direct3d_does() {
    case("early");
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
