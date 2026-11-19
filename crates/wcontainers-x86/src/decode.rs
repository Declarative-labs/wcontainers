//! The instruction at the start of some bytes, from iced-x86, with what iced does not say: the address size,
//! and the length the CPU counts for an encoding iced calls invalid or one of a feature never presented.

use crate::X86_64;
use crate::encoding::{
    ADDRESS_SIZE, ESCAPE, ESCAPE_0F3A, ESCAPES_THREE_BYTE, EVEX, EVEX_LENGTH, LEGACY_PREFIXES,
    MAP_0F, MAP_0F3A, MAP_0F38, MAP_ONE_BYTE, MODE_DISPLACEMENT8, MODE_DISPLACEMENT32,
    MODE_REGISTER, MODRM_FIELD, MODRM_MODE_SHIFT, MODRM_REG_SHIFT, OPERAND_SIZE, REX_W,
    RM_DISPLACEMENT32, RM_SIB, VEX_MAP_LOW, VEX2, VEX2_LENGTH, VEX3, VEX3_LENGTH, is_rex,
};
use iced_x86::{Code, Decoder, DecoderError, DecoderOptions};
use wcontainers_guest_cpu::Guest;

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
    let mut decoder = Decoder::with_ip(crate::MODE_BITS, code, 0, options());
    let mut iced = decoder.decode();
    // iced may want more bytes than the CPU counts for an invalid encoding.
    let cut = decoder.last_error() == DecoderError::NoMoreBytes;
    let prefixes = prefix_run(code);
    let address32 = code[..prefixes].contains(&ADDRESS_SIZE);
    let mut too_long = false;
    // iced stops reading at 15 bytes and at what makes an encoding invalid; the CPU counts the whole
    // encoding, and its limit wins over every other rule. An instruction of a feature never presented is
    // counted as the reference CPU counts it too, which for other makers' instructions (3DNow, XOP, SSE4a) is
    // not their length.
    if cut || iced.is_invalid() || claim_group(iced.code()) == crate::claims::GROUP_NEVER {
        let length = cpu_length(code, prefixes)?;
        too_long = length as u64 > X86_64::MAX_LENGTH;
        iced.set_len(length.min(u8::MAX.into()));
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
        .take_while(|&&b| LEGACY_PREFIXES.contains(&b) || is_rex(b))
        .count()
}

/// What the CPU's length decoder counts after an opcode. A `z` immediate is 16 bits with 66 and 32
/// without; a `v` one is 64 bits with REX.W and `z` without.
#[derive(Clone, Copy)]
enum After {
    Nothing,
    /// A ModRM with the SIB and displacement it asks for.
    ModRm,
    ModRmImm8,
    ModRmImmZ,
    /// A ModRM whose mode the CPU ignores (one byte).
    ModRmIgnored,
    Imm8,
    Imm16,
    Imm16Imm8,
    ImmZ,
    /// 32 bits whatever the operand size.
    Imm32,
    ImmV,
    /// A `z` offset and a 16-bit selector.
    FarPointer,
    /// 64 bits, 32 with 67.
    Address,
    /// A ModRM and, for its reg field 0 or 1, an 8-bit immediate.
    GroupImm8,
    /// A ModRM and, for its reg field 0 or 1, a `z` immediate.
    GroupImmZ,
}

/// The character standing for each `After` in the tables below.
const KINDS: &[(u8, After)] = &[
    (b'.', After::Nothing),
    (b'm', After::ModRm),
    (b'i', After::ModRmImm8),
    (b'I', After::ModRmImmZ),
    (b'r', After::ModRmIgnored),
    (b'b', After::Imm8),
    (b'w', After::Imm16),
    (b'e', After::Imm16Imm8),
    (b'z', After::ImmZ),
    (b'd', After::Imm32),
    (b'v', After::ImmV),
    (b'p', After::FarPointer),
    (b'o', After::Address),
    (b'g', After::GroupImm8),
    (b'G', After::GroupImmZ),
];

/// How many opcodes a map has.
const OPCODES: usize = 1 << u8::BITS;

/// A table of characters of `KINDS`, one an opcode, as `After`s.
const fn kinds(table: &str) -> [After; OPCODES] {
    let table = table.as_bytes();
    assert!(table.len() == OPCODES);
    let mut out = [After::Nothing; OPCODES];
    let mut opcode = 0;
    while opcode < OPCODES {
        let mut k = 0;
        while KINDS[k].0 != table[opcode] {
            k += 1;
        }
        out[opcode] = KINDS[k].1;
        opcode += 1;
    }
    out
}

