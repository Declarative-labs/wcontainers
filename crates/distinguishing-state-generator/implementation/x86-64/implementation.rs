//! A generator of x86-64 states that forms every combination of the critical values of a form's operands, except
//! where the other operands' values already decide the state's outcome without its data: there each data operand
//! (a register's value, an immediate, a count, a vector register's or memory's value, CMPXCHG's comparison, MXCSR,
//! which decides nothing before the access) takes one value.
//!
//! The outcome does not depend on the data, by what the values mean, when:
//! - the code faults while it is fetched or measured (a layout across an unmapped page or the canonical edge, or
//!   longer than 15 bytes);
//! - the access faults whatever is stored: its first byte is not canonical, or a byte of it lies on a page that
//!   refuses the access (unmapped, read-only for a write, the kernel's half), or it is misaligned where the form
//!   requires alignment, or AC is set and it is misaligned;
//! - the value only moves where the instruction or its access lies, which no data operand reads: any memory operand
//!   but the plain base at the start of a writable page through the default segment with the code at its page's
//!   start, and any register form's layout but the page start, except a relative jump's, whose target moves with
//!   it.
//!
//! Flags are not data: they hold AC, which decides between faults, and the condition a form reads, so they keep
//! every critical value. Where a store faults part-way, what it writes below the fault depends on the value in
//! memory and, for CMPXCHG, on whether the accumulator equals it, and for an MMX store on the MMX value, so those keep
//! every critical value there too.
//!
//! The flags of a form without a memory operand that reads none of them are the one exception: nothing reads them
//! and AC decides nothing, so they only pass through. Where the form has another data operand they take their
//! values in turn instead of in the product, the sum of every other operand's index choosing, so that each meets
//! every value of every other operand.
//!
//! An EVEX writemask decides which lanes are written, independently of what is computed in them: where the data
//! take every combination, the state without a writemask takes each, and each other writemask takes those whose
//! data indices sum to its own index less one modulo the other writemasks' count, so that it meets every value of
//! every data operand and the data's combinations are not repeated for each writemask.
//!
//! Where the data take every combination, an EVEX memory form's opmask takes its turn as a writemask does (the
//! access lies at a writable page's start, so no element faults and the opmask only merges), and an EVEX form's
//! register choice at the page's start takes the combinations whose data indices plus the opmask's index sum to its
//! own index modulo the number of choices, so that each choice of registers meets every value of every data operand
//! but the data's combinations are not repeated for each; a form with fewer combinations than choices keeps them
//! all.
//!
//! Where the outcome does not depend on the data, an EVEX memory form's opmask takes its values in turn as a
//! writemask does where the access cannot fault, and its first source's register takes its values in turn, the sum
//! of the other structure operands' indices choosing: neither decides a fault.
//!
//! Two more operands of an EVEX form take their values in turn, the sum of the other structure operands' indices
//! choosing: the flags where the access is wider than eight bytes, which alignment checking never covers, so that
//! they only pass through; and the writemask where the outcome does not depend on the data and the access cannot
//! fault (or the code faults before it runs), so that the writemask decides no fault and the merging it does is
//! the plain access's.
//!
//! A form not yet modeled has one state, its sample encoding: a chip that lacks the form raises #UD whatever the
//! state holds.

#![no_std]

extern crate alloc;

mod evex;

use alloc::vec::{self, Vec};

