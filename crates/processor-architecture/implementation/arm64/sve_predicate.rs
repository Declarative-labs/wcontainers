//! The SVE instructions that write a predicate, NZCV from one, or a count of one's elements: [PTRUE](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/PTRUE--Initialise-predicate-from-named-constraint-), the
//! predicate logic, the breaks, the predicate permutes, FFR, CNTP, WHILE, CTERM and the integer compares.

use core::convert::Infallible;

use crate::encoding::Field;
use crate::operand::{RegisterThirtyOne, Signedness};
use crate::registers::{read_register, write_register};
use crate::simd::Part;
use crate::sve::{
    GeneralValues, PD4, PG3, PG4, PM4, PN4, PredicateValues, RD5, RM5, RN5, SIZE,
    ScalableInstruction, ScalableOperand, ScalableOperation, VectorValues, count_elements,
    decode_count, is_active, read_element, read_predicate_element, test_predicate, to_element_bits,
    to_integer, write_predicate_element, write_predicate_truth,
};
use crate::{ConditionFlags, Exception, Instruction, Nzcv, Processor};

/// The pattern of PTRUE.
const PATTERN: Field = Field::new(5, 5);
/// M of BRKA and BRKB: whether inactive elements keep the destination's value instead of being cleared.
const MERGING: Field = Field::new(4, 1);
/// sf of WHILE: whether its registers are 64 bits wide.
const WHILE_WIDE: Field = Field::new(12, 1);
/// sz of CTERM: whether its registers are 64 bits wide.
const TERMINATE_WIDE: Field = Field::new(22, 1);
const SIGNED_IMMEDIATE: Field = Field::new(16, 5);
const UNSIGNED_IMMEDIATE: Field = Field::new(14, 7);
/// The bits of a predicate's element of a byte.
const BYTE: u32 = u8::BITS;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum PredicateLogic {
    And,
    /// BIC: the first and the second's complement.
    AndNot,
    Or,
    OrNot,
    ExclusiveOr,
    NotOr,
    NotAnd,
    /// SEL: the first where the governing predicate is set, the second elsewhere.
    Select,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum BreakKind {
    /// BRKA: active elements up to and including the first true one.
    After,
    /// BRKB: active elements before the first true one.
    Before,
    /// BRKPA: as BRKA over the second operand, where the first's last active element is true.
    PropagateAfter,
    PropagateBefore,
    /// BRKN: the second operand where the first's last active element is true, otherwise none.
    Next,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum PredicatePermutation {
    Zip(Part),
    Unzip(Part),
    Transpose(Part),
    Reverse,
    /// PUNPKLO and PUNPKHI: one half's byte elements widened to halfword elements.
    Unpack(Part),
}

/// The test of WHILE between its counter and its limit; a decreasing WHILE fills from the last element down.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum WhileCondition {
    Less(Signedness),
    LessOrEqual(Signedness),
    GreaterOrEqual(Signedness),
    Greater(Signedness),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum Comparison {
    Equal,
    NotEqual,
    GreaterOrEqual(Signedness),
    Greater(Signedness),
    Less(Signedness),
    LessOrEqual(Signedness),
}

/// What an integer compare compares each element with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum CompareWith {
    Vector,
    /// The doubleword element of the second vector that holds the element.
    Wide,
    Immediate(Signedness),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_arm64::sve)
)]
pub enum PredicateOperation {
    True {
        sets_flags: bool,
    },
    False,
    Test,
    First,
    Next,
    Logic {
        logic: PredicateLogic,
        sets_flags: bool,
    },
    Break {
        kind: BreakKind,
        sets_flags: bool,
    },
    Permute(PredicatePermutation),
    /// RDFFR, under a governing predicate or not.
    ReadFirstFault {
        governed: bool,
        sets_flags: bool,
    },
    WriteFirstFault,
    SetFirstFault,
    /// CNTP: the elements active in both predicates.
    Count,
    While(WhileCondition),
    /// WHILEWR and WHILERW: the elements free of a conflict between two addresses.
    WhileFree {
        either_order: bool,
    },
    /// CTERMEQ and CTERMNE.
    Terminate {
        equal: bool,
    },
    Compare {
        comparison: Comparison,
        with: CompareWith,
    },
    /// MATCH and NMATCH: each active element true where it equals some element of the second vector, or none.
    Match {
        negated: bool,
    },
}

const fn predicate(field: Field, values: PredicateValues) -> ScalableOperand {
    ScalableOperand::Predicate { field, values }
}

const fn vector(field: Field, values: VectorValues) -> ScalableOperand {
    ScalableOperand::Vector {
        field,
        values,
        pair: false,
    }
}

