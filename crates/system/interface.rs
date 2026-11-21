#![no_std]
#![allow(unused_variables)]
// TODO: a wait costs one indirect call to wake the kernel, because Rust cannot yet resolve a future's waker at compile time. Check on https://github.com/rust-lang/rfcs/pull/3434 (a future generic over its context) and on https://goals.rust-lang.org/2026/async-statemachine-optimisation.html; adopt either when it is stable.
#![allow(async_fn_in_trait)]

use core::ops::{ControlFlow, Deref, Range};

use domain::base::{Message, Name, Question};
use wcontainers_abi::{HostError, SystemCallRequest};
use wcontainers_clock::Clock;
use wcontainers_executor::Executor;
use wcontainers_icmp_sockets::IcmpSockets;
use wcontainers_kernel::signal_information::{PollEvents, SignalInformation};
use wcontainers_kernel::{
    AccessKind, MemoryObject, ProcessOf, ProcessorFeatures, SystemCallResponse, TaskId,
};
use wcontainers_random_source::RandomSource;
use wcontainers_raw_sockets::RawSockets;
use wcontainers_route_resolver::RouteResolver;
use wcontainers_tcp_sockets::TcpSockets;
use wcontainers_udp_sockets::UdpSockets;

pub trait System:
    ProcessTracer
    + ProcessorInputs
    + TaskIdAllocator
    + StreamTransport
    + Clock
    + RandomSource
    + FallbackObserver
    + FuseServer
    + TcpSockets
    + UdpSockets
    + IcmpSockets
    + RawSockets
    + RouteResolver
    + DomainNameResolver
    + Sync
{
}

/// The observer and controller of a process's execution.
pub trait ProcessTracer {
    type ProcessState: Clone + Send;
    type Executor: Executor;

    fn intercept_system_call_request<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        ControlFlow::Continue(request)
    }
    fn intercept_system_call_response<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        thread: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse {
        response
    }

    fn on_process_starting<P: ProcessOf<Self::ProcessState, Self::Executor>>(&self, process: P) {}
    fn on_program_starting<P: ProcessOf<Self::ProcessState, Self::Executor>>(&self, process: P) {}
    fn on_thread_starting<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        thread: TaskId,
    ) {
    }
    fn on_process_exiting<P: ProcessOf<Self::ProcessState, Self::Executor>>(&self, process: P) {}
    fn on_process_outgrown<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        bytes: u64,
    ) {
    }

    fn on_watched_memory_accessing<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        thread: TaskId,
        address: u64,
        kind: AccessKind,
    ) {
    }
    fn on_memory_written_by_kernel<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        thread: TaskId,
        range: Range<u64>,
    ) {
    }

    fn on_shared_memory_mapping<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
    }
    fn on_shared_memory_unmapped<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    ) {
    }

    fn on_signal_delivering<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        thread: TaskId,
        signal: &SignalInformation,
    ) {
    }

    /// Another process is about to read this one's internal state.
    fn on_inspection_starting<P: ProcessOf<Self::ProcessState, Self::Executor>>(&self, process: P) {
    }
    fn on_inspection_ended<P: ProcessOf<Self::ProcessState, Self::Executor>>(&self, process: P) {}
}

/// The answers to what a program reads from its processor directly (without a [system call](https://man7.org/linux/man-pages/man2/syscalls.2.html)).
pub trait ProcessorInputs: ProcessTracer {
    fn choose_processor_features<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        supported: &ProcessorFeatures,
    ) -> ProcessorFeatures {
        *supported
    }
    fn read_monotonic_nanoseconds<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        thread: TaskId,
    ) -> u64;
    fn read_random_number<P: ProcessOf<Self::ProcessState, Self::Executor>>(
        &self,
        process: P,
        thread: TaskId,
    ) -> u64;
}

/// The largest [`pid_max`](https://man7.org/linux/man-pages/man5/proc_sys_kernel.5.html) Linux allows on a 64-bit machine.
pub const TASK_ID_LIMIT: u32 = 4_194_304;

/// The task ID that allocation restarts from after it reaches the limit, as Linux keeps the lower ones for early processes.
pub const FIRST_TASK_ID_AFTER_WRAPPING: u32 = 300;

pub trait TaskIdAllocator {
    fn allocate_task_id(&self, previous: TaskId) -> TaskId {
        let next = previous.0 + 1;
        TaskId(if next >= TASK_ID_LIMIT {
            FIRST_TASK_ID_AFTER_WRAPPING
        } else {
            next
        })
    }
    fn free_task_id(&self, id: TaskId);
}

/// A transport of byte streams to and from endpoints.
pub trait StreamTransport {
    type Endpoint: Ord;

    fn send_stream(&self, peer: &Self::Endpoint, bytes: &[u8]) -> Result<usize, HostError>;
    fn receive_stream(&self, peer: &Self::Endpoint, into: &mut [u8]) -> Result<usize, HostError>;
    /// Waits until the stream has one of the events asked for, and returns the events it has.
    async fn wait_for_stream_events(
        &self,
        peer: &Self::Endpoint,
        interest: PollEvents,
    ) -> Result<PollEvents, HostError>;
    fn close_stream(&self, peer: &Self::Endpoint);
}

// When a fallback implementation is used internally somewhere, this will fire.
pub trait FallbackObserver {
    type Fallback;

    fn on_fallback_used(&self, which: &Self::Fallback) {}
}

pub trait FuseServer {
    type Reply: Deref<Target = [u8]>;

    /// Sends a FUSE request and returns its reply; a request that FUSE gives no reply finishes at once with an empty one.
    async fn send_request(&self, request: &[u8]) -> Result<Self::Reply, HostError>;
}

/// A [DNS resolver](https://man7.org/linux/man-pages/man5/resolv.conf.5.html).
pub trait DomainNameResolver {
    type Answer: Deref<Target = Message<[u8]>>;

    async fn resolve_domain(
        &self,
        question: Question<&Name<[u8]>>,
    ) -> Result<Self::Answer, HostError>;
}