use iced_x86::OpCodeOperandKind;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_instruction_input_domain_x86_64::integer::to_access_bytes;
use wcontainers_instruction_input_domain_x86_64::{segment, string, system};
use wcontainers_instruction_input_domain_x86_64::{
    Access, AccessPlacement, AddressSize, Addressing, CodeLayout, DataPlacement, FlagSet, LOWER_DATA_ADDRESS, OperandSize, OperandType,
    OperandValue, SegmentChoice, UPPER_DATA_ADDRESS, ValueSet, X86_64InputDomain, to_data_address,
};
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_x86_64::avx::{AvxOperation, find_avx_form};
use wcontainers_processor_architecture_x86_64::branch::find_branch_form;
use wcontainers_processor_architecture_x86_64::extension::{ExtensionForm, find_extension_form};
use wcontainers_processor_architecture_x86_64::mmx::find_mmx_form;
use wcontainers_processor_architecture_x86_64::stack::{StackForm, find_stack_form};
use wcontainers_processor_architecture_x86_64::x87::find_x87_form;
use wcontainers_processor_architecture_x86_64::evex::find_evex_form;
use wcontainers_processor_architecture_x86_64::integer::{OperandPlace, find_integer_form};
use wcontainers_processor_architecture_x86_64::float::find_float_form;
use wcontainers_processor_architecture_x86_64::vector::find_vector_form;
use wcontainers_processor_architecture_x86_64::sse::find_sse_form;
use wcontainers_processor_architecture_x86_64::avx512_integer::{EvexIntegerPlace, find_evex_integer_form};
use wcontainers_processor_architecture_x86_64::opmask::{OpmaskPlace, find_opmask_form};
use wcontainers_processor_architecture_x86_64::optional::find_optional_form;
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::{DataAccess, Flags, Instruction, PagePermission, X86_64};

/// Legacy SSE's 16-byte memory operands must be aligned to 16.
const SSE_ALIGNMENT: u64 = 16;

#[derive(Clone, Copy, Default, Debug)]
pub struct CriticalValueCut {
    domain: X86_64InputDomain,
}

impl CriticalValueCut {
    pub const fn new(domain: X86_64InputDomain) -> Self {
        Self { domain }
    }
}

/// What a form does with its memory operand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct AccessShape {
    write: bool,
    /// The alignment the form requires whatever AC holds.
    alignment: Option<u64>,
}

impl DistinguishingStateGenerator<X86_64> for CriticalValueCut {
    type States<'generator> = vec::IntoIter<ExecutionState<'static, X86_64>>;

