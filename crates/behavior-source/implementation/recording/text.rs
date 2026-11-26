//! The text form of a recording of an ARM64 chip, one line for each state, so that a change shows as a line in a diff.
//!
//! ```text
//! instruction StrImmediateUnsignedOffset64
//! processor x0=0xffffffffffffffff x1=0xfffffffffffffffe ... exception=none
//! bytes #0 fffefdfc...
//! before x1=0x2000 | permission 0x1000..0x2000 read-only+execute; value 0x1000 f9000020; value 0x2000 #0
//! after pc=0x1004 | value 0x2000 ffffffffffffffff
//! ```
//!
//! `processor` holds every field of the first state before. A `before` line lists the fields in which its
//! processor differs from that one, then its memory changes; the `after` line below it lists the fields in
//! which the chip's processor differs from the before, then the memory changes the chip made. A run of bytes
//! longer than [`INLINE_BYTES`] is written once on a `bytes` line and named by its number.

use std::borrow::Cow;
use std::convert::Infallible;

use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, ConditionFlags, DataAccess, Exception, FaultStatus, Instruction,
    InstructionFetch, PagePermission, Processor, SmeTrapCode, SystemAccess, User, WaitInstruction,
    ZERO_PROCESSOR,
};

use crate::RecordedCase;

/// The longest run of bytes written where it is assigned; a store pair's 16 bytes fit.
const INLINE_BYTES: usize = 16;
const NO_EXCEPTION: &str = "none";
const INSTRUCTION_FETCH: &str = "+execute";

/// Why a text is no recording.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MalformedRecording {
    /// The line, counted from one, cannot be read.
    UnreadableLine(usize),
    NoInstruction,
    /// The last state before has no state after.
    UnfinishedCase,
}

/// Returns the name of the file that holds the instruction's recording.
pub(crate) fn to_file_name(instruction: Instruction<Infallible>) -> String {
    format!("{instruction:?}.txt")
}

