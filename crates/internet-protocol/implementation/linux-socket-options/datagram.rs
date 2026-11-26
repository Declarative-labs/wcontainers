//! The datagram half of the sockets of crates/internet-protocol, from send fields to received packets and memberships.
//!
//! Header and send fields go onto options and control messages; received control messages become packet fields,
//! reports and whole packets.

pub mod addresses;
pub mod exchange;
pub mod memberships;
pub mod receive;
pub mod send;
pub mod sticky;
