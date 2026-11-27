//! Writes an outside API's crate from the API's own description.
//!
//!   api-crate-generator census <Windows.Win32.winmd> <namespace>...
//!   api-crate-generator generate <Windows.Win32.winmd> <api> <output folder>
//!   api-crate-generator layout-check <Windows.Win32.winmd> <api> <output.c> [C type left out]...
//!   api-crate-generator once-check <Windows.Win32.winmd> <folder holding crates/>

mod api;
mod complete;
mod cheader;
mod mesacompiler;
mod emit;
mod khronos;
mod layout;
mod mapping;
mod metadata;
mod names;
mod runtime;
mod trust;
mod undo;
mod visibility;

use std::collections::BTreeMap;
use std::process::ExitCode;

use windows_metadata::reader::TypeIndex;

fn census(index: &TypeIndex, namespaces: &[&str]) {
    let read = metadata::read(index, namespaces);
    for namespace in namespaces {
        let interfaces: Vec<_> = read.interfaces.values().filter(|i| i.namespace == *namespace).collect();
        let methods: usize = interfaces.iter().map(|i| i.methods.len()).sum();
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for record in read.records.values().filter(|r| r.namespace == *namespace) {
            *kinds.entry(format!("{:?}", record.kind)).or_default() += 1;
        }
        let constants = read.constants.keys().filter(|(n, _)| n == namespace).count();
        let mut libraries: BTreeMap<String, usize> = BTreeMap::new();
        for ((function_namespace, _), function) in &read.functions {
            if function_namespace == namespace {
                *libraries.entry(function.library.clone().unwrap_or_default().to_lowercase()).or_default() += 1;
            }
        }
        println!(
            "CENSUS {namespace}: interfaces {} methods {methods} records {kinds:?} constants {constants} functions by library {libraries:?}",
            interfaces.len()
        );
    }
    let mut attribute_counts: BTreeMap<String, usize> = BTreeMap::new();
    for interface in read.interfaces.values() {
        for method in &interface.methods {
            for param in &method.params {
                for attribute in &param.attributes {
                    *attribute_counts.entry(attribute.name.clone()).or_default() += 1;
                }
            }
        }
    }
    println!("CENSUS parameter attributes of every method: {attribute_counts:?}");
    for interface in read.interfaces.values() {
        println!(
            "INTERFACE {}.{} : {:?} methods {}",
            interface.namespace,
            interface.name,
            interface.base.as_ref().map(|(_, name)| name),
            interface.methods.len()
        );
    }
    for ((namespace, name), function) in &read.functions {
        println!("FUNCTION {namespace}.{name} {:?}", function.library);
    }
    let mut record_attributes: BTreeMap<String, usize> = BTreeMap::new();
    for record in read.records.values() {
        for attribute in &record.attributes {
            *record_attributes.entry(attribute.name.clone()).or_default() += 1;
        }
    }
    println!("CENSUS record attributes: {record_attributes:?}");
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = arguments.split_first() else {
        eprintln!("usage: api-crate-generator census|generate <Windows.Win32.winmd> ...");
        return ExitCode::FAILURE;
    };
    // A C header's census and its default-deny classification (AC11's bridge, AC8's Gallium headers).
    if command == "c-header" {
        let texts: Result<Vec<String>, String> = rest.iter().map(|path| std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))).collect();
        return match texts {
            Ok(texts) => {
                print!("{}", cheader::census(&cheader::read(&texts)));
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("api-crate-generator: {error}");
                ExitCode::FAILURE
            }
        };
    }
    // Gallium's driver interface (AC8): the function pointers of p_screen.h and p_context.h.
    if command == "gallium-census" {
        let texts: Result<Vec<String>, String> = rest.iter().map(|path| std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))).collect();
        return match texts {
            Ok(texts) => {
                print!("{}", cheader::gallium_census(&cheader::read(&texts)));
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("api-crate-generator: {error}");
                ExitCode::FAILURE
            }
        };
    }
    // The Mesa compiler's crates from the bridge's headers.
    if let ("mesa-compiler", [flat, bridge, out]) = (command.as_str(), rest) {
        let texts: Result<Vec<String>, String> = [flat, bridge].iter().map(|path| std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))).collect();
        return match texts.and_then(|texts| mesacompiler::generate(&texts, std::path::Path::new(out))) {
            Ok(report) => {
                print!("{report}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("api-crate-generator: {error}");
                ExitCode::FAILURE
            }
        };
    }
    // Khronos's registries, not Microsoft's metadata.
    if let ("khronos", [gl, egl, angle, out]) = (command.as_str(), rest) {
        return match khronos::generate(gl, egl, angle, std::path::Path::new(out)) {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("api-crate-generator: {error}");
                ExitCode::FAILURE
            }
        };
    }
    let Some((winmd, rest)) = rest.split_first() else {
        eprintln!("usage: api-crate-generator {command} <Windows.Win32.winmd> ...");
        return ExitCode::FAILURE;
    };
    let Some(index) = TypeIndex::read(winmd) else {
        eprintln!("api-crate-generator: cannot read {winmd}");
        return ExitCode::FAILURE;
    };
    match (command.as_str(), rest) {
        ("census", namespaces) => {
            let namespaces: Vec<&str> = namespaces.iter().map(String::as_str).collect();
            census(&index, &namespaces);
            ExitCode::SUCCESS
        }
        ("layout-check", [name, out, left_out @ ..]) => {
            let Some(api) = api::by_name(name) else {
                eprintln!("api-crate-generator: no API named {name}");
                return ExitCode::FAILURE;
            };
            match layout::write_check(&index, api, std::path::Path::new(out), left_out) {
                Ok(count) => {
                    println!("LAYOUT CHECK: {count} assertions written to {out}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("api-crate-generator: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        // The check that every safe trait of the other party is asked once per value, on crates written earlier.
        ("once-check", [out]) => match trust::check(std::path::Path::new(out), None) {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(failure) => {
                println!("{failure}");
                ExitCode::FAILURE
            }
        },
        ("generate", [name, out]) => {
            let Some(api) = api::by_name(name) else {
                eprintln!("api-crate-generator: no API named {name}; known: {}", api::names().join(", "));
                return ExitCode::FAILURE;
            };
            match emit::generate(&index, api, std::path::Path::new(out), api::writes_glue(name)) {
                Ok(report) => {
                    println!("{report}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("api-crate-generator: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: api-crate-generator census|generate <Windows.Win32.winmd> ...");
            ExitCode::FAILURE
        }
    }
}
