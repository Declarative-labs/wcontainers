//! The inputs of the segment loads and far transfers: the operand size; the register or memory operand; the segment
//! register MOV writes; where the stack access of POP, RET and IRET lies, or the far CALL's stack; and the selector,
//! far pointer or IRET frame, with the flags.
//!
//! The selectors are those of [`crate::descriptor`]. A far pointer is each of them with a target in the code page, a
//! non-canonical one and a canonical one above 4 GiB. An IRET frame is a plain return, and one at a time each part
//! changed: CS to the 32-bit code segment, to an RPL below 3, to data, to null; SS to null, to an RPL below 3, to code,
//! to read-only data; RFLAGS to none, every bit, IOPL, TF, RF, VM, and AC with ID and NT; the target out of range for
//! each mode; RSP not canonical.

use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use core::array;

use iced_x86::{Code, Instruction as Encoded, Register};
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::memory::PAGE_BYTES;
use wcontainers_processor_architecture_x86_64::segment::{SegmentForm, SegmentOperation, find_segment_form};
use wcontainers_processor_architecture_x86_64::{
    DataAccess, Flags, Instruction, InstructionFetch, PagePermission, Processor, STACK_POINTER, X86_64, ZERO_PROCESSOR,
};

use crate::integer::{CodeLayout, ImmediateClass, encode_at_code_address, lay_out, place_memory_operand, to_padding};
use crate::string::{PLAIN, place, refuses, to_address};
use crate::{
    CODE_ADDRESS, FlagSet, GENERAL_16, GENERAL_32, GENERAL_64, OperandSize, OperandSizes, OperandType, OperandValue, Splits,
};

/// The far CALL's stack: a page of its own.
const STACK_PAGE: u64 = 0x6000;
const STACK_TOP: u64 = STACK_PAGE + PAGE_BYTES / 2;
/// A target in the code page.
const NEAR_TARGET: u64 = CODE_ADDRESS + 0x40;
const SEGMENTS: [Register; 5] = [Register::ES, Register::SS, Register::DS, Register::FS, Register::GS];
/// What LFS, LGS and LSS's offset holds.
const OFFSET: u64 = 0x0123_4567_89AB_CDEF;
const READ_WRITE: PagePermission =
    PagePermission::Mapped { data_access: DataAccess::ReadWrite, instruction_fetch: InstructionFetch::Forbidden };

/// Where the far CALL's pushes go.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallStack {
    /// The middle of a writable page of its own.
    Own,
    /// RSP on the first byte of an unmapped page, so that both pushes fault.
    Unmapped,
    /// RSP not canonical.
    NonCanonical,
    /// RSP half a push above the end of a writable page with the page above unmapped, so that CS's push, the first,
    /// faults part-way.
    AcrossPage,
}

/// The five slots IRET pops.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IretFrame {
    pub offset: u64,
    pub code: u64,
    pub flags: u64,
    pub stack: u64,
    pub stack_segment: u64,
}

pub fn generate_selectors() -> Vec<OperandValue> {
    crate::descriptor::generate_selectors()
}

pub fn generate_far_pointers() -> Vec<OperandValue> {
    let selectors = [0x33, 0x30, 0x23, 0x20, 0x2B, 0x10, 0x00, 0x03, 0x7B, 0x43, 0x80, 0x37];
    selectors
        .iter()
        .flat_map(|&selector| [NEAR_TARGET, 1 << 47, 0x1_0000_1040].map(move |offset| OperandValue::FarPointer { offset, selector }))
        .collect()
}

pub fn generate_frames() -> Vec<OperandValue> {
    let plain = IretFrame { offset: NEAR_TARGET, code: 0x33, flags: 0x202, stack: 0x7000, stack_segment: 0x2B };
    let mut frames = vec![plain];
    frames.extend([0x23, 0x30, 0x2B, 0x00, 0x20].map(|code| IretFrame { code, ..plain }));
    frames.extend([0x00, 0x28, 0x33, 0x7B].map(|stack_segment| IretFrame { stack_segment, ..plain }));
    frames.extend([0, u64::MAX, 0x3202, 0x302, 0x1_0202, 0x2_0202, 0x24_4202].map(|flags| IretFrame { flags, ..plain }));
    frames.push(IretFrame { offset: 1 << 47, ..plain });
    frames.push(IretFrame { offset: 0x1_0000_1040, code: 0x23, ..plain });
    frames.push(IretFrame { stack: 1 << 47, ..plain });
    frames.into_iter().map(OperandValue::IretFrame).collect()
}

pub fn generate_call_stacks() -> Vec<OperandValue> {
    [CallStack::Own, CallStack::Unmapped, CallStack::NonCanonical, CallStack::AcrossPage].map(OperandValue::CallStack).to_vec()
}

