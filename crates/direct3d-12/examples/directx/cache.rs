//! The cache file of plan section 8 natively: one per program, in the directory the host names in
//! `WC_GPU_CACHE` (the name and place the Direct3D 11 library uses), opened before the first
//! device. Without the variable conversions are kept for the process only.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use wcontainers_shaders::cache::Cache;

pub fn open() {
    let Some(dir) = std::env::var_os("WC_GPU_CACHE") else {
        return;
    };
    if let Err(e) = open_in(PathBuf::from(dir)) {
        eprintln!("wcontainers direct3d 12: no cache file ({e}); nothing is kept across runs");
    }
}

fn open_in(dir: PathBuf) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    std::fs::create_dir_all(&dir)?;
    // The program's name, and a hash of its full path so programs of one name keep apart.
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
    let cache = match Cache::load(bytes) {
        Ok((cache, dropped)) => {
            if dropped > 0 {
                // A cut last entry: appending continues the well-formed part.
                file.set_len(cache.bytes().len() as u64)?;
            }
            cache
        }
        Err(e) => {
            eprintln!(
                "wcontainers direct3d 12: {} is not a cache of this version ({e}); started again",
                path.display()
            );
            Cache::default()
        }
    };
    if cache.bytes().len() as u64 != file.metadata()?.len() {
        // Empty or unreadable: the file starts again with the header.
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(cache.bytes())?;
    }
    file.seek(SeekFrom::End(0))?;
    wcontainers_direct3d_12::cache::open(cache, Box::new(file));
    Ok(())
}
