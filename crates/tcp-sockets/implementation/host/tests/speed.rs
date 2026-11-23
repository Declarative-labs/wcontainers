//! What one send and one receive of a few bytes over a loopback connection take: run with `--ignored --nocapture`.
#![cfg(target_os = "linux")]

use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    BindAddress, LocalBinding, LocalPort, Port, Report, TransmitReportKey, TransmitReportRequest,
};
use wcontainers_kernel::UserId;
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpConnections, TcpContinuation, TcpFastOpen, TcpListeners, TcpLocalEnd,
    TcpLocalEndError, TcpProtocol, TcpReceiveRequest, TcpRecordBoundary, TcpSendRequest,
    TcpUrgency,
};
use wcontainers_tcp_sockets_host::HostTcpSockets;

/// Sends made before their bytes are all received, far fewer bytes than a loopback connection's buffers hold.
const BATCH: usize = 32;
/// Batches measured; the median batch is reported.
const BATCHES: usize = 4000;
const PAYLOAD: [u8; 64] = [0x5a; 64];

fn unbound() -> TcpLocalEnd<'static, wcontainers_tcp_sockets_host::HostTcpBoundSocket> {
    TcpLocalEnd::Unbound {
        protocol: TcpProtocol::Tcp,
        owner: UserId(u32::MIN),
        local: LocalBinding {
            address: BindAddress::V4(Ipv4Addr::LOCALHOST),
            port: LocalPort::Requested(Port(u16::MIN)),
        },
    }
}

/// The error of a connect or listen of a new socket.
fn error<B>(error: TcpLocalEndError<B>) -> HostError {
    match error {
        TcpLocalEndError::Bound { error, .. } | TcpLocalEndError::Unbound(error) => error,
    }
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2] / u32::try_from(BATCH).expect("a small batch")
}

#[test]
#[ignore = "a measurement, run on its own"]
fn a_send_and_a_receive_take() {
    let sockets = HostTcpSockets::new().expect("make the host's TCP sockets");
    let listening = sockets
        .listen(unbound(), ListenBacklog(1), &[])
        .map_err(error)
        .expect("listen");
    let client = sockets
        .connect(unbound(), listening.local, &[], TcpFastOpen::Off)
        .map_err(error)
        .expect("connect")
        .connection;
    let server = loop {
        match sockets.accept(&listening.listener, &mut []) {
            Ok(accepted) => break accepted.connection,
            Err(HostError::WouldBlock) => {}
            Err(error) => panic!("accept: {error:?}"),
        }
    };
    let request = TcpSendRequest {
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
    };
    let mut into = [u8::MIN; PAYLOAD.len()];
    let (mut sends, mut receives) = (Vec::new(), Vec::new());
    for _ in 0..BATCHES {
        let start = Instant::now();
        for _ in 0..BATCH {
            let taken = sockets.send(&client, request, &PAYLOAD).expect("send");
            assert_eq!(taken.0, PAYLOAD.len());
        }
        sends.push(start.elapsed());
        let start = Instant::now();
        let mut received = 0;
        while received < BATCH * PAYLOAD.len() {
            match sockets.receive(&server, TcpReceiveRequest::Read, &mut into) {
                Ok(got) => received += got.length.0,
                Err(HostError::WouldBlock) => {}
                Err(error) => panic!("receive: {error:?}"),
            }
        }
        receives.push(start.elapsed());
    }
    println!(
        "tcp loopback, 64-byte sends and receives: send {:?}, receive {:?} (median of {BATCHES} batches of {BATCH})",
        median(sends),
        median(receives)
    );
}
