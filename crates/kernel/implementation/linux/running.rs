//! One turn of a guest thread on the executor's [`Execution::run`]: the thread's registers as
//! the processor's state and back, and the trap that ended the turn as the [`Exit`] it is, which
//! the kernel answers outside the execution.

use core::any::Any;
use core::future::Future;
use core::pin::pin;
use core::task::{Context, Waker};

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_abi::{Access, RegisterBlock, Resume, SystemCallRequest};
use wcontainers_executor::{
    Execution, ExecutionStop, Executor, InterruptResponse, MemoryFaultResponse, ProcessorStateOf,
    SystemCallResponse, TrapHandler,
};
use wcontainers_processor_architecture_arm64::{ConditionFlags, Exception, ExclusiveMonitor};

use crate::guest_machine::Predicates;

/// The register a system call's value is returned in on arm64, x0
/// (<https://man7.org/linux/man-pages/man2/syscall.2.html#:~:text=arm64%20%20%20%20%20%20%20%20%20%20svc%20%230>).
const RESULT_REGISTER: usize = 0;

/// Why a thread's turn ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Exit {
    /// A system call, with the program counter after its instruction.
    Syscall(SystemCallRequest),
    /// An interrupt, which changed nothing of the thread.
    Stop,
    /// An access the page does not allow, with the program counter at its instruction.
    MemoryFault { address: u64, access: Access },
    /// A misaligned stack pointer or jump, for which Linux sends SIGBUS, BUS_ADRALN.
    AlignmentFault { address: u64 },
    /// An instruction the processor presented does not have.
    IllegalInstruction,
    /// A breakpoint instruction (`brk`), with its 16-bit comment.
    Breakpoint { comment: u16 },
}

/// The processor's state of a thread at EL0, which adds no state to what every level has.
type Processor = wcontainers_processor_architecture_arm64::Processor<()>;

/// Ends the execution at its first trap and keeps that trap as the exit it is.
#[derive(Default)]
struct FirstTrap {
    exit: Option<Exit>,
}

impl TrapHandler for FirstTrap {
    fn handle_system_call(&mut self, request: SystemCallRequest) -> SystemCallResponse {
        self.exit = Some(Exit::Syscall(request));
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, address: u64, access: Access) -> MemoryFaultResponse {
        self.exit = Some(Exit::MemoryFault { address, access });
        MemoryFaultResponse::Terminate
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.exit = Some(Exit::Stop);
        InterruptResponse::Terminate
    }
}

/// Runs the thread whose registers are `registers` and `predicates` on `memory` with `executor`,
/// carrying on as `resume` says, until its first trap, and returns that trap as the exit it is,
/// with `registers` and `predicates` as the thread left them. `started` is given each
/// execution's interrupter before it runs.
///
/// # Panics
///
/// If the executor's processor is not arm64's, the only one whose registers the kernel keeps.
pub(crate) fn turn<C: Executor + 'static>(
    executor: &mut C,
    registers: &mut RegisterBlock,
    predicates: &mut Predicates,
    memory: &C::GuestMemory,
    resume: Resume,
    mut started: impl FnMut(C::Interrupter),
) -> Exit {
    if let Resume::Return(value) = resume {
        registers.x[RESULT_REGISTER] = value;
    }
    let mut trap = FirstTrap::default();
    let mut processor = executors_own::<ProcessorStateOf<C>>(to_processor(registers, *predicates))
        .expect("the kernel runs arm64 threads only");
    // Runs until the next trap, which the handler ends the execution at; an execution that must
    // wait is waited out and prepared again from where it stopped.
    let processor = loop {
        let (execution, interrupter) = executor.prepare_execution(processor, memory);
        started(interrupter);
        match execution.run(&mut trap) {
            ExecutionStop::Terminated(stopped) => break stopped,
            ExecutionStop::Waiting {
                processor: stopped,
                wait,
            } => {
                waited(wait);
                processor = stopped;
            }
        }
    };
    let processor: &Processor = (&processor as &dyn Any)
        .downcast_ref()
        .expect("the executor's processor is the one it was given");
    *registers = to_registers(processor);
    // A supervisor call is taken after its instruction (`Exception::is_taken_after_instruction`),
    // so the processor's state at a system call already holds the program counter after it.
    let exit = match trap.exit {
        Some(exit) => exit,
        None => exit_of(processor.exception, registers),
    };
    *predicates = match exit {
        Exit::Syscall(_) => Predicates::default(),
        _ => Predicates {
            registers: processor.predicate_registers,
            first_fault: processor.first_fault_register,
        },
    };
    exit
}

