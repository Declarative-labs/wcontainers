//! Our own two Direct3D 11 programs (the examples `own_handled` and `own_refused`) run as ordinary
//! programs, once beside the system's d3d11.dll and once with our forwarding d3d11.dll and library
//! beside them, and what each printed compared.
#![cfg(windows)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{fresh, place_libraries, profile};

/// Runs example `name` from `dir` (copying it there) with `args`, returning its exit code and output.
fn run_with(name: &str, dir: &Path, args: &[&str]) -> (i32, String) {
    let exe = dir.join(format!("{name}.exe"));
    std::fs::copy(profile().join(format!("examples/{name}.exe")), &exe).unwrap();
    let out = Command::new(&exe)
        .args(args)
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
        "CreateGeometryShader 0x00000000\nCreateTexture3D 0x00000000\n\
         CreateBuffer(structured) 0x00000000\nGetDeviceRemovedReason 0x00000000\n"
    );
    let (code, ours) = run("own_refused", &place("refused-ours", true));
    assert_eq!(code, 0, "{ours}");
    // E_NOTIMPL for each creation, and the unservable draw removes the device (DXGI_ERROR_DRIVER_INTERNAL_ERROR).
    assert_eq!(
        ours,
        "CreateGeometryShader 0x80004001\nCreateTexture3D 0x80004001\n\
         CreateBuffer(structured) 0x80004001\nGetDeviceRemovedReason 0x887a0020\n"
    );
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
#[ignore = "known failure: the webgpu back end makes a depth-only view of a D24_UNORM_S8_UINT \
            texture, which wgpu refuses as a pass's attachment (the interface says it holds both \
            aspects); the device is removed"]
fn own_handled_draws_the_same_through_ours_with_depth_and_stencil() {
    handled_with_depth("d24s8");
}
