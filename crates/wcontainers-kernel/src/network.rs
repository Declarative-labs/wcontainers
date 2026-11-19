//! The made-up network the container sees (plan section 15): a loopback interface and one
//! Ethernet interface on a bridge like Docker's default one. A host writes the container's
//! `/etc/hosts` and `/etc/resolv.conf` from these, as Docker does.

/// The container's own address on its Ethernet interface, its prefix length and broadcast address.
pub const ADDRESS: [u8; 4] = [172, 17, 0, 2];
pub const PREFIX: u8 = 16;
pub const BROADCAST: [u8; 4] = [172, 17, 255, 255];
/// The bridge's address, the default route's gateway.
pub const GATEWAY: [u8; 4] = [172, 17, 0, 1];
/// The address the kernel answers name queries on (UDP port 53), as Docker's own name server.
pub const NAME_SERVER: [u8; 4] = [127, 0, 0, 11];
/// The Ethernet interface's hardware address and largest packet.
pub const HARDWARE: [u8; 6] = [0x02, 0x42, 0xac, 0x11, 0x00, 0x02];
pub const MTU: u32 = 65535;
/// The loopback interface's largest packet.
pub const LOOPBACK_MTU: u32 = 65536;
/// The interfaces' names and numbers.
pub const LOOPBACK_NAME: &str = "lo";
pub const LOOPBACK_INDEX: u32 = 1;
pub const ETHERNET_NAME: &str = "eth0";
pub const ETHERNET_INDEX: u32 = 2;

/// The interfaces by name and number.
const INTERFACES: [(&str, u32); 2] = [
    (LOOPBACK_NAME, LOOPBACK_INDEX),
    (ETHERNET_NAME, ETHERNET_INDEX),
];

/// The number of the interface called `name`.
pub fn index_of(name: &[u8]) -> Option<u32> {
    INTERFACES
        .iter()
        .find(|(known, _)| known.as_bytes() == name)
        .map(|(_, index)| *index)
}

/// The name of interface `index`.
pub fn name_of(index: u32) -> Option<&'static str> {
    INTERFACES
        .iter()
        .find(|(_, known)| *known == index)
        .map(|(name, _)| *name)
}
