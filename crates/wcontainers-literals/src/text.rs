//! Whether text that is otherwise a message carries a value: a number, or a file path. A message may
//! say what went wrong, but it may not be where a value lives ("1048576\n", "/proc/{pid}/maps").

use crate::scan::FREE_NUMBERS;

const PLACEHOLDER_OPEN: char = '{';
const PLACEHOLDER_CLOSE: char = '}';
/// What a placeholder leaves behind: a word boundary, so the text on either side stays apart.
const PLACEHOLDER_STANDIN: char = ' ';
/// Joins the parts of a dotted version or a decimal fraction, which are not values on their own.
pub const DECIMAL_POINT: char = '.';
/// Continues an identifier, so a digit after it is part of a name (`x86_64`).
pub const WORD_JOINER: char = '_';
/// Starts an absolute path.
const PATH_ROOT: char = '/';
/// Characters after which a `/` starts a path rather than joining two words (`and/or`).
const PATH_STARTERS: &[char] = &['(', '[', '{', '}', '=', ':', '\'', '"', '`', ','];

/// The text with every `{...}` placeholder replaced by a space; `{{` and `}}` stay as braces.
pub fn without_placeholders(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            PLACEHOLDER_OPEN if chars.peek() == Some(&PLACEHOLDER_OPEN) => {
                chars.next();
                out.push(c);
            }
            PLACEHOLDER_OPEN => {
                for inner in chars.by_ref() {
                    if inner == PLACEHOLDER_CLOSE {
                        break;
                    }
                }
                out.push(PLACEHOLDER_STANDIN);
            }
            _ => out.push(c),
        }
    }
    out
}

/// A number (other than the free numbers) that starts a word and is not part of a dotted version.
pub fn has_number(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut i = usize::default();
    while let Some(&c) = chars.get(i) {
        let prev = i.checked_sub(1).and_then(|p| chars.get(p)).copied();
        let starts_word =
            prev.is_none_or(|p| !(p.is_alphanumeric() || p == WORD_JOINER || p == DECIMAL_POINT));
        if !(c.is_ascii_digit() && starts_word) {
            i += 1;
            continue;
        }
        let start = i;
        while chars
            .get(i)
            .is_some_and(|c| c.is_alphanumeric() || *c == WORD_JOINER)
        {
            i += 1;
        }
        if chars.get(i) == Some(&DECIMAL_POINT)
            && chars.get(i + 1).is_some_and(char::is_ascii_digit)
        {
            while chars
                .get(i)
                .is_some_and(|c| c.is_alphanumeric() || *c == DECIMAL_POINT)
            {
                i += 1;
            }
            continue;
        }
        let token: String = chars[start..i].iter().collect();
        let value = syn::parse_str::<syn::LitInt>(&token)
            .ok()
            .and_then(|l| l.base10_parse::<u128>().ok());
        if value.is_some_and(|v| !i128::try_from(v).is_ok_and(|v| FREE_NUMBERS.contains(&v))) {
            return true;
        }
    }
    false
}

/// An absolute or relative file path: a `/` that starts a word and is followed by a name.
pub fn has_path(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    chars.iter().enumerate().any(|(i, &c)| {
        let prev = i.checked_sub(1).and_then(|p| chars.get(p)).copied();
        let next = chars.get(i + 1).copied();
        c == PATH_ROOT
            && prev.is_none_or(|p| {
                p.is_whitespace() || PATH_STARTERS.contains(&p) || p == DECIMAL_POINT
            })
            && next.is_some_and(|n| n.is_alphanumeric() || n == DECIMAL_POINT || n == WORD_JOINER)
    })
}

/// Message text that nonetheless holds a value, outside its placeholders.
pub fn value_like(text: &str) -> bool {
    let text = without_placeholders(text);
    has_number(&text) || has_path(&text)
}

/// Text that is data rather than words for a person: outside its placeholders it has no space and is
/// not one plain lower-case word (`"{name}.tar"`, `"HOME={}"`, `"GET"`, `"{}:{}"`), so a program reads
/// it as a key, a file name or a protocol word.
pub fn data_like(text: &str) -> bool {
    if text.contains(char::is_whitespace) {
        return false;
    }
    let written: String = without_placeholders(text)
        .split(PLACEHOLDER_STANDIN)
        .collect();
    !written.is_empty() && !written.chars().all(|c| c.is_lowercase())
}

/// Text that may stand as a message: it holds no value and is not data.
pub fn message_text(text: &str) -> bool {
    !value_like(text) && !data_like(text)
}
