//! The images of the x87 state in memory, as the [manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)
//! (vol. 1, 8.1.10 and 10.5.1) lays them out in 64-bit mode: FNSTENV's and FLDENV's environment in the protected-mode
//! format of 28 bytes, or of 14 under the operand-size prefix; FNSAVE's and FRSTOR's environment followed by ST(0) to
//! ST(7); FXSAVE's 512 bytes with XMM0 to XMM15; and the 18 packed decimal digits of FBLD and FBSTP.
//!
//! This chip deprecates FCS and FDS (CPUID leaf 7's EBX bit 13), so it stores them as zeros.

use alloc::vec;
use alloc::vec::Vec;

use crate::X87;
use crate::x87::{Class, classify};

/// The 2-bit tag of a register in use: valid, zero, or special (a NaN, an infinity, a denormal or a format no
/// operation takes); an empty register's is 3.
pub fn to_tag(x87: &X87, physical: usize) -> u16 {
    if x87.tags & (1 << physical) == 0 {
        return 3;
    }
    match classify(x87.registers[physical]) {
        Class::Normal => 0,
        Class::Zero => 1,
        _ => 2,
    }
}

/// The full tag word: each physical register's tag at twice its number.
pub fn to_tag_word(x87: &X87) -> u16 {
    (0..8).fold(0, |word, physical| word | to_tag(x87, physical) << (2 * physical))
}

/// The abridged tags a full tag word gives: a register is in use unless its tag is 3.
pub fn from_tag_word(word: u16) -> u8 {
    (0..8).fold(0, |tags, physical| if word >> (2 * physical) & 3 == 3 { tags } else { tags | 1 << physical })
}

/// The environment's bytes: 28 in the 32-bit format, 14 in the 16-bit one; the bits above each 16-bit field of the
/// 32-bit format hold `filler`.
pub fn write_environment(x87: &X87, bytes: usize, filler: u16) -> Vec<u8> {
    let mut image = Vec::with_capacity(bytes);
    let mut push = |value: u16| image.extend_from_slice(&value.to_le_bytes());
    if bytes == 14 {
        for value in [x87.control, x87.status, to_tag_word(x87), x87.instruction_pointer as u16, 0, x87.data_pointer as u16, 0] {
            push(value);
        }
    } else {
        for value in [x87.control, filler, x87.status, filler, to_tag_word(x87), filler] {
            push(value);
        }
        push(x87.instruction_pointer as u16);
        push((x87.instruction_pointer >> 16) as u16);
        push(0);
        push(x87.opcode);
        push(x87.data_pointer as u16);
        push((x87.data_pointer >> 16) as u16);
        push(0);
        push(filler);
    }
    image
}

/// The x87 state an environment image of `bytes` (14 or 28) loads, the registers kept: FCW with its reserved bits as
/// FLDCW keeps them, FSW with ES and B set where a flag FCW leaves unmasked is set and clear elsewhere, the tags from
/// the full tag word, and the pointers zero-extended; the 16-bit format has no opcode, so FOP becomes 0.
pub fn read_environment(x87: &X87, image: &[u8], bytes: usize) -> X87 {
    let word = |index: usize| u16::from_le_bytes([image[index], image[index + 1]]);
    let mut loaded = *x87;
    if bytes == 28 {
        loaded.control = word(0) & 0x1F3F | 0x0040;
        loaded.status = word(4);
        loaded.tags = from_tag_word(word(8));
        loaded.instruction_pointer = u64::from(word(12)) | u64::from(word(14)) << 16;
        loaded.opcode = word(18) & 0x07FF;
        loaded.data_pointer = u64::from(word(20)) | u64::from(word(22)) << 16;
    } else {
        loaded.control = word(0) & 0x1F3F | 0x0040;
        loaded.status = word(2);
        loaded.tags = from_tag_word(word(4));
        loaded.instruction_pointer = u64::from(word(6));
        loaded.data_pointer = u64::from(word(10));
        loaded.opcode = 0;
    }
    set_summary(&mut loaded);
    loaded
}

/// Sets FSW's ES and B where a flag FCW leaves unmasked is set, and clears them elsewhere.
pub fn set_summary(x87: &mut X87) {
    const SUMMARY: u16 = 1 << 7 | 1 << 15;
    let pending = x87.status & !x87.control & 0x3F != 0;
    x87.status = if pending { x87.status | SUMMARY } else { x87.status & !SUMMARY };
}

/// ST(0) to ST(7), ten bytes each, after an environment.
pub fn write_registers(x87: &X87) -> Vec<u8> {
    (0..8).flat_map(|index| x87.read(index).to_le_bytes()[..10].to_vec()).collect()
}