const fn general(field: Field, values: Option<GeneralValues>) -> ScalableOperand {
    ScalableOperand::General {
        field,
        thirty_one: RegisterThirtyOne::ZeroRegister,
        values,
    }
}

use PredicateValues::{Governing, Operand, Written};
use ScalableOperand::{ElementSize, Selector, WrittenConditionFlags};

const TRUE: &[ScalableOperand] = &[
    predicate(PD4, Written),
    ElementSize(SIZE),
    ScalableOperand::Pattern(PATTERN),
];
const TRUE_FLAGS: &[ScalableOperand] = &[
    predicate(PD4, Written),
    ElementSize(SIZE),
    ScalableOperand::Pattern(PATTERN),
    WrittenConditionFlags,
];
const FALSE: &[ScalableOperand] = &[predicate(PD4, Written)];
const TEST: &[ScalableOperand] = &[
    predicate(PG4, Operand),
    predicate(PN4, Operand),
    WrittenConditionFlags,
];
const FIRST: &[ScalableOperand] = &[
    predicate(PD4, Operand),
    predicate(PN4, Operand),
    WrittenConditionFlags,
];
const NEXT: &[ScalableOperand] = &[
    predicate(PD4, Operand),
    predicate(PN4, Operand),
    ElementSize(SIZE),
    WrittenConditionFlags,
];
const LOGIC: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PG4, Operand),
    predicate(PN4, Operand),
    predicate(PM4, Operand),
];
const LOGIC_FLAGS: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PG4, Operand),
    predicate(PN4, Operand),
    predicate(PM4, Operand),
    WrittenConditionFlags,
];
const BREAK_MERGING: &[ScalableOperand] = &[
    predicate(PD4, Operand),
    predicate(PG4, Operand),
    predicate(PN4, Operand),
    Selector(MERGING),
];
const BREAK_FLAGS: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PG4, Operand),
    predicate(PN4, Operand),
    WrittenConditionFlags,
];
const BREAK_NEXT: &[ScalableOperand] = &[
    predicate(PD4, Operand),
    predicate(PG4, Operand),
    predicate(PN4, Operand),
];
const BREAK_NEXT_FLAGS: &[ScalableOperand] = &[
    predicate(PD4, Operand),
    predicate(PG4, Operand),
    predicate(PN4, Operand),
    WrittenConditionFlags,
];
const PERMUTE: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PN4, Operand),
    predicate(PM4, Operand),
    ElementSize(SIZE),
];
const REVERSE: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PN4, Operand),
    ElementSize(SIZE),
];
const UNPACK: &[ScalableOperand] = &[predicate(PD4, Written), predicate(PN4, Operand)];
const READ_FIRST_FAULT_GOVERNED: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PN4, Operand),
    ScalableOperand::FirstFault,
];
const READ_FIRST_FAULT_FLAGS: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PN4, Operand),
    ScalableOperand::FirstFault,
    WrittenConditionFlags,
];
const READ_FIRST_FAULT: &[ScalableOperand] =
    &[predicate(PD4, Written), ScalableOperand::FirstFault];
const WRITE_FIRST_FAULT: &[ScalableOperand] =
    &[predicate(PN4, Operand), ScalableOperand::WrittenFirstFault];
const SET_FIRST_FAULT: &[ScalableOperand] = &[ScalableOperand::WrittenFirstFault];
const COUNT: &[ScalableOperand] = &[
    general(RD5, None),
    predicate(PG4, Operand),
    predicate(PN4, Operand),
    ElementSize(SIZE),
];
const WHILE: &[ScalableOperand] = &[
    predicate(PD4, Written),
    general(RN5, Some(GeneralValues::Bound)),
    general(RM5, Some(GeneralValues::Bound)),
    ElementSize(SIZE),
    Selector(WHILE_WIDE),
    WrittenConditionFlags,
];
const WHILE_FREE: &[ScalableOperand] = &[
    predicate(PD4, Written),
    general(RN5, Some(GeneralValues::Bound)),
    general(RM5, Some(GeneralValues::Bound)),
    ElementSize(SIZE),
    WrittenConditionFlags,
];
const TERMINATE: &[ScalableOperand] = &[
    general(RN5, Some(GeneralValues::Plain)),
    general(RM5, Some(GeneralValues::Plain)),
    Selector(TERMINATE_WIDE),
    ScalableOperand::ConditionFlags,
];
const COMPARE_VECTORS: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PG3, Governing),
    vector(RN5, VectorValues::Data),
    vector(RM5, VectorValues::Data),
    ElementSize(SIZE),
    WrittenConditionFlags,
];
const COMPARE_SIGNED_IMMEDIATE: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PG3, Governing),
    vector(RN5, VectorValues::Data),
    ScalableOperand::SignedImmediate(SIGNED_IMMEDIATE),
    ElementSize(SIZE),
    WrittenConditionFlags,
];
const COMPARE_UNSIGNED_IMMEDIATE: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PG3, Governing),
    vector(RN5, VectorValues::Data),
    ScalableOperand::UnsignedImmediate(UNSIGNED_IMMEDIATE),
    ElementSize(SIZE),
    WrittenConditionFlags,
];

