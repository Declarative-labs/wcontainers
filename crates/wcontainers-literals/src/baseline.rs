//! The baseline file: one accepted finding (or policy line) per line, sorted, so that branches that
//! remove different lines merge without touching each other.

use std::collections::BTreeMap;
use std::path::Path;

/// The baseline, at the workspace root.
pub const BASELINE_FILE: &str = "literals.baseline";

/// A multiset of baseline lines: the same literal may sit twice in one item.
pub type Keys = BTreeMap<String, usize>;

/// Starts the last field of a line added after the baseline started whose growth is accepted, with
/// why: the line stays only until the adoption its reason names (`... when kernel adopts it`).
pub const ACCEPTED_MARK: &str = "accepted: ";
/// How the reason of an accepted line names the adoption that removes it: `when <track> adopts it`.
const ADOPTION_START: &str = "when ";
const ADOPTION_END: &str = " adopts it";

/// The line without the reason of an accepted line, and that reason when it has one.
pub fn split_reason(line: &str) -> (&str, Option<&str>) {
    line.rsplit_once(crate::FIELD_SEPARATOR)
        .and_then(|(key, last)| Some((key, Some(last.strip_prefix(ACCEPTED_MARK)?))))
        .unwrap_or((line, None))
}

/// The track whose adoption removes an accepted line, when its reason ends by naming one.
pub fn adopter(reason: &str) -> Option<&str> {
    let (_, track) = reason
        .strip_suffix(ADOPTION_END)?
        .rsplit_once(ADOPTION_START)?;
    (!track.is_empty() && !track.contains(char::is_whitespace)).then_some(track)
}

pub fn parse(text: &str) -> Keys {
    let mut keys = Keys::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        *keys.entry(line.to_string()).or_default() += 1;
    }
    keys
}

/// The file's text; a missing file is an empty baseline.
pub fn read(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// The 1-based number of the first line that sorts before the line above it.
pub fn first_unsorted(text: &str) -> Option<usize> {
    let lines: Vec<&str> = text.lines().collect();
    lines.windows(2).position(|w| w[1] < w[0]).map(|i| i + 2)
}

/// The file's text for these lines: sorted, one per line.
pub fn render(mut keys: Vec<String>) -> String {
    keys.sort();
    keys.iter()
        .map(|k| format!("{k}{}", crate::NEW_LINE))
        .collect()
}
