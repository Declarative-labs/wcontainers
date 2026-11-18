//! A pattern search of shell and Python scripts for every number they write (outside comments and
//! shell syntax), with the constants of the same value named in a note.

use std::io::Read;
use std::path::Path;

use crate::eval::Value;
use crate::findings::{Finding, Kind, List, MAX_LITERAL_CHARS, clean_literal};
use crate::text::{DECIMAL_POINT, WORD_JOINER};

/// File extensions of the scripts searched; files without an extension are searched when they start
/// with [`SHEBANG`].
pub const SCRIPT_EXTENSIONS: &[&str] = &["sh", "bash", "py"];
pub const SHEBANG: &str = "#!";
/// Starts a comment in both shell and Python when it begins a word.
const COMMENT_MARK: char = '#';
const QUOTES: &[char] = &['\'', '"'];
/// Shell syntax that holds digits but no value: a one-digit descriptor redirection (`2>`, `>&2`), a positional
/// parameter or field (`$1`, `${2}`), and a character class (`[0-9]`).
const REDIRECTIONS: &[char] = &['<', '>'];
const DUPLICATE: char = '&';
const PARAMETER: char = '$';
const PARAMETER_BRACE: char = '{';
const CLASS_OPEN: char = '[';
const CLASS_CLOSE: char = ']';
const CLASS_RANGE: char = '-';
/// Starts a script finding's item, which is the text of its line, so that a number keeps its key when
/// other lines change and loses it when its own line does.
const LINE_ITEM_PREFIX: &str = "line: ";

/// Digits that are shell syntax, not a value.
fn syntax(chars: &[char], start: usize, end: usize) -> bool {
    let at = |i: Option<usize>| i.and_then(|i| chars.get(i)).copied();
    let previous = start.checked_sub(1);
    let before = at(previous);
    let before2 = at(previous.and_then(|p| p.checked_sub(1)));
    let after = chars.get(end).copied();
    // A descriptor in a redirection is one digit: `4096>size` and `$((4096>>2))` compare and shift.
    let descriptor = end == start + 1;
    let redirected = descriptor
        && (after.is_some_and(|c| REDIRECTIONS.contains(&c))
            || (before == Some(DUPLICATE) && before2.is_some_and(|c| REDIRECTIONS.contains(&c))));
    let parameter = before == Some(PARAMETER)
        || (before == Some(PARAMETER_BRACE) && before2 == Some(PARAMETER));
    let class = (before == Some(CLASS_OPEN) && after == Some(CLASS_RANGE))
        || (before == Some(CLASS_RANGE) && after == Some(CLASS_CLOSE));
    redirected || parameter || class
}

/// Every integer written in the script outside comments and shell syntax, with its 1-based line and
/// column.
pub fn numbers(text: &str) -> Vec<(usize, usize, String, i128)> {
    let mut out = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut quote: Option<char> = None;
        let mut i = usize::default();
        while i < chars.len() {
            let c = chars[i];
            let prev = i.checked_sub(1).map(|p| chars[p]);
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
            } else if QUOTES.contains(&c) {
                quote = Some(c);
            } else if c == COMMENT_MARK && prev.is_none_or(char::is_whitespace) {
                break;
            }
            let starts_word = prev
                .is_none_or(|p| !(p.is_alphanumeric() || p == WORD_JOINER || p == DECIMAL_POINT));
            if !(c.is_ascii_digit() && starts_word) {
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == WORD_JOINER) {
                i += 1;
            }
            let fraction = chars.get(i) == Some(&DECIMAL_POINT)
                && chars.get(i + 1).is_some_and(char::is_ascii_digit);
            if fraction {
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == DECIMAL_POINT) {
                    i += 1;
                }
                continue;
            }
            if syntax(&chars, start, i) {
                continue;
            }
            let token: String = chars[start..i].iter().collect();
            let value = syn::parse_str::<syn::LitInt>(&token)
                .ok()
                .and_then(|l| l.base10_parse::<u128>().ok())
                .and_then(|v| i128::try_from(v).ok());
            if let Some(v) = value {
                out.push((line_index + 1, start + 1, token, v));
            }
        }
    }
    out
}

/// One finding per number in one script other than the free numbers; `matches` names the constants of its value.
pub fn scan(
    rel: &str,
    crate_dir: &str,
    path: &Path,
    matches: &dyn Fn(&Value, &str) -> Option<String>,
) -> Vec<Finding> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let lines: Vec<&str> = text.lines().collect();
    numbers(&text)
        .into_iter()
        .filter_map(|(line, column, token, v)| {
            let value = Value::Int(v);
            if value.is_free() {
                return None;
            }
            let shown: String = lines
                .get(line - 1)
                .map(|l| l.trim())
                .unwrap_or_default()
                .chars()
                .take(MAX_LITERAL_CHARS)
                .collect();
            Some(Finding {
                list: List::Script,
                path: rel.to_string(),
                line,
                column,
                kind: Kind::Int,
                item: clean_literal(&format!("{LINE_ITEM_PREFIX}{shown}")),
                literal: clean_literal(&token),
                note: matches(&value, crate_dir).unwrap_or_default(),
                value: Some(value),
                crate_dir: crate_dir.to_string(),
                in_const: false,
            })
        })
        .collect()
}

pub fn is_script(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => SCRIPT_EXTENSIONS.contains(&ext),
        None => {
            let mut head = [u8::default(); SHEBANG.len()];
            std::fs::File::open(path)
                .and_then(|mut f| f.read_exact(&mut head))
                .is_ok()
                && head == SHEBANG.as_bytes()
        }
    }
}
