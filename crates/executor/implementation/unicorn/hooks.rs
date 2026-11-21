//! What a test does at the kernel's hooks and task id calls.

use std::ops::{ControlFlow, Range};

use wcontainers_abi::{Cpu, SystemCallRequest};
use wcontainers_kernel_linux::linux::general::{SI_DETHREAD, SI_KERNEL, SI_TIMER};
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
        Record::SegmentationFault { code, .. } => (Signal::SIGSEGV, discriminant(&code), 0),
        Record::BusError { code, .. } => (Signal::SIGBUS, discriminant(&code), 0),
        Record::Trap { code, .. } => (Signal::SIGTRAP, discriminant(&code), 0),
        Record::ChildTerminated { signal, code, .. } => (signal, discriminant(&code), 0),
        Record::ChildStateChanged { code, .. } => (Signal::SIGCHLD, discriminant(&code), 0),
        Record::BadSystemCall { code, .. } => (Signal::SIGSYS, discriminant(&code), 0),
        Record::TracerStop { .. } => (Signal::SIGTRAP, 0, 0),
    };
    (signal as i32, code, value)
}

/// The number of one of the interface's `repr(i32)` codes that carry fields.
fn discriminant<T: Copy>(code: &T) -> i32 {
    // SAFETY: called only with the interface's `repr(i32)` code enums, which start with their
    // discriminant as an `i32`.
    unsafe { std::ptr::from_ref(code).cast::<i32>().read() }
}