/// IRET's stack placements for a frame of `bytes`: the system family's whose five slots lie on their own size. Once
/// IRET has loaded AC from its frame, a misaligned pop raises #AC, which KVM under the host's split-lock detection
/// takes for a split lock and retries for ever, so the chip source cannot run such a state.
pub fn generate_frame_layouts(bytes: u64) -> Vec<OperandValue> {
    crate::system::generate_layouts(bytes)
        .into_iter()
        .filter(|value| match value {
            OperandValue::SystemLayout(layout) => to_address(layout.placement, bytes, false).is_multiple_of(bytes / 5),
            _ => true,
        })
        .collect()
}

/// The bytes the form's stack access reaches at its smallest operand size, which its placements are made for.
fn to_stack_bytes(form: &SegmentForm) -> u64 {
    match form.codes[0] {
        _ if form.operation == SegmentOperation::Pop => 8,
        _ if form.operation == SegmentOperation::ReturnFar => 16,
        Code::Iretw => 10,
        Code::Iretd => 20,
        _ => 40,
    }
}

pub fn list_operand_types(form: &SegmentForm) -> Vec<OperandType> {
    let mut types = Vec::new();
    match form.codes.len() {
        2 if form.operation == SegmentOperation::Move => types.push(OperandType::OperandSize(OperandSizes::WithoutSixteen)),
        2 => types.push(OperandType::OperandSize(OperandSizes::WithoutThirtyTwo)),
        3 => types.push(OperandType::OperandSize(OperandSizes::All)),
        _ => {}
    }
    match form.operation {
        SegmentOperation::Move => {
            types.push(if form.in_memory { OperandType::Memory { splits: Splits::Allowed, offset: false } } else { OperandType::Encoding(1) });
            types.push(OperandType::SegmentLoad);
        }
        SegmentOperation::LoadFar => types.extend([OperandType::Memory { splits: Splits::Allowed, offset: false }, OperandType::SegmentSelector]),
        SegmentOperation::CallFar => {
            types.extend([OperandType::Memory { splits: Splits::Allowed, offset: false }, OperandType::CallStack, OperandType::FarPointer]);
        }
        SegmentOperation::JumpFar => types.extend([OperandType::Memory { splits: Splits::Allowed, offset: false }, OperandType::FarPointer]),
        SegmentOperation::Pop => types.extend([OperandType::SystemLayout { bytes: to_stack_bytes(form) }, OperandType::SegmentSelector]),
        SegmentOperation::ReturnFar => {
            types.extend([OperandType::SystemLayout { bytes: to_stack_bytes(form) }, OperandType::FarPointer]);
            if form.codes[0] == Code::Retfw_imm16 {
                types.push(OperandType::Immediate(ImmediateClass::Relative8));
            }
        }
        SegmentOperation::InterruptReturn => {
            types.extend([OperandType::FrameLayout { bytes: to_stack_bytes(form) }, OperandType::IretFrame]);
            types.push(OperandType::Flags(FlagSet::Condition(Flags::NESTED_TASK)));
            return types;
        }
    }
    types.push(OperandType::Flags(FlagSet::Unread));
    types
}

/// Writes the bytes from the address, those in the program's addresses.
fn write_at(address: u64, bytes: &[u8], memory: &mut Vec<MemoryDelta<'static, X86_64>>) {
    let held: Vec<u8> = bytes
        .iter()
        .enumerate()
        .take_while(|(offset, _)| X86_64::ADDRESSABLE.contains(&address.wrapping_add(*offset as u64)))
        .map(|(_, byte)| *byte)
        .collect();
    if !held.is_empty() {
        memory.push(MemoryDelta::ValueAssignment { start: address, bytes: Cow::Owned(held) });
    }
}

