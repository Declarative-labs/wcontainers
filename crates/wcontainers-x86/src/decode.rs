//! The instruction at the start of some bytes, from iced-x86, with what iced does not say: the address size,
//! and the length the CPU counts for an encoding iced calls invalid or one of a feature never presented.

use iced_x86::{Code, Decoder, DecoderError, DecoderOptions};

mod groups {
    include!(concat!(env!("OUT_DIR"), "/groups.rs"));
}

/// One decoded instruction.
#[derive(Clone, Copy, Debug)]
pub struct Instruction {
    pub iced: iced_x86::Instruction,
    /// An address-size prefix (0x67) makes its addresses 32 bits.
    pub address32: bool,
    /// Longer than the CPU's 15-byte limit: general protection.
    pub too_long: bool,
}

/// The claim group of a `Code` (`claims::GROUP_*`), from iced's tables at build time.
pub fn claim_group(code: Code) -> u8 {
    groups::GROUPS[code as usize]
}

fn options() -> u32 {
    DecoderOptions::FORCE_RESERVED_NOP
}

/// The instruction at the start of `code` with its length, `None` when it runs past the end.
pub fn decode(code: &[u8]) -> Option<(Instruction, u8)> {
    let mut decoder = Decoder::with_ip(64, code, 0, options());
    let mut iced = decoder.decode();
    // iced may want more bytes than the CPU counts for an invalid encoding.
    let cut = decoder.last_error() == DecoderError::NoMoreBytes;
    let prefixes = prefix_run(code);
    let address32 = code[..prefixes].contains(&0x67);
    let mut too_long = false;
    // iced stops reading at 15 bytes and at what makes an encoding invalid; the CPU counts the whole
    // encoding, and its limit wins over every other rule. An instruction of a feature never presented is
    // counted as the reference CPU counts it too, which for other makers' instructions (3DNow, XOP, SSE4a) is
    // not their length.
    if cut || iced.is_invalid() || claim_group(iced.code()) == crate::claims::GROUP_NEVER {
        let length = cpu_length(code, prefixes)?;
        too_long = length > 15;
        iced.set_len(length.min(255));
    }
    let length = iced.len() as u8;
    Some((
        Instruction {
            iced,
            address32,
            too_long,
        },
        length,
    ))
}

/// How many bytes at the start of `code` are prefixes (legacy ones and REX).
fn prefix_run(code: &[u8]) -> usize {
    code.iter()
        .take_while(|&&b| is_legacy_prefix(b) || (0x40..=0x4f).contains(&b))
        .count()
}

fn is_legacy_prefix(b: u8) -> bool {
    matches!(
        b,
        0x26 | 0x2e | 0x36 | 0x3e | 0x64 | 0x65 | 0x66 | 0x67 | 0xf0 | 0xf2 | 0xf3
    )
}

/// What the CPU's length decoder counts after each opcode of the one-byte map and of the 0F map, one
/// character an opcode (from the reference's sweep, `tests/lengths.txt`; Intel's opcode map where a byte
/// is a prefix or the instruction faults however short it is): `.` nothing, `m` a ModRM with the SIB and
/// displacement it asks for, `i` a ModRM and an 8-bit immediate, `I` a ModRM and a 16- or 32-bit one, `r`
/// a ModRM whose mode the CPU ignores (one byte), `b` an 8-bit immediate, `w` a 16-bit one, `e` a 16-bit and
/// an 8-bit one, `z` a 16- or 32-bit one, `d` a 32-bit one whatever the operand size, `v` a 16-, 32- or
/// 64-bit one, `p` a far pointer (`z` and 2 bytes), `o` an address (8 bytes, 4 with 67), `g` and `G` a
/// ModRM and, for its reg field 0 or 1, an 8-bit or a `z` immediate. The 0F38 map counts a ModRM after
/// every opcode, the 0F3A map a ModRM and an 8-bit immediate.
const ONE_BYTE: &[u8] = concat!(
    "mmmmbz..mmmmbz..",
    "mmmmbz..mmmmbz..",
    "mmmmbz..mmmmbz..",
    "mmmmbz..mmmmbz..",
    "................",
    "................",
    "...m....zIbi....",
    "bbbbbbbbbbbbbbbb",
    "iIiimmmmmmmmmmmm",
    "..........p.....",
    "oooo....bz......",
    "bbbbbbbbvvvvvvvv",
    "iiw...iIe.w..b..",
    "mmmmbb..mmmmmmmm",
    "bbbbbbbbddpb....",
    "......gG......mm",
)
.as_bytes();
const TWO_BYTE: &[u8] = concat!(
    "mmmm.........m..",
    "mmmmmmmmmmmmmmmm",
    "rrrr....mmmmmmmm",
    "................",
    "mmmmmmmmmmmmmmmm",
    "mmmmmmmmmmmmmmmm",
    "mmmmmmmmmmmmmmmm",
    "iiiimmm.mmmmmmmm",
    "dddddddddddddddd",
    "mmmmmmmmmmmmmmmm",
    "...mimmm...mimmm",
    "mmmmmmmmmmimmmmm",
    "mmimiiim........",
    "mmmmmmmmmmmmmmmm",
    "mmmmmmmmmmmmmmmm",
    "mmmmmmmmmmmmmmmm",
)
.as_bytes();
const _: () = assert!(ONE_BYTE.len() == 256 && TWO_BYTE.len() == 256);

