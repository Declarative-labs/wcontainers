//! Mirrors the export tables of the system's d3d11.dll and d3d12.dll in one library, copied under
//! either name: every export becomes a thunk of ours with the same name (or none) and ordinal, except
//! the device-creating ones, which go to typed functions of the crate; the two libraries' ordinals
//! do not overlap. A data export cannot be forwarded and is left out (UNFORWARDED). Also reads the
//! method tables of the counted COM interfaces from the Windows SDK's headers and makes the counting
//! thunks the census patches into them (src/census.rs).

use std::fmt::Write;
use std::path::PathBuf;

use object::LittleEndian;
use object::pe::{IMAGE_SCN_MEM_EXECUTE, ImageNtHeaders64};
use object::read::pe::{ExportTarget, PeFile};

/// The libraries mirrored, each with the exports served by typed functions `wc_typed_<name>` of the crate.
const LIBRARIES: &[(&str, &[&str])] = &[
    (
        "d3d11",
        &["D3D11CreateDevice", "D3D11CreateDeviceAndSwapChain"],
    ),
    ("d3d12", &["D3D12CreateDevice"]),
];
/// Interfaces whose methods the census counts, with the SDK header defining each.
const INTERFACES: &[(&str, &str)] = &[
    ("ID3D11Device", "d3d11.h"),
    ("ID3D11Device1", "d3d11_1.h"),
    ("ID3D11Device2", "d3d11_2.h"),
    ("ID3D11Device3", "d3d11_3.h"),
    ("ID3D11Device4", "d3d11_4.h"),
    ("ID3D11Device5", "d3d11_4.h"),
    ("ID3D11DeviceContext", "d3d11.h"),
    ("ID3D11DeviceContext1", "d3d11_1.h"),
    ("ID3D11DeviceContext2", "d3d11_2.h"),
    ("ID3D11DeviceContext3", "d3d11_3.h"),
    ("ID3D11DeviceContext4", "d3d11_3.h"),
    ("IDXGISwapChain", "dxgi.h"),
    ("IDXGISwapChain1", "dxgi1_2.h"),
    ("IDXGISwapChain2", "dxgi1_3.h"),
    ("IDXGISwapChain3", "dxgi1_4.h"),
    ("IDXGISwapChain4", "dxgi1_5.h"),
    ("IDXGIFactory", "dxgi.h"),
    ("IDXGIFactory1", "dxgi.h"),
    ("IDXGIFactory2", "dxgi1_2.h"),
    ("IDXGIFactory3", "dxgi1_3.h"),
    ("IDXGIFactory4", "dxgi1_4.h"),
    ("IDXGIFactory5", "dxgi1_5.h"),
    ("IDXGIFactory6", "dxgi1_6.h"),
    ("IDXGIFactory7", "dxgi1_6.h"),
    ("ID3D12Device", "d3d12.h"),
    ("ID3D12Device1", "d3d12.h"),
    ("ID3D12Device2", "d3d12.h"),
    ("ID3D12Device3", "d3d12.h"),
    ("ID3D12Device4", "d3d12.h"),
    ("ID3D12Device5", "d3d12.h"),
    ("ID3D12Device6", "d3d12.h"),
    ("ID3D12Device7", "d3d12.h"),
    ("ID3D12Device8", "d3d12.h"),
    ("ID3D12Device9", "d3d12.h"),
    ("ID3D12Device10", "d3d12.h"),
    ("ID3D12Device11", "d3d12.h"),
    ("ID3D12Device12", "d3d12.h"),
    ("ID3D12Device13", "d3d12.h"),
    ("ID3D12Device14", "d3d12.h"),
    ("ID3D12CommandQueue", "d3d12.h"),
    ("ID3D12GraphicsCommandList", "d3d12.h"),
    ("ID3D12GraphicsCommandList1", "d3d12.h"),
    ("ID3D12GraphicsCommandList2", "d3d12.h"),
    ("ID3D12GraphicsCommandList3", "d3d12.h"),
    ("ID3D12GraphicsCommandList4", "d3d12.h"),
    ("ID3D12GraphicsCommandList5", "d3d12.h"),
    ("ID3D12GraphicsCommandList6", "d3d12.h"),
    ("ID3D12GraphicsCommandList7", "d3d12.h"),
    ("ID3D12GraphicsCommandList8", "d3d12.h"),
    ("ID3D12GraphicsCommandList9", "d3d12.h"),
    ("ID3D12GraphicsCommandList10", "d3d12.h"),
];
/// How many distinct method tables the census can patch.
const INSTANCES: usize = 24;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("exports.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        std::fs::write(&out, "").unwrap();
        std::fs::write(out.with_file_name("census.rs"), "").unwrap();
        return;
    }
    assert_eq!(
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap(),
        "x86_64",
        "the thunks are x86-64 code"
    );

    let system = std::env::var("SystemRoot").expect("SystemRoot");
    let mut table = String::from("static EXPORTS: &[(usize, Option<&CStr>, u16)] = &[\n");
    let mut unforwarded = String::from("pub static UNFORWARDED: &[&str] = &[\n");
    let mut asm = String::new();
    let mut names = std::collections::HashSet::new();
    let mut ordinals = std::collections::HashSet::new();
    let mut index = 0;
    for (library, (name, typed)) in LIBRARIES.iter().enumerate() {
        let path = PathBuf::from(&system)
            .join("System32")
            .join(format!("{name}.dll"));
        println!("cargo::rerun-if-changed={}", path.display());
        let data =
            std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        let file = PeFile::<ImageNtHeaders64>::parse(&*data).expect("a 64-bit PE file");
        let exports = file
            .export_table()
            .unwrap()
            .expect("an export table")
            .exports()
            .unwrap();
        let sections = file.section_table();
        for export in exports {
            let label = export
                .name
                .map(|n| std::str::from_utf8(n).unwrap().to_owned());
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
                if !executable {
                    // Data has no call to forward; a program importing it would not load.
                    writeln!(
                        unforwarded,
                        "    \"{name}.dll {} (data)\",",
                        label.as_deref().unwrap_or("[no name]")
                    )
                    .unwrap();
                    continue;
                }
            }
            assert!(
                ordinals.insert(export.ordinal),
                "ordinal {} twice",
                export.ordinal
            );
            let thunk = format!("wc_forward_{index}");
            match &label {
                Some(label) => {
                    assert!(names.insert(label.clone()), "export {label} twice");
                    writeln!(
                        table,
                        "    ({library}, Some(c\"{label}\"), {}),",
                        export.ordinal
                    )
                    .unwrap();
                    if typed.contains(&label.as_str()) {
                        println!(
                            "cargo::rustc-cdylib-link-arg=/EXPORT:{label}=wc_typed_{label},@{}",
                            export.ordinal
                        );
                        index += 1;
                        continue;
                    }
                    println!(
                        "cargo::rustc-cdylib-link-arg=/EXPORT:{label}={thunk},@{}",
                        export.ordinal
                    );
                }
                None => {
                    writeln!(table, "    ({library}, None, {}),", export.ordinal).unwrap();
                    println!(
                        "cargo::rustc-cdylib-link-arg=/EXPORT:{thunk},@{},NONAME",
                        export.ordinal
                    );
                }
            }
            // Keeps the four argument registers of both kinds, asks `resolve` for the real
            // function, and jumps to it with the stack as the caller left it.
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
            index += 1;
        }
    }
    unforwarded.push_str("];\n");
    table.push_str("];\n");
    let libraries: Vec<&str> = LIBRARIES.iter().map(|(name, _)| *name).collect();
    let source = format!(
        "pub const LIBRARIES: &[&str] = &{libraries:?};\n{table}{unforwarded}\ncore::arch::global_asm!({asm:?}, resolve = sym resolve);\n"
    );
    std::fs::write(&out, source).unwrap();
    std::fs::write(out.with_file_name("census.rs"), census()).unwrap();
}