/// The element size of MATCH and NMATCH: a byte or a halfword.
const MATCH_SIZE: Field = Field::new(22, 1);
const MATCH: &[ScalableOperand] = &[
    predicate(PD4, Written),
    predicate(PG3, Governing),
    vector(RN5, VectorValues::Data),
    vector(RM5, VectorValues::Data),
    Selector(MATCH_SIZE),
    WrittenConditionFlags,
];

/// The model of each instruction of this kind, none for every other instruction.
pub(crate) fn to_predicate_instruction(
    instruction: Instruction<Infallible>,
) -> Option<ScalableInstruction> {
    use BreakKind as B;
    use CompareWith::{Immediate, Vector, Wide};
    use Comparison as C;
    use Instruction as I;
    use PredicateLogic as L;
    use PredicateOperation as O;
    use PredicatePermutation as M;
    use Signedness::{Signed, Unsigned};
    let logic = |logic, sets_flags| {
        (
            O::Logic { logic, sets_flags },
            if sets_flags { LOGIC_FLAGS } else { LOGIC },
        )
    };
    let compare = |comparison, with| {
        let operands = match with {
            Vector | Wide => COMPARE_VECTORS,
            Immediate(Signed) => COMPARE_SIGNED_IMMEDIATE,
            Immediate(Unsigned) => COMPARE_UNSIGNED_IMMEDIATE,
        };
        (O::Compare { comparison, with }, operands)
    };
    let (operation, operands) = match instruction {
        I::Ptrue => (O::True { sets_flags: false }, TRUE),
        I::Ptrues => (O::True { sets_flags: true }, TRUE_FLAGS),
        I::Pfalse => (O::False, FALSE),
        I::Ptest => (O::Test, TEST),
        I::Pfirst => (O::First, FIRST),
        I::Pnext => (O::Next, NEXT),
        I::AndPPzPP => logic(L::And, false),
        I::BicPPzPP => logic(L::AndNot, false),
        I::OrrPPzPP => logic(L::Or, false),
        I::OrnPPzPP => logic(L::OrNot, false),
        I::EorPPzPP => logic(L::ExclusiveOr, false),
        I::Nor => logic(L::NotOr, false),
        I::Nand => logic(L::NotAnd, false),
        I::SelPPPP => logic(L::Select, false),
        I::AndsPPzPP => logic(L::And, true),
        I::BicsPPzPP => logic(L::AndNot, true),
        I::Orrs => logic(L::Or, true),
        I::Orns => logic(L::OrNot, true),
        I::Eors => logic(L::ExclusiveOr, true),
        I::Nors => logic(L::NotOr, true),
        I::Nands => logic(L::NotAnd, true),
        I::BrkaPPP => (
            O::Break {
                kind: B::After,
                sets_flags: false,
            },
            BREAK_MERGING,
        ),
        I::BrkbPPP => (
            O::Break {
                kind: B::Before,
                sets_flags: false,
            },
            BREAK_MERGING,
        ),
        I::Brkas => (
            O::Break {
                kind: B::After,
                sets_flags: true,
            },
            BREAK_FLAGS,
        ),
        I::Brkbs => (
            O::Break {
                kind: B::Before,
                sets_flags: true,
            },
            BREAK_FLAGS,
        ),
        I::Brkpa => (
            O::Break {
                kind: B::PropagateAfter,
                sets_flags: false,
            },
            LOGIC,
        ),
        I::Brkpas => (
            O::Break {
                kind: B::PropagateAfter,
                sets_flags: true,
            },
            LOGIC_FLAGS,
        ),
        I::Brkpb => (
            O::Break {
                kind: B::PropagateBefore,
                sets_flags: false,
            },
            LOGIC,
        ),
        I::Brkpbs => (
            O::Break {
                kind: B::PropagateBefore,
                sets_flags: true,
            },
            LOGIC_FLAGS,
        ),
        I::Brkn => (
            O::Break {
                kind: B::Next,
                sets_flags: false,
            },
            BREAK_NEXT,
        ),
        I::Brkns => (
            O::Break {
                kind: B::Next,
                sets_flags: true,
            },
            BREAK_NEXT_FLAGS,
        ),
        I::Zip1PPP => (O::Permute(M::Zip(Part::First)), PERMUTE),
        I::Zip2PPP => (O::Permute(M::Zip(Part::Second)), PERMUTE),
        I::Uzp1PPP => (O::Permute(M::Unzip(Part::First)), PERMUTE),
        I::Uzp2PPP => (O::Permute(M::Unzip(Part::Second)), PERMUTE),
        I::Trn1PPP => (O::Permute(M::Transpose(Part::First)), PERMUTE),
        I::Trn2PPP => (O::Permute(M::Transpose(Part::Second)), PERMUTE),
        I::RevPP => (O::Permute(M::Reverse), REVERSE),
        I::Punpklo => (O::Permute(M::Unpack(Part::First)), UNPACK),
        I::Punpkhi => (O::Permute(M::Unpack(Part::Second)), UNPACK),
        I::RdffrPPz => (
            O::ReadFirstFault {
                governed: true,
                sets_flags: false,
            },
            READ_FIRST_FAULT_GOVERNED,
        ),
        I::Rdffrs => (
            O::ReadFirstFault {
                governed: true,
                sets_flags: true,
            },
            READ_FIRST_FAULT_FLAGS,
        ),
        I::RdffrP => (
            O::ReadFirstFault {
                governed: false,
                sets_flags: false,
            },
            READ_FIRST_FAULT,
        ),
        I::Wrffr => (O::WriteFirstFault, WRITE_FIRST_FAULT),
        I::Setffr => (O::SetFirstFault, SET_FIRST_FAULT),
        I::CntpXPP => (O::Count, COUNT),
        I::WhileltPPRr => (O::While(WhileCondition::Less(Signed)), WHILE),
        I::WhileloPPRr => (O::While(WhileCondition::Less(Unsigned)), WHILE),
        I::WhilelePPRr => (O::While(WhileCondition::LessOrEqual(Signed)), WHILE),
        I::WhilelsPPRr => (O::While(WhileCondition::LessOrEqual(Unsigned)), WHILE),
        I::WhilegePPRr => (O::While(WhileCondition::GreaterOrEqual(Signed)), WHILE),
        I::WhilehsPPRr => (O::While(WhileCondition::GreaterOrEqual(Unsigned)), WHILE),
        I::WhilegtPPRr => (O::While(WhileCondition::Greater(Signed)), WHILE),
        I::WhilehiPPRr => (O::While(WhileCondition::Greater(Unsigned)), WHILE),
        I::Whilewr => (
            O::WhileFree {
                either_order: false,
            },
            WHILE_FREE,
        ),
        I::Whilerw => (O::WhileFree { either_order: true }, WHILE_FREE),
        I::CtermeqRr => (O::Terminate { equal: true }, TERMINATE),
        I::CtermneRr => (O::Terminate { equal: false }, TERMINATE),
        I::CmpeqPPzZZ => compare(C::Equal, Vector),
        I::CmpnePPzZZ => compare(C::NotEqual, Vector),
        I::CmpgePPzZZ => compare(C::GreaterOrEqual(Signed), Vector),
        I::CmpgtPPzZZ => compare(C::Greater(Signed), Vector),
        I::CmphsPPzZZ => compare(C::GreaterOrEqual(Unsigned), Vector),
        I::CmphiPPzZZ => compare(C::Greater(Unsigned), Vector),
        I::CmpeqWIDEPPzZZ => compare(C::Equal, Wide),
        I::CmpneWIDEPPzZZ => compare(C::NotEqual, Wide),
        I::CmpgeWIDEPPzZZ => compare(C::GreaterOrEqual(Signed), Wide),
        I::CmpgtWIDEPPzZZ => compare(C::Greater(Signed), Wide),
        I::CmpltWIDEPPzZZ => compare(C::Less(Signed), Wide),
        I::CmpleWIDEPPzZZ => compare(C::LessOrEqual(Signed), Wide),
        I::CmphsWIDEPPzZZ => compare(C::GreaterOrEqual(Unsigned), Wide),
        I::CmphiWIDEPPzZZ => compare(C::Greater(Unsigned), Wide),
        I::CmploWIDEPPzZZ => compare(C::Less(Unsigned), Wide),
        I::CmplsWIDEPPzZZ => compare(C::LessOrEqual(Unsigned), Wide),
        I::CmpeqPPzZI => compare(C::Equal, Immediate(Signed)),
        I::CmpnePPzZI => compare(C::NotEqual, Immediate(Signed)),
        I::CmpgePPzZI => compare(C::GreaterOrEqual(Signed), Immediate(Signed)),
        I::CmpgtPPzZI => compare(C::Greater(Signed), Immediate(Signed)),
        I::CmpltPPzZI => compare(C::Less(Signed), Immediate(Signed)),
        I::CmplePPzZI => compare(C::LessOrEqual(Signed), Immediate(Signed)),
        I::CmphsPPzZI => compare(C::GreaterOrEqual(Unsigned), Immediate(Unsigned)),
        I::CmphiPPzZI => compare(C::Greater(Unsigned), Immediate(Unsigned)),
        I::CmploPPzZI => compare(C::Less(Unsigned), Immediate(Unsigned)),
        I::CmplsPPzZI => compare(C::LessOrEqual(Unsigned), Immediate(Unsigned)),
        I::Match => (O::Match { negated: false }, MATCH),
        I::Nmatch => (O::Match { negated: true }, MATCH),
        _ => return None,
    };
    Some(ScalableInstruction {
        operation: ScalableOperation::Predicate(operation),
        operands,
    })
}

