//! Seeded mutations of compiled shaders through `convert`: every case must return, converted or with an
//! error, and never end the process. Each mutated container is signed again (Direct3D's checksum) so
//! the mutations reach the converters instead of stopping at the checksum.
//!
//! One command, replayable: `WCONTAINERS_FUZZ_SEED` (default 1) and `WCONTAINERS_FUZZ_CASES` (default
//! 2000) choose the cases; inputs are the rig's DXBC and DXIL, this crate's own shaders and, when set,
//! the corpus in `WCONTAINERS_SHADER_CORPUS`. Each case is written to stderr before it runs, so a case
//! that ends the process names itself; `WCONTAINERS_FUZZ_FIRST` starts at that case.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use wcontainers_gpu::VariantKey;
use wcontainers_shaders::{ConvertError, Source, container, convert, token};

/// The largest single allocation the test's Rust code may make: a larger one is refused, as on a host
/// short of memory, so a size taken unchecked from the bytecode ends the test on every machine.
const LARGEST_ALLOCATION: usize = 1 << 30;

struct Bounded;

// SAFETY: every call is passed on to the system allocator unchanged, or refused with a null pointer.
unsafe impl GlobalAlloc for Bounded {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > LARGEST_ALLOCATION {
            return std::ptr::null_mut();
        }
        // SAFETY: the caller's layout, as `GlobalAlloc::alloc` requires.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Bounded = Bounded;

/// xorshift64, seeded.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Where a container's header holds its checksum.
const CHECKSUM_AT: usize = 4;

/// Writes Direct3D's checksum of the container `bytes` into its header.
fn sign(bytes: &mut [u8]) {
    let checksum = container::checksum(bytes);
    bytes[CHECKSUM_AT..CHECKSUM_AT + checksum.len()].copy_from_slice(&checksum);
}

fn inputs() -> Vec<(String, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut dirs = vec![
        root.join("../wcontainers-graphics-test/shaders/dxbc"),
        root.join("../wcontainers-graphics-test/shaders/dxil"),
        root.join("tests/own"),
    ];
    if let Some(corpus) = std::env::var_os("WCONTAINERS_SHADER_CORPUS") {
        dirs.push(corpus.into());
    }
    let mut files = Vec::new();
    for dir in dirs {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "dxbc" || e == "dxil") {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                files.push((name, std::fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn signing_again_gives_the_compilers_checksum() {
    for (name, original) in inputs() {
        let mut again = original.clone();
        again[CHECKSUM_AT..CHECKSUM_AT + size_of::<container::Checksum>()].fill(0);
        sign(&mut again);
        assert_eq!(again, original, "{name}");
    }
}

#[test]
#[ignore = "seeded fuzz, a long run (WCONTAINERS_FUZZ_SEED, WCONTAINERS_FUZZ_CASES); a known failure: \
            malformed DXIL can end the process inside dxil-spirv, in its bitcode reader and, after the \
            reader accepted the module, in its converter's metadata reads and instruction emission (gap A11)"]
fn mutated_bytecode_converts_or_fails_without_ending_the_process() {
    let number = |name: &str, default: u64| {
        std::env::var(name)
            .ok()
            .map_or(default, |s| s.parse().expect(name))
    };
    let seed = number("WCONTAINERS_FUZZ_SEED", 1);
    let cases = number("WCONTAINERS_FUZZ_CASES", 2000) as usize;
    let first = number("WCONTAINERS_FUZZ_FIRST", 0) as usize;
    let inputs = inputs();
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut counts = BTreeMap::<String, usize>::new();
    for case in 0..cases {
        let (name, original) = &inputs[rng.below(inputs.len())];
        let mut b = original.clone();
        let mut what = Vec::new();
        for _ in 0..1 + rng.below(4) {
            let at = 32 + rng.below(b.len().saturating_sub(32));
            match rng.below(4) {
                0 if at < b.len() => {
                    let bit = rng.below(8);
                    b[at] ^= 1 << bit;
                    what.push(format!("flip@{at}.{bit}"));
                }
                1 if at < b.len() => {
                    b[at] = rng.next() as u8;
                    what.push(format!("byte@{at}={}", b[at]));
                }
                2 => {
                    let at = 32 + rng.below(b.len().saturating_sub(32));
                    b.truncate(at);
                    what.push(format!("cut@{at}"));
                }
                3 if at + 4 <= b.len() => {
                    let word = rng.next() as u32;
                    b[at..at + 4].copy_from_slice(&word.to_le_bytes());
                    what.push(format!("word@{at}={word}"));
                }
                _ => {}
            }
        }
        if case < first {
            continue;
        }
        if b.len() >= 20 {
            sign(&mut b);
        }
        writeln!(
            std::io::stderr(),
            "case {case} seed {seed}: {name} {}",
            what.join(",")
        )
        .unwrap();
        let source = if name.ends_with(".dxil") {
            Source::Dxil(&b)
        } else {
            Source::Dxbc(&b)
        };
        let class = match convert(source, &VariantKey::default(), None) {
            Ok(_) => "ok".into(),
            Err(e @ (ConvertError::Malformed(_) | ConvertError::Invalid(_))) => {
                // Refused, but by our lowering's own checks or naga's validation rather than by a
                // converter: listed per case, since on real bytecode these are our bugs.
                eprintln!("case {case} seed {seed}: ours: {e}");
                format!("{e}")
                    .split(':')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(":")
            }
            Err(e) => format!("{e}").split(':').next().unwrap().to_string(),
        };
        *counts.entry(class).or_insert(0) += 1;
    }
    eprintln!(
        "seed {seed}, {} cases from {first}:",
        cases - first.min(cases)
    );
    for (class, n) in &counts {
        eprintln!("  {n:5} {class}");
    }
}

/// Applies to `b` the mutations `what` as the seeded fuzz writes them: `flip@<byte>.<bit>`,
/// `byte@<byte>=<value>` and `word@<byte>=<little-endian word>`, joined by commas.
fn mutate(b: &mut [u8], what: &str) {
    for m in what.split(',') {
        let (kind, place) = m.split_once('@').unwrap();
        let (at, value) = place.split_once(['.', '=']).unwrap();
        let at: usize = at.parse().unwrap();
        match kind {
            "flip" => b[at] ^= 1 << value.parse::<u32>().unwrap(),
            "byte" => b[at] = value.parse().unwrap(),
            "word" => b[at..at + size_of::<u32>()]
                .copy_from_slice(&value.parse::<u32>().unwrap().to_le_bytes()),
            other => panic!("no mutation {other}"),
        }
    }
}

/// Cases of the seeded fuzz (seed 31337, the corpus beside it) whose DXIL module names no entry
/// point, which ended the process in dxil-spirv's entry point lookups, as its log names them: each
/// is refused.
#[test]
fn dxil_naming_no_entry_point_is_refused() {
    let cases = [
        (
            "hs_line_pow2_point.dxil",
            "byte@2289=162,word@2265=3220753132",
        ),
        (
            "ps_pull.dxil",
            "flip@837.0,flip@2898.4,word@2367=2612216941",
        ),
        (
            "vs_eval_exact.dxil",
            "word@1024=3974470098,byte@3377=152,byte@2577=231,flip@1210.2",
        ),
    ];
    let own = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/own");
    for (name, what) in cases {
        let mut b = std::fs::read(own.join(name)).unwrap();
        mutate(&mut b, what);
        sign(&mut b);
        match convert(Source::Dxil(&b), &VariantKey::default(), None) {
            Ok(_) => panic!("{name}: converted"),
            Err(e) => eprintln!("{name}: {e}"),
        }
    }
}

/// dxbc-spirv's enums, as build.rs writes them from its headers (`dxbc_op`, `dxbc_uav`).
mod codes {
    include!(concat!(env!("OUT_DIR"), "/ir_codes.rs"));
}

/// Direct3D's opcode of a sampler declaration, and the word of a declaration holding its last register
/// (after the opcode, operand, id and first register).
const SAMPLER_DECLARATION: u32 = codes::dxbc_op::DCL_SAMPLER;
const LAST_REGISTER_WORD: usize = 4;
/// The byte offset in container `b` of its program's first instruction whose opcode `wanted` takes.
fn instruction_at(b: &[u8], wanted: impl Fn(u32) -> bool) -> usize {
    let program = container::chunk_list(b)
        .unwrap()
        .map_while(Result::ok)
        .find(|c| c.tag().is_some_and(container::is_program))
        .and_then(|c| c.contents())
        .expect("the program chunk");
    let instruction = container::instructions(program)
        .map_while(Result::ok)
        .find(|i| wanted(token::opcode(i.opcode_token())))
        .expect("the instruction");
    instruction.bytes().as_ptr().addr() - b.as_ptr().addr()
}
/// A last register far past any binding range.
const FAR_REGISTER: u32 = 0x3fff_ffff;

/// A signed shader (model 5.1) declaring a sampler array whose last register is far past any
/// binding: refused by name, where its register list once asked for gigabytes (past
/// [`LARGEST_ALLOCATION`]) and ended the process.
#[test]
fn a_resource_array_longer_than_its_kind_binds_is_refused_by_name() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/own/ps_spaces.dxbc");
    let mut b = std::fs::read(path).unwrap();
    let last =
        instruction_at(&b, |op| op == SAMPLER_DECLARATION) + size_of::<u32>() * LAST_REGISTER_WORD;
    b[last..last + size_of::<u32>()].copy_from_slice(&FAR_REGISTER.to_le_bytes());
    sign(&mut b);
    assert_eq!(
        convert(Source::Dxbc(&b), &VariantKey::default(), None).map(|_| ()),
        Err(ConvertError::Unsupported(
            wcontainers_shaders::refusal::BINDINGS_BEYOND_LIMIT
        ))
    );
}

/// Direct3D's opcodes of the typed, raw and structured view declarations, and the bit of their first
/// word declaring the view globally coherent: the view flags start at bit 16 (dxbc-spirv's
/// `OpToken::getUavFlags`).
const VIEW_DECLARATIONS: std::ops::RangeInclusive<u32> =
    codes::dxbc_op::DCL_UAV_TYPED..=codes::dxbc_op::DCL_UAV_STRUCTURED;
const VIEW_FLAGS_SHIFT: u32 = 16;
const GLOBALLY_COHERENT_BIT: u32 = codes::dxbc_uav::GLOBALLY_COHERENT << VIEW_FLAGS_SHIFT;

/// A signed compute shader whose first view is declared `globallycoherent`: refused by name, as no
/// back end makes its stores seen across workgroups; unchanged, it converts.
#[test]
fn a_globally_coherent_view_is_refused_by_name() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/own/cs_atomics.dxbc");
    let mut b = std::fs::read(path).unwrap();
    assert!(convert(Source::Dxbc(&b), &VariantKey::default(), None).is_ok());
    let first = instruction_at(&b, |op| VIEW_DECLARATIONS.contains(&op));
    let word = u32::from_le_bytes(b[first..first + size_of::<u32>()].try_into().unwrap());
    let marked = word | GLOBALLY_COHERENT_BIT;
    b[first..first + size_of::<u32>()].copy_from_slice(&marked.to_le_bytes());
    sign(&mut b);
    assert_eq!(
        convert(Source::Dxbc(&b), &VariantKey::default(), None).map(|_| ()),
        Err(ConvertError::Unsupported(
            wcontainers_shaders::refusal::GLOBALLY_COHERENT
        ))
    );
}
