//! One finding per literal, printed one per line so that lists can be sorted, diffed and baselined.

use std::fmt;
use std::hash::Hasher;

use crate::eval::Value;

/// Separates the fields of a printed finding and of a baseline key.
pub const FIELD_SEPARATOR: char = '\t';
/// Starts the free-text note at the end of a finding; the note is not part of the baseline key.
pub const NOTE_MARK: &str = "# ";
/// Joins the names of nested items, as Rust paths do.
pub const PATH_SEPARATOR: &str = "::";
/// Joins a file's path, line and column where a finding or a mark is.
pub const LOCATION_SEPARATOR: char = ':';
/// Joins the components of a file path in findings and baseline keys, whatever the host's own
/// separator is, so that a baseline written on one system matches a run on another.
pub const PATH_COMPONENT_SEPARATOR: &str = "/";
/// The enclosing item of a literal that sits outside every named item (a top-level macro).
pub const FILE_SCOPE: &str = "(file)";
/// Longest literal text shown in a finding; longer text is cut and marked with [`CUT_MARK`].
pub const MAX_LITERAL_CHARS: usize = 160;
/// Marks a literal whose text was cut at [`MAX_LITERAL_CHARS`].
pub const CUT_MARK: char = '…';
/// Separates a cut literal from the hash of its full text in a baseline key, so that a change after
/// the cut still changes the key.
const HASH_MARK: &str = " #";

/// Which list a finding belongs to. The order here is the order lists are printed in.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum List {
    /// A literal in code: a violation unless the baseline accepts it.
    Code,
    /// A constant whose name and value restate a constant defined elsewhere (in the workspace or in a
    /// reference crate such as linux-raw-sys), reported on the copy, not on the owner.
    Redefinition,
    /// A literal in a test, or inside a constant's initialiser, whose value is a constant that its crate
    /// can already name (its own crate's for a test, an upstream crate's for a constant); and a message
    /// that writes out the value of a named constant.
    Restated,
    /// A -1, 0, 1 or 2 whose context names a role (a user or group id, a file descriptor, a process id,
    /// an exit status), so it is a meaningful value, not plain arithmetic.
    Role,
    /// A number in a shell or Python script.
    Script,
    /// A Rust file the parser could not read, so nothing in it was checked.
    Unparsed,
    /// A mark that cannot stand: no reason, not a number, nothing under it, or a number that has a name.
    Mark,
    /// A literal inside the initialiser (or type) of a named const or static item, or an enum
    /// discriminant: the one place a value may be written.
    Const,
    /// A literal in test code: the crate's tests/, benches/ or examples/ folder, a module declared
    /// under #[cfg(test)], or an item under #[test], #[bench] or a cfg that requires test.
    Test,
    /// A literal in a file the baseline's policy lists as generated.
    Generated,
    /// -1, 0, 1 or 2 in code whose context does not name a role.
    Free,
    /// Text shown to a person that holds no number, path or named value.
    Message,
    /// A number under a mark with a reason; every one is listed in the check's report.
    Marked,
}

impl List {
    pub const ALL: &[List] = &[
        List::Code,
        List::Redefinition,
        List::Restated,
        List::Role,
        List::Script,
        List::Unparsed,
        List::Mark,
        List::Const,
        List::Test,
        List::Generated,
        List::Free,
        List::Message,
        List::Marked,
    ];

    /// The list printed under this name.
    pub fn named(name: &str) -> Option<List> {
        List::ALL.iter().copied().find(|l| l.name() == name)
    }

    /// Lists that fail the check when they hold a finding the baseline does not accept.
    pub fn gated(self) -> bool {
        matches!(
            self,
            List::Code
                | List::Redefinition
                | List::Restated
                | List::Role
                | List::Script
                | List::Unparsed
                | List::Mark
        )
    }

    pub fn name(self) -> String {
        format!("{self:?}").to_lowercase()
    }

    /// What to do about a finding of this list that the baseline does not accept.
    pub fn instruction(self) -> &'static str {
        // A code finding with a note starts with USE_THE_NOTE (see `Finding::instruction`).
        match self {
            List::Code => NAME_IT,
            List::Redefinition => IMPORT_IT,
            List::Restated => WRITE_THE_NAME,
            List::Role => NAME_THE_ROLE,
            List::Script => NAME_IN_SCRIPT,
            List::Unparsed => FIX_THE_FILE,
            List::Mark => FIX_THE_MARK,
            List::Const
            | List::Test
            | List::Generated
            | List::Free
            | List::Message
            | List::Marked => NOTHING_TO_DO,
        }
    }
}