/// The predicate register an encoding field names.
fn read_predicate<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    field: Field,
    word: u32,
) -> u16 {
    processor.predicate_registers[field.read(word) as usize]
}

fn write_predicate<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    field: Field,
    word: u32,
    value: u16,
) {
    processor.predicate_registers[field.read(word) as usize] = value;
}

/// The first and the last element active in the predicate.
fn find_active_ends(predicate: u16, bits: u32) -> Option<(u32, u32)> {
    let mut active = (0..count_elements(bits)).filter(|index| is_active(predicate, *index, bits));
    let first = active.next()?;
    Some((first, active.next_back().unwrap_or(first)))
}

/// Whether the result's last element active in the mask is true, as [LastActive](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-sve#impl-aarch64.LastActive.3) has it.
fn is_last_active(mask: u16, result: u16, bits: u32) -> bool {
    find_active_ends(mask, bits).is_some_and(|(_, last)| is_active(result, last, bits))
}

pub(crate) fn simulate_predicate<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    word: u32,
    operation: PredicateOperation,
) -> Result<(), Exception> {
    use PredicateOperation as O;
    let bits = to_element_bits(SIZE.read(word));
    let elements = count_elements(bits);
    let flags = match operation {
        O::True { sets_flags } => {
            let count = decode_count(PATTERN.read(word), bits)
                + u32::from(crate::planted::is(1001) && PATTERN.read(word) == 7);
            let mut result = 0;
            for index in 0..elements {
                write_predicate_truth(&mut result, index, bits, index < count);
            }
            write_predicate(processor, PD4, word, result);
            sets_flags.then(|| test_predicate(result, result, bits))
        }
        O::False => {
            write_predicate(processor, PD4, word, 0);
            None
        }
        O::Test => Some(test_predicate(
            read_predicate(processor, PG4, word),
            read_predicate(processor, PN4, word),
            BYTE,
        )),
        O::First => {
            let mask = read_predicate(processor, PN4, word);
            let mut result = read_predicate(processor, PD4, word);
            if let Some((first, _)) = find_active_ends(mask, BYTE) {
                write_predicate_truth(&mut result, first, BYTE, true);
            }
            write_predicate(processor, PD4, word, result);
            Some(test_predicate(mask, result, BYTE))
        }
        O::Next => {
            let mask = read_predicate(processor, PN4, word);
            let operand = read_predicate(processor, PD4, word);
            let start = find_active_ends(operand, bits)
                .map_or(0, |(_, last)| last + u32::from(!crate::planted::is(1005)));
            let next = (start..elements).find(|index| is_active(mask, *index, bits));
            let mut result = 0;
            if let Some(next) = next {
                write_predicate_truth(&mut result, next, bits, true);
            }
            write_predicate(processor, PD4, word, result);
            Some(test_predicate(mask, result, bits))
        }
        O::Logic { logic, sets_flags } => {
            let mask = read_predicate(processor, PG4, word);
            let first = read_predicate(processor, PN4, word);
            let second = read_predicate(processor, PM4, word);
            let result = match logic {
                PredicateLogic::And => mask & first & second,
                PredicateLogic::AndNot => mask & first & !second,
                PredicateLogic::Or => mask & (first | second),
                PredicateLogic::OrNot => mask & (first | !second),
                PredicateLogic::ExclusiveOr => mask & (first ^ second),
                PredicateLogic::NotOr => mask & !(first | second),
                PredicateLogic::NotAnd => mask & !(first & second),
                PredicateLogic::Select => (mask & first) | (!mask & second),
            };
            write_predicate(processor, PD4, word, result);
            sets_flags.then(|| test_predicate(mask, result, BYTE))
        }
        O::Break { kind, sets_flags } => {
            let mask = read_predicate(processor, PG4, word);
            let first = read_predicate(processor, PN4, word);
            let old = read_predicate(processor, PD4, word);
            let merging = matches!(kind, BreakKind::After | BreakKind::Before)
                && !sets_flags
                && MERGING.read(word) != 0;
            let result = simulate_break(
                kind,
                mask,
                first,
                read_predicate(processor, PM4, word),
                old,
                merging,
            );
            write_predicate(processor, PD4, word, result);
            let flag_mask = if kind == BreakKind::Next {
                u16::MAX
            } else {
                mask
            };
            sets_flags.then(|| test_predicate(flag_mask, result, BYTE))
        }
        O::Permute(permutation) => {
            let first = read_predicate(processor, PN4, word);
            let second = read_predicate(processor, PM4, word);
            let result = permute_predicate(permutation, first, second, bits);
            write_predicate(processor, PD4, word, result);
            None
        }
        O::ReadFirstFault {
            governed,
            sets_flags,
        } => {
            let first_fault = processor.first_fault_register;
            let mask = if governed {
                read_predicate(processor, PN4, word)
            } else {
                u16::MAX
            };
            let result = first_fault & mask;
            write_predicate(processor, PD4, word, result);
            let flag_mask = if crate::planted::is(1011) {
                u16::MAX
            } else {
                mask
            };
            sets_flags.then(|| test_predicate(flag_mask, result, BYTE))
        }
        O::WriteFirstFault => {
            processor.first_fault_register = read_predicate(processor, PN4, word);
            None
        }
        O::SetFirstFault => {
            processor.first_fault_register = u16::MAX;
            None
        }
        O::Count => {
            let mask = read_predicate(processor, PG4, word);
            let counted = read_predicate(processor, PN4, word);
            let count = (0..elements)
                .filter(|index| {
                    (is_active(mask, *index, bits) || crate::planted::is(1012))
                        && is_active(counted, *index, bits)
                })
                .count();
            write_register(
                processor,
                RD5.read(word),
                count as u64,
                RegisterThirtyOne::ZeroRegister,
            );
            None
        }
        O::While(condition) => {
            let wide = WHILE_WIDE.read(word) != 0;
            let result = simulate_while(processor, word, condition, wide, bits);
            write_predicate(processor, PD4, word, result);
            Some(test_predicate(u16::MAX, result, bits))
        }
        O::WhileFree { either_order } => {
            let read = |field: Field| {
                read_register(processor, field.read(word), RegisterThirtyOne::ZeroRegister)
            };
            // The addresses as unsigned numbers, their difference not wrapped.
            let difference = if crate::planted::is(1009) {
                i128::from(read(RM5).wrapping_sub(read(RN5)) as i64)
            } else {
                i128::from(read(RM5)) - i128::from(read(RN5))
            };
            let bytes = i128::from(bits / u8::BITS);
            let elements_apart = if either_order {
                difference.abs() / bytes
            } else {
                difference.div_euclid(bytes)
            };
            let result = (0..elements).fold(0, |mut result, index| {
                let free = if either_order {
                    elements_apart == 0 || i128::from(index) < elements_apart
                } else {
                    elements_apart <= 0 || i128::from(index) < elements_apart
                };
                write_predicate_truth(&mut result, index, bits, free);
                result
            });
            write_predicate(processor, PD4, word, result);
            Some(test_predicate(u16::MAX, result, bits))
        }
        O::Terminate { equal } => {
            let width_mask = if TERMINATE_WIDE.read(word) != 0 {
                u64::MAX
            } else {
                u64::from(u32::MAX)
            };
            let read = |field: Field| {
                read_register(processor, field.read(word), RegisterThirtyOne::ZeroRegister)
                    & width_mask
            };
            let terminates = (read(RN5) == read(RM5)) == equal;
            let mut flags = ConditionFlags::from(processor.condition_flags);
            let carry = flags.contains(ConditionFlags::CARRY);
            flags.set(ConditionFlags::NEGATIVE, terminates);
            flags.set(
                ConditionFlags::OVERFLOW,
                !terminates && carry == crate::planted::is(1010),
            );
            Some(flags)
        }
        O::Compare { comparison, with } => {
            let mask = read_predicate(processor, PG3, word);
            let first = processor.vector_registers[RN5.read(word) as usize];
            let second = processor.vector_registers[RM5.read(word) as usize];
            if with == CompareWith::Wide && bits == u64::BITS {
                return Err(Exception::UndefinedInstruction);
            }
            let signedness = match comparison {
                Comparison::Equal | Comparison::NotEqual => Signedness::Signed,
                Comparison::GreaterOrEqual(signedness)
                | Comparison::Greater(signedness)
                | Comparison::Less(signedness)
                | Comparison::LessOrEqual(signedness) => signedness,
            };
            let signedness = if crate::planted::is(1013) {
                Signedness::Signed
            } else {
                signedness
            };
            let mut result = 0;
            for index in 0..elements {
                let element = to_integer(read_element(first, index, bits), bits, signedness);
                let other = match with {
                    CompareWith::Vector => {
                        to_integer(read_element(second, index, bits), bits, signedness)
                    }
                    CompareWith::Wide => {
                        let wide = if crate::planted::is(1014) {
                            index % 2
                        } else {
                            index * bits / u64::BITS
                        };
                        to_integer(read_element(second, wide, u64::BITS), u64::BITS, signedness)
                    }
                    CompareWith::Immediate(Signedness::Signed) => {
                        SIGNED_IMMEDIATE.read_signed(word).into()
                    }
                    CompareWith::Immediate(Signedness::Unsigned) => (UNSIGNED_IMMEDIATE.read(word)
                        & if crate::planted::is(1015) { 0x3F } else { 0x7F })
                    .into(),
                };
                let holds = match comparison {
                    Comparison::Equal => element == other,
                    Comparison::NotEqual => element != other,
                    Comparison::GreaterOrEqual(_) => element >= other,
                    Comparison::Greater(_) => element > other,
                    Comparison::Less(_) => element < other,
                    Comparison::LessOrEqual(_) => element <= other,
                };
                write_predicate_truth(
                    &mut result,
                    index,
                    bits,
                    holds && is_active(mask, index, bits),
                );
            }
            write_predicate(processor, PD4, word, result);
            Some(test_predicate(mask, result, bits))
        }
        O::Match { negated } => {
            let bits = to_element_bits(MATCH_SIZE.read(word));
            let mask = read_predicate(processor, PG3, word);
            let first = processor.vector_registers[RN5.read(word) as usize];
            let second = processor.vector_registers[RM5.read(word) as usize];
            let mut result = 0;
            for index in 0..count_elements(bits) {
                let element = read_element(first, index, bits);
                let found = if crate::planted::is(1039) {
                    read_element(second, index, bits) == element
                } else {
                    (0..count_elements(bits))
                        .any(|other| read_element(second, other, bits) == element)
                };
                let holds = found != negated && is_active(mask, index, bits);
                write_predicate_truth(&mut result, index, bits, holds);
            }
            write_predicate(processor, PD4, word, result);
            Some(test_predicate(mask, result, bits))
        }
    };
    if let Some(flags) = flags {
        processor.condition_flags = Nzcv::from(flags);
    }
    Ok(())
}