pub fn build_state(form: &SegmentForm, operands: &[(OperandType, OperandValue)]) -> ExecutionState<'static, X86_64> {
    let mut processor = Processor {
        general_purpose_registers: array::from_fn(|number| !(number as u64)),
        instruction_pointer: CODE_ADDRESS,
        vector_registers: array::from_fn(|number| !(number as u128)),
        upper_vector_halves: crate::fill_upper_vector_halves(),
        ..ZERO_PROCESSOR
    };
    let (mut size, mut layout, mut registers, mut access) = (None, CodeLayout::PageStart, [0, 0], None);
    let (mut segment, mut selector, mut pointer, mut frame, mut stack, mut immediate, mut placement) = (0, 0, None, None, CallStack::Own, 0, PLAIN);
    for (operand_type, operand) in operands {
        match (operand_type, *operand) {
            (OperandType::OperandSize(_), OperandValue::OperandSize(chosen)) => size = Some(chosen),
            (OperandType::Encoding(_), OperandValue::Encoding(encoding)) => (registers, layout) = (encoding.registers, encoding.layout),
            (OperandType::Memory { .. }, OperandValue::Access(chosen)) => {
                registers[0] = chosen.register;
                layout = chosen.layout;
                access = Some(chosen);
            }
            (OperandType::SystemLayout { .. } | OperandType::FrameLayout { .. }, OperandValue::SystemLayout(chosen)) => {
                (placement, layout) = (chosen.placement, chosen.code);
            }
            (OperandType::SegmentLoad, OperandValue::SegmentLoad { segment: chosen_segment, selector: chosen }) => {
                (segment, selector) = (chosen_segment, chosen);
            }
            (OperandType::SegmentSelector, OperandValue::Bits(chosen)) => selector = chosen as u16,
            (OperandType::FarPointer, OperandValue::FarPointer { offset, selector }) => pointer = Some((offset, selector)),
            (OperandType::IretFrame, OperandValue::IretFrame(chosen)) => frame = Some(chosen),
            (OperandType::CallStack, OperandValue::CallStack(chosen)) => stack = chosen,
            (OperandType::Immediate(_), OperandValue::Immediate(chosen)) => immediate = chosen as u16,
            (OperandType::Flags(_), OperandValue::Flags(flags)) => processor.flags = flags,
            (operand_type, operand) => panic!("{operand:?} is no value of {operand_type:?}"),
        }
    }
    let code = match size {
        None | Some(OperandSize::Bits16) => form.codes[0],
        Some(OperandSize::Bits32) if form.codes.len() == 3 => form.codes[1],
        Some(OperandSize::Bits32) => form.codes[0],
        Some(OperandSize::Bits64) => form.codes[form.codes.len() - 1],
    };
    let bytes: u64 = match size {
        Some(OperandSize::Bits16) => 2,
        Some(OperandSize::Bits32) => 4,
        _ => 8,
    };
    let bytes = match code {
        Code::Popw_FS | Code::Popw_GS | Code::Iretw => 2,
        Code::Iretd => 4,
        Code::Iretq => 8,
        _ => bytes,
    };
    let mut memory_mutations = Vec::new();
    // The stack access of POP, RET and IRET at the placement.
    if matches!(form.operation, SegmentOperation::Pop | SegmentOperation::ReturnFar | SegmentOperation::InterruptReturn) {
        place(placement, false, &mut memory_mutations);
        let slots: u64 = match form.operation {
            SegmentOperation::Pop => 1,
            SegmentOperation::ReturnFar => 2,
            _ => 5,
        };
        let address = to_address(placement, slots * bytes, false);
        processor.general_purpose_registers[STACK_POINTER] = address;
        let values: Vec<u64> = match form.operation {
            SegmentOperation::Pop => vec![0xA5A5_A5A5_A5A5_0000 | u64::from(selector)],
            SegmentOperation::ReturnFar => {
                let (offset, far_selector) = pointer.expect("a far pointer");
                vec![offset, 0xA5A5_A5A5_A5A5_0000 | u64::from(far_selector)]
            }
            _ => {
                let frame = frame.expect("a frame");
                vec![frame.offset, frame.code, frame.flags, frame.stack, frame.stack_segment]
            }
        };
        let image: Vec<u8> = values.iter().flat_map(|value| value.to_le_bytes()[..bytes as usize].to_vec()).collect();
        write_at(address, &image, &mut memory_mutations);
    }
    if form.operation == SegmentOperation::CallFar {
        let (rsp, pages): (u64, Vec<(u64, PagePermission)>) = match stack {
            CallStack::Own => (STACK_TOP, vec![(STACK_PAGE, READ_WRITE)]),
            CallStack::Unmapped => (STACK_PAGE, vec![]),
            CallStack::NonCanonical => (1 << 47, vec![]),
            CallStack::AcrossPage => (STACK_PAGE + PAGE_BYTES + bytes / 2, vec![(STACK_PAGE, READ_WRITE)]),
        };
        processor.general_purpose_registers[STACK_POINTER] = rsp;
        for (page, permission) in pages {
            memory_mutations.extend([
                MemoryDelta::ValueAssignment { start: page, bytes: Cow::Owned(vec![0x5A; PAGE_BYTES as usize]) },
                MemoryDelta::PermissionAssignment { addresses: page..page + PAGE_BYTES, permission },
            ]);
        }
    }
    let mut memory_operand = None;
    if let Some(access) = access {
        let width = match form.operation {
            SegmentOperation::Move => 2,
            _ => bytes + 2,
        };
        let (operand, address) = place_memory_operand(access, width, &mut processor, &mut memory_mutations);
        let image: Vec<u8> = match form.operation {
            SegmentOperation::Move => selector.to_le_bytes().to_vec(),
            SegmentOperation::LoadFar => OFFSET.to_le_bytes()[..bytes as usize].iter().copied().chain(selector.to_le_bytes()).collect(),
            _ => {
                let (offset, far_selector) = pointer.expect("a far pointer");
                offset.to_le_bytes()[..bytes as usize].iter().copied().chain(far_selector.to_le_bytes()).collect()
            }
        };
        write_at(address, &image, &mut memory_mutations);
        memory_operand = Some(operand);
    }
    if form.operation == SegmentOperation::Move && access.is_none() {
        processor.general_purpose_registers[registers[0]] = 0xA5A5_A5A5_A5A5_0000 | u64::from(selector);
    }
    let general = |number: usize| match bytes {
        2 => GENERAL_16[number],
        4 => GENERAL_32[number],
        _ => GENERAL_64[number],
    };
    let encoded = match (form.operation, memory_operand) {
        (SegmentOperation::Move, Some(operand)) => Encoded::with2(code, SEGMENTS[segment], operand),
        (SegmentOperation::Move, None) => {
            let source = if code == Code::Mov_Sreg_r64m16 { GENERAL_64[registers[0]] } else { GENERAL_32[registers[0]] };
            Encoded::with2(code, SEGMENTS[segment], source)
        }
        (SegmentOperation::LoadFar, Some(operand)) => Encoded::with2(code, general(registers[0]), operand),
        (_, Some(operand)) => Encoded::with1(code, operand),
        (SegmentOperation::Pop, None) => Encoded::with1(code, if form.instruction == Instruction::PopFs { Register::FS } else { Register::GS }),
        (SegmentOperation::ReturnFar, None) if form.codes[0] == Code::Retfw_imm16 => Encoded::with1(code, u32::from(immediate)),
        _ => Ok(Encoded::with(code)),
    }
    .unwrap_or_else(|error| panic!("{code:?} takes its operands: {error}"));
    let (code_page, start, code_bytes) = lay_out(layout, &encode_at_code_address(&encoded), to_padding(memory_operand));
    processor.instruction_pointer = start;
    let executable = PagePermission::Mapped { data_access: DataAccess::ReadOnly, instruction_fetch: InstructionFetch::Allowed };
    let mut all = vec![
        MemoryDelta::PermissionAssignment { addresses: code_page..code_page + PAGE_BYTES, permission: executable },
        MemoryDelta::ValueAssignment { start, bytes: Cow::Owned(code_bytes) },
    ];
    all.append(&mut memory_mutations);
    ExecutionState { processor, memory_mutations: all }
}