/// Returns the text of an instruction's recording.
pub(crate) fn to_text(instruction: Instruction<Infallible>, cases: &[RecordedCase]) -> String {
    let mut lines = vec![format!("instruction {instruction:?}")];
    if let Some((first, _)) = cases.first() {
        let mut long_runs: Vec<&[u8]> = Vec::new();
        for (_, after) in cases {
            for mutation in &after.memory_mutations {
                if let MemoryDelta::ValueAssignment { bytes, .. } = mutation
                    && bytes.len() > INLINE_BYTES
                    && !long_runs.contains(&&bytes[..])
                {
                    long_runs.push(&bytes[..]);
                }
            }
        }
        let base = &first.processor;
        lines.push(format!("processor {}", to_differing_fields(None, base)));
        for (number, bytes) in long_runs.iter().enumerate() {
            lines.push(format!("bytes #{number} {}", to_hex(bytes)));
        }
        for (before, after) in cases {
            let made = &after.memory_mutations[before.memory_mutations.len()..];
            for (keyword, earlier, state, mutations) in [
                ("before", base, before, &before.memory_mutations[..]),
                ("after", &before.processor, after, made),
            ] {
                let mutations: Vec<String> = mutations
                    .iter()
                    .map(|mutation| to_mutation_text(mutation, &long_runs))
                    .collect();
                let fields = to_differing_fields(Some(earlier), &state.processor);
                let mutations = mutations.join("; ");
                let parts = [keyword, &fields, "|", &mutations];
                let written: Vec<&str> =
                    parts.into_iter().filter(|part| !part.is_empty()).collect();
                lines.push(written.join(" "));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Returns the instruction a recording's text names and its cases.
pub(crate) fn from_text(
    text: &str,
) -> Result<(Instruction<Infallible>, Vec<RecordedCase>), MalformedRecording> {
    let mut instruction = None;
    let mut base = None;
    let mut long_runs = Vec::new();
    let mut before: Option<ExecutionState<'static, Arm64<User>>> = None;
    let mut cases = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let mut read = || -> Option<()> {
            let (keyword, rest) = line.split_once(' ')?;
            match keyword {
                "instruction" => {
                    instruction = Some(
                        Arm64::<User>::list_instructions()
                            .find(|instruction| format!("{instruction:?}") == rest)?,
                    );
                }
                "processor" => base = Some(from_fields(ZERO_PROCESSOR, rest)?),
                "bytes" => {
                    let (name, hex) = rest.split_once(' ')?;
                    (name == format!("#{}", long_runs.len())).then_some(())?;
                    long_runs.push(from_hex(hex)?);
                }
                "before" => {
                    let (fields, memory) = rest.split_once('|')?;
                    before = Some(ExecutionState {
                        processor: from_fields(base.clone()?, fields)?,
                        memory_mutations: from_mutations_text(memory, &long_runs)?,
                    });
                }
                "after" => {
                    let before = before.take()?;
                    let (fields, memory) = rest.split_once('|')?;
                    let mut after = ExecutionState {
                        processor: from_fields(before.processor.clone(), fields)?,
                        memory_mutations: before.memory_mutations.clone(),
                    };
                    after
                        .memory_mutations
                        .extend(from_mutations_text(memory, &long_runs)?);
                    cases.push((before, after));
                }
                _ => return None,
            }
            Some(())
        };
        read().ok_or(MalformedRecording::UnreadableLine(index + 1))?;
    }
    if before.is_some() {
        return Err(MalformedRecording::UnfinishedCase);
    }
    Ok((instruction.ok_or(MalformedRecording::NoInstruction)?, cases))
}

/// Writes `name=value` for each field of the processor whose value differs from the earlier one's.
fn to_differing_fields(earlier: Option<&Processor<()>>, processor: &Processor<()>) -> String {
    let fields = to_fields(processor);
    let earlier_fields = earlier.map(to_fields);
    // The first state writes the predicates and FFR only where they are set, so that a recording of an instruction
    // without SVE operands reads as it did before they were held.
    let zero_fields = to_fields(&ZERO_PROCESSOR);
    let differing = fields.iter().enumerate().filter(|(index, field)| {
        earlier_fields.as_ref().map_or_else(
            || !is_scalable_field(&field.0) || zero_fields[*index] != **field,
            |earlier| earlier[*index] != **field,
        )
    });
    let written: Vec<String> = differing
        .map(|(_, (name, value))| format!("{name}={value}"))
        .collect();
    written.join(" ")
}

/// Leaves out the exclusive monitor, which is open in every recorded state: a chip source refuses a state that holds
/// it, and a single step opens it.
fn to_fields(processor: &Processor<()>) -> Vec<(String, String)> {
    let Processor {
        general_purpose_registers,
        stack_pointer,
        program_counter,
        condition_flags,
        vector_registers,
        predicate_registers,
        first_fault_register: first_fault,
        floating_point_control,
        floating_point_status,
        software_thread_id,
        exclusive_monitor: _,
        exception,
        level_specific: (),
    } = processor;
    let general = general_purpose_registers
        .iter()
        .enumerate()
        .map(|(number, value)| (format!("x{number}"), format!("{value:#x}")));
    let vector = vector_registers
        .iter()
        .enumerate()
        .map(|(number, value)| (format!("v{number}"), format!("{value:#x}")));
    let named = [
        ("sp", format!("{stack_pointer:#x}")),
        ("pc", format!("{program_counter:#x}")),
        ("nzcv", format!("{:#x}", condition_flags.bits())),
        ("fpcr", format!("{floating_point_control:#x}")),
        ("fpsr", format!("{floating_point_status:#x}")),
        ("tpidr_el0", format!("{software_thread_id:#x}")),
        ("exception", to_exception_text(*exception)),
    ];
    let predicate = predicate_registers
        .iter()
        .enumerate()
        .map(|(number, value)| (format!("p{number}"), format!("{value:#x}")));
    general
        .chain(named.map(|(name, value)| (name.to_owned(), value)))
        .chain(vector)
        .chain(predicate)
        .chain([(FIRST_FAULT_FIELD.to_owned(), format!("{first_fault:#x}"))])
        .collect()
}

/// The name of FFR's field.
const FIRST_FAULT_FIELD: &str = "ffr";

/// Whether the field is a predicate's or FFR's.
fn is_scalable_field(name: &str) -> bool {
    name == FIRST_FAULT_FIELD
        || name
            .strip_prefix('p')
            .is_some_and(|number| number.parse::<usize>().is_ok())
}

fn from_fields(mut processor: Processor<()>, fields: &str) -> Option<Processor<()>> {
    for field in fields.split_whitespace() {
        let (name, value) = field.split_once('=')?;
        match name {
            "sp" => processor.stack_pointer = from_hex_number(value)?,
            "pc" => processor.program_counter = from_hex_number(value)?,
            "nzcv" => {
                processor.condition_flags = ConditionFlags::from_bits(from_hex_number(value)?)?
            }
            "fpcr" => processor.floating_point_control = from_hex_number(value)?,
            "fpsr" => processor.floating_point_status = from_hex_number(value)?,
            "tpidr_el0" => processor.software_thread_id = from_hex_number(value)?,
            "exception" => processor.exception = from_exception_text(value)?,
            FIRST_FAULT_FIELD => processor.first_fault_register = from_hex_number(value)?,
            _ => match (name.strip_prefix('x'), name.strip_prefix('v')) {
                (Some(number), None) => {
                    *processor
                        .general_purpose_registers
                        .get_mut(number.parse::<usize>().ok()?)? = from_hex_number(value)?
                }
                (None, Some(number)) => {
                    *processor
                        .vector_registers
                        .get_mut(number.parse::<usize>().ok()?)? = from_hex_number(value)?
                }
                (None, None) if let Some(number) = name.strip_prefix('p') => {
                    *processor
                        .predicate_registers
                        .get_mut(number.parse::<usize>().ok()?)? = from_hex_number(value)?
                }
                (None, None) | (Some(_), Some(_)) => return None,
            },
        }
    }
    Some(processor)
}

/// Writes `none`, or the exception's name, then each field after a colon, and its fault address after `@`.
fn to_exception_text(exception: Option<Exception>) -> String {
    let Some(exception) = exception else {
        return NO_EXCEPTION.to_owned();
    };
    match exception {
        Exception::UndefinedInstruction => "UndefinedInstruction".to_owned(),
        Exception::ProgramCounterAlignmentFault { address } => {
            format!("ProgramCounterAlignmentFault@{address:#x}")
        }
        Exception::StackPointerAlignmentFault => "StackPointerAlignmentFault".to_owned(),
        Exception::InstructionAbort { status, address } => {
            format!("InstructionAbort:{}@{address:#x}", to_status_text(status))
        }
        Exception::DataAbort {
            status,
            access,
            address,
        } => format!(
            "DataAbort:{}:{}@{address:#x}",
            to_status_text(status),
            to_access_text(access)
        ),
        Exception::Breakpoint { comment } => format!("Breakpoint:{comment:#x}"),
        Exception::SmeTrap(code) => format!("SmeTrap:{}", to_sme_trap_text(code)),
        Exception::SupervisorCall { immediate } => format!("SupervisorCall:{immediate:#x}"),
        Exception::WaitTrapped(instruction) => format!(
            "WaitTrapped:{}",
            match instruction {
                WaitInstruction::WaitForInterrupt => "WaitForInterrupt",
                WaitInstruction::WaitForEvent => "WaitForEvent",
            }
        ),
        Exception::SystemAccessTrapped(access) => format!(
            "SystemAccessTrapped:{:#x}:{:#x}:{:#x}:{:#x}:{:#x}:{:#x}:{}",
            access.op0,
            access.op1,
            access.crn,
            access.crm,
            access.op2,
            access.rt,
            to_access_text(access.direction)
        ),
        Exception::Undecoded {
            syndrome,
            fault_address,
        } => format!("Undecoded:{syndrome:#x}@{fault_address:#x}"),
    }
}

fn from_exception_text(text: &str) -> Option<Option<Exception>> {
    if text == NO_EXCEPTION {
        return Some(None);
    }
    let (fields, address) = match text.split_once('@') {
        Some((fields, address)) => (fields, Some(from_hex_number(address)?)),
        None => (text, None),
    };
    let fields: Vec<&str> = fields.split(':').collect();
    let exception = match (&fields[..], address) {
        (["UndefinedInstruction"], None) => Exception::UndefinedInstruction,
        (["ProgramCounterAlignmentFault"], Some(address)) => {
            Exception::ProgramCounterAlignmentFault { address }
        }
        (["StackPointerAlignmentFault"], None) => Exception::StackPointerAlignmentFault,
        (["InstructionAbort", status], Some(address)) => Exception::InstructionAbort {
            status: from_status_text(status)?,
            address,
        },
        (["DataAbort", status, access], Some(address)) => Exception::DataAbort {
            status: from_status_text(status)?,
            access: from_access_text(access)?,
            address,
        },
        (["Breakpoint", comment], None) => Exception::Breakpoint {
            comment: from_hex_number(comment)?,
        },
        (["SmeTrap", code], None) => Exception::SmeTrap(from_sme_trap_text(code)?),
        (["SupervisorCall", immediate], None) => Exception::SupervisorCall {
            immediate: from_hex_number(immediate)?,
        },
        (["WaitTrapped", "WaitForInterrupt"], None) => {
            Exception::WaitTrapped(WaitInstruction::WaitForInterrupt)
        }
        (["WaitTrapped", "WaitForEvent"], None) => {
            Exception::WaitTrapped(WaitInstruction::WaitForEvent)
        }
        (
            [
                "SystemAccessTrapped",
                op0,
                op1,
                crn,
                crm,
                op2,
                rt,
                direction,
            ],
            None,
        ) => Exception::SystemAccessTrapped(SystemAccess {
            op0: from_hex_number(op0)?,
            op1: from_hex_number(op1)?,
            crn: from_hex_number(crn)?,
            crm: from_hex_number(crm)?,
            op2: from_hex_number(op2)?,
            rt: from_hex_number(rt)?,
            direction: from_access_text(direction)?,
        }),
        (["Undecoded", syndrome], Some(fault_address)) => Exception::Undecoded {
            syndrome: from_hex_number(syndrome)?,
            fault_address,
        },
        _ => return None,
    };
    Some(Some(exception))
}

fn to_status_text(status: FaultStatus) -> &'static str {
    match status {
        FaultStatus::TranslationFaultLevel0 => "TranslationFaultLevel0",
        FaultStatus::TranslationFaultLevel3 => "TranslationFaultLevel3",
        FaultStatus::PermissionFaultLevel3 => "PermissionFaultLevel3",
        FaultStatus::AlignmentFault => "AlignmentFault",
    }
}

fn from_status_text(text: &str) -> Option<FaultStatus> {
    match text {
        "TranslationFaultLevel0" => Some(FaultStatus::TranslationFaultLevel0),
        "TranslationFaultLevel3" => Some(FaultStatus::TranslationFaultLevel3),
        "PermissionFaultLevel3" => Some(FaultStatus::PermissionFaultLevel3),
        "AlignmentFault" => Some(FaultStatus::AlignmentFault),
        _ => None,
    }
}

fn to_access_text(access: AccessDirection) -> &'static str {
    match access {
        AccessDirection::Read => "Read",
        AccessDirection::Write => "Write",
    }
}

fn from_access_text(text: &str) -> Option<AccessDirection> {
    match text {
        "Read" => Some(AccessDirection::Read),
        "Write" => Some(AccessDirection::Write),
        _ => None,
    }
}

fn to_sme_trap_text(code: SmeTrapCode) -> &'static str {
    match code {
        SmeTrapCode::Disabled => "Disabled",
        SmeTrapCode::NotInStreamingMode => "NotInStreamingMode",
    }
}

