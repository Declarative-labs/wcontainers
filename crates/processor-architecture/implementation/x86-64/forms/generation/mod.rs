//! Turns Intel XED's instruction database into the data file and the instruction module.

pub mod amd_cpuid;
pub mod form;
pub mod privilege;
pub mod render;
pub mod sample;
pub mod xed;

use std::path::{Path, PathBuf};

/// The data file, in the crate's folder.
pub const DATA_FILE: &str = "forms/instruction-forms.jsonl";
/// The instruction module, in the crate's folder.
pub const MODULE_FILE: &str = "forms/instruction.rs";
/// The database that XED's `pysrc/xed_to_db.py` is told to write into XED's build folder.
pub const DATABASE_FILE: &str = "xed_db.json";
/// The table of every iform that XED's build generates for its decoder.
pub const IFORM_TABLE_FILE: &str = "xed-iform-map-init.c";

pub fn locate_in_crate(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(file)
}
