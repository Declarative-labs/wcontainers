use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use crate::pseudocode::condition_holds;

bitflags::bitflags! {
    /// PSTATE's [NZCV](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/NZCV--Condition-Flags), at its bit positions in that register.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct ConditionFlags: u32 {
        const NEGATIVE = 1 << 31;
        const ZERO = 1 << 30;
        const CARRY = 1 << 29;
        const OVERFLOW = 1 << 28;
    }
}

verus! {

/// The bit of V in an NZCV immediate, which holds N, Z, C and V from its most significant bit.
pub const IMMEDIATE_OVERFLOW: u64 = 1;
pub const IMMEDIATE_CARRY: u64 = IMMEDIATE_OVERFLOW << 1;
pub const IMMEDIATE_ZERO: u64 = IMMEDIATE_CARRY << 1;
pub const IMMEDIATE_NEGATIVE: u64 = IMMEDIATE_ZERO << 1;

/// PSTATE's NZCV as one boolean per flag, the form in which verified code reads and sets them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Nzcv {
    pub negative: bool,
    pub zero: bool,
    pub carry: bool,
    pub overflow: bool,
}

} // verus!

// VERUS: Verus cannot read a `bitflags` constant, so verified code holds the flags as `Nzcv` and converts here.
impl From<Nzcv> for ConditionFlags {
    fn from(nzcv: Nzcv) -> Self {
        let mut flags = Self::empty();
        flags.set(Self::NEGATIVE, nzcv.negative);
        flags.set(Self::ZERO, nzcv.zero);
        flags.set(Self::CARRY, nzcv.carry);
        flags.set(Self::OVERFLOW, nzcv.overflow);
        flags
    }
}

impl From<ConditionFlags> for Nzcv {
    fn from(flags: ConditionFlags) -> Self {
        Self {
            negative: flags.contains(ConditionFlags::NEGATIVE),
            zero: flags.contains(ConditionFlags::ZERO),
            carry: flags.contains(ConditionFlags::CARRY),
            overflow: flags.contains(ConditionFlags::OVERFLOW),
        }
    }
}

verus! {

/// The [conditions](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/shared-functions-system#impl-shared.ConditionHolds.1) in the order of their encodings.
pub const CONDITIONS: [Condition; 16] = [
    Condition::Eq,
    Condition::Ne,
    Condition::Cs,
    Condition::Cc,
    Condition::Mi,
    Condition::Pl,
    Condition::Vs,
    Condition::Vc,
    Condition::Hi,
    Condition::Ls,
    Condition::Ge,
    Condition::Lt,
    Condition::Gt,
    Condition::Le,
    Condition::Al,
    Condition::Nv,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Condition {
    Eq,
    Ne,
    Cs,
    Cc,
    Mi,
    Pl,
    Vs,
    Vc,
    Hi,
    Ls,
    Ge,
    Lt,
    Gt,
    Le,
    Al,
    Nv,
}

impl Condition {
    /// The condition's place in [`CONDITIONS`], which is its encoding.
    pub open spec fn encoding(self) -> nat {
        match self {
            Self::Eq => 0,
            Self::Ne => 1,
            Self::Cs => 2,
            Self::Cc => 3,
            Self::Mi => 4,
            Self::Pl => 5,
            Self::Vs => 6,
            Self::Vc => 7,
            Self::Hi => 8,
            Self::Ls => 9,
            Self::Ge => 10,
            Self::Lt => 11,
            Self::Gt => 12,
            Self::Le => 13,
            Self::Al => 14,
            Self::Nv => 15,
        }
    }

    /// Whether the condition holds in the flags.
    pub fn holds_in(self, flags: Nzcv) -> (holds: bool)
        ensures
            holds == condition_holds(self.encoding(), flags),
    {
        let negative = flags.negative;
        let zero = flags.zero;
        let carry = flags.carry;
        let overflow = flags.overflow;
        match self {
            Self::Eq => zero,
            Self::Ne => !zero,
            Self::Cs => carry,
            Self::Cc => !carry,
            Self::Mi => negative,
            Self::Pl => !negative,
            Self::Vs => overflow,
            Self::Vc => !overflow,
            Self::Hi => carry && !zero,
            Self::Ls => !(carry && !zero),
            Self::Ge => negative == overflow,
            Self::Lt => negative != overflow,
            Self::Gt => negative == overflow && !zero,
            Self::Le => !(negative == overflow && !zero),
            Self::Al => true,
            Self::Nv => !crate::planted::is(118),
        }
    }
}

} // verus!

impl Condition {
    pub fn holds(self, flags: ConditionFlags) -> bool {
        self.holds_in(Nzcv::from(flags))
    }
}