fn from_sme_trap_text(text: &str) -> Option<SmeTrapCode> {
    match text {
        "Disabled" => Some(SmeTrapCode::Disabled),
        "NotInStreamingMode" => Some(SmeTrapCode::NotInStreamingMode),
        _ => None,
    }
}

fn to_mutation_text(mutation: &MemoryDelta<'_, Arm64<User>>, long_runs: &[&[u8]]) -> String {
    match mutation {
        MemoryDelta::PermissionAssignment {
            addresses,
            permission,
        } => format!(
            "permission {:#x}..{:#x} {}",
            addresses.start,
            addresses.end,
            to_permission_text(*permission)
        ),
        MemoryDelta::ValueAssignment { start, bytes } => {
            match long_runs.iter().position(|run| *run == &bytes[..]) {
                Some(number) => format!("value {start:#x} #{number}"),
                None => format!("value {start:#x} {}", to_hex(bytes)),
            }
        }
    }
}

fn from_mutations_text(
    text: &str,
    long_runs: &[Vec<u8>],
) -> Option<Vec<MemoryDelta<'static, Arm64<User>>>> {
    let entries = text
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty());
    entries
        .map(|entry| {
            let words: Vec<&str> = entry.split(' ').collect();
            match words[..] {
                ["permission", addresses, permission] => {
                    let (start, end) = addresses.split_once("..")?;
                    Some(MemoryDelta::PermissionAssignment {
                        addresses: from_hex_number(start)?..from_hex_number(end)?,
                        permission: from_permission_text(permission)?,
                    })
                }
                ["value", start, bytes] => {
                    let bytes = match bytes.strip_prefix('#') {
                        Some(number) => long_runs.get(number.parse::<usize>().ok()?)?.clone(),
                        None => from_hex(bytes)?,
                    };
                    Some(MemoryDelta::ValueAssignment {
                        start: from_hex_number(start)?,
                        bytes: Cow::Owned(bytes),
                    })
                }
                _ => None,
            }
        })
        .collect()
}

