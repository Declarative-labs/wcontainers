//! Gallium's declared values (AC8, the implementations track's "values"): every enumeration of the headers read whose
//! name starts with `pipe_` (and `mesa_shader_stage`, `mesa_prim`), each a type of its own holding its enumerators as
//! associated constants under their C names; the enumerators of the anonymous enumerations and the `PIPE_*` defines
//! with a number for a value, as constants of their own. Each value is the generator's reading of the header's
//! expression; the direct crate's test compares every one with C's own (flat.c's `wc_value`).
//!
//! A field or a parameter that C declares with one of these enumerations takes its type in the trait.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::cheader::Header;

/// Enumerations kept besides those named `pipe_*`.
const ALSO: &[&str] = &["mesa_shader_stage", "mesa_prim"];

/// One enumeration: its C name, its Rust type, the Rust number its values fit, and its enumerators with their values.
pub struct Enumeration {
    pub c: String,
    pub rust: String,
    pub number: &'static str,
    pub values: Vec<(String, i64)>,
}

/// Every value written: the enumerations, and the loose constants (anonymous enumerations' enumerators and defines).
pub struct Values {
    pub enumerations: Vec<Enumeration>,
    pub loose: Vec<(String, i64)>,
    /// Every enumerator and define evaluated, whatever its name (for array lengths).
    pub known: BTreeMap<String, i64>,
}

impl Values {
    /// The type of each field whose comment names its values' prefix (`Header::field_families`): the one enumeration
    /// every enumerator of which has that prefix; a prefix no enumeration or several have types nothing.
    pub fn field_types(&self, families: &[(String, String, String)]) -> std::collections::BTreeMap<(String, String), (String, &'static str)> {
        let mut out = std::collections::BTreeMap::new();
        for (structure, field, prefix) in families {
            let matching: Vec<&Enumeration> = self.enumerations.iter().filter(|enumeration| !enumeration.values.is_empty() && enumeration.values.iter().all(|(name, _)| name.starts_with(prefix.as_str()))).collect();
            if let [enumeration] = matching.as_slice() {
                out.insert((structure.clone(), field.clone()), (format!("values::{}", enumeration.rust), enumeration.number));
            }
        }
        out
    }

    /// Each enumeration's C name with its type in the trait and its number, for `Header::value_types`.
    pub fn types(&self) -> std::collections::BTreeMap<String, (String, &'static str)> {
        self.enumerations.iter().map(|enumeration| (enumeration.c.clone(), (format!("values::{}", enumeration.rust), enumeration.number))).collect()
    }
}

/// Reads a C integer expression of the subset the headers use: numbers (decimal, hex, with u, l suffixes), names of
/// enumerators and defines, parentheses, unary `-` and `~`, and `* + - << >> & ^ |`.
struct Expression<'a> {
    tokens: Vec<String>,
    at: usize,
    names: &'a dyn Fn(&str) -> Option<i64>,
}

impl Expression<'_> {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.at).map(String::as_str)
    }

    fn next(&mut self) -> Option<String> {
        let token = self.tokens.get(self.at).cloned();
        self.at += 1;
        token
    }

    fn primary(&mut self) -> Option<i64> {
        let token = self.next()?;
        match token.as_str() {
            "(" => {
                let value = self.or()?;
                (self.next()? == ")").then_some(value)
            }
            "-" => self.primary().map(|value| -value),
            "~" => self.primary().map(|value| !value),
            _ if token.starts_with(|c: char| c.is_ascii_digit()) => {
                let digits = token.trim_end_matches(['u', 'U', 'l', 'L']);
                match digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
                    Some(hex) => i64::from_str_radix(hex, 16).ok(),
                    None => digits.parse().ok(),
                }
            }
            _ => (self.names)(&token),
        }
    }

    fn binary(&mut self, level: usize) -> Option<i64> {
        const LEVELS: [&[&str]; 6] = [&["|"], &["^"], &["&"], &["<<", ">>"], &["+", "-"], &["*"]];
        if level == LEVELS.len() {
            return self.primary();
        }
        let mut value = self.binary(level + 1)?;
        while let Some(operator) = self.peek().filter(|token| LEVELS[level].contains(token)).map(str::to_string) {
            self.at += 1;
            let right = self.binary(level + 1)?;
            value = match operator.as_str() {
                "|" => value | right,
                "^" => value ^ right,
                "&" => value & right,
                "<<" => value.checked_shl(u32::try_from(right).ok()?)?,
                ">>" => value.checked_shr(u32::try_from(right).ok()?)?,
                "+" => value.checked_add(right)?,
                "-" => value.checked_sub(right)?,
                _ => value.checked_mul(right)?,
            };
        }
        Some(value)
    }

    fn or(&mut self) -> Option<i64> {
        self.binary(0)
    }
}

fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let characters: Vec<char> = text.chars().collect();
    let mut at = 0;
    while at < characters.len() {
        let c = characters[at];
        if c.is_whitespace() {
            at += 1;
        } else if c.is_ascii_alphanumeric() || c == '_' {
            let start = at;
            while at < characters.len() && (characters[at].is_ascii_alphanumeric() || characters[at] == '_') {
                at += 1;
            }
            out.push(characters[start..at].iter().collect());
        } else if (c == '<' || c == '>') && characters.get(at + 1) == Some(&c) {
            out.push(format!("{c}{c}"));
            at += 2;
        } else {
            out.push(c.to_string());
            at += 1;
        }
    }
    out
}

/// Evaluates `text` with `names` for the names it uses; None where any part is outside the subset.
fn evaluate(text: &str, names: &dyn Fn(&str) -> Option<i64>) -> Option<i64> {
    let mut expression = Expression { tokens: tokens(text), at: 0, names };
    let value = expression.or()?;
    (expression.at == expression.tokens.len()).then_some(value)
}

/// `pipe_compare_func` to `CompareFunc`, `mesa_shader_stage` to `MesaShaderStage`.
fn rust_name(c: &str) -> String {
    c.trim_start_matches("pipe_")
        .split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map(|first| first.to_ascii_uppercase().to_string() + letters.as_str()).unwrap_or_default()
        })
        .collect()
}

/// Every value of the headers, evaluated; the names whose expression is outside the subset are listed in the error.
pub fn collect(header: &Header) -> Result<Values, String> {
    // Every enumerator's value first (each may name an earlier one or a define), then the defines on demand.
    let mut known: BTreeMap<String, i64> = BTreeMap::new();
    let mut failures = Vec::new();
    let define = |name: &str, known: &BTreeMap<String, i64>| -> Option<i64> {
        fn resolve(name: &str, header: &Header, known: &BTreeMap<String, i64>, depth: usize) -> Option<i64> {
            if let Some(value) = known.get(name) {
                return Some(*value);
            }
            let text = header.define_texts.get(name)?;
            if depth > 16 {
                return None;
            }
            evaluate(text, &|inner| resolve(inner, header, known, depth + 1))
        }
        resolve(name, header, known, 0)
    };
    let mut enumerations = Vec::new();
    let mut loose = Vec::new();
    for (name, enumerators) in &header.enumerators {
        let kept = name.starts_with("pipe_") || ALSO.contains(&name.as_str());
        let anonymous = name.starts_with("anonymous ");
        let mut values = Vec::new();
        let mut last: Option<i64> = None;
        for (enumerator, expression) in enumerators {
            let value = match expression {
                Some(text) => evaluate(text, &|inner| define(inner, &known)),
                None => Some(last.map_or(0, |last| last + 1)),
            };
            let Some(value) = value else {
                if kept || (anonymous && enumerator.starts_with("PIPE_")) {
                    failures.push(format!("{name}.{enumerator} = {expression:?}"));
                }
                break;
            };
            known.insert(enumerator.clone(), value);
            last = Some(value);
            values.push((enumerator.clone(), value));
        }
        if kept {
            let number = if values.iter().any(|(_, value)| *value < 0) { "i32" } else { "u32" };
            enumerations.push(Enumeration { c: name.clone(), rust: rust_name(name), number, values });
        } else if anonymous {
            loose.extend(values.into_iter().filter(|(enumerator, _)| enumerator.starts_with("PIPE_")));
        }
    }
    for name in header.define_texts.keys().filter(|name| name.starts_with("PIPE_")) {
        if let Some(value) = define(name, &known) {
            loose.push((name.clone(), value));
        }
    }
    // A name both an anonymous enumerator and a define, or defined twice, is written once; Rust's name of each is
    // its C name in upper case (`PIPE_FORMAT_ASTC_4x4` is `PIPE_FORMAT_ASTC_4X4`), which must name one value.
    let mut seen = std::collections::BTreeSet::new();
    loose.retain(|(name, _)| seen.insert(name.clone()));
    for enumeration in &enumerations {
        let mut upper = std::collections::BTreeSet::new();
        for (name, _) in &enumeration.values {
            if !upper.insert(name.to_uppercase()) {
                failures.push(format!("{}.{name}: another enumerator has its name in upper case", enumeration.c));
            }
        }
    }
    for name in header.define_texts.keys() {
        if let Some(value) = define(name, &known) {
            known.insert(name.clone(), value);
        }
    }
    if failures.is_empty() { Ok(Values { enumerations, loose, known }) } else { Err(format!("GALLIUM VALUES FAILED {}:\n  {}", failures.len(), failures.join("\n  "))) }
}

