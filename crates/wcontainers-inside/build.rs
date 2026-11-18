//! Links the inside's programs at the address the monitor loads them at.

use std::{env, fs, path::PathBuf};

#[allow(dead_code)]
#[path = "src/layout.rs"]
mod layout;

fn main() {
    println!("cargo::rerun-if-changed=src/layout.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none") {
        return;
    }
    let script = format!(
        "ENTRY(_start)
SECTIONS {{
  . = {image:#x};
  .text : {{ KEEP(*(.text.entry)) *(.text.vectors) *(.text .text.*) }}
  .rodata : ALIGN({page}) {{ *(.rodata .rodata.*) }}
  .data : ALIGN({page}) {{ *(.data .data.*) }}
  .bss : ALIGN({page}) {{ *(.bss .bss.*) *(COMMON) }}
  /DISCARD/ : {{ *(.eh_frame .eh_frame_hdr) }}
}}
",
        image = layout::IMAGE,
        page = wcontainers_abi::PAGE_SIZE,
    );
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let path = out.join("inside.ld");
    fs::write(&path, script).expect("linker script");
    println!("cargo::rustc-link-arg-bins=-T{}", path.display());
}
