#!/bin/bash
# Breaks the Linux host implementation in copies of the checkout, one way each, and runs the conformance suite
# on every copy: each must fail, and the checks that fail are printed. The copies live under the build folder.
# Arguments, if any, are the names of the mutants to run; without them every mutant runs.
# Run from the checkout on a Linux machine with sudo: crates/tcp-sockets/implementation/host/mutants.sh
set -uo pipefail
checkout=$(cd "$(dirname "$0")/../../../.." && pwd)
implementation=crates/tcp-sockets/implementation/host/linux.rs
icmp_error=crates/internet-protocol/implementation/linux-socket-options/icmp_error.rs
readiness=crates/internet-protocol/implementation/linux-socket-options/readiness.rs
shared_linux=crates/internet-protocol/implementation/linux-socket-options/linux.rs
state=crates/tcp-sockets/implementation/host/linux/state.rs
mutants=${CARGO_TARGET_DIR:-$checkout/target}/mutants
names=() files=() from=() to=()
# A mutant: its name, the file it breaks, the text it replaces (once, exactly) and what it puts there.
mutant() { names+=("$1"); files+=("$2"); from+=("$3"); to+=("$4"); }
mutant more-to-come-ignored "$implementation" 'flags |= libc::MSG_MORE;' ';'
mutant abort-closes-in-order "$implementation" 'if manner == TcpClose::Abort {' 'if manner == TcpClose::Abort && false {'
mutant peek-offset-ignored "$implementation" 'offset = count(from.0)?;' 'offset = 0; let _ = from;'
mutant push-does-nothing "$implementation" 'let no_delay = socket.tcp_nodelay().map_err(io_host_error)?;' \
    'return Ok(()); #[allow(unreachable_code)] let no_delay = socket.tcp_nodelay().map_err(io_host_error)?;'
mutant icmp-errors-refused "$implementation" 'libc::SO_EE_ORIGIN_ICMP | libc::SO_EE_ORIGIN_ICMP6 => {' \
    'libc::SO_EE_ORIGIN_ICMP | libc::SO_EE_ORIGIN_ICMP6 if false => {'
mutant quoted-addresses-swapped "$icmp_error" '&ipv4_header(us, destination,' '&ipv4_header(destination, us,'
mutant report-packet-dropped "$implementation" 'report.packet_length = Some(ByteCount(message.length));' 'report.packet_length = None;'
mutant statistics-dropped "$implementation" '.then(|| transmit_statistics(data))' \
    '.then(|| { let _ = data; None::<TcpTransmitStatistics> })'
mutant advice-noop "$implementation" $'uapi::SO_CNX_ADVICE,\n            true,' $'uapi::SO_CNX_ADVICE,\n            false,'
mutant delayed-acknowledgement-noop "$implementation" 'let before = quick.tcp_quickack().map_err(io_host_error)?;' \
    'let _ = afterwards; return Ok(()); #[allow(unreachable_code)] let before = quick.tcp_quickack().map_err(io_host_error)?;'
mutant rearm-noop "$implementation" 'let idle = get_int(socket, uapi::SOL_TCP, uapi::TCP_KEEPIDLE)?;' \
    'return Ok(()); #[allow(unreachable_code)] let idle = get_int(socket, uapi::SOL_TCP, uapi::TCP_KEEPIDLE)?;'
mutant window-probe-noop "$implementation" 'let sharing = SockRef::from(&socket)' \
    'return Ok(()); #[allow(unreachable_code)] let sharing = SockRef::from(&socket)'
mutant window-probe-clears-sharing "$implementation" '} else if sharing {' '} else if sharing && false {'
mutant clock-always-refused "$implementation" 'let microseconds = reading.unit == TcpTimestampUnit::Microseconds;' \
    'let _ = (socket, reading); return Err(HostError::Errno(libc::EPERM)); #[allow(unreachable_code)] let microseconds = reading.unit == TcpTimestampUnit::Microseconds;'