    fn generate_states(&self, instruction: Instruction) -> vec::IntoIter<ExecutionState<'static, X86_64>> {
        let types: Vec<OperandType> = self.domain.list_operand_types(instruction).collect();
        // Only JCXZ, which no byte string encodes in 64-bit mode, has no operand.
        if types.is_empty() {
            return Vec::new().into_iter();
        }
        let values: Vec<Vec<OperandValue>> =
            types.iter().map(|operand_type| self.domain.generate_critical_values(*operand_type).collect()).collect();
        let data: Vec<bool> = types.iter().map(|operand_type| is_data(*operand_type)).collect();
        let shape = find_access_shape(instruction);
        let relative = find_branch_form(instruction).is_some_and(|form| !form.indirect);
        let mut states = Vec::new();
        let in_turn = find_flags_in_turn(&types, &data, &values);
        let structure: Vec<usize> = (0..types.len()).filter(|index| !data[*index] && Some(*index) != in_turn).collect();
        let data_operands: Vec<usize> = (0..types.len()).filter(|index| data[*index]).collect();
        let writemask = types.iter().position(|operand_type| matches!(operand_type, OperandType::Writemask { .. }));
        let evex_mask = types.iter().position(|operand_type| matches!(operand_type, OperandType::EvexMask(_)));
        let first_source = types.iter().position(|operand_type| *operand_type == OperandType::EvexRegister);
        let registers = types.iter().position(|operand_type| {
            matches!(operand_type, OperandType::EvexEncoding | OperandType::EvexRegister | OperandType::EvexIntegerEncoding { .. })
        });
        let evex = find_evex_form(instruction).is_some() || find_evex_integer_form(instruction).is_some();
        let in_memory = types.iter().any(|operand_type| matches!(operand_type, OperandType::Memory { .. }));
        let position = |index: Option<usize>| index.and_then(|index| structure.iter().position(|&operand| operand == index));
        let flags = position(types.iter().position(|operand_type| *operand_type == OperandType::Flags(FlagSet::Unread)));
        let writemask_position = position(writemask);
        for chosen in combine(structure.iter().map(|&index| values[index].len()).collect()) {
            let mut operands: Vec<Option<OperandValue>> = alloc::vec![None; types.len()];
            for (&index, &value) in structure.iter().zip(&chosen) {
                operands[index] = Some(values[index][value]);
            }
            let width = find_access_bytes(instruction, &operands);
            let decided = operands.iter().flatten().any(|value| decides_without_data(*value, shape, width, relative))
                || is_misaligned_under_alignment_check(&operands, width)
                || string::decides_without_data(instruction, &operands)
                || system::decides_without_data(instruction, &operands)
                || segment::decides_without_data(instruction, &operands);
            // Whether the structure operand at the position takes another value than its turn.
            let out_of_turn = |position: usize| {
                let others = chosen.iter().enumerate().filter(|(other, _)| *other != position).map(|(_, value)| value).sum::<usize>();
                chosen[position] != others % values[structure[position]].len()
            };
            if evex && in_memory && width > 8 && flags.is_some_and(out_of_turn) {
                continue;
            }
            let access = operands.iter().flatten().find_map(|value| match value {
                OperandValue::Access(access) => Some(*access),
                _ => None,
            });
            let mask_decides = access.is_some_and(|access| {
                !access.layout.faults_before_execution()
                    && (shape.is_some_and(|shape| faults(access, shape, width)) || is_misaligned_under_alignment_check(&operands, width))
            });
            if decided && access.is_some() && !mask_decides && writemask_position.is_some_and(out_of_turn) {
                continue;
            }
            // An EVEX memory form's opmask likewise where the access cannot fault, and its first source's register
            // wherever the outcome does not depend on the data.
            if evex && decided && access.is_some() && !mask_decides && position(evex_mask).is_some_and(out_of_turn) {
                continue;
            }
            if evex && decided && position(first_source).is_some_and(out_of_turn) {
                continue;
            }
            let in_part = operands.iter().flatten().any(|value| stores_in_part(*value, shape, width));
            let data_choices: Vec<Vec<usize>> = if decided {
                // Each data operand one value, but the value in memory and CMPXCHG's comparison every value where the
                // store faults part-way: they decide what it writes below the fault.
                let counts = data_operands
                    .iter()
                    .map(|&index| match types[index] {
                        OperandType::MemoryValue(_) | OperandType::Comparison | OperandType::MmxValue(..)
                        | OperandType::X87Value(..)
                        | OperandType::X87Stack
                        | OperandType::X87Case(_)
                            if in_part =>
                        {
                            values[index].len()
                        }
                        _ => 1,
                    })
                    .collect();
                combine(counts)
                    .into_iter()
                    .map(|choice| {
                        choice
                            .iter()
                            .zip(&data_operands)
                            .map(|(&value, &index)| match types[index] {
                                OperandType::MemoryValue(_) | OperandType::Comparison | OperandType::MmxValue(..)
                                | OperandType::X87Value(..)
                                | OperandType::X87Stack
                                | OperandType::X87Case(_)
                                    if in_part =>
                                {
                                    value
                                }
                                _ => to_representative(types[index], values[index].len()),
                            })
                            .collect()
                    })
                    .collect()
            } else {
                let all = combine(data_operands.iter().map(|&index| values[index].len()).collect());
                // The writemask's index among its values; the first is no writemask. An EVEX memory form's opmask
                // takes its turn the same way where the access cannot fault.
                let mask = writemask.or(if evex { evex_mask } else { None });
                let turn = mask.and_then(|index| {
                    let position = structure.iter().position(|&operand| operand == index)?;
                    (chosen[position] > 0 && values[index].len() > 2).then(|| (chosen[position] - 1, values[index].len() - 1))
                });
                let mask_index = mask.and_then(|index| structure.iter().position(|&operand| operand == index)).map_or(0, |position| chosen[position]);
                let all: Vec<Vec<usize>> = match turn {
                    Some((turn, others)) => all.into_iter().filter(|choice| choice.iter().sum::<usize>() % others == turn).collect(),
                    None => all,
                };
                // The register choice at the page's start, among the register choices there.
                let register_turn = registers.filter(|_| evex).and_then(|index| {
                    let position = structure.iter().position(|&operand| operand == index)?;
                    let at_start: Vec<usize> = (0..values[index].len()).filter(|&value| starts_page(values[index][value])).collect();
                    let turn = at_start.iter().position(|&value| value == chosen[position])?;
                    (at_start.len() > 1 && all.len() >= at_start.len()).then_some((turn, at_start.len()))
                });
                match register_turn {
                    Some((turn, choices)) => {
                        all.into_iter().filter(|choice| (choice.iter().sum::<usize>() + mask_index) % choices == turn).collect()
                    }
                    None => all,
                }
            };
            for choice in data_choices {
                for (&index, &value) in data_operands.iter().zip(&choice) {
                    operands[index] = Some(values[index][value]);
                }
                if let Some(index) = in_turn {
                    let turn = chosen.iter().chain(&choice).sum::<usize>() % values[index].len();
                    operands[index] = Some(values[index][turn]);
                }
                let operands: Vec<OperandValue> = operands.iter().map(|value| value.expect("every operand")).collect();
                states.push(self.domain.build_state(instruction, &operands));
            }
        }
        states.into_iter()
    }
}