/// The breaks of [BRKA](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/BRKA--Break-after-first-true-condition-), BRKB, BRKPA, BRKPB and BRKN over byte elements.
fn simulate_break(
    kind: BreakKind,
    mask: u16,
    first: u16,
    second: u16,
    old: u16,
    merging: bool,
) -> u16 {
    let elements = count_elements(BYTE);
    let mut result = 0;
    match kind {
        BreakKind::After | BreakKind::Before => {
            let mut broken = false;
            for index in 0..elements {
                let element = is_active(first, index, BYTE);
                if is_active(mask, index, BYTE) {
                    if kind == BreakKind::Before && !crate::planted::is(1003) {
                        broken |= element;
                    }
                    write_predicate_truth(&mut result, index, BYTE, !broken);
                    if kind == BreakKind::After || crate::planted::is(1003) {
                        broken |= element;
                    }
                } else if merging {
                    write_predicate_element(
                        &mut result,
                        index,
                        BYTE,
                        read_predicate_element(old, index, BYTE),
                    );
                }
            }
        }
        BreakKind::PropagateAfter | BreakKind::PropagateBefore => {
            let mut last = if crate::planted::is(1004) {
                find_active_ends(mask, BYTE)
                    .is_some_and(|(first_active, _)| is_active(first, first_active, BYTE))
            } else {
                is_last_active(mask, first, BYTE)
            };
            for index in 0..elements {
                if !is_active(mask, index, BYTE) {
                    continue;
                }
                let element = is_active(second, index, BYTE);
                if kind == BreakKind::PropagateBefore {
                    last &= !element;
                }
                write_predicate_truth(&mut result, index, BYTE, last);
                if kind == BreakKind::PropagateAfter {
                    last &= !element;
                }
            }
        }
        BreakKind::Next => {
            result = if is_last_active(mask, first, BYTE) {
                old
            } else {
                0
            };
        }
    }
    result
}