mutant clock-unit-ignored "$implementation" 'let microseconds = reading.unit == TcpTimestampUnit::Microseconds;' 'let microseconds = false;'
mutant clock-microseconds-refused-on-a-millisecond-route "$implementation" 'let before: u32 = options::get_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP)?;' \
    'let route: uapi::tcp_info = options::get_option(socket, uapi::SOL_TCP, uapi::TCP_INFO)?; if u32::from(route.tcpi_options) & uapi::TCPI_OPT_USEC_TS == 0 { return Err(HostError::NotSupported); } let before: u32 = options::get_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP)?;'
mutant clock-milliseconds-refused-on-a-microsecond-route "$implementation" 'return set_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &value);' \
    'let route: uapi::tcp_info = options::get_option(socket, uapi::SOL_TCP, uapi::TCP_INFO)?; if u32::from(route.tcpi_options) & uapi::TCPI_OPT_USEC_TS != 0 { return Err(HostError::NotSupported); } return set_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &value);'
mutant clock-microseconds-as-milliseconds "$implementation" 'let value = reading.value.0 & !TIMESTAMP_IN_MICROSECONDS | unit;' \
    'let value = reading.value.0 & !TIMESTAMP_IN_MICROSECONDS | unit; if microseconds { return Ok(()); }'
mutant multipath-is-tcp "$implementation" 'TcpProtocol::Multipath => ipproto::MPTCP,' 'TcpProtocol::Multipath => ipproto::TCP,'
mutant tcp-is-multipath "$implementation" 'TcpProtocol::Tcp => ipproto::TCP,' 'TcpProtocol::Tcp => ipproto::MPTCP,'
mutant shutdown-both-is-write "$implementation" 'TcpShutdown::ReadAndWrite => std::net::Shutdown::Both,' \
    'TcpShutdown::ReadAndWrite => std::net::Shutdown::Write,'
mutant multipath-save-syn-refusal-skipped "$implementation" 'set_int(socket, uapi::SOL_TCP, uapi::TCP_SAVE_SYN, saved)?;' \
    'let _ = set_int(socket, uapi::SOL_TCP, uapi::TCP_SAVE_SYN, saved);'
mutant link-header-kept-as-network "$implementation" 'SAVE_FROM_NETWORK_HEADER => Some(network),' \
    'SAVE_FROM_NETWORK_HEADER | SAVE_FROM_LINK_HEADER => Some(network),'
mutant link-header-not-saved "$implementation" 'TcpSavedSynchronize::FromLinkHeader => SAVE_FROM_LINK_HEADER,' \
    'TcpSavedSynchronize::FromLinkHeader => SAVE_FROM_NETWORK_HEADER,'
