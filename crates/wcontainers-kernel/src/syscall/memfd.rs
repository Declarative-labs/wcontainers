//! Anonymous memory files (`memfd_create`) and the seals `fcntl` puts on tmpfs files, as Linux's
//! `mm/memfd.c` gives them.

use alloc::vec::Vec;

use super::{Args, guest};
use crate::file::OpenFile;
use crate::fs::{Content, Kind, New, Node, Regular, new_inode};
use crate::linux::general::*;
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// What every memory file's name starts with (`MFD_NAME_PREFIX`).
const PREFIX: &[u8] = b"memfd:";
/// The longest name a program may give (`MFD_NAME_MAX_LEN`).
const NAME_MAX_LENGTH: u64 = NAME_MAX as u64 - PREFIX.len() as u64;
const ALL_FLAGS: u32 = MFD_CLOEXEC | MFD_ALLOW_SEALING | MFD_HUGETLB | MFD_NOEXEC_SEAL | MFD_EXEC;
const ALL_SEALS: u32 =
    F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE | F_SEAL_FUTURE_WRITE | F_SEAL_EXEC;

pub(super) fn memfd_create(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [name, flags, ..]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `sanitize_flags`, `alloc_name`, the descriptor and the file.
    let mut flags = flags as u32;
    let allowed = if flags & MFD_HUGETLB != 0 {
        ALL_FLAGS | (MFD_HUGE_MASK << MFD_HUGE_SHIFT)
    } else {
        ALL_FLAGS
    };
    if flags & !allowed != 0 || flags & (MFD_EXEC | MFD_NOEXEC_SEAL) == MFD_EXEC | MFD_NOEXEC_SEAL {
        return Err(Errno::EINVAL);
    }
    // `vm.memfd_noexec` is 0, as in the reference: a file is executable unless asked otherwise.
    if flags & MFD_NOEXEC_SEAL == 0 {
        flags |= MFD_EXEC;
    }
    let name = match guest(kernel, cx, tid).read_string(name, NAME_MAX_LENGTH + 1) {
        Ok(name) => name,
        Err(Errno::ENAMETOOLONG) => return Err(Errno::EINVAL),
        Err(_) => return Err(Errno::EFAULT),
    };
    let process = kernel.process_of(tid);
    let limit = process.limits[RLIMIT_NOFILE as usize].current;
    process.files.lowest_free(0, limit)?;
    // Huge page files live on hugetlbfs, which is not built: refused as by a kernel without it.
    if flags & MFD_HUGETLB != 0 {
        return Err(kernel.counters.refuse_flag(
            cx.host,
            __NR_memfd_create,
            MFD_HUGETLB.into(),
            Errno::ENOSYS,
        ));
    }
    let process = kernel.process_of(tid);
    let (uid, gid) = (process.credentials.fsuid, process.credentials.fsgid);
    let now = kernel.now(cx);
    let fs = kernel.files.memfd_fs.clone();
    let ino = kernel.files.numbers.ino();
    let mut regular = Regular::default();
    let mut mode = S_IFREG | 0o777;
    if flags & MFD_NOEXEC_SEAL != 0 {
        mode &= !0o111;
        regular.seals = F_SEAL_EXEC;
    } else if flags & MFD_ALLOW_SEALING != 0 {
        regular.seals = 0;
    }
    let mut full_name = PREFIX.to_vec();
    full_name.extend_from_slice(&name);
    let node: Node = new_inode(
        fs,
        ino,
        New {
            mode,
            uid,
            gid,
            rdev: 0,
            now,
            content: Content::Regular(regular),
            parent: None,
            name: full_name.clone(),
        },
    )?;
    // It has no name in any directory.
    node.borrow_mut().nlink = 0;
    let mut path: Vec<u8> = b"/".to_vec();
    path.extend_from_slice(&full_name);
    let opener = kernel.process_of(tid).credentials.serial;
    let file = OpenFile::new(node, O_RDWR | O_LARGEFILE, path, opener);
    file.borrow_mut().direct_io = false;
    let process = kernel.process_of_mut(tid);
    let added = process
        .files
        .add(file, flags & MFD_CLOEXEC != 0, 0, limit)?;
    Ok(added as u64)
}

/// `memfd_secret`: an unnamed file of memory only its shared mappings reach (Linux's `secretmem`).
pub(super) fn memfd_secret(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !O_CLOEXEC != 0 {
        return Err(Errno::EINVAL);
    }
    let process = kernel.process_of(tid);
    let limit = process.limits[RLIMIT_NOFILE as usize].current;
    process.files.lowest_free(0, limit)?;
    let (uid, gid) = (process.credentials.fsuid, process.credentials.fsgid);
    let now = kernel.now(cx);
    let fs = kernel
        .files
        .secretmem_fs
        .get_or_insert_with(|| kernel.files.numbers.file_system(Kind::Secretmem, 0))
        .clone();
    let ino = kernel.files.numbers.ino();
    let node: Node = new_inode(
        fs,
        ino,
        New {
            mode: S_IFREG | 0o600,
            uid,
            gid,
            rdev: 0,
            now,
            content: Content::Regular(Regular::default()),
            parent: None,
            name: b"secretmem".to_vec(),
        },
    )?;
    let opener = kernel.process_of(tid).credentials.serial;
    let file = OpenFile::new(node, O_RDWR | O_LARGEFILE, b"/secretmem".to_vec(), opener);
    let added = kernel
        .process_of_mut(tid)
        .files
        .add(file, flags & O_CLOEXEC != 0, 0, limit)?;
    Ok(added as u64)
}

/// The seals of the file an open file is, where it has them: tmpfs's regular files.
fn seals_of(file: &OpenFile) -> Option<u32> {
    let inode = file.node.borrow();
    match &inode.content {
        Content::Regular(regular) if inode.fs.kind == Kind::Tmpfs => Some(regular.seals),
        _ => None,
    }
}

/// `F_GET_SEALS`.
pub(super) fn get_seals(file: &OpenFile) -> Result<u64, Errno> {
    seals_of(file).map(u64::from).ok_or(Errno::EINVAL)
}

/// `F_ADD_SEALS`, as Linux's `memfd_add_seals` checks and widens them.
pub(super) fn add_seals(kernel: &Kernel, file: &OpenFile, seals: u64) -> Result<u64, Errno> {
    if !file.writable() {
        return Err(Errno::EPERM);
    }
    let mut seals = seals as u32;
    if seals & !ALL_SEALS != 0 {
        return Err(Errno::EINVAL);
    }
    let current = seals_of(file).ok_or(Errno::EINVAL)?;
    if current & F_SEAL_SEAL != 0 {
        return Err(Errno::EPERM);
    }
    // No shared mapping may write the file once it is sealed against writing.
    if seals & F_SEAL_WRITE != 0
        && current & F_SEAL_WRITE == 0
        && crate::shared::writably_mapped(kernel, &file.node)
    {
        return Err(Errno::EBUSY);
    }
    let mut inode = file.node.borrow_mut();
    // Sealing against running makes an executable file unchangeable as well.
    if seals & F_SEAL_EXEC != 0 && inode.mode & 0o111 != 0 {
        seals |= F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE | F_SEAL_FUTURE_WRITE;
    }
    if let Content::Regular(regular) = &mut inode.content {
        regular.seals |= seals;
    }
    Ok(0)
}