/// ZIP, UZP, TRN, REV and PUNPK of predicates, each moving whole elements of the bits.
fn permute_predicate(permutation: PredicatePermutation, first: u16, second: u16, bits: u32) -> u16 {
    let elements = count_elements(bits);
    let half = elements / 2;
    let take = |index: u32| {
        if index < elements {
            read_predicate_element(first, index, bits)
        } else {
            read_predicate_element(second, index - elements, bits)
        }
    };
    let mut result = 0;
    match permutation {
        PredicatePermutation::Zip(part) => {
            let base = if part == Part::Second {
                half - u32::from(crate::planted::is(1006))
            } else {
                0
            };
            for pair in 0..half {
                write_predicate_element(
                    &mut result,
                    2 * pair,
                    bits,
                    read_predicate_element(first, base + pair, bits),
                );
                write_predicate_element(
                    &mut result,
                    2 * pair + 1,
                    bits,
                    read_predicate_element(second, base + pair, bits),
                );
            }
        }
        PredicatePermutation::Unzip(part) => {
            for index in 0..elements {
                write_predicate_element(
                    &mut result,
                    index,
                    bits,
                    take(2 * index + u32::from(part == Part::Second)),
                );
            }
        }
        PredicatePermutation::Transpose(part) => {
            for pair in 0..half {
                let taken = 2 * pair + u32::from(part == Part::Second);
                write_predicate_element(
                    &mut result,
                    2 * pair,
                    bits,
                    read_predicate_element(first, taken, bits),
                );
                write_predicate_element(
                    &mut result,
                    2 * pair + 1,
                    bits,
                    read_predicate_element(second, taken, bits),
                );
            }
        }
        PredicatePermutation::Reverse => {
            for index in 0..elements {
                write_predicate_element(
                    &mut result,
                    index,
                    bits,
                    read_predicate_element(first, elements - 1 - index, bits),
                );
            }
        }
        PredicatePermutation::Unpack(part) => {
            let wide = u16::BITS;
            let base = if part == Part::Second && !crate::planted::is(1007) {
                count_elements(wide)
            } else {
                0
            };
            for index in 0..count_elements(wide) {
                let bit = read_predicate_element(first, base + index, BYTE);
                write_predicate_element(&mut result, index, wide, bit);
            }
        }
    }
    result
}

