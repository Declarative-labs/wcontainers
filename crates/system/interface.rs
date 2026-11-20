//! The embedder's side of its interface to the kernel: one trait per concern, and their union.

#![no_std]
#![allow(unused_variables)]

use core::ops::{ControlFlow, Range};

use wcontainers_abi::{BackendRequestId, ConnectionId, End, HostError, SystemCallRequest, Warning};
use wcontainers_kernel::TaskId;
use wcontainers_kernel_interface::{
    AccessKind, MemoryObject, Process, SignalInformation, SystemCallResponse,
};

/// The embedder of a kernel, as the union of every concern below.
pub trait System:
    ProcessTracer
    + TaskIdSource
    + StreamPeer
    + Clock
    + RandomSource
    + WarningLog
    + FileServer
    + Network
    + NameResolver
    + Sync
{
}

/// The system call proxy and the hooks on processes, as ruled.
pub trait ProcessTracer: Sync {
    type ProcessState: Clone + Send;

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
    fn on_thread_starting<P: Process<State = Self::ProcessState>>(
        &self,
        process: P,
        thread: TaskId,
    ) {
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
    /// Announces that another process has finished reading this one's internal state.
    fn on_inspection_ended<P: Process<State = Self::ProcessState>>(&self, process: P) {}
}

/// The source of the ids of processes and threads, as ruled.
pub trait TaskIdSource: Sync {
    fn allocate_task_id(&self, previous: TaskId) -> TaskId {
        wcontainers_kernel::task_id_after(previous)
    }
    fn free_task_id(&self, id: TaskId);
}

/// The other side of every stream end the kernel holds, as ruled.
pub trait StreamPeer: Sync {
    fn send_stream(&self, end: End, bytes: &[u8]);
    fn credit_stream(&self, end: End, bytes: u64);
    fn close_stream(&self, end: End);
}

/// The monotonic clock and the wall clock, as abi's `Host` has them today and not yet ruled.
pub trait Clock: Sync {
    fn monotonic_nanoseconds(&self) -> u64;
    fn realtime_nanoseconds(&self) -> u64;
}

/// A source of random bytes, as abi's `Host` has it today and not yet ruled.
pub trait RandomSource: Sync {
    fn fill_random_bytes(&self, into: &mut [u8]);
}

/// The log of the kernel's warnings, as abi's `Host` has it today and not yet ruled.
pub trait WarningLog: Sync {
    fn warn(&self, warning: Warning<'_>);
}

/// The server of the container's files over FUSE, as abi's `Host` has it today and not yet ruled.
pub trait FileServer: Sync {
    fn send_backend(&self, request: BackendRequestId, message: &[u8]) -> Result<(), HostError>;
}

/// The network outside the container, as abi's `Host` has it today and not yet ruled.
pub trait Network: Sync {
    fn connect(
        &self,
        connection: ConnectionId,
        address: &[u8],
        port: u16,
        stream: bool,
    ) -> Result<(), HostError>;
    fn send(&self, connection: ConnectionId, bytes: &[u8]) -> Result<usize, HostError>;
    fn receive(&self, connection: ConnectionId, into: &mut [u8]) -> Result<usize, HostError>;
    fn finish(&self, connection: ConnectionId);
    fn close(&self, connection: ConnectionId);
    fn listen(&self, listener: ConnectionId, port: u16) -> Result<(), HostError>;
    fn accept(&self, listener: ConnectionId, connection: ConnectionId) -> Result<(), HostError>;
}

/// The resolver of names to addresses, as abi's `Host` has it today and not yet ruled.
pub trait NameResolver: Sync {
    fn look_up(&self, request: u64, name: &str) -> Result<(), HostError>;
}
