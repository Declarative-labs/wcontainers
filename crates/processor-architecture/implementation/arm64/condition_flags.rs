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
    pub fn holds(self, flags: ConditionFlags) -> bool {
        let negative = flags.contains(ConditionFlags::NEGATIVE);
        let zero = flags.contains(ConditionFlags::ZERO);
        let carry = flags.contains(ConditionFlags::CARRY);
        let overflow = flags.contains(ConditionFlags::OVERFLOW);
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
            Self::Al | Self::Nv => true,
        }
    }
}
