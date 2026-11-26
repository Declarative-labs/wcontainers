//! A generator of ARM64 states that forms every combination of the critical values of an instruction's operands,
//! except where the other operands' values already decide what a value could change, for every known behavior and
//! every flag the instruction may write: there that operand takes one value, or each of its values once.
//!
//! Register numbers are chosen together, not in every combination: first every register operand names a register of
//! its own, and only there do the other operands take every combination; then each operand names each special
//! register (the zero register or stack pointer, the link register, the vector register and the predicate holding
//! zero) while the others keep their own, and each two operands of one kind name the same register, an ordinary one and
//! each special one.
//! Under those numbers the values, controls and written state take each of their values once, beside each other.
//!
//! By what the values mean, an operand's value cannot change the outcome when:
//! - the word is undefined: a reserved operand value makes it so whatever the rest hold, so every operand but the
//!   register numbers and the fields takes one value;
//! - the value only moves where the access or the code lies, or the access faults whatever is stored: any access but
//!   the one at the lower page's start, untagged, on a page it may read and write, and any instruction at its page's
//!   end; there every field, every register's value, the flags and what memory holds take one value, but not the
//!   offset an access adds to its base, which is where it lies, and a store that faults part-way keeps every value
//!   of its data and of memory, since what it writes below the fault depends on them;
//! - the state does not hold it: a register's value given to the zero register, to the base of an access, or to a
//!   register a later operand names again;
//! - the condition and the flags only choose which path the instruction takes: each path takes every value of its
//!   sources at one condition and flags, the first in order where it holds and the first where it fails, and every
//!   other condition and flags take one value of each source, a different one for each, and of FPCR and FPSR;
//! - a source is a NaN or an infinity: no result is rounded, so the rounding mode takes one value, and without a
//!   subnormal source nothing is flushed, so the flush controls do too;
//! - a page the access does not reach: its permission takes one value.
//!
//! A source the instruction is documented not to read on a path (the register a conditional select does not select,
//! what a conditional compare compares where its condition fails, with FPCR) is not decided by that: a wrong model
//! could read it, and raise a flag on a signaling NaN or fault on an address, so on that path's first condition and
//! flags it takes each of its values once. The immediate fields such a path does not read take one value.
//!
//! FPSR before an instruction takes no flag and every flag with each control; each flag alone takes FPCR's first
//! value, since the instruction only adds flags to it.
//!
//! The states come grouped by what their data pages hold and allow, so that consecutive states differ in as few bytes
//! and permissions as they can; a state the chip cannot hold is refused before any caller sees it.

#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::Infallible;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_instruction_input_domain_arm64::{
    AccessPlacement, Arm64InputDomain, CodePlacement, DataPage, OperandType, OperandValue,
    encode_word,
};
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::encoding::{Field, FixedBits};
use wcontainers_processor_architecture_arm64::floating_point::{
    ALTERNATIVE_HALF, DEFAULT_NAN, FLUSH_HALF_TO_ZERO, FLUSH_TO_ZERO, FloatingPointOperation,
    Precision, ROUNDING_MODE_BITS, ROUNDING_MODE_SHIFT,
};
use wcontainers_processor_architecture_arm64::integer::IntegerOperation;
use wcontainers_processor_architecture_arm64::load_store::Transfer;
use wcontainers_processor_architecture_arm64::operand::Operand;
use wcontainers_processor_architecture_arm64::system_register::{
    HeldRegister, SystemRegisterAccess, to_system_register_access,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, CONDITIONS, DataAccess, DocumentedOperation, Instruction, InstructionFetch,
    LoadStoreInstruction, PagePermission, Processor, User, to_documented_operation,
};

/// The bits of FPCR a Neoverse V2 holds, the fewest of the known chips: AHP, DN, FZ, RMode and FZ16. It has no FEAT_AFP
/// (AH, FIZ and NEP), traps no floating-point exception and has no AArch32 Len or Stride, so it reads every other bit
/// as zero.
const FLOATING_POINT_CONTROL_BITS: u64 = ALTERNATIVE_HALF
    | DEFAULT_NAN
    | FLUSH_TO_ZERO
    | (((1 << ROUNDING_MODE_BITS) - 1) << ROUNDING_MODE_SHIFT)
    | FLUSH_HALF_TO_ZERO;

/// Whether a chip can hold the state: one whose FPCR sets a bit the chip does not hold cannot, since the chip reads that
/// bit back as zero.
const fn is_possible<LevelSpecific>(processor: &Processor<LevelSpecific>) -> bool {
    processor.floating_point_control & !FLOATING_POINT_CONTROL_BITS == 0
}

