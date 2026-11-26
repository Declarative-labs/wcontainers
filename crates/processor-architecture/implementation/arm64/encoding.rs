//! Where operands sit in an instruction's encoding.

use core::convert::Infallible;

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

use crate::Instruction;
#[cfg(verus_keep_ghost)]
use crate::pseudocode::{ones, sint, uint};

verus! {

/// A run of bits in an instruction's encoding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Field {
    pub lowest_bit: u32,
    pub width: u32,
}

impl Field {
    /// Whether the field lies inside a 32-bit encoding.
    pub open spec fn fits(self) -> bool {
        self.lowest_bit < u32::BITS && self.lowest_bit + self.width <= u32::BITS
    }

    pub const fn new(lowest_bit: u32, width: u32) -> Self {
        Self { lowest_bit, width }
    }

    pub const fn to_mask(self) -> (mask: u32)
        requires
            self.fits(),
        ensures
            mask == ((ones(self.width as nat) as u32) << self.lowest_bit),
    {
        proof {
            vstd::arithmetic::power2::lemma2_to64();
            if self.width < 32 {
                vstd::arithmetic::power2::lemma_pow2_strictly_increases(self.width as nat, 32);
            }
            vstd::bits::lemma_u64_shl_is_mul(1, self.width as u64);
            let width = self.width;
            assert(1u64 << width == 1u64 << (width as u64)) by (bit_vector);
        }
        (((1_u64 << self.width) - 1) as u32) << self.lowest_bit
    }

    /// The field's bits of the word, `word<lowest_bit + width - 1:lowest_bit>`.
    pub const fn read(self, word: u32) -> (value: u32)
        requires
            self.fits(),
        ensures
            value as int == uint(word as int / (pow2(self.lowest_bit as nat) as int), self.width as nat),
    {
        let mask = self.to_mask();
        proof {
            let (low, width) = (self.lowest_bit, self.width);
            vstd::arithmetic::power2::lemma2_to64();
            let ones_mask = ones(width as nat) as u32;
            assert((word & (ones_mask << low)) >> low == (word >> low) & ones_mask) by (bit_vector);
            vstd::bits::lemma_u32_shr_is_div(word, low);
            crate::lemmas::lemma_mask_32(word >> low, width as nat);
        }
        (word & mask) >> self.lowest_bit
    }

    /// Reads the field as a two's complement number.
    pub const fn read_signed(self, word: u32) -> (value: i64)
        requires
            self.fits(),
            0 < self.width,
        ensures
            value as int == sint(word as int / (pow2(self.lowest_bit as nat) as int), self.width as nat),
    {
        let bits_above = i64::BITS - self.width;
        let unsigned = self.read(word);
        proof {
            let wide = unsigned as u64;
            assert((unsigned as i64) << bits_above == (wide << bits_above) as i64) by (bit_vector)
                requires
                    bits_above < 64,
                    wide == unsigned as u64;
            crate::lemmas::lemma_sign_extension(wide, bits_above);
            let field = word as int / (pow2(self.lowest_bit as nat) as int);
            vstd::arithmetic::power2::lemma_pow2_pos(self.width as nat);
            vstd::arithmetic::div_mod::lemma_mod_twice(field, pow2(self.width as nat) as int);
        }
        ((unsigned as i64) << bits_above) >> bits_above
    }
}

} // verus!

impl Field {
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

impl FixedBits {
    pub const fn is_matching(self, word: u32) -> bool {
        word & self.mask == self.value
    }
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

impl Instruction<Infallible> {
    pub const fn to_fixed_bits(self) -> FixedBits {
        match self.to_record() {
            Some(record) => record.fixed_bits,
            // MRS with op0 3, op1 3, CRn 0, CRm 0, op2 1: https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CTR-EL0--Cache-Type-Register#:~:text=Accessing%20CTR_EL0
            None => FixedBits {
                mask: 0xFFFF_FFE0,
                value: 0xD53B_0020,
            },
        }
    }
}
