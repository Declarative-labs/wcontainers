//! Narrows what each API's direct crate exports to what another crate of the set names.
//!
//! An implementation exports only what implements its interface (the linter's `public_outside_interface`); the glue
//! writes many items `pub` because the files of one crate name each other. After every crate of a set is written, an
//! item of a direct crate that no other crate names, and that no item kept public names in its signature, becomes
//! `pub(crate)`; one its own crate does not use stays as it is (it would be dead code).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Every identifier of `text`, comments included (a doc naming an item keeps it, which errs on the side of public).
fn identifiers(text: &str, into: &mut BTreeMap<String, usize>) {
    let mut word = String::new();
    for ch in text.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            word.push(ch);
        } else if !word.is_empty() {
            if !word.starts_with(|c: char| c.is_ascii_digit()) {
                *into.entry(std::mem::take(&mut word)).or_default() += 1;
            }
            word.clear();
        }
    }
}

fn rust_files(folder: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

/// The keywords an item's line has after `pub `, and the name after them.
const ITEMS: [&str; 10] = ["unsafe extern \"system\" fn ", "extern \"system\" fn ", "unsafe fn ", "const fn ", "fn ", "struct ", "enum ", "const ", "static ", "type "];

/// The item a line declares `pub`: its name, or a field's.
fn declared(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix("pub ")?;
    let after = ITEMS.iter().find_map(|keyword| rest.strip_prefix(keyword)).or_else(|| {
        // A field: `pub name: Type,`.
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        (!name.is_empty() && rest[name.len()..].starts_with(':')).then_some(rest)
    })?;
    let name: String = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    (!name.is_empty()).then_some(name)
}

/// The text of the declaration starting at line `at`: to its first `{` or `;` at no depth, or a struct's whole body.
fn signature(lines: &[&str], at: usize) -> String {
    let mut text = String::new();
    let whole_body = lines[at].contains("struct ");
    let mut depth = 0_i32;
    for line in &lines[at..] {
        text.push_str(line);
        text.push('\n');
        for ch in line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        let opened = line.contains('{');
        if (!whole_body && (opened || line.trim_end().ends_with(';'))) || (whole_body && depth <= 0 && (opened || line.contains('}') || line.trim_end().ends_with(';'))) {
            break;
        }
    }
    text
}

/// Narrows the direct crates of `folders` under `out` (a folder holding `crates/`); returns the report's line.
pub fn narrow(out: &Path, folders: &[&str]) -> Result<String, String> {
    let crates = out.join("crates");
    let mut all = Vec::new();
    rust_files(&crates, &mut all);
    let mut texts: BTreeMap<PathBuf, String> = BTreeMap::new();
    for path in &all {
        texts.insert(path.clone(), std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?);
    }
    let mut narrowed = 0;
    let mut kept = 0;
    for folder in folders {
        let direct = crates.join(folder).join("implementation").join("direct");
        // The crate's own files: the top of its folder only (tests, the harness and the C caller are crates of their own).
        let own: Vec<PathBuf> = texts.keys().filter(|path| path.parent() == Some(direct.as_path())).cloned().collect();
        if own.is_empty() {
            continue;
        }
        let mut others: BTreeMap<String, usize> = BTreeMap::new();
        let mut inside: BTreeMap<String, usize> = BTreeMap::new();
        for (path, text) in &texts {
            if own.contains(path) {
                // In its own crate, a use is code: a comment naming an item uses nothing.
                let code: String = text.lines().filter(|line| !line.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
                identifiers(&code, &mut inside);
            } else {
                identifiers(text, &mut others);
            }
        }
        // What stays public: what another crate names, and what a public item's declaration names, to a fixed point.
        let mut public: BTreeSet<String> = BTreeSet::new();
        let mut declarations: Vec<(String, String)> = Vec::new();
        for path in &own {
            let lines: Vec<&str> = texts[path].lines().collect();
            for (at, line) in lines.iter().enumerate() {
                if line.contains('$') {
                    continue;
                }
                if let Some(name) = declared(line) {
                    declarations.push((name, signature(&lines, at)));
                }
            }
        }
        for (name, text) in &declarations {
            // Used in its own crate only where it is declared (its doc, its impl blocks' headers aside): narrowed, it
            // would be dead code. An implementation's own types (`System…`) are what the crate exports.
            let mut own_text = BTreeMap::new();
            identifiers(text, &mut own_text);
            let elsewhere = inside.get(name).copied().unwrap_or(0).saturating_sub(own_text.get(name).copied().unwrap_or(0));
            // An implementation's own types (`System…`) are what the crate exports; a kind's table of interfaces is
            // what another API's glue binds.
            let implementation_type = name.starts_with("System") && text.contains("struct ") || name.ends_with("_interfaces");
            if others.contains_key(name) || elsewhere == 0 || implementation_type {
                public.insert(name.clone());
            }
        }
        loop {
            let before = public.len();
            for (name, text) in &declarations {
                if public.contains(name) {
                    let mut named = BTreeMap::new();
                    identifiers(text, &mut named);
                    for word in named.keys() {
                        if declarations.iter().any(|(declared, _)| declared == word) {
                            public.insert(word.clone());
                        }
                    }
                }
            }
            if public.len() == before {
                break;
            }
        }
        for path in &own {
            let text = &texts[path];
            let mut written = String::with_capacity(text.len());
            for line in text.lines() {
                match declared(line).filter(|_| !line.contains('$')) {
                    Some(name) if !public.contains(&name) => {
                        written.push_str(&line.replacen("pub ", "pub(crate) ", 1));
                        narrowed += 1;
                    }
                    Some(_) => {
                        written.push_str(line);
                        kept += 1;
                    }
                    None => written.push_str(line),
                }
                written.push('\n');
            }
            std::fs::write(path, written).map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(format!("VISIBILITY: {narrowed} items and fields of the direct crates narrowed to the crate, {kept} kept public (named by another crate or by a public item's declaration)\n"))
}
