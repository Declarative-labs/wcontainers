//! The local system as the system interface describes it: the kernel it serves, processor inputs and the machine.
//!
//! Its ICMP echo sockets, raw sockets and routes are the host's.

use core::cell::RefCell;
use std::num::NonZeroUsize;

use etherparse::IpNumber;
use wcontainers_abi::{Cpu, HostError, Warning};
use wcontainers_clock::Clock;
use wcontainers_configurable::Configurable;
use wcontainers_executor::Executor;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_icmp_sockets::{EchoBinding, EchoIdentifier, IcmpSockets};
use wcontainers_icmp_sockets_host::{HostIcmpSocket, HostIcmpSockets};
use wcontainers_internet_protocol::{
    BufferSizes, ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion,
    NeighbourConfirmation, Received, Route, RouteSelectors, SendError,
};
use wcontainers_kernel::{ProcessOf, TaskId, UserId};
use wcontainers_kernel_linux::KernelWithExecutor;
use wcontainers_random_source::RandomSource;
use wcontainers_raw_sockets::{RawPacket, RawSocketSetting, RawSockets};
use wcontainers_raw_sockets_host::{HostRawSocket, HostRawSockets};
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};
use wcontainers_route_resolver::RouteResolver;
use wcontainers_route_resolver_host::HostRouteResolver;
use wcontainers_system::{FallbackObserver, MachineDescription, ProcessTracer, ProcessorInputs};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::{Interruptible, LocalSystem};

/// Why the host's sockets of a kind are open whenever one of their sockets is asked about.
const OPENED_WITH_ITS_SOCKET: &str =
    "a socket is only given out once the host's sockets of its kind opened";

/// The kernel a local system on the executor `E` serves; its processes carry no state of the system's.
pub(crate) type LocalKernel<E> = KernelWithExecutor<Interruptible<E>, (), LocalSystem<E>>;

impl<E> ProcessTracer for LocalSystem<E>
where
    E: Cpu
        + Executor<TranslatorConfig: Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>>
        + Send
        + 'static,
{
    type ProcessState = ();
    type Kernel = LocalKernel<E>;
}

impl<E> ProcessorInputs for LocalSystem<E>
where
    E: Cpu
        + Executor<TranslatorConfig: Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>>
        + Send
        + 'static,
{
    fn read_monotonic_nanoseconds<P: ProcessOf<(), LocalKernel<E>>>(&self, _: P, _: TaskId) -> u64 {
        Clock::monotonic_nanoseconds(self)
    }

    fn read_random_number<P: ProcessOf<(), LocalKernel<E>>>(&self, _: P, _: TaskId) -> u64 {
        let mut bytes = u64::MIN.to_ne_bytes();
        self.fill_random_bytes(&RefCell::new(&mut bytes[..]))
            .expect("the host gives random bytes");
        u64::from_ne_bytes(bytes)
    }
}

impl<E> MachineDescription for LocalSystem<E>
where
    E: Cpu
        + Executor<TranslatorConfig: Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>>
        + Send
        + 'static,
{
    fn report_online_processors<P: ProcessOf<(), LocalKernel<E>>>(
        &self,
        _: P,
        available: NonZeroUsize,
    ) -> NonZeroUsize {
        available
    }

    /// Returns what [`LocalSystem::describe_memory`] set as the container was made.
    fn report_memory_size<P: ProcessOf<(), LocalKernel<E>>>(&self, _: P) -> u64 {
        *self
            .machine_memory
            .get()
            .expect("the machine's memory is described as the container is made")
    }
}

/// Every fallback is kept with the kernel's warnings.
impl<E> FallbackObserver<Warning<'static>> for LocalSystem<E> {
    fn on_fallback_used(&self, which: &Warning<'static>) {
        self.state().keep(*which, false);
    }
}

impl<E> LocalSystem<E> {
    fn icmp(&self) -> Result<&HostIcmpSockets, HostError> {
        self.icmp
            .get_or_init(HostIcmpSockets::new)
            .as_ref()
            .map_err(|error| *error)
    }

    fn raw(&self) -> Result<&HostRawSockets, HostError> {
        self.raw
            .get_or_init(HostRawSockets::new)
            .as_ref()
            .map_err(|error| *error)
    }

    fn routes(&self) -> &HostRouteResolver {
        self.routes.get_or_init(HostRouteResolver::open)
    }
}

/// The host's ping sockets.
impl<E> IcmpSockets for LocalSystem<E> {
    type Socket = HostIcmpSocket;

    fn open(&self, version: IpVersion, owner: UserId) -> Result<HostIcmpSocket, HostError> {
        self.icmp()?.open(version, owner)
    }

    fn bind(
        &self,
        socket: &HostIcmpSocket,
        local: EchoBinding,
    ) -> Result<EchoIdentifier, HostError> {
        self.icmp()?.bind(socket, local)
    }

