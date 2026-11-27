//! Measures whether today's native output for the in-process way (form I) stays confined: the checker run over every
//! piece the translator makes of words drawn from every A64 form, and of every instruction of the ELF files named on
//! the command line, under each entry rule.
//!
//! `measure [words per form] [seed] [elf files...]`

use std::collections::BTreeMap;
use std::sync::Mutex;

use wcontainers_arm64::way::{self, Kept};
use wcontainers_arm64::{Arm64, Way, forms, mirror, thread_state};
use wcontainers_translator::{LookAhead, translate_native_for};
use wcontainers_translator_confinement::{
    BASE, Breach, FLOOR_ENTRY_BYTES, FLOOR_PAGE, FRAME, FRAME_BYTES, Found, PROCESS_SLOTS, Place,
    SLOT_BYTES, SLOT_SHIFT, State, Step, WINDOW_BITS, check, decode,
};

/// The slot the pieces are made for, and where they are mapped.
const SLOT: u16 = 3;
const CODE: u64 = (1 << 45) + (1 << 34);
const FRAME_AT: u64 = 0x7fff_0000_1000;

/// The checker's constants are the translator's and the executor's.
fn constants_agree() {
    assert_eq!(PROCESS_SLOTS, way::PROCESS_SLOTS);
    assert_eq!(SLOT_BYTES, 1 << way::SLOT_BITS);
    assert_eq!(WINDOW_BITS, way::WINDOW_BITS);
    assert_eq!(SLOT_SHIFT, mirror::SLOT_SHIFT);
    assert_eq!(FLOOR_PAGE, way::IN_PROCESS_ENTRIES & !0xfff);
    assert_eq!(FLOOR_ENTRY_BYTES, way::PROCESS_ENTRY_BYTES);
    assert_eq!(FRAME, thread_state::PAGE);
    assert_eq!(BASE, Kept::USUAL.base);
    assert!(
        (way::IN_PROCESS_WITHHELD_AT + 8 * way::AFTER_PAGE_WORDS) as u64 <= FRAME_BYTES,
        "the page and the words after it are in the frame's own words"
    );
}

/// An entry rule measured: where a run may enter a piece and with what.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Rule {
    /// Any registers, at the piece's first instruction.
    AnyAtStart,
    /// Any registers, at any instruction.
    AnyAnywhere,
    /// The run sets x28, x24 and (unless displaced) a stack pointer in the window; at the first instruction.
    SetAtStart,
    /// As `SetAtStart`, but at any instruction.
    SetAnywhere,
    /// `SetAtStart` without x28's.
    SetButFrame,
    /// `SetAtStart` without x24's.
    SetButBase,
    /// `SetAtStart` without the stack pointer's.
    SetButStack,
    /// `SetAtStart` with the 64 KiB below the window reserved as well: the lowest guest address Linux maps
    /// (`wcontainers_abi::LOWEST_ADDRESS`), the stack step and its probe's page.
    SetAtStartBelowReserved,
}

const RULES: [Rule; 8] = [
    Rule::AnyAtStart,
    Rule::AnyAnywhere,
    Rule::SetAtStart,
    Rule::SetAnywhere,
    Rule::SetButFrame,
    Rule::SetButBase,
    Rule::SetButStack,
    Rule::SetAtStartBelowReserved,
];

#[derive(Default)]
struct Tally {
    pieces: u64,
    breached: u64,
    /// Breaches by kind: how many pieces, and the first few examples.
    kinds: BTreeMap<Breach, (u64, Vec<String>)>,
    /// The breached pieces by the first word of their label (the form's name, the program's), counted.
    names: BTreeMap<String, u64>,
}

#[derive(Default)]
struct Totals {
    /// By source, way and rule.
    tallies: BTreeMap<(String, bool, Rule), Tally>,
    /// Pieces whose access words the checker and the translator's decoder read differently.
    disagreements: Vec<String>,
    /// At points a run resumes mid-piece (after a call into the floor, at an access that may fault): the registers
    /// that hold a known value there under `SetAtStart`, counted.
    resume_registers: BTreeMap<String, u64>,
    resume_points: u64,
}

fn place(displaced: bool, below: u64) -> Place {
    Place {
        base: u64::from(SLOT) << way::SLOT_BITS,
        frame: FRAME_AT,
        code: CODE,
        below,
        displaced,
    }
}