/// Where the top byte of an address starts, which TBI0 leaves out of translation.
const ADDRESS_TAG_BITS_SHIFT: u32 = u64::BITS - u8::BITS;

#[derive(Clone, Copy, Default, Debug)]
pub struct CriticalValueCut {
    domain: Arm64InputDomain,
}

impl CriticalValueCut {
    pub const fn new(domain: Arm64InputDomain) -> Self {
        Self { domain }
    }

    /// Every state the generator makes for the instruction, in the order a run takes them, with its operands in the
    /// domain's order. A state the chip cannot hold is refused here, the one place states are made, so that no probe
    /// or check runs one: the Neoverse V2, which holds the fewest FPCR bits of the known chips, decides.
    pub fn generate_states_with_operands(
        &self,
        instruction: Instruction<Infallible>,
    ) -> impl Iterator<Item = (Vec<OperandValue>, ExecutionState<'static, Arm64<User>>)> + use<>
    {
        let domain = self.domain;
        self.generate_operand_values(instruction)
            .into_iter()
            .filter_map(move |operands| {
                let state = domain.build_state(instruction, &operands);
                is_possible(&state.processor).then_some((operands, state))
            })
    }

    /// The operands of every state, each in the domain's order, grouped by what the data pages hold and allow.
    fn generate_operand_values(
        &self,
        instruction: Instruction<Infallible>,
    ) -> Vec<Vec<OperandValue>> {
        let cut = Cut::new(self.domain, instruction);
        let mut chosen = vec![0; cut.types.len()];
        let mut all = Vec::new();
        let mut pending = Vec::new();
        if cut.register_assignments.is_empty() {
            cut.descend(0, &mut chosen, Facts::UNKNOWN, &mut pending, &mut all);
        }
        for (number, assignment) in cut.register_assignments.iter().enumerate() {
            for &(operand, value) in assignment {
                chosen[operand] = value;
            }
            let facts = Facts {
                register_representative: number == 0,
                ..Facts::UNKNOWN
            };
            cut.descend(
                cut.register_operands,
                &mut chosen,
                facts,
                &mut pending,
                &mut all,
            );
        }
        all.sort_by_key(|chosen| {
            cut.layout
                .iter()
                .map(|operand| chosen[*operand])
                .collect::<Vec<_>>()
        });
        all.into_iter()
            .map(|chosen| {
                chosen
                    .iter()
                    .zip(&cut.values)
                    .map(|(value, values)| values[*value])
                    .collect()
            })
            .collect()
    }
}

impl DistinguishingStateGenerator<Arm64<User>> for CriticalValueCut {
    // The states come from a method that returns `impl Iterator`, a type no name reaches, so they are boxed.
    type States<'generator>
        = Box<dyn Iterator<Item = ExecutionState<'static, Arm64<User>>> + 'generator>
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Instruction<Infallible>) -> Self::States<'_> {
        Box::new(
            self.generate_states_with_operands(instruction)
                .map(|(_, state)| state),
        )
    }
}

/// How an operand takes its values, given the operands chosen before it.
enum Allowed {
    /// Each value with every combination of the later operands' values.
    Every(Vec<usize>),
    /// Each value once, beside the other operands that take each of theirs once: as many states as the longest list.
    EachOnce(Vec<usize>),
}

/// The kinds of operand, in the order the generator chooses them, so that each decides what the later ones take.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Group {
    Register,
    Placement,
    /// The field that names the system register an MRS or MSR reaches, which decides what else it reads.
    Selector,
    /// The field that names a condition, which with the flags decides what the instruction reads.
    Condition,
    Flags,
    Field,
    Data,
    Control,
    Written,
}

const GROUPS: [Group; 9] = [
    Group::Register,
    Group::Placement,
    Group::Selector,
    Group::Condition,
    Group::Flags,
    Group::Field,
    Group::Data,
    Group::Control,
    Group::Written,
];