/// The length the CPU counts for an encoding iced reports as invalid, which iced stops reading early. The
/// prefixes that can change a length are the last 66 and REX.W in a REX directly before the opcode (not
/// before a VEX or EVEX prefix, where they only make it invalid), and 67. A VEX or EVEX prefix's map is
/// the low two bits of its map field; with those 0 the CPU counts the byte after C4 or 62 as a ModRM and
/// nothing more. In the VEX and EVEX 0F map, 38 to 3F are opcodes with nothing after them; in the legacy 0F
/// map they lead a third opcode byte, of the 0F38 map's kind when bit 1 is clear and of the 0F3A map's
/// when set.
fn cpu_length(code: &[u8], prefixes: usize) -> Option<usize> {
    let (mut operand16, mut address32, mut rex) = (false, false, None);
    for &b in &code[..prefixes] {
        match b {
            0x66 => operand16 = true,
            0x67 => address32 = true,
            0x40..=0x4f => {
                rex = Some(b);
                continue;
            }
            _ => {}
        }
        rex = None;
    }
    let body = &code[prefixes..];
    let first = *body.first()?;
    let vex = matches!(first, 0xc4 | 0xc5 | 0x62);
    let wide = !vex && rex.is_some_and(|r| r & 8 != 0);
    let operand16 = operand16 && !vex && !wide;
    let (map, at) = match (first, body.get(1)) {
        (0xc5, _) => (1, 2),
        (0xc4 | 0x62, Some(&fields)) if fields & 3 != 0 => {
            (fields & 3, if first == 0xc4 { 3 } else { 4 })
        }
        (0xc4 | 0x62, _) => {
            let length = prefixes + 1 + modrm_length(body.get(1..)?)?;
            return (length <= code.len()).then_some(length);
        }
        (0x0f, Some(&escape)) if (0x38..=0x3f).contains(&escape) => (2 + (escape >> 1 & 1), 2),
        (0x0f, _) => (1, 1),
        _ => (0, 0),
    };
    let opcode = usize::from(*body.get(at)?);
    let rest = body.get(at + 1..)?;
    let kind = match map {
        0 => ONE_BYTE[opcode],
        1 if vex && (0x38..=0x3f).contains(&opcode) => b'.',
        1 => TWO_BYTE[opcode],
        2 => b'm',
        _ => b'i',
    };
    let z = if operand16 { 2 } else { 4 };
    let modrm = || modrm_length(rest);
    let reg_low = || rest.first().map(|m| m >> 3 & 7 < 2);
    let after = match kind {
        b'.' => 0,
        b'm' => modrm()?,
        b'i' => modrm()? + 1,
        b'I' => modrm()? + z,
        b'r' | b'b' => 1,
        b'w' => 2,
        b'e' => 3,
        b'z' => z,
        b'd' => 4,
        b'v' => {
            if wide {
                8
            } else {
                z
            }
        }
        b'p' => z + 2,
        b'o' => {
            if address32 {
                4
            } else {
                8
            }
        }
        b'g' => modrm()? + usize::from(reg_low()?),
        b'G' => modrm()? + if reg_low()? { z } else { 0 },
        other => unreachable!("no kind {}", other as char),
    };
    let length = prefixes + at + 1 + after;
    (length <= code.len()).then_some(length)
}

/// How many bytes a ModRM at the start of `rest` takes with the SIB and displacement it asks for, `None`
/// when they run past the end. 32-bit addressing (67) lays them out the same in 64-bit mode.
fn modrm_length(rest: &[u8]) -> Option<usize> {
    let modrm = *rest.first()?;
    let (mode, rm) = (modrm >> 6, modrm & 7);
    let mut length = 1;
    if mode != 3 && rm == 4 {
        let sib = *rest.get(1)?;
        length += 1;
        if mode == 0 && sib & 7 == 5 {
            length += 4;
        }
    }
    length += match (mode, rm) {
        (1, _) => 1,
        (2, _) | (0, 5) => 4,
        _ => 0,
    };
    Some(length)
}
