//! On Windows, makes the forwarding libraries programs load by Direct3D's names: each is made by the
//! linker from a `.def` file, holds no code, and forwards every export to `wcontainers_directx.dll`.
//! They are written beside the library in the profile's output directory.

use std::path::PathBuf;

/// Each forwarding library and the entry points the library exports for it.
const FORWARDERS: &[(&str, &[&str])] = &[(
    "d3d11",
    &[
        "D3D11CreateDevice",
        "D3D11CreateDeviceAndSwapChain",
        "D3D11On12CreateDevice",
    ],
)];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let target = std::env::var("TARGET").unwrap();
    if !target.ends_with("windows-msvc") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // OUT_DIR is <profile>/build/<package>-<hash>/out.
    let profile = out.ancestors().nth(3).unwrap().to_path_buf();
    let machine = match std::env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
        "x86_64" => "X64",
        "x86" => "X86",
        "aarch64" => "ARM64",
        other => panic!("no forwarders for {other}"),
    };
    for (name, exports) in FORWARDERS {
        let def = out.join(format!("{name}.def"));
        let mut text = format!("LIBRARY {name}\nEXPORTS\n");
        for e in *exports {
            text.push_str(&format!("    {e}=wcontainers_directx.{e}\n"));
        }
        std::fs::write(&def, text).unwrap();
        let dll = out.join(format!("{name}.dll"));
        let mut link = cc::windows_registry::find_tool(&target, "link.exe")
            .expect("the MSVC linker")
            .to_command();
        link.arg("/NOLOGO")
            .arg("/DLL")
            .arg("/NOENTRY")
            .arg(format!("/MACHINE:{machine}"))
            .arg(format!("/DEF:{}", def.display()))
            .arg(format!(
                "/IMPLIB:{}",
                out.join(format!("{name}.lib")).display()
            ))
            .arg(format!("/OUT:{}", dll.display()));
        let status = link.status().expect("running the MSVC linker");
        assert!(status.success(), "linking {name}.dll failed");
        std::fs::copy(&dll, profile.join(format!("{name}.dll"))).unwrap();
    }
}
