#!/bin/bash
# Breaks the Linux host implementation in copies of the checkout, one way each, and runs the conformance suite
# on every copy: each must fail, and the checks that fail are printed. The copies live under the build folder.
# Arguments, if any, are the names of the mutants to run; without them every mutant runs.
# Run from the checkout on a Linux machine with sudo: crates/tcp-sockets/implementation/linux-host/mutants.sh
set -uo pipefail
checkout=$(cd "$(dirname "$0")/../../../.." && pwd)
implementation=crates/tcp-sockets/implementation/linux-host/implementation.rs
icmp_error=crates/internet-protocol/implementation/linux-socket-options/icmp_error.rs
mutants=${CARGO_TARGET_DIR:-$checkout/target}/mutants
names=() files=() from=() to=()
# A mutant: its name, the file it breaks, the text it replaces (once, exactly) and what it puts there.
mutant() { names+=("$1"); files+=("$2"); from+=("$3"); to+=("$4"); }
mutant more-to-come-ignored "$implementation" 'flags |= libc::MSG_MORE;' ';'
mutant abort-closes-in-order "$implementation" 'if manner == TcpClose::Abort {' 'if manner == TcpClose::Abort && false {'
mutant peek-offset-ignored "$implementation" 'offset = count(from.0)?;' 'offset = 0; let _ = from;'
mutant push-does-nothing "$implementation" 'let no_delay = sockopt::tcp_nodelay(socket).map_err(host_error)?;' \
    'return Ok(()); #[allow(unreachable_code)] let no_delay = sockopt::tcp_nodelay(socket).map_err(host_error)?;'
mutant icmp-errors-refused "$implementation" 'libc::SO_EE_ORIGIN_ICMP | libc::SO_EE_ORIGIN_ICMP6 => {' \
    'libc::SO_EE_ORIGIN_ICMP | libc::SO_EE_ORIGIN_ICMP6 if false => {'
mutant quoted-addresses-swapped "$icmp_error" 'let inner = ipv4_header(us, destination,' 'let inner = ipv4_header(destination, us,'
mutant report-packet-dropped "$implementation" 'report.packet_length = Some(ByteCount(message.length));' 'report.packet_length = None;'
mutant statistics-dropped "$implementation" '.then(|| transmit_statistics(data))' \
    '.then(|| { let _ = data; None::<TcpTransmitStatistics> })'
mutant advice-noop "$implementation" $'uapi::SO_CNX_ADVICE,\n            true,' $'uapi::SO_CNX_ADVICE,\n            false,'
mutant delayed-acknowledgement-noop "$implementation" 'let before = sockopt::tcp_quickack(socket).map_err(host_error)?;' \
    'let _ = afterwards; return Ok(()); #[allow(unreachable_code)] let before = sockopt::tcp_quickack(socket).map_err(host_error)?;'
mutant rearm-noop "$implementation" 'let idle = get_int(socket, uapi::SOL_TCP, uapi::TCP_KEEPIDLE)?;' \
    'return Ok(()); #[allow(unreachable_code)] let idle = get_int(socket, uapi::SOL_TCP, uapi::TCP_KEEPIDLE)?;'
mutant window-probe-noop "$implementation" 'let sharing = sockopt::socket_reuseaddr(socket).map_err(host_error)?;' \
    'return Ok(()); #[allow(unreachable_code)] let sharing = sockopt::socket_reuseaddr(socket).map_err(host_error)?;'
mutant window-probe-clears-sharing "$implementation" '} else if sharing {' '} else if sharing && false {'
mutant clock-always-refused "$implementation" 'let unit = u32::from(reading.unit == TcpTimestampUnit::Microseconds);' \
    'let _ = (socket, reading); return Err(HostError::Errno(libc::EPERM)); #[allow(unreachable_code)] let unit = u32::from(reading.unit == TcpTimestampUnit::Microseconds);'
mutant push-neither-noop "$implementation" 'let corked = sockopt::tcp_cork(socket).map_err(host_error)?;' \
    'return Ok(()); #[allow(unreachable_code)] let corked = sockopt::tcp_cork(socket).map_err(host_error)?;'
mutant record-end-ignored "$implementation" 'flags |= libc::MSG_EOR;' ';'
mutant fast-open-connect-ignored "$implementation" 'set_flag(socket, uapi::SOL_TCP, uapi::TCP_FASTOPEN_CONNECT, true)?;' ';'
mutant fast-open-data-ignored "$implementation" 'let flags = SendFlags::from_bits_retain(libc::MSG_FASTOPEN as u32)' \
    'let flags = SendFlags::from_bits_retain(0)'
