//! The near jumps, each a row of [`BRANCH_FORMS`]: a target relative to the next instruction or in a register,
//! taken always, on the flags or on RCX, and #GP(0) at the jump where a taken target is not canonical.

use iced_x86::Code;
use crate::forms::Instruction;

use crate::integer::Condition;

/// What decides whether a jump is taken.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    /// JMP.
    Always,
    /// Jcc.
    If(Condition),
    /// JRCXZ and JECXZ: the count register, RCX or ECX by the address size, is zero.
    CountIsZero,
    /// LOOP: the count register less one is not zero; it is written back first.
    Loop,
    /// LOOPE: as LOOP, and ZF is set.
    LoopWhileZero,
    /// LOOPNE: as LOOP, and ZF is clear.
    LoopWhileNotZero,
}

/// One near jump form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BranchForm {
    pub instruction: Instruction,
    pub decision: Decision,
    /// Whether the target is a register's value rather than relative to the next instruction.
    pub indirect: bool,
    pub codes: &'static [Code],
}

impl Decision {
    /// Whether the jump reads the count register.
    pub const fn counts(self) -> bool {
        matches!(self, Self::CountIsZero | Self::Loop | Self::LoopWhileZero | Self::LoopWhileNotZero)
    }
}

/// The row of a near jump form, or `None` for any other form.
pub fn find_branch_form(instruction: Instruction) -> Option<&'static BranchForm> {
    BRANCH_FORMS.iter().find(|form| form.instruction == instruction)
}

const fn relative(instruction: Instruction, decision: Decision, codes: &'static [Code]) -> BranchForm {
    BranchForm { instruction, decision, indirect: false, codes }
}

use Condition::{
    Below, BelowOrEqual, Less, LessOrEqual, NotBelow, NotBelowOrEqual, NotLess, NotLessOrEqual, NotOverflow,
    NotParity, NotSign, NotZero, Overflow, Parity, Sign, Zero,
};
use Decision::{Always, CountIsZero, If, Loop, LoopWhileNotZero, LoopWhileZero};

/// Every near jump form modeled so far.
pub static BRANCH_FORMS: &[BranchForm] = &[
    relative(Instruction::JmpRelbrb, Always, &[Code::Jmp_rel8_64]),
    relative(Instruction::JmpRelbrd, Always, &[Code::Jmp_rel32_64]),
    BranchForm { instruction: Instruction::JmpGprv, decision: Always, indirect: true, codes: &[Code::Jmp_rm64] },
    relative(Instruction::JoRelbrb, If(Overflow), &[Code::Jo_rel8_64]),
    relative(Instruction::JoRelbrd, If(Overflow), &[Code::Jo_rel32_64]),
    relative(Instruction::JnoRelbrb, If(NotOverflow), &[Code::Jno_rel8_64]),
    relative(Instruction::JnoRelbrd, If(NotOverflow), &[Code::Jno_rel32_64]),
    relative(Instruction::JbRelbrb, If(Below), &[Code::Jb_rel8_64]),
    relative(Instruction::JbRelbrd, If(Below), &[Code::Jb_rel32_64]),
    relative(Instruction::JnbRelbrb, If(NotBelow), &[Code::Jae_rel8_64]),
    relative(Instruction::JnbRelbrd, If(NotBelow), &[Code::Jae_rel32_64]),
    relative(Instruction::JzRelbrb, If(Zero), &[Code::Je_rel8_64]),
    relative(Instruction::JzRelbrd, If(Zero), &[Code::Je_rel32_64]),
    relative(Instruction::JnzRelbrb, If(NotZero), &[Code::Jne_rel8_64]),
    relative(Instruction::JnzRelbrd, If(NotZero), &[Code::Jne_rel32_64]),
    relative(Instruction::JbeRelbrb, If(BelowOrEqual), &[Code::Jbe_rel8_64]),
    relative(Instruction::JbeRelbrd, If(BelowOrEqual), &[Code::Jbe_rel32_64]),
    relative(Instruction::JnbeRelbrb, If(NotBelowOrEqual), &[Code::Ja_rel8_64]),
    relative(Instruction::JnbeRelbrd, If(NotBelowOrEqual), &[Code::Ja_rel32_64]),
    relative(Instruction::JsRelbrb, If(Sign), &[Code::Js_rel8_64]),
    relative(Instruction::JsRelbrd, If(Sign), &[Code::Js_rel32_64]),
    relative(Instruction::JnsRelbrb, If(NotSign), &[Code::Jns_rel8_64]),
    relative(Instruction::JnsRelbrd, If(NotSign), &[Code::Jns_rel32_64]),
    relative(Instruction::JpRelbrb, If(Parity), &[Code::Jp_rel8_64]),
    relative(Instruction::JpRelbrd, If(Parity), &[Code::Jp_rel32_64]),
    relative(Instruction::JnpRelbrb, If(NotParity), &[Code::Jnp_rel8_64]),
    relative(Instruction::JnpRelbrd, If(NotParity), &[Code::Jnp_rel32_64]),
    relative(Instruction::JlRelbrb, If(Less), &[Code::Jl_rel8_64]),
    relative(Instruction::JlRelbrd, If(Less), &[Code::Jl_rel32_64]),
    relative(Instruction::JnlRelbrb, If(NotLess), &[Code::Jge_rel8_64]),
    relative(Instruction::JnlRelbrd, If(NotLess), &[Code::Jge_rel32_64]),
    relative(Instruction::JleRelbrb, If(LessOrEqual), &[Code::Jle_rel8_64]),
    relative(Instruction::JleRelbrd, If(LessOrEqual), &[Code::Jle_rel32_64]),
    relative(Instruction::JnleRelbrb, If(NotLessOrEqual), &[Code::Jg_rel8_64]),
    relative(Instruction::JnleRelbrd, If(NotLessOrEqual), &[Code::Jg_rel32_64]),
    relative(Instruction::JrcxzRelbrb, CountIsZero, &[Code::Jrcxz_rel8_64]),
    relative(Instruction::JecxzRelbrb, CountIsZero, &[Code::Jecxz_rel8_64]),
    // JCXZ runs in a 32-bit code segment only, whose codes these are.
    relative(Instruction::JcxzRelbrb, CountIsZero, &[Code::Jcxz_rel8_32, Code::Jcxz_rel8_16]),
    relative(Instruction::LoopRelbrb, Loop, &[Code::Loop_rel8_64_RCX, Code::Loop_rel8_64_ECX]),
    relative(Instruction::LoopeRelbrb, LoopWhileZero, &[Code::Loope_rel8_64_RCX, Code::Loope_rel8_64_ECX]),
    relative(Instruction::LoopneRelbrb, LoopWhileNotZero, &[Code::Loopne_rel8_64_RCX, Code::Loopne_rel8_64_ECX]),
];