/// `processor` as the executor's own processor's state `P`, if that is arm64's.
fn executors_own<P: 'static>(processor: Processor) -> Option<P> {
    let mut processor = Some(processor);
    (&mut processor as &mut dyn Any)
        .downcast_mut::<Option<P>>()
        .and_then(Option::take)
}

/// Polls an executor's `wait` until it completes, again at once each time, as the kernel's runner
/// has nothing else to do.
fn waited<F: Future<Output = ()>>(wait: F) {
    let mut wait = pin!(wait);
    let mut context = Context::from_waker(Waker::noop());
    while wait.as_mut().poll(&mut context).is_pending() {
        core::hint::spin_loop();
    }
}

/// Returns the exit of an exception no hook was told of: the traps that always end an execution.
///
/// # Panics
///
/// If there is no exception, or it is one the executor interface hands to a hook first.
fn exit_of(exception: Option<Exception>, registers: &RegisterBlock) -> Exit {
    match exception.expect("an execution that no hook ended stopped on an exception") {
        Exception::UndefinedInstruction => Exit::IllegalInstruction,
        Exception::Breakpoint { comment } => Exit::Breakpoint { comment },
        Exception::ProgramCounterAlignmentFault { .. } => Exit::AlignmentFault {
            address: registers.pc,
        },
        Exception::StackPointerAlignmentFault => Exit::AlignmentFault {
            address: registers.sp,
        },
        // A page's fault goes to the handler first, so a data abort it was not told of is an
        // access the processor requires to be aligned.
        Exception::DataAbort { address, .. } => Exit::AlignmentFault { address },
        Exception::Undecoded {
            syndrome,
            fault_address,
        } => exit_of_syndrome(syndrome, fault_address, registers),
        // A supervisor call goes to the hook first, and Linux lets EL0 wait untrapped. A trapped
        // system access names no exit yet, as when the processor kept it as its syndrome.
        exception @ (Exception::InstructionAbort { .. }
        | Exception::SmeTrap(_)
        | Exception::SupervisorCall { .. }
        | Exception::WaitTrapped(_)
        | Exception::SystemAccessTrapped(_)) => {
            panic!("an execution ended on an exception no exit names: {exception:?}")
        }
    }
}

/// Returns the exit of an exception the processor's state keeps as its syndrome and fault address.
///
/// # Panics
///
/// If the syndrome's class is one no exit names.
fn exit_of_syndrome(syndrome: u64, fault_address: u64, registers: &RegisterBlock) -> Exit {
    use ESR_EL1::EC::Value as Class;
    match ESR_EL1::EC.read_as_enum(syndrome) {
        Some(Class::Unknown) => Exit::IllegalInstruction,
        Some(Class::Brk64) => Exit::Breakpoint {
            comment: u16::try_from(ESR_EL1::ISS.read(syndrome) & u64::from(u16::MAX))
                .expect("a breakpoint's comment is its syndrome's low 16 bits"),
        },
        Some(Class::PCAlignmentFault) => Exit::AlignmentFault {
            address: registers.pc,
        },
        Some(Class::SPAlignmentFault) => Exit::AlignmentFault {
            address: registers.sp,
        },
        // As in `exit_of`: a data abort no handler was told of is a misaligned access.
        Some(Class::DataAbortLowerEL) => Exit::AlignmentFault {
            address: fault_address,
        },
        _ => panic!("an execution ended on an exception no exit names: syndrome {syndrome:#x}"),
    }
}

/// The processor's state of a thread whose registers are `registers` and `predicates`, with no
/// exception.
///
/// Its exclusive monitor is open: the registers keep no monitor, so a turn begins as a return to
/// EL0 that cleared it.
fn to_processor(registers: &RegisterBlock, predicates: Predicates) -> Processor {
    Processor {
        general_purpose_registers: registers.x,
        stack_pointer: registers.sp,
        program_counter: registers.pc,
        // The flags lie in PSTATE's low word.
        condition_flags: ConditionFlags::from_bits_truncate(
            u32::try_from(registers.pstate & u64::from(u32::MAX)).expect("masked to a word"),
        ),
        vector_registers: registers
            .v
            .map(|[low, high]| u128::from(low) | (u128::from(high) << u64::BITS)),
        floating_point_control: u64::from(registers.fpcr),
        floating_point_status: u64::from(registers.fpsr),
        predicate_registers: predicates.registers,
        first_fault_register: predicates.first_fault,
        software_thread_id: registers.tpidr_el0,
        exclusive_monitor: ExclusiveMonitor::Open,
        exception: None,
        level_specific: (),
    }
}

