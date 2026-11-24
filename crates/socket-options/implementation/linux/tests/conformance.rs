//! The suite of wcontainers-socket-options against Linux's own calls, run as root.
#![cfg(target_os = "linux")]

use wcontainers_socket_options_conformance as suite;
use wcontainers_socket_options_linux::LinuxSocketOptions;

#[test]
fn every_option_is_known_at_its_level() {
    suite::every_option_is_known_at_its_level(LinuxSocketOptions);
}

#[test]
fn an_option_reads_back_what_was_set() {
    suite::an_option_reads_back_what_was_set(LinuxSocketOptions);
}

#[test]
fn another_length_is_refused() {
    suite::another_length_is_refused(LinuxSocketOptions);
}

#[test]
fn a_quiet_connection_has_nothing_queued() {
    suite::a_quiet_connection_has_nothing_queued(LinuxSocketOptions);
}

#[test]
fn written_bytes_are_counted() {
    suite::written_bytes_are_counted(LinuxSocketOptions);
}

#[test]
fn a_multipath_connection_has_one_subflow() {
    suite::a_multipath_connection_has_one_subflow(LinuxSocketOptions);
}
