//! Compresses the kernel's configuration (`fs/config`) for `/proc/config.gz`, as Linux's
//! build does with `gzip -n -9`.

use std::io::Write;

/// The kernel's configuration, relative to the crate.
const CONFIGURATION: &str = "fs/config";

fn main() {
    let source = CONFIGURATION;
    println!("cargo::rerun-if-changed={source}");
    let text = std::fs::read(source).expect("the configuration");
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder
        .write_all(&text)
        .expect("compressing the configuration");
    let compressed = encoder.finish().expect("compressing the configuration");
    let out =
        std::path::Path::new(&std::env::var("OUT_DIR").expect("cargo's OUT_DIR")).join("config.gz");
    std::fs::write(out, compressed).expect("writing the compressed configuration");
}
