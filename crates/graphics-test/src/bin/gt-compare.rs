//! Compares two pictures at their own depth: `gt-compare <a> <b> [tolerance]`. The tolerance (default
//! 0) is in the pictures' units: steps of an 8 or 16-bit channel, or the value of a float one. Exits 0
//! when no pixel differs in any channel by more than the tolerance, 1 when some do, 2 on an error.

use std::path::Path;
use std::process::ExitCode;

use wcontainers_graphics_test::{compare, load};

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("gt-compare: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> anyhow::Result<bool> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (a, b, tolerance) = match args.as_slice() {
        [a, b] => (a, b, 0.0),
        [a, b, t] => (a, b, t.parse()?),
        _ => anyhow::bail!("usage: gt-compare <a> <b> [tolerance]"),
    };
    let d = compare::compare(&load(Path::new(a))?, &load(Path::new(b))?, tolerance)?;
    print!(
        "{} {} pixels, {} differ by more than {tolerance}, largest channel difference {}",
        d.pixels,
        d.depth.name(),
        d.differing,
        d.largest
    );
    match d.first {
        Some((x, y)) => println!(", first at {x},{y}"),
        None => println!(),
    }
    Ok(d.differing == 0)
}