/// [WHILELT](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/WHILELT--While-incrementing-signed-scalar-less-than-scalar-) and its kin: the counter is the first register, stepped once an element
/// and wrapping at the register's width; each element is true while the test has held for it and every one before
/// it.
fn simulate_while<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    word: u32,
    condition: WhileCondition,
    wide: bool,
    bits: u32,
) -> u16 {
    let width = if wide { u64::BITS } else { u32::BITS };
    let signedness = match condition {
        WhileCondition::Less(signedness)
        | WhileCondition::LessOrEqual(signedness)
        | WhileCondition::GreaterOrEqual(signedness)
        | WhileCondition::Greater(signedness) => signedness,
    };
    let read = |field: Field| {
        let value = read_register(processor, field.read(word), RegisterThirtyOne::ZeroRegister);
        to_integer(u128::from(value), width, signedness)
    };
    let (mut counter, limit) = (read(RN5), read(RM5));
    let elements = count_elements(bits);
    let decreasing = matches!(
        condition,
        WhileCondition::GreaterOrEqual(_) | WhileCondition::Greater(_)
    );
    let mut holding = true;
    let mut result = 0;
    for step in 0..elements {
        holding &= match condition {
            WhileCondition::Less(_) => counter < limit,
            WhileCondition::LessOrEqual(_) => counter <= limit,
            WhileCondition::GreaterOrEqual(_) => counter >= limit,
            WhileCondition::Greater(_) => counter > limit,
        };
        let index = if decreasing {
            elements - 1 - step
        } else {
            step
        };
        write_predicate_truth(&mut result, index, bits, holding);
        let stepped = counter + if decreasing { -1 } else { 1 };
        counter = if crate::planted::is(1008) {
            stepped
        } else {
            to_integer(stepped as u128, width, signedness)
        };
    }
    result
}
