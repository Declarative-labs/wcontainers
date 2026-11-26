//! What the marked code of the direct crates assumes of the safe traits the other party implements.
//!
//! A safe trait promises nothing its signature does not hold: a caller's `VolatileBytes` may answer a different `len`
//! on each call, and an implementation's `Clone` may panic. `OTHER_PARTY` lists every safe trait whose implementation
//! comes from the caller of the calling glue or from the implementation behind the serving glue, each method the glue
//! calls, and what the glue's marks assume of its answers (nothing but what the type holds). `check` reads every Rust
//! file of the direct crates written and fails the generator when one function asks one value the same method of such
//! a trait twice: the glue asks once and carries the answer.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Who writes a trait's implementation that the glue meets.
#[derive(Clone, Copy, Debug)]
pub enum Party {
    /// Safe code calling the calling glue.
    Caller,
    /// The implementation the serving glue serves.
    Implementation,
    /// Neither: only generated glue can implement it (an `unsafe fn` in it, which `forbid` refuses elsewhere), or the
    /// glue implements it on its own types.
    GlueOnly,
}

/// One safe trait the glue meets, the methods it calls, and how the marked code is kept from relying on its answers.
pub struct Trusted {
    pub name: &'static str,
    pub party: Party,
    /// The methods of it the glue calls, as one string.
    pub called: &'static str,
    /// What the glue holds, whatever the answers: asked once per value and operation, consistency with another method
    /// never assumed, a panic, a re-entrant call or a call that never returns leaving no table or lent buffer in a
    /// state a mark relies on.
    pub held: &'static str,
}

/// Every safe trait of the other party the glue meets.
pub const OTHER_PARTY: &[Trusted] = &[
    Trusted {
        name: "VolatileBytes (the caller's bytes, out)",
        party: Party::Caller,
        called: "len once, write once",
        held: "len asked once before any step of the call; that answer sizes our buffer and is the count the system is told; the system writes only our buffer; write runs last, after every object is adopted, and its answer is not used; no lock or reservation is held while either runs",
    },
    Trusted {
        name: "VolatileBytes (an implementation's Mapped, serving Map and Unmap)",
        party: Party::Implementation,
        called: "len once at Map, read_to once at Map, write_from once at Unmap",
        held: "the shadow lent to C is ours, sized by the recorded description and pitches and no longer than the one len; read_to and write_from reach only sinks and sources of ours bounded by the shadow; the subresource is reserved, not locked, while they run, and a reservation dropped by a panic frees it",
    },
    Trusted {
        name: "every generated API trait (the implementation's methods)",
        party: Party::Implementation,
        called: "each method once per call from C",
        held: "descriptions, kinds and sizes answered from what the glue recorded, never asked again; a method runs under no lock of the glue, with the object held by an Arc; a panic is caught (guarded) and answered E_FAIL; a re-entrant call finds the table as any other call does",
    },
    Trusted {
        name: "Clone, PartialEq, Eq, Hash of an implementation's objects",
        party: Party::Implementation,
        called: "never",
        held: "the glue keys its tables by the address of its own words and by role; an object handed out again comes from the record",
    },
    Trusted {
        name: "Drop of an implementation's objects and Mapped",
        party: Party::Implementation,
        called: "when the last reference goes",
        held: "dropped after every lock, inside guarded: a panic is caught and its payload kept, a re-entrant call finds no lock held",
    },
    Trusted {
        name: "slices and Options of plain data and of the calling glue's objects",
        party: Party::GlueOnly,
        called: "len, iter, to_vec",
        held: "a slice's length is the language's; the elements are the generated plain types and the glue's own System types, whose Clone and Hash the glue writes",
    },
    Trusted {
        name: "Adopt",
        party: Party::GlueOnly,
        called: "adopt, INTERFACE",
        held: "adopt is an unsafe fn, which no crate under forbid(unsafe_code) can write; the glue calls it only on its own System types",
    },
    Trusted {
        name: "Deref, AsRef, Borrow, Ord of the caller's types",
        party: Party::Caller,
        called: "never",
        held: "no trait signature takes a type bound by them; Found's Deref is the glue's own",
    },
    Trusted {
        name: "Iterator of the caller's types",
        party: Party::Caller,
        called: "never",
        held: "no trait signature takes an iterator",
    },
];