/// Whether a value of a register operand puts the code at its page's start, where the data take their combinations.
fn starts_page(value: OperandValue) -> bool {
    match value {
        OperandValue::EvexEncoding(encoding) => encoding.layout == CodeLayout::PageStart,
        OperandValue::EvexIntegerEncoding(encoding) => encoding.layout == CodeLayout::PageStart,
        _ => true,
    }
}

/// The flags operand that takes its values in turn: the unread flags of a form with no memory operand and another
/// data operand of more than one value.
fn find_flags_in_turn(types: &[OperandType], data: &[bool], values: &[Vec<OperandValue>]) -> Option<usize> {
    let in_memory = types
        .iter()
        .any(|operand_type| matches!(operand_type, OperandType::Memory { .. } | OperandType::AccessPlacement(_)));
    let other_data = (0..types.len()).any(|index| data[index] && values[index].len() > 1);
    let flags = types.iter().position(|operand_type| *operand_type == OperandType::Flags(FlagSet::Unread));
    flags.filter(|_| !in_memory && other_data)
}

/// The critical value a data operand takes where the outcome does not depend on the data: the second, which is not
/// zero, and for the value in memory the third, so that every operation's result differs from what memory holds
/// and a store of it, whole or in part, changes memory.
fn to_representative(operand_type: OperandType, count: usize) -> usize {
    let wanted = if matches!(operand_type, OperandType::MemoryValue(_)) { 2 } else { 1 };
    wanted.min(count - 1)
}

/// Every combination of one index below each count, the last changing fastest; one empty combination for none.
fn combine(counts: Vec<usize>) -> Vec<Vec<usize>> {
    let mut all = alloc::vec![Vec::new()];
    for count in counts {
        all = all
            .into_iter()
            .flat_map(|prefix: Vec<usize>| {
                (0..count).map(move |value| {
                    let mut next = prefix.clone();
                    next.push(value);
                    next
                })
            })
            .collect();
    }
    all
}