mutant setting-Timestamping-noop "$implementation" '            TcpSetting::Ip(IpSocketSetting::Timestamping(timestamping)) => {' \
    'TcpSetting::Ip(IpSocketSetting::Timestamping(_)) => Ok(()), TcpSetting::Ip(IpSocketSetting::Timestamping(timestamping)) => {'
mutant exit-node-port-by-kernel-only "$implementation" 'options::bind_chosen_by_exit_node(socket, address, &choice, &self.shortfalls)' \
    'options::in_port_ranges(socket, Some(&choice), || rustix::net::bind(socket, &SocketAddr::new(address, 0)).map_err(host_error))'
# Each setting doing nothing, one at a time.
for pattern in 'Ip(IpSocketSetting::SendBuffer(..))' 'Ip(IpSocketSetting::ReceiveBuffer(..))' \
    'Ip(IpSocketSetting::MaximumPacingRate(..))' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::HopLimit { .. })' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::TrafficClass { .. })' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::PathMtuDiscovery { .. })' \
    'Packet(wcontainers_internet_protocol::IpPacketSetting::FlowLabel(..))' \
    'Port(wcontainers_internet_protocol::PortSetting::AddressSharing(..))' \
    'Port(wcontainers_internet_protocol::PortSetting::PortSharing(..))' \
    'Port(wcontainers_internet_protocol::PortSetting::NonlocalAddresses(..))' \
    'BufferLocks(..)' 'ReceiveLowWater(..)' 'Filter(..)' 'TransmitRehash(..)' 'MinimumHopLimit { .. }' \
    'IcmpErrorReports { .. }' 'KeepAlive(..)' 'KeepAliveIdle(..)' 'KeepAliveInterval(..)' 'KeepAliveProbes(..)' \
    'NoDelay(..)' 'Cork(..)' 'UrgentInline(..)' 'QuickAcknowledgement(..)' 'MaximumSegmentSize(..)' 'WindowClamp(..)' \
    'SynchronizeRetries(..)' 'UserTimeout(..)' 'DeferAccept(..)' 'FinWait2(..)' 'RetransmissionTimeoutMinimum(..)' \
    'RetransmissionTimeoutMaximum(..)' 'DelayedAcknowledgementMaximum(..)' 'TransmitDelay(..)' \
    'ThinLinearTimeouts(..)' 'CongestionControl(..)' 'NotSentLowWater(..)' 'FastOpenWithoutCookie(..)' \
    'UnreadByteReports(..)' 'Repair(..)'; do
    name=$(grep -oE '[A-Za-z0-9]+(\(\.\.\)| \{ \.\. \})' <<<"$pattern" | sed 's/[({ ].*//')
    mutant "setting-$name-noop" "$implementation" 'let refuse = |reason| Err(self.shortfalls.refuse(reason));' \
        "if matches!(*setting, TcpSetting::$pattern) { return Ok(()); } let refuse = |reason| Err(self.shortfalls.refuse(reason));"
done
for pattern in 'Backlog(..)' 'FastOpenQueue(..)' 'FastOpenKeys(..)' 'FlowLabelReflection(..)'; do
    mutant "listener-setting-${pattern%%(*}-noop" "$implementation" '            TcpListenerSetting::Socket(setting) => {' \
        "TcpListenerSetting::$pattern => Ok(()), TcpListenerSetting::Socket(setting) => {"
done
status=0
for index in "${!names[@]}"; do
    name=${names[$index]}
    if [ $# -gt 0 ] && ! printf '%s\n' "$@" | grep -qxF "$name"; then continue; fi
    copy=$mutants/$name
    rm -rf "$copy" && mkdir -p "$copy"
    rsync -a --exclude /target --exclude /.git "$checkout/" "$copy/"
    # The copies share a build folder, so every file any copy breaks is rebuilt in each.
    touch "$copy/$implementation" "$copy/$icmp_error"
    if ! python3 -c 'import sys
path, old, new = sys.argv[1:]
text = open(path).read()
if text.count(old) != 1:
    sys.exit(1)
open(path, "w").write(text.replace(old, new))' "$copy/${files[$index]}" "${from[$index]}" "${to[$index]}"; then
        echo "$name: the text to break is not there exactly once"; status=1; continue
    fi
    echo "== $name"
    output=$(CARGO_TARGET_DIR=$mutants/target "$copy/crates/tcp-sockets/implementation/linux-host/run-conformance.sh" 2>&1)
    grep -E '^test .* FAILED$|^test result|^error' <<<"$output"
    grep -q '^test result: FAILED' <<<"$output" || { echo "$name: the suite did not fail"; status=1; }
    rm -rf "$copy"
done
exit $status