fn breaches(bytes: &[u8], rule: Rule, displaced: bool) -> Vec<Found> {
    let below = if rule == Rule::SetAtStartBelowReserved {
        1 << 16
    } else {
        0
    };
    let place = place(displaced, below);
    let mut set = State::entry(&place);
    match rule {
        Rule::SetButFrame => set.0[usize::from(FRAME)] = None,
        Rule::SetButBase => set.0[usize::from(BASE)] = None,
        Rule::SetButStack => set.0[31] = None,
        _ => {}
    }
    let start = match rule {
        Rule::AnyAtStart | Rule::AnyAnywhere => State::anything(),
        _ => set,
    };
    let anywhere = matches!(rule, Rule::AnyAnywhere | Rule::SetAnywhere);
    if !anywhere {
        return check(bytes, &place, &[(0, start)]).0;
    }
    let mut all = Vec::new();
    for at in (0..bytes.len()).step_by(4) {
        for found in check(bytes, &place, &[(at, start.clone())]).0 {
            if !all.contains(&found) {
                all.push(found);
            }
        }
    }
    all
}

/// The registers known at each point a run resumes mid-piece, under `SetAtStart`.
fn resume_registers(bytes: &[u8], displaced: bool, totals: &mut Totals) {
    let place = place(displaced, 0);
    let (_, states) = check(bytes, &place, &[(0, State::entry(&place))]);
    for (&at, state) in &states {
        let word = u32::from_le_bytes(bytes[at..at + 4].try_into().expect("a word"));
        let after_call = at >= 4
            && matches!(
                decode(u32::from_le_bytes(
                    bytes[at - 4..at].try_into().expect("a word")
                )),
                Step::Jump { link: true, .. }
            );
        let faults = matches!(decode(word), Step::Access(_));
        if !(after_call || faults) {
            continue;
        }
        totals.resume_points += 1;
        let known: Vec<String> = (0..32)
            .filter(|&r| state.0[r].is_some())
            .map(|r| {
                if r == 31 {
                    "sp".to_string()
                } else {
                    format!("x{r}")
                }
            })
            .collect();
        *totals.resume_registers.entry(known.join(" ")).or_insert(0) += 1;
    }
}

/// Whether the checker reads each access of the piece with the base register the translator's decoder names.
fn agrees(bytes: &[u8]) -> Option<String> {
    for word in bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
    {
        let theirs = wcontainers_arm64::decode(word).memory_access();
        let ours = decode(word);
        match (theirs, ours) {
            (Some(_), Step::Access(_)) | (None, _) => {}
            (Some(m), other) => {
                return Some(format!("{word:#010x}: translator {m:?}, checker {other:?}"));
            }
        }
    }
    None
}

fn record(totals: &Mutex<Totals>, source: &str, label: &str, bytes: &[u8], displaced: bool) {
    let mut found_by_rule = Vec::new();
    for rule in RULES {
        found_by_rule.push((rule, breaches(bytes, rule, displaced)));
    }
    let disagreement = agrees(bytes);
    let mut totals = totals
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    resume_registers(bytes, displaced, &mut totals);
    if let Some(d) = disagreement
        && totals.disagreements.len() < 20
    {
        totals.disagreements.push(format!("{label}: {d}"));
    }
    for (rule, found) in found_by_rule {
        let tally = totals
            .tallies
            .entry((source.to_string(), displaced, rule))
            .or_default();
        tally.pieces += 1;
        if found.is_empty() {
            continue;
        }
        tally.breached += 1;
        let name = label.split(' ').next().unwrap_or(label).to_string();
        *tally.names.entry(name).or_insert(0) += 1;
        let mut kinds: Vec<Breach> = found.iter().map(|f| f.breach).collect();
        kinds.sort();
        kinds.dedup();
        for kind in kinds {
            let entry = tally.kinds.entry(kind).or_default();
            entry.0 += 1;
            if entry.1.len() < 4 {
                let f = found
                    .iter()
                    .find(|f| f.breach == kind)
                    .expect("a breach of the kind");
                entry.1.push(format!(
                    "{label} host +{} {:#010x} of {} words",
                    f.at,
                    f.word,
                    bytes.len() / 4
                ));
            }
        }
    }
}