/// Whether the operand holds data the instruction computes with, rather than where things lie or what decides them;
/// a bit test's offset moves its access, so it is not data.
fn is_data(operand_type: OperandType) -> bool {
    match operand_type {
        OperandType::RegisterValue(_, set) => set != ValueSet::BitOffset,
        OperandType::MemoryValue(_)
        | OperandType::Immediate(_)
        | OperandType::ShiftCount
        | OperandType::VectorValue(..)
        | OperandType::FloatOperands(_)
        | OperandType::Mxcsr(_)
        | OperandType::Comparison
        | OperandType::CpuidLeaf
        | OperandType::CpuidSubleaf
        | OperandType::CpuidUpperHalves
        | OperandType::AvxData(_)
        | OperandType::MmxValue(..)
        | OperandType::X87Environment
        | OperandType::X87Stack
        | OperandType::X87Value(..)
        | OperandType::EvexFloatData(_)
        | OperandType::EvexIntegerData(_)
        | OperandType::EvexFusedData(_)
        | OperandType::EvexConvertData(_)
        | OperandType::EvexCompareData(_)
        | OperandType::EvexMoveData(_)
        | OperandType::EvexSpecialData(_)
        | OperandType::EvexGatherData(_)
        | OperandType::OpmaskValue { .. }
        | OperandType::ZmmValue(..)
        | OperandType::StringData(_)
        | OperandType::PoppedFlags
        | OperandType::ConvertSource(_)
        | OperandType::X87Case(_)
        | OperandType::Selector
        | OperandType::ReciprocalSource
        | OperandType::SegmentSelector
        | OperandType::SegmentLoad
        | OperandType::FarPointer
        | OperandType::IretFrame
        | OperandType::HypercallNumber
        | OperandType::KeyRights
        | OperandType::OptionalVector(_)
        | OperandType::OptionalMxcsr { .. }
        | OperandType::OptionalBase => true,
        OperandType::Sse(operand) => operand.is_data(),
        _ => false,
    }
}

/// Whether the form writes its memory operand, and the alignment it requires; `None` for a form without one.
fn find_access_shape(instruction: Instruction) -> Option<AccessShape> {
    use OpCodeOperandKind as Kind;
    if let Some(form) = find_mmx_form(instruction) {
        return (form.place != OperandPlace::Register).then_some(AccessShape { write: form.writes_memory(), alignment: None });
    }
    if let Some(form) = find_x87_form(instruction) {
        return form.has_memory_operand().then_some(AccessShape { write: form.writes_memory(), alignment: None });
    }
    if let Some(form) = find_stack_form(instruction) {
        return (form.place != OperandPlace::Register).then_some(AccessShape { write: form.writes_memory(), alignment: None });
    }
    if let Some(form) = find_extension_form(instruction) {
        return (form.place != OperandPlace::Register).then_some(AccessShape {
            write: form.writes_memory(),
            alignment: form.required_alignment(),
        });
    }
    if let Some(form) = find_integer_form(instruction) {
        if form.place == OperandPlace::Register {
            return None;
        }
        let destination = form.codes[0].op_code().op_kind(0);
        let in_destination =
            matches!(destination, Kind::r8_or_mem | Kind::r16_or_mem | Kind::r32_or_mem | Kind::r64_or_mem | Kind::mem_offs);
        return Some(AccessShape {
            write: in_destination && form.operation.writes_destination(),
            alignment: None,
        });
    }
    if let Some(form) = find_sse_form(instruction) {
        return (form.place != OperandPlace::Register).then_some(AccessShape {
            write: form.writes_memory(),
            alignment: form.requires_alignment().then_some(SSE_ALIGNMENT),
        });
    }
    if let Some(form) = find_float_form(instruction) {
        return (form.place != OperandPlace::Register).then_some(AccessShape {
            write: false,
            alignment: (form.memory_bytes() == 16).then_some(SSE_ALIGNMENT),
        });
    }
    if let Some(form) = find_avx_form(instruction) {
        // A masked form's mask decides whether its access faults, so no placement decides that alone.
        let masked = matches!(form.operation, AvxOperation::MaskedLoad(_) | AvxOperation::MaskedStore(_));
        return (form.place != OperandPlace::Register && !masked).then_some(AccessShape {
            write: form.writes_memory(),
            alignment: form.aligned.then_some(u64::from(form.memory_bytes)),
        });
    }
    if let Some(form) = find_opmask_form(instruction) {
        return matches!(form.place, OpmaskPlace::Load | OpmaskPlace::Store)
            .then_some(AccessShape { write: form.place == OpmaskPlace::Store, alignment: None });
    }
    if let Some(form) = find_evex_form(instruction) {
        return (form.place != OperandPlace::Register).then_some(AccessShape { write: form.stores(), alignment: None });
    }
    if let Some(form) = find_evex_integer_form(instruction) {
        return matches!(form.place, EvexIntegerPlace::Memory | EvexIntegerPlace::Store)
            .then_some(AccessShape { write: form.place == EvexIntegerPlace::Store, alignment: form.to_alignment() });
    }
    if let Some(form) = find_optional_form(instruction) {
        // A writemask decides whether a masked form's access faults, so no placement decides that alone.
        return (form.place != OperandPlace::Register && !form.can_mask())
            .then_some(AccessShape { write: form.writes_memory(), alignment: None });
    }
    find_vector_form(instruction).filter(|form| form.place != OperandPlace::Register).map(|form| AccessShape {
        write: false,
        alignment: form.requires_alignment().then_some(SSE_ALIGNMENT),
    })
}