mutant link-header-given-by-accept "$implementation" '            &saved[start..]' '            { let _ = start; saved }'
mutant records-refused "$implementation" 'let on = report == Report::Requested;
                for &option in IPV4_RECORDS {' 'let on = report == Report::Requested; if on { return Err(HostError::NotSupported); }
                for &option in IPV4_RECORDS {'
mutant receive-unlocked "$implementation" 'let _receiving = locked(&connection.receiving);' 'let _receiving = ();'
mutant window-probe-repair-queue-lost "$implementation" 'set_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE, queue)?;' 'let _ = queue;'
mutant push-leaves-no-delay-on "$implementation" 'if !no_delay {' 'if false {'
mutant md5-key-ignored "$implementation" 'TcpSetting::Md5Key(change) => set_md5_key(socket, family, change),' 'TcpSetting::Md5Key(_) => Ok(()),'
mutant md5-absent-removal-refused "$implementation" 'Err(HostError::Errno(libc::ENOENT)) if key.is_empty() => Ok(()),' ''
mutant fixed-flow-label-not-held "$implementation" 'hold_flow_label(socket, peer, label)?;' 'let _ = (peer, label);'
mutant fixed-flow-label-forgotten "$implementation" 'let _ = connection.fixed_flow_label.set(label);' ';'
mutant multicast-refused "$implementation" 'TcpSetting::Ip(IpSocketSetting::MulticastV4(ref group)) => set_multicast_group(' \
    'TcpSetting::Ip(IpSocketSetting::MulticastV4(_)) => Err(HostError::NotSupported), TcpSetting::Ip(IpSocketSetting::MulticastV4(ref group)) => set_multicast_group('
mutant urgent-wait-unregistered "$readiness" 'event.set_priority(interest.contains(ReadyFor::URGENT_DATA));' 'event.set_priority(false);'
mutant peer-shutdown-unregistered "$readiness" '(ReadyFor::PEER_FINISHED_SENDING, PollFlags::RDHUP),' '(ReadyFor::empty(), PollFlags::RDHUP),'
mutant discard-counts-cuts "$implementation" 'match self.next_error_queue_entry(connection, &mut []) {' \
    'match self.take_error_queue_entry(connection, &mut []).map(|entry| (entry, false)) {'
mutant push-neither-noop "$implementation" 'let corked = socket.tcp_cork().map_err(io_host_error)?;' \
    'return Ok(()); #[allow(unreachable_code)] let corked = socket.tcp_cork().map_err(io_host_error)?;'
mutant record-end-ignored "$implementation" 'flags |= libc::MSG_EOR;' ';'
mutant fast-open-connect-ignored "$implementation" 'set_flag(socket, uapi::SOL_TCP, uapi::TCP_FASTOPEN_CONNECT, true)?;' ';'
mutant fast-open-data-ignored "$implementation" 'let flags = libc::MSG_FASTOPEN | libc::MSG_NOSIGNAL' \
    'let flags = libc::MSG_NOSIGNAL'
mutant setting-Timestamping-noop "$implementation" '            TcpSetting::Ip(IpSocketSetting::Timestamping(timestamping)) => {' \
    'TcpSetting::Ip(IpSocketSetting::Timestamping(_)) => Ok(()), TcpSetting::Ip(IpSocketSetting::Timestamping(timestamping)) => {'
mutant exit-node-port-by-kernel-only "$implementation" 'options::bind_chosen_by_exit_node(socket, address, &choice, &self.shortfalls)' \
    'options::in_port_ranges(socket, Some(&choice), || rustix::net::bind(socket, &SocketAddr::new(address, 0)).map_err(host_error))'
mutant accept-interface-none "$implementation" 'self.shortfalls.count(SYNCHRONIZE_FIELDS_OF_THE_SOCKET);' \
    'self.shortfalls.count(SYNCHRONIZE_FIELDS_OF_THE_SOCKET); let interface = { let _ = interface; NO_INTERFACE };'
mutant accept-interface-exact-address-only "$implementation" '.or_else(|| table.iter().find(|held| held.holds(address)))' ''
mutant accept-no-specific-destination "$implementation" 'IpAddr::V4(address) => Some(address),' 'IpAddr::V4(_) => None,'
mutant accept-mark-zero "$implementation" 'mark: PacketMark(options::get_option(' 'mark: PacketMark(u32::MIN & options::get_option::<u32>('
mutant connect-repair-as-synchronize-sent "$implementation" 'Some(_) => TcpConnectStart::EstablishedForRepair,' \
    'Some(_) => TcpConnectStart::SynchronizeSent { fast_open_data_taken: ByteCount(0) },'
mutant stats-smoothed-is-minimum "$implementation" 'smoothed_round_trip_time: microseconds(uapi::TCP_NLA_SRTT as usize)?,' \
    'smoothed_round_trip_time: microseconds(uapi::TCP_NLA_MIN_RTT as usize)?,'
mutant stats-busy-is-receive-window-limited "$implementation" 'busy: microseconds(uapi::TCP_NLA_BUSY as usize)?,' \
    'busy: microseconds(uapi::TCP_NLA_RWND_LIMITED as usize)?,'
mutant stats-hop-limit-of-another "$implementation" 'at(uapi::TCP_NLA_TTL as usize)' 'at(uapi::TCP_NLA_REHASH as usize)'
mutant stats-rate-limit-swapped "$implementation" 'TCP_NLA_DELIVERY_RATE_APP_LMT as usize)? == 0 {' \
    'TCP_NLA_DELIVERY_RATE_APP_LMT as usize)? != 0 {'
mutant delayed-ack-mode-even-unscheduled "$implementation" 'let wanted = if scheduled {' 'let wanted = if scheduled || true {'
mutant accept-drops-listener-timestamping "$implementation" 'listener.receive_timestamping.load(Ordering::Relaxed),' '0,'
mutant saved-synchronize-not-kept "$implementation" \
    'if let Some(kept) = kept.filter(|kept| !kept.is_empty()) {' 'if let Some(kept) = kept.filter(|_| false) {'
mutant saved-synchronize-off-ignored "$implementation" 'listener.saves_synchronize.store(value, Ordering::Relaxed);' \
    'listener.saves_synchronize.store(value.max(listener.saves_synchronize.load(Ordering::Relaxed)), Ordering::Relaxed);'
mutant saved-synchronize-taken-twice "$implementation" '*saved = None;' ';'
mutant wait-finishes-at-once "$implementation" 'connection.socket.wait_until_ready(interest).await' \
    'Ok(ReadinessReport { ready_for: interest, unconditional: wcontainers_readiness::UnconditionalEvents::empty() })'
mutant wait-never-finishes "$implementation" 'connection.socket.wait_until_ready(interest).await' 'std::future::pending().await'
mutant listener-wait-finishes-at-once "$implementation" 'listener.socket.wait_until_ready(interest).await' \
    'Ok(ReadinessReport { ready_for: interest, unconditional: wcontainers_readiness::UnconditionalEvents::empty() })'
mutant listener-wait-never-finishes "$implementation" 'listener.socket.wait_until_ready(interest).await' \
    'std::future::pending().await'
mutant readiness-withdraw-noop "$readiness" '    fn withdraw(&mut self) {' '    fn withdraw(&mut self) { if self.id.is_some() { return; }'
mutant readiness-wakes-nothing "$readiness" '        waker.wake();' '        drop(waker);'
mutant unconditional-events-end-no-wait "$readiness" '    report.ready_for.intersects(interest) || !report.unconditional.is_empty()' '    report.ready_for.intersects(interest)'
mutant hang-up-unreported "$readiness" '(UnconditionalEvents::HANG_UP, PollFlags::HUP),' '(UnconditionalEvents::empty(), PollFlags::HUP),'
mutant error-unreported "$readiness" '(UnconditionalEvents::ERROR, PollFlags::ERR),' '(UnconditionalEvents::empty(), PollFlags::ERR),'
mutant plain-sends-uncounted-once-armed "$implementation" '&& !connection.reports_armed.load(Ordering::Relaxed)' ''
# Each setting doing nothing, one at a time.
mutant multicast-noop "$implementation" '            TcpSetting::Ip(IpSocketSetting::MulticastV4(ref group)) => set_multicast_group(' \
    '            TcpSetting::Ip(IpSocketSetting::MulticastV4(_) | IpSocketSetting::MulticastV6(_)) => Ok(()), TcpSetting::Ip(IpSocketSetting::MulticastV4(ref group)) => set_multicast_group('
mutant anycast-noop "$implementation" 'locked(anycast).set(socket, address)' '{ let _ = (anycast, address); Ok(()) }'
mutant owner-ignored "$shared_linux" 'if previous == owner.0 {' 'if true {'
mutant listener-unread-reports-kept "$implementation" $'connection.unread_reports.store(\n            listener.unread_reports.load(Ordering::Relaxed),' \
    $'connection.unread_reports.store(\n            false,'
mutant keep-alive-probes-always-one "$implementation" 'tcp(uapi::TCP_KEEPCNT, c_int::from(probes.0))' 'tcp(uapi::TCP_KEEPCNT, { let _ = probes; 1 })'
mutant authentication-mac-length-ignored "$implementation" 'request.maclen = addition.mac_length.0;' 'request.maclen = 0;'
mutant authentication-coverage-ignored "$implementation" 'if addition.other_options == TcpAuthenticationOptionCoverage::Excluded {' 'if false {'
mutant authentication-key-use-ignored "$implementation" 'request.set_set_current(u32::from(current));' 'request.set_set_current(0);'
mutant authentication-held-key-use-ignored "$implementation" 'if current || next {' 'if false {'
mutant authentication-held-key-refused "$implementation" 'Err(HostError::Errno(libc::EEXIST)) if holds_authentication_key(socket, &request) => {' \
    'Err(HostError::Errno(libc::EEXIST)) if false && holds_authentication_key(socket, &request) => {'
mutant authentication-removal-current-ignored "$implementation" $'        request.set_set_current(u32::from(true));\n        request.current_key = key.0;\n    }\n    if let Some(key) = removal.requested_next {' \
    $'        let _ = key;\n    }\n    if let Some(key) = removal.requested_next {'
mutant authentication-absent-removal-refused "$implementation" 'if removal.current.is_none() && removal.requested_next.is_none() {' \
    'if removal.current.is_none() && removal.requested_next.is_none() { return Err(HostError::Errno(libc::ENOENT)); }  if false {'
mutant authentication-options-current-ignored "$implementation" $'        if let Some(key) = choice.current {\n            options.set_set_current(u32::from(true));' \
    $'        if let Some(key) = choice.current.filter(|_| false) {\n            options.set_set_current(u32::from(true));'
mutant authentication-requirement-ignored "$implementation" 'choice.requirement == TcpAuthenticationRequirement::Required,' 'false,'
for pattern in 'Ip(IpSocketSetting::SendBuffer(..))' 'Ip(IpSocketSetting::ReceiveBuffer(..))' \
    'Ip(IpSocketSetting::MaximumPacingRate(..))' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::HopLimit { .. })' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::TrafficClass { .. })' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::PathMtuDiscovery { .. })' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::FlowLabel(..))' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::Ipv4Options(..))' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::HopByHopOptions(..))' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::DestinationOptions(..))' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::Mark(..))' \
    'Port(wcontainers_internet_protocol::PortSetting::AddressSharing(..))' \
    'Port(wcontainers_internet_protocol::PortSetting::PortSharing(..))' \
    'Port(wcontainers_internet_protocol::PortSetting::NonlocalAddresses(..))' \
    'BufferLocks(..)' 'ReceiveLowWater(..)' 'Filter(..)' 'TransmitRehash(..)' 'MinimumHopLimit { .. }' \
    'IcmpErrorReports { .. }' 'KeepAlive(..)' 'KeepAliveIdle(..)' 'KeepAliveInterval(..)' 'KeepAliveProbes(..)' \
    'NoDelay(..)' 'Cork(..)' 'UrgentInline(..)' 'QuickAcknowledgement(..)' 'MaximumSegmentSize(..)' 'WindowClamp(..)' \
    'SynchronizeRetries(..)' 'UserTimeout(..)' 'DeferAccept(..)' 'FinWait2(..)' 'RetransmissionTimeoutMinimum(..)' \
    'RetransmissionTimeoutMaximum(..)' 'DelayedAcknowledgementMaximum(..)' 'TransmitDelay(..)' \
    'ThinLinearTimeouts(..)' 'CongestionControl(..)' 'NotSentLowWater(..)' 'FastOpenWithoutCookie(..)' \
    'UnreadByteReports(..)' 'Repair(..)' 'AddAuthenticationKey(..)' 'RemoveAuthenticationKey(..)' \
    'AuthenticationOptions(..)'; do
    name=$(grep -oE '[A-Za-z0-9]+(\(\.\.\)| \{ \.\. \})' <<<"$pattern" | sed 's/[({ ].*//')
    mutant "setting-$name-noop" "$implementation" 'let refuse = |reason| Err(self.shortfalls.refuse(reason));' \
        "if matches!(*setting, TcpSetting::$pattern) { return Ok(()); } let refuse = |reason| Err(self.shortfalls.refuse(reason));"