/// What the CPU's length decoder counts after each opcode of the one-byte map and of the 0F map (from the
/// reference's sweep, `tests/lengths.txt`; Intel's opcode map where a byte is a prefix or the instruction
/// faults however short it is). The 0F38 map counts a ModRM after every opcode, the 0F3A map a ModRM and
/// an 8-bit immediate.
const ONE_BYTE: [After; OPCODES] = kinds(concat!(
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
));
const TWO_BYTE: [After; OPCODES] = kinds(concat!(
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
));

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
            OPERAND_SIZE => operand16 = true,
            ADDRESS_SIZE => address32 = true,
            _ if is_rex(b) => {
                rex = Some(b);
                continue;
            }
            _ => {}
        }
        rex = None;
    }
    let body = &code[prefixes..];
    let first = *body.first()?;
    let vex = matches!(first, VEX3 | VEX2 | EVEX);
    let wide = !vex && rex.is_some_and(|r| r & REX_W != 0);
    let operand16 = operand16 && !vex && !wide;
    let (map, at) = match (first, body.get(1)) {
        (VEX2, _) => (MAP_0F, VEX2_LENGTH),
        (VEX3 | EVEX, Some(&fields)) if fields & VEX_MAP_LOW != 0 => (
            fields & VEX_MAP_LOW,
            if first == VEX3 {
                VEX3_LENGTH
            } else {
                EVEX_LENGTH
            },
        ),
        (VEX3 | EVEX, _) => {
            let length = prefixes + 1 + modrm_length(body.get(1..)?)?;
            return (length <= code.len()).then_some(length);
        }
        (ESCAPE, Some(&escape)) if ESCAPES_THREE_BYTE.contains(&escape) => {
            let map = if escape & ESCAPE_0F3A != 0 {
                MAP_0F3A
            } else {
                MAP_0F38
            };
            (map, 2)
        }
        (ESCAPE, _) => (MAP_0F, 1),
        _ => (MAP_ONE_BYTE, 0),
    };
    let opcode = *body.get(at)?;
    let rest = body.get(at + 1..)?;
    let kind = match map {
        MAP_ONE_BYTE => ONE_BYTE[usize::from(opcode)],
        MAP_0F if vex && ESCAPES_THREE_BYTE.contains(&opcode) => After::Nothing,
        MAP_0F => TWO_BYTE[usize::from(opcode)],
        MAP_0F38 => After::ModRm,
        _ => After::ModRmImm8,
    };
    let z = if operand16 {
        size_of::<u16>()
    } else {
        size_of::<u32>()
    };
    let modrm = || modrm_length(rest);
    let reg_low = || rest.first().map(|m| m >> MODRM_REG_SHIFT & MODRM_FIELD < 2);
    let after = match kind {
        After::Nothing => 0,
        After::ModRm => modrm()?,
        After::ModRmImm8 => modrm()? + size_of::<u8>(),
        After::ModRmImmZ => modrm()? + z,
        After::ModRmIgnored | After::Imm8 => size_of::<u8>(),
        After::Imm16 => size_of::<u16>(),
        After::Imm16Imm8 => size_of::<u16>() + size_of::<u8>(),
        After::ImmZ => z,
        After::Imm32 => size_of::<u32>(),
        After::ImmV => {
            if wide {
                size_of::<u64>()
            } else {
                z
            }
        }
        After::FarPointer => z + size_of::<u16>(),
        After::Address => {
            if address32 {
                size_of::<u32>()
            } else {
                size_of::<u64>()
            }
        }
        After::GroupImm8 => modrm()? + if reg_low()? { size_of::<u8>() } else { 0 },
        After::GroupImmZ => modrm()? + if reg_low()? { z } else { 0 },
    };
    let length = prefixes + at + 1 + after;
    (length <= code.len()).then_some(length)
}

/// How many bytes a ModRM at the start of `rest` takes with the SIB and displacement it asks for, `None`
/// when they run past the end. 32-bit addressing (67) lays them out the same in 64-bit mode.
fn modrm_length(rest: &[u8]) -> Option<usize> {
    let modrm = *rest.first()?;
    let (mode, rm) = (modrm >> MODRM_MODE_SHIFT, modrm & MODRM_FIELD);
    let mut length = 1;
    if mode != MODE_REGISTER && rm == RM_SIB {
        let sib = *rest.get(1)?;
        length += 1;
        if mode == 0 && sib & MODRM_FIELD == RM_DISPLACEMENT32 {
            length += size_of::<u32>();
        }
    }
    length += match (mode, rm) {
        (MODE_DISPLACEMENT8, _) => size_of::<u8>(),
        (MODE_DISPLACEMENT32, _) | (0, RM_DISPLACEMENT32) => size_of::<u32>(),
        _ => 0,
    };
    Some(length)
}