/// The bytes the form's memory operand reaches, from the operand size and the form.
fn find_access_bytes(instruction: Instruction, operands: &[Option<OperandValue>]) -> u64 {
    if let Some(form) = find_evex_form(instruction) {
        return evex::to_reference_bytes(&form, operands);
    }
    if let Some(form) = find_vector_form(instruction) {
        return form.memory_bytes();
    }
    if let Some(form) = find_float_form(instruction) {
        return form.memory_bytes();
    }
    if let Some(form) = find_avx_form(instruction) {
        return u64::from(form.memory_bytes);
    }
    if let Some(form) = find_opmask_form(instruction) {
        return u64::from(form.bits / 8);
    }
    if let Some(form) = find_evex_integer_form(instruction) {
        let broadcast = operands.iter().flatten().any(|value| *value == OperandValue::LaneBroadcast(true));
        return form.memory_bytes(broadcast);
    }
    if let Some(form) = find_optional_form(instruction) {
        let broadcast = operands.iter().flatten().any(|value| *value == OperandValue::LaneBroadcast(true));
        return form.memory_bytes(broadcast);
    }
    let size = operands.iter().flatten().find_map(|value| match value {
        OperandValue::OperandSize(size) => Some(*size),
        _ => None,
    });
    if let Some(form) = find_sse_form(instruction) {
        return wcontainers_instruction_input_domain_x86_64::sse::to_access_bytes(form, size);
    }
    if let Some(form) = find_mmx_form(instruction) {
        return form.memory_bytes();
    }
    if let Some(form) = find_x87_form(instruction) {
        return form.memory_bytes();
    }
    if let Some(form) = find_stack_form(instruction) {
        let code = match size {
            Some(OperandSize::Bits64) => form.codes[form.codes.len() - 1],
            Some(OperandSize::Bits32) if form.codes.len() == 3 => form.codes[1],
            _ => form.codes[0],
        };
        return StackForm::to_memory_bytes(code);
    }
    if let Some(form) = find_extension_form(instruction) {
        let code = match size {
            Some(OperandSize::Bits32) if form.codes.len() == 3 => form.codes[1],
            Some(OperandSize::Bits64) if form.codes.len() == 3 => form.codes[2],
            _ => form.codes[0],
        };
        return ExtensionForm::to_memory_bytes(code);
    }
    match find_integer_form(instruction) {
        Some(form) => to_access_bytes(form, size),
        None => size.map_or(1, |size| size.to_bytes()),
    }
}

/// Whether the value decides the state's outcome whatever the data operands hold.
/// A relative jump's target moves with the code, so only a layout that faults first decides it.
fn decides_without_data(value: OperandValue, shape: Option<AccessShape>, width: u64, relative: bool) -> bool {
    match value {
        OperandValue::Encoding(encoding) if relative => encoding.layout.faults_before_execution(),
        OperandValue::Encoding(encoding) => encoding.layout != CodeLayout::PageStart,
        OperandValue::EvexEncoding(encoding) => encoding.layout != CodeLayout::PageStart,
        OperandValue::OpmaskEncoding(encoding) => encoding.layout != CodeLayout::PageStart,
        OperandValue::EvexIntegerEncoding(encoding) => encoding.layout != CodeLayout::PageStart,
        OperandValue::Access(access) => !is_plain(access) || shape.is_some_and(|shape| faults(access, shape, width)),
        _ => false,
    }
}

