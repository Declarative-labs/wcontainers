//! Times `convert` of every DXBC and DXIL container under the given directories: the process's first
//! conversion (which loads the converters), then each converted shader's best of `rounds` conversions
//! after one more, with the median, 95th percentile and worst per shader for each kind of bytecode.
//!   cargo run --release --example conversion_time -- <rounds> <dir>...

use std::path::PathBuf;
use std::time::{Duration, Instant};
use wcontainers_gpu::VariantKey;
use wcontainers_shaders::{Source, convert};

/// The file extensions of the two kinds of bytecode.
const KINDS: [&str; 2] = ["dxbc", "dxil"];

/// The percentile reported beside the median and the worst.
const PERCENTILE: usize = 95;

/// A shader as `convert` takes it.
fn source((dxil, bytes): &(bool, Vec<u8>)) -> Source<'_> {
    match dxil {
        true => Source::Dxil(bytes),
        false => Source::Dxbc(bytes),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let rounds: usize = args
        .next()
        .and_then(|r| r.parse().ok())
        .expect("a round count");
    let mut files: Vec<PathBuf> = args
        .flat_map(|dir| std::fs::read_dir(dir).expect("a readable directory"))
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|e| KINDS.iter().any(|k| e == *k))
        })
        .collect();
    files.sort();
    let shaders: Vec<(bool, Vec<u8>)> = files
        .iter()
        .map(|path| {
            let dxil = path.extension().is_some_and(|e| e == KINDS[1]);
            (dxil, std::fs::read(path).expect("a readable file"))
        })
        .collect();
    let variant = VariantKey::default();
    let start = Instant::now();
    let first = shaders
        .first()
        .map(|s| convert(source(s), &variant, None).is_ok());
    println!(
        "first conversion {:.1} ms (converted: {first:?})",
        start.elapsed().as_secs_f64() * 1e3
    );
    let mut times: [Vec<(Duration, &PathBuf)>; 2] = Default::default();
    let mut refused = [0; 2];
    for (shader, file) in shaders.iter().zip(&files) {
        let kind = usize::from(shader.0);
        if convert(source(shader), &variant, None).is_err() {
            refused[kind] += 1;
            continue;
        }
        let best = (0..rounds)
            .map(|_| {
                let start = Instant::now();
                let _ = convert(source(shader), &variant, None);
                start.elapsed()
            })
            .min()
            .unwrap_or_default();
        times[kind].push((best, file));
    }
    for (kind, mut times) in KINDS.iter().zip(times) {
        times.sort();
        let at = |n: usize| times.get(n).map_or(0.0, |t| t.0.as_secs_f64() * 1e6);
        let total: Duration = times.iter().map(|t| t.0).sum();
        let count = times.len();
        println!(
            "{kind}: {count} converted, {:.1} ms in all; per shader median {:.1} us, {PERCENTILE}th percentile {:.1} us, worst {:.1} us ({:?})",
            total.as_secs_f64() * 1e3,
            at(count / 2),
            at((count * PERCENTILE / 100).min(count.saturating_sub(1))),
            at(count.saturating_sub(1)),
            times.last().map(|t| t.1),
        );
    }
    println!("refused: dxbc {}, dxil {}", refused[0], refused[1]);
}