/// The group of an operand, the field naming a condition aside.
const fn to_group(operand_type: OperandType) -> Group {
    match operand_type {
        OperandType::RegisterNumber(_)
        | OperandType::VectorRegisterNumber(_)
        | OperandType::PredicateRegisterNumber(_) => Group::Register,
        OperandType::AccessPlacement
        | OperandType::AddressTag
        | OperandType::DataPagePermission(_)
        | OperandType::CodePlacement => Group::Placement,
        OperandType::UnsignedImmediate(_)
        | OperandType::SignedImmediate(_)
        | OperandType::ShiftAmount { .. }
        | OperandType::BitmaskImmediate(_)
        | OperandType::ProgramCounterOffset(_)
        | OperandType::SplitImmediate(_)
        | OperandType::Pattern(_)
        | OperandType::Word(_) => Group::Field,
        OperandType::ConditionFlags => Group::Flags,
        OperandType::RegisterValue { .. }
        | OperandType::Address { .. }
        | OperandType::FloatingPointValue { .. }
        | OperandType::ConvertedInteger { .. }
        | OperandType::ConvertedFloatingPoint { .. }
        | OperandType::Comparand { .. }
        | OperandType::SoftwareThreadId
        | OperandType::VectorValue { .. }
        | OperandType::GoverningPredicate(_)
        | OperandType::PredicateValue(_)
        | OperandType::NextVectorValue(_)
        | OperandType::ShiftAmountVector(_)
        | OperandType::RegisterBound(_)
        | OperandType::RegisterNearSaturation(_)
        | OperandType::FirstFaultValue => Group::Data,
        OperandType::SystemRegister(_) => Group::Selector,
        OperandType::FloatingPointControl(_) => Group::Control,
        OperandType::StatusFlags
        | OperandType::WrittenConditionFlags
        | OperandType::MemoryFill
        | OperandType::WrittenFirstFault => Group::Written,
    }
}

/// Which of its sources (register values and immediates, in the syntax's order) an instruction with a condition reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Selection {
    /// Every one whatever its condition.
    Always,
    /// The first when its condition holds, the second when it fails.
    OneOfTwo,
    /// The two it compares when its condition holds, the third, the flags it sets instead, when it fails.
    ComparesWhenHolding,
}

/// The rank of the flags a conditional compare sets where its condition fails among its sources.
const FLAGS_SOURCE: usize = 2;

/// What the operands chosen so far decide about the rest.
#[derive(Clone, Copy, Debug)]
struct Facts {
    /// The operands whose value the state holds, one bit each.
    held: u64,
    moved: bool,
    partial_store: bool,
    word: u32,
    undefined: bool,
    condition_holds: Option<bool>,
    /// Whether the condition and the flags are the first where the condition holds or the first where it fails, or
    /// the instruction has no condition, so that its sources take every value.
    path_representative: bool,
    nonfinite_source: bool,
    subnormal_source: bool,
    first_control: bool,
    /// Whether every register operand names a register of its own, the one choice of register numbers under which the
    /// other operands take every combination.
    register_representative: bool,
}

impl Facts {
    const UNKNOWN: Self = Self {
        held: u64::MAX,
        moved: false,
        partial_store: false,
        word: 0,
        undefined: false,
        condition_holds: None,
        path_representative: true,
        nonfinite_source: false,
        subnormal_source: false,
        first_control: true,
        register_representative: true,
    };
}

struct Cut {
    domain: Arm64InputDomain,
    instruction: Instruction<Infallible>,
    types: Vec<OperandType>,
    values: Vec<Vec<OperandValue>>,
    groups: Vec<Group>,
    /// Each operand's index, by group and then in the domain's order.
    order: Vec<usize>,
    /// Each operand's rank among the sources a condition selects from, none for every other operand.
    sources: Vec<Option<usize>>,
    selection: Selection,
    writes_memory: bool,
    /// The fields of the offset an access adds to its base, an immediate or a register with its extension.
    addressing: Vec<Field>,
    reserved: &'static [FixedBits],
    /// The register operands, which come first in `order`.
    register_operands: usize,
    /// Each choice of register numbers: the value of each register operand; the first names a register of its own
    /// for every operand.
    register_assignments: Vec<Vec<(usize, usize)>>,
    /// The operands that set what the data pages and the code page hold and allow, the order's key.
    layout: Vec<usize>,
}

