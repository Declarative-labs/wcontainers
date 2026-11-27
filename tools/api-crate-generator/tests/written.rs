//! The generator writes, on the pinned inputs, exactly the files the repository holds for the APIs it writes from C
//! headers and Khronos's registries: crates/gallium (Mesa 26.3.1's Gallium headers), crates/mesa-compiler (the bridge's
//! headers), crates/egl and crates/opengl-es (gl.xml, egl.xml, ANGLE's gl_angle_ext.xml). Those files are reviewed and
//! tested in the repository (their crates' suites state the right values), so a change of the generator that changes
//! what it writes fails here. What it prints (its reports: the counts, every parameter's mapping row, the census of the
//! headers, what is left out and why) is compared with reports/, as reviewed. The inputs come from the environment,
//! and the test fails without them:
//!   WCONTAINERS_GENERATOR_REPOSITORY  the repository's root (the files compared with, and the bridge's headers)
//!   WCONTAINERS_MESA                  Mesa's Meson build folder (crates/nir/mesa/build): its source folder beside it
//!   WCONTAINERS_GENERATOR_REGISTRIES  the folder fetch-registries.sh fetched gl.xml, egl.xml and gl_angle_ext.xml into

use std::path::{Path, PathBuf};
use std::process::Command;

fn input(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} names an input of this test (tests/written.rs); it fails without it")))
}

/// Every file under `folder`, relative to it.
fn files(folder: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(at) = pending.pop() {
        for entry in std::fs::read_dir(&at).unwrap_or_else(|error| panic!("{}: {error}", at.display())) {
            let path = entry.expect("a folder entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                out.push(path.strip_prefix(folder).expect("under the folder").to_path_buf());
            }
        }
    }
    out.sort();
    out
}

/// Runs the generator with `arguments`; its report (what it prints) must be reports/<report>.txt.
fn reported(report: &str, arguments: &[PathBuf]) -> std::process::Output {
    let run = Command::new(env!("CARGO_BIN_EXE_api-crate-generator")).args(arguments).output().expect("the generator runs");
    assert!(run.status.success(), "{report}: the generator failed: {}", String::from_utf8_lossy(&run.stderr));
    let expected = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("reports").join(format!("{report}.txt"))).unwrap_or_default();
    let printed = String::from_utf8_lossy(&run.stdout);
    assert!(printed == expected, "{report}: the report differs from reports/{report}.txt; it printed:\n{printed}");
    run
}

/// Runs the generator with `arguments` writing into `out` (its report compared as `reported` does), and compares every
/// file it wrote under out/crates with the repository's.
fn written_equal(what: &str, arguments: &[PathBuf], out: &Path) {
    let _ = std::fs::remove_dir_all(out);
    let mut with_out = arguments.to_vec();
    with_out.push(out.to_path_buf());
    reported(what, &with_out);
    let repository = input("WCONTAINERS_GENERATOR_REPOSITORY");
    let crates = out.join("crates");
    let written = files(&crates);
    assert!(!written.is_empty(), "{what}: nothing written");
    let mut different = Vec::new();
    for file in &written {
        let ours = std::fs::read(crates.join(file)).expect("a written file");
        let theirs = std::fs::read(repository.join("crates").join(file)).unwrap_or_default();
        if ours != theirs {
            different.push(file.display().to_string());
        }
    }
    assert!(different.is_empty(), "{what}: {} of {} files differ from the repository's: {different:?}", different.len(), written.len());
    println!("WRITTEN {what}: {} files, each as the repository holds it", written.len());
}

fn work(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

#[test]
fn gallium_written_as_held() {
    let build = input("WCONTAINERS_MESA");
    let source = build.parent().expect("the build folder's parent").join("src");
    let gallium = source.join("src/gallium");
    let mut arguments = vec![PathBuf::from("gallium"), Path::new(env!("CARGO_MANIFEST_DIR")).join("inputs/gallium/in-scope.txt")];
    arguments.extend([
        source.join("src/compiler/shader_enums.h"),
        build.join("src/util/format/u_format_gen.h"),
        source.join("src/util/format/u_formats.h"),
        source.join("src/util/blend.h"),
        source.join("src/util/box.h"),
        gallium.join("include/pipe/p_defines.h"),
        gallium.join("include/pipe/p_state.h"),
        gallium.join("include/pipe/p_screen.h"),
        gallium.join("include/pipe/p_context.h"),
        gallium.join("auxiliary/util/u_transfer_helper.h"),
    ]);
    written_equal("gallium", &arguments, &work("written-gallium"));
    let mut plan = arguments.clone();
    plan[0] = PathBuf::from("gallium-plan");
    reported("gallium-plan", &plan);
    let mut census = vec![PathBuf::from("gallium-census")];
    census.extend(arguments[2..].iter().cloned());
    reported("gallium-census", &census);
}

#[test]
fn mesa_compiler_written_as_held() {
    let bridge = input("WCONTAINERS_GENERATOR_REPOSITORY").join("crates/mesa-compiler/implementation/direct/bridge");
    written_equal("mesa-compiler", &[PathBuf::from("mesa-compiler"), bridge.join("flat.h"), bridge.join("bridge.h")], &work("written-mesa-compiler"));
    reported("c-header", &[PathBuf::from("c-header"), bridge.join("flat.h"), bridge.join("bridge.h")]);
}

#[test]
fn khronos_written_as_held() {
    let registries = input("WCONTAINERS_GENERATOR_REGISTRIES");
    written_equal("khronos", &[PathBuf::from("khronos"), registries.join("gl.xml"), registries.join("egl.xml"), registries.join("gl_angle_ext.xml")], &work("written-khronos"));
}
