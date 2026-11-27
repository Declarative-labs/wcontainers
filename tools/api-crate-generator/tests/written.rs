//! What the generator writes for the Direct3D set (`direct3d-glue`) is, byte for byte, what the repository keeps in
//! crates/, the crates every suite runs on. With `GAC_WINMD` (the pinned metadata) and `GAC_TREE` (the repository's
//! root) set, this is the mutation run's oracle for the generator: any change of what it writes is seen. Without them it
//! checks nothing (the generator's own runs never need the repository).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The API folders the set writes.
const FOLDERS: [&str; 5] = ["component-object-model", "direct3d-common", "directx-graphics-infrastructure", "direct3d-11", "direct3d-12"];

/// Every file under `root`, relative to it, leaving out the hand-written fronts (`implementation/translated`).
fn files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !path.ends_with("implementation/translated") {
                    pending.push(path);
                }
            } else if let Ok(relative) = path.strip_prefix(root) {
                found.push(relative.to_path_buf());
            }
        }
    }
    found.sort();
    found
}

#[test]
fn written_equals_the_tree() {
    let (Some(winmd), Some(tree)) = (std::env::var_os("GAC_WINMD"), std::env::var_os("GAC_TREE")) else {
        return;
    };
    let out = std::env::temp_dir().join(format!("gac-written-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let status = Command::new(env!("CARGO_BIN_EXE_api-crate-generator"))
        .arg("generate")
        .arg(&winmd)
        .arg("direct3d-glue")
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("the generator runs");
    assert!(status.success(), "the generator fails: {status}");
    let mut differing = Vec::new();
    for folder in FOLDERS {
        let written_root = out.join("crates").join(folder);
        let kept_root = Path::new(&tree).join("crates").join(folder);
        let written = files(&written_root);
        let kept = files(&kept_root);
        if written != kept {
            differing.push(format!("{folder}: the files differ ({} written, {} kept)", written.len(), kept.len()));
            continue;
        }
        for file in written {
            if std::fs::read(written_root.join(&file)).ok() != std::fs::read(kept_root.join(&file)).ok() {
                differing.push(format!("{folder}/{}", file.display()));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&out);
    assert!(differing.is_empty(), "written differently from the tree: {differing:?}");
}
