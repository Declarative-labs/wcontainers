//! The conditions of `jcc`, `setcc` and `cmovcc`, and whether one holds for given rflags in plain form, for code
//! that decides one without emitting the meaning: the inside and the handler of a link's `int3`.

use crate::{CF, OF, PF, SF, ZF};

/// The condition of a `jcc`, `setcc` or `cmovcc`, in the order of its encoding (the low four bits of the
/// opcode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Overflow,
    NotOverflow,
    Below,
    AboveOrEqual,
    Equal,
    NotEqual,
    BelowOrEqual,
    Above,
    Sign,
    NotSign,
    Parity,
    NotParity,
    Less,
    GreaterOrEqual,
    LessOrEqual,
    Greater,
}

/// What a condition tests; a condition holds when the test does, or is its negation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Test {
    Overflow,
    Below,
    Equal,
    BelowOrEqual,
    Sign,
    Parity,
    Less,
    LessOrEqual,
}

impl Condition {
    pub const fn test(self) -> Test {
        use Condition::*;
        match self {
            Overflow | NotOverflow => Test::Overflow,
            Below | AboveOrEqual => Test::Below,
            Equal | NotEqual => Test::Equal,
            BelowOrEqual | Above => Test::BelowOrEqual,
            Sign | NotSign => Test::Sign,
            Parity | NotParity => Test::Parity,
            Less | GreaterOrEqual => Test::Less,
            LessOrEqual | Greater => Test::LessOrEqual,
        }
    }

    /// Whether it holds when its test fails.
    pub const fn negated(self) -> bool {
        use Condition::*;
        matches!(
            self,
            NotOverflow
                | AboveOrEqual
                | NotEqual
                | Above
                | NotSign
                | NotParity
                | GreaterOrEqual
                | Greater
        )
    }
}

impl Condition {
    /// Every condition, by its code.
    pub const ALL: [Condition; 16] = [
        Condition::Overflow,
        Condition::NotOverflow,
        Condition::Below,
        Condition::AboveOrEqual,
        Condition::Equal,
        Condition::NotEqual,
        Condition::BelowOrEqual,
        Condition::Above,
        Condition::Sign,
        Condition::NotSign,
        Condition::Parity,
        Condition::NotParity,
        Condition::Less,
        Condition::GreaterOrEqual,
        Condition::LessOrEqual,
        Condition::Greater,
    ];

    /// The condition a `jcc`'s low four bits name.
    pub const fn from_code(code: u8) -> Self {
        Self::ALL[(code as usize) % Self::ALL.len()]
    }

    /// Whether the condition holds for these rflags, as `condition_from_flags` computes it.
    pub fn holds(self, rflags: u64) -> bool {
        let set = |flag: u64| rflags & flag != 0;
        let tested = match self.test() {
            Test::Overflow => set(OF),
            Test::Below => set(CF),
            Test::Equal => set(ZF),
            Test::BelowOrEqual => set(CF) || set(ZF),
            Test::Sign => set(SF),
            Test::Parity => set(PF),
            Test::Less => set(SF) != set(OF),
            Test::LessOrEqual => set(ZF) || set(SF) != set(OF),
        };
        tested != self.negated()
    }
}

#[cfg(test)]
mod tests {
    use super::Condition;
    use crate::{CF, OF, PF, SF, ZF};

    /// Every condition on every combination of the five flags it can read, against the SDM's table (Vol. 2,
    /// Jcc), by code.
    #[test]
    fn every_condition_on_every_flag_combination() {
        let flags = [CF, PF, ZF, SF, OF];
        for combination in 0..1u32 << flags.len() {
            let rflags: u64 = flags
                .iter()
                .enumerate()
                .filter(|(k, _)| combination & 1 << k != 0)
                .map(|(_, f)| f)
                .sum();
            let [c, p, z, s, o] = flags.map(|f| rflags & f != 0);
            let table = [
                o,
                !o,
                c,
                !c,
                z,
                !z,
                c || z,
                !c && !z,
                s,
                !s,
                p,
                !p,
                s != o,
                s == o,
                z || s != o,
                !z && s == o,
            ];
            for (code, expected) in table.into_iter().enumerate() {
                let condition = Condition::from_code(code as u8);
                assert_eq!(condition, Condition::ALL[code]);
                assert_eq!(
                    condition.holds(rflags),
                    expected,
                    "{condition:?} with {rflags:#x}"
                );
            }
        }
    }
}
