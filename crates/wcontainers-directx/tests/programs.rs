//! Our own two Direct3D 11 programs (the examples `own_handled` and `own_refused`) run as ordinary
//! programs, once beside the system's d3d11.dll and once with our forwarding d3d11.dll and library
//! beside them, and what each printed compared.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The profile's output directory: the tests run from its `deps`.
fn profile() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent().unwrap().parent().unwrap().to_path_buf()
}

/// Runs example `name` from `dir` (copying it there), returning its exit code and output.
fn run(name: &str, dir: &Path) -> (i32, String) {
    let exe = dir.join(format!("{name}.exe"));
    std::fs::copy(profile().join(format!("examples/{name}.exe")), &exe).unwrap();
    let out = Command::new(&exe).current_dir(dir).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!(
        "{name} in {}:\n{text}{}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
}

/// A directory with nothing but what `name` needs: with ours, the forwarder and the library.
fn place(label: &str, ours: bool) -> PathBuf {
    let dir = profile().join("own-programs").join(label);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if ours {
        for file in ["d3d11.dll", "wcontainers_directx.dll"] {
            std::fs::copy(profile().join(file), dir.join(file)).unwrap();
        }
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
