//! Compiles the scenes' HLSL (shaders/*.hlsl) with the Windows SDK's compilers when building for
//! Windows: fxc to DXBC for Direct3D 11 into OUT_DIR/dxbc, dxc to DXIL for Direct3D 12 into
//! OUT_DIR/dxil, one file `<shader>.<entry>.<dxbc|dxil>` per entry point. Each HLSL file names its
//! entry points in header lines `// fxc: <target> <entry>, ...` and `// dxc: <target> <entry>, ...`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let shaders = manifest.join("shaders");
    println!("cargo:rerun-if-changed={}", shaders.display());
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let bin = sdk_bin();
    let mut sources: Vec<PathBuf> = std::fs::read_dir(&shaders)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "hlsl"))
        .collect();
    sources.sort();
    for (compiler, kind) in [("fxc", "dxbc"), ("dxc", "dxil")] {
        std::fs::create_dir_all(out.join(kind)).unwrap();
        for source in &sources {
            let text = std::fs::read_to_string(source).unwrap();
            let stem = source.file_stem().unwrap().to_str().unwrap();
            for (target, entry) in entries(&text, compiler) {
                let output = out.join(kind).join(format!("{stem}.{entry}.{kind}"));
                compile(&bin, compiler, source, &target, &entry, &output, &shaders);
            }
        }
    }
}

/// The `(target, entry)` pairs of a source's `// <compiler>:` header lines.
fn entries(text: &str, compiler: &str) -> Vec<(String, String)> {
    let prefix = format!("// {compiler}:");
    text.lines()
        .filter_map(|l| l.strip_prefix(&prefix))
        .flat_map(|l| l.split(','))
        .map(|pair| {
            let mut words = pair.split_whitespace();
            let (Some(target), Some(entry), None) = (words.next(), words.next(), words.next())
            else {
                panic!("bad entry `{pair}`: entries are `<target> <entry>`");
            };
            (target.to_owned(), entry.to_owned())
        })
        .collect()
}

fn compile(
    bin: &Path,
    compiler: &str,
    source: &Path,
    target: &str,
    entry: &str,
    output: &Path,
    include: &Path,
) {
    let mut command = Command::new(bin.join(format!("{compiler}.exe")));
    match compiler {
        "fxc" => command.args(["/nologo", "/Ges", "/O3", "/T", target, "/E", entry, "/I"]),
        _ => command.args(["-nologo", "-O3", "-T", target, "-E", entry, "-I"]),
    };
    command.arg(include).arg("-Fo").arg(output).arg(source);
    let result = command
        .output()
        .unwrap_or_else(|e| panic!("running {compiler}: {e}"));
    if !result.status.success() {
        panic!(
            "{compiler} {} {entry} {target}:\n{}{}",
            source.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

/// The newest Windows SDK's x64 tool directory holding both compilers.
fn sdk_bin() -> PathBuf {
    let root = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(root)
        .expect("the Windows SDK is not installed")
        .map(|e| e.unwrap().path().join("x64"))
        .filter(|p| p.join("fxc.exe").exists() && p.join("dxc.exe").exists())
        .collect();
    versions.sort();
    versions
        .pop()
        .expect("no Windows SDK with fxc.exe and dxc.exe")
}