    fn send_to(
        &self,
        socket: &HostIcmpSocket,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        message: &impl VolatileBytes,
    ) -> Result<(), SendError> {
        self.icmp()
            .map_err(SendError::Host)?
            .send_to(socket, header, fields, message)
    }

    fn receive(
        &self,
        socket: &HostIcmpSocket,
        into: &impl VolatileBytes,
    ) -> Result<Received, HostError> {
        self.icmp()?.receive(socket, into)
    }

    fn count_unsent_bytes(&self, socket: &HostIcmpSocket) -> ByteCount {
        self.icmp()
            .expect(OPENED_WITH_ITS_SOCKET)
            .count_unsent_bytes(socket)
    }

    fn read_buffer_sizes(&self, socket: &HostIcmpSocket) -> BufferSizes {
        self.icmp()
            .expect(OPENED_WITH_ITS_SOCKET)
            .read_buffer_sizes(socket)
    }

    fn close(&self, socket: HostIcmpSocket) {
        if let Ok(icmp) = self.icmp() {
            icmp.close(socket);
        }
    }
}

impl<E> Readiness<HostIcmpSocket> for LocalSystem<E> {
    async fn wait_until_ready(
        &self,
        socket: &HostIcmpSocket,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        self.icmp()?.wait_until_ready(socket, interest).await
    }
}

impl<'a, E> Configurable<HostIcmpSocket, IpSocketSetting<'a>> for LocalSystem<E> {
    fn configure(
        &self,
        socket: &HostIcmpSocket,
        setting: IpSocketSetting<'a>,
    ) -> Result<(), HostError> {
        self.icmp()?.configure(socket, setting)
    }
}

/// The host's raw sockets.
impl<E> RawSockets for LocalSystem<E> {
    type Socket = HostRawSocket;

    fn open(
        &self,
        version: IpVersion,
        protocol: IpNumber,
        owner: UserId,
    ) -> Result<HostRawSocket, HostError> {
        self.raw()?.open(version, protocol, owner)
    }

    fn send_to(
        &self,
        socket: &HostRawSocket,
        packet: RawPacket<'_>,
        fields: IpSendFields,
    ) -> Result<(), SendError> {
        self.raw()
            .map_err(SendError::Host)?
            .send_to(socket, packet, fields)
    }

    fn receive(
        &self,
        socket: &HostRawSocket,
        into: &impl VolatileBytes,
    ) -> Result<Received, HostError> {
        self.raw()?.receive(socket, into)
    }

    fn count_unsent_bytes(&self, socket: &HostRawSocket) -> ByteCount {
        self.raw()
            .expect(OPENED_WITH_ITS_SOCKET)
            .count_unsent_bytes(socket)
    }

    fn read_buffer_sizes(&self, socket: &HostRawSocket) -> BufferSizes {
        self.raw()
            .expect(OPENED_WITH_ITS_SOCKET)
            .read_buffer_sizes(socket)
    }

    fn close(&self, socket: HostRawSocket) {
        if let Ok(raw) = self.raw() {
            raw.close(socket);
        }
    }
}

impl<E> Readiness<HostRawSocket> for LocalSystem<E> {
    async fn wait_until_ready(
        &self,
        socket: &HostRawSocket,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        self.raw()?.wait_until_ready(socket, interest).await
    }
}

impl<'a, E> Configurable<HostRawSocket, RawSocketSetting<'a>> for LocalSystem<E> {
    fn configure(
        &self,
        socket: &HostRawSocket,
        setting: RawSocketSetting<'a>,
    ) -> Result<(), HostError> {
        self.raw()?.configure(socket, setting)
    }
}

/// The host's routes.
impl<E> RouteResolver for LocalSystem<E> {
    fn resolve_route(
        &self,
        route: RouteSelectors<'_>,
        owner: UserId,
        neighbour: NeighbourConfirmation,
    ) -> Result<Route, HostError> {
        self.routes().resolve_route(route, owner, neighbour)
    }
}

impl<E> wcontainers_tcp_sockets::TcpSockets for LocalSystem<E> {}

impl<E> wcontainers_system::System for LocalSystem<E> where
    E: Cpu
        + Executor<TranslatorConfig: Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>>
        + Send
        + 'static
{
}

/// Checks, as it builds, that every kernel this system can serve is served by the whole system interface.
#[cfg(test)]
#[expect(dead_code, reason = "checked as it is built, never called")]
fn serves_the_whole_interface<E>()
where
    E: Cpu
        + Executor<TranslatorConfig: Send + Sync, GuestMemory: Send + MemorySpace<PageTable: Send>>
        + Send
        + 'static,
{
    fn system<S: wcontainers_system::System>() {}
    system::<LocalSystem<E>>();
}
