//! Writes the data file and the instruction module from a built XED: `cargo run -p wcontainers-processor-architecture-x86-64 --example generate_forms -- <XED's obj folder>`.
//!
//! The folder must hold the database `python3 pysrc/xed_to_db.py --xed-dgen=obj/dgen --out=obj/xed_db.json` writes.

#[path = "generation/mod.rs"]
#[allow(dead_code, reason = "the generator and each test use different parts")]
mod generation;

use std::error::Error;
use std::path::PathBuf;
use std::{env, fs};

use generation::form::{convert_database, render_data_file};
use generation::render::render_module;
use generation::xed::Database;
use generation::{DATA_FILE, DATABASE_FILE, MODULE_FILE, locate_in_crate};

fn main() -> Result<(), Box<dyn Error>> {
    let xed_build = PathBuf::from(
        env::args_os()
            .nth(1)
            .ok_or("usage: cargo run -p wcontainers-processor-architecture-x86-64 --example generate_forms -- <XED's obj folder>")?,
    );
    let database = Database::parse(&fs::read_to_string(xed_build.join(DATABASE_FILE))?)?;
    let listing = convert_database(&database)?;
    fs::write(
        locate_in_crate(DATA_FILE),
        render_data_file(&listing.forms)?,
    )?;
    fs::write(
        locate_in_crate(MODULE_FILE),
        render_module(&listing.forms, &database.version),
    )?;
    println!(
        "XED {}: {} forms execute in 64-bit mode; {} exist only in 16-bit or 32-bit mode",
        database.version,
        listing.forms.len(),
        listing.legacy_mode_only.len()
    );
    Ok(())
}