/// Whether the values decide the outcome whatever selector, far pointer or frame the form reads: its code faults
/// while it is fetched, or its stack access faults.
pub fn decides_without_data(instruction: Instruction, operands: &[Option<OperandValue>]) -> bool {
    let Some(form) = find_segment_form(instruction) else {
        return false;
    };
    if !matches!(form.operation, SegmentOperation::Pop | SegmentOperation::ReturnFar | SegmentOperation::InterruptReturn) {
        return false;
    }
    let (mut layout, mut bytes) = (None, 8);
    for operand in operands.iter().flatten() {
        match *operand {
            OperandValue::SystemLayout(chosen) => layout = Some(chosen),
            OperandValue::OperandSize(size) => bytes = size.to_bytes(),
            _ => {}
        }
    }
    let bytes = match instruction {
        Instruction::Iret => 2,
        Instruction::Iretd => 4,
        _ => bytes,
    };
    let slots = match form.operation {
        SegmentOperation::Pop => 1,
        SegmentOperation::ReturnFar => 2,
        _ => 5,
    };
    layout.is_some_and(|layout: crate::system::SystemLayout| {
        layout.code.faults_before_execution() || refuses(layout.placement, slots * bytes, false, false, false)
    })
}

/// MOV's segment register and selector together: every selector into ES, DS, FS and GS, and into SS every one but
/// 0x2B, the only one SS takes at ring 3, after which MOV holds off the trap until after the next instruction, so that
/// the chip source cannot step it.
pub fn generate_segment_loads() -> Vec<OperandValue> {
    let selectors: Vec<u16> = crate::descriptor::generate_selectors()
        .into_iter()
        .filter_map(|value| match value {
            OperandValue::Bits(selector) => Some(selector as u16),
            _ => None,
        })
        .collect();
    (0..SEGMENTS.len())
        .flat_map(|segment| {
            selectors
                .iter()
                .filter(move |selector| !(SEGMENTS[segment] == Register::SS && **selector == 0x2B))
                .map(move |&selector| OperandValue::SegmentLoad { segment, selector })
                .collect::<Vec<_>>()
        })
        .collect()
}
