//! Finds the shader containers (DXBC and DXIL) in every file under the given directories, as programs
//! carry them in their binaries and shader files, and writes each distinct one to `<out>/<hash>.dxbc` or
//! `.dxil`, with `<out>/sources.txt` naming the files each came from. A test tool for the corpus of real
//! programs' shaders.
//!   cargo run --example containers -- <out> <dir>...

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The container at the start of `bytes`, if its header and chunk table hold together: its length
/// and whether it is DXIL.
fn container(bytes: &[u8]) -> Option<(usize, bool)> {
    let u32_at = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    if bytes.get(..4)? != b"DXBC" || u32_at(20)? != 1 {
        return None;
    }
    let size = u32_at(24)? as usize;
    let chunks = u32_at(28)? as usize;
    if size > bytes.len() || chunks == 0 || chunks > 64 || 32 + 4 * chunks > size {
        return None;
    }
    let (mut code, mut dxil) = (false, false);
    for i in 0..chunks {
        let at = u32_at(32 + 4 * i)? as usize;
        let tag = bytes.get(at..at + 4)?;
        let length = u32_at(at + 4)? as usize;
        if at + 8 + length > size || !tag.iter().all(u8::is_ascii_alphanumeric) {
            return None;
        }
        match tag {
            b"DXIL" => dxil = true,
            b"SHDR" | b"SHEX" => code = true,
            _ => {}
        }
    }
    (code || dxil).then_some((size, dxil))
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(&path, files),
            Ok(t) if t.is_file() => files.push(path),
            _ => {}
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (out, dirs) = args.split_first().expect("containers <out> <dir>...");
    let out = Path::new(out);
    std::fs::create_dir_all(out).unwrap();
    let mut files = Vec::new();
    for d in dirs {
        walk(Path::new(d), &mut files);
    }
    files.sort();
    let mut found: BTreeMap<String, (bool, Vec<String>)> = BTreeMap::new();
    for file in &files {
        let Ok(bytes) = std::fs::read(file) else {
            continue;
        };
        let mut at = 0;
        while let Some(i) = bytes[at..].windows(4).position(|w| w == b"DXBC") {
            let start = at + i;
            match container(&bytes[start..]) {
                Some((size, dxil)) => {
                    let code = &bytes[start..start + size];
                    let name = blake3::hash(code).to_hex()[..16].to_string();
                    let ext = if dxil { "dxil" } else { "dxbc" };
                    let path = out.join(format!("{name}.{ext}"));
                    if !path.exists() {
                        std::fs::write(&path, code).unwrap();
                    }
                    let source = format!("{}@{start}", file.display());
                    found
                        .entry(name)
                        .or_insert((dxil, Vec::new()))
                        .1
                        .push(source);
                    at = start + size;
                }
                None => at = start + 4,
            }
        }
    }
    let mut list = String::new();
    for (name, (dxil, sources)) in &found {
        let ext = if *dxil { "dxil" } else { "dxbc" };
        list.push_str(&format!("{name}.{ext}\t{}\n", sources.join("\t")));
    }
    std::fs::write(out.join("sources.txt"), list).unwrap();
    let dxil = found.values().filter(|(d, _)| *d).count();
    println!(
        "{} files read, {} distinct containers: {} DXBC, {dxil} DXIL",
        files.len(),
        found.len(),
        found.len() - dxil
    );
}