done
for pattern in 'Backlog(..)' 'FastOpenQueue(..)' 'FastOpenKeys(..)' 'FlowLabelReflection(..)'; do
    mutant "listener-setting-${pattern%%(*}-noop" "$implementation" '            TcpListenerSetting::Socket(setting) => self.configure_socket(' \
        "TcpListenerSetting::$pattern => Ok(()), TcpListenerSetting::Socket(setting) => self.configure_socket("
done
mutant listen-backlog-argument-ignored "$implementation" $'self.bind(socket, &local)?;\n        SockRef::from(&socket)\n            .listen(i32::try_from(backlog.0).unwrap_or(i32::MAX))' \
    $'self.bind(socket, &local)?; let _ = backlog;\n        SockRef::from(&socket)\n            .listen(4096)'
mutant clock-set-outside-repair "$implementation" 'return set_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &value);' \
    'return set_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &value).or(Ok(()));'
mutant synchronize-frame-is-the-link-header "$implementation" '.and_then(|link| u32::try_from(link + length).ok())' \
    '.and_then(|link| u32::try_from(link).ok())'
mutant information-state-shifted "$state" '.checked_sub(FIRST_STATE)' '.checked_add(u8::MIN)'
mutant information-acknowledged-as-received "$state" 'bytes_acknowledged: ByteTotal(info.tcpi_bytes_acked),' 'bytes_acknowledged: ByteTotal(info.tcpi_bytes_received),'
mutant congestion-information-none "$state" '(_, usize::MIN) => None,' '(_, _) if true => None,'
mutant congestion-name-cut-short "$state" 'let cut = length.min(into.len());' 'let cut = length.min(into.len()).saturating_sub(1);'
mutant queues-unsent-unacknowledged "$state" 'unacknowledged: ByteCount(queued.saturating_sub(unsent)),' 'unacknowledged: ByteCount(queued),'
mutant urgent-mark-never-reached "$state" 'Ok(_) => UrgentMark::Reached,' 'Ok(_) => UrgentMark::NotReached,'
mutant multipath-queues-refused "$state" 'Err(_) if is_multipath(socket) => {' 'Err(_) if false => {'
mutant multipath-tuned-passes-the-errno "$state" $'    if is_multipath(socket) {\n        shortfalls.refuse(NO_MULTIPATH_CLAMP)' $'    if false {\n        shortfalls.refuse(NO_MULTIPATH_CLAMP)'
mutant tuned-clamp-is-the-segment "$state" 'uapi::SOL_TCP, uapi::TCP_WINDOW_CLAMP)' 'uapi::SOL_TCP, uapi::TCP_MAXSEG)'
mutant timestamp-unit-milliseconds "$state" 'unit: if u32::from(information.tcpi_options) & uapi::TCPI_OPT_USEC_TS != u32::MIN {' 'unit: if false {'
mutant repair-sequences-one-queue "$state" 'let sequences = sequence_of(uapi::TCP_SEND_QUEUE as c_int)' 'let sequences = sequence_of(uapi::TCP_RECV_QUEUE as c_int)'
mutant repair-options-noop "$implementation" 'TcpSetting::RepairOptions(repair) => state::set_repair_options(socket, repair),' 'TcpSetting::RepairOptions(repair) => { let _ = repair; Ok(()) }'
mutant repair-window-noop "$implementation" 'TcpSetting::RepairWindow(ref window) => state::set_repair_window(socket, window),' 'TcpSetting::RepairWindow(ref window) => { let _ = window; Ok(()) }'
mutant authentication-repair-noop "$implementation" 'state::set_authentication_repair(socket, repair)' '{ let _ = repair; Ok(()) }'
mutant authentication-repair-host-order "$state" 'sent_initial_sequence: TcpSequenceNumber(u32::from_be(found.snt_isn)),' 'sent_initial_sequence: TcpSequenceNumber(found.snt_isn),'
mutant authentication-counters-noop "$implementation" 'set_authentication_counters(connection.socket.fd(), &values)' '{ let _ = (connection, values); Ok(()) }'
mutant listener-authentication-counters-noop "$implementation" 'set_authentication_counters(listener.socket.fd(), &values)' '{ let _ = (listener, values); Ok(()) }'
mutant authentication-requirement-inverted "$state" 'requirement: if found.ao_required() == u32::MIN {' 'requirement: if found.ao_required() != u32::MIN {'
mutant authentication-key-never-held "$state" 'Ok(_) if asked.nkeys == ONE_KEY =>' 'Ok(_) if false =>'
mutant multipath-sent-as-received "$state" 'bytes_sent: ByteTotal(found.bytes_sent),' 'bytes_sent: ByteTotal(found.bytes_received),'
mutant subflow-addresses-swapped "$state" 'local: address(subflow.local)?,' 'local: address(subflow.remote)?,'
mutant last-header-ipv4-unread "$state" $'uapi::IP_PKTOPTIONS,\n                &mut control.bytes,' $'uapi::IP_PKTOPTIONS,\n                &mut [],'
mutant last-header-bytes-dropped "$state" 'header: &into[..written],' 'header: &into[..usize::MIN],'
mutant last-header-room-of-the-packet-information "$state" $'uapi::IPV6_2292PKTOPTIONS,\n            &mut control.bytes,' \
    $'uapi::IPV6_2292PKTOPTIONS,\n            &mut control.bytes[..super::PACKET_OPTIONS_ROOM],'
