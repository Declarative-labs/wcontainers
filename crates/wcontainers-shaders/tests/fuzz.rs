//! Seeded mutations of compiled shaders through `convert`: every case must return, converted or with an
//! error, and never end the process. Each mutated container is signed again (Direct3D's checksum) so
//! the mutations reach the converters instead of stopping at the checksum.
//!
//! One command, replayable: `WCONTAINERS_FUZZ_SEED` (default 1) and `WCONTAINERS_FUZZ_CASES` (default
//! 2000) choose the cases; inputs are the rig's DXBC and DXIL, this crate's own shaders and, when set,
//! the corpus in `WCONTAINERS_SHADER_CORPUS`. Each case is written to stderr before it runs, so a case
//! that ends the process names itself; `WCONTAINERS_FUZZ_FIRST` starts at that case.

use md5::block_api::{Md5Core, compress};
use md5::digest::common::hazmat::SerializableState;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use wcontainers_gpu::VariantKey;
use wcontainers_shaders::{ConvertError, Source, convert, token};

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

/// Direct3D's container checksum: MD5's block function (the `md-5` crate's) over the container past
/// the checksum, with its own last block (as dxbc-spirv's `hashDxbcBinary`), from MD5's initial state.
fn sign(container: &mut [u8]) {
    let data = &container[20..];
    let bits = (data.len() as u32).wrapping_mul(8);
    let (a, b) = (bits.to_le_bytes(), ((bits >> 2) | 1).to_le_bytes());
    let full = data.len() - data.len() % 64;
    let rest = &data[full..];
    let mut tail = Vec::new();
    if rest.len() >= 56 {
        tail.extend_from_slice(rest);
        tail.push(0x80);
        tail.resize(64, 0);
        tail.extend_from_slice(&a);
        tail.resize(124, 0);
        tail.extend_from_slice(&b);
    } else {
        tail.extend_from_slice(&a);
        tail.extend_from_slice(rest);
        tail.push(0x80);
        tail.resize(60, 0);
        tail.extend_from_slice(&b);
    }
    let initial = Md5Core::default().serialize();
    let mut state = [0u32; 4];
    for (word, bytes) in state.iter_mut().zip(initial.as_chunks::<4>().0) {
        *word = u32::from_le_bytes(*bytes);
    }
    compress(&mut state, data[..full].as_chunks::<64>().0);
    compress(&mut state, tail.as_chunks::<64>().0);
    for (i, word) in state.iter().enumerate() {
        container[4 + 4 * i..8 + 4 * i].copy_from_slice(&word.to_le_bytes());
    }
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
        again[4..20].fill(0);
        sign(&mut again);
        assert_eq!(again, original, "{name}");
    }
}

#[test]
#[ignore = "seeded fuzz, a long run: WCONTAINERS_FUZZ_SEED, WCONTAINERS_FUZZ_CASES"]
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
                    b[at] ^= 1 << rng.below(8);
                    what.push(format!("flip@{at}"));
                }
                1 if at < b.len() => {
                    b[at] = rng.next() as u8;
                    what.push(format!("byte@{at}"));
                }
                2 => {
                    let at = 32 + rng.below(b.len().saturating_sub(32));
                    b.truncate(at);
                    what.push(format!("cut@{at}"));
                }
                3 if at + 4 <= b.len() => {
                    b[at..at + 4].copy_from_slice(&(rng.next() as u32).to_le_bytes());
                    what.push(format!("word@{at}"));
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

/// dxbc-spirv's enums, as build.rs writes them from its headers (`dxbc_op`, `dxbc_uav`).
mod codes {
    include!(concat!(env!("OUT_DIR"), "/ir_codes.rs"));
}

/// Direct3D's opcode of a sampler declaration, and the word of a declaration holding its last register
/// (after the opcode, operand, id and first register).
const SAMPLER_DECLARATION: u32 = codes::dxbc_op::DCL_SAMPLER;
const LAST_REGISTER_WORD: usize = 4;
/// The program chunk's words before its first instruction: tag, size, version, length.
const PROGRAM_HEADER_WORDS: usize = 4;
/// A last register far past any binding range.
const FAR_REGISTER: u32 = 0x3fff_ffff;

/// A signed shader (model 5.1) declaring a sampler array whose last register is far past any
/// binding: refused by name, where its register list once asked for gigabytes (past
/// [`LARGEST_ALLOCATION`]) and ended the process.
#[test]
fn a_resource_array_longer_than_its_kind_binds_is_refused_by_name() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/own/ps_spaces.dxbc");
    let mut b = std::fs::read(path).unwrap();
    let words: Vec<u32> = b
        .as_chunks::<{ size_of::<u32>() }>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect();
    let tag = u32::from_le_bytes(*b"SHEX");
    let mut at = words
        .iter()
        .position(|&w| w == tag)
        .expect("the program chunk")
        + PROGRAM_HEADER_WORDS;
    while words[at] & token::OPCODE_MASK != SAMPLER_DECLARATION {
        at += ((words[at] >> token::LENGTH_SHIFT) & token::LENGTH_MASK) as usize;
    }
    let last = size_of::<u32>() * (at + LAST_REGISTER_WORD);
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
    let words: Vec<u32> = b
        .as_chunks::<{ size_of::<u32>() }>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect();
    let tag = u32::from_le_bytes(*b"SHEX");
    let mut at = words
        .iter()
        .position(|&w| w == tag)
        .expect("the program chunk")
        + PROGRAM_HEADER_WORDS;
    while !VIEW_DECLARATIONS.contains(&(words[at] & token::OPCODE_MASK)) {
        at += ((words[at] >> token::LENGTH_SHIFT) & token::LENGTH_MASK) as usize;
    }
    let first = size_of::<u32>() * at;
    let marked = words[at] | GLOBALLY_COHERENT_BIT;
    b[first..first + size_of::<u32>()].copy_from_slice(&marked.to_le_bytes());
    sign(&mut b);
    assert_eq!(
        convert(Source::Dxbc(&b), &VariantKey::default(), None).map(|_| ()),
        Err(ConvertError::Unsupported(
            wcontainers_shaders::refusal::GLOBALLY_COHERENT
        ))
    );
}
