//! The instruction at the start of some bytes, from iced-x86, with what iced does not say: the address size,
//! and the length the CPU counts for an encoding longer than 15 bytes.

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
    if decoder.last_error() == DecoderError::NoMoreBytes {
        return None;
    }
    let prefixes = prefix_run(code);
    let address32 = code[..prefixes].contains(&0x67);
    let mut too_long = false;
    // iced stops reading at 15 bytes; the CPU counts on, and its limit wins over every other rule.
    if iced.is_invalid() && iced.len() == 15 {
        let length = cpu_length(code, prefixes)?;
        if length > 15 {
            too_long = true;
            iced.set_len(length.min(255));
        }
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

/// The length the CPU counts for an encoding iced reports as 15 invalid bytes: the prefixes that can change
/// what follows (the last 66, the last 67, the last of F2 and F3, and a REX directly before the opcode) are
/// kept, the rest are counted and dropped, and iced decodes what is left. Before a VEX or EVEX instruction
/// (C4, C5 or 62 in 64-bit mode) only 67 changes its length.
fn cpu_length(code: &[u8], prefixes: usize) -> Option<usize> {
    let (mut operand, mut address, mut repeat, mut rex) = (false, false, None, None);
    for &b in &code[..prefixes] {
        match b {
            0x66 => operand = true,
            0x67 => address = true,
            0xf2 | 0xf3 => repeat = Some(b),
            0x40..=0x4f => {
                rex = Some(b);
                continue;
            }
            _ => {}
        }
        rex = None;
    }
    let body = code.get(prefixes..)?;
    if matches!(body.first(), Some(0xc4 | 0xc5 | 0x62)) {
        (operand, repeat, rex) = (false, None, None);
    }
    let mut rest: Vec<u8> = Vec::with_capacity(4 + body.len().min(15));
    rest.extend(operand.then_some(0x66));
    rest.extend(address.then_some(0x67));
    rest.extend(repeat);
    rest.extend(rex);
    let kept = rest.len();
    rest.extend(&body[..body.len().min(15)]);
    let mut decoder = Decoder::with_ip(64, &rest, 0, options());
    let instruction = decoder.decode();
    if decoder.last_error() == DecoderError::NoMoreBytes {
        return None;
    }
    Some(prefixes - kept + instruction.len())
}