impl Cut {
    fn new(domain: Arm64InputDomain, instruction: Instruction<Infallible>) -> Self {
        let types: Vec<OperandType> = domain.list_operand_types(instruction).collect();
        assert!(
            types.len() <= u64::BITS as usize,
            "{instruction:?} has too many operands"
        );
        let values: Vec<Vec<OperandValue>> = types
            .iter()
            .map(|operand_type| {
                let mut values: Vec<OperandValue> =
                    domain.generate_critical_values(*operand_type).collect();
                for number in domain.generate_distinct_register_numbers(*operand_type) {
                    if !values.contains(&number) {
                        values.push(number);
                    }
                }
                values
            })
            .collect();
        let selection = match to_documented_operation(instruction) {
            Some(
                DocumentedOperation::Integer(IntegerOperation::ConditionalSelect(_))
                | DocumentedOperation::FloatingPoint(FloatingPointOperation::ConditionalSelect),
            ) => Selection::OneOfTwo,
            Some(
                DocumentedOperation::Integer(IntegerOperation::ConditionalCompare(_))
                | DocumentedOperation::FloatingPoint(FloatingPointOperation::ConditionalCompare {
                    ..
                }),
            ) => Selection::ComparesWhenHolding,
            _ => Selection::Always,
        };
        let record = instruction.to_record();
        let condition = record.and_then(|record| {
            record.operands.iter().find_map(|operand| match *operand {
                Operand::Condition(field) => Some(field),
                _ => None,
            })
        });
        let groups: Vec<Group> = types
            .iter()
            .map(|operand_type| match *operand_type {
                OperandType::UnsignedImmediate(field) if Some(field) == condition => {
                    Group::Condition
                }
                other => to_group(other),
            })
            .collect();
        let mut order: Vec<usize> = (0..types.len()).collect();
        order.sort_by_key(|index| groups[*index]);
        let mut rank = 0;
        let sources = types
            .iter()
            .zip(&groups)
            .map(|(operand_type, group)| {
                let source = matches!(
                    operand_type,
                    OperandType::RegisterValue { .. }
                        | OperandType::Address { .. }
                        | OperandType::FloatingPointValue { .. }
                        | OperandType::UnsignedImmediate(_)
                        | OperandType::SignedImmediate(_)
                ) && *group != Group::Condition;
                source.then(|| {
                    rank += 1;
                    rank - 1
                })
            })
            .collect();
        let writes_memory = LoadStoreInstruction::new(instruction).is_some_and(|load_store| {
            matches!(
                load_store.to_operation().transfer,
                Transfer::Store
                    | Transfer::StoreExclusive
                    | Transfer::Atomic(_)
                    | Transfer::CompareAndSwapPair
            )
        });
        let addressing = record.map_or(Vec::new(), |record| {
            record
                .operands
                .iter()
                .flat_map(|operand| match *operand {
                    Operand::AddressOffset { field, .. } => vec![field],
                    Operand::RegisterOffset {
                        number,
                        signed,
                        doubleword,
                        shift,
                    } => [Some(number), signed, doubleword, Some(shift)]
                        .into_iter()
                        .flatten()
                        .collect(),
                    _ => Vec::new(),
                })
                .collect()
        });
        let register_operands = groups
            .iter()
            .filter(|group| **group == Group::Register)
            .count();
        let register_assignments = to_register_assignments(domain, &types, &values);
        let layout = [
            OperandType::DataPagePermission(DataPage::Lower),
            OperandType::DataPagePermission(DataPage::Upper),
            OperandType::MemoryFill,
            OperandType::CodePlacement,
        ]
        .into_iter()
        .filter_map(|wanted| {
            types
                .iter()
                .position(|operand_type| *operand_type == wanted)
        })
        .collect();
        Self {
            domain,
            instruction,
            types,
            values,
            groups,
            order,
            sources,
            selection,
            writes_memory,
            addressing,
            reserved: record.map_or(&[], |record| record.reserved),
            register_operands,
            register_assignments,
            layout,
        }
    }

    /// Chooses the operands from `depth` in the order on; at the end, makes one state for each row of the operands
    /// that take each value once, each operand's values in turn from its own place in the rows.
    fn descend(
        &self,
        depth: usize,
        chosen: &mut Vec<usize>,
        facts: Facts,
        pending: &mut Vec<(usize, Vec<usize>)>,
        all: &mut Vec<Vec<usize>>,
    ) {
        let Some(&index) = self.order.get(depth) else {
            let rows = pending
                .iter()
                .map(|(_, values)| values.len())
                .max()
                .unwrap_or(1);
            for row in 0..rows {
                for (place, (operand, values)) in pending.iter().enumerate() {
                    chosen[*operand] = values[(row + place) % values.len()];
                }
                all.push(chosen.clone());
            }
            for (operand, values) in pending.iter() {
                chosen[*operand] = values[0];
            }
            return;
        };
        let group = self.groups[index];
        let facts = match depth
            .checked_sub(1)
            .map(|last| self.groups[self.order[last]])
        {
            Some(last) if last != group => self.learn(facts, last, group, chosen),
            _ => facts,
        };
        match self.allow(index, chosen, facts) {
            Allowed::Every(values) => {
                for value in values {
                    chosen[index] = value;
                    self.descend(depth + 1, chosen, facts, pending, all);
                }
            }
            Allowed::EachOnce(values) => {
                chosen[index] = values[0];
                pending.push((index, values));
                self.descend(depth + 1, chosen, facts, pending, all);
                pending.pop();
            }
        }
    }

