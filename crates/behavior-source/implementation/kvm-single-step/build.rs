//! Assembles the handler and writes its bytes and the offset of its entry for the machine to include.

#![allow(
    dead_code,
    reason = "the build script shares the machine's layout modules and reads only what the handler is assembled with"
)]

use std::mem::offset_of;
use std::path::PathBuf;
use std::{env, fs};

use object::{Object, ObjectSection, ObjectSymbol};

#[path = "guest_layout.rs"]
mod guest_layout;
#[path = "slot.rs"]
mod slot;

use guest_layout::{DOORBELL_FUNCTION, VECTOR_BYTES, VECTOR_TABLE_BYTES};
use slot::{Ending, Registers, Slot};

/// The assembly source of the handler.
const HANDLER_SOURCE: &str = "handler.S";
/// What the build writes into its output folder: the handler's bytes, and the offset of its entry in them.
const HANDLER_BYTES: &str = "handler.bin";
const HANDLER_OFFSETS: &str = "handler_offsets.rs";
/// The labels of the handler's start, its entry and its end.
const VECTORS_LABEL: &str = "handler_vectors";
const ENTRY_LABEL: &str = "handler_entry";
const END_LABEL: &str = "handler_end";
/// The section the assembler puts the handler in.
const TEXT_SECTION: &str = ".text";
/// The target the machine runs on, as cargo names its architecture and operating system.
const TARGET_ARCH: &str = "aarch64";
const TARGET_OS: &str = "linux";

fn main() {
    println!("cargo::rerun-if-changed={HANDLER_SOURCE}");
    let Some(output) = env::var_os("OUT_DIR").map(PathBuf::from) else {
        panic!("cargo sets OUT_DIR for a build script");
    };
    let is_target = env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|arch| arch == TARGET_ARCH)
        && env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == TARGET_OS);
    // Elsewhere the machine is not built, so the handler is not assembled.
    let (bytes, entry) = if is_target {
        assemble()
    } else {
        (Vec::new(), u64::MIN)
    };
    let written = fs::write(output.join(HANDLER_BYTES), bytes).and_then(|()| {
        fs::write(
            output.join(HANDLER_OFFSETS),
            format!("/// Bytes from the handler's start to its entry.\nconst HANDLER_ENTRY_OFFSET: u64 = {entry};\n"),
        )
    });
    if let Err(error) = written {
        panic!("writing the handler into {}: {error}", output.display());
    }
}

/// Returns the bytes of the handler from its vectors to its end, and the offset of its entry in them.
fn assemble() -> (Vec<u8>, u64) {
    let defines = [
        ("VECTOR_TABLE_BYTES", VECTOR_TABLE_BYTES),
        ("VECTOR_BYTES", VECTOR_BYTES),
        ("DOORBELL_FUNCTION", u64::from(DOORBELL_FUNCTION)),
        (
            "SLOT_TRANSLATION_TABLE",
            offset_of!(Slot, translation_table) as u64,
        ),
        ("SLOT_NEXT", offset_of!(Slot, next) as u64),
        ("SLOT_ENDING", offset_of!(Slot, ending) as u64),
        ("SLOT_BEFORE", offset_of!(Slot, before) as u64),
        ("SLOT_AFTER", offset_of!(Slot, after) as u64),
        (
            "REGISTERS_STACK_POINTER",
            offset_of!(Registers, stack_pointer) as u64,
        ),
        (
            "REGISTERS_PROGRAM_COUNTER",
            offset_of!(Registers, program_counter) as u64,
        ),
        (
            "REGISTERS_SAVED_PROGRAM_STATUS",
            offset_of!(Registers, saved_program_status) as u64,
        ),
        (
            "REGISTERS_FLOATING_POINT_CONTROL",
            offset_of!(Registers, floating_point_control) as u64,
        ),
        ("REGISTERS_SYNDROME", offset_of!(Registers, syndrome) as u64),
        ("REGISTERS_VECTOR", offset_of!(Registers, vector) as u64),
        ("ENDING_EXCEPTION_FROM_EL0", Ending::ExceptionFromEl0 as u64),
        ("ENDING_UNEXPECTED", Ending::Unexpected as u64),
    ];
    let mut build = cc::Build::new();
    build.file(HANDLER_SOURCE);
    for (name, value) in defines {
        build.define(name, value.to_string().as_str());
    }
    let objects = build.compile_intermediates();
    let [object_path] = &objects[..] else {
        panic!(
            "the handler assembles into one object, not {}",
            objects.len()
        );
    };
    let object_bytes = match fs::read(object_path) {
        Ok(bytes) => bytes,
        Err(error) => panic!("reading {}: {error}", object_path.display()),
    };
    let file = match object::File::parse(&*object_bytes) {
        Ok(file) => file,
        Err(error) => panic!("parsing {}: {error}", object_path.display()),
    };
    let find_label = |name: &str| {
        file.symbols()
            .find(|symbol| symbol.name() == Ok(name))
            .map(|symbol| symbol.address())
            .unwrap_or_else(|| panic!("the handler has no label {name}"))
    };
    let (start, entry, end) = (
        find_label(VECTORS_LABEL),
        find_label(ENTRY_LABEL),
        find_label(END_LABEL),
    );
    let text = file
        .section_by_name(TEXT_SECTION)
        .and_then(|section| section.data().ok())
        .unwrap_or_else(|| panic!("the handler's object has no {TEXT_SECTION}"));
    (text[start as usize..end as usize].to_vec(), entry - start)
}
