//! The suite's guests on ARM64, stopped as the ARM64 architecture crate defines a stop.

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_abi::{
    Access, SYSTEM_CALL_ARGUMENT_COUNT, SYSTEM_CALL_NUMBER_REGISTER, SystemCallRequest, WORD,
};
use wcontainers_arm64::encode::{self, Register, form};
use wcontainers_processor_architecture_arm64::syndrome::{
    FaultStatus, UNDEFINED_INSTRUCTION, WRITE_NOT_READ, encode_syndrome,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, Exception, INSTRUCTION_BYTES, Processor, ZERO_PROCESSOR,
};

use crate::{FirstTrap, Guest, GuestArchitecture};

/// The first argument register of a system call and its result register, x0.
///
/// <https://man7.org/linux/man-pages/man2/syscall.2.html>
const FIRST_ARGUMENT_REGISTER: Register = 0;
const RESULT_REGISTER: Register = FIRST_ARGUMENT_REGISTER;
/// The register of the address a guest stores to or loads from, and of the count a guest counts down.
const ADDRESS_REGISTER: Register = FIRST_ARGUMENT_REGISTER;
const COUNTER_REGISTER: Register = FIRST_ARGUMENT_REGISTER;
/// The register of the value a guest stores or loads, the one after the address's.
const VALUE_REGISTER: Register = ADDRESS_REGISTER + 1;
/// The immediate of every system call, breakpoint and undefined instruction of the guests.
const IMMEDIATE: u16 = u16::MIN;
/// What the counting guest adds to its count each time round, which the encoder writes as a subtraction.
///
/// <https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SUB--immediate---Subtract-immediate-value->
const COUNT_STEP: i64 = -1;

impl GuestArchitecture for Arm64 {
    fn build_system_call(code: u64, request: SystemCallRequest, returned: u64) -> Guest<Self> {
        let mut start = start_at(code);
        let first = usize::from(FIRST_ARGUMENT_REGISTER);
        start.general_purpose_registers[first..first + SYSTEM_CALL_ARGUMENT_COUNT]
            .copy_from_slice(&request.arguments);
        start.general_purpose_registers[SYSTEM_CALL_NUMBER_REGISTER] = request.number.0;
        let mut answered = start.clone();
        answered.general_purpose_registers[usize::from(RESULT_REGISTER)] = returned;
        let call = encode::exception(form::SVC, IMMEDIATE);
        end_at_breakpoint(&[call], start, answered)
    }

    fn build_store(code: u64, address: u64, value: u64) -> Guest<Self> {
        let mut start = start_at(code);
        start.general_purpose_registers[usize::from(ADDRESS_REGISTER)] = address;
        start.general_purpose_registers[usize::from(VALUE_REGISTER)] = value;
        let store = encode::load_store(form::STR, VALUE_REGISTER, ADDRESS_REGISTER, u64::MIN, WORD)
            .expect("a store with no offset encodes");
        let stored = start.clone();
        end_at_breakpoint(&[store], start, stored)
    }

    fn build_load(code: u64, address: u64, value: u64) -> Guest<Self> {
        let mut start = start_at(code);
        start.general_purpose_registers[usize::from(ADDRESS_REGISTER)] = address;
        let mut loaded = start.clone();
        loaded.general_purpose_registers[usize::from(VALUE_REGISTER)] = value;
        let load = encode::load_store(form::LDR, VALUE_REGISTER, ADDRESS_REGISTER, u64::MIN, WORD)
            .expect("a load with no offset encodes");
        end_at_breakpoint(&[load], start, loaded)
    }

    fn build_undefined_instruction(code: u64) -> Guest<Self> {
        let start = start_at(code);
        Guest {
            code: to_code(&[encode::exception(form::UDF, IMMEDIATE)]),
            finished: stopped_by(start.clone(), UNDEFINED_INSTRUCTION),
            start,
        }
    }

    fn build_count_down(code: u64, count: u64) -> Guest<Self> {
        let mut start = start_at(code);
        start.general_purpose_registers[usize::from(COUNTER_REGISTER)] = count;
        let back = -i64::try_from(INSTRUCTION_BYTES).expect("an instruction's size fits");
        let step = encode::add_immediate(COUNTER_REGISTER, COUNTER_REGISTER, COUNT_STEP)
            .expect("a step of one encodes");
        let loop_back = encode::compare_branch(true, COUNTER_REGISTER, back)
            .expect("a branch to the instruction before encodes");
        let mut counted = start.clone();
        counted.general_purpose_registers[usize::from(COUNTER_REGISTER)] = u64::MIN;
        end_at_breakpoint(&[step, loop_back], start, counted)
    }

    fn list_stops_at_first_instruction(guest: &Guest<Self>, trap: FirstTrap) -> Vec<Processor> {
        let exceptions = match trap {
            FirstTrap::SystemCall => vec![Exception {
                syndrome: encode_syndrome(ESR_EL1::EC::Value::SVC64, u64::from(IMMEDIATE)),
                fault_address: None,
            }],
            FirstTrap::MemoryFault { address, access } => {
                let (class, write_not_read) = match access {
                    Access::Read => (ESR_EL1::EC::Value::DataAbortLowerEL, u64::MIN),
                    Access::Write => (ESR_EL1::EC::Value::DataAbortLowerEL, WRITE_NOT_READ),
                    Access::Execute => (ESR_EL1::EC::Value::InstrAbortLowerEL, u64::MIN),
                };
                // A page that allows nothing may be held unmapped or mapped without the access: the interface does
                // not say which.
                [
                    FaultStatus::TranslationFaultLevel3,
                    FaultStatus::PermissionFaultLevel3,
                ]
                .map(|status| Exception {
                    syndrome: encode_syndrome(class, write_not_read | status as u64),
                    fault_address: Some(address),
                })
                .to_vec()
            }
        };
        exceptions
            .into_iter()
            .map(|exception| stopped_by(guest.start.clone(), exception))
            .collect()
    }
}

/// Returns the processor with every register zero but the program counter, at the code.
fn start_at(code: u64) -> Processor {
    Processor {
        program_counter: code,
        ..ZERO_PROCESSOR
    }
}

/// Returns the guest of the instructions and a breakpoint after them, finished at it with the registers given.
fn end_at_breakpoint(instructions: &[u32], start: Processor, mut ended: Processor) -> Guest<Arm64> {
    let breakpoint = encode::exception(form::BRK, IMMEDIATE);
    let code = to_code(&[instructions, &[breakpoint]].concat());
    let code_bytes = u64::try_from(code.len()).expect("a guest's length fits in an address");
    ended.program_counter = start.program_counter + code_bytes - INSTRUCTION_BYTES;
    let syndrome = encode_syndrome(ESR_EL1::EC::Value::Brk64, u64::from(IMMEDIATE));
    let finished = stopped_by(
        ended,
        Exception {
            syndrome,
            fault_address: None,
        },
    );
    Guest {
        code,
        start,
        finished,
    }
}

fn to_code(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn stopped_by(mut processor: Processor, exception: Exception) -> Processor {
    processor.exception = Some(exception);
    processor
}