    /// What the groups from `from` up to but not including `to` decide, once each of their operands is chosen.
    fn learn(&self, mut facts: Facts, from: Group, to: Group, chosen: &[usize]) -> Facts {
        for group in GROUPS
            .into_iter()
            .filter(|group| (from..to).contains(group))
        {
            match group {
                Group::Register => facts.held = self.find_held(chosen),
                Group::Placement => {
                    let placement = self.find(chosen, |operand_type| {
                        operand_type == OperandType::AccessPlacement
                    });
                    let page = |page| {
                        self.find(chosen, |operand_type| {
                            operand_type == OperandType::DataPagePermission(page)
                        })
                    };
                    let tag = self.find(chosen, |operand_type| {
                        operand_type == OperandType::AddressTag
                    });
                    let code = self.find(chosen, |operand_type| {
                        operand_type == OperandType::CodePlacement
                    });
                    let read_write = Some(OperandValue::PagePermission(PagePermission::Mapped {
                        data_access: DataAccess::ReadWrite,
                        instruction_fetch: InstructionFetch::Forbidden,
                    }));
                    let plain = placement
                        == Some(OperandValue::AccessPlacement(
                            AccessPlacement::LowerPageStart,
                        ))
                        && tag == Some(OperandValue::Bits(0))
                        && page(DataPage::Lower) == read_write;
                    facts.moved = (placement.is_some() && !plain)
                        || code == Some(OperandValue::CodePlacement(CodePlacement::PageEnd));
                    facts.partial_store = self.writes_memory
                        && placement
                            == Some(OperandValue::AccessPlacement(
                                AccessPlacement::StraddlesBoundary,
                            ))
                        && page(DataPage::Lower) == read_write
                        && page(DataPage::Upper) != read_write;
                }
                Group::Field => {
                    facts.word = encode_word(
                        self.instruction,
                        (0..self.types.len())
                            .filter(|index| {
                                matches!(
                                    self.groups[*index],
                                    Group::Register | Group::Condition | Group::Field
                                )
                            })
                            .map(|index| (self.types[index], self.values[index][chosen[index]])),
                    );
                    facts.undefined |= self
                        .reserved
                        .iter()
                        .any(|pattern| pattern.is_matching(facts.word));
                }
                Group::Condition => {}
                // An encoding that is undefined or that EL1 traps decides the outcome whatever the state holds.
                Group::Selector => {
                    if let Some(OperandValue::Bits(field)) = self.find(chosen, |operand_type| {
                        matches!(operand_type, OperandType::SystemRegister(_))
                    }) {
                        let write = self.instruction == Instruction::MsrRegister;
                        facts.undefined |= match to_system_register_access(field as u32) {
                            SystemRegisterAccess::Undefined | SystemRegisterAccess::Trapped => true,
                            SystemRegisterAccess::Fixed(_)
                            | SystemRegisterAccess::ReadOnlyZero
                            | SystemRegisterAccess::ReadTrapped
                            | SystemRegisterAccess::RandomNumber => write,
                            SystemRegisterAccess::Held(_) | SystemRegisterAccess::ReadsZero => {
                                false
                            }
                        };
                    }
                }
                Group::Flags => {
                    let flags = self.find(chosen, |operand_type| {
                        operand_type == OperandType::ConditionFlags
                    });
                    let condition = (0..self.types.len())
                        .find(|index| self.groups[*index] == Group::Condition)
                        .map(|index| (index, self.values[index][chosen[index]]));
                    facts.condition_holds = match (condition, flags) {
                        (
                            Some((_, OperandValue::Bits(condition))),
                            Some(OperandValue::ConditionFlags(flags)),
                        ) => Some(CONDITIONS[condition as usize].holds(flags)),
                        _ => None,
                    };
                    facts.path_representative = match (condition, facts.condition_holds) {
                        (Some((index, _)), Some(holds)) if self.selection != Selection::Always => {
                            chosen[index] == 0
                                && Some(chosen[self.flags_index()]) == self.find_first_flags(holds)
                        }
                        _ => true,
                    };
                }
                Group::Data => {
                    facts.nonfinite_source = false;
                    facts.subnormal_source = false;
                    for (index, operand_type) in self.types.iter().enumerate() {
                        let (OperandType::FloatingPointValue { precision, .. }
                        | OperandType::ConvertedFloatingPoint { precision, .. }) = *operand_type
                        else {
                            continue;
                        };
                        let OperandValue::Bits(bits) = self.values[index][chosen[index]] else {
                            continue;
                        };
                        if facts.held & (1 << index) == 0 {
                            continue;
                        }
                        let (exponent, fraction) = to_exponent_and_fraction(bits, precision);
                        let exponent_ones = (1 << precision.to_exponent_bits()) - 1;
                        facts.nonfinite_source |= exponent == exponent_ones;
                        facts.subnormal_source |= exponent == 0 && fraction != 0;
                    }
                }
                Group::Control => {
                    facts.first_control = self
                        .groups
                        .iter()
                        .zip(chosen)
                        .all(|(group, value)| *group != Group::Control || *value == 0);
                }
                Group::Written => {}
            }
        }
        facts
    }

