//! What a wait and a receive allocate on the waiting thread, counted by allocation-counter's allocator.

use std::cell::RefCell;
use std::future::Future;
use std::net::Ipv4Addr;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Wake, Waker};
use std::thread::Thread;
use std::time::{Duration, Instant};

use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BindAddress, ByteCount, IpSocketSetting, LocalBinding, LocalPort, Port, Report, Timestamping,
    TransmitReportKey, TransmitReportRequest,
};
use wcontainers_kernel::UserId;
use wcontainers_readiness::{Readiness, ReadyFor};
use wcontainers_tcp_sockets::settings::TcpSetting;
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpConnections, TcpContinuation, TcpFastOpen, TcpListeners, TcpLocalEnd,
    TcpLocalEndError, TcpProtocol, TcpReceiveRequest, TcpRecordBoundary, TcpSendRequest,
    TcpUrgency,
};
use wcontainers_tcp_sockets_host::{
    HostTcpBoundSocket, HostTcpConnection, HostTcpListener, HostTcpSockets,
};

struct Unparking(Thread);

impl Wake for Unparking {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// The cycles each check measures.
const MEASURED_CYCLES: usize = 100;

/// The cycles before, which warm the connection's waiter lists.
const WARM_UP: usize = 10;

/// How long a wait may take to be woken once its data is sent.
const DEADLINE: Duration = Duration::from_secs(10);

/// Gives a listener and the two ends of a connection to it, on IPv4 loopback.
fn connected(sockets: &HostTcpSockets) -> (HostTcpListener, HostTcpConnection, HostTcpConnection) {
    let local = || TcpLocalEnd::Unbound {
        protocol: TcpProtocol::Tcp,
        owner: UserId(u32::MIN),
        local: LocalBinding {
            address: BindAddress::V4(Ipv4Addr::LOCALHOST),
            port: LocalPort::Requested(Port(u16::MIN)),
        },
    };
    let failed = |error: TcpLocalEndError<HostTcpBoundSocket>| match error {
        TcpLocalEndError::Bound { error, .. } | TcpLocalEndError::Unbound(error) => error,
    };
    let listening = sockets
        .listen(local(), ListenBacklog(1), &[])
        .map_err(failed)
        .expect("listen");
    let client = sockets
        .connect(local(), listening.local, &[], TcpFastOpen::Off)
        .map_err(failed)
        .expect("connect")
        .connection;
    let server = loop {
        match sockets.accept(&listening.listener, &mut []) {
            Ok(accepted) => break accepted.connection,
            Err(HostError::WouldBlock) => std::thread::yield_now(),
            Err(error) => panic!("accept: {error:?}"),
        }
    };
    (listening.listener, client, server)
}

fn plain() -> TcpSendRequest {
    TcpSendRequest {
        urgency: TcpUrgency::Normal,
        continuation: TcpContinuation::Push,
        record: TcpRecordBoundary::Open,
        transmit_reports: TransmitReportRequest {
            key: TransmitReportKey(u32::MIN),
            scheduled: Report::NotRequested,
            software: Report::NotRequested,
            hardware: Report::NotRequested,
            acknowledgement: Report::NotRequested,
            completion: Report::NotRequested,
            wireless_status: Report::NotRequested,
            packet: Report::NotRequested,
        },
        transmit_statistics: Report::NotRequested,
    }
}

/// Polls the wait until it finishes, parking between polls.
fn finish<F: Future>(mut wait: std::pin::Pin<&mut F>, context: &mut Context<'_>) {
    let start = Instant::now();
    while wait.as_mut().poll(context).is_pending() {
        assert!(start.elapsed() < DEADLINE, "the wait was not woken");
        std::thread::park_timeout(DEADLINE);
    }
}

/// Runs the warm-up cycles, then gives the allocations of the measured cycles.
fn allocations_of(mut cycle: impl FnMut()) -> u64 {
    for _ in 0..WARM_UP {
        cycle();
    }
    allocation_counter::measure(|| {
        for _ in 0..MEASURED_CYCLES {
            cycle();
        }
    })
    .count_total
}

/// Once a connection has waited, its waits that have to wait allocate nothing; a receive that brings control
/// messages, a timestamp and the unread byte count allocates nothing.
fn a_warm_wait_and_a_receive_with_control_messages_allocate_nothing() {
    let sockets = HostTcpSockets::new().expect("readiness");
    let (_listener, client, server) = connected(&sockets);
    let waker = Waker::from(Arc::new(Unparking(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut byte = [0; 1];
    let waits = allocations_of(|| {
        let mut wait = pin!(sockets.wait_until_ready(&server, ReadyFor::READABLE));
        assert!(wait.as_mut().poll(&mut context).is_pending());
        sockets
            .send(&client, plain(), &RefCell::new(&mut byte[..]))
            .expect("send");
        finish(wait, &mut context);
        sockets
            .receive(
                &server,
                TcpReceiveRequest::Read,
                &RefCell::new(&mut byte[..]),
            )
            .expect("receive");
    });
    assert_eq!(waits, 0, "allocations of {MEASURED_CYCLES} waits");

    let timestamping = Timestamping {
        receive_software: Report::Requested,
        receive_hardware: Report::NotRequested,
        bound_clock: None,
    };
    sockets
        .configure(
            &server,
            TcpSetting::Ip(IpSocketSetting::Timestamping(timestamping)),
        )
        .expect("timestamping");
    sockets
        .configure(&server, TcpSetting::UnreadByteReports(Report::Requested))
        .expect("unread byte reports");
    let receives = allocations_of(|| {
        sockets
            .send(&client, plain(), &RefCell::new(&mut byte[..]))
            .expect("send");
        finish(
            pin!(sockets.wait_until_ready(&server, ReadyFor::READABLE)),
            &mut context,
        );
        let received = sockets
            .receive(
                &server,
                TcpReceiveRequest::Read,
                &RefCell::new(&mut byte[..]),
            )
            .expect("receive");
        assert!(received.timestamps.software.is_some());
        assert_eq!(received.unread_after, Some(ByteCount(0)));
    });
    assert_eq!(receives, 0, "allocations of {MEASURED_CYCLES} receives");

    let counted = allocation_counter::measure(|| drop(std::hint::black_box(Box::new(u8::MIN))));
    assert_eq!(counted.count_total, 1, "the allocator counts");
}

/// Runs the check on Linux, the system whose implementation it measures.
fn main() {
    if cfg!(target_os = "linux") {
        a_warm_wait_and_a_receive_with_control_messages_allocate_nothing();
    }
}
