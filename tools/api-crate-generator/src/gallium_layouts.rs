//! The independent count of Mesa's fields (AC8, after the check of ce6790aaa): clang's own record layouts of Mesa's
//! headers (`clang -fsyntax-only -Xclang -fdump-record-layouts` over the direct crate's flat.c) compared field by field
//! with what the glue's test printed (`FIELD` and `LEFT` rows of tests/layouts.rs, made from the generator's reading of
//! the same headers). Every structure the flat forms carry must have, in clang's layout, exactly the fields the
//! generator carries or leaves out, by name, each at the offset C's offsetof gave and each bit-field of the width the
//! generator read. A field the header reader drops (as it dropped `min_lod` of `float min_lod, max_lod;`) is a field
//! clang lists and the generator does not: it fails here.
//!
//!   api-crate-generator gallium-layouts <clang record dump> <test output>

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

/// One `FIELD` row of the test: structure, field, Mesa offset (None for a bit-field), width.
struct Printed {
    field: String,
    mesa_offset: Option<u64>,
    bits: u64,
}

/// Compares; Ok with the report when every structure agrees, Err with it otherwise.
pub fn compare(dump: &str, printed: &str) -> Result<String, String> {
    let clang = clang_structures(dump);
    let mut carried: BTreeMap<String, Vec<Printed>> = BTreeMap::new();
    let mut left: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut agreed: Option<usize> = None;
    for line in printed.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["FIELD", structure, field, "mesa_offset", offset, "mesa_size", _, "flat_offset", _, "flat_size", _, "bits", bits] => {
                carried.entry((*structure).to_string()).or_default().push(Printed { field: (*field).to_string(), mesa_offset: offset.parse().ok(), bits: bits.parse().unwrap_or(0) });
            }
            ["LEFT", structure, field] => left.entry((*structure).to_string()).or_default().push((*field).to_string()),
            ["FIELDS", "AGREE", count] => agreed = count.parse().ok(),
            _ => {}
        }
    }
    let mut report = String::new();
    let mut problems = Vec::new();
    // Every row the test printed was read (a row sharing its line with other output would be missed).
    let rows: usize = carried.values().map(Vec::len).sum();
    if agreed != Some(rows) {
        problems.push(format!("the test printed {agreed:?} fields, {rows} rows read"));
    }
    let (mut fields, mut offsets, mut widths) = (0, 0, 0);
    for (structure, rows) in &carried {
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
            let is_left = left_here.contains(&field.name);
            match rows.iter().find(|row| row.field == field.name) {
                Some(row) => {
                    fields += 1;
                    match (row.mesa_offset, field.bits) {
                        (Some(offset), None) if offset == field.offset => offsets += 1,
                        (None, Some(width)) if width == row.bits => widths += 1,
                        _ => problems.push(format!("{structure}.{}: clang offset {} width {:?}, generator offset {:?} width {}", field.name, field.offset, field.bits, row.mesa_offset, row.bits)),
                    }
                }
                None if is_left => {}
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

#[cfg(test)]
mod tests {
    use super::compare;

    const DUMP: &str = "
*** Dumping AST Record Layout
         0 | struct pipe_sampler_state
     0:0-2 |   unsigned int wrap_s
         4 |   float min_lod
         8 |   float max_lod
        12 |   union pipe_sampler_state::(unnamed at p_state.h:1:1) u
        12 |     unsigned int a
        12 |     float b
        16 |   union pipe_sampler_state::(anonymous at p_state.h:2:2)
        16 |     unsigned int c
        20 |   struct pipe_box box
        20 |     int x
           | [sizeof=24, align=4]
";

    /// The test's rows with every field, then with the first name of `float min_lod, max_lod;` dropped as the header
    /// reader dropped it: the first agrees, the second fails on min_lod.
    #[test]
    fn a_dropped_field_fails() {
        let all = "FIELD pipe_sampler_state wrap_s mesa_offset bits mesa_size bits flat_offset 0 flat_size 4 bits 3
FIELD pipe_sampler_state min_lod mesa_offset 4 mesa_size 4 flat_offset 4 flat_size 4 bits 0
FIELD pipe_sampler_state max_lod mesa_offset 8 mesa_size 4 flat_offset 8 flat_size 4 bits 0
FIELD pipe_sampler_state u mesa_offset 12 mesa_size 4 flat_offset 12 flat_size 8 bits 0
FIELD pipe_sampler_state c mesa_offset 16 mesa_size 4 flat_offset 20 flat_size 4 bits 0
FIELD pipe_sampler_state box mesa_offset 20 mesa_size 4 flat_offset 24 flat_size 4 bits 0
FIELD pipe_sampler_state.u a mesa_offset 0 mesa_size 4 flat_offset 0 flat_size 4 bits 0
FIELD pipe_sampler_state.u b mesa_offset 0 mesa_size 4 flat_offset 4 flat_size 4 bits 0
FIELDS AGREE 8
";
        assert!(compare(DUMP, all).is_ok(), "{:?}", compare(DUMP, all));
        let dropped: String = all.lines().filter(|line| !line.contains(" min_lod ")).map(|line| format!("{line}\n")).collect::<String>().replace("FIELDS AGREE 8", "FIELDS AGREE 7");
        let report = compare(DUMP, &dropped).expect_err("a dropped field must fail");
        assert!(report.contains("pipe_sampler_state.min_lod: in clang's layout, neither carried nor left out"), "{report}");
        let moved = all.replace("max_lod mesa_offset 8", "max_lod mesa_offset 12");
        assert!(compare(DUMP, &moved).is_err());
        // A row the comparison could not read (sharing its line with other output) fails too.
        let hidden = all.replacen("FIELD pipe_sampler_state wrap_s", "test fields_agree ... FIELD pipe_sampler_state wrap_s", 1);
        assert!(compare(DUMP, &hidden).is_err());
    }
}