    /// The operands whose value reaches the state once the register numbers are chosen: changing the value of one
    /// that does not leaves the state as it was.
    fn find_held(&self, chosen: &[usize]) -> u64 {
        let base: Vec<usize> = self
            .groups
            .iter()
            .zip(chosen)
            .map(|(group, value)| if *group == Group::Register { *value } else { 0 })
            .collect();
        let build = |choice: &[usize]| {
            let operands: Vec<OperandValue> = choice
                .iter()
                .zip(&self.values)
                .map(|(value, values)| values[*value])
                .collect();
            self.domain.build_state(self.instruction, &operands)
        };
        let unchanged = build(&base);
        let mut held = 0;
        for index in 0..self.types.len() {
            if self.groups[index] != Group::Data || self.values[index].len() < 2 {
                held |= 1 << index;
                continue;
            }
            let mut changed = base.clone();
            changed[index] = 1;
            if build(&changed) != unchanged {
                held |= 1 << index;
            }
        }
        held
    }

    fn find(&self, chosen: &[usize], wanted: impl Fn(OperandType) -> bool) -> Option<OperandValue> {
        self.types
            .iter()
            .position(|operand_type| wanted(*operand_type))
            .map(|index| self.values[index][chosen[index]])
    }

    /// How the operand takes its values, given the operands chosen before it.
    fn allow(&self, index: usize, chosen: &[usize], facts: Facts) -> Allowed {
        let group = self.groups[index];
        let every: Vec<usize> = (0..self.values[index].len()).collect();
        // A source the path does not read, and the FPCR it would read with, take every value that would matter if a
        // model read them, on the path's first condition and flags.
        if facts.path_representative && !facts.undefined {
            let unread_source = group == Group::Data
                && !self.is_read(index, facts)
                && facts.held & (1 << index) != 0;
            let unread_control = group == Group::Control && !self.compares(facts);
            if unread_source || unread_control {
                return Allowed::EachOnce(every);
            }
        }
        // An MRS or MSR reads or writes one register: the others the state holds take each value once.
        if let Some(selected) = self.find(chosen, |operand_type| {
            matches!(operand_type, OperandType::SystemRegister(_))
        }) && let Some(held) = to_held_register(self.types[index])
        {
            let OperandValue::Bits(field) = selected else {
                return Allowed::Every(every);
            };
            let reached = to_system_register_access(field as u32);
            if facts.undefined {
                return Allowed::Every(vec![to_representative(self.types[index], every.len())]);
            }
            if reached != SystemRegisterAccess::Held(held) {
                return Allowed::EachOnce(every);
            }
            return Allowed::Every(every);
        }
        let values = self.allow_values(index, chosen, facts);
        if !facts.register_representative
            && matches!(group, Group::Data | Group::Control | Group::Written)
        {
            Allowed::EachOnce(values)
        } else {
            Allowed::Every(values)
        }
    }

    /// Whether the instruction reads the operand on the path the condition and flags chose.
    fn is_read(&self, index: usize, facts: Facts) -> bool {
        match (self.selection, facts.condition_holds, self.sources[index]) {
            (Selection::OneOfTwo, Some(holds), rank) => rank != Some(usize::from(holds)),
            (Selection::ComparesWhenHolding, Some(holds), rank) => {
                (rank == Some(FLAGS_SOURCE)) != holds || rank.is_none()
            }
            _ => true,
        }
    }

    /// Whether the instruction compares anything: a conditional compare where its condition fails compares nothing,
    /// so it reads no control.
    fn compares(&self, facts: Facts) -> bool {
        !(self.selection == Selection::ComparesWhenHolding && facts.condition_holds == Some(false))
    }