/// The methods of `VolatileBytes` the glue may call, and the glue's helpers that call one on their argument.
const METHODS: [&str; 5] = ["len", "read", "write", "read_to", "write_from"];
const HELPERS: [(&str, &str); 2] = [(".fill_from(", "read_to"), (".empty_into(", "write_from")];

/// One function: its name, its parameters by name and type, and its body, whitespace collapsed.
struct Function {
    name: String,
    parameters: Vec<(String, String)>,
    body: String,
}

/// Returns the position just after the bracket closing the one at `open`, skipping string literals.
fn closing(text: &[u8], open: usize) -> Option<usize> {
    let (opening, close) = match text[open] {
        b'(' => (b'(', b')'),
        b'{' => (b'{', b'}'),
        _ => return None,
    };
    let mut depth = 0_usize;
    let mut at = open;
    while at < text.len() {
        match text[at] {
            b'"' => {
                at += 1;
                while at < text.len() && text[at] != b'"' {
                    at += if text[at] == b'\\' { 2 } else { 1 };
                }
            }
            byte if byte == opening => depth += 1,
            byte if byte == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(at + 1);
                }
            }
            _ => {}
        }
        at += 1;
    }
    None
}

/// Splits a parameter list at its top-level commas into names and types.
fn parameters(list: &str) -> Vec<(String, String)> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0_i32, 0);
    for (at, ch) in list.char_indices() {
        match ch {
            '(' | '<' | '[' | '{' => depth += 1,
            ')' | '>' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&list[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&list[start..]);
    parts
        .into_iter()
        .filter_map(|part| part.split_once(':').map(|(name, ty)| (name.trim().trim_start_matches("mut ").to_string(), ty.trim().to_string())))
        .collect()
}

/// Every function of `text` that has a body.
fn functions(text: &str) -> Vec<Function> {
    let flat: String = without_comments(text).split_whitespace().collect::<Vec<_>>().join(" ");
    let bytes = flat.as_bytes();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = flat[from..].find("fn ").map(|at| at + from) {
        from = at + 3;
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        let name: String = flat[at + 3..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        let Some(open) = flat[at..].find('(').map(|o| o + at) else { break };
        let Some(close) = closing(bytes, open) else { continue };
        // The body opens at the first `{` after the parameters at no depth of `<`; a `;` first means no body.
        let rest = &flat[close..];
        let Some(brace) = rest.find('{').map(|b| b + close) else { continue };
        if rest.find(';').is_some_and(|semi| semi + close < brace) {
            continue;
        }
        let Some(end) = closing(bytes, brace) else { continue };
        found.push(Function { name, parameters: parameters(&flat[open + 1..close - 1]), body: flat[brace..end].to_string() });
    }
    found
}

/// Whether `word` stands alone at `at` in `text`: no identifier character or `.` before it, none after it.
fn alone(text: &str, at: usize, word: &str) -> bool {
    let before = text[..at].chars().next_back();
    let after = text[at + word.len()..].chars().next();
    !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The trait's methods called on the value named at `at`: directly (`name.len()`), on a closure's binding of it
/// (`name.map_or(0, |bytes| bytes.len())`), or on a binding of `if let Some(bytes) = name { .. }`.
fn on_value(body: &str, at: usize, name: &str) -> Vec<&'static str> {
    let bytes = body.as_bytes();
    let after = &body[at + name.len()..];
    let mut found = Vec::new();
    // Directly, or through a combinator whose closure binds it.
    if let Some(rest) = after.strip_prefix('.') {
        let method: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if let Some(hit) = METHODS.iter().find(|m| **m == method) {
            if rest[method.len()..].starts_with('(') {
                found.push(*hit);
            }
            return found;
        }
        let open = at + name.len() + 1 + method.len();
        if bytes.get(open) == Some(&b'(')
            && let Some(close) = closing(bytes, open)
        {
            let inside = &body[open + 1..close - 1];
            if let Some(bar) = inside.find('|') {
                let binding: String = inside[bar + 1..].chars().take_while(|c| *c != '|').collect();
                found.extend(calls_on(inside, binding.trim()));
            }
        }
        return found;
    }
    // A binding of `if let Some(binding) = name { .. }`.
    let before = body[..at].trim_end();
    if let Some(head) = before.strip_suffix('=').map(str::trim_end).and_then(|head| head.strip_suffix(')'))
        && let Some(open) = head.rfind("Some(")
    {
        let binding = head[open + "Some(".len()..].trim();
        let brace = at + name.len() + after.find('{').unwrap_or(after.len());
        if after.trim_start().starts_with('{')
            && let Some(close) = closing(bytes, brace)
        {
            found.extend(calls_on(&body[brace..close], binding));
        }
    }
    found
}

/// The trait's methods called on `binding` in `text`.
fn calls_on(text: &str, binding: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    if binding.is_empty() {
        return found;
    }
    for method in METHODS {
        let call = format!("{binding}.{method}(");
        let mut from = 0;
        while let Some(at) = text[from..].find(&call).map(|at| at + from) {
            from = at + call.len();
            if alone(text, at, binding) {
                found.push(method);
            }
        }
    }
    found
}

/// The text without its line comments (`//` outside a string literal) and with every string's contents blanked, so that
/// neither a comment's nor a string's names and brackets count.
fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut in_string = false;
        let mut escaped = false;
        let mut cut = line.len();
        let mut blank: Vec<usize> = Vec::new();
        let chars: Vec<(usize, char)> = line.char_indices().collect();
        for (index, (at, ch)) in chars.iter().enumerate() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if *ch == '\\' {
                    escaped = true;
                } else if *ch == '"' {
                    in_string = false;
                }
                // A string's contents are no code: blanked, so that neither a name nor a bracket in it counts.
                if in_string || escaped {
                    blank.push(*at);
                }
            } else if *ch == '"' {
                in_string = true;
            } else if *ch == '/' && chars.get(index + 1).is_some_and(|(_, next)| *next == '/') {
                cut = *at;
                break;
            }
        }
        let kept: String = line[..cut].char_indices().map(|(at, ch)| if blank.contains(&at) { ' ' } else { ch }).collect();
        out.push_str(&kept);
        out.push('\n');
    }
    out
}