mutant listener-state-refused "$implementation" 'self.read_socket_state(listener.socket.fd(), listener.family, query)' '{ let _ = (listener, query); Err(HostError::NotSupported) }'
mutant incoming-processor-none "$state" 'NO_PROCESSOR => Ok(None),' 'processor if processor >= NO_PROCESSOR => Ok(None),'
mutant incoming-processor-always-first "$state" '.map(|index| Some(ProcessorIndex(index)))' '.map(|_index| Some(ProcessorIndex(u32::MIN)))'
mutant tuned-path-mtu-fixed "$state" 'path_mtu: MaximumTransmissionUnit(information.tcpi_pmtu),' 'path_mtu: MaximumTransmissionUnit(1500),'
mutant tuned-receive-buffer-is-send "$state" 'receive_buffer: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_RCVBUF)?)?),' \
    'receive_buffer: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_SNDBUF)?)?),'
mutant information-rtt-is-variance "$state" 'round_trip_time: microseconds(info.tcpi_rtt),' 'round_trip_time: microseconds(info.tcpi_rttvar),'
mutant bbr-bandwidth-halves-swapped "$state" 'u64::from(found.bandwidth_high) << u32::BITS | u64::from(found.bandwidth_low),' \
    'u64::from(found.bandwidth_low) << u32::BITS | u64::from(found.bandwidth_high),'
