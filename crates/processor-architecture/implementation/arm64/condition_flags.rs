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
