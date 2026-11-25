//! Every A64 instruction variant, made by `build.rs` from `list/a64-instructions.tsv`.

use core::convert::Infallible;

use crate::encoding::{Field, FixedBits};
use crate::floating_point::Precision;
use crate::operand::{
    Indexing, Operand, OperandAccess, RegisterThirtyOne, RegisterWidth, Rotation, Signedness,
};

include!(concat!(env!("OUT_DIR"), "/instruction_list.rs"));

/// What the list says of one instruction variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InstructionRecord {
    pub mnemonic: &'static str,
    pub fixed_bits: FixedBits,
    /// Bits the manual says should hold a value but that select no other instruction.
    pub should_be: FixedBits,
    /// The patterns of operand values inside the fixed bits that are unallocated: a word matching one is undefined.
    pub reserved: &'static [FixedBits],
    /// A word LLVM decodes as this variant.
    pub example: Option<u32>,
    /// In the order of the assembler syntax, then the fields it does not name.
    pub operands: &'static [Operand],
    pub class: InstructionClass,
    /// The Arm features LLVM requires: `&` for all of, `|` for any of.
    pub feature: &'static str,
    pub privilege: PrivilegeBehavior,
}

/// The group of the A64 encoding space, with cryptography, pointer authentication and memory tagging taken out.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum InstructionClass {
    DataProcessingImmediate,
    DataProcessingRegister,
    LoadsAndStores,
    BranchesExceptionsAndSystem,
    FloatingPoint,
    AdvancedSimd,
    Sve,
    Sme,
    Cryptography,
    PointerAuthentication,
    MemoryTagging,
}

/// How an instruction behaves at EL0 compared with higher exception levels.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PrivilegeBehavior {
    Unprivileged,
    /// Undefined or trapped at EL0 whatever higher levels configure.
    TrapsAtEl0,
    /// What it does at EL0 depends on what higher levels configure.
    DependsOnPrivilegeLevel,
}

impl<LevelSpecific> Instruction<LevelSpecific> {
    /// Returns the same instruction as one every privilege level has, none for one only `LevelSpecific` has.
    pub const fn to_every_level(&self) -> Option<Instruction<Infallible>> {
        match self.to_list_index() {
            Some(index) => Some(LISTED_INSTRUCTIONS[index]),
            None if matches!(self, Self::MrsCtrEl0) => Some(Instruction::MrsCtrEl0),
            None => None,
        }
    }
}

impl Instruction<Infallible> {
    /// None for the variants the list does not hold.
    pub const fn to_record(self) -> Option<&'static InstructionRecord> {
        match self.to_list_index() {
            Some(index) => Some(&RECORDS[index]),
            None => None,
        }
    }
}
