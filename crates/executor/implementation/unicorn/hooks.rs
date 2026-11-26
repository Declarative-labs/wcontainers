//! What a test does at the kernel's hooks and task id calls.

use std::ops::{ControlFlow, Range};

use wcontainers_abi::{Cpu, SystemCallRequest};
use wcontainers_kernel::signal_information::{
    BadSystemCallCode, BusErrorCode, ChildStateCode, ChildTerminationCode, SegmentationFaultCode,
    TrapCode,
};
use wcontainers_kernel_linux::linux::general::{self, SI_DETHREAD, SI_KERNEL, SI_TIMER};
use wcontainers_kernel_linux::{
    AccessKind, Kernel, MemoryObject, Process, SignalInformation, SystemCallResponse, TaskId,
    task_id_after,
};

/// The embedder guest's `interrupted` sender that leaves the signal to the embedder (guest/embedder.c),
/// a word of its own that shares its spelling with the look-ahead none.
pub const EMBEDDER_SENDS: &str = "none";

/// A test's side of the kernel's hooks and task id calls, for the tests that act at some of them;
/// each left alone does nothing, or for task ids, counts as the kernel's default does.
pub trait Hooks: Send {
    fn allocate_task_id(&mut self, previous: TaskId) -> TaskId {
        task_id_after(previous)
    }
    fn free_task_id(&mut self, id: TaskId) {
        let _ = id;
    }
    fn intercept_system_call_request(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        let _ = (process, thread);
        ControlFlow::Continue(request)
    }
    fn intercept_system_call_response(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        let _ = (process, thread, request);
        response
    }
    fn on_process_starting(&mut self, process: Process<'_>) {
        let _ = process;
    }
    fn on_program_starting(&mut self, process: Process<'_>) {
        let _ = process;
    }
    fn on_thread_starting(&mut self, process: Process<'_>, thread: TaskId) {
        let _ = (process, thread);
    }
    fn on_process_exiting(&mut self, process: Process<'_>) {
        let _ = process;
    }
    fn on_process_outgrown(&mut self, process: Process<'_>, bytes: u64) {
        let _ = (process, bytes);
    }
    fn on_watched_memory_accessing(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        address: u64,
        kind: AccessKind,
    ) {
        let _ = (process, thread, address, kind);
    }
    fn on_memory_written_by_kernel(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        range: Range<u64>,
    ) {
        let _ = (process, thread, range);
    }
    fn on_shared_memory_mapping(
        &mut self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        let _ = (process, source, offset, range);
    }
    fn on_shared_memory_unmapped(
        &mut self,
        process: Process<'_>,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
        let _ = (process, source, offset, range);
    }
    fn on_signal_delivering(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        signal: &SignalInformation,
    ) {
        let _ = (process, thread, signal);
    }
    /// The kernel has nothing to run: the test may act on it through ordinary calls. Whether it
    /// did anything that may let the kernel go on.
    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let _ = (kernel, cpu);
        false
    }
}

/// No hooks of the test's: the kernel's provided defaults answer.
pub struct NoHooks;

/// What the driver asks of a host's hooks when the kernel has nothing to run.
pub trait BetweenSteps {
    /// Whether the hooks did anything that may let the kernel go on.
    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool;
}

impl BetweenSteps for NoHooks {
    fn between_steps(&mut self, _: &mut Kernel, _: &mut dyn Cpu) -> bool {
        false
    }
}

impl BetweenSteps for Box<dyn Hooks> {
    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        (**self).between_steps(kernel, cpu)
    }
}

/// A record's signal number, code and value (`si_value.sival_int`, where its layout has one), as
/// a handler reads them.
pub fn signal_parts(record: &SignalInformation) -> (i32, i32, i32) {
    use wcontainers_kernel::signal::Signal;
    use wcontainers_kernel::signal_information::SignalInformation as Record;
    let (signal, code, value) = match *record {
        Record::SentByProcess { signal, code, .. } => (signal, code as i32, 0),
        Record::QueuedByProcess {
            signal,
            code,
            value,
            ..
        } => (signal, code as i32, value.0 as i32),
        Record::SuppliedByProcess { signal, code, .. } => (signal, code.0, 0),
        Record::TimerExpired { signal, value, .. } => (signal, SI_TIMER, value.0 as i32),
        Record::SentByKernel { signal } => (signal, SI_KERNEL as i32, 0),
        Record::ThreadDestroyedByExecve => (Signal::SIGKILL, SI_DETHREAD, 0),
        Record::InputOutputEvent { signal, code, .. } => (signal, code as i32, 0),
        Record::IllegalInstruction { code, .. } => (Signal::SIGILL, code as i32, 0),
        Record::ArithmeticError { code, .. } => (Signal::SIGFPE, code as i32, 0),
        Record::SegmentationFault { code, .. } => {
            (Signal::SIGSEGV, segmentation_fault_code(code), 0)
        }
        Record::BusError { code, .. } => (Signal::SIGBUS, bus_error_code(code), 0),
        Record::Trap { code, .. } => (Signal::SIGTRAP, trap_code(code), 0),
        Record::ChildTerminated { signal, code, .. } => (signal, child_termination_code(code), 0),
        Record::ChildStateChanged { code, .. } => (Signal::SIGCHLD, child_state_code(code), 0),
        Record::BadSystemCall { code, .. } => (Signal::SIGSYS, bad_system_call_code(code), 0),
        Record::TracerStop { .. } => (Signal::SIGTRAP, 0, 0),
    };
    (signal as i32, code, value)
}

