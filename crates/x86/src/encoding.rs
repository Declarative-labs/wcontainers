//! Bytes of the x86-64 instruction encoding (Intel SDM Vol. 2A chapter 2) that iced-x86 does not name.

/// Legacy prefixes (2.1.1): lock and the repeat prefixes, the segment overrides, operand and address size.
pub const LOCK: u8 = 0xf0;
pub const REPNE: u8 = 0xf2;
pub const REP: u8 = 0xf3;
pub const ES: u8 = 0x26;
pub const CS: u8 = 0x2e;
pub const SS: u8 = 0x36;
pub const DS: u8 = 0x3e;
pub const FS: u8 = 0x64;
pub const GS: u8 = 0x65;
pub const OPERAND_SIZE: u8 = 0x66;
pub const ADDRESS_SIZE: u8 = 0x67;
pub const LEGACY_PREFIXES: [u8; 11] = [
    ES,
    CS,
    SS,
    DS,
    FS,
    GS,
    OPERAND_SIZE,
    ADDRESS_SIZE,
    LOCK,
    REPNE,
    REP,
];

/// REX prefixes (2.2.1): `REX` with W, R, X and B in its low four bits.
pub const REX: u8 = 0x40;
pub const REX_W: u8 = 0x08;
pub const REX_BITS: u8 = 0x0f;

/// Whether `b` is a REX prefix (in 64-bit mode).
pub const fn is_rex(b: u8) -> bool {
    b & !REX_BITS == REX
}

/// The escape to the two-byte opcode map.
pub const ESCAPE: u8 = 0x0f;

/// Bytes after `ESCAPE` that lead a third opcode byte (Vol. 2A A.3); `ESCAPE_0F3A` set selects the 0F3A map,
/// clear the 0F38 map.
pub const ESCAPES_THREE_BYTE: core::ops::RangeInclusive<u8> = 0x38..=0x3f;
pub const ESCAPE_0F3A: u8 = 0x02;

/// The opcode maps by number (Vol. 2A 2.3.6.1, the VEX `mmmmm` values): one-byte, 0F, 0F38 and 0F3A.
pub const MAP_ONE_BYTE: u8 = 0;
pub const MAP_0F: u8 = 1;
pub const MAP_0F38: u8 = 2;
pub const MAP_0F3A: u8 = 3;

/// The two-byte and three-byte VEX prefixes and the EVEX prefix (2.3.5, 2.7.1), and how many bytes each
/// takes before the opcode.
pub const VEX2: u8 = 0xc5;
pub const VEX3: u8 = 0xc4;
pub const EVEX: u8 = 0x62;
pub const VEX2_LENGTH: usize = 2;
pub const VEX3_LENGTH: usize = 3;
pub const EVEX_LENGTH: usize = 4;
/// The low bits of the map field in the byte after `VEX3` or `EVEX` that the CPU's length decoder reads.
pub const VEX_MAP_LOW: u8 = 0x03;

/// ModRM (2.1.5): its fields, the mode of a register operand, the r/m that asks for a SIB and the r/m
/// (or SIB base) that with mode 0 means a 32-bit displacement and no register.
pub const MODRM_MODE_SHIFT: u8 = 6;
pub const MODRM_REG_SHIFT: u8 = 3;
pub const MODRM_FIELD: u8 = 0x07;
pub const MODE_DISPLACEMENT8: u8 = 1;
pub const MODE_DISPLACEMENT32: u8 = 2;
pub const MODE_REGISTER: u8 = 3;
pub const RM_SIB: u8 = 4;
pub const RM_DISPLACEMENT32: u8 = 5;
