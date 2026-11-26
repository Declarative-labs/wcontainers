//! The operands of an instruction's assembler syntax and the fields of its encoding that hold them.

#[cfg(verus_keep_ghost)]
use vstd::arithmetic::power2::pow2;
use vstd::prelude::*;

use crate::encoding::Field;
use crate::floating_point::Precision;
#[cfg(verus_keep_ghost)]
use crate::pseudocode::ones;

/// One operand of an instruction's assembler syntax, as far as its encoding is modeled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operand {
    GeneralPurposeRegister {
        number: Field,
        width: RegisterWidth,
        thirty_one: RegisterThirtyOne,
        access: OperandAccess,
    },
    /// The low `precision` bits of a SIMD and floating-point register.
    FloatingPointRegister {
        number: Field,
        precision: Precision,
        access: OperandAccess,
    },
    /// A SIMD and floating-point register a load or store moves the low bytes of, as many as the access's size.
    VectorRegister {
        number: Field,
        access: OperandAccess,
    },
    /// A field LLVM leaves undecoded, which the manual fixes to zero.
    Ignored(Field),
    /// Two consecutive registers from an even number, the first at the lower address of a pair access.
    GeneralPurposeRegisterPair {
        number: Field,
        width: RegisterWidth,
    },
    /// An unsigned immediate split over fields, the most significant first.
    SplitImmediate(&'static [Field]),
    /// A register shifted by an immediate amount, the [shift types](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ORR--shifted-register---Bitwise-OR--shifted-register--#:~:text=shift) in the order of their encodings.
    ShiftedRegister {
        number: Field,
        shift_type: Field,
        amount: Field,
        width: RegisterWidth,
        rotation: Rotation,
    },
    /// A register [extended](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/ADD--extended-register---Add-extended-and-scaled-register-#:~:text=option) from its low byte, halfword, word or doubleword, then shifted left by up to four bits.
    ExtendedRegister {
        number: Field,
        extend_type: Field,
        amount: Field,
        width: RegisterWidth,
    },
    /// An unsigned 12-bit immediate, shifted left by 12 bits when its shift bit is set.
    ArithmeticImmediate {
        value: Field,
        shift: Field,
    },
    /// The N, immr and imms of a [bitmask immediate](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-bitmasks#impl-aarch64.DecodeBitMasks.5); a 32-bit encoding has no N.
    BitmaskImmediate {
        field: Field,
        width: RegisterWidth,
    },
    /// The hw field of a move wide: the 16-bit immediate goes to bit `16 * hw`.
    WideImmediateShift {
        field: Field,
        width: RegisterWidth,
    },
    UnsignedImmediate(Field),
    SignedImmediate(Field),
    /// A [condition](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-system#impl-shared.ConditionHolds.1) in the order of its encodings.
    Condition(Field),
    /// A signed offset from the instruction's address, its fields from the most significant, in units of `1 << scale` bytes.
    ProgramCounterOffset {
        fields: &'static [Field],
        scale: u32,
    },
    /// The base register of a load or store, and whether it writes the address back.
    AddressBase {
        number: Field,
        indexing: Indexing,
    },
    /// An immediate a load or store adds to its base, in units of `1 << scale` bytes.
    AddressOffset {
        field: Field,
        signedness: Signedness,
        scale: u32,
    },
    /// A register a load or store adds to its base: [option](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/LDR--register---Load-register--register--#:~:text=option)<2> extends it signed and
    /// option<0> takes it whole, each fixed where its field is absent, and S shifts it left by the access size's
    /// logarithm.
    RegisterOffset {
        number: Field,
        signed: Option<Field>,
        doubleword: Option<Field>,
        shift: Field,
    },
    /// TODO: temporary placeholder until every operand of every listed instruction is modeled.
    Unmodeled,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Signedness {
    Unsigned,
    Signed,
}

/// Where a load or store's address is, and what it writes back to its base.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Indexing {
    /// At the base plus the offset, which it does not write back.
    Offset,
    /// At the base plus the offset, which it writes back.
    PreIndex,
    /// At the base, after which it writes the base plus the offset back.
    PostIndex,
}

verus! {

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegisterWidth {
    Bits32,
    Bits64,
}

impl RegisterWidth {
    /// The number of bits of the width, as the specification reads it.
    pub open spec fn bits(self) -> nat {
        match self {
            Self::Bits32 => 32,
            Self::Bits64 => 64,
        }
    }

    pub const fn to_bits(self) -> (bits: u32)
        ensures
            bits == self.bits(),
    {
        match self {
            Self::Bits32 => u32::BITS,
            Self::Bits64 => u64::BITS,
        }
    }

    /// Ones in every bit of the width.
    pub const fn to_mask(self) -> (mask: u64)
        ensures
            mask == ones(self.bits()),
    {
        proof {
            assert(u64::MAX >> (u64::BITS - u32::BITS) == 0xffff_ffffu64) by (compute_only);
            assert(u64::MAX >> (u64::BITS - u64::BITS) == u64::MAX) by (compute_only);
            vstd::arithmetic::power2::lemma2_to64();
            vstd::bits::lemma_unsigned_int_max_is_pow2_minus_1();
        }
        u64::MAX >> (u64::BITS - self.to_bits())
    }
}

} // verus!

/// What register number 31 names in an operand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegisterThirtyOne {
    ZeroRegister,
    StackPointer,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandAccess {
    Read,
    Written,
}

/// Whether a shifted register may rotate, which the arithmetic instructions leave unallocated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rotation {
    Allowed,
    Reserved,
}
