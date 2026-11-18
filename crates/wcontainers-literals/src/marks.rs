//! Marks: the one way a number that truly stands alone may stay where it is. A mark is a line comment
//! directly above the line that holds the number, naming the number exactly as written and giving a
//! one-line reason:
//!
//! ```text
//! // standalone 4720: the signal frame's size, measured on the reference machine
//! let size = 4720;
//! ```
//!
//! Several marks may stand one above the other for several numbers on the same line.

/// The word that starts a mark, after `//`.
pub const MARK_WORD: &str = "standalone";
const LINE_COMMENT: &str = "//";
/// What follows `//` in a doc comment (`///`, `//!`), which is never a mark.
const DOC_COMMENT_MARKS: &[char] = &['/', '!'];
/// Separates the number from the reason.
const REASON_SEPARATOR: char = ':';
const MINUS: char = '-';

/// Why a mark cannot stand, when that is visible from the mark alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Problem {
    NoReason,
    NotANumber,
}

impl Problem {
    pub fn note(self) -> &'static str {
        match self {
            Problem::NoReason => NO_REASON,
            Problem::NotANumber => NOT_A_NUMBER,
        }
    }
}

const NO_REASON: &str =
    "a mark needs a reason after the colon: `// standalone NUMBER: why no name fits`";
const NOT_A_NUMBER: &str = "a mark names one number exactly as written: `// standalone NUMBER: reason`; text is never marked";

#[derive(Clone, Debug)]
pub struct Mark {
    /// The 1-based line of the mark itself.
    pub line: usize,
    /// The 1-based line the mark covers: the first line below that is not a mark.
    pub target: usize,
    /// The number as written, or the whole text after the mark word when it is not a number.
    pub literal: String,
    pub reason: Result<String, Problem>,
}

/// The text after `// standalone`, when the line is a mark.
fn mark_text(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix(LINE_COMMENT)?;
    if rest.starts_with(DOC_COMMENT_MARKS) {
        return None;
    }
    let after = rest.trim_start().strip_prefix(MARK_WORD)?;
    (after.is_empty() || after.starts_with(char::is_whitespace)).then(|| after.trim())
}

fn is_number(text: &str) -> bool {
    let digits = text.strip_prefix(MINUS).unwrap_or(text);
    matches!(
        syn::parse_str::<syn::Lit>(digits),
        Ok(syn::Lit::Int(_) | syn::Lit::Float(_))
    )
}

/// The lines inside literals that span several lines (after their first), where text that looks like
/// a mark is part of a string.
fn inside_literals(
    tokens: proc_macro2::TokenStream,
    out: &mut Vec<std::ops::RangeInclusive<usize>>,
) {
    for token in tokens {
        match token {
            proc_macro2::TokenTree::Group(g) => inside_literals(g.stream(), out),
            proc_macro2::TokenTree::Literal(l) => {
                let (start, end) = (l.span().start().line, l.span().end().line);
                if end > start {
                    out.push(start + 1..=end);
                }
            }
            _ => {}
        }
    }
}

/// Every mark in a file's text.
pub fn parse(text: &str) -> Vec<Mark> {
    let lines: Vec<&str> = text.lines().collect();
    let mut strings = Vec::new();
    if let Ok(tokens) = text.parse() {
        inside_literals(tokens, &mut strings);
    }
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(text) = mark_text(line) else {
            continue;
        };
        if strings.iter().any(|r| r.contains(&(i + 1))) {
            continue;
        }
        let below = lines
            .iter()
            .enumerate()
            .skip(i + 1)
            .find(|(_, l)| mark_text(l).is_none())
            .map_or(lines.len(), |(j, _)| j);
        let (literal, reason) = match text.split_once(REASON_SEPARATOR) {
            Some((number, reason)) => (number.trim(), reason.trim()),
            None => (text, ""),
        };
        let reason = if !is_number(literal) {
            Err(Problem::NotANumber)
        } else if reason.is_empty() {
            Err(Problem::NoReason)
        } else {
            Ok(reason.to_string())
        };
        out.push(Mark {
            line: i + 1,
            target: below + 1,
            literal: literal.to_string(),
            reason,
        });
    }
    out
}
