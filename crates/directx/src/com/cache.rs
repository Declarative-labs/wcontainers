//! The cache file of plan section 8 natively: one per program, in the directory the host names in
//! `WC_GPU_CACHE`, opened by the process's first device.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Once;

use wcontainers_shaders::cache::Cache;

/// Opens the program's cache file once per process; where the host names no directory the front
/// converts and builds as on a first run.
pub(super) fn open_once() {
    static OPENED: Once = Once::new();
    OPENED.call_once(|| {
        let Some(dir) = std::env::var_os("WC_GPU_CACHE") else {
            return;
        };
        if let Err(e) = open(PathBuf::from(dir)) {
            eprintln!("wcontainers direct3d 11: no cache file ({e}); nothing is kept across runs");
        }
    });
}

fn open(dir: PathBuf) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    std::fs::create_dir_all(&dir)?;
    // One file per program: its name, and a hash of its full path so equal names do not meet.
    let full = exe.to_string_lossy().to_lowercase();
    let stem = exe
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let hash = blake3::hash(full.as_bytes()).to_hex();
    let path = dir.join(format!("{stem}-{}.wcgpu", &hash[..16]));
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let empty = bytes.is_empty();
    let cache = match Cache::load(bytes) {
        Ok((cache, _)) if !empty => {
            // A cut last entry is dropped, so appending continues a well-formed file.
            file.set_len(cache.bytes().len() as u64)?;
            cache
        }
        loaded => {
            if let Err(e) = loaded {
                eprintln!(
                    "wcontainers direct3d 11: {} is not a cache of this version ({e}); started again",
                    path.display()
                );
            }
            let cache = Cache::default();
            file.set_len(0)?;
            file.write_all(cache.bytes())?;
            cache
        }
    };
    file.seek(SeekFrom::End(0))?;
    wcontainers_direct3d_11::cache::open(cache, Box::new(file));
    Ok(())
}