fn ways() -> [(Way, bool); 2] {
    [false, true].map(|displaced| {
        (
            Way::InProcess {
                slot: SLOT,
                displaced,
                kept: Kept::USUAL,
            },
            displaced,
        )
    })
}

/// Executable sections of an ELF file: their bytes and addresses.
fn text_of(path: &str) -> Vec<(u64, Vec<u8>)> {
    use object::{Object, ObjectSection, SectionKind};
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let file = object::File::parse(&*data).unwrap_or_else(|e| panic!("{path}: {e}"));
    file.sections()
        .filter(|s| s.kind() == SectionKind::Text)
        .filter_map(|s| Some((s.address(), s.data().ok()?.to_vec())))
        .collect()
}

fn main() {
    constants_agree();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let per_form: u64 = args.first().and_then(|a| a.parse().ok()).unwrap_or(64);
    let seed: u64 = args
        .get(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(0x2545_f491_4f6c_dd1d);
    let elves: Vec<String> = args.iter().skip(2).cloned().collect();
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(12);
    let totals = Mutex::new(Totals::default());
    let all = forms::all();
    let guest = 0x40_1000u64;
    std::thread::scope(|scope| {
        for t in 0..threads {
            let totals = &totals;
            scope.spawn(move || {
                let mut rng = fastrand::Rng::with_seed(seed ^ t as u64);
                for form in all.iter().skip(t).step_by(threads) {
                    for _ in 0..per_form {
                        let word = form.word(rng.u32(..));
                        for (way, displaced) in ways() {
                            let Some(code) = translate_native_for::<Arm64>(
                                &word.to_le_bytes(),
                                guest,
                                LookAhead::None,
                                true,
                                &way,
                                &(),
                            ) else {
                                continue;
                            };
                            let label = format!("{} {word:#010x}", form.name);
                            record(totals, "forms", &label, &code.bytes, displaced);
                        }
                    }
                }
            });
        }
    });
    for path in &elves {
        let sections = text_of(path);
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        for (address, text) in &sections {
            let offsets: Vec<usize> = (0..text.len()).step_by(4).collect();
            std::thread::scope(|scope| {
                for t in 0..threads {
                    let (totals, offsets, name) = (&totals, &offsets, &name);
                    scope.spawn(move || {
                        for &at in offsets.iter().skip(t).step_by(threads) {
                            for (look, source) in [
                                (LookAhead::None, "programs none"),
                                (LookAhead::Bounded(16), "programs bounded 16"),
                            ] {
                                // The bounded pieces start at every eighth instruction, enough to cover their loops.
                                if source.ends_with("16") && at % 32 != 0 {
                                    continue;
                                }
                                for (way, displaced) in ways() {
                                    let Some(code) = translate_native_for::<Arm64>(
                                        &text[at..],
                                        address + at as u64,
                                        look,
                                        true,
                                        &way,
                                        &(),
                                    ) else {
                                        continue;
                                    };
                                    let word = u32::from_le_bytes(
                                        text[at..at + 4].try_into().expect("a word"),
                                    );
                                    let label =
                                        format!("{name}+{:#x} {word:#010x}", address + at as u64);
                                    record(totals, source, &label, &code.bytes, displaced);
                                }
                            }
                        }
                    });
                }
            });
        }
    }
    let totals = totals
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for ((source, displaced, rule), tally) in &totals.tallies {
        let way = if *displaced { "displaced" } else { "in window" };
        println!(
            "{source} | {way} | {rule:?} | pieces {} | breached {}",
            tally.pieces, tally.breached
        );
        for (kind, (count, examples)) in &tally.kinds {
            println!("    {kind:?}: {count} pieces; e.g. {}", examples.join("; "));
        }
        if source == "forms" && matches!(rule, Rule::SetAtStart | Rule::SetAtStartBelowReserved) {
            let names: Vec<String> = tally
                .names
                .iter()
                .map(|(n, c)| format!("{n} {c}"))
                .collect();
            println!("    breached: {}", names.join(", "));
        }
    }
    println!("resume points: {}", totals.resume_points);
    for (registers, count) in &totals.resume_registers {
        println!("    known at resume: [{registers}] {count}");
    }
    println!(
        "decoder disagreements (first 20): {}",
        totals.disagreements.len()
    );
    for d in &totals.disagreements {
        println!("    {d}");
    }
}