/// The calls of the trait's methods per receiver in one function: on a parameter whose type names `VolatileBytes` (as
/// `on_value` finds them), and on any expression passed to `VolatileBytes::m` or to a
/// helper of the glue that calls one.
fn calls(function: &Function) -> BTreeMap<(String, &'static str), usize> {
    let mut counted: BTreeMap<(String, &'static str), usize> = BTreeMap::new();
    let body = function.body.as_str();
    for (name, ty) in &function.parameters {
        if !ty.contains("VolatileBytes") {
            continue;
        }
        let mut from = 0;
        while let Some(at) = body[from..].find(name.as_str()).map(|at| at + from) {
            from = at + name.len();
            if !alone(body, at, name) {
                continue;
            }
            for method in on_value(body, at, name) {
                *counted.entry((name.clone(), method)).or_default() += 1;
            }
        }
    }
    let bytes = body.as_bytes();
    let mut forms: Vec<(String, &'static str)> = METHODS.iter().map(|method| (format!("VolatileBytes::{method}("), *method)).collect();
    forms.extend(HELPERS.iter().map(|(helper, method)| ((*helper).to_string(), *method)));
    for (form, method) in &forms {
        let mut from = 0;
        while let Some(at) = body[from..].find(form.as_str()).map(|at| at + from) {
            from = at + form.len();
            let open = at + form.len() - 1;
            let Some(close) = closing(bytes, open) else { continue };
            let receiver = body[open + 1..close - 1].trim().trim_start_matches('&').trim().to_string();
            *counted.entry((receiver, *method)).or_default() += 1;
        }
    }
    counted
}

/// The functions of `text` that ask one value the same method twice, as failure lines, and how many calls were seen.
fn twice(file: &str, text: &str) -> (Vec<String>, usize) {
    let mut failures = Vec::new();
    let mut seen = 0;
    for function in functions(text) {
        for ((receiver, method), count) in calls(&function) {
            seen += count;
            if count > 1 {
                failures.push(format!("ASKED TWICE: {file}: fn {} calls `{receiver}`'s {method} {count} times: the other party's answer may change between calls", function.name));
            }
        }
    }
    (failures, seen)
}

fn direct_files(folder: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            // The harness's tests and stand-ins are no glue.
            if !path.ends_with("harness") && !path.ends_with("tests") && !path.ends_with("recorder") {
                direct_files(&path, files);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") && path.components().any(|part| part.as_os_str() == "direct") {
            files.push(path);
        }
    }
}

/// Checks every direct crate written under `out` (a folder holding `crates/`), and returns the report's section: the
/// list, then the calls seen.
pub fn check(out: &Path) -> Result<String, String> {
    let mut report = String::from("OTHER PARTY (safe traits the glue meets; what the marked code holds whatever they answer):\n");
    for trusted in OTHER_PARTY {
        let _ = writeln!(report, "  {} [{:?}]: calls {}; {}", trusted.name, trusted.party, trusted.called, trusted.held);
    }
    let mut files = Vec::new();
    direct_files(&out.join("crates"), &mut files);
    let mut failures = Vec::new();
    let mut seen = 0;
    for path in &files {
        let text = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let (found, calls) = twice(&path.display().to_string(), &text);
        failures.extend(found);
        seen += calls;
    }
    let _ = writeln!(report, "ASKED ONCE: {} files of the direct crates, {seen} calls of the other party's VolatileBytes, {} asked twice", files.len(), failures.len());
    if failures.is_empty() { Ok(report) } else { Err(format!("{report}{}", failures.join("\n"))) }
}

#[cfg(test)]
mod tests {
    use super::twice;

    #[test]
    fn a_length_asked_twice_of_the_callers_bytes_fails() {
        // The form the check of 90b14f72a found: the buffer sized by one answer, the system told the other.
        let text = "fn get_data(&self, data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>) -> Result<(), HResult> {
            let Ok(data_length) = usize::try_from(data.map_or(0, |bytes| bytes.len())) else { return Err(E_INVALIDARG); };
            let mut data_buffer = Vec::new();
            let Ok(data_size) = u32::try_from(data.map_or(0, |bytes| bytes.len())) else { return Err(E_INVALIDARG); };
            if let Some(bytes) = data { bytes.write(&data_buffer); }
            Ok(())
        }";
        let (failures, _) = twice("t.rs", text);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].contains("`data`'s len 2 times"), "{failures:?}");
    }

    #[test]
    fn a_length_asked_once_and_carried_passes() {
        let text = "fn get_data(&self, data: Option<&impl VolatileBytes>) -> Result<(), HResult> {
            let Ok(data_length) = usize::try_from(data.map_or(0, |bytes| bytes.len())) else { return Err(E_INVALIDARG); };
            let Ok(data_size) = u32::try_from(data_length) else { return Err(E_INVALIDARG); };
            let _ = \"data.len() in a string is no call {\";
            if let Some(bytes) = data { bytes.write(&data_buffer); }
            Ok(())
        }";
        let (failures, seen) = twice("t.rs", text);
        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(seen, 2);
    }

    #[test]
    fn an_implementations_bytes_asked_twice_through_the_trait_path_fails() {
        let text = "pub fn map<V>(found: V) { let a = VolatileBytes::len(&value.data); let b = wcontainers_volatile_bytes::VolatileBytes::len(&value.data); shadow.fill_from(&value.data); }";
        let (failures, _) = twice("t.rs", text);
        assert_eq!(failures.len(), 1, "{failures:?}");
    }

    #[test]
    fn another_arguments_length_in_the_same_statement_is_not_the_bytes() {
        let text = "fn f(&self, data: &impl VolatileBytes, views: &[u32]) { let Ok(data_length) = usize::try_from(data.len()) else { return; }; let r = call(if true { data_buffer.as_mut_ptr() } else { null() }, views.len(), data.write(&b)); }";
        let (failures, seen) = twice("t.rs", text);
        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(seen, 2);
    }

    #[test]
    fn a_helper_that_reads_twice_fails() {
        let text = "fn fill(&self, source: &impl VolatileBytes) { let _ = source.read_to(&mut a); let _ = source.read_to(&mut b); }";
        assert_eq!(twice("t.rs", text).0.len(), 1);
    }
}
