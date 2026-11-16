//! `name_to_handle_at`: a handle naming a file, as Linux's `do_sys_name_to_handle` and the file
//! systems' encoders make it. Docker's profile refuses `open_by_handle_at`, so a handle is only ever
//! compared or kept.

use alloc::vec::Vec;
use core::hash::Hasher;

use siphasher::sip::SipHasher13;

use super::{Args, guest};
use crate::fs::{Inode, Kind};
use crate::linux::general::*;
use crate::linux_extra::{
    FILEID_INO32_GEN, FILEID_INO64_GEN, FILEID_INVALID, FILEID_KERNFS, MAX_HANDLE_SZ,
    MNT_UNIQUE_ID_OFFSET,
};
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// The number statx reports for the mount a file is on.
pub(super) fn mount_id(inode: &Inode) -> u64 {
    (inode.fs.device & 0xff) + 1
}

/// The mount's number that is never used again (Linux's `mnt_id_unique`, counted from
/// `MNT_UNIQUE_ID_OFFSET` as mounts are made; each mount here is made once).
pub(super) fn unique_mount_id(inode: &Inode) -> u64 {
    MNT_UNIQUE_ID_OFFSET + mount_id(inode)
}

/// The words of a file's handle and their type: tmpfs's own encoding (generation, then the inode
/// number), pidfs's (the inode number, `FILEID_KERNFS`), or, only when asked for an identifier, the
/// generic one other file systems get (the inode number, then a generation of 0).
fn encode(kernel: &Kernel, inode: &Inode, identifier: bool) -> Result<(i32, Vec<u32>), Errno> {
    let ino = [inode.ino as u32, (inode.ino >> 32) as u32];
    match inode.fs.kind {
        Kind::Tmpfs => Ok((
            FILEID_INO32_GEN,
            alloc::vec![generation(kernel, inode.ino), ino[0], ino[1]],
        )),
        Kind::Pidfs => Ok((FILEID_KERNFS, ino.to_vec())),
        Kind::Proc | Kind::Devpts | Kind::Pipe | Kind::Secretmem if identifier => {
            Ok((FILEID_INO64_GEN, alloc::vec![ino[0], ino[1], 0]))
        }
        Kind::Proc | Kind::Devpts | Kind::Pipe | Kind::Secretmem => Err(Errno::EOPNOTSUPP),
    }
}

/// A tmpfs inode's generation, which Linux draws at random when it makes the inode: here a keyed hash
/// of its number, which is never reused.
fn generation(kernel: &Kernel, ino: u64) -> u32 {
    let mut hasher = SipHasher13::new_with_key(&kernel.files.generation_key);
    hasher.write_u64(ino);
    hasher.finish() as u32
}

pub(super) fn name_to_handle_at(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [dirfd, path, handle, mount, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags
        & !(AT_SYMLINK_FOLLOW
            | AT_EMPTY_PATH
            | AT_HANDLE_FID
            | AT_HANDLE_MNT_ID_UNIQUE
            | AT_HANDLE_CONNECTABLE)
        != 0
        || flags & AT_HANDLE_CONNECTABLE != 0 && flags & (AT_HANDLE_FID | AT_EMPTY_PATH) != 0
    {
        return Err(Errno::EINVAL);
    }
    let node = super::files::lookup(
        kernel,
        cx,
        tid,
        dirfd,
        path,
        flags & AT_EMPTY_PATH,
        flags & AT_SYMLINK_FOLLOW != 0,
    )?;
    let inode = node.borrow();
    // No file system here can decode a handle back to a parent, which a connectable one needs.
    if flags & AT_HANDLE_CONNECTABLE != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    let (kind, words) = encode(kernel, &inode, flags & AT_HANDLE_FID != 0)?;
    let (mount_id, unique_mount_id) = (mount_id(&inode), unique_mount_id(&inode));
    drop(inode);
    let mut guest = guest(kernel, cx, tid);
    let room: u32 = guest.read_value(handle)?;
    if room > MAX_HANDLE_SZ {
        return Err(Errno::EINVAL);
    }
    let fits = room / 4 >= words.len() as u32;
    let mut out = Vec::new();
    out.extend_from_slice(&(words.len() as u32 * 4).to_le_bytes());
    out.extend_from_slice(&(if fits { kind } else { FILEID_INVALID }).to_le_bytes());
    if fits {
        for word in words {
            out.extend_from_slice(&word.to_le_bytes());
        }
    }
    let written = if flags & AT_HANDLE_MNT_ID_UNIQUE != 0 {
        guest.put(mount, &unique_mount_id.to_le_bytes())
    } else {
        guest.put(mount, &(mount_id as u32).to_le_bytes())
    };
    written?;
    guest.write(handle, &out)?;
    if fits { Ok(0) } else { Err(Errno::EOVERFLOW) }
}