/// The access every other value of a memory operand is a variation of: the first base, the default segment, the
/// start of a writable page, the code at its page's start.
fn is_plain(access: Access) -> bool {
    let plain_addressing = matches!(
        access.addressing,
        Addressing::Base { base: 3, displacement: 0 } | Addressing::Offset
    ) && access.address_size == AddressSize::Bits64;
    let plain_data = matches!(
        access.data,
        DataPlacement::Pages {
            placement: AccessPlacement::LowerPageStart,
            lower: PagePermission::Mapped { data_access: DataAccess::ReadWrite, .. },
            ..
        }
    );
    plain_addressing && plain_data && access.segment == SegmentChoice::Default && access.layout == CodeLayout::PageStart
}

/// Whether the access faults whatever is stored and whatever the flags hold.
fn faults(access: Access, shape: AccessShape, width: u64) -> bool {
    let address = to_data_address(access.data, width);
    if shape.alignment.is_some_and(|alignment| !address.is_multiple_of(alignment)) {
        return true;
    }
    match access.data {
        DataPlacement::Pages { placement, lower, upper } => {
            placement == AccessPlacement::NonCanonical
                || (address..address + width).any(|byte| {
                    let permission = if (LOWER_DATA_ADDRESS..UPPER_DATA_ADDRESS).contains(&byte) {
                        lower
                    } else if (UPPER_DATA_ADDRESS..UPPER_DATA_ADDRESS + PAGE_BYTES).contains(&byte) {
                        upper
                    } else {
                        PagePermission::Unmapped
                    };
                    match permission {
                        PagePermission::Unmapped => true,
                        PagePermission::Mapped { data_access, .. } => shape.write && data_access == DataAccess::ReadOnly,
                    }
                })
        }
        DataPlacement::EndsAtCanonicalEdge | DataPlacement::BelowFourGiB => false,
        DataPlacement::Kernel
        | DataPlacement::UpperHalf
        | DataPlacement::AcrossCanonicalEdge
        | DataPlacement::MisalignedNonCanonical
        | DataPlacement::ZeroExtendedSignBit => true,
    }
}

/// Whether the access is a store that faults part-way, so that the bytes below the fault may be written: across the
/// data pages' boundary onto a page that refuses it from a writable one, or across the canonical edge.
fn stores_in_part(value: OperandValue, shape: Option<AccessShape>, width: u64) -> bool {
    let OperandValue::Access(access) = value else {
        return false;
    };
    let writable = |permission| matches!(permission, PagePermission::Mapped { data_access: DataAccess::ReadWrite, .. });
    shape.is_some_and(|shape| shape.write)
        && width > 1
        && match access.data {
            DataPlacement::Pages {
                placement: AccessPlacement::StraddlesBoundary,
                lower,
                upper,
            } => writable(lower) && !writable(upper),
            DataPlacement::AcrossCanonicalEdge => true,
            _ => false,
        }
}

/// Whether AC is set and the access of 2 to 8 bytes is misaligned, so that it faults with #AC whatever is stored,
/// the first byte being canonical, or with #GP or #SS where it is not.
fn is_misaligned_under_alignment_check(operands: &[Option<OperandValue>], width: u64) -> bool {
    let alignment_check = operands.iter().flatten().any(|value| {
        matches!(value, OperandValue::Flags(flags) if flags.contains(Flags::ALIGNMENT_CHECK))
    });
    let misaligned = operands.iter().flatten().any(|value| {
        matches!(value, OperandValue::Access(access) if !to_data_address(access.data, width).is_multiple_of(width))
    });
    alignment_check && (2..=8).contains(&width) && misaligned
}
