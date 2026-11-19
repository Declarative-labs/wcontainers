//! The container's mount table, as `/proc/<pid>/mounts` and `/proc/<pid>/mountinfo` show it.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use super::{FileSystem, decode_device};
use crate::linux_extra::{ST_NOATIME, ST_NODEV, ST_NOEXEC, ST_NOSUID, ST_RDONLY, ST_RELATIME};

/// One mount, in the order the container's mounts were made.
pub(crate) struct Mount {
    pub fs: FileSystem,
    /// The directory of the file system that is mounted (`/` but for a bind mount of part of one).
    pub root: &'static str,
    pub point: Vec<u8>,
    /// `mount`'s source and file system type.
    pub source: &'static str,
    pub kind: &'static str,
    /// What the file system adds to its options (its `show_options`).
    pub options: &'static str,
    /// Whether the file system itself is read-only (`SB_RDONLY`, as sysfs mounted in a container
    /// without CAP_SYS_ADMIN), not only the mount.
    pub read_only_super: bool,
}

/// The number the first mount of the container's namespace takes; the one it is mounted on is the
/// machine's, outside the namespace.
const FIRST_ID: u32 = 22;

/// The mount options `show_mnt_opts` shows, after `rw` or `ro`.
fn mount_options(out: &mut String, flags: u32) {
    out.push_str(if flags & ST_RDONLY != 0 { "ro" } else { "rw" });
    for (bit, name) in [
        (ST_NOSUID, ",nosuid"),
        (ST_NODEV, ",nodev"),
        (ST_NOEXEC, ",noexec"),
        (ST_NOATIME, ",noatime"),
        (ST_RELATIME, ",relatime"),
    ] {
        if flags & bit != 0 {
            out.push_str(name);
        }
    }
}

/// `/proc/<pid>/mounts` (`show_vfsmnt`): source, point, type, then the mount's options and the file
/// system's.
pub(crate) fn mounts(table: &[Mount]) -> String {
    let mut out = String::new();
    for mount in table {
        let _ = write!(
            out,
            "{} {} {} ",
            mount.source,
            String::from_utf8_lossy(&mount.point),
            mount.kind
        );
        mount_options(&mut out, mount.fs.flags);
        if !mount.options.is_empty() {
            out.push(',');
            out.push_str(mount.options);
        }
        out.push_str(" 0 0\n");
    }
    out
}

/// `/proc/<pid>/mountinfo` (`show_mountinfo`): each mount's number and its parent's, the device,
/// the root and point, the mount's options, no propagation (every mount is private), then the type,
/// source and the file system's options. A mount's parent is the latest earlier one whose point
/// holds its point.
pub(crate) fn mountinfo(table: &[Mount]) -> String {
    let mut out = String::new();
    for (index, mount) in table.iter().enumerate() {
        let parent = table[..index]
            .iter()
            .rposition(|other| holds(&other.point, &mount.point))
            .map_or(FIRST_ID - 1, |parent| FIRST_ID + parent as u32);
        let (major, minor) = decode_device(mount.fs.device);
        let _ = write!(
            out,
            "{} {parent} {major}:{minor} {} {} ",
            FIRST_ID + index as u32,
            mount.root,
            String::from_utf8_lossy(&mount.point)
        );
        mount_options(&mut out, mount.fs.flags);
        let super_flags = if mount.read_only_super { "ro" } else { "rw" };
        let _ = write!(out, " - {} {} {super_flags}", mount.kind, mount.source);
        if !mount.options.is_empty() {
            out.push(',');
            out.push_str(mount.options);
        }
        out.push('\n');
    }
    out
}

/// Whether the directory `outer` holds the path `inner` (a mount is not its own parent).
fn holds(outer: &[u8], inner: &[u8]) -> bool {
    outer != inner
        && (outer == b"/"
            || inner
                .strip_prefix(outer)
                .is_some_and(|rest| rest.first() == Some(&b'/')))
}
