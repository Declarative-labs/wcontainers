//! The character devices of a container's `/dev`: what reading and writing each one does.

use super::decode_device;

/// A device the kernel has a driver for, found by its device number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Device {
    Null,
    Zero,
    Full,
    Random,
    Urandom,
    /// The calling process's controlling terminal.
    Tty,
    /// The pseudo-terminal multiplexer.
    Ptmx,
    /// A pseudo-terminal slave.
    Pts,
}

/// The devices a default Docker container has in `/dev`: name, major and minor, in the order runc
/// makes them (its default device list).
pub(crate) const DOCKER_DEVICES: &[(&[u8], u32, u32)] = &[
    (b"null", 1, 3),
    (b"random", 1, 8),
    (b"full", 1, 7),
    (b"tty", 5, 0),
    (b"zero", 1, 5),
    (b"urandom", 1, 9),
];

/// The character devices Docker's device rules let a container open besides those built here: the
/// console and the network tunnel (`/dev/net/tun`). Every other device, and every block device, is
/// EPERM.
pub(crate) const ALLOWED_NOT_BUILT: &[(u32, u32)] = &[(5, 1), (10, 200)];

impl Device {
    pub fn from_number(rdev: u64) -> Option<Self> {
        Some(match decode_device(rdev) {
            (1, 3) => Device::Null,
            (1, 5) => Device::Zero,
            (1, 7) => Device::Full,
            (1, 8) => Device::Random,
            (1, 9) => Device::Urandom,
            (5, 0) => Device::Tty,
            (5, 2) => Device::Ptmx,
            (crate::linux_extra::UNIX98_PTY_SLAVE_MAJOR, _) => Device::Pts,
            _ => return None,
        })
    }
}
