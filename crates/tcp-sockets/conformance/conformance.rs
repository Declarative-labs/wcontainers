//! The tests every implementation of wcontainers-tcp-sockets runs, written against its traits alone.

use core::net::{Ipv4Addr, SocketAddr};

use wcontainers_internet_protocol::{
    BindAddress, ByteCount, IpSocketSetting, LocalBinding, LocalPort, Port, Report,
    TransmitReportKey, TransmitReportRequest,
};
use wcontainers_kernel_interface::UserId;
use wcontainers_tcp_sockets::authentication::TcpAuthenticationCounters;
use wcontainers_tcp_sockets::repair::{TcpTimestamp, TcpTimestampClock, TcpTimestampUnit};
use wcontainers_tcp_sockets::settings::{
    SegmentCount, TcpListenerSetting, TcpQuickAcknowledgement, TcpSetting,
};
use wcontainers_tcp_sockets::state::{TcpListenerQuery, TcpQuery};
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpClose, TcpContinuation, TcpFastOpen, TcpProtocol, TcpPushOverride,
    TcpReceiveRequest, TcpRecordBoundary, TcpSendRequest, TcpShutdown, TcpSockets, TcpUrgency,
};

/// Calls each method of `TcpSockets` as a placeholder that fails until the real checks are written.
pub fn call_tcp_sockets_methods<S: TcpSockets>(sockets: S) {
    let owner = UserId(u32::MIN);
    let local = LocalBinding {
        address: BindAddress::V4(Ipv4Addr::UNSPECIFIED),
        port: LocalPort::Requested(Port(u16::MIN)),
    };
    let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, u16::MIN));
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
    let counters = TcpAuthenticationCounters {
        verified: SegmentCount(u64::MIN),
        failed: SegmentCount(u64::MIN),
        key_not_found: SegmentCount(u64::MIN),
        signature_missing: SegmentCount(u64::MIN),
        icmp_errors_ignored: SegmentCount(u64::MIN),
    };
    let clock = TcpTimestampClock {
        value: TcpTimestamp(u32::MIN),
        unit: TcpTimestampUnit::Milliseconds,
    };
    if let Ok(connected) =
        sockets.connect(TcpProtocol::Tcp, owner, local, peer, &[], TcpFastOpen::Off)
    {
        let connection = connected.connection;
        let _ = sockets.configure(
            &connection,
            TcpSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(usize::MIN))),
        );
        let _ = sockets.send(&connection, request, &[]);
        let _ = sockets.receive(&connection, TcpReceiveRequest::Read, &mut []);
        let _ = sockets.take_error_queue_entry(&connection, &mut []);
        let _ = sockets.read_state(&connection, TcpQuery::Information);
        let _ = sockets.push_queued_data(&connection, TcpPushOverride::Neither);
        let _ = sockets.send_delayed_acknowledgement(&connection, TcpQuickAcknowledgement::On);
        let _ = sockets.rearm_keep_alive_timer(&connection);
        let _ = sockets.discard_error_queue_entries(&connection);
        let _ = sockets.send_window_probe(&connection);
        let _ = sockets.set_timestamp_clock(&connection, clock);
        let _ = sockets.set_authentication_counters(&connection, counters);
        let _ = sockets.give_negative_route_advice(&connection);
        let _ = sockets.shut_down(&connection, TcpShutdown::Write);
        sockets.close(connection, TcpClose::Orderly);
    }
    if let Ok(listening) =
        sockets.listen(TcpProtocol::Tcp, owner, local, ListenBacklog(u32::MIN), &[])
    {
        let listener = listening.listener;
        let _ = sockets.configure(
            &listener,
            TcpListenerSetting::Backlog(ListenBacklog(u32::MIN)),
        );
        if let Ok(accepted) = sockets.accept(&listener, &mut []) {
            sockets.close(accepted.connection, TcpClose::Orderly);
        }
        let _ = sockets.read_listener_state(&listener, TcpListenerQuery::Information);
        let _ = sockets.set_listener_authentication_counters(&listener, counters);
        let _ = sockets.set_listener_timestamp_clock(&listener, clock);
        sockets.stop_listening(listener);
    }
    todo!("this is a placeholder: the real checks of TcpSockets are not written yet");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn call_tcp_sockets_methods() {
            $crate::call_tcp_sockets_methods($value);
        }
    };
}
