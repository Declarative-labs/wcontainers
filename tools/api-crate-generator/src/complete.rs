//! Every entry of every table is a function, and no body the generator writes for an implementation panics: checked on
//! every set written, from the files written. A method the glue does not carry is refused (src/glue.rs `refusal`), never
//! left null for C to jump to address 0, and never answered by `unimplemented!` for safe code to reach.

use std::path::{Path, PathBuf};

/// What a generated implementation's body may not hold: each panics when reached.
const PANICS: [&str; 4] = ["unimplemented!(", "todo!(", "panic!(", "unreachable!("];

/// The implementations whose bodies are checked: the glue and the recorder. The hostile implementation panics on
/// purpose (its lies), and tests assert.
const CHECKED: [&str; 2] = ["direct", "recorder"];

/// One finding in a file's text: a null entry or a panicking body.
fn findings_in(name: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut in_table = false;
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        for panic in PANICS {
            if trimmed.contains(panic) {
                found.push(format!("PANIC BODY {name}:{}: {trimmed}", number + 1));
            }
        }
        if trimmed.ends_with(": Absent,") {
            found.push(format!("NULL ENTRY {name}:{}: {trimmed}", number + 1));
        }
        // A table's constant: every field a function, none `None`.
        if trimmed.starts_with("pub const ") && trimmed.contains("Table = &") {
            in_table = true;
        } else if in_table && trimmed.starts_with("};") {
            in_table = false;
        } else if in_table && trimmed.ends_with(": None,") {
            found.push(format!("NULL ENTRY {name}:{}: {trimmed}", number + 1));
        }
    }
    found
}

fn rust_files(folder: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if !matches!(name, "harness" | "tests" | "c-caller") {
                rust_files(&path, into);
            }
        } else if name.ends_with(".rs") && !name.ends_with("_tests.rs") {
            into.push(path);
        }
    }
}

/// Checks every direct and recorder crate written under `out`, and the generator's own failures (what it could not write
/// without a null entry or a panic); returns the report's lines, or the findings as the failure.
pub fn check(out: &Path, failures: &[String]) -> Result<String, String> {
    let mut files = Vec::new();
    let crates = out.join("crates");
    for entry in std::fs::read_dir(&crates).map_err(|error| format!("{}: {error}", crates.display()))?.flatten() {
        for implementation in CHECKED {
            rust_files(&entry.path().join("implementation").join(implementation), &mut files);
        }
    }
    files.sort();
    let mut found: Vec<String> = failures.to_vec();
    for file in &files {
        let text = std::fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))?;
        let name = file.strip_prefix(out).unwrap_or(file).display().to_string();
        found.extend(findings_in(&name, &text));
    }
    if found.is_empty() {
        Ok(format!("COMPLETE: {} files of the direct and recorder crates; null entries 0, panicking bodies 0\n", files.len()))
    } else {
        Err(format!("COMPLETE FAILED: {} findings\n{}\n", found.len(), found.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::findings_in;

    #[test]
    fn a_null_entry_fails() {
        let table = "impl<V: Device> Tables<V> {\n    pub const DEVICE: &'static DeviceTable = &DeviceTable {\n        unknown: unknown_table::<V>(),\n        device_get_private_data: None,\n    };\n}\n";
        assert_eq!(findings_in("serve.rs", table).len(), 1);
        assert_eq!(findings_in("abi.rs", "pub struct DeviceTable {\n    pub device_get_private_data: Absent,\n}\n").len(), 1);
    }

    #[test]
    fn a_panicking_body_fails() {
        let body = "    fn get_crypto_type(&self) -> Guid {\n        unimplemented!(\"not glued\")\n    }\n";
        assert_eq!(findings_in("call.rs", body).len(), 1);
        assert_eq!(findings_in("call.rs", "        todo!()\n        panic!(\"x\")\n        unreachable!()\n").len(), 3);
    }

    #[test]
    fn refused_entries_and_none_outside_a_table_pass() {
        let text = "    pub const DEVICE: &'static DeviceTable = &DeviceTable {\n        device_get_private_data: device_get_private_data_refused,\n    };\n    let x = Foo { a: None, };\n    // unimplemented!( in a comment\n    Foo {\n        b: None,\n    }\n";
        assert!(findings_in("serve.rs", text).is_empty());
    }
}
