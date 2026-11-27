#![no_std]
#![allow(unused_variables)]
// TODO: a wait costs one indirect call to wake the kernel, because Rust cannot yet resolve a future's waker at compile time. Check on https://github.com/rust-lang/rfcs/pull/3434 (a future generic over its context) and on https://goals.rust-lang.org/2026/async-statemachine-optimisation.html; adopt either when it is stable.
#![allow(async_fn_in_trait)]

use core::num::NonZeroUsize;
use core::ops::{ControlFlow, Deref, Range};

use domain::base::{Message, Name, Question};
use vstd::prelude::*;
use wcontainers_abi::{HostError, SystemCallRequest};
use wcontainers_clock::Clock;
use wcontainers_icmp_sockets::IcmpSockets;
use wcontainers_kernel::errno::Errno;
use wcontainers_kernel::signal_information::SignalInformation;
use wcontainers_kernel::{
    AccessKind, Kernel, MemoryObject, ProcessOf, ProcessorFeatures, SystemCallResponse, TaskId,
};
use wcontainers_random_source::RandomSource;
use wcontainers_raw_sockets::RawSockets;
use wcontainers_readiness::Readiness;
use wcontainers_route_resolver::RouteResolver;
use wcontainers_tcp_sockets::TcpSockets;
use wcontainers_udp_sockets::UdpSockets;
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

pub trait System:
    ProcessTracer
    + ProcessorInputs
    + MachineDescription
    + TaskIdAllocator
    + StreamTransport
    + Clock
    + RandomSource
    + FallbackObserver<<<Self as ProcessTracer>::Kernel as Kernel>::Fallback>
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
    type Kernel: Kernel;

    fn intercept_system_call_request<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
        ControlFlow::Continue(request)
    }
    fn intercept_system_call_response<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        thread: TaskId,
        request: &SystemCallRequest,
        response: SystemCallResponse,
    ) -> SystemCallResponse
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
        response
    }

    fn on_process_starting<P: ProcessOf<Self::ProcessState, Self::Kernel>>(&self, process: P)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {}
    fn on_program_starting<P: ProcessOf<Self::ProcessState, Self::Kernel>>(&self, process: P)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {}
    fn on_thread_starting<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        thread: TaskId,
    )
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
    }
    fn on_process_exiting<P: ProcessOf<Self::ProcessState, Self::Kernel>>(&self, process: P)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {}
    fn on_process_outgrown<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        bytes: u64,
    )
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
    }

    fn on_watched_memory_accessing<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        thread: TaskId,
        address: u64,
        kind: AccessKind,
    )
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
    }
    fn on_memory_written_by_kernel<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        thread: TaskId,
        range: Range<u64>,
    )
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
    }

    fn on_shared_memory_mapping<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    )
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
    }
    fn on_shared_memory_unmapped<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        source: MemoryObject,
        offset: u64,
        range: Range<u64>,
    )
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
    }

    fn on_signal_delivering<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        thread: TaskId,
        signal: &SignalInformation,
    )
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
    }

    /// Runs when another process is about to read this one's internal state.
    fn on_inspection_starting<P: ProcessOf<Self::ProcessState, Self::Kernel>>(&self, process: P)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {}
    fn on_inspection_ended<P: ProcessOf<Self::ProcessState, Self::Kernel>>(&self, process: P)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {}
}

/// The answers to what a program reads from its processor directly (without a [system call](https://man7.org/linux/man-pages/man2/syscalls.2.html)).
pub trait ProcessorInputs: ProcessTracer {
    fn choose_processor_features<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        supported: &ProcessorFeatures,
    ) -> ProcessorFeatures
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
        *supported
    }
    fn read_monotonic_nanoseconds<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        thread: TaskId,
    ) -> u64
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn read_random_number<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        thread: TaskId,
    ) -> u64
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

pub trait MachineDescription: ProcessTracer {
    /// Returns the number of [processors the process is told are online](https://man7.org/linux/man-pages/man3/sysconf.3.html#:~:text=The%20number%20of%20processors%20currently%20online).
    fn report_online_processors<P: ProcessOf<Self::ProcessState, Self::Kernel>>(
        &self,
        process: P,
        available: NonZeroUsize,
    ) -> NonZeroUsize
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
        available
    }

    /// Returns the size in bytes of the memory the process is told the machine has.
    fn report_memory_size<P: ProcessOf<Self::ProcessState, Self::Kernel>>(&self, process: P)
    -> u64
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

pub trait TaskIdAllocator {
    fn allocate_task_id(&self, previous: TaskId) -> TaskId
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
        let next = previous.0 + 1;
        TaskId(if next >= TASK_ID_LIMIT {
            FIRST_TASK_ID_AFTER_WRAPPING
        } else {
            next
        })
    }
    fn free_task_id(&self, id: TaskId)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

/// The largest [`pid_max`](https://man7.org/linux/man-pages/man5/proc_sys_kernel.5.html#:~:text=pid_max%20%28since) Linux allows on a 64-bit machine.
pub const TASK_ID_LIMIT: u32 = 4_194_304;

/// The task ID that allocation restarts from after it reaches the limit, as Linux [keeps the lower ones](https://github.com/torvalds/linux/blob/v6.18/include/linux/pid.h#L48) for early processes.
pub const FIRST_TASK_ID_AFTER_WRAPPING: u32 = 300;

#[allow(
    reviewable::explicit_shared_error::HostError,
    reason = "both directions of one stream fail the same ways and the kernel tells only would-block from hung up"
)]
/// A transport of byte streams to and from endpoints.
pub trait StreamTransport: Readiness<Self::Endpoint> {
    type Endpoint: Ord;

    fn send_stream(
        &self,
        peer: &Self::Endpoint,
        payload: &impl VolatileBytes,
    ) -> Result<usize, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn receive_stream(
        &self,
        peer: &Self::Endpoint,
        into: &impl VolatileBytes,
    ) -> Result<usize, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn close_stream(&self, peer: &Self::Endpoint)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

// When a fallback implementation is used internally somewhere, this will fire.
pub trait FallbackObserver<Fallback> {
    fn on_fallback_used(&self, which: &Fallback)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {}
}

#[allow(
    reviewable::explicit_shared_error::HostError,
    reason = "both methods use the one connection to the server, so both fail only when there is no server or it is gone"
)]
/// A [FUSE](https://man7.org/linux/man-pages/man4/fuse.4.html) server.
pub trait FuseServer {
    type Reply: Deref<Target = [u8]>;
    type Notice: Deref<Target = [u8]>;

    /// Sends a FUSE request and returns its reply; a request FUSE never replies to completes immediately with an empty reply.
    ///
    /// The reply completes only after every notice the server sent before it has been applied.
    async fn send_request(&self, request: &[u8]) -> Result<Self::Reply, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Waits for the next [notice](https://libfuse.github.io/doxygen/fuse__lowlevel_8h.html#a9cb974af9745294ff446d11cba2422f1) the server sends unasked.
    ///
    /// Calling it again tells the server that the notice before was applied, and with what result.
    async fn receive_notice(
        &self,
        applied: Option<Result<(), Errno>>,
    ) -> Result<Self::Notice, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

/// A [DNS resolver](https://man7.org/linux/man-pages/man5/resolv.conf.5.html).
pub trait DomainNameResolver {
    type AnswerBytes: AsRef<[u8]>;

    async fn resolve_domain(
        &self,
        question: Question<&Name<[u8]>>,
    ) -> Result<Message<Self::AnswerBytes>, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