    /// The values the operand takes, given the operands chosen before it.
    fn allow_values(&self, index: usize, chosen: &[usize], facts: Facts) -> Vec<usize> {
        let operand_type = self.types[index];
        let count = self.values[index].len();
        let all = (0..count).collect();
        let one = vec![to_representative(operand_type, count)];
        let read = self.is_read(index, facts);
        let path_one = vec![self.to_source_representative(index)];
        if !facts.path_representative
            && matches!(
                self.groups[index],
                Group::Field | Group::Data | Group::Control | Group::Written
            )
        {
            return match self.groups[index] {
                Group::Field if self.sources[index].is_none() => all,
                _ => path_one,
            };
        }
        let compares = self.compares(facts);
        match self.groups[index] {
            Group::Register | Group::Condition | Group::Selector => all,
            Group::Placement => {
                let placement = self.find(chosen, |operand_type| {
                    operand_type == OperandType::AccessPlacement
                });
                let reaches_upper = matches!(
                    placement,
                    Some(OperandValue::AccessPlacement(
                        AccessPlacement::StraddlesBoundary | AccessPlacement::UpperPageStart
                    ))
                );
                let reaches_lower = placement
                    != Some(OperandValue::AccessPlacement(
                        AccessPlacement::UpperPageStart,
                    ));
                // An address above those a program can use, but for its tag, faults at level 0 whatever its pages allow.
                let past_addressable = matches!(
                    self.find(chosen, |operand_type| operand_type == OperandType::AddressTag),
                    Some(OperandValue::Bits(high)) if high & !(u64::MAX << ADDRESS_TAG_BITS_SHIFT) != 0
                );
                match operand_type {
                    OperandType::DataPagePermission(_) if past_addressable => one,
                    OperandType::DataPagePermission(DataPage::Lower) if !reaches_lower => one,
                    OperandType::DataPagePermission(DataPage::Upper) if !reaches_upper => one,
                    _ => all,
                }
            }
            Group::Field if (facts.moved && !self.is_addressing(operand_type)) || !read => one,
            Group::Field => all,
            Group::Flags if facts.moved => one,
            Group::Flags => all,
            Group::Data => {
                let decided = facts.undefined
                    || (facts.moved && !facts.partial_store && !self.is_addressing(operand_type));
                if decided || facts.held & (1 << index) == 0 || !read {
                    one
                } else {
                    all
                }
            }
            Group::Control if facts.undefined || !compares => one,
            Group::Control => (0..count)
                .filter(|value| {
                    let OperandValue::Bits(control) = self.values[index][*value] else {
                        return true;
                    };
                    let mode = (control >> ROUNDING_MODE_SHIFT) & ((1 << ROUNDING_MODE_BITS) - 1);
                    let flush = control & (FLUSH_TO_ZERO | FLUSH_HALF_TO_ZERO);
                    let flushes_nothing = facts.nonfinite_source && !facts.subnormal_source;
                    (!facts.nonfinite_source || mode == 0) && (!flushes_nothing || flush == 0)
                })
                .collect(),
            Group::Written => match operand_type {
                _ if facts.undefined => one,
                OperandType::MemoryFill if facts.moved && !facts.partial_store => one,
                OperandType::MemoryFill => all,
                _ if facts.moved => one,
                // No flag and every flag with every control; each flag alone with the first.
                OperandType::StatusFlags if !facts.first_control => vec![0, 1],
                _ => all,
            },
        }
    }

    fn flags_index(&self) -> usize {
        self.types
            .iter()
            .position(|operand_type| *operand_type == OperandType::ConditionFlags)
            .unwrap_or(0)
    }

    /// The first value of the flags where the first condition holds, or where it fails.
    fn find_first_flags(&self, holds: bool) -> Option<usize> {
        let condition =
            (0..self.types.len()).find(|index| self.groups[*index] == Group::Condition)?;
        let OperandValue::Bits(first) = self.values[condition][0] else {
            return None;
        };
        self.values[self.flags_index()].iter().position(|flags| {
            matches!(flags, OperandValue::ConditionFlags(flags) if CONDITIONS[first as usize].holds(*flags) == holds)
        })
    }

    /// The value an operand takes off the representative path: a source takes one after its rank, so that two
    /// sources differ.
    fn to_source_representative(&self, index: usize) -> usize {
        let count = self.values[index].len();
        match self.sources[index] {
            Some(rank) => (1 + rank).min(count - 1),
            None => to_representative(self.types[index], count),
        }
    }

    /// Whether the operand is part of the offset an access adds to its base.
    fn is_addressing(&self, operand_type: OperandType) -> bool {
        match operand_type {
            OperandType::UnsignedImmediate(field)
            | OperandType::SignedImmediate(field)
            | OperandType::RegisterValue { field, .. } => self.addressing.contains(&field),
            _ => false,
        }
    }
}

