//! Links the inside's programs at the address the monitor loads them at, with Arm's memcpy.

use std::{env, fs, path::Path, path::PathBuf};

#[allow(dead_code)]
#[path = "layout.rs"]
mod layout;

#[allow(dead_code)]
#[path = "build_environment.rs"]
mod build_environment;

use build_environment::{
    LINK_ARG_BINS, OUT_DIR, RERUN_IF_CHANGED, RERUN_IF_ENV_CHANGED, TARGET_OS,
};

/// The target system of the bare-metal targets the inside runs on.
const BARE_METAL: &str = "none";

/// The linker script's file name in the output directory.
const LINKER_SCRIPT: &str = "inside.ld";

/// The linker option that takes a linker script, followed by its path.
const LINKER_SCRIPT_OPTION: &str = "-T";

/// The linker script, given the image's address and the page size.
const LINKER_SCRIPT_TEXT: &str = "ENTRY(_start)
SECTIONS {
  . = IMAGE;
  .text : { KEEP(*(.text.entry)) *(.text.vectors) *(.text .text.*) }
  .rodata : ALIGN(PAGE) { *(.rodata .rodata.*) }
  .data : ALIGN(PAGE) { *(.data .data.*) }
  .bss : ALIGN(PAGE) { *(.bss .bss.*) *(COMMON) }
  /DISCARD/ : { *(.eh_frame .eh_frame_hdr) }
}
";

/// Arm's optimized-routines, whose memcpy and memmove use general registers only (the inside never touches the
/// vector registers, which hold the guest's) and copy far faster than compiler_builtins' word loop.
const ROUTINES: &str = "vendor/optimized-routines";

/// The routine's source in [`ROUTINES`].
const MEMCPY: &str = "memcpy.S";

/// The routine's entries, defined to the names the compiler calls.
const ENTRIES: [(&str, &str); 2] = [
    ("__memcpy_aarch64", "memcpy"),
    ("__memmove_aarch64", "memmove"),
];

/// Leaves out the routine's property note, which the linker script places nowhere.
const NO_PROPERTY_NOTE: (&str, &str) = ("WANT_GNU_PROPERTY", "0");

/// The names the linker script's values take in [`LINKER_SCRIPT_TEXT`].
const IMAGE_NAME: &str = "IMAGE";
const PAGE_NAME: &str = "PAGE";

fn main() {
    // The script's inputs are its own code (the layout included) and the target.
    println!("{RERUN_IF_ENV_CHANGED}{TARGET_OS}");
    if env::var(TARGET_OS).as_deref() != Ok(BARE_METAL) {
        return;
    }
    let script = format!(
        "{IMAGE_NAME} = {:#x};\n{PAGE_NAME} = {:#x};\n{LINKER_SCRIPT_TEXT}",
        layout::IMAGE,
        wcontainers_abi::PAGE_SIZE,
    );
    let out = PathBuf::from(env::var(OUT_DIR).expect("output directory"));
    let path = out.join(LINKER_SCRIPT);
    fs::write(&path, script).expect("linker script");
    println!("{LINK_ARG_BINS}{LINKER_SCRIPT_OPTION}{}", path.display());
    // Linked as objects, so their strong symbols take the place of compiler_builtins' weak ones.
    let mut routines = cc::Build::new();
    routines
        .file(Path::new(ROUTINES).join(MEMCPY))
        .include(ROUTINES)
        .define(NO_PROPERTY_NOTE.0, NO_PROPERTY_NOTE.1);
    for (entry, name) in ENTRIES {
        routines.define(entry, name);
    }
    for object in routines.compile_intermediates() {
        println!("{LINK_ARG_BINS}{}", object.display());
    }
    println!("{RERUN_IF_CHANGED}{ROUTINES}");
}
