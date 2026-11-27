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

    /// The field's bits of the word, `UInt(word<lowest_bit + width - 1:lowest_bit>)`.
    pub open spec fn value(self, word: u32) -> nat {
        uint(word as int / (pow2(self.lowest_bit as nat) as int), self.width as nat) as nat
    }

    /// The field's bits of the word are below 2^bits where the field holds at most `bits` bits.
    pub proof fn lemma_value_below(self, word: u32, bits: nat)
        requires
            self.width <= bits,
        ensures
            self.value(word) < pow2(bits),
    {
        vstd::arithmetic::power2::lemma_pow2_pos(self.lowest_bit as nat);
        vstd::arithmetic::power2::lemma_pow2_pos(self.width as nat);
        vstd::arithmetic::div_mod::lemma_div_pos_is_pos(word as int, pow2(self.lowest_bit as nat) as int);
        vstd::arithmetic::div_mod::lemma_mod_pos_bound(
            word as int / (pow2(self.lowest_bit as nat) as int),
            pow2(self.width as nat) as int,
        );
        if self.width < bits {
            vstd::arithmetic::power2::lemma_pow2_strictly_increases(self.width as nat, bits);
        }
    }

    pub const fn is_inside(self) -> (inside: bool)
        ensures
            inside == self.fits(),
    {
        self.lowest_bit < u32::BITS && self.width <= u32::BITS - self.lowest_bit
    }
}

/// Whether every field lies inside a 32-bit encoding.
pub open spec fn fields_fit(fields: Seq<Field>) -> bool {
    forall|index: int| 0 <= index < fields.len() ==> #[trigger] fields[index].fits()
}

/// The bits the fields hold together.
pub open spec fn fields_width(fields: Seq<Field>) -> nat
    decreases fields.len(),
{
    if fields.len() == 0 { 0 } else { fields_width(fields.drop_last()) + fields.last().width as nat }
}

/// The fields' bits of the word one after the other, the first the most significant: `field0:field1:...`.
pub open spec fn concatenation(word: u32, fields: Seq<Field>) -> nat
    decreases fields.len(),
{
    if fields.len() == 0 {
        0
    } else {
        concatenation(word, fields.drop_last()) * pow2(fields.last().width as nat) + fields.last().value(word)
    }
}

/// The fields of a prefix are at most as wide as all of them.
pub proof fn lemma_fields_width_prefix(fields: Seq<Field>, count: int)
    requires
        0 <= count <= fields.len(),
    ensures
        fields_width(fields.take(count)) <= fields_width(fields),
    decreases fields.len() - count,
{
    if count < fields.len() {
        lemma_fields_width_prefix(fields, count + 1);
        assert(fields.take(count + 1).drop_last() =~= fields.take(count));
    } else {
        assert(fields.take(count) =~= fields);
    }
}

/// Whether every field lies inside a 32-bit encoding and together they hold between `least` and 64 bits.
pub fn fields_fit_within(fields: &[Field], least: u32) -> (within: bool)
    ensures
        within == (fields_fit(fields@) && least <= fields_width(fields@) <= 64),
{
    let mut width: u32 = 0;
    let mut index: usize = 0;
    while index < fields.len()
        invariant
            index <= fields.len(),
            fields_fit(fields@.take(index as int)),
            width == fields_width(fields@.take(index as int)),
            width <= 64,
        decreases fields.len() - index,
    {
        let field = fields[index];
        proof {
            assert(fields@.take(index + 1).drop_last() =~= fields@.take(index as int));
            assert(fields@.take(index + 1).last() == field);
            lemma_fields_width_prefix(fields@, index + 1);
        }
        if !field.is_inside() {
            proof {
                assert(!fields@[index as int].fits());
            }
            return false;
        }
        if width + field.width > 64 {
            return false;
        }
        width += field.width;
        index += 1;
        proof {
            assert forall|i: int| 0 <= i < fields@.take(index as int).len() implies #[trigger] fields@.take(index as int)[i].fits() by {
                if i < index - 1 {
                    assert(fields@.take(index as int)[i] == fields@.take(index - 1)[i]);
                }
            }
        }
    }
    proof {
        assert(fields@.take(index as int) =~= fields@);
    }
    width >= least
}

/// The fields' bits of the word one after the other, the first the most significant, and how many bits they hold.
pub fn concatenate(word: u32, fields: &[Field]) -> (concatenated: (u64, u32))
    requires
        fields_fit(fields@),
        fields_width(fields@) <= 64,
    ensures
        concatenated.0 as nat == concatenation(word, fields@),
        concatenated.1 as nat == fields_width(fields@),
        (concatenated.0 as nat) < pow2(concatenated.1 as nat),
{
    let mut bits: u64 = 0;
    let mut width: u32 = 0;
    let mut index: usize = 0;
    proof {
        vstd::arithmetic::power2::lemma2_to64();
    }
    while index < fields.len()
        invariant
            index <= fields.len(),
            fields_fit(fields@),
            fields_width(fields@) <= 64,
            bits as nat == concatenation(word, fields@.take(index as int)),
            width as nat == fields_width(fields@.take(index as int)),
            (bits as nat) < pow2(width as nat),
        decreases fields.len() - index,
    {
        let field = fields[index];
        proof {
            assert(fields@.take(index + 1).drop_last() =~= fields@.take(index as int));
            assert(fields@.take(index + 1).last() == field);
            lemma_fields_width_prefix(fields@, index + 1);
            assert(fields@[index as int].fits());
        }
        let value = field.read(word);
        proof {
            let field_value = word as int / (pow2(field.lowest_bit as nat) as int);
            vstd::arithmetic::power2::lemma_pow2_pos(field.lowest_bit as nat);
            vstd::arithmetic::power2::lemma_pow2_pos(field.width as nat);
            vstd::arithmetic::div_mod::lemma_div_pos_is_pos(word as int, pow2(field.lowest_bit as nat) as int);
            vstd::arithmetic::div_mod::lemma_mod_pos_bound(field_value, pow2(field.width as nat) as int);
            crate::lemmas::lemma_concatenate(bits, value as u64, width as nat, field.width);
        }
        // VERUS: no specification links `|` to `+`, so the field's bits are added below the shifted bits, which are zero
        // there.
        bits = (bits << field.width) + value as u64;
        width += field.width;
        index += 1;
    }
    proof {
        assert(fields@.take(index as int) =~= fields@);
    }
    (bits, width)
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