/// Each choice of register numbers, as the value of each register operand: first every operand names a register of
/// its own, then each names each special register of its kind while the others keep theirs, then each two of a kind
/// name one register, an ordinary one and each special one. Operands that encode one field name one register. The
/// kinds are the general-purpose, the vector and the predicate registers.
///
/// # Panics
///
/// When an instruction names more registers of a kind than the domain gives distinct numbers.
fn to_register_assignments(
    domain: Arm64InputDomain,
    types: &[OperandType],
    values: &[Vec<OperandValue>],
) -> Vec<Vec<(usize, usize)>> {
    let field = |operand_type: OperandType| match operand_type {
        OperandType::RegisterNumber(field) => Some((field, RegisterKind::General)),
        OperandType::VectorRegisterNumber(field) => Some((field, RegisterKind::Vector)),
        OperandType::PredicateRegisterNumber(field) => Some((field, RegisterKind::Predicate)),
        _ => None,
    };
    // The first operand of each field, with its kind.
    let mut named: Vec<(usize, Field, RegisterKind)> = Vec::new();
    for (index, operand_type) in types.iter().enumerate() {
        if let Some((field, kind)) = field(*operand_type)
            && !named.iter().any(|(_, other, _)| *other == field)
        {
            named.push((index, field, kind));
        }
    }
    if named.is_empty() {
        return Vec::new();
    }
    let distinct = |index: usize| -> Vec<OperandValue> {
        domain
            .generate_distinct_register_numbers(types[index])
            .collect()
    };
    let special = |index: usize| -> Vec<OperandValue> {
        values[index]
            .iter()
            .copied()
            .filter(|value| !distinct(index).contains(value))
            .collect()
    };
    let mut used = [0_usize; REGISTER_KINDS];
    let base: Vec<OperandValue> = named
        .iter()
        .map(|(index, _, kind)| {
            let own = distinct(*index);
            let place = &mut used[*kind as usize];
            let Some(number) = own.get(*place).copied() else {
                panic!("more registers of one kind than distinct numbers in {types:?}")
            };
            *place += 1;
            number
        })
        .collect();
    let mut assignments = vec![base.clone()];
    for (place, (index, _, _)) in named.iter().enumerate() {
        for number in special(*index) {
            let mut assignment = base.clone();
            assignment[place] = number;
            assignments.push(assignment);
        }
    }
    for (first, (index, _, kind)) in named.iter().enumerate() {
        for (second, (_, _, other)) in named.iter().enumerate().skip(first + 1) {
            if kind != other {
                continue;
            }
            for number in [base[first]].into_iter().chain(special(*index)) {
                let mut assignment = base.clone();
                assignment[first] = number;
                assignment[second] = number;
                assignments.push(assignment);
            }
        }
    }
    let mut unique: Vec<Vec<OperandValue>> = Vec::new();
    for assignment in assignments {
        if !unique.contains(&assignment) {
            unique.push(assignment);
        }
    }
    unique
        .into_iter()
        .map(|assignment| {
            types
                .iter()
                .enumerate()
                .filter_map(|(index, operand_type)| {
                    let (operand_field, _) = field(*operand_type)?;
                    let place = named
                        .iter()
                        .position(|(_, field, _)| *field == operand_field)?;
                    let value = values[index]
                        .iter()
                        .position(|value| *value == assignment[place])?;
                    Some((index, value))
                })
                .collect()
        })
        .collect()
}

/// The kinds of register an operand can name, each with its own numbers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RegisterKind {
    General,
    Vector,
    Predicate,
}

const REGISTER_KINDS: usize = 3;

/// The register of the state an operand of an MRS or MSR gives a value.
const fn to_held_register(operand_type: OperandType) -> Option<HeldRegister> {
    match operand_type {
        OperandType::ConditionFlags => Some(HeldRegister::ConditionFlags),
        OperandType::FloatingPointControl(_) => Some(HeldRegister::FloatingPointControl),
        OperandType::StatusFlags => Some(HeldRegister::FloatingPointStatus),
        OperandType::SoftwareThreadId => Some(HeldRegister::SoftwareThreadId),
        _ => None,
    }
}

/// The critical value an operand takes where its value cannot change the outcome: the second, which is not zero, but
/// the first for the pattern in memory, which tells addresses apart, for FPCR, for a page's permission (unmapped, so
/// that an access that should not reach it faults where it does), and for a comparand (equal, so that it stores).
fn to_representative(operand_type: OperandType, count: usize) -> usize {
    let wanted = match operand_type {
        OperandType::MemoryFill
        | OperandType::FloatingPointControl(_)
        | OperandType::DataPagePermission(_)
        | OperandType::Comparand { .. } => 0,
        _ => 1,
    };
    wanted.min(count - 1)
}

/// The biased exponent and the fraction of a value of the precision.
const fn to_exponent_and_fraction(bits: u64, precision: Precision) -> (u64, u64) {
    let fraction_bits = precision.to_fraction_bits();
    let exponent_ones = (1 << precision.to_exponent_bits()) - 1;
    (
        (bits >> fraction_bits) & exponent_ones,
        bits & ((1 << fraction_bits) - 1),
    )
}