mutant multipath-token-zero "$state" 'token: MultipathToken(found.token),' 'token: MultipathToken(0),'
mutant subflow-information-zeroed "$state" 'information: information_of(information, shortfalls)?,' \
    'information: information_of(&{ let mut zeroed = *information; zeroed.bytes[8..].fill(0); zeroed }, shortfalls)?,'
mutant repair-window-read-swapped "$state" $'send_window: WindowSize(window.snd_wnd),\n            maximum_window: WindowSize(window.max_window),\n            receive_window: WindowSize(window.rcv_wnd),' \
    $'send_window: WindowSize(window.rcv_wnd),\n            maximum_window: WindowSize(window.max_window),\n            receive_window: WindowSize(window.snd_wnd),'
mutant ic-send-buffer-is-receive "$state" 'send_buffer: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_SNDBUF)?)?),' 'send_buffer: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_RCVBUF)?)?),'
mutant ic-rto-ato-swapped "$state" $'retransmission_timeout: microseconds(info.tcpi_rto),\n        acknowledgement_timeout: microseconds(info.tcpi_ato),' $'retransmission_timeout: microseconds(info.tcpi_ato),\n        acknowledgement_timeout: microseconds(info.tcpi_rto),'
mutant ic-info-windows-swapped "$state" $'send_window: WindowSize(info.tcpi_snd_wnd),\n        receive_window: WindowSize(info.tcpi_rcv_wnd),' $'send_window: WindowSize(info.tcpi_rcv_wnd),\n        receive_window: WindowSize(info.tcpi_snd_wnd),'
mutant ic-multipath-last-swapped "$state" $'since_last_data_sent: milliseconds(found.last_data_sent),\n        since_last_data_received: milliseconds(found.last_data_recv),' $'since_last_data_sent: milliseconds(found.last_data_recv),\n        since_last_data_received: milliseconds(found.last_data_sent),'
mutant ic-subflow-identifier-zero "$state" 'identifier: MultipathSubflowIdentifier(subflow.id),' 'identifier: MultipathSubflowIdentifier(u32::MIN),'
mutant ic-segments-in-is-out "$state" 'segments_in: segments(info.tcpi_segs_in),' 'segments_in: segments(info.tcpi_segs_out),'
mutant ic-authentication-icmp-inverted "$state" 'icmp_errors: if found.accept_icmps() == u32::MIN {' 'icmp_errors: if found.accept_icmps() != u32::MIN {'
mutant ic-repair-timestamps-as-sack "$state" 'TcpRepairOption::Timestamps => (KIND_TIMESTAMP, NO_OPTION_VALUE),' 'TcpRepairOption::Timestamps => (KIND_SELECTIVE_ACK_PERMITTED, NO_OPTION_VALUE),'
mutant ic-repair-max-window-is-send "$state" 'max_window: window.maximum_window.0,' 'max_window: window.send_window.0,'
mutant ic-multipath-checksums-on "$state" 'checksums: found.csum_enabled != u8::MIN,' 'checksums: true,'
status=0
for index in "${!names[@]}"; do
    name=${names[$index]}
    if [ $# -gt 0 ] && ! printf '%s\n' "$@" | grep -qxF "$name"; then continue; fi
    copy=$mutants/$name
    rm -rf "$copy" && mkdir -p "$copy"
    rsync -a --exclude /target --exclude /.git "$checkout/" "$copy/"
    # The copies share a build folder, so every file any copy breaks is rebuilt in each.
    touch "$copy/$implementation" "$copy/$state" "$copy/$icmp_error" "$copy/$readiness" "$copy/$shared_linux"
    if ! python3 -c 'import sys
path, old, new = sys.argv[1:]
text = open(path).read()
if text.count(old) != 1:
    sys.exit(1)
open(path, "w").write(text.replace(old, new))' "$copy/${files[$index]}" "${from[$index]}" "${to[$index]}"; then
        echo "$name: the text to break is not there exactly once"; status=1; continue
    fi
    echo "== $name"
    output=$(CARGO_TARGET_DIR=$mutants/target "$copy/crates/tcp-sockets/implementation/host/run-conformance.sh" 2>&1)
    grep -E '^test .* FAILED$|^test result|^error' <<<"$output"
    grep -q '^test result: FAILED' <<<"$output" || { echo "$name: the suite did not fail"; status=1; }
    rm -rf "$copy"
done
exit $status
