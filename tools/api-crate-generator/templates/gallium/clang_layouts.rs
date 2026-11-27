//! The independent count of Mesa's fields (G10): clang's own record layouts of Mesa's headers (`clang -fsyntax-only
//! -Xclang -fdump-record-layouts` over a file that lays out every structure the flat forms carry, with the flags the
//! crate's build.rs compiles the glue's C with) compared field by field with the generator's reading of the same
//! headers: every structure must have, in clang's layout, exactly the fields the generator carries or leaves out, by
//! name, each at the offset C's offsetof gave in Mesa's structure and each bit-field of the width the generator read.
//! A field the header reader drops (as it dropped `min_lod` of `float min_lod, max_lod;`) is a field clang lists and
//! the generator does not: it fails here.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// One field as clang lays it out: its name, its byte offset from the structure holding it, and its width if it is a
/// bit-field.
#[derive(Debug, Clone)]
struct ClangField {
    name: String,
    offset: u64,
    bits: Option<u64>,
}

/// What a line of the dump belongs to: the structure (by the generator's name) its fields go to with that structure's
/// own offset, or nothing (the fields of a structure held by value, which has its own dump).
type Target = Option<(String, u64)>;

/// `   8 |   float min_lod` or ` 1:0-2 |   unsigned int wrap_s` to (offset, bits, depth, text).
fn line_parts(line: &str) -> Option<(u64, Option<u64>, usize, &str)> {
    let (left, right) = line.split_once('|')?;
    let left = left.trim();
    if left.is_empty() {
        return None;
    }
    let (offset, bits) = match left.split_once(':') {
        Some((byte, range)) => {
            let (first, last) = range.split_once('-')?;
            let width = last.trim().parse::<u64>().ok()? + 1 - first.trim().parse::<u64>().ok()?;
            (byte.trim().parse().ok()?, Some(width))
        }
        None => (left.parse().ok()?, None),
    };
    let text = right.trim_start();
    let spaces = right.len() - text.len();
    Some((offset, bits, spaces.saturating_sub(1) / 2, text.trim_end()))
}

/// Every structure of the dump, by the generator's names (`pipe_draw_info.index` for a union declared inside
/// `pipe_draw_info` and named `index`), each with its fields; the fields of an anonymous structure or union are the
/// structure's around it, as the generator reads them.
fn clang_structures(dump: &str) -> BTreeMap<String, Vec<ClangField>> {
    let mut out: BTreeMap<String, Vec<ClangField>> = BTreeMap::new();
    for record in dump.split("*** Dumping AST Record Layout").skip(1) {
        let mut lines = record.lines().filter_map(line_parts);
        let Some((_, _, 0, first)) = lines.next() else { continue };
        let Some(top) = first.strip_prefix("struct ").or_else(|| first.strip_prefix("union ")) else { continue };
        // An unnamed record's own dump: its fields are read inside the named record holding it.
        if top.contains('(') || out.contains_key(top) {
            continue;
        }
        out.insert(top.to_string(), Vec::new());
        let mut stack: Vec<(usize, Target)> = vec![(0, Some((top.to_string(), 0)))];
        for (offset, bits, depth, text) in lines {
            while stack.last().is_some_and(|(at, _)| *at >= depth) {
                stack.pop();
            }
            let Some((_, target)) = stack.last().cloned() else { break };
            let Some((structure, base)) = target else {
                stack.push((depth, None));
                continue;
            };
            let unnamed = text.contains("(anonymous") || text.contains("(unnamed");
            if unnamed && text.ends_with(')') {
                // An anonymous structure or union: its fields are this structure's.
                stack.push((depth, Some((structure, base))));
                continue;
            }
            let name = text.rsplit(' ').next().unwrap_or_default().to_string();
            out.entry(structure.clone()).or_default().push(ClangField { name: name.clone(), offset: offset - base, bits });
            if unnamed {
                // A structure or union declared in place and named: the generator's `outer.name`.
                let nested = format!("{structure}.{name}");
                out.entry(nested.clone()).or_default();
                stack.push((depth, Some((nested, offset))));
            } else {
                stack.push((depth, None));
            }
        }
    }
    out
}

/// One field the generator carries: its name, Mesa's offset of it (None for a bit-field) and its width (0 where it is
/// not a bit-field).
pub struct Carried {
    pub field: String,
    pub mesa_offset: Option<u64>,
    pub bits: u64,
}

/// Compares clang's dump with the generator's fields by structure (`carried`) and the fields it leaves out (`left`);
/// Ok with the report when every structure agrees, Err with it otherwise.
pub fn compare(dump: &str, carried: &BTreeMap<String, Vec<Carried>>, left: &BTreeMap<String, Vec<String>>) -> Result<String, String> {
    let clang = clang_structures(dump);
    let mut report = String::new();
    let mut problems = Vec::new();
    let (mut fields, mut offsets, mut widths) = (0, 0, 0);
    for (structure, rows) in carried {
        let Some(theirs) = clang.get(structure) else {
            problems.push(format!("{structure}: no record layout in clang's dump"));
            continue;
        };
        let left_here = left.get(structure).cloned().unwrap_or_default();
        let ours = rows.len() + left_here.len();
        let _ = writeln!(report, "  {structure}: clang {} fields, generator {ours} ({} carried, {} left out)", theirs.len(), rows.len(), left_here.len());
        if theirs.len() != ours {
            problems.push(format!("{structure}: clang counts {} fields, the generator {ours}", theirs.len()));
        }
        for field in theirs {
            match rows.iter().find(|row| row.field == field.name) {
                Some(row) => {
                    fields += 1;
                    match (row.mesa_offset, field.bits) {
                        (Some(offset), None) if offset == field.offset => offsets += 1,
                        (None, Some(width)) if width == row.bits => widths += 1,
                        _ => problems.push(format!("{structure}.{}: clang offset {} width {:?}, generator offset {:?} width {}", field.name, field.offset, field.bits, row.mesa_offset, row.bits)),
                    }
                }
                None if left_here.contains(&field.name) => {}
                None => problems.push(format!("{structure}.{}: in clang's layout, neither carried nor left out by the generator", field.name)),
            }
        }
        for row in rows {
            if !theirs.iter().any(|field| field.name == row.field) {
                problems.push(format!("{structure}.{}: carried by the generator, not in clang's layout", row.field));
            }
        }
    }
    let _ = writeln!(report, "FIELD LAYOUTS: {} structures, {fields} fields carried ({offsets} offsets equal, {widths} bit-field widths equal); PROBLEMS {}", carried.len(), problems.len());
    for problem in &problems {
        let _ = writeln!(report, "  {problem}");
    }
    if problems.is_empty() && fields > 0 { Ok(report) } else { Err(report) }
}
