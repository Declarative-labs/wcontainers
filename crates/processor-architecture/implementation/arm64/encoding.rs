//! Where the operands of the ten instructions sit in their encodings.

use crate::Instruction;

/// A run of bits in an instruction's encoding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Field {
    pub lowest_bit: u32,
    pub width: u32,
}

impl Field {
    pub const fn to_mask(self) -> u32 {
        (((1_u64 << self.width) - 1) as u32) << self.lowest_bit
    }

    pub const fn read(self, word: u32) -> u32 {
        (word & self.to_mask()) >> self.lowest_bit
    }

    /// Reads the field as a two's complement number.
    pub const fn read_signed(self, word: u32) -> i64 {
        let bits_above = i64::BITS - self.width;
        ((self.read(word) as i64) << bits_above) >> bits_above
    }

    /// Places the low bits of the value in the field, dropping the rest.
    pub const fn to_bits(self, value: u32) -> u32 {
        (value << self.lowest_bit) & self.to_mask()
    }
}

/// The bits every encoding of an instruction shares.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FixedBits {
    pub mask: u32,
    pub value: u32,
}

pub const RD: Field = Field {
    lowest_bit: 0,
    width: 5,
};
pub const RT: Field = Field {
    lowest_bit: 0,
    width: 5,
};
pub const RN: Field = Field {
    lowest_bit: 5,
    width: 5,
};
pub const RT2: Field = Field {
    lowest_bit: 10,
    width: 5,
};
pub const RM: Field = Field {
    lowest_bit: 16,
    width: 5,
};
/// The shift type of a shifted register operand.
pub const SHIFT: Field = Field {
    lowest_bit: 22,
    width: 2,
};
/// The shift amount of a shifted register operand.
pub const SHIFT_IMM6: Field = Field {
    lowest_bit: 10,
    width: 6,
};
pub const COND: Field = Field {
    lowest_bit: 12,
    width: 4,
};
/// The unsigned offset of a load or store, in units of its access size.
pub const IMM12: Field = Field {
    lowest_bit: 10,
    width: 12,
};
/// The signed byte offset of a post-indexed load or store.
pub const IMM9: Field = Field {
    lowest_bit: 12,
    width: 9,
};
/// The signed offset of a store pair, in units of one register.
pub const IMM7: Field = Field {
    lowest_bit: 15,
    width: 7,
};
/// The comment of a breakpoint.
pub const IMM16: Field = Field {
    lowest_bit: 5,
    width: 16,
};
/// The signed multiple of the vector length RDVL reads.
pub const RDVL_IMM6: Field = Field {
    lowest_bit: 5,
    width: 6,
};

impl Instruction {
    pub const fn to_fixed_bits(self) -> FixedBits {
        let (mask, value) = match self {
            // https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--shifted-register---Add-optionally-shifted-register-
            Self::AddShiftedRegister64 => (0xFF20_0000, 0x8B00_0000),
            // https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADDS--shifted-register---Add-optionally-shifted-register--setting-flags-
            Self::AddsShiftedRegister64 => (0xFF20_0000, 0xAB00_0000),
            // https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/CSEL--Conditional-select-
            Self::Csel64 => (0xFFE0_0C00, 0x9A80_0000),
            // https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate--#:~:text=Unsigned%20offset
            Self::LdrImmediateUnsignedOffset64 => (0xFFC0_0000, 0xF940_0000),
            // https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-register--immediate--#:~:text=Unsigned%20offset
            Self::StrImmediateUnsignedOffset64 => (0xFFC0_0000, 0xF900_0000),
            // https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STP--Store-pair-of-registers-#:~:text=Signed%20offset
            Self::StpSignedOffset64 => (0xFFC0_0000, 0xA900_0000),
            // https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--immediate---Load-register--immediate--#:~:text=Post%2Dindex
            Self::LdrImmediatePostIndex64 => (0xFFE0_0C00, 0xF840_0400),
            // https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BRK--Breakpoint-instruction-
            Self::Brk => (0xFFE0_001F, 0xD420_0000),
            // https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/RDVL--Read-multiple-of-vector-register-size-to-scalar-register-
            Self::Rdvl => (0xFFFF_F800, 0x04BF_5000),
            // MRS with op0 3, op1 3, CRn 0, CRm 0, op2 1: https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register#:~:text=Accessing%20CTR_EL0
            Self::MrsCtrEl0 => (0xFFFF_FFE0, 0xD53B_0020),
        };
        FixedBits { mask, value }
    }
}