/// Writes `unmapped`, or the data access with `+execute` after it where instructions may be fetched.
fn to_permission_text(permission: PagePermission) -> String {
    match permission {
        PagePermission::Unmapped => "unmapped".to_owned(),
        PagePermission::Mapped {
            data_access,
            instruction_fetch,
        } => {
            let data_access = match data_access {
                DataAccess::Forbidden => "forbidden",
                DataAccess::ReadOnly => "read-only",
                DataAccess::ReadWrite => "read-write",
            };
            match instruction_fetch {
                InstructionFetch::Forbidden => data_access.to_owned(),
                InstructionFetch::Allowed => format!("{data_access}{INSTRUCTION_FETCH}"),
            }
        }
    }
}

fn from_permission_text(text: &str) -> Option<PagePermission> {
    if text == "unmapped" {
        return Some(PagePermission::Unmapped);
    }
    let (data_access, instruction_fetch) = match text.strip_suffix(INSTRUCTION_FETCH) {
        Some(data_access) => (data_access, InstructionFetch::Allowed),
        None => (text, InstructionFetch::Forbidden),
    };
    let data_access = match data_access {
        "forbidden" => DataAccess::Forbidden,
        "read-only" => DataAccess::ReadOnly,
        "read-write" => DataAccess::ReadWrite,
        _ => return None,
    };
    Some(PagePermission::Mapped {
        data_access,
        instruction_fetch,
    })
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    let digits = text.as_bytes();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

/// Reads a number written `0x` and hexadecimal digits.
fn from_hex_number<Number: TryFrom<u128>>(text: &str) -> Option<Number> {
    let digits = text.strip_prefix("0x")?;
    Number::try_from(u128::from_str_radix(digits, 16).ok()?).ok()
}
