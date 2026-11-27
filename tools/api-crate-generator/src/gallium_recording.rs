//! Gallium's recording implementation (AC8 step 2): `crates/gallium/implementation/recording`, an implementation of
//! both traits that records every call with its arguments as the implementation received them (Debug text) and makes
//! a plain object for every object asked for, so the serving glue's tests can see what reached the implementation.
//!
//! Written by `api-crate-generator gallium` from the methods the trait layer wrote.

use std::fmt::Write as _;
use std::path::Path;

const HEADER: &str = "// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.\n";

/// One parameter text split at its top-level commas (`program: P, state: &S` is two).
fn split_params(params: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for text in params {
        let mut depth = 0_i32;
        let mut current = String::new();
        for character in text.chars() {
            match character {
                '<' | '(' | '[' => depth += 1,
                '>' | ')' | ']' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(std::mem::take(&mut current));
                    continue;
                }
                _ => {}
            }
            current.push(character);
        }
        out.push(current);
    }
    out.iter().filter_map(|param| param.trim().split_once(": ").map(|(name, ty)| (name.to_string(), ty.to_string()))).collect()
}

/// The value a recording method returns for a return type, or None where no rule makes one.
fn returned(ret: &str, params: &[(String, String)], screen: bool) -> Option<String> {
    let last = |text: &str| text.trim_end_matches('>').rsplit("::").next().unwrap_or_default().to_string();
    Some(match ret {
        "" => String::new(),
        "bool" => "true".to_string(),
        "Vec<u8>" => "b\"recording\".to_vec()".to_string(),
        "u8" | "u16" | "u32" | "u64" | "i32" | "i64" => "0".to_string(),
        "[f32; 2]" => "[0.5, 0.5]".to_string(),
        "Option<Self::Context>" if screen => "Some(RecordingContext { log: Arc::clone(&self.log) })".to_string(),
        values if values.starts_with("values::") => format!("{values}(0)"),
        mapped if mapped.starts_with("Option<Mapped<") => "Some(Mapped { mapping: self.log.made(\"Mapping\"), bytes: vec![0; MAPPED_BYTES], stride: 0, layer_stride: 0 })".to_string(),
        made if made.starts_with("Option<") => format!("Some(self.log.made(\"{}\"))", last(made)),
        turned if turned.starts_with("Result<") => {
            // A turn: the object taken by value, handed on as the turned kind (every kind but the context is `Object`).
            let (name, _) = params.iter().find(|(_, ty)| !ty.starts_with('&') && !ty.starts_with("Option<") && ty.contains("::"))?;
            format!("Ok({name})")
        }
        _ => return None,
    })
}

fn method(name: &str, params: &[String], ret: &str, screen: bool) -> Result<String, String> {
    let split = split_params(params);
    let value = returned(ret, &split, screen).ok_or_else(|| format!("recording: no value for {name}'s return {ret}"))?;
    let shown: Vec<String> = split.iter().map(|(param, ty)| if ty.contains("MesaCompiler") { format!("{param} <program>") } else { format!("{param} {{{param}:?}}") }).collect();
    let mut body = String::new();
    for (param, ty) in &split {
        if ty.contains("MesaCompiler") {
            let _ = writeln!(body, "        let _ = &{param};");
        }
    }
    if shown.is_empty() {
        let _ = writeln!(body, "        self.log.record(\"{name}\".to_string());");
    } else {
        let _ = writeln!(body, "        self.log.record(format!(\"{name} {}\"));", shown.join(" "));
    }
    if !value.is_empty() {
        let _ = writeln!(body, "        {value}");
    }
    let receiver = std::iter::once("&self".to_string()).chain(params.iter().cloned()).collect::<Vec<_>>().join(", ");
    let arrow = if ret.is_empty() { String::new() } else { format!(" -> {ret}") };
    Ok(format!("    fn {name}({receiver}){arrow} {{\n{body}    }}\n\n"))
}

