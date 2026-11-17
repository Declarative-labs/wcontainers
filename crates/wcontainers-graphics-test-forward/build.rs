//! Mirrors the export table of the system's d3d11.dll: every export becomes a thunk of ours with the
//! same name and ordinal.

use std::fmt::Write;
use std::path::PathBuf;

use object::LittleEndian;
use object::pe::{IMAGE_SCN_MEM_EXECUTE, ImageNtHeaders64};
use object::read::pe::{ExportTarget, PeFile};

const LIBRARY: &str = "d3d11";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("exports.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        std::fs::write(&out, "").unwrap();
        return;
    }
    assert_eq!(
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap(),
        "x86_64",
        "the thunks are x86-64 code"
    );

    let system = std::env::var("SystemRoot").expect("SystemRoot");
    let path = PathBuf::from(system)
        .join("System32")
        .join(format!("{LIBRARY}.dll"));
    println!("cargo::rerun-if-changed={}", path.display());
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let file = PeFile::<ImageNtHeaders64>::parse(&*data).expect("a 64-bit PE file");
    let exports = file
        .export_table()
        .unwrap()
        .expect("an export table")
        .exports()
        .unwrap();
    let sections = file.section_table();

    let mut table = String::from("static EXPORTS: &[&CStr] = &[\n");
    let mut asm = String::new();
    for (index, export) in exports.iter().enumerate() {
        if let ExportTarget::Address(rva) = export.target {
            let executable = sections.iter().any(|s| {
                let (start, size) = (
                    s.virtual_address.get(LittleEndian),
                    s.virtual_size.get(LittleEndian),
                );
                (start..start + size).contains(&rva)
                    && s.characteristics
                        .get(LittleEndian)
                        .contains(IMAGE_SCN_MEM_EXECUTE)
            });
            assert!(
                executable,
                "export {:?} of {LIBRARY}.dll is data, which a thunk cannot forward",
                export.name.map(String::from_utf8_lossy)
            );
        }
        let thunk = format!("wc_forward_{index}");
        let name = export
            .name
            .expect("every export has a name, so programs that import by name find it");
        let name = std::str::from_utf8(name).unwrap();
        writeln!(table, "    c\"{name}\",").unwrap();
        println!(
            "cargo::rustc-cdylib-link-arg=/EXPORT:{name}={thunk},@{}",
            export.ordinal
        );
        // Keeps the four argument registers of both kinds, asks `resolve` for the real function,
        // and jumps to it with the stack as the caller left it.
        write!(
            asm,
            ".globl {thunk}\n{thunk}:\n\
             push rcx\npush rdx\npush r8\npush r9\nsub rsp, 0x68\n\
             movdqu xmmword ptr [rsp + 0x20], xmm0\nmovdqu xmmword ptr [rsp + 0x30], xmm1\n\
             movdqu xmmword ptr [rsp + 0x40], xmm2\nmovdqu xmmword ptr [rsp + 0x50], xmm3\n\
             mov ecx, {index}\ncall {{resolve}}\n\
             movdqu xmm0, xmmword ptr [rsp + 0x20]\nmovdqu xmm1, xmmword ptr [rsp + 0x30]\n\
             movdqu xmm2, xmmword ptr [rsp + 0x40]\nmovdqu xmm3, xmmword ptr [rsp + 0x50]\n\
             add rsp, 0x68\npop r9\npop r8\npop rdx\npop rcx\njmp rax\n"
        )
        .unwrap();
    }
    table.push_str("];\n");
    let source = format!(
        "pub const LIBRARY: &str = \"{LIBRARY}\";\n{table}\ncore::arch::global_asm!({asm:?}, resolve = sym resolve);\n"
    );
    std::fs::write(&out, source).unwrap();
}
