//! The x86-64 reference CPU's instruction lengths for every opcode of every map, under the prefixes and VEX
//! and EVEX fields that can change a length: the sweep `wcontainers-x86`'s length table is checked against.
//!
//! Each probe is an escape (legacy prefixes and map bytes, or a VEX or EVEX prefix), an opcode and a filler.
//! The CPU's 15-byte limit wins over every other rule, so with `k` segment prefixes before the probe it gives
//! general protection at the instruction exactly when `k` plus the length it counts passes 15; the smallest
//! such `k` gives the length. A probe that gives general protection with no prefix before it (a kernel-only
//! instruction, a non-canonical address) has no length measured.
//!
//! Usage: `WC_REFERENCE_X86_64=<host> x86-lengths <output> [connections]`.

use std::fmt::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use wcontainers_guest_cpu::Guest;
use wcontainers_translator_test::reference::Reference;
use wcontainers_translator_test::x86_64::guest::Stop;
use wcontainers_translator_test::x86_64::layer1::on_reference;
use wcontainers_translator_test::x86_64::part::{self, RIP, plain};
use wcontainers_x86::X86_64;
use wcontainers_x86::encoding::{
    ADDRESS_SIZE, ES, ESCAPE, ESCAPE_0F3A, ESCAPES_THREE_BYTE, EVEX, OPERAND_SIZE, REP, REPNE, REX,
    REX_W, VEX2, VEX3,
};

/// The fillers after the opcode: a ModRM asking for nothing more (reg 0 and reg 7), and one asking for a SIB
/// and a 32-bit displacement. The rest of each is `c0`.
const FILLERS: [&[u8]; 3] = [&[0xc0], &[0x04, 0x05], &[0xf8]];

/// The escapes before the opcode.
fn escapes() -> Vec<Vec<u8>> {
    let mut all: Vec<Vec<u8>> = Vec::new();
    // The four legacy maps, alone and after each prefix that can change what follows.
    let escape_0f38 = *ESCAPES_THREE_BYTE.start();
    let wide = REX | REX_W;
    for context in [
        &[][..],
        &[OPERAND_SIZE],
        &[wide],
        &[ADDRESS_SIZE],
        &[REPNE],
        &[REP],
    ] {
        for map in [
            &[][..],
            &[ESCAPE],
            &[ESCAPE, escape_0f38],
            &[ESCAPE, escape_0f38 | ESCAPE_0F3A],
        ] {
            all.push([context, map].concat());
        }
    }
    // In place of the opcode: the byte after 8F (XOP's map field, a ModRM to Intel CPUs), and a 3DNow suffix
    // after 0F 0F and a register ModRM.
    all.push(vec![0x8f]);
    all.push(vec![ESCAPE, ESCAPE, 0xc0]);
    // Two-byte VEX, without and with pp = 66.
    all.push(vec![VEX2, 0xf8]);
    all.push(vec![VEX2, 0xf9]);
    // Three-byte VEX, every value of the map field; W and pp for the first four.
    for map in 0..32u8 {
        all.push(vec![VEX3, 0xe0 | map, 0x78]);
    }
    for map in 0..4u8 {
        for fields in [0xf8, 0x79, 0xf9] {
            all.push(vec![VEX3, 0xe0 | map, fields]);
        }
    }
    // EVEX, every value of the three map bits; for the first four W, pp, the bit of P1 that must be 1, the
    // bit of P0 that must be 0, and R and X both set (which would be a ModRM's register mode outside 64-bit
    // mode).
    for map in 0..8u8 {
        all.push(vec![EVEX, 0xf0 | map, 0x7c, 0x08]);
    }
    for map in 0..4u8 {
        for (p0, p1) in [
            (0xf0, 0xfc),
            (0xf0, 0x7d),
            (0xf0, 0x78),
            (0xf8, 0x7c),
            (0x30, 0x7c),
        ] {
            all.push(vec![EVEX, p0 | map, p1, 0x08]);
        }
    }
    all
}

/// The probe's first `MAX_LENGTH` + 1 bytes after `prefixes` segment prefixes. What lies past them never decides the
/// answer: a byte there that the CPU reads makes the instruction longer than 15 bytes whatever it is.
fn probe(prefixes: usize, escape: &[u8], opcode: u8, filler: &[u8]) -> Vec<u8> {
    let mut bytes = vec![ES; prefixes];
    bytes.extend(escape);
    bytes.push(opcode);
    bytes.extend(filler);
    bytes.resize(longest() + 1, 0xc0);
    bytes
}

/// The length the CPU counts, `None` when the probe gives general protection with no prefix before it.
fn length(reference: &mut Reference, escape: &[u8], opcode: u8, filler: &[u8]) -> Option<u8> {
    let mut too_long = |prefixes: usize| {
        let case = plain(&probe(prefixes, escape, opcode, filler));
        let outcome = on_reference(reference, &case);
        outcome.stop == Stop::GeneralProtection
            && outcome.state.0[RIP] == u128::from(wcontainers_translator_test::layout::CODE)
    };
    if too_long(0) {
        return None;
    }
    // Fewer than `low` prefixes fit; `high` do not (one byte past the limit at least).
    let (mut low, mut high) = (0, longest());
    while high - low > 1 {
        let middle = (low + high) / 2;
        if too_long(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    Some((longest() + 1 - high) as u8)
}

/// The CPU's limit on an instruction's length.
fn longest() -> usize {
    X86_64::MAX_LENGTH as usize
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let (output, connections) = match arguments.as_slice() {
        [_, output] => (output.clone(), 6),
        [_, output, n] => (output.clone(), n.parse().expect("a number of connections")),
        _ => panic!("usage: x86-lengths <output> [connections]"),
    };
    let lines: Vec<(Vec<u8>, &[u8])> = escapes()
        .into_iter()
        .flat_map(|escape| FILLERS.map(|filler| (escape.clone(), filler)))
        .collect();
    let results: Mutex<Vec<Option<String>>> = Mutex::new(vec![None; lines.len()]);
    let next = AtomicUsize::new(0);
    let model = Mutex::new(String::new());
    std::thread::scope(|scope| {
        for _ in 0..connections {
            scope.spawn(|| {
                let mut reference = Reference::start(&part::REFERENCE);
                *model.lock().unwrap() = reference.model.clone();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((escape, filler)) = lines.get(i) else {
                        break;
                    };
                    let lengths: String = (0..=255u8)
                        .map(
                            |opcode| match length(&mut reference, escape, opcode, filler) {
                                Some(n) => char::from_digit(u32::from(n), 16).unwrap(),
                                None => '-',
                            },
                        )
                        .collect();
                    let line = format!("{} {} {lengths}", hex(escape), hex(filler));
                    eprintln!("{i} of {}: {line}", lines.len());
                    results.lock().unwrap()[i] = Some(line);
                }
            });
        }
    });
    let mut text = format!(
        "# The length the x86-64 reference CPU counts for each opcode, by\n\
         # `cargo run -p wcontainers-translator-test --bin x86-lengths`, on {}.\n\
         # Each line: the escape (prefixes and map bytes, or a VEX or EVEX prefix) in hex, the filler after\n\
         # the opcode (then c0 bytes), and for opcodes 00 to ff the length of the whole instruction as one hex\n\
         # digit, or - where it gives general protection however short it is.\n",
        model.lock().unwrap()
    );
    for line in results.into_inner().unwrap() {
        text.push_str(&line.expect("every line swept"));
        text.push('\n');
    }
    std::fs::write(&output, text).expect("write the sweep");
}