/// The newest Windows SDK's header directory (holding um and shared).
fn sdk_headers() -> PathBuf {
    let root = std::path::Path::new(r"C:\Program Files (x86)\Windows Kits\10\Include");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(root)
        .expect("the Windows SDK is not installed")
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("um").join("d3d11_4.h").exists())
        .collect();
    versions.sort();
    versions.pop().expect("no Windows SDK with d3d11_4.h")
}

/// The methods of `interface`, in table order, from its C method table struct in `header`.
fn methods(header: &str, interface: &str) -> Vec<String> {
    let start = format!("typedef struct {interface}Vtbl");
    let end = format!("}} {interface}Vtbl;");
    let from = header.find(&start).unwrap_or_else(|| panic!("no {start}"));
    let body = &header[from..from + header[from..].find(&end).unwrap()];
    body.split("STDMETHODCALLTYPE *")
        .skip(1)
        .map(|rest| rest[..rest.find(')').unwrap()].trim().to_owned())
        .collect()
}

/// The census's interface table and its counting thunks: thunk n, THUNK bytes from the previous
/// one, adds one to COUNTS[n] unless the calling thread is the one in SAVING (the census's own
/// calls) and jumps to ORIGINALS[n], touching no argument register or stack slot (only r11, which
/// a call may change). Instance i of a patched table uses thunks i * SLOTS to i * SLOTS + its
/// length - 1.
fn census() -> String {
    let dir = sdk_headers();
    let mut text = String::from("pub static INTERFACES: &[(&str, &[&str])] = &[\n");
    let mut slots = 0;
    for (interface, file) in INTERFACES {
        let path = ["um", "shared"]
            .iter()
            .map(|sub| dir.join(sub).join(file))
            .find(|p| p.exists())
            .unwrap_or_else(|| panic!("no {file} in the SDK"));
        println!("cargo::rerun-if-changed={}", path.display());
        let header = std::fs::read_to_string(&path).unwrap();
        let names = methods(&header, interface);
        slots = slots.max(names.len());
        writeln!(text, "    (\"{interface}\", &{names:?}),").unwrap();
    }
    text.push_str("];\n");
    writeln!(
        text,
        "pub const INSTANCES: usize = {INSTANCES};\npub const SLOTS: usize = {slots};\npub const THUNK: usize = 32;"
    )
    .unwrap();
    // gs:[0x48] is the calling thread's id (the TEB's ClientId.UniqueThread).
    let own = "mov r11, qword ptr gs:[0x48]\ncmp r11, qword ptr [rip + {saving}]\nje 2f\n";
    let mut asm = String::from(".balign 32\n.globl wc_census_thunks\nwc_census_thunks:\n");
    for n in 0..INSTANCES * slots {
        write!(
            asm,
            ".balign 32\n{own}lock inc qword ptr [rip + {{counts}} + {}]\n2:\njmp qword ptr [rip + {{originals}} + {}]\n",
            8 * n,
            8 * n
        )
        .unwrap();
    }
    // Wrapper thunks for the contexts, which the program is handed wrapped (census.rs): thunk
    // t * CONTEXT_SLOTS + k, 64 bytes from the previous one, counts method k of table t (as above),
    // puts the wrapped object in place of the wrapper as `this` and jumps through the object's own
    // table as it is at that moment.
    let context = INTERFACES
        .iter()
        .find(|(name, _)| *name == "ID3D11DeviceContext4")
        .unwrap();
    let context_slots = methods(
        &std::fs::read_to_string(dir.join("um").join(context.1)).unwrap(),
        context.0,
    )
    .len();
    writeln!(text, "pub const CONTEXT_SLOTS: usize = {context_slots};").unwrap();
    asm.push_str(".balign 64\n.globl wc_wrapper_thunks\nwc_wrapper_thunks:\n");
    for t in 0..2 {
        for k in 0..context_slots {
            write!(
                asm,
                ".balign 64\n{own}lock inc qword ptr [rip + {{wrapped}} + {}]\n2:\nmov rcx, qword ptr [rcx + 8]\n\
                 mov rax, qword ptr [rcx]\njmp qword ptr [rax + {}]\n",
                8 * (t * context_slots + k),
                8 * k
            )
            .unwrap();
        }
    }
    writeln!(
        text,
        "core::arch::global_asm!({asm:?}, counts = sym COUNTS, originals = sym ORIGINALS, wrapped = sym WRAPPED_COUNTS, saving = sym SAVING);"
    )
    .unwrap();
    text
}