/// Writes the recording crate from the methods (on the screen's trait or not, name, parameters, return).
pub fn generate(methods: &[(bool, String, Vec<String>, String)], crate_folder: &Path) -> Result<String, String> {
    let mut screen = String::new();
    let mut context = String::new();
    for (on_screen, name, params, ret) in methods {
        let text = method(name, params, ret, *on_screen)?;
        if *on_screen { screen.push_str(&text) } else { context.push_str(&text) }
    }
    let kinds: String = crate::gallium::KINDS
        .iter()
        .filter(|(kind, _)| *kind != "Screen" && *kind != "Context")
        .map(|(kind, _)| format!("    type {kind} = Object;\n"))
        .collect();
    let source = format!(
        "{HEADER}//! A Gallium implementation that records every call: each method writes its name and its arguments as it\n//! received them into the shared `Log`, and every object asked for is a plain `Object` with a number of its own.\n//! For the serving glue's tests, which read the log to see what reached the implementation.\n\nuse std::sync::atomic::{{AtomicUsize, Ordering}};\nuse std::sync::{{Arc, Mutex, PoisonError}};\n\nuse wcontainers_gallium::*;\n\n/// The bytes of every mapping the recording implementation lends.\nconst MAPPED_BYTES: usize = 64;\n\n/// What the implementation received, in order, and the numbers of the objects it made.\n#[derive(Debug, Default)]\npub struct Log {{\n    calls: Mutex<Vec<String>>,\n    made: AtomicUsize,\n}}\n\nimpl Log {{\n    /// Records one call.\n    pub fn record(&self, call: String) {{\n        self.calls.lock().unwrap_or_else(PoisonError::into_inner).push(call);\n    }}\n\n    /// Every call recorded so far.\n    pub fn calls(&self) -> Vec<String> {{\n        self.calls.lock().unwrap_or_else(PoisonError::into_inner).clone()\n    }}\n\n    /// A new object of a kind.\n    pub fn made(&self, kind: &'static str) -> Object {{\n        Object {{ kind, number: self.made.fetch_add(1, Ordering::Relaxed) }}\n    }}\n}}\n\n/// Every object the recording implementation makes, of any kind but the context.\n#[derive(Debug, PartialEq, Eq)]\npub struct Object {{\n    /// Its kind, as the trait names it.\n    pub kind: &'static str,\n    /// Its number, in the order made.\n    pub number: usize,\n}}\n\n/// The recording screen.\n#[derive(Debug, Default)]\npub struct Recording {{\n    /// What every call received.\n    pub log: Arc<Log>,\n}}\n\nimpl Recording {{\n    /// A screen recording into a new log.\n    pub fn new() -> Recording {{\n        Recording::default()\n    }}\n}}\n\n/// A recording context, recording into its screen's log.\n#[derive(Debug)]\npub struct RecordingContext {{\n    /// The screen's log.\n    pub log: Arc<Log>,\n}}\n\nimpl Gallium for Recording {{\n    type Compiler = wcontainers_mesa_compiler_direct::Bridge;\n    type Context = RecordingContext;\n{kinds}\n{screen}}}\n\nimpl GalliumContext for RecordingContext {{\n    type Screen = Recording;\n\n{context}}}\n"
    );
    let folder = crate_folder.join("implementation").join("recording");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    std::fs::write(folder.join("implementation.rs"), source).map_err(|error| error.to_string())?;
    std::fs::write(
        folder.join("Cargo.toml"),
        "[package]\nname = \"wcontainers-gallium-recording\"\ndescription = \"A Gallium implementation that records every call, for the serving glue's tests.\"\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\nlicense.workspace = true\nrepository.workspace = true\npublish = false\n\n[lib]\npath = \"implementation.rs\"\n\n[dependencies]\nwcontainers-gallium = { path = \"../..\" }\nwcontainers-mesa-compiler = { path = \"../../../mesa-compiler\" }\nwcontainers-mesa-compiler-direct = { path = \"../../../mesa-compiler/implementation/direct\" }\n\n[lints]\nworkspace = true\n",
    )
    .map_err(|error| error.to_string())?;
    Ok(format!("GALLIUM RECORDING: {} methods\n", methods.len()))
}