/// The registers of a thread whose processor's state is `processor`.
fn to_registers(processor: &Processor) -> RegisterBlock {
    let halves = |value: u128| {
        let low = u64::try_from(value & u128::from(u64::MAX)).expect("the low half fits");
        let high = u64::try_from(value >> u64::BITS).expect("the high half fits");
        [low, high]
    };
    RegisterBlock {
        x: processor.general_purpose_registers,
        sp: processor.stack_pointer,
        pc: processor.program_counter,
        pstate: u64::from(processor.condition_flags.bits()),
        v: processor.vector_registers.map(halves),
        fpsr: u32::try_from(processor.floating_point_status)
            .expect("FPSR's upper half is reserved as zero"),
        fpcr: u32::try_from(processor.floating_point_control)
            .expect("FPCR's upper half is reserved as zero"),
        tpidr_el0: processor.software_thread_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wcontainers_processor_architecture_arm64::INSTRUCTION_SIZE;

    #[test]
    fn registers_come_back_from_the_processor_state_unchanged() {
        let mut registers = RegisterBlock::ZERO;
        for (index, register) in registers.x.iter_mut().enumerate() {
            *register = u64::MAX - u64::try_from(index).expect("an index fits");
        }
        registers.sp = u64::MAX;
        registers.pc = u64::from(u32::MAX);
        registers.pstate = u64::from(ConditionFlags::all().bits());
        registers.v[1] = [u64::MAX, u64::MIN];
        registers.v[2] = [u64::MIN, u64::MAX];
        registers.fpsr = u32::MAX;
        registers.fpcr = u32::MAX;
        registers.tpidr_el0 = u64::MAX;
        assert_eq!(
            to_registers(&to_processor(&registers, Predicates::default())),
            registers
        );
    }

    /// The [DFSC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D) of an alignment fault.
    const ALIGNMENT_FAULT: u64 = 0b10_0001;

    /// An exception the processor's state keeps whole, as ESR_EL1 and FAR_EL1 hold it.
    fn undecoded(
        class: ESR_EL1::EC::Value,
        specific: u64,
        fault_address: u64,
    ) -> Option<Exception> {
        Some(Exception::Undecoded {
            syndrome: ESR_EL1::EC.val(class as u64).value
                | ESR_EL1::IL::SET.value
                | ESR_EL1::ISS.val(specific).value,
            fault_address,
        })
    }

    #[test]
    fn traps_without_a_hook_are_the_exits_they_are() {
        use ESR_EL1::EC::Value as Class;
        let mut registers = RegisterBlock::ZERO;
        registers.pc = INSTRUCTION_SIZE;
        registers.sp = INSTRUCTION_SIZE * 2;
        let comment = u16::MAX;
        let misaligned = registers.sp + 1;
        let cases = [
            (
                Exception::Breakpoint { comment },
                Exit::Breakpoint { comment },
            ),
            (Exception::UndefinedInstruction, Exit::IllegalInstruction),
            (
                Exception::StackPointerAlignmentFault,
                Exit::AlignmentFault {
                    address: registers.sp,
                },
            ),
            (
                Exception::ProgramCounterAlignmentFault {
                    address: registers.pc,
                },
                Exit::AlignmentFault {
                    address: registers.pc,
                },
            ),
        ];
        for (exception, exit) in cases {
            assert_eq!(exit_of(Some(exception), &registers), exit, "{exception:?}");
        }
        let kept_whole = [
            (
                undecoded(Class::Brk64, u64::from(comment), 0),
                Exit::Breakpoint { comment },
            ),
            (undecoded(Class::Unknown, 0, 0), Exit::IllegalInstruction),
            (
                undecoded(Class::SPAlignmentFault, 0, 0),
                Exit::AlignmentFault {
                    address: registers.sp,
                },
            ),
            (
                undecoded(Class::PCAlignmentFault, 0, registers.pc),
                Exit::AlignmentFault {
                    address: registers.pc,
                },
            ),
            (
                undecoded(Class::DataAbortLowerEL, ALIGNMENT_FAULT, misaligned),
                Exit::AlignmentFault {
                    address: misaligned,
                },
            ),
        ];
        for (exception, exit) in kept_whole {
            assert_eq!(exit_of(exception, &registers), exit, "{exception:?}");
        }
    }
}
