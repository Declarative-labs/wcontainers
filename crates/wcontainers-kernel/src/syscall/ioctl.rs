//! `ioctl`: the requests every file takes (Linux's `do_vfs_ioctl` and `file_ioctl`), tmpfs's inode
//! flags, and then each file's own requests (pipes, the random devices); a file with none answers
//! ENOTTY, as Linux's `vfs_ioctl` does.

use super::{Args, guest};
use crate::credentials::{CAP_SYS_ADMIN, Credentials};
use crate::file::FileRef;
use crate::fs::dev::Device;
use crate::fs::{Content, Inode, Kind};
use crate::linux::general::*;
use crate::linux::ioctl::*;
use crate::linux_extra::{
    FS_IOC_GETFSSYSFSPATH, FS_IOC_GETFSUUID, FS_IOC_RESVSP, FS_IOC_RESVSP64, FS_IOC_UNRESVSP,
    FS_IOC_UNRESVSP64, FS_IOC_ZERO_RANGE,
};
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// What a request gives: an answer, or Linux's `ENOIOCTLCMD`, which passes it to the file's own
/// requests.
type Answer = Option<Result<u64, Errno>>;

pub(super) fn ioctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, request, argument, ..]: Args,
) -> Result<u64, Errno> {
    let fd = fd as i32;
    let file = kernel.process_of(tid).files.file(fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let request = request as u32;
    match every_file(kernel, cx, tid, fd, &file, request, argument) {
        Some(answer) => answer,
        None => own(kernel, cx, tid, &file, request, argument),
    }
}

/// Linux's `do_vfs_ioctl`, and `file_ioctl` for regular files.
fn every_file(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    fd: i32,
    file: &FileRef,
    request: u32,
    argument: u64,
) -> Answer {
    let node = file.borrow().node.clone();
    let (file_type, regular, kind, fs_uuid) = {
        let inode = node.borrow();
        (
            inode.file_type(),
            inode.file_type() == S_IFREG && !inode.is_secret() && !inode.is_anonymous(),
            inode.fs.kind,
            (inode.fs.kind == Kind::Tmpfs).then(|| uuid(kernel, inode.fs.device)),
        )
    };
    let has = |kernel: &Kernel, power: u32| kernel.process_of(tid).credentials.has(power);
    Some(match request {
        FIOCLEX | FIONCLEX => kernel
            .process_of_mut(tid)
            .files
            .get_mut(fd)
            .map(|descriptor| {
                descriptor.close_on_exec = request == FIOCLEX;
                0
            }),
        FIONBIO => guest(kernel, cx, tid)
            .read_value::<i32>(argument)
            .map(|value| {
                let mut file = file.borrow_mut();
                if value != 0 {
                    file.flags |= O_NONBLOCK;
                } else {
                    file.flags &= !O_NONBLOCK;
                }
                0
            }),
        FIOASYNC => guest(kernel, cx, tid)
            .read_value::<i32>(argument)
            .and_then(|on| {
                let changes = (file.borrow().flags & FASYNC != 0) != (on != 0);
                if changes {
                    set_async(kernel, tid, file, on != 0, fd).ok_or(Errno::ENOTTY)?;
                }
                Ok(0)
            }),
        FIOQSIZE if regular || matches!(file_type, S_IFDIR | S_IFLNK) => {
            let bytes = node.borrow().blocks() * 512;
            guest(kernel, cx, tid)
                .write(argument, &bytes.to_le_bytes())
                .map(|()| 0)
        }
        FIOQSIZE => Err(Errno::ENOTTY),
        // Neither file system here can be frozen (`freeze_fs`), and nothing is frozen to thaw.
        FIFREEZE if !has(kernel, CAP_SYS_ADMIN) => Err(Errno::EPERM),
        FIFREEZE => Err(Errno::EOPNOTSUPP),
        FITHAW if !has(kernel, CAP_SYS_ADMIN) => Err(Errno::EPERM),
        FITHAW => Err(Errno::EINVAL),
        // No file system here maps extents (`fiemap`).
        FS_IOC_FIEMAP => Err(Errno::EOPNOTSUPP),
        FIGETBSZ => {
            let size = kind.block_size() as i32;
            guest(kernel, cx, tid)
                .put(argument, &size.to_le_bytes())
                .map(|()| 0)
        }
        FICLONE => clone_range(kernel, tid, file, argument as u32 as i32),
        FICLONERANGE => guest(kernel, cx, tid)
            .read_value::<file_clone_range>(argument)
            .and_then(|range| clone_range(kernel, tid, file, range.src_fd as i32)),
        FIDEDUPERANGE => dedupe_range(kernel, cx, tid, file, argument),
        FIONREAD if regular => {
            let left = node.borrow().size() as i64 - file.borrow().offset as i64;
            guest(kernel, cx, tid)
                .put(argument, &(left as i32).to_le_bytes())
                .map(|()| 0)
        }
        FIONREAD => return None,
        FS_IOC_GETFLAGS => {
            let flags = attributes(&node.borrow())?;
            guest(kernel, cx, tid)
                .put(argument, &flags.to_le_bytes())
                .map(|()| 0)
        }
        FS_IOC_SETFLAGS => {
            let flags = guest(kernel, cx, tid).read_value::<u32>(argument);
            match flags.and_then(|flags| writable_mount(&node.borrow()).map(|()| flags)) {
                Err(error) => Err(error),
                Ok(flags) => set_attributes(kernel, cx, tid, file, Attributes::Flags(flags))?,
            }
        }
        FS_IOC_FSGETXATTR => {
            let flags = attributes(&node.borrow())?;
            let mut out: fsxattr = zerocopy::FromZeros::new_zeroed();
            out.fsx_xflags = to_xflags(flags);
            guest(kernel, cx, tid)
                .write_value(argument, &out)
                .map(|()| 0)
        }
        FS_IOC_FSSETXATTR => {
            // `copy_fsxattr_from_user` refuses flags Linux does not know, whatever the file.
            let wanted = guest(kernel, cx, tid)
                .read_value::<fsxattr>(argument)
                .and_then(|wanted| {
                    if wanted.fsx_xflags & !crate::linux_extra::FS_XFLAGS_MASK != 0 {
                        Err(Errno::EOPNOTSUPP)
                    } else {
                        Ok(wanted)
                    }
                });
            match wanted.and_then(|wanted| writable_mount(&node.borrow()).map(|()| wanted)) {
                Err(error) => Err(error),
                Ok(wanted) => set_attributes(kernel, cx, tid, file, Attributes::Extended(wanted))?,
            }
        }
        FS_IOC_GETFSUUID => match fs_uuid {
            Some(uuid) => {
                let mut out: fsuuid2 = zerocopy::FromZeros::new_zeroed();
                out.len = 16;
                out.uuid = uuid;
                guest(kernel, cx, tid)
                    .write_value(argument, &out)
                    .map(|()| 0)
            }
            None => Err(Errno::ENOTTY),
        },
        // No file system here has a name under `/sys/fs`.
        FS_IOC_GETFSSYSFSPATH => Err(Errno::ENOTTY),
        FIBMAP if regular => {
            if !has(kernel, crate::credentials::CAP_SYS_RAWIO) {
                Err(Errno::EPERM)
            } else {
                // tmpfs has no block map: the block is 0 and the answer EINVAL (`bmap`).
                let mut guest = guest(kernel, cx, tid);
                guest.read_value::<i32>(argument).and_then(|block| {
                    if block < 0 {
                        return Err(Errno::EINVAL);
                    }
                    guest.put(argument, &0i32.to_le_bytes())?;
                    Err(Errno::EINVAL)
                })
            }
        }
        FS_IOC_RESVSP | FS_IOC_RESVSP64 if regular => {
            preallocate(kernel, cx, tid, fd, file, FALLOC_FL_KEEP_SIZE, argument)
        }
        FS_IOC_UNRESVSP | FS_IOC_UNRESVSP64 if regular => preallocate(
            kernel,
            cx,
            tid,
            fd,
            file,
            FALLOC_FL_KEEP_SIZE | FALLOC_FL_PUNCH_HOLE,
            argument,
        ),
        FS_IOC_ZERO_RANGE if regular => preallocate(
            kernel,
            cx,
            tid,
            fd,
            file,
            FALLOC_FL_KEEP_SIZE | FALLOC_FL_ZERO_RANGE,
            argument,
        ),
        _ => return None,
    })
}

/// A tmpfs's identity, which Linux draws at random when it is mounted: a keyed hash of its device
/// number, with the key drawn at random when the container starts.
fn uuid(kernel: &Kernel, device: u64) -> [u8; 16] {
    use core::hash::Hasher;
    let mut hasher = siphasher::sip128::SipHasher13::new_with_key(&kernel.files.generation_key);
    hasher.write_u64(device);
    siphasher::sip128::Hasher128::finish128(&hasher).as_bytes()
}

/// Sets or clears `FASYNC` through the file's `fasync`, if it has one (pipes, terminals and the
/// random devices), which notes the descriptor the I/O signals carry; a terminal with no owner yet
/// takes its foreground group, or else the caller's process (`__tty_fasync`).
pub(super) fn set_async(
    kernel: &mut Kernel,
    tid: Tid,
    file: &FileRef,
    on: bool,
    fd: i32,
) -> Option<()> {
    let has_fasync = {
        let file = file.borrow();
        let inode = file.node.borrow();
        match inode.content {
            Content::Pipe(_) => true,
            Content::Special => matches!(
                Device::from_number(inode.rdev),
                Some(Device::Random | Device::Urandom)
            ),
            _ => false,
        }
    };
    let terminal = super::tty::can_async(file);
    if !has_fasync && !terminal {
        return None;
    }
    if on && terminal && file.borrow().owner.is_none_or(|owner| owner.pid == 0) {
        let (pid, kind) = match super::tty::foreground_group(file) {
            Some(group) => (group, crate::file::OwnerKind::Group),
            None => (kernel.process_of(tid).pid, crate::file::OwnerKind::Process),
        };
        super::io::set_owner_of(kernel, tid, file, pid, kind);
    }
    {
        let mut file = file.borrow_mut();
        if on {
            file.flags |= FASYNC;
            file.async_fd = Some(fd);
        } else {
            file.flags &= !FASYNC;
            file.async_fd = None;
        }
    }
    Some(())
}

/// Linux's `vfs_clone_file_range`: no file system here can share extents (`remap_file_range`), so
/// what passes the checks is EOPNOTSUPP.
fn clone_range(
    kernel: &Kernel,
    tid: Tid,
    destination: &FileRef,
    source: i32,
) -> Result<u64, Errno> {
    let source = kernel.process_of(tid).files.file(source)?;
    if source.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let input = source.borrow().node.clone();
    let output = destination.borrow().node.clone();
    if input.borrow().fs.device != output.borrow().fs.device {
        return Err(Errno::EXDEV);
    }
    remap_checks(&source, destination)?;
    Err(Errno::EOPNOTSUPP)
}

/// Linux's `generic_file_rw_checks`.
fn remap_checks(input: &FileRef, output: &FileRef) -> Result<(), Errno> {
    let types = [input, output].map(|file| file.borrow().node.borrow().file_type());
    if types.contains(&S_IFDIR) {
        return Err(Errno::EISDIR);
    }
    if types.iter().any(|kind| *kind != S_IFREG) {
        return Err(Errno::EINVAL);
    }
    if !input.borrow().readable()
        || !output.borrow().writable()
        || output.borrow().flags & O_APPEND != 0
    {
        return Err(Errno::EBADF);
    }
    Ok(())
}

/// Linux's `ioctl_file_dedupe_range` and `vfs_dedupe_file_range` up to the file system's own
/// `remap_file_range`, which none here has.
fn dedupe_range(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    argument: u64,
) -> Result<u64, Errno> {
    let header = size_of::<file_dedupe_range>() as u64;
    let mut guest = guest(kernel, cx, tid);
    let count: u16 =
        guest.read_value(argument + core::mem::offset_of!(file_dedupe_range, dest_count) as u64)?;
    let size = header + u64::from(count) * size_of::<file_dedupe_range_info>() as u64;
    if size > wcontainers_abi::PAGE_SIZE {
        return Err(Errno::ENOMEM);
    }
    let bytes = guest.read(argument, size)?;
    let at =
        |offset: usize, length: usize| bytes[offset..offset + length].iter().any(|byte| *byte != 0);
    let reserved = at(core::mem::offset_of!(file_dedupe_range, reserved1), 2)
        || at(core::mem::offset_of!(file_dedupe_range, reserved2), 4);
    if !file.borrow().readable() || reserved {
        return Err(Errno::EINVAL);
    }
    match file.borrow().node.borrow().file_type() {
        S_IFDIR => Err(Errno::EISDIR),
        S_IFREG => Err(Errno::EOPNOTSUPP),
        _ => Err(Errno::EINVAL),
    }
}

/// Linux's `ioctl_preallocate`: a `struct space_resv` names a range from an origin, which is
/// allocated or punched with the size kept, as `fallocate`.
fn preallocate(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    fd: i32,
    file: &FileRef,
    mode: u32,
    argument: u64,
) -> Result<u64, Errno> {
    let bytes =
        guest(kernel, cx, tid).read(argument, crate::linux_extra::SPACE_RESV_SIZE as u64)?;
    let word = |at: usize| i64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"));
    let whence = i16::from_le_bytes([bytes[2], bytes[3]]);
    let mut start = word(8);
    let length = word(16);
    start = start.wrapping_add(match u32::try_from(whence) {
        Ok(SEEK_SET) => 0,
        Ok(SEEK_CUR) => file.borrow().offset as i64,
        Ok(SEEK_END) => file.borrow().node.borrow().size() as i64,
        _ => return Err(Errno::EINVAL),
    });
    super::files::fallocate(
        kernel,
        cx,
        tid,
        [fd as u64, mode.into(), start as u64, length as u64, 0, 0],
    )
}

/// A file's own requests, after the common ones passed (`vfs_ioctl`).
fn own(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    if super::tty::end(file).is_some() {
        return super::tty::ioctl(kernel, cx, tid, file, request, argument);
    }
    if matches!(file.borrow().node.borrow().content, Content::Pidfd(..)) {
        return super::pidfd::ioctl(kernel, cx, tid, file, request, argument);
    }
    let (unread, device) = {
        let file = file.borrow();
        let inode = file.node.borrow();
        (
            match &inode.content {
                Content::Pipe(pipe) => Some(pipe.unread()),
                _ => None,
            },
            match inode.content {
                Content::Special => Device::from_number(inode.rdev),
                _ => None,
            },
        )
    };
    if let Some(unread) = unread {
        // `pipe_ioctl`: the bytes waiting; the host takes a stream's bytes as they are written.
        return match request {
            FIONREAD => guest(kernel, cx, tid)
                .put(argument, &(unread as i32).to_le_bytes())
                .map(|()| 0),
            IOC_WATCH_QUEUE_SET_SIZE | IOC_WATCH_QUEUE_SET_FILTER => Err(kernel.counters.refuse(
                cx.host,
                crate::counters::reason::IOCTL_NOT_BUILT,
                request.into(),
                Errno::ENOTTY,
            )),
            _ => Err(Errno::ENOTTY),
        };
    }
    match device {
        Some(Device::Random | Device::Urandom) => random(kernel, cx, tid, request, argument),
        // tmpfs's files and directories, `/proc`, devpts's directory and `/dev/null`, `/dev/zero`
        // and `/dev/full` have no requests of their own.
        _ => Err(Errno::ENOTTY),
    }
}

/// `random_ioctl`. The pool is always ready, so it holds its full 256 bits; what may add to it or
/// clear it needs CAP_SYS_ADMIN, and what it is given cannot change what the host's generator
/// gives.
fn random(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    request: u32,
    argument: u64,
) -> Result<u64, Errno> {
    let capable = kernel.process_of(tid).credentials.has(CAP_SYS_ADMIN);
    let mut guest = guest(kernel, cx, tid);
    match request {
        RNDGETENTCNT => guest.put(argument, &256i32.to_le_bytes()).map(|()| 0),
        RNDADDTOENTCNT | RNDADDENTROPY | RNDZAPENTCNT | RNDCLEARPOOL | RNDRESEEDCRNG
            if !capable =>
        {
            Err(Errno::EPERM)
        }
        RNDADDTOENTCNT => {
            if guest.read_value::<i32>(argument)? < 0 {
                return Err(Errno::EINVAL);
            }
            Ok(0)
        }
        RNDADDENTROPY => {
            if guest.read_value::<i32>(argument)? < 0 {
                return Err(Errno::EINVAL);
            }
            let length: i32 = guest.read_value(argument + 4)?;
            let length = u64::from(length as u32);
            if !crate::user::access_ok(argument + 8, length) {
                return Err(Errno::EFAULT);
            }
            guest.read(argument + 8, length)?;
            Ok(0)
        }
        RNDZAPENTCNT | RNDCLEARPOOL | RNDRESEEDCRNG => Ok(0),
        _ => Err(Errno::EINVAL),
    }
}

/// tmpfs's inode flags a program may see and change (`SHMEM_FL_USER_VISIBLE`,
/// `SHMEM_FL_USER_MODIFIABLE`).
pub(crate) const TMPFS_FLAGS: u32 =
    FS_IMMUTABLE_FL | FS_APPEND_FL | FS_NODUMP_FL | FS_NOATIME_FL | FS_CASEFOLD_FL;

/// The flags common to every file system that keeps them (`FS_COMMON_FL`).
const COMMON_FLAGS: u32 = FS_SYNC_FL
    | FS_IMMUTABLE_FL
    | FS_APPEND_FL
    | FS_NODUMP_FL
    | FS_NOATIME_FL
    | FS_DAX_FL
    | FS_PROJINHERIT_FL;

/// The extended flags with a plain flag of their own (`FS_XFLAG_COMMON`).
const COMMON_XFLAGS: u32 = FS_XFLAG_SYNC
    | FS_XFLAG_IMMUTABLE
    | FS_XFLAG_APPEND
    | FS_XFLAG_NODUMP
    | FS_XFLAG_NOATIME
    | FS_XFLAG_DAX
    | FS_XFLAG_PROJINHERIT;

const FLAG_PAIRS: [(u32, u32); 7] = [
    (FS_SYNC_FL, FS_XFLAG_SYNC),
    (FS_IMMUTABLE_FL, FS_XFLAG_IMMUTABLE),
    (FS_APPEND_FL, FS_XFLAG_APPEND),
    (FS_NODUMP_FL, FS_XFLAG_NODUMP),
    (FS_NOATIME_FL, FS_XFLAG_NOATIME),
    (FS_DAX_FL, FS_XFLAG_DAX),
    (FS_PROJINHERIT_FL, FS_XFLAG_PROJINHERIT),
];

fn to_xflags(flags: u32) -> u32 {
    FLAG_PAIRS
        .iter()
        .filter(|(flag, _)| flags & flag != 0)
        .fold(0, |out, (_, xflag)| out | xflag)
}

fn from_xflags(xflags: u32) -> u32 {
    FLAG_PAIRS
        .iter()
        .filter(|(_, xflag)| xflags & xflag != 0)
        .fold(0, |out, (flag, _)| out | flag)
}

/// Linux's `vfs_fileattr_get`: only tmpfs's files and directories keep flags; for the rest the
/// request is the file's own.
fn attributes(inode: &Inode) -> Option<u32> {
    keeps_flags(inode).then_some(inode.fsflags & TMPFS_FLAGS)
}

fn keeps_flags(inode: &Inode) -> bool {
    inode.fs.kind == Kind::Tmpfs
        && matches!(inode.content, Content::Regular(_) | Content::Directory(_))
}

/// `mnt_want_write_file`.
fn writable_mount(inode: &Inode) -> Result<(), Errno> {
    if inode.fs.read_only() {
        return Err(Errno::EROFS);
    }
    Ok(())
}

enum Attributes {
    Flags(u32),
    Extended(fsxattr),
}

/// Linux's `vfs_fileattr_set` with tmpfs's `shmem_fileattr_set`; `None` for a file that keeps no
/// flags.
fn set_attributes(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    wanted: Attributes,
) -> Answer {
    let node = file.borrow().node.clone();
    if !keeps_flags(&node.borrow()) {
        return None;
    }
    let now = kernel.now(cx);
    let credentials: &Credentials = &kernel.process_of(tid).credentials;
    let mut inode = node.borrow_mut();
    if !credentials.owns_or_capable(inode.uid) {
        return Some(Err(Errno::EPERM));
    }
    let old = inode.fsflags & TMPFS_FLAGS;
    let (flags, extended) = match wanted {
        Attributes::Flags(flags) => (flags | (old & !COMMON_FLAGS), None),
        Attributes::Extended(wanted) => (
            from_xflags(wanted.fsx_xflags) | (old & !COMMON_FLAGS),
            Some(wanted),
        ),
    };
    // `fileattr_set_prepare`.
    if (flags ^ old) & (FS_APPEND_FL | FS_IMMUTABLE_FL) != 0
        && !credentials.has(crate::credentials::CAP_LINUX_IMMUTABLE)
    {
        return Some(Err(Errno::EPERM));
    }
    if let Some(wanted) = extended {
        let file_type = inode.file_type();
        // Flags that are only reported are dropped from a request (Linux's `FS_XFLAG_RDONLY_MASK`,
        // measured on the reference).
        let mut xflags = wanted.fsx_xflags
            & !(FS_XFLAG_PREALLOC | crate::linux_extra::FS_XFLAG_ATOMICWRITES | FS_XFLAG_HASATTR);
        if xflags & FS_XFLAG_EXTSIZE != 0 && file_type != S_IFREG
            || xflags & FS_XFLAG_EXTSZINHERIT != 0 && file_type != S_IFDIR
            || xflags & FS_XFLAG_COWEXTSIZE != 0 && !matches!(file_type, S_IFREG | S_IFDIR)
        {
            return Some(Err(Errno::EINVAL));
        }
        if wanted.fsx_extsize == 0 {
            xflags &= !(FS_XFLAG_EXTSIZE | FS_XFLAG_EXTSZINHERIT);
        }
        if wanted.fsx_cowextsize == 0 {
            xflags &= !FS_XFLAG_COWEXTSIZE;
        }
        // `fileattr_has_fsx`: tmpfs keeps none of the extended fields.
        if xflags & !COMMON_XFLAGS != 0
            || wanted.fsx_extsize != 0
            || wanted.fsx_projid != 0
            || wanted.fsx_cowextsize != 0
        {
            return Some(Err(Errno::EOPNOTSUPP));
        }
    }
    // Case folding needs a file system mounted with an encoding, which none here is.
    if flags & !TMPFS_FLAGS != 0 || flags & FS_CASEFOLD_FL != 0 {
        return Some(Err(Errno::EOPNOTSUPP));
    }
    inode.fsflags = flags;
    inode.ctime = now;
    Some(Ok(0))
}
