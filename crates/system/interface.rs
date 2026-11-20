//! The embedder's side of its interface to the kernel: the task ids it hands out, the system call proxy and the hooks.

#![allow(unused_variables)]

use std::ops::{ControlFlow, Range};

use wcontainers_abi::SystemCallRequest;
use wcontainers_kernel::TaskId;
use wcontainers_process::{
    AccessKind, MemoryObject, Process, SignalInformation, SystemCallResponse,
};

/// The embedder of a kernel, which every hook and service of the kernel reaches.
pub trait System: Sync {
    type ProcessState: Clone + Send;

    fn allocate_task_id(&self, previous: TaskId) -> TaskId {
        wcontainers_kernel::task_id_after(previous)
    }
    fn free_task_id(&self, id: TaskId);

    fn intercept_system_call_request<P: Process<State = Self::ProcessState>>(
        &self,
        process: P,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        ControlFlow::Continue(request)
    }
    fn intercept_system_call_response<P: Process<State = Self::ProcessState>>(
        &self,
        process: P,
        thread: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        response
    }

    fn on_process_starting<P: Process<State = Self::ProcessState>>(&self, process: P) {}
    fn on_program_starting<P: Process<State = Self::ProcessState>>(&self, process: P) {}
    fn on_thread_starting<P: Process<State = Self::ProcessState>>(&self, process: P, thread: TaskId) {
    }
    fn on_process_exiting<P: Process<State = Self::ProcessState>>(&self, process: P) {}
    fn on_process_outgrown<P: Process<State = Self::ProcessState>>(&self, process: P, bytes: u64) {}

    fn on_watched_memory_accessing<P: Process<State = Self::ProcessState>>(
        &self,
        process: P,
        thread: TaskId,
        address: u64,
        kind: AccessKind,
    ) {
    }
    fn on_memory_written_by_kernel<P: Process<State = Self::ProcessState>>(
        &self,
        process: P,
        thread: TaskId,
        range: Range<u64>,
    ) {
    }

    fn on_signal_delivering<P: Process<State = Self::ProcessState>>(
        &self,
        process: P,
        thread: TaskId,
        signal: &SignalInformation,
    ) {
    }

    fn on_shared_memory_mapping<P: Process<State = Self::ProcessState>>(
        &self,
        process: P,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
    }
    fn on_shared_memory_unmapped<P: Process<State = Self::ProcessState>>(
        &self,
        process: P,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
    }

    /// Another process is about to read this one's internal state.
    fn on_inspection_starting<P: Process<State = Self::ProcessState>>(&self, process: P) {}
    /// Another process has finished reading this one's internal state.
    fn on_inspection_ended<P: Process<State = Self::ProcessState>>(&self, process: P) {}
}
