//! The versions of the rules, one entry each saying what that version gates beyond the one before. The
//! baseline may gain lines only at a change where this list is longer than at the baseline's previous
//! change, and only for literals that were already in the tree (see the history module). Every change of
//! the rules that finds more appends an entry here.

pub const RULES: &[&str] = &[
    "the rule of docs/literals.md as the literals track's milestone L1 left it",
    "roles through closures, Some, Ok, branches and bindings, and the words user and owner; a negated literal in parentheses; attributes inside a conditional attribute; stringify!; tests that write a named number as a float or a string; string constants and messages that write out another constant or the crate's own name",
    "target roots of any extension, the files of submodules, and a submodule that cannot be read; roles compared through a method, through an array, and the word code; concat! as one string; a byte string's text; a constant that restates, under a name sharing a word, one in a crate that does not depend on it",
    "a test that writes a named value inside a constant of its own, whatever the constant is called",
];

/// This file, relative to the workspace root when Cargo builds it there.
pub const SOURCE: &str = file!();

/// The number of versions a copy of this file holds: the entries of its first constant, or none when
/// the copy is missing or unreadable (rules from before versions were counted).
pub fn version(text: &str) -> usize {
    let Ok(file) = syn::parse_file(text) else {
        return usize::default();
    };
    file.items
        .iter()
        .find_map(|item| match item {
            syn::Item::Const(c) => Some(c),
            _ => None,
        })
        .map_or(usize::default(), |c| match &*c.expr {
            syn::Expr::Reference(r) => match &*r.expr {
                syn::Expr::Array(a) => a.elems.len(),
                _ => usize::default(),
            },
            _ => usize::default(),
        })
}