/// `crates/gallium/values.rs`.
pub fn rust(values: &Values, header_line: &str) -> String {
    let mut text = String::from(header_line);
    text.push_str("//! Gallium's declared values, under their C names: each enumeration of Mesa's headers a type holding its\n//! enumerators, and the anonymous enumerations' enumerators and the `PIPE_*` defines as constants.\n\n");
    for enumeration in &values.enumerations {
        let _ = writeln!(text, "/// Mesa's `{}`.\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\npub struct {}(pub {});\n", enumeration.c, enumeration.rust, enumeration.number);
        if enumeration.values.is_empty() {
            continue;
        }
        let _ = writeln!(text, "impl {} {{", enumeration.rust);
        for (name, value) in &enumeration.values {
            let _ = writeln!(text, "    /// `{name}`.\n    pub const {}: {} = {}({value});", name.to_uppercase(), enumeration.rust, enumeration.rust);
        }
        let _ = writeln!(text, "}}\n");
    }
    for (name, value) in &values.loose {
        let number = if *value < 0 { "i64" } else if *value > i64::from(u32::MAX) { "u64" } else { "u32" };
        let _ = writeln!(text, "/// `{name}`.\npub const {}: {number} = {value};", name.to_uppercase());
    }
    text
}

/// flat.c's `wc_value`: each value as C computes it, in the order of `rust_list`.
pub fn c_cases(values: &Values) -> String {
    let mut text = String::new();
    let names = values.enumerations.iter().flat_map(|enumeration| enumeration.values.iter().map(|(name, _)| name)).chain(values.loose.iter().map(|(name, _)| name));
    for (at, name) in names.enumerate() {
        let _ = writeln!(text, "        case {at}: return (int64_t){name};");
    }
    text
}

/// flat.rs's `values()`: each value as the trait gives it, with its name.
pub fn rust_list(values: &Values) -> String {
    let mut text = String::new();
    for enumeration in &values.enumerations {
        for (name, _) in &enumeration.values {
            let _ = writeln!(text, "        (\"{name}\", i64::from(wcontainers_gallium::values::{}::{}.0)),", enumeration.rust, name.to_uppercase());
        }
    }
    for (name, value) in &values.loose {
        let rust = name.to_uppercase();
        let conversion = if *value > i64::from(u32::MAX) { format!("i64::try_from(wcontainers_gallium::values::{rust}).unwrap_or(i64::MAX)") } else { format!("i64::from(wcontainers_gallium::values::{rust})") };
        let _ = writeln!(text, "        (\"{name}\", {conversion}),");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::evaluate;

    #[test]
    fn expressions_of_the_headers() {
        let names = |name: &str| match name {
            "PIPE_FACE_FRONT" => Some(1),
            "PIPE_FACE_BACK" => Some(2),
            _ => None,
        };
        assert_eq!(evaluate("(PIPE_FACE_FRONT | PIPE_FACE_BACK)", &names), Some(3));
        assert_eq!(evaluate("(1 << 4)", &names), Some(16));
        assert_eq!(evaluate("0x10u", &names), Some(16));
        assert_eq!(evaluate("-1", &names), Some(-1));
        assert_eq!(evaluate("~0 & 0xff", &names), Some(255));
        assert_eq!(evaluate("1 + 2 * 3", &names), Some(7));
        assert_eq!(evaluate("sizeof(int)", &names), None);
    }
}
