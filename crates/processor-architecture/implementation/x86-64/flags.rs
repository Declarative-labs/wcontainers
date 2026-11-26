bitflags::bitflags! {
    /// The bits of [RFLAGS](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) a program at ring 3 can change, at their places in RFLAGS.
    ///
    /// IF is always set and IOPL always 0, as Linux runs a program; RF, VM, VIF and VIP are always clear at ring 3
    /// as a program can read them. TF is not modeled yet: the chip source uses it to step.
    ///
    /// These are exactly the bits a state may set. A state whose RFLAGS sets any other bit (TF, RF, a reserved bit)
    /// is invalid, not normalized: the chip source refuses it before it runs anything, and a simulation of it means
    /// nothing. Every bit here a ring 3 program can set with POPF, and every one of them is a critical value.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Flags: u64 {
        const CARRY = 1 << 0;
        const PARITY = 1 << 2;
        const AUXILIARY_CARRY = 1 << 4;
        const ZERO = 1 << 6;
        const SIGN = 1 << 7;
        const DIRECTION = 1 << 10;
        const OVERFLOW = 1 << 11;
        const NESTED_TASK = 1 << 14;
        const ALIGNMENT_CHECK = 1 << 18;
        const IDENTIFICATION = 1 << 21;
    }
}

impl Flags {
    /// The six status flags the arithmetic instructions write.
    pub const STATUS: Self = Self::CARRY
        .union(Self::PARITY)
        .union(Self::AUXILIARY_CARRY)
        .union(Self::ZERO)
        .union(Self::SIGN)
        .union(Self::OVERFLOW);
}