/// Loads ST(0) to ST(7) from ten bytes each, by the TOP the state holds.
pub fn read_registers(x87: &mut X87, image: &[u8]) {
    for index in 0..8 {
        let mut bytes = [0; 16];
        bytes[..10].copy_from_slice(&image[10 * index..10 * index + 10]);
        let physical = x87.to_physical(index);
        x87.registers[physical] = u128::from_le_bytes(bytes);
    }
}

/// The bytes of FXSAVE's area up to the end of XMM15, with MXCSR and its mask; FIP and FDP 64-bit for FXSAVE64, else
/// 32-bit beside a selector of zero; the reserved bytes zero.
pub fn write_extended(x87: &X87, mxcsr: u32, mask: u32, vectors: &[u128; 16], wide: bool) -> Vec<u8> {
    let mut image = vec![0u8; 416];
    image[0..2].copy_from_slice(&x87.control.to_le_bytes());
    image[2..4].copy_from_slice(&x87.status.to_le_bytes());
    image[4] = x87.tags;
    image[6..8].copy_from_slice(&x87.opcode.to_le_bytes());
    if wide {
        image[8..16].copy_from_slice(&x87.instruction_pointer.to_le_bytes());
        image[16..24].copy_from_slice(&x87.data_pointer.to_le_bytes());
    } else {
        image[8..12].copy_from_slice(&(x87.instruction_pointer as u32).to_le_bytes());
        image[16..20].copy_from_slice(&(x87.data_pointer as u32).to_le_bytes());
    }
    image[24..28].copy_from_slice(&mxcsr.to_le_bytes());
    image[28..32].copy_from_slice(&mask.to_le_bytes());
    for index in 0..8 {
        let at = 32 + 16 * index;
        image[at..at + 10].copy_from_slice(&x87.read(index).to_le_bytes()[..10]);
    }
    for (number, value) in vectors.iter().enumerate() {
        let at = 160 + 16 * number;
        image[at..at + 16].copy_from_slice(&value.to_le_bytes());
    }
    image
}

/// The x87 state, MXCSR and XMM0 to XMM15 FXRSTOR loads from the area's first 416 bytes, FCW as FLDCW keeps it.
pub fn read_extended(image: &[u8], wide: bool) -> (X87, u32, [u128; 16]) {
    let word = |index: usize| u16::from_le_bytes([image[index], image[index + 1]]);
    let quad = |index: usize| u64::from_le_bytes(image[index..index + 8].try_into().expect("eight bytes"));
    let double = |index: usize| u32::from_le_bytes(image[index..index + 4].try_into().expect("four bytes"));
    let mut x87 = X87 {
        registers: [0; 8],
        control: word(0) & 0x1F3F | 0x0040,
        status: word(2),
        tags: image[4],
        opcode: word(6) & 0x07FF,
        instruction_pointer: if wide { quad(8) } else { u64::from(double(8)) },
        data_pointer: if wide { quad(16) } else { u64::from(double(16)) },
    };
    for index in 0..8 {
        let mut bytes = [0; 16];
        bytes[..10].copy_from_slice(&image[32 + 16 * index..42 + 16 * index]);
        let physical = x87.to_physical(index);
        x87.registers[physical] = u128::from_le_bytes(bytes);
    }
    let vectors = core::array::from_fn(|number| u128::from_le_bytes(image[160 + 16 * number..176 + 16 * number].try_into().expect("sixteen bytes")));
    (x87, double(24), vectors)
}

/// The integer 18 packed decimal digits give, each nibble weighed as its own value even above 9, and the sign.
pub fn read_decimal(bytes: u128) -> (bool, u64) {
    let magnitude = (0..18).rev().fold(0u64, |value, digit| value.wrapping_mul(10).wrapping_add((bytes >> (4 * digit) & 0xF) as u64));
    (bytes >> 79 & 1 != 0, magnitude)
}

/// The packed decimal digits of a magnitude below 10 to the 18th, with the sign in the last byte's top bit.
pub fn write_decimal(negative: bool, magnitude: u64) -> u128 {
    let mut rest = magnitude;
    let mut bytes = 0u128;
    for digit in 0..18 {
        bytes |= u128::from(rest % 10) << (4 * digit);
        rest /= 10;
    }
    bytes | u128::from(negative) << 79
}

/// FBSTP's packed decimal indefinite.
pub const DECIMAL_INDEFINITE: u128 = 0xFFFF_C000_0000_0000_0000;
