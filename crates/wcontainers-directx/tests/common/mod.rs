//! Where the tests find the libraries this test run built.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The profile's output directory: the tests run from its `deps`.
pub fn profile() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent().unwrap().parent().unwrap().to_path_buf()
}

/// Copies the library and the forwarding d3d11.dll this test run built into `dir`: the library from
/// `deps`, where `cargo test` builds it (only `cargo build` copies it into the profile directory), and
/// the forwarder from the build script's output.
pub fn place_libraries(dir: &Path) {
    let built = [
        profile().join("deps").join("wcontainers_directx.dll"),
        Path::new(env!("OUT_DIR")).join(wcontainers_directx::com::D3D11_LIBRARY),
    ];
    for from in built {
        let to = dir.join(from.file_name().unwrap());
        std::fs::copy(&from, &to).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
    }
}

/// One directory holding the libraries this test run built, placed once in the process: Windows
/// resolves the forwarder's import by the library's name to whichever copy was loaded first.
pub fn libraries() -> &'static Path {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = fresh("libraries");
        place_libraries(&dir);
        dir
    })
}

/// An empty directory `label` of the tests' own, made afresh.
pub fn fresh(label: &str) -> PathBuf {
    let dir = profile().join("directx-tests").join(label);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
