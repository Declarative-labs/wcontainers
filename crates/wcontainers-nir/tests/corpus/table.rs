//! The corpus's tables of defines (`corpus/godot-defines` and the like): one row a line, its first
//! word the key, a line starting as dEQP's comments do a comment.

use std::path::Path;

/// What starts a comment in the tables and in dEQP's case files.
pub const COMMENT: char = '#';
/// The escaped line break of the tables' defines.
pub const ESCAPED_LINE_BREAK: &str = "\\n";

/// Each row that is not a comment, split into its first word and the rest.
pub fn rows(table: &str) -> impl Iterator<Item = (&str, &str)> {
    table
        .lines()
        .filter(|l| !l.starts_with(COMMENT) && !l.trim().is_empty())
        .map(|l| l.split_once(char::is_whitespace).unwrap_or((l, "")))
}

/// The defines of the row keyed by `path` (none when it is not listed).
pub fn general_defines(table: &str, path: &Path) -> String {
    rows(table)
        .find_map(|(file, defines)| (Path::new(file) == path).then(|| defines.trim().to_owned()))
        .unwrap_or_default()
}

/// A table's defines as the text they stand for.
pub fn unescape(defines: &str) -> String {
    defines.replace(ESCAPED_LINE_BREAK, "\n")
}