// Each code's number, by name: a code that carries fields cannot be cast to its number, and its
// bytes are not read as one.

fn segmentation_fault_code(code: SegmentationFaultCode) -> i32 {
    use SegmentationFaultCode as Code;
    (match code {
        Code::SEGV_MAPERR => general::SEGV_MAPERR,
        Code::SEGV_ACCERR => general::SEGV_ACCERR,
        Code::SEGV_BNDERR(_) => general::SEGV_BNDERR,
        Code::SEGV_PKUERR(_) => general::SEGV_PKUERR,
        Code::SEGV_ACCADI => general::SEGV_ACCADI,
        Code::SEGV_ADIDERR => general::SEGV_ADIDERR,
        Code::SEGV_ADIPERR => general::SEGV_ADIPERR,
        Code::SEGV_MTEAERR => general::SEGV_MTEAERR,
        Code::SEGV_MTESERR => general::SEGV_MTESERR,
        Code::SEGV_CPERR => general::SEGV_CPERR,
    }) as i32
}

fn bus_error_code(code: BusErrorCode) -> i32 {
    use BusErrorCode as Code;
    (match code {
        Code::BUS_ADRALN => general::BUS_ADRALN,
        Code::BUS_ADRERR => general::BUS_ADRERR,
        Code::BUS_OBJERR => general::BUS_OBJERR,
        Code::BUS_MCEERR_AR(_) => general::BUS_MCEERR_AR,
        Code::BUS_MCEERR_AO(_) => general::BUS_MCEERR_AO,
    }) as i32
}

fn trap_code(code: TrapCode) -> i32 {
    use TrapCode as Code;
    (match code {
        Code::TRAP_BRKPT => general::TRAP_BRKPT,
        Code::TRAP_TRACE => general::TRAP_TRACE,
        Code::TRAP_BRANCH => general::TRAP_BRANCH,
        Code::TRAP_HWBKPT => general::TRAP_HWBKPT,
        Code::TRAP_UNK => general::TRAP_UNK,
        Code::TRAP_PERF { .. } => general::TRAP_PERF,
    }) as i32
}

fn child_termination_code(code: ChildTerminationCode) -> i32 {
    use ChildTerminationCode as Code;
    (match code {
        Code::CLD_EXITED(_) => general::CLD_EXITED,
        Code::CLD_KILLED(_) => general::CLD_KILLED,
        Code::CLD_DUMPED(_) => general::CLD_DUMPED,
    }) as i32
}

fn child_state_code(code: ChildStateCode) -> i32 {
    use ChildStateCode as Code;
    (match code {
        Code::CLD_TRAPPED(_) => general::CLD_TRAPPED,
        Code::CLD_STOPPED(_) => general::CLD_STOPPED,
        Code::CLD_CONTINUED => general::CLD_CONTINUED,
    }) as i32
}

fn bad_system_call_code(code: BadSystemCallCode) -> i32 {
    use BadSystemCallCode as Code;
    (match code {
        Code::SYS_SECCOMP(_) => general::SYS_SECCOMP,
        Code::SYS_USER_DISPATCH => general::SYS_USER_DISPATCH,
    }) as i32
}

#[cfg(test)]
mod tests {
    use wcontainers_kernel::signal::Signal;
    use wcontainers_kernel::signal_information::{
        AddressBounds, AddressLeastSignificantBit, BadSystemCallCode, BusErrorCode, ChildStateCode,
        ChildTerminationCode, ExitStatus, PerfEventDelivery, PerfEventSignalData, PerfEventType,
        SeccompReturnData, SegmentationFaultCode, TrapCode,
    };

    use super::{
        bad_system_call_code, bus_error_code, child_state_code, child_termination_code,
        segmentation_fault_code, trap_code,
    };

    /// The codes that carry fields have Linux's numbers. These were read before as the first four bytes of
    /// any value given, which a safe caller could point at a smaller one.
    #[test]
    fn codes_with_fields_have_linux_numbers() {
        let bounds = AddressBounds { lower: 1, upper: 2 };
        assert_eq!(
            segmentation_fault_code(SegmentationFaultCode::SEGV_BNDERR(bounds)),
            3
        );
        assert_eq!(
            segmentation_fault_code(SegmentationFaultCode::SEGV_CPERR),
            10
        );
        let bit = AddressLeastSignificantBit::new(12).expect("a bit");
        assert_eq!(bus_error_code(BusErrorCode::BUS_MCEERR_AO(bit)), 5);
        let perf = TrapCode::TRAP_PERF {
            data: PerfEventSignalData(7),
            event_type: PerfEventType(1),
            delivery: PerfEventDelivery::Asynchronous,
        };
        assert_eq!(trap_code(perf), 6);
        assert_eq!(
            child_termination_code(ChildTerminationCode::CLD_EXITED(ExitStatus(3))),
            1
        );
        assert_eq!(
            child_termination_code(ChildTerminationCode::CLD_DUMPED(Signal::SIGSEGV)),
            3
        );
        assert_eq!(
            child_state_code(ChildStateCode::CLD_STOPPED(Signal::SIGSTOP)),
            5
        );
        assert_eq!(child_state_code(ChildStateCode::CLD_CONTINUED), 6);
        let data = SeccompReturnData(9);
        assert_eq!(
            bad_system_call_code(BadSystemCallCode::SYS_SECCOMP(data)),
            1
        );
        assert_eq!(
            bad_system_call_code(BadSystemCallCode::SYS_USER_DISPATCH),
            2
        );
    }
}