const NAME_IT: &str = "derive it, import it from the standard library or a crate that has it, or define it once as a named constant; a number that truly stands alone may carry a mark (docs/literals.md)";
const USE_THE_NOTE: &str = "use the constant the note names if it is this fact; otherwise ";
const IMPORT_IT: &str = "delete this copy and import the constant the note names";
const WRITE_THE_NAME: &str = "write it with the constant the note names";
const NAME_THE_ROLE: &str =
    "use the named constant for this id, descriptor or status, from libc or the crate that owns it";
const NAME_IN_SCRIPT: &str =
    "pass the value in from where it is named, or name it once at the top of the script";
const FIX_THE_FILE: &str = "make the file parse; nothing in it is checked until then";
const FIX_THE_MARK: &str = "correct the mark as the note says, or remove it";
const NOTHING_TO_DO: &str = "nothing: this list is not gated";

/// What kind of literal a finding is.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Kind {
    Int,
    Float,
    Byte,
    Char,
    Str,
    ByteStr,
    CStr,
    Other,
    /// A file that could not be parsed.
    Parse,
    /// A mark comment.
    Mark,
}

impl Kind {
    /// Text (strings and characters) as opposed to numbers.
    pub fn is_text(self) -> bool {
        matches!(self, Kind::Char | Kind::Str | Kind::ByteStr | Kind::CStr)
    }

    pub fn name(self) -> String {
        format!("{self:?}").to_lowercase()
    }
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub list: List,
    /// Path relative to the scanned root, with [`PATH_COMPONENT_SEPARATOR`] between components.
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub kind: Kind,
    /// The enclosing item, as a Rust-like path (`module::Type::method`); for a script, the line's text.
    pub item: String,
    /// The literal as written in the source, control characters escaped, never cut.
    pub literal: String,
    /// The literal's value, when it has one that constants can share.
    pub value: Option<Value>,
    pub note: String,
    /// The folder of the crate the finding is in; not printed.
    pub crate_dir: String,
    /// The literal sits in a named constant's initialiser (in code or in a test); not printed.
    pub in_const: bool,
}

impl Finding {
    pub fn sort_key(&self) -> (List, &str, usize, usize, Kind, &str) {
        (
            self.list,
            &self.path,
            self.line,
            self.column,
            self.kind,
            &self.literal,
        )
    }

    /// What to do about this finding when the baseline does not accept it.
    pub fn instruction(&self) -> String {
        let named = self.list == List::Code && !self.note.is_empty();
        let first = if named { USE_THE_NOTE } else { "" };
        format!("{first}{}", self.list.instruction())
    }

    /// The literal as shown: cut at [`MAX_LITERAL_CHARS`].
    pub fn shown(&self) -> String {
        cut(&self.literal)
    }

    /// Everything the baseline matches on except the list: used to tell when a literal the baseline
    /// accepted has moved to a list the check does not gate.
    pub fn rest_key(&self) -> String {
        let literal = if self.literal.chars().count() > MAX_LITERAL_CHARS {
            format!(
                "{}{HASH_MARK}{:x}",
                cut(&self.literal),
                stable_hash(&self.literal)
            )
        } else {
            self.literal.clone()
        };
        [
            self.path.clone(),
            self.kind.name(),
            self.item.clone(),
            literal,
        ]
        .join(&FIELD_SEPARATOR.to_string())
    }

    /// The part of a finding the baseline matches on: no line, column or note, so that edits elsewhere
    /// in a file do not invalidate it. A long literal is keyed by its cut text and the hash of all of it.
    pub fn key(&self) -> String {
        format!("{}{FIELD_SEPARATOR}{}", self.list.name(), self.rest_key())
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sep = FIELD_SEPARATOR;
        write!(
            f,
            "{}{sep}{}{LOCATION_SEPARATOR}{}{LOCATION_SEPARATOR}{}{sep}{}{sep}{}{sep}{}",
            self.list.name(),
            self.path,
            self.line,
            self.column,
            self.kind.name(),
            self.item,
            self.shown()
        )?;
        if !self.note.is_empty() {
            write!(f, "{sep}{NOTE_MARK}{}", self.note)?;
        }
        Ok(())
    }
}

/// Literal text as it appears in a finding: one line, no tabs; never cut (see [`Finding::shown`]).
pub fn clean_literal(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_control() {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
    }
    out
}

fn cut(text: &str) -> String {
    let mut out: String = text.chars().take(MAX_LITERAL_CHARS).collect();
    if text.chars().count() > MAX_LITERAL_CHARS {
        out.push(CUT_MARK);
    }
    out
}

/// A hash that does not change between runs, releases or hosts (FNV-1a, from the fnv crate).
pub fn stable_hash(text: &str) -> u64 {
    let mut hasher = fnv::FnvHasher::default();
    hasher.write(text.as_bytes());
    hasher.finish()
}
