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
