//! Reading, writing and moving around in open files, and what descriptors refer to.

use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;

use wcontainers_abi::{PAGE_SIZE, Permissions};

use super::{Args, guest};
use crate::counters::reason;
use crate::file::{Descriptor, FileRef, Owner, OwnerKind};
use crate::fs::Content;
use crate::fs::dev::Device;
use crate::fs::proc::{Style, Writing};
use crate::linux::general::*;
use crate::process::{Channel, Tid};
use crate::user::{Guest, access_ok};
use crate::{Context, Errno, Kernel};

/// The most one read or write moves, as Linux's `MAX_RW_COUNT`.
pub(super) const MAX_RW_COUNT: u64 = 0x7fff_f000;

/// The most bytes the kernel makes at once for a transfer it fills itself, so a large request is
/// filled in pieces rather than held whole.
const KERNEL_BUFFER: u64 = PAGE_SIZE * 16;

/// The most segments `readv` and `writev` take (`UIO_MAXIOV`).
pub(super) const MAX_SEGMENTS: u64 = 1024;

/// The end-of-directory position tmpfs gives (`DIR_OFFSET_EOD`).
const END_OF_DIRECTORY: u64 = i32::MAX as u64;

pub(super) fn file(kernel: &Kernel, tid: Tid, fd: u64) -> Result<FileRef, Errno> {
    kernel.files(tid).file(fd as i32)
}

/// A descriptor's open file as `fdget` finds it: not an `O_PATH` one.
pub(super) fn open_file(kernel: &Kernel, tid: Tid, fd: u64) -> Result<FileRef, Errno> {
    let found = file(kernel, tid, fd)?;
    if found.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    Ok(found)
}

pub(super) fn read(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, count, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    lock_position(kernel, tid, &file)?;
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
    moves_bytes(&file, false)?;
    if !access_ok(buffer, count) {
        return Err(Errno::EFAULT);
    }
    if !super::pipe::is_pipe(&file) {
        verify_area(file.borrow().offset, count)?;
    }
    read_at(
        kernel,
        cx,
        tid,
        &file,
        buffer,
        count.min(MAX_RW_COUNT),
        None,
    )
}

pub(super) fn write(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, count, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    lock_position(kernel, tid, &file)?;
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
    moves_bytes(&file, true)?;
    if !access_ok(buffer, count) {
        return Err(Errno::EFAULT);
    }
    if !super::pipe::is_pipe(&file) {
        verify_area(file.borrow().offset, count)?;
    }
    let append = appends(&file);
    write_at(
        kernel,
        cx,
        tid,
        &file,
        buffer,
        count.min(MAX_RW_COUNT),
        None,
        append,
    )
}

pub(super) fn pread64(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, count, offset, ..]: Args,
) -> Result<u64, Errno> {
    if (offset as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    let file = file(kernel, tid, fd)?;
    positioned(&file, false)?;
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
    moves_bytes(&file, false)?;
    if !access_ok(buffer, count) {
        return Err(Errno::EFAULT);
    }
    verify_area(offset, count)?;
    read_at(
        kernel,
        cx,
        tid,
        &file,
        buffer,
        count.min(MAX_RW_COUNT),
        Some(offset),
    )
}

pub(super) fn pwrite64(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, count, offset, ..]: Args,
) -> Result<u64, Errno> {
    if (offset as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    let file = file(kernel, tid, fd)?;
    positioned(&file, true)?;
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
    moves_bytes(&file, true)?;
    if !access_ok(buffer, count) {
        return Err(Errno::EFAULT);
    }
    verify_area(offset, count)?;
    let append = appends(&file);
    write_at(
        kernel,
        cx,
        tid,
        &file,
        buffer,
        count.min(MAX_RW_COUNT),
        Some(offset),
        append,
    )
}

/// `preadv2` and `pwritev2`: an offset of -1 is the file's own position, and the flags are checked as
/// Linux's `kiocb_set_rw_flags` does, only once there is something to move.
pub(super) fn preadv2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, offset, _, flags]: Args,
) -> Result<u64, Errno> {
    vectored2(kernel, cx, tid, [fd, vector, count, offset, flags], false)
}

pub(super) fn pwritev2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, offset, _, flags]: Args,
) -> Result<u64, Errno> {
    vectored2(kernel, cx, tid, [fd, vector, count, offset, flags], true)
}

fn vectored2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, offset, flags]: [u64; 5],
    writing: bool,
) -> Result<u64, Errno> {
    use crate::linux_extra::{RWF_ATOMIC, RWF_DONTCACHE, RWF_NOAPPEND, RWF_NOSIGNAL};
    let flags = flags as u32;
    let current = offset as i64 == -1;
    if !current && (offset as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    let file = file(kernel, tid, fd)?;
    if current {
        lock_position(kernel, tid, &file)?;
    } else {
        positioned(&file, writing)?;
    }
    let allowed = if writing {
        file.borrow().writable()
    } else {
        file.borrow().readable()
    };
    if !allowed {
        return Err(Errno::EBADF);
    }
    moves_bytes(&file, writing)?;
    let parts = segments(&mut guest(kernel, cx, tid), vector, count)?;
    if parts.iter().all(|(_, length)| *length == 0) {
        return Ok(0);
    }
    // The range is checked before the flags, as in `vfs_readv` and `vfs_writev`.
    if !super::pipe::is_pipe(&file) {
        let total = parts.iter().map(|(_, length)| length).sum();
        let position = if current {
            file.borrow().offset
        } else {
            offset
        };
        verify_area(position, total)?;
    }
    // A file without Linux's iterating read or write (directories, message queues, and writes to
    // `/dev/full`) is moved a segment at a time by `do_loop_readv_writev`, which takes no flag but
    // RWF_HIPRI.
    let looped = {
        let file = file.borrow();
        let inode = file.node.borrow();
        inode.file_type() == S_IFDIR
            || writing
                && matches!(inode.content, Content::Special)
                && matches!(Device::from_number(inode.rdev), Some(Device::Full))
            || matches!(inode.content, Content::Proc(entry) if entry.loops(writing))
            || matches!(inode.content, Content::Mqueue(_))
    };
    if looped && flags & !RWF_HIPRI != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    // `/proc`'s files, tmpfs's regular files and terminals do not take reads or writes that may
    // not wait (`FMODE_NOWAIT` unset; measured: devices and pipes take them).
    let waits = super::tty::end(&file).is_some() || {
        let file = file.borrow();
        let inode = file.node.borrow();
        matches!(inode.content, Content::Proc(_))
            || inode.fs.kind == crate::fs::Kind::Tmpfs && inode.file_type() == S_IFREG
    };
    if waits && flags & RWF_NOWAIT != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    let known =
        RWF_HIPRI | RWF_DSYNC | RWF_SYNC | RWF_NOWAIT | RWF_APPEND | RWF_NOAPPEND | RWF_NOSIGNAL;
    // Atomic writes and uncached access are not offered by tmpfs, nor by anything else here.
    if flags & !(known | RWF_ATOMIC | RWF_DONTCACHE) != 0
        || flags & (RWF_ATOMIC | RWF_DONTCACHE) != 0
    {
        return Err(Errno::EOPNOTSUPP);
    }
    let mut append = appends(&file);
    if flags & RWF_NOAPPEND != 0 {
        append = false;
    }
    if flags & RWF_APPEND != 0 {
        append = true;
    }
    let at = (!current).then_some(offset);
    vectored(
        kernel,
        cx,
        tid,
        &file,
        &parts,
        at,
        writing.then_some(append),
    )
}

pub(super) fn copy_file_range(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [in_fd, in_offset, out_fd, out_offset, length, flags]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `copy_file_range`, `vfs_copy_file_range` and `generic_copy_file_checks`.
    let input = file(kernel, tid, in_fd)?;
    let output = file(kernel, tid, out_fd)?;
    if input.borrow().is_path() || output.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let mut read_offset = |address: u64, file: &FileRef| -> Result<u64, Errno> {
        if address == 0 {
            Ok(file.borrow().offset)
        } else {
            guest(kernel, cx, tid).read_value::<u64>(address)
        }
    };
    let position_in = read_offset(in_offset, &input)?;
    let position_out = read_offset(out_offset, &output)?;
    if flags != 0 {
        return Err(Errno::EINVAL);
    }
    let node_in = input.borrow().node.clone();
    let node_out = output.borrow().node.clone();
    let (type_in, type_out) = (node_in.borrow().file_type(), node_out.borrow().file_type());
    if type_in == S_IFDIR || type_out == S_IFDIR {
        return Err(Errno::EISDIR);
    }
    if type_in != S_IFREG || type_out != S_IFREG {
        return Err(Errno::EINVAL);
    }
    if !input.borrow().readable() || !output.borrow().writable() || appends(&output) {
        return Err(Errno::EBADF);
    }
    // No file system here has its own `copy_file_range`, so both files must share a superblock.
    if node_in.borrow().fs.device != node_out.borrow().fs.device {
        return Err(Errno::EXDEV);
    }
    if position_in.checked_add(length).is_none() || position_out.checked_add(length).is_none() {
        return Err(Errno::EOVERFLOW);
    }
    let size_in = node_in.borrow().size();
    let mut count = if position_in >= size_in {
        0
    } else {
        length.min(size_in - position_in)
    };
    count = write_check_limits(kernel, cx, tid, &output, position_out, count)?;
    if crate::fs::same_inode(&node_in, &node_out)
        && position_out + count > position_in
        && position_out < position_in + count
    {
        return Err(Errno::EINVAL);
    }
    verify_area(position_in, count)?;
    verify_area(position_out, count)?;
    if count == 0 {
        return Ok(0);
    }
    let count = count.min(MAX_RW_COUNT);
    let bytes = file_bytes(cx, &input, count, position_in)?;
    accessed(kernel, cx, &node_in, input.borrow().flags);
    let written = write_bytes(
        kernel,
        cx,
        tid,
        &output,
        &bytes,
        bytes.len() as u64,
        Some(position_out),
        false,
    )?;
    let mut put_back = |address: u64, file: &FileRef, position: u64| -> Result<(), Errno> {
        if address == 0 {
            file.borrow_mut().offset = position;
            Ok(())
        } else {
            guest(kernel, cx, tid).write(address, &position.to_le_bytes())
        }
    };
    put_back(in_offset, &input, position_in + written)?;
    put_back(out_offset, &output, position_out + written)?;
    Ok(written)
}

/// `fadvise64`: tmpfs and the devices have no backing device, so every known piece of advice is taken
/// and ignored, as Linux's `generic_fadvise` does for them.
pub(super) fn fadvise64(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, _, length, advice, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    if file.borrow().node.borrow().file_type() == S_IFIFO {
        return Err(Errno::ESPIPE);
    }
    if (length as i64) < 0 || advice as u32 > POSIX_FADV_NOREUSE {
        return Err(Errno::EINVAL);
    }
    Ok(0)
}

pub(super) fn readahead(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, _, length, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
    // A secret memory file cannot be read ahead (measured).
    let inode = file.borrow().node.clone();
    let inode = inode.borrow();
    if inode.file_type() != S_IFREG
        || inode.is_secret()
        || inode.is_anonymous()
        || (length as i64) < 0
    {
        return Err(Errno::EINVAL);
    }
    Ok(0)
}

pub(super) fn sync_file_range(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, offset, length, flags, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let end = offset.wrapping_add(length);
    if flags as u32 & !SYNC_FILE_RANGE_WRITE_AND_WAIT != 0
        || (offset as i64) < 0
        || (end as i64) < 0
        || end < offset
    {
        return Err(Errno::EINVAL);
    }
    if !matches!(
        file.borrow().node.borrow().file_type(),
        S_IFREG | S_IFBLK | S_IFDIR | S_IFLNK
    ) {
        return Err(Errno::ESPIPE);
    }
    Ok(0)
}

/// Whether a file takes reads or writes at a given offset (Linux's `FMODE_PREAD` and `FMODE_PWRITE`):
/// a pipe and a secret memory file take neither, and a `seq_file` of `/proc` no writes. A descriptor
/// open only for its path is no descriptor here.
pub(super) fn positioned(file: &FileRef, writing: bool) -> Result<(), Errno> {
    let file = file.borrow();
    if file.is_path() {
        return Err(Errno::EBADF);
    }
    let inode = file.node.borrow();
    let sequence = matches!(inode.content, Content::Proc(entry)
        if inode.file_type() == S_IFREG && entry.style() == Style::Sequence);
    // Terminals are opened unseekable (`nonseekable_open`).
    if matches!(inode.content, Content::Pipe(_) | Content::Anon)
        || inode.is_secret()
        || (writing && sequence)
        || file.terminal.is_some()
    {
        return Err(Errno::ESPIPE);
    }
    Ok(())
}

/// Whether a file has a read or write operation at all (Linux's `FMODE_CAN_READ` and
/// `FMODE_CAN_WRITE`): a secret memory file has neither, and most of `/proc`'s files no write.
pub(super) fn moves_bytes(file: &FileRef, writing: bool) -> Result<(), Errno> {
    let file = file.borrow();
    let inode = file.node.borrow();
    let no_write = matches!(inode.content, Content::Proc(entry)
        if inode.file_type() == S_IFREG && entry.writing() == Writing::None);
    // A signal descriptor only reads; a process descriptor does neither.
    let anon_write = writing && matches!(inode.content, Content::Anon | Content::Mqueue(_));
    let pidfd = matches!(inode.content, Content::Pidfd(..));
    if inode.is_secret() || (writing && no_write) || anon_write || pidfd {
        return Err(Errno::EINVAL);
    }
    Ok(())
}

/// Linux's `generic_write_checks_count` for a write of `count` bytes at `offset` into `file`: nothing
/// to write passes, otherwise `generic_write_check_limits`.
pub(super) fn write_limit(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    offset: u64,
    count: u64,
) -> Result<u64, Errno> {
    if count == 0 {
        return Ok(0);
    }
    write_check_limits(kernel, cx, tid, file, offset, count)
}

/// Linux's `generic_write_check_limits`: past the file size limit is `SIGXFSZ` and EFBIG, as is a
/// position at or past the largest file (`s_maxbytes`, or `MAX_NON_LFS` for a file opened without
/// O_LARGEFILE); the count is cut to both.
pub(super) fn write_check_limits(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    offset: u64,
    count: u64,
) -> Result<u64, Errno> {
    let mut count = count;
    let limit = kernel.process_of(tid).limits[RLIMIT_FSIZE as usize].current;
    if limit != RLIM_INFINITY as u64 {
        if offset >= limit {
            // A signal that cannot be acted on is counted where it is sent; the write fails either way.
            kernel.send_file_size_signal(cx, tid);
            return Err(Errno::EFBIG);
        }
        count = count.min(limit - offset);
    }
    let largest = {
        let file = file.borrow();
        if file.flags & O_LARGEFILE == 0 {
            i32::MAX as u64
        } else {
            file.node.borrow().fs.kind.max_bytes()
        }
    };
    if offset >= largest {
        return Err(Errno::EFBIG);
    }
    Ok(count.min(largest - offset))
}

/// Linux's `inode_newsize_ok` for a regular file growing from `size` to `new_size`: past the file
/// size limit is `SIGXFSZ` and `EFBIG`.
pub(super) fn new_size_ok(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    size: u64,
    new_size: u64,
) -> Result<(), Errno> {
    if new_size > size {
        let limit = kernel.process_of(tid).limits[RLIMIT_FSIZE as usize].current;
        if new_size > limit {
            kernel.send_file_size_signal(cx, tid);
            return Err(Errno::EFBIG);
        }
        if new_size > i64::MAX as u64 {
            return Err(Errno::EFBIG);
        }
    }
    Ok(())
}

/// Linux's `rw_verify_area`: the range must end within the largest file offset.
pub(super) fn verify_area(offset: u64, count: u64) -> Result<(), Errno> {
    if (count as i64) < 0
        || (offset as i64) < 0
        || (offset as i64).checked_add(count as i64).is_none()
    {
        return Err(Errno::EINVAL);
    }
    Ok(())
}

/// The segments of a `readv` or `writev`, checked as Linux's `import_iovec` does.
pub(super) fn segments(
    guest: &mut Guest<'_>,
    address: u64,
    count: u64,
) -> Result<Vec<(u64, u64)>, Errno> {
    if count > MAX_SEGMENTS {
        return Err(Errno::EINVAL);
    }
    let bytes = guest.read(address, count * 16)?;
    let entries: Vec<(u64, u64)> = bytes
        .as_chunks::<16>()
        .0
        .iter()
        .map(|chunk| {
            (
                u64::from_le_bytes(chunk[..8].try_into().expect("8 bytes")),
                u64::from_le_bytes(chunk[8..].try_into().expect("8 bytes")),
            )
        })
        .collect();
    // Every length is checked as the list is copied in (`iovec_from_user`), before any address.
    if entries.iter().any(|(_, length)| (*length as i64) < 0) {
        return Err(Errno::EINVAL);
    }
    let mut out = Vec::with_capacity(count as usize);
    let mut total: u64 = 0;
    for (base, length) in entries {
        // Linux shortens the list so the total stays within the largest single transfer.
        let length = length.min(MAX_RW_COUNT - total);
        if !access_ok(base, length) {
            return Err(Errno::EFAULT);
        }
        total += length;
        out.push((base, length));
    }
    Ok(out)
}

pub(super) fn readv(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    lock_position(kernel, tid, &file)?;
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
    moves_bytes(&file, false)?;
    let parts = segments(&mut guest(kernel, cx, tid), vector, count)?;
    vectored(kernel, cx, tid, &file, &parts, None, None)
}

pub(super) fn writev(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    lock_position(kernel, tid, &file)?;
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
    moves_bytes(&file, true)?;
    let parts = segments(&mut guest(kernel, cx, tid), vector, count)?;
    vectored(kernel, cx, tid, &file, &parts, None, Some(appends(&file)))
}

pub(super) fn preadv(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, offset, ..]: Args,
) -> Result<u64, Errno> {
    if (offset as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    let file = file(kernel, tid, fd)?;
    positioned(&file, false)?;
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
    moves_bytes(&file, false)?;
    let parts = segments(&mut guest(kernel, cx, tid), vector, count)?;
    vectored(kernel, cx, tid, &file, &parts, Some(offset), None)
}

pub(super) fn pwritev(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, vector, count, offset, ..]: Args,
) -> Result<u64, Errno> {
    if (offset as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    let file = file(kernel, tid, fd)?;
    positioned(&file, true)?;
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
    moves_bytes(&file, true)?;
    let parts = segments(&mut guest(kernel, cx, tid), vector, count)?;
    vectored(
        kernel,
        cx,
        tid,
        &file,
        &parts,
        Some(offset),
        Some(appends(&file)),
    )
}

/// Whether writes to a file go to its end.
fn appends(file: &FileRef) -> bool {
    file.borrow().flags & O_APPEND != 0
}

/// Reads or writes a list of segments in turn; `writing` says whether it writes, and if so whether at
/// the file's end.
#[allow(
    clippy::too_many_arguments,
    reason = "one call shape for every vectored read and write"
)]
fn vectored(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
    mut offset: Option<u64>,
    writing: Option<bool>,
) -> Result<u64, Errno> {
    // Linux's `vfs_readv` and `vfs_writev`: nothing to move is done at once, else the whole range is
    // checked against the largest offset before the file sees it.
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    if total == 0 {
        return Ok(0);
    }
    if super::tty::end(file).is_some() {
        return match writing {
            Some(_) => super::tty::write(kernel, cx, tid, file, parts),
            None => super::tty::read(kernel, cx, tid, file, parts),
        };
    }
    // A pipe takes or gives the whole vector in one go (`pipe_read`, `pipe_write`).
    if super::pipe::is_pipe(file) {
        return match writing {
            Some(_) => super::pipe::write(kernel, cx, tid, file, parts),
            None => super::pipe::read(kernel, cx, tid, file, parts),
        };
    }
    if writing.is_none() && super::signalfd::mask_of(&file.borrow()).is_some() {
        return super::signalfd::read(kernel, cx, tid, file, parts);
    }
    if writing.is_none() && super::timer::of(&file.borrow()).is_some() {
        return super::timer::read(kernel, cx, tid, file, parts);
    }
    verify_area(offset.unwrap_or(file.borrow().offset), total)?;
    // The file size limit applies to the whole write at once.
    let mut parts = parts.to_vec();
    if let Some(append) = writing
        && matches!(file.borrow().node.borrow().content, Content::Regular(_))
    {
        let start = if append {
            file.borrow().node.borrow().size()
        } else {
            offset.unwrap_or(file.borrow().offset)
        };
        let total = parts.iter().map(|(_, length)| length).sum();
        let mut allowed = write_limit(kernel, cx, tid, file, start, total)?;
        for (_, length) in &mut parts {
            *length = (*length).min(allowed);
            allowed -= *length;
        }
    }
    if writing.is_none() && is_sysctl(file) {
        return read_sysctl(kernel, cx, tid, file, &parts, offset);
    }
    // The random devices fill the whole vector block by block.
    if writing.is_none()
        && matches!(&file.borrow().node.borrow().content, Content::Special)
        && matches!(
            Device::from_number(file.borrow().node.borrow().rdev),
            Some(Device::Random | Device::Urandom)
        )
    {
        return random_bytes(kernel, cx, tid, &parts);
    }
    if on_backend_regular(file) {
        return match writing {
            Some(append) => write_parts(kernel, cx, tid, file, &parts, offset, append),
            None => read_parts(kernel, cx, tid, file, &parts, offset),
        };
    }
    let mut done = 0;
    for &(base, length) in &parts {
        if length == 0 {
            continue;
        }
        let result = if let Some(append) = writing {
            write_at(kernel, cx, tid, file, base, length, offset, append)
        } else {
            read_at(kernel, cx, tid, file, base, length, offset)
        };
        match result {
            Ok(moved) => {
                done += moved;
                offset = offset.map(|offset| offset + moved);
                if moved < length {
                    break;
                }
            }
            Err(error) if done == 0 => return Err(error),
            Err(_) => break,
        }
    }
    Ok(done)
}

/// The bytes a read of up to `count` bytes at `offset` gives, without moving the file's position.
pub(super) fn file_bytes(
    cx: &mut Context<'_>,
    file: &FileRef,
    count: u64,
    offset: u64,
) -> Result<Vec<u8>, Errno> {
    let node = file.borrow().node.clone();
    let inode = node.borrow();
    let window = |data: &[u8]| {
        let start = (offset as usize).min(data.len());
        let end = (start + count as usize).min(data.len());
        data[start..end].to_vec()
    };
    Ok(match &inode.content {
        Content::Directory(_) => return Err(Errno::EISDIR),
        Content::Proc(_) | Content::Backend(_) if inode.is_directory() => {
            return Err(Errno::EISDIR);
        }
        Content::Backend(_) => {
            drop(inode);
            let file = file.borrow();
            let handle = file.backend.as_ref().ok_or(Errno::EBADF)?;
            return crate::shared::backend_read(cx, &node, handle, offset, count);
        }
        Content::Regular(regular) => regular.read(cx.cpu, offset, count),
        Content::Proc(_) => window(file.borrow().contents.as_deref().unwrap_or_default()),
        Content::Special => match Device::from_number(inode.rdev) {
            Some(Device::Null) => Vec::new(),
            Some(Device::Zero | Device::Full) => vec![0; count as usize],
            Some(Device::Random | Device::Urandom) => {
                let mut bytes = vec![0; count as usize];
                cx.host.random(&mut bytes);
                bytes
            }
            _ => return Err(Errno::ENXIO),
        },
        Content::Pipe(_) | Content::Symlink(_) | Content::Pidfd(..) | Content::Anon => {
            return Err(Errno::EINVAL);
        }
        // `mqueue_read_file`: the queue's size and request, which a read that returns bytes marks
        // as accessed.
        Content::Mqueue(queue) => {
            let bytes = window(&queue.text());
            drop(inode);
            if !bytes.is_empty() {
                let now = crate::fs::Time::from_nanoseconds(cx.host.realtime_nanoseconds());
                let mut inode = node.borrow_mut();
                inode.atime = now;
                inode.ctime = now;
            }
            bytes
        }
    })
}

/// Whether a file is on the backend, which opened it for direct reads and writes (FOPEN_DIRECT_IO).
pub(super) fn direct_on_backend(file: &FileRef) -> bool {
    file.borrow()
        .backend
        .as_ref()
        .is_some_and(|handle| handle.direct_io())
}

fn is_sysctl(file: &FileRef) -> bool {
    matches!(file.borrow().node.borrow().content, Content::Proc(entry)
        if entry.style() == Style::Sysctl)
}

/// A sysctl's read (`proc_sys_call_handler`): made into a kernel buffer the size of the whole request,
/// then copied out whole across the segments, or EFAULT with the position left where it was.
fn read_sysctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
    at: Option<u64>,
) -> Result<u64, Errno> {
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    if total >= crate::linux_extra::KMALLOC_MAX_SIZE {
        return Err(Errno::ENOMEM);
    }
    let offset = at.unwrap_or(file.borrow().offset);
    refresh_proc(kernel, cx, tid, file, offset)?;
    let bytes = file_bytes(cx, file, total, offset)?;
    let (node, flags) = (file.borrow().node.clone(), file.borrow().flags);
    accessed(kernel, cx, &node, flags);
    let done = bytes.len() as u64;
    if copy_to_segments(&mut guest(kernel, cx, tid), parts, 0, &bytes, 0) < done {
        return Err(Errno::EFAULT);
    }
    file.borrow_mut().read_position = offset + done;
    if at.is_none() {
        file.borrow_mut().offset = offset + done;
    }
    Ok(done)
}

/// `get_random_bytes_user`: the whole request filled from the host's generator in 64-byte blocks,
/// each copied with arm64's copy routine, until a copy falls short. The reference's block lies 8
/// bytes past 16-byte alignment (measured).
pub(super) fn random_bytes(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    const BLOCK: usize = 64;
    let count: u64 = parts.iter().map(|(_, length)| length).sum();
    let mut done = 0;
    let mut bytes = vec![0; count.min(KERNEL_BUFFER) as usize];
    while done < count {
        let part = (count - done).min(KERNEL_BUFFER) as usize;
        cx.host.random(&mut bytes[..part]);
        let mut guest = guest(kernel, cx, tid);
        for block in bytes[..part].chunks(BLOCK) {
            let stored = copy_to_segments(&mut guest, parts, done, block, 8);
            done += stored;
            if stored < block.len() as u64 {
                return if done == 0 {
                    Err(Errno::EFAULT)
                } else {
                    Ok(done)
                };
            }
        }
    }
    Ok(done)
}

/// `copy_to_iter` of `bytes` from a kernel buffer at `source` into the segments, from `done` bytes
/// into them: one copy per segment it reaches, stopping at the first that falls short. Returns how
/// many bytes were counted copied.
pub(super) fn copy_to_segments(
    guest: &mut crate::user::Guest<'_>,
    parts: &[(u64, u64)],
    done: u64,
    bytes: &[u8],
    source: u64,
) -> u64 {
    let mut copied = 0;
    let mut start = 0;
    for &(base, length) in parts {
        let end = start + length;
        let at = done + copied;
        if at < end && copied < bytes.len() as u64 {
            let within = at - start;
            let part = (length - within).min(bytes.len() as u64 - copied) as usize;
            let from = copied as usize;
            let stored = guest.copy_out(base + within, &bytes[from..from + part], source + copied);
            copied += stored;
            if stored < part as u64 {
                break;
            }
        }
        start = end;
    }
    copied
}

/// Reads up to `count` bytes into the guest at the file's position, or at `at` without moving it.
pub(super) fn read_at(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    buffer: u64,
    count: u64,
    at: Option<u64>,
) -> Result<u64, Errno> {
    if super::tty::end(file).is_some() {
        return super::tty::read(kernel, cx, tid, file, &[(buffer, count)]);
    }
    if super::pipe::is_pipe(file) {
        return super::pipe::read(kernel, cx, tid, file, &[(buffer, count)]);
    }
    if super::signalfd::mask_of(&file.borrow()).is_some() {
        return super::signalfd::read(kernel, cx, tid, file, &[(buffer, count)]);
    }
    if super::timer::of(&file.borrow()).is_some() {
        return super::timer::read(kernel, cx, tid, file, &[(buffer, count)]);
    }
    // The zero devices clear the buffer in place and keep no position. `/dev/zero` stops at the first
    // short clear (Linux's `read_zero`); `/dev/full` clears again from where it stopped until nothing
    // more is cleared (`read_iter_zero`).
    let device = {
        let file = file.borrow();
        let inode = file.node.borrow();
        match inode.content {
            Content::Special => Device::from_number(inode.rdev),
            _ => None,
        }
    };
    if matches!(device, Some(Device::Random | Device::Urandom)) {
        return random_bytes(kernel, cx, tid, &[(buffer, count)]);
    }
    if let Some(device) = device.filter(|device| matches!(device, Device::Zero | Device::Full)) {
        let mut guest = guest(kernel, cx, tid);
        let mut cleared = guest.clear_partial(buffer, count);
        if device == Device::Full {
            while cleared < count {
                let more = guest.clear_partial(buffer + cleared, (count - cleared).min(PAGE_SIZE));
                if more == 0 {
                    break;
                }
                cleared += more;
            }
        }
        if cleared == 0 && count > 0 {
            return Err(Errno::EFAULT);
        }
        return Ok(cleared);
    }
    moves_bytes(file, false)?;
    if is_sysctl(file) {
        return read_sysctl(kernel, cx, tid, file, &[(buffer, count)], at);
    }
    if on_backend_regular(file) {
        return read_parts(kernel, cx, tid, file, &[(buffer, count)], at);
    }
    let offset = at.unwrap_or(file.borrow().offset);
    refresh_proc(kernel, cx, tid, file, offset)?;
    let bytes = file_bytes(cx, file, count, offset)?;
    let (node, flags) = (file.borrow().node.clone(), file.borrow().flags);
    accessed(kernel, cx, &node, flags);
    // A `seq_file` copies from its kernel buffer with arm64's copy routine; pages of files and the
    // process's memory reach exactly up to a fault (measured).
    let sequence = matches!(node.borrow().content, Content::Proc(entry)
        if entry.style() == Style::Sequence);
    let written = if sequence {
        guest(kernel, cx, tid).copy_out(buffer, &bytes, offset) as usize
    } else {
        guest(kernel, cx, tid).write_partial(buffer, &bytes)
    };
    if written == 0 && !bytes.is_empty() {
        return Err(Errno::EFAULT);
    }
    file.borrow_mut().read_position = offset + written as u64;
    // A device's reads leave the position alone.
    let device = matches!(file.borrow().node.borrow().content, Content::Special);
    if at.is_none() && !device {
        file.borrow_mut().offset = offset + written as u64;
    }
    Ok(written as u64)
}

/// Whether a file is a regular file on the backend.
fn on_backend_regular(file: &FileRef) -> bool {
    let file = file.borrow();
    let inode = file.node.borrow();
    matches!(inode.content, Content::Backend(_)) && !inode.is_directory()
}

/// Reads a backend file into the segments in one go at the file's position, or at `at` without
/// moving it, as Linux's FUSE takes the whole iterator: a run of the call after a wait asks the
/// same questions again.
fn read_parts(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
    at: Option<u64>,
) -> Result<u64, Errno> {
    let offset = at.unwrap_or(file.borrow().offset);
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    // `fuse_direct_io`: a file the backend opened for direct reads and writes is read into the
    // buffers' pages, taken first: as far as they can be written, EFAULT if not at all.
    let count = if direct_on_backend(file) {
        let mut guest = guest(kernel, cx, tid);
        let mut reachable = 0;
        for &(base, length) in parts {
            let part = guest.accessible(base, length, Permissions::WRITE);
            reachable += part;
            if part < length {
                break;
            }
        }
        if reachable == 0 && total > 0 {
            return Err(Errno::EFAULT);
        }
        reachable
    } else {
        total
    };
    let bytes = file_bytes(cx, file, count, offset)?;
    let (node, flags) = (file.borrow().node.clone(), file.borrow().flags);
    accessed(kernel, cx, &node, flags);
    // Pages of files reach exactly up to a fault (measured).
    let mut written = 0;
    let mut guest = guest(kernel, cx, tid);
    for &(base, length) in parts {
        let piece = &bytes[written..(written + length as usize).min(bytes.len())];
        if piece.is_empty() {
            break;
        }
        let stored = guest.write_partial(base, piece);
        written += stored;
        if stored < piece.len() {
            break;
        }
    }
    if written == 0 && !bytes.is_empty() {
        return Err(Errno::EFAULT);
    }
    let mut file = file.borrow_mut();
    file.read_position = offset + written as u64;
    if at.is_none() {
        file.offset = offset + written as u64;
    }
    Ok(written as u64)
}

/// Writes the segments to a regular file in one go at the file's position, or at `at` without
/// moving it, or at the file's end if `append`: as much as can be read, up to the file size limit.
#[allow(clippy::too_many_arguments, reason = "every write names where it goes")]
fn write_parts(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
    at: Option<u64>,
    append: bool,
) -> Result<u64, Errno> {
    let node = file.borrow().node.clone();
    lock_backend_writes(kernel, tid, &node)?;
    // A backend's file ends where the backend says now (`fuse_update_attributes`).
    if append && let Some(handle) = &file.borrow().backend {
        crate::fs::backend::getattr(&node, Some(handle.fh))?;
    }
    let offset = if append {
        node.borrow().size()
    } else {
        at.unwrap_or(file.borrow().offset)
    };
    let total = parts.iter().map(|(_, length)| length).sum();
    let count = write_limit(kernel, cx, tid, file, offset, total)?;
    // As much of the buffers as can be read; where it stops, the write ends (`write_bytes`).
    let mut bytes = Vec::new();
    let mut left = count;
    let mut guest = guest(kernel, cx, tid);
    for &(base, length) in parts {
        let length = length.min(left);
        let readable = guest.accessible(base, length, Permissions::READ);
        bytes.extend_from_slice(&guest.read(base, readable)?);
        left -= length;
        if readable < length || left == 0 {
            break;
        }
    }
    write_bytes(kernel, cx, tid, file, &bytes, count, at, append)
}

/// `inode_lock` around a write to a backend file (`fuse_direct_write_iter` without
/// FOPEN_PARALLEL_DIRECT_WRITES, `fuse_cache_write_iter`), a truncation (`do_truncate`) or an
/// allocation that changes its size or bytes (`fuse_file_fallocate`), held while the call waits
/// for the backend. Files in memory finish these without waiting, so nothing else needs it.
pub(super) fn lock_backend_writes(
    kernel: &mut Kernel,
    tid: Tid,
    node: &crate::fs::Node,
) -> Result<(), Errno> {
    let holder = match &node.borrow().content {
        Content::Backend(remote) if !node.borrow().is_directory() => remote.writer.get(),
        _ => return Ok(()),
    };
    kernel.take_lock(
        tid,
        holder,
        Channel::Held(Rc::as_ptr(node) as *const () as usize),
    )?;
    if let Content::Backend(remote) = &node.borrow().content {
        remote.writer.set(Some(tid));
    }
    Ok(())
}

/// `fdget_pos`: a regular file's or directory's position is moved by one call at a time
/// (`f_pos_lock` under FMODE_ATOMIC_POS), held while the call waits.
pub(super) fn lock_position(kernel: &mut Kernel, tid: Tid, file: &FileRef) -> Result<(), Errno> {
    let kind = file.borrow().node.borrow().file_type();
    if kind != S_IFREG && kind != S_IFDIR {
        return Ok(());
    }
    let holder = file.borrow().position_holder;
    kernel.take_lock(
        tid,
        holder,
        Channel::Held(Rc::as_ptr(file) as *const () as usize),
    )?;
    file.borrow_mut().position_holder = Some(tid);
    Ok(())
}

/// Writes up to `count` bytes from the guest at the file's position, or at `at` without moving it, or
/// at the file's end if `append`.
#[allow(clippy::too_many_arguments, reason = "every write names where it goes")]
pub(super) fn write_at(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    buffer: u64,
    count: u64,
    at: Option<u64>,
    append: bool,
) -> Result<u64, Errno> {
    let node = file.borrow().node.clone();
    if super::tty::end(file).is_some() {
        return super::tty::write(kernel, cx, tid, file, &[(buffer, count)]);
    }
    if super::pipe::is_pipe(file) {
        return super::pipe::write(kernel, cx, tid, file, &[(buffer, count)]);
    }
    moves_bytes(file, true)?;
    let entry = match &node.borrow().content {
        Content::Proc(entry) => Some(*entry),
        _ => None,
    };
    if let Some(entry) = entry {
        return write_proc(kernel, cx, tid, entry, buffer, count);
    }
    let (regular, device) = {
        let inode = node.borrow();
        (
            matches!(inode.content, Content::Regular(_))
                || matches!(inode.content, Content::Backend(_)) && !inode.is_directory(),
            Device::from_number(inode.rdev),
        )
    };
    if regular {
        return write_parts(kernel, cx, tid, file, &[(buffer, count)], at, append);
    }
    // The random devices take in as much of the buffer as can be read (`write_pool_user`); the rest
    // only count the bytes.
    if matches!(device, Some(Device::Random | Device::Urandom)) {
        let readable = guest(kernel, cx, tid).accessible(buffer, count, Permissions::READ);
        if readable == 0 && count > 0 {
            return Err(Errno::EFAULT);
        }
        return Ok(readable);
    }
    write_bytes(kernel, cx, tid, file, &[], 0, at, append).map(|_| count)
}

/// A write to a file of `/proc`, which takes no position and moves none.
fn write_proc(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    entry: crate::fs::proc::Entry,
    buffer: u64,
    count: u64,
) -> Result<u64, Errno> {
    match entry.writing() {
        // `comm_write`: the first 15 bytes name the file's thread, up to a zero, and the whole count
        // is taken; only threads of the same process may name it.
        Writing::Comm => {
            let bytes = guest(kernel, cx, tid).read(buffer, count.min(15))?;
            let crate::fs::proc::Entry::ProcessFile(task, _) = entry else {
                return Err(Errno::EINVAL);
            };
            if !kernel.threads.contains_key(&task.tid) {
                return Err(Errno::ESRCH);
            }
            if task.pid != kernel.threads[&tid].process {
                return Err(Errno::EINVAL);
            }
            kernel.thread_mut(task.tid).set_name_exact(&bytes);
            Ok(count)
        }
        Writing::Io => Err(Errno::EIO),
        Writing::Sysctl => Err(Errno::EROFS),
        Writing::None => Err(Errno::EINVAL),
    }
}

/// Writes bytes the kernel holds to a file, at its position or at `at`, of the `wanted` the caller
/// asked for: fewer when the caller's buffer stopped being readable. Devices that take anything
/// report nothing written here; the caller counts what it offered.
#[allow(clippy::too_many_arguments, reason = "every write names where it goes")]
pub(super) fn write_bytes(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    bytes: &[u8],
    wanted: u64,
    at: Option<u64>,
    append: bool,
) -> Result<u64, Errno> {
    let node = file.borrow().node.clone();
    lock_backend_writes(kernel, tid, &node)?;
    let now = kernel.now(cx);
    let may_keep_privileges = kernel.credentials(tid).has(crate::credentials::CAP_FSETID);
    let mut inode = node.borrow_mut();
    let rdev = inode.rdev;
    let regular = match &mut inode.content {
        Content::Regular(regular) => regular,
        Content::Backend(remote) => {
            let offset = if append {
                remote.size.get()
            } else {
                at.unwrap_or(file.borrow().offset)
            };
            drop(inode);
            if offset
                .checked_add(bytes.len() as u64)
                .is_none_or(|end| end > i64::MAX as u64)
            {
                return Err(Errno::EFBIG);
            }
            let written = {
                let file = file.borrow();
                let handle = file.backend.as_ref().ok_or(Errno::EBADF)?;
                let written = crate::fs::backend::write(handle, offset, bytes)?;
                // Linux's page cache takes what a cached write wrote (`fuse_fill_write_pages`).
                if !handle.direct_io() {
                    crate::shared::backend_written(cx, &node, offset, &bytes[..written as usize]);
                }
                written
            };
            // `fuse_write_update_attr`: the file ends at least where the write did.
            if let Content::Backend(remote) = &node.borrow().content
                && remote.size.get() < offset + written
            {
                remote.set_size(offset + written);
            }
            crate::shared::changed(kernel, cx, &node)?;
            if written == 0 && wanted > 0 {
                return Err(Errno::EFAULT);
            }
            if at.is_none() {
                file.borrow_mut().offset = offset + written;
            }
            return Ok(written);
        }
        Content::Pipe(_) => {
            drop(inode);
            return Ok(super::pipe::put(cx, kernel, file, bytes, 0));
        }
        Content::Special => {
            return match Device::from_number(rdev) {
                Some(Device::Null | Device::Zero | Device::Random | Device::Urandom) => {
                    Ok(bytes.len() as u64)
                }
                Some(Device::Full) => Err(Errno::ENOSPC),
                _ => Err(Errno::ENXIO),
            };
        }
        Content::Directory(_)
        | Content::Proc(_)
        | Content::Symlink(_)
        | Content::Pidfd(..)
        | Content::Anon
        | Content::Mqueue(_) => {
            return Err(Errno::EINVAL);
        }
    };
    let offset = if append {
        regular.size()
    } else {
        at.unwrap_or(file.borrow().offset)
    };
    if offset
        .checked_add(bytes.len() as u64)
        .is_none_or(|end| end > i64::MAX as u64)
    {
        return Err(Errno::EFBIG);
    }
    let mut bytes = bytes;
    if wanted > 0 {
        // The times change and privileges go before the seals are looked at, as in tmpfs.
        let mut allowed = regular.writable(offset, bytes.len() as u64);
        if let Ok(written) = allowed {
            bytes = &bytes[..written as usize];
            regular.write(cx.cpu, offset, bytes);
            // Where the buffer stopped being readable, Linux's `generic_perform_write` has already
            // taken the next page from tmpfs (`shmem_write_begin`) when the copy into it fails: the
            // page stays, holding zeros, and the write ends there, EFAULT if nothing was written.
            let next = offset + written;
            if written < wanted && written as usize == bytes.len() {
                let chunk = ((next / 4096 + 1) * 4096 - next).min(wanted - written);
                let page = regular.writable(next, chunk);
                if page.is_ok() {
                    regular.hold_page(next / 4096);
                }
                if written == 0 {
                    allowed = page.and(Err(Errno::EFAULT));
                }
            }
        }
        inode.mtime = now;
        inode.ctime = now;
        // Writing clears the set-user and set-group bits for a writer without `CAP_FSETID`, and file
        // capabilities always, as Linux's `file_remove_privs` does.
        inode.drop_privileges(!may_keep_privileges);
        allowed?;
    }
    drop(inode);
    crate::shared::changed(kernel, cx, &node)?;
    if at.is_none() {
        file.borrow_mut().offset = offset + bytes.len() as u64;
    }
    Ok(bytes.len() as u64)
}

pub(super) fn sendfile(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [out_fd, in_fd, offset_address, count, ..]: Args,
) -> Result<u64, Errno> {
    // Linux's `sendfile64`: the offset is read first and written back whatever the transfer did.
    if offset_address == 0 {
        return send(kernel, cx, tid, out_fd, in_fd, None, count);
    }
    let mut offset: i64 = guest(kernel, cx, tid).read_value(offset_address)?;
    let result = send(kernel, cx, tid, out_fd, in_fd, Some(&mut offset), count);
    if result == Err(Errno::PARKED) {
        return result;
    }
    guest(kernel, cx, tid).put(offset_address, &offset.to_le_bytes())?;
    result
}

/// Linux's `do_sendfile`, in its order: the input, its offset, the output, then the transfer.
fn send(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    out_fd: u64,
    in_fd: u64,
    explicit: Option<&mut i64>,
    count: u64,
) -> Result<u64, Errno> {
    let input = file(kernel, tid, in_fd)?;
    if input.borrow().is_path() || !input.borrow().readable() {
        return Err(Errno::EBADF);
    }
    if explicit.is_some() {
        positioned(&input, false)?;
    }
    let position = match &explicit {
        Some(offset) => **offset,
        None => input.borrow().offset as i64,
    };
    // `rw_verify_area`.
    if (count as i64) < 0 || position < 0 || position.checked_add(count as i64).is_none() {
        return Err(Errno::EINVAL);
    }
    let count = count.min(MAX_RW_COUNT);
    let output = file(kernel, tid, out_fd)?;
    if output.borrow().is_path() || !output.borrow().writable() {
        return Err(Errno::EBADF);
    }
    // `rw_verify_area` of the output at its own position.
    if (output.borrow().offset as i64)
        .checked_add(count as i64)
        .is_none()
    {
        return Err(Errno::EINVAL);
    }
    // A pipe takes the input spliced into it once, as much as it has room for
    // (`splice_file_to_pipe`). Anything else goes through a private pipe, round after round,
    // until the count is moved or a round falls short (`splice_direct_to_actor`).
    let (input_node, output_node) = (input.borrow().node.clone(), output.borrow().node.clone());
    let to_pipe = matches!(output_node.borrow().content, Content::Pipe(_));
    // The largest file either side's file system has.
    let max = input_node
        .borrow()
        .fs
        .kind
        .max_bytes()
        .min(output_node.borrow().fs.kind.max_bytes());
    let position_now = position as u64;
    let mut count = count;
    if position_now + count > max {
        if position_now >= max {
            return Err(Errno::EOVERFLOW);
        }
        count = max - position_now;
    }
    if !to_pipe {
        if output.borrow().flags & O_APPEND != 0 {
            return Err(Errno::EINVAL);
        }
        // A pipe, a terminal, secret memory and a process descriptor as input cannot seek
        // (`FMODE_LSEEK`), even for nothing; timer and signal descriptors can, but have no
        // `splice_read`, so only nothing is moved from them (measured; into a pipe nothing is
        // moved without asking it).
        if matches!(
            input_node.borrow().content,
            Content::Pipe(_) | Content::Pidfd(..)
        ) || super::tty::end(&input).is_some()
            || input_node.borrow().is_secret()
        {
            return Err(Errno::EINVAL);
        }
        if input_node.borrow().is_anonymous() {
            return if count == 0 {
                Ok(0)
            } else {
                Err(Errno::EINVAL)
            };
        }
    }
    // Into a pipe: `splice_file_to_pipe`, which waits for room unless the pipe's descriptor is
    // nonblocking.
    if to_pipe {
        super::pipe::lock(kernel, tid, &output)?;
        let nonblock = output.borrow().flags & O_NONBLOCK != 0;
        let mut pos = position as u64;
        let moved = super::splice::file_to_pipe(
            kernel, cx, tid, &input, &output, &mut pos, count, nonblock,
        )?;
        if moved > 0 {
            input.borrow_mut().read_position = pos;
            match explicit {
                Some(offset) => *offset = pos as i64,
                None => input.borrow_mut().offset = pos,
            }
        }
        return Ok(moved);
    }
    let device = |node: &crate::fs::Node| {
        let inode = node.borrow();
        match inode.content {
            Content::Special => Device::from_number(inode.rdev),
            _ => None,
        }
    };
    let flags = input.borrow().flags;
    // A call that waited for a terminal's room goes on after what it wrote, reading the rest of
    // that round again (Linux keeps the round in its private pipe).
    let resumed = kernel.threads[&tid].call.progress;
    let mut pos = position as u64
        + if device(&input_node).is_some() {
            0
        } else {
            resumed
        };
    let mut moved = resumed;
    let mut failure = None;
    let mut remaining = count - resumed;
    while remaining > 0 {
        // What cannot be spliced from (no `splice_read`: directories, secret memory, `/dev/null`,
        // the process files of `/proc`), or into (secret memory has no `splice_write`), fails at the
        // first round that asks for bytes.
        let unspliceable = {
            let inode = input_node.borrow();
            inode.is_directory()
                || inode.is_secret()
                || output_node.borrow().is_secret()
                || matches!(
                    inode.content,
                    Content::Proc(crate::fs::proc::Entry::ProcessFile(..))
                )
        };
        if unspliceable || device(&input_node) == Some(Device::Null) {
            failure = Some(Errno::EINVAL);
            break;
        }
        // One round fills a pipe's pages; tmpfs puts each page's part in its own buffer.
        let room = crate::linux_extra::PIPE_DEF_BUFFERS * PAGE_SIZE;
        let regular = matches!(input_node.borrow().content, Content::Regular(_));
        let wanted = remaining.min(if regular {
            room - pos % PAGE_SIZE
        } else {
            room
        });
        // A terminal (only into a pipe: it cannot seek) is read as `copy_splice_read` reads it.
        let read = if super::tty::end(&input).is_some() {
            super::tty::take(kernel, cx, tid, &input, wanted)
        } else {
            refresh_proc(kernel, cx, tid, &input, pos)
                .and_then(|()| file_bytes(cx, &input, wanted, pos))
        };
        let bytes = match read {
            Ok(bytes) => bytes,
            Err(error) => {
                failure = Some(error);
                break;
            }
        };
        if bytes.is_empty() {
            break;
        }
        // A device's read leaves the position alone (`copy_splice_read` keeps what the read left).
        let read_to = if device(&input_node).is_some() {
            pos
        } else {
            pos + bytes.len() as u64
        };
        // The output takes what was read; `/dev/full` and `/proc` files have no `splice_write`.
        let read = bytes.len() as u64;
        let mut bytes = bytes;
        let written = if device(&output_node) == Some(Device::Full)
            || matches!(output_node.borrow().content, Content::Proc(_))
        {
            Err(Errno::EINVAL)
        } else if super::tty::end(&output).is_some() {
            super::tty::put(kernel, cx, tid, &output, &bytes)
        } else if matches!(output_node.borrow().content, Content::Regular(_)) {
            let at = output.borrow().offset;
            write_limit(kernel, cx, tid, &output, at, bytes.len() as u64).and_then(|allowed| {
                bytes.truncate(allowed as usize);
                write_bytes(kernel, cx, tid, &output, &bytes, allowed, None, false)
            })
        } else {
            write_bytes(
                kernel,
                cx,
                tid,
                &output,
                &bytes,
                bytes.len() as u64,
                None,
                false,
            )
        };
        let written = match written {
            Ok(written) => written,
            Err(error) => {
                failure = Some(error);
                break;
            }
        };
        input.borrow_mut().read_position = pos + written;
        moved += written;
        remaining -= written;
        if written < read {
            pos += written;
            // A full terminal: a blocking one is waited for.
            if super::tty::end(&output).is_some() {
                match super::tty::wait_for_room(kernel, tid, &output, moved) {
                    Errno::PARKED => return Err(Errno::PARKED),
                    error => failure = Some(error),
                }
            }
            break;
        }
        pos = read_to;
    }
    touch_atime(kernel, cx, &input_node, flags);
    if moved == 0 {
        return failure.map_or(Ok(0), Err);
    }
    match explicit {
        Some(offset) => *offset = pos as i64,
        None => input.borrow_mut().offset = pos,
    }
    Ok(moved)
}

pub(super) fn lseek(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, offset, whence, ..]: Args,
) -> Result<u64, Errno> {
    let file_ref = file(kernel, tid, fd)?;
    lock_position(kernel, tid, &file_ref)?;
    let mut file = file_ref.borrow_mut();
    if file.is_path() {
        return Err(Errno::EBADF);
    }
    let offset = offset as i64;
    let whence = whence as u32;
    // An unknown origin is refused before the file has its say.
    if whence > SEEK_MAX {
        return Err(Errno::EINVAL);
    }
    if file.node.borrow().is_secret() || file.terminal.is_some() {
        return Err(Errno::ESPIPE);
    }
    let current = || {
        (file.offset as i64)
            .checked_add(offset)
            .ok_or(Errno::EINVAL)
    };
    if let Some(handle) = &file.backend {
        // `fuse_file_llseek`: the backend finds data and holes; the end is where it says now. A
        // directory seeks over the size it holds (`generic_file_llseek`).
        match whence {
            SEEK_END if !handle.directory => {
                crate::fs::backend::getattr(&file.node, Some(handle.fh))?
            }
            SEEK_DATA | SEEK_HOLE if !handle.directory => {
                let position = crate::fs::backend::lseek(handle, offset, whence)?;
                file.offset = position;
                return Ok(position);
            }
            _ => {}
        }
    }
    let inode = file.node.borrow();
    let mut sequence = None;
    let new = match &inode.content {
        Content::Pipe(_) | Content::Pidfd(..) => return Err(Errno::ESPIPE),
        // Signal descriptors keep their position at 0 (`noop_llseek`).
        Content::Anon => file.offset as i64,
        // A queue seeks as `default_llseek` does over the size it shows.
        Content::Mqueue(queue) => {
            let size = queue.size as i64;
            match whence {
                SEEK_SET => offset,
                SEEK_CUR => current()?,
                SEEK_END => size.checked_add(offset).ok_or(Errno::EINVAL)?,
                SEEK_DATA if offset < size => offset,
                SEEK_HOLE if offset < size => size,
                SEEK_DATA | SEEK_HOLE => return Err(Errno::ENXIO),
                _ => return Err(Errno::EINVAL),
            }
        }
        Content::Special => match Device::from_number(inode.rdev) {
            // `/dev/null`, `/dev/zero` and `/dev/full` always seek to 0 (`null_lseek`); the random
            // devices keep their position (`noop_llseek`).
            Some(Device::Null | Device::Zero | Device::Full) => 0,
            _ => file.offset as i64,
        },
        Content::Backend(remote) => {
            let size = remote.size.get() as i64;
            match whence {
                SEEK_SET => offset,
                SEEK_CUR => current()?,
                SEEK_END => size.checked_add(offset).ok_or(Errno::EINVAL)?,
                // Only directories come here: all data up to the size, as an unsigned offset.
                SEEK_DATA | SEEK_HOLE if offset as u64 >= size as u64 => return Err(Errno::ENXIO),
                SEEK_DATA => offset,
                _ => size,
            }
        }
        Content::Regular(regular) => {
            let size = regular.size() as i64;
            match whence {
                SEEK_SET => offset,
                SEEK_CUR => current()?,
                SEEK_END => size.checked_add(offset).ok_or(Errno::EINVAL)?,
                SEEK_DATA | SEEK_HOLE => {
                    if offset < 0 || offset >= size {
                        return Err(Errno::ENXIO);
                    }
                    let page = offset as u64 / PAGE_SIZE;
                    let found = if whence == SEEK_DATA {
                        regular
                            .pages
                            .range(page..)
                            .next()
                            .map(|data| (*data * PAGE_SIZE).max(offset as u64))
                    } else {
                        let mut at = page;
                        while regular.pages.contains(&at) {
                            at += 1;
                        }
                        Some((at * PAGE_SIZE).max(offset as u64))
                    };
                    match found {
                        Some(position) if (position as i64) < size => position as i64,
                        _ if whence == SEEK_HOLE => size,
                        _ => return Err(Errno::ENXIO),
                    }
                }
                _ => return Err(Errno::EINVAL),
            }
        }
        // The process information tree's directories and `cmdline`, `environ` and `auxv` seek as
        // Linux's `generic_file_llseek` does with a size of zero, up to proc's largest offset, the
        // default `MAX_NON_LFS` (`vfs_setpos`).
        Content::Proc(entry) if inode.is_directory() || entry.style() == Style::Memory => {
            let new = match whence {
                SEEK_SET | SEEK_END => offset,
                SEEK_CUR => current()?,
                _ => return Err(Errno::ENXIO),
            };
            if new > i64::from(i32::MAX) {
                return Err(Errno::EINVAL);
            }
            new
        }
        // Sysctls seek as `default_llseek` with a size of zero, with no largest offset.
        Content::Proc(entry) if entry.style() == Style::Sysctl => match whence {
            SEEK_SET | SEEK_END => offset,
            SEEK_CUR => current()?,
            _ => return Err(Errno::ENXIO),
        },
        Content::Proc(entry) => {
            sequence = Some(*entry);
            match whence {
                SEEK_SET => offset,
                SEEK_CUR => current()?,
                _ => return Err(Errno::EINVAL),
            }
        }
        Content::Directory(_) | Content::Symlink(_) => match whence {
            SEEK_SET => offset,
            SEEK_CUR => current()?,
            _ => return Err(Errno::EINVAL),
        },
    };
    if new < 0 {
        return Err(Errno::EINVAL);
    }
    drop(inode);
    file.offset = new as u64;
    // `seq_lseek` to anywhere but where the last read stopped makes the bytes again and keeps them
    // from there (`traverse`); to the start it only forgets them.
    if let Some(entry) = sequence
        && new as u64 != file.read_position
    {
        file.read_position = new as u64;
        file.contents = None;
        drop(file);
        if new != 0 {
            let bytes = super::files::proc_contents(kernel, cx, tid, &entry)?;
            file_ref.borrow_mut().contents = Some(bytes);
        }
    }
    Ok(new as u64)
}

/// Linux's `touch_atime` for a read of `node` through a file opened with `flags` (0 for a read by
/// path), for tmpfs's files, directories and links, the only ones here that keep access times: none with `O_NOATIME`, the
/// no-access-time inode flag or a read-only mount; on a `relatime` mount only when the access time
/// is not after the last change or is a day old; else whenever it is not already now.
pub(super) fn accessed(kernel: &Kernel, cx: &mut Context<'_>, node: &crate::fs::Node, flags: u32) {
    // Devices' own reads leave the time alone.
    let kept = matches!(
        node.borrow().content,
        Content::Regular(_) | Content::Directory(_) | Content::Symlink(_)
    );
    if kept {
        touch_atime(kernel, cx, node, flags);
    }
}

/// Linux's `touch_atime` itself, which a splice also makes for a device it read.
fn touch_atime(kernel: &Kernel, cx: &mut Context<'_>, node: &crate::fs::Node, flags: u32) {
    let now = kernel.now(cx);
    let mut inode = node.borrow_mut();
    if inode.fs.kind != crate::fs::Kind::Tmpfs
        || flags & O_NOATIME != 0
        || inode.fsflags & FS_NOATIME_FL != 0
        || inode.fs.read_only()
    {
        return;
    }
    const DAY: i64 = 24 * 60 * 60;
    if inode.fs.flags & crate::linux_extra::ST_RELATIME != 0
        && inode.mtime < inode.atime
        && inode.ctime < inode.atime
        && now.seconds - inode.atime.seconds < DAY
    {
        return;
    }
    inode.atime = now;
}

/// Makes a `/proc` file's bytes again where Linux would before a read at `offset`: at each read for
/// a file read from memory or a sysctl, and for a `seq_file` at a read from the start or from
/// anywhere but where the last read stopped.
pub(super) fn refresh_proc(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    offset: u64,
) -> Result<(), Errno> {
    let entry = {
        let file = file.borrow();
        let inode = file.node.borrow();
        match inode.content {
            Content::Proc(entry) if inode.file_type() == S_IFREG => entry,
            _ => return Ok(()),
        }
    };
    let again = match entry.style() {
        Style::Memory | Style::Sysctl => true,
        Style::Sequence => {
            let file = file.borrow();
            offset == 0 || offset != file.read_position || file.contents.is_none()
        }
    };
    if again {
        let bytes = super::files::proc_contents(kernel, cx, tid, &entry)?;
        file.borrow_mut().contents = Some(bytes);
    }
    Ok(())
}

pub(super) fn getdents64(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, count, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    lock_position(kernel, tid, &file)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    // The buffer is checked only as each entry is written, after the file is found to be a
    // directory, as Linux's `filldir64` does.
    let count = count as u32 as u64;
    let node = file.borrow().node.clone();
    let position = file.borrow().offset;
    let mut entries: Vec<Listed> = Vec::new();
    {
        if !node.borrow().is_directory() {
            return Err(Errno::ENOTDIR);
        }
        // `iterate_dir` counts a listing of a directory not removed as reading it.
        if node.borrow().nlink > 0 {
            accessed(kernel, cx, &node, file.borrow().flags);
        }
        let inode = node.borrow();
        let parent_ino = inode
            .place
            .parent
            .as_ref()
            .and_then(|parent| parent.upgrade())
            .map_or(inode.ino, |parent| parent.borrow().ino);
        match &inode.content {
            // devpts and mqueue list by count (`dcache_readdir`): `.` at 0, `..` at 1, then each
            // entry at the next number, newest first.
            Content::Directory(directory)
                if matches!(
                    inode.fs.kind,
                    crate::fs::Kind::Devpts | crate::fs::Kind::Mqueue
                ) =>
            {
                if position == 0 {
                    entries.push((b".".to_vec(), inode.ino, DT_DIR as u8, 0, 1));
                }
                if position <= 1 {
                    entries.push((b"..".to_vec(), parent_ino, DT_DIR as u8, 1, 2));
                }
                for (index, (_, name, child)) in directory.listed_from(2).enumerate() {
                    let own = index as u64 + 2;
                    if own >= position {
                        let child = child.borrow();
                        let kind = child.dirent_type();
                        entries.push((name.to_vec(), child.ino, kind, own, own + 1));
                    }
                }
            }
            Content::Directory(directory) => {
                // `.` at 0, `..` at 1, then the entries in tmpfs's order, each giving the next one's
                // position.
                let newest = directory.listed_from(2).next().map(|(at, _, _)| at);
                if position == 0 {
                    entries.push((b".".to_vec(), inode.ino, DT_DIR as u8, 0, 1));
                }
                if position <= 1 {
                    entries.push((
                        b"..".to_vec(),
                        parent_ino,
                        DT_DIR as u8,
                        1,
                        newest.unwrap_or(END_OF_DIRECTORY),
                    ));
                }
                if position != END_OF_DIRECTORY {
                    let listed: Vec<_> = directory.listed_from(position).collect();
                    for (index, (at, name, child)) in listed.iter().enumerate() {
                        let next = listed
                            .get(index + 1)
                            .map_or(END_OF_DIRECTORY, |(at, _, _)| *at);
                        let child = child.borrow();
                        let kind = child.dirent_type();
                        entries.push((name.to_vec(), child.ino, kind, *at, next));
                    }
                }
            }
            // `fuse_readdir`: the backend's entries from the position, which it numbers.
            Content::Backend(_) => {
                drop(inode);
                let file = file.borrow();
                let handle = file.backend.as_ref().ok_or(Errno::EBADF)?;
                let mut own = position;
                for entry in crate::fs::backend::readdir(handle, position)? {
                    entries.push((entry.name, entry.ino, entry.kind, own, entry.next));
                    own = entry.next;
                }
            }
            Content::Proc(entry) => {
                let entry = *entry;
                drop(inode);
                // Each entry has Linux's position. What follows an entry (its `d_off`) is the position
                // after its own, except where Linux moves to the next entry's before listing it:
                // descriptors (`proc_readfd_common`, which ends at the table's size plus 2), and
                // `thread-self` and processes in `/proc` (`proc_pid_readdir`, which ends past the
                // largest number).
                use crate::fs::proc::Entry;
                let (listed, end) = kernel.with_walk(tid, |walk, _| {
                    let mut out = Vec::new();
                    for (name, child, position) in crate::fs::proc::list(walk.proc, &entry) {
                        let jumps = match child {
                            Entry::Fd(..) | Entry::ThreadSelfLink => true,
                            Entry::Process(task) => !task.own,
                            _ => false,
                        };
                        let child_node = crate::fs::proc::node(walk.proc, None, &name, child);
                        let child_inode = child_node.borrow();
                        let kind = child_inode.dirent_type();
                        out.push((name, child_inode.ino, kind, position, jumps));
                    }
                    let end = match entry {
                        Entry::Fds(task) => walk
                            .proc
                            .threads
                            .get(&task.tid)
                            .map_or(2, |thread| u64::from(thread.files.borrow().capacity()) + 2),
                        _ => crate::fs::proc::PROCESSES_END,
                    };
                    Ok((out, end))
                })?;
                let ino = node.borrow().ino;
                let fds = matches!(entry, Entry::Fds(_));
                if position == 0 {
                    entries.push((b".".to_vec(), ino, DT_DIR as u8, 0, 1));
                }
                if position <= 1 {
                    let after = if fds {
                        listed.first().map_or(end, |listed| listed.3)
                    } else {
                        2
                    };
                    entries.push((b"..".to_vec(), parent_ino, DT_DIR as u8, 1, after));
                }
                let nexts: Vec<u64> = listed
                    .iter()
                    .skip(1)
                    .map(|listed| listed.3)
                    .chain([end])
                    .collect();
                for ((name, ino, kind, own, jumps), next) in listed.into_iter().zip(nexts) {
                    if own >= position {
                        let after = if jumps { next } else { own + 1 };
                        entries.push((name, ino, kind, own, after));
                    }
                }
            }
            _ => return Err(Errno::ENOTDIR),
        }
    }
    // The entries that fit, each at its offset in the buffer.
    let mut placed = Vec::new();
    let mut size = 0;
    for entry in entries {
        let length = (19 + entry.0.len() + 1).next_multiple_of(8) as u64;
        if size + length > count {
            if placed.is_empty() {
                return Err(Errno::EINVAL);
            }
            break;
        }
        placed.push((size, length, entry));
        size += length;
    }
    let (complete, size) = store_entries(&mut guest(kernel, cx, tid), buffer, &placed);
    if complete == 0 && !placed.is_empty() {
        return Err(Errno::EFAULT);
    }
    if complete > 0 || position <= 1 {
        file.borrow_mut().offset = match complete {
            0 => position,
            complete => placed[complete - 1].2.4,
        };
    }
    Ok(size)
}

/// A directory entry to list: name, inode number, type, its own position and the one after it.
type Listed = (Vec<u8>, u64, u8, u64, u64);

/// Stores listed entries as `filldir64` does, entry by entry: the previous entry's `d_off` (for the
/// first, its own position), `d_ino`, `d_reclen`, `d_type`, the name's closing zero, then the name
/// in pieces of 8, 4, 2 and 1 bytes, and at the end the last entry's `d_off`; padding is not written.
/// A store that faults ends the listing there. Returns how many entries were stored whole and their
/// size.
fn store_entries(
    guest: &mut crate::user::Guest<'_>,
    buffer: u64,
    placed: &[(u64, u64, Listed)],
) -> (usize, u64) {
    let Some(&(last_at, last_length, _)) = placed.last() else {
        return (0, 0);
    };
    let extent = last_at + last_length;
    let writable = if crate::user::access_ok(buffer, extent) {
        guest.accessible(buffer, extent, wcontainers_abi::Permissions::WRITE)
    } else {
        0
    };
    if writable == extent {
        // Nothing faults: each entry's bytes as they end up, in one store each.
        for (at, _, (name, ino, kind, _, next)) in placed {
            let mut bytes = Vec::with_capacity(20 + name.len());
            bytes.extend_from_slice(&ino.to_le_bytes());
            bytes.extend_from_slice(&next.to_le_bytes());
            bytes.extend_from_slice(
                &((19 + name.len() + 1).next_multiple_of(8) as u16).to_le_bytes(),
            );
            bytes.push(*kind);
            bytes.extend_from_slice(name);
            bytes.push(0);
            if guest.put(buffer + at, &bytes).is_err() {
                return (0, 0);
            }
        }
        return (placed.len(), extent);
    }
    let mut stores: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut ends = Vec::new();
    for (index, (at, length, (name, ino, kind, own, _))) in placed.iter().enumerate() {
        let previous = if index == 0 { *at } else { placed[index - 1].0 };
        stores.push((previous + 8, own.to_le_bytes().to_vec()));
        stores.push((*at, ino.to_le_bytes().to_vec()));
        stores.push((at + 16, (*length as u16).to_le_bytes().to_vec()));
        stores.push((at + 18, alloc::vec![*kind]));
        stores.push((at + 19 + name.len() as u64, alloc::vec![0]));
        let mut done = 0;
        for size in [8, 4, 2, 1] {
            while name.len() - done >= size {
                stores.push((at + 19 + done as u64, name[done..done + size].to_vec()));
                done += size;
            }
        }
        ends.push(stores.len());
    }
    let borrowed: Vec<(u64, &[u8])> = stores
        .iter()
        .map(|(offset, bytes)| (*offset, bytes.as_slice()))
        .collect();
    let made = guest.put_each(buffer, &borrowed);
    let complete = ends.iter().take_while(|end| **end <= made).count();
    if complete == 0 {
        return (0, 0);
    }
    let (at, length, (.., next)) = &placed[complete - 1];
    let _ = guest.put(buffer + at + 8, &next.to_le_bytes());
    (complete, at + length)
}

pub(super) fn close(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, ..]: Args,
) -> Result<u64, Errno> {
    let pid = kernel.threads[&tid].process;
    // `filp_close`: a file on the backend is flushed there, and the descriptor goes whatever the
    // flush answers.
    let flushed = match kernel.files(tid).file(fd as i32) {
        Ok(file) => match &file.borrow().backend {
            Some(handle) => crate::fs::backend::flush(handle, kernel.files(tid).lock_owner()),
            None => Ok(()),
        },
        Err(_) => Ok(()),
    };
    if flushed == Err(Errno::PARKED) {
        return Err(Errno::PARKED);
    }
    kernel.files_mut(tid).remove(fd as i32, pid, true)?;
    kernel.files_released(cx);
    flushed.map(|()| 0)
}

fn nofile_limit(kernel: &Kernel, tid: Tid) -> u64 {
    kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current
}

pub(super) fn close_range(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [first, last, flags, ..]: Args,
) -> Result<u64, Errno> {
    use crate::linux_extra::{CLOSE_RANGE_CLOEXEC, CLOSE_RANGE_UNSHARE};
    let (first, last, flags) = (first as u32, last as u32, flags as u32);
    if flags & !(CLOSE_RANGE_UNSHARE | CLOSE_RANGE_CLOEXEC) != 0 || first > last {
        return Err(Errno::EINVAL);
    }
    // A shared table is copied first, and the copy closes (`__close_range`).
    if flags & CLOSE_RANGE_UNSHARE != 0 {
        kernel.unshare_files(tid);
    }
    let pid = kernel.threads[&tid].process;
    let mut files = kernel.files_mut(tid);
    let fds: Vec<i32> = files
        .iter()
        .map(|(fd, _)| fd)
        .filter(|fd| (first..=last).contains(&(*fd as u32)))
        .collect();
    let closes = flags & CLOSE_RANGE_CLOEXEC == 0 && !fds.is_empty();
    for fd in fds {
        if flags & CLOSE_RANGE_CLOEXEC != 0 {
            files.get_mut(fd)?.close_on_exec = true;
        } else {
            files.remove(fd, pid, false)?;
        }
    }
    drop(files);
    if closes {
        kernel.files_released(cx);
    }
    Ok(0)
}

pub(super) fn dup(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, ..]: Args,
) -> Result<u64, Errno> {
    let limit = nofile_limit(kernel, tid);
    let mut files = kernel.files_mut(tid);
    let file = files.file(fd as i32)?;
    Ok(files.add(file, false, 0, limit)? as u64)
}

pub(super) fn dup3(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [old, new, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (old, new, flags) = (old as i32, new as i32, flags as u32);
    if flags & !O_CLOEXEC != 0 {
        return Err(Errno::EINVAL);
    }
    if old == new {
        return Err(Errno::EINVAL);
    }
    let limit = nofile_limit(kernel, tid);
    if new < 0 || new as u64 >= limit {
        return Err(Errno::EBADF);
    }
    let pid = kernel.threads[&tid].process;
    let mut files = kernel.files_mut(tid);
    let file = files.file(old)?;
    let replaced = files.set(
        new,
        Descriptor {
            file,
            close_on_exec: flags & O_CLOEXEC != 0,
        },
        pid,
    );
    drop(files);
    if replaced {
        kernel.files_released(cx);
    }
    Ok(new as u64)
}

/// The status flags `F_SETFL` may change.
const SETTABLE_FLAGS: u32 = O_APPEND | O_NONBLOCK | O_NDELAY | O_DIRECT | O_NOATIME;

pub(super) fn fcntl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    arguments: Args,
) -> super::Outcome {
    match fcntl_typed(kernel, cx, tid, arguments) {
        Ok(Some(value)) => super::Outcome::Return(value),
        Ok(None) => {
            // A lock another holder has: the wait lasts until it is let go or a signal comes.
            if kernel.signal_pending(tid) {
                return Err(Errno::ERESTARTSYS).into();
            }
            kernel.park(tid, crate::process::Channel::Locks);
            super::Outcome::Parked
        }
        Err(error) => Err(error).into(),
    }
}

/// `fcntl`: Some(value) when it is done, None when a lock must wait for its holder to let go.
fn fcntl_typed(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, command, argument, ..]: Args,
) -> Result<Option<u64>, Errno> {
    let fd = fd as i32;
    let command = command as u32;
    if matches!(
        command,
        F_GETLK | F_SETLK | F_SETLKW | F_OFD_GETLK | F_OFD_SETLK | F_OFD_SETLKW
    ) {
        let file = kernel.files(tid).file(fd)?;
        if file.borrow().is_path() {
            return Err(Errno::EBADF);
        }
        return record_lock(kernel, cx, tid, &file, command, argument);
    }
    fcntl_other(kernel, cx, tid, fd, command, argument).map(Some)
}

/// fcntl's record locks, as Linux's `fcntl_getlk` and `fcntl_setlk`: the request is read, checked,
/// tested or applied, and for a test written back.
fn record_lock(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &crate::file::FileRef,
    command: u32,
    argument: u64,
) -> Result<Option<u64>, Errno> {
    use crate::locks::{LockOwner, OFFSET_MAX, RecordLock};
    use core::mem::offset_of;
    let mut raw = guest(kernel, cx, tid).read(argument, size_of::<flock>() as u64)?;
    let short = |raw: &[u8], at: usize| i16::from_le_bytes([raw[at], raw[at + 1]]);
    let long = |raw: &[u8], at: usize| i64::from_le_bytes(raw[at..at + 8].try_into().expect("8"));
    let kind = short(&raw, offset_of!(flock, l_type)) as i32;
    let whence = short(&raw, offset_of!(flock, l_whence)) as u32;
    let (l_start, l_len) = (
        long(&raw, offset_of!(flock, l_start)),
        long(&raw, offset_of!(flock, l_len)),
    );
    let l_pid = i32::from_le_bytes(
        raw[offset_of!(flock, l_pid)..offset_of!(flock, l_pid) + 4]
            .try_into()
            .expect("4"),
    );
    let getting = matches!(command, F_GETLK | F_OFD_GETLK);
    let private = matches!(command, F_OFD_GETLK | F_OFD_SETLK | F_OFD_SETLKW);
    if command == F_GETLK && kind != F_RDLCK as i32 && kind != F_WRLCK as i32 {
        return Err(Errno::EINVAL);
    }
    // `flock64_to_posix_lock`: the range from where it is measured, then the kind.
    let (offset, size) = {
        let file = file.borrow();
        (file.offset as i64, file.node.borrow().size() as i64)
    };
    let base = match whence {
        SEEK_SET => 0,
        SEEK_CUR => offset,
        SEEK_END => size,
        _ => return Err(Errno::EINVAL),
    };
    let max = OFFSET_MAX as i64;
    if l_start > max - base {
        return Err(Errno::EOVERFLOW);
    }
    let mut start = base + l_start;
    if start < 0 {
        return Err(Errno::EINVAL);
    }
    let end = if l_len > 0 {
        if l_len - 1 > max - start {
            return Err(Errno::EOVERFLOW);
        }
        start + (l_len - 1)
    } else if l_len < 0 {
        if start + l_len < 0 {
            return Err(Errno::EINVAL);
        }
        let end = start - 1;
        start += l_len;
        end
    } else {
        max
    };
    let (write, unlock) = match kind as u32 {
        F_RDLCK => (false, false),
        F_WRLCK => (true, false),
        F_UNLCK => (false, true),
        _ => return Err(Errno::EINVAL),
    };
    // A lock needs the file open for its kind (`check_fmode_for_setlk`).
    if !getting && !unlock {
        let file = file.borrow();
        if (write && !file.writable()) || (!write && !file.readable()) {
            return Err(Errno::EBADF);
        }
    }
    if private && l_pid != 0 {
        return Err(Errno::EINVAL);
    }
    let pid = kernel.threads[&tid].process;
    let request = RecordLock {
        owner: if private {
            LockOwner::File(alloc::rc::Rc::downgrade(file))
        } else {
            LockOwner::Table(kernel.files(tid).lock_owner())
        },
        pid,
        write,
        start: start as u64,
        end: end as u64,
    };
    let node = file.borrow().node.clone();
    if getting {
        let found = crate::locks::test(&mut node.borrow_mut().record_locks, &request, unlock);
        let put_short = |raw: &mut [u8], at: usize, value: i16| {
            raw[at..at + 2].copy_from_slice(&value.to_le_bytes())
        };
        match found {
            None => put_short(&mut raw, offset_of!(flock, l_type), F_UNLCK as i16),
            Some(held) => {
                let kind = if held.write { F_WRLCK } else { F_RDLCK };
                put_short(&mut raw, offset_of!(flock, l_type), kind as i16);
                put_short(&mut raw, offset_of!(flock, l_whence), SEEK_SET as i16);
                let length = if held.end == OFFSET_MAX {
                    0
                } else {
                    held.end - held.start + 1
                };
                let at = offset_of!(flock, l_start);
                raw[at..at + 8].copy_from_slice(&held.start.to_le_bytes());
                let at = offset_of!(flock, l_len);
                raw[at..at + 8].copy_from_slice(&length.to_le_bytes());
                let shown = if held.owner.is_file() { -1 } else { held.pid };
                let at = offset_of!(flock, l_pid);
                raw[at..at + 4].copy_from_slice(&shown.to_le_bytes());
            }
        }
        guest(kernel, cx, tid).write(argument, &raw)?;
        return Ok(Some(0));
    }
    let asking = match &request.owner {
        LockOwner::Table(table) => Some(*table),
        LockOwner::File(_) => None,
    };
    let result = crate::locks::set(&mut node.borrow_mut().record_locks, request, unlock);
    match result {
        Ok(changed) => {
            // Waiters blocked on a lock this changed try again (`locks_wake_up_blocks`).
            if changed {
                kernel.wake(crate::process::Channel::Locks);
            }
            Ok(Some(0))
        }
        Err(held) if matches!(command, F_SETLKW | F_OFD_SETLKW) => {
            let holder = match held.owner {
                LockOwner::Table(table) => Some(table),
                LockOwner::File(_) => None,
            };
            // An open file's request is not looked at for deadlocks, nor kept where others look.
            if let Some(asking) = asking {
                if posix_locks_deadlock(kernel, asking, holder) {
                    return Err(Errno::EDEADLK);
                }
                kernel.thread_mut(tid).call.lock_wait = Some((asking, holder));
            }
            Ok(None)
        }
        Err(_) => Err(Errno::EAGAIN),
    }
}

/// Linux's `posix_locks_deadlock`: whether following who the lock in the way's holder waits for
/// comes back to the table asking, giving up after `MAX_DEADLK_ITERATIONS`.
fn posix_locks_deadlock(kernel: &Kernel, asking: u64, mut holder: Option<u64>) -> bool {
    const MAX_DEADLK_ITERATIONS: usize = 10;
    let waiting_for = |table: u64| {
        kernel
            .threads
            .values()
            .find_map(|thread| match thread.call {
                crate::process::Call {
                    wait: Some(crate::process::Channel::Locks),
                    lock_wait: Some((waiter, blocker)),
                    ..
                } if waiter == table => Some(blocker),
                _ => None,
            })
    };
    let mut iterations = 0;
    while let Some(blocker) = holder.and_then(waiting_for) {
        if iterations > MAX_DEADLK_ITERATIONS {
            return false;
        }
        iterations += 1;
        if blocker == Some(asking) {
            return true;
        }
        holder = blocker;
    }
    false
}

/// Every other `fcntl` command.
fn fcntl_other(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    fd: i32,
    command: u32,
    argument: u64,
) -> Result<u64, Errno> {
    let limit = nofile_limit(kernel, tid);
    let files = kernel.threads[&tid].files.clone();
    let descriptor = files.borrow().get(fd)?.clone();
    let path_only = descriptor.file.borrow().is_path();
    match command {
        F_DUPFD | F_DUPFD_CLOEXEC => {
            let minimum = argument as i32;
            if minimum < 0 || minimum as u64 >= limit {
                return Err(Errno::EINVAL);
            }
            let close_on_exec = command == F_DUPFD_CLOEXEC;
            Ok(files
                .borrow_mut()
                .add(descriptor.file, close_on_exec, minimum, limit)? as u64)
        }
        F_GETFD => Ok(u64::from(descriptor.close_on_exec)),
        F_SETFD => {
            files.borrow_mut().get_mut(fd)?.close_on_exec = argument & u64::from(FD_CLOEXEC) != 0;
            Ok(0)
        }
        F_GETFL => Ok(u64::from(descriptor.file.borrow().flags)),
        F_SETFL if !path_only => {
            let mut file = descriptor.file.borrow_mut();
            let wanted = argument as u32;
            // In the order of Linux's `setfl`. Only the owner or a holder of CAP_FOWNER may stop
            // access times; only tmpfs's regular files take direct I/O (pipes read the flag as packet
            // mode).
            {
                let inode = file.node.borrow();
                let credentials = kernel.credentials(tid);
                if wanted & O_NOATIME != 0
                    && file.flags & O_NOATIME == 0
                    && !credentials.owns_or_capable(inode.uid)
                {
                    return Err(Errno::EPERM);
                }
                if wanted & O_DIRECT != 0 && inode.file_type() != S_IFIFO && !file.direct_io {
                    return Err(Errno::EINVAL);
                }
            }
            // A change of `FASYNC` goes to the file's `fasync`, if it has one, which sets the flag.
            if (wanted ^ file.flags) & FASYNC != 0 {
                drop(file);
                let _ = super::ioctl::set_async(
                    kernel,
                    tid,
                    &descriptor.file,
                    wanted & FASYNC != 0,
                    fd,
                );
                file = descriptor.file.borrow_mut();
            }
            // `O_APPEND` cannot be taken off an append-only file; nothing here is append-only.
            file.flags = (file.flags & !SETTABLE_FLAGS) | (wanted & SETTABLE_FLAGS);
            Ok(0)
        }
        F_CREATED_QUERY => Ok(u64::from(descriptor.file.borrow().created)),
        F_DUPFD_QUERY => {
            let other = files.borrow().file(argument as i32)?;
            Ok(u64::from(alloc::rc::Rc::ptr_eq(&other, &descriptor.file)))
        }
        // No lease can be taken, so a file never holds one.
        F_GETLEASE if !path_only => Ok(u64::from(F_UNLCK)),
        F_GETOWN if !path_only => {
            let owner = descriptor.file.borrow().owner;
            Ok(owner.map_or(0, |owner| match owner_lives(kernel, &owner) {
                true if owner.kind == OwnerKind::Group => -owner.pid,
                true => owner.pid,
                false => 0,
            }) as i64 as u64)
        }
        F_SETOWN if !path_only => {
            // `f_setown`: below zero names a group.
            let who = argument as i32;
            if who == i32::MIN {
                return Err(Errno::EINVAL);
            }
            let kind = if who < 0 {
                OwnerKind::Group
            } else {
                OwnerKind::Process
            };
            set_owner(kernel, tid, &descriptor.file, who.abs(), kind)
        }
        F_GETOWN_EX if !path_only => {
            let owner = descriptor.file.borrow().owner.unwrap_or_default();
            let pid = if owner_lives(kernel, &owner) {
                owner.pid
            } else {
                0
            };
            let kind = match owner.kind {
                OwnerKind::Thread => F_OWNER_TID,
                OwnerKind::Process => F_OWNER_PID,
                OwnerKind::Group => F_OWNER_PGRP,
            };
            let mut bytes = [0u8; size_of::<f_owner_ex>()];
            bytes[..4].copy_from_slice(&kind.to_le_bytes());
            bytes[4..].copy_from_slice(&pid.to_le_bytes());
            guest(kernel, cx, tid).write(argument, &bytes)?;
            Ok(0)
        }
        F_SETOWN_EX if !path_only => {
            let bytes = guest(kernel, cx, tid).read(argument, size_of::<f_owner_ex>() as u64)?;
            let word = |at: usize| i32::from_le_bytes(bytes[at..at + 4].try_into().expect("4"));
            let kind = match word(0) as u32 {
                F_OWNER_TID => OwnerKind::Thread,
                F_OWNER_PID => OwnerKind::Process,
                F_OWNER_PGRP => OwnerKind::Group,
                _ => return Err(Errno::EINVAL),
            };
            set_owner(kernel, tid, &descriptor.file, word(4), kind)
        }
        F_GETSIG if !path_only => Ok(descriptor
            .file
            .borrow()
            .owner
            .map_or(0, |owner| owner.signal as u64)),
        F_SETSIG if !path_only => {
            let signal = argument as i32;
            if !(0..=crate::process::SIGNALS as i32).contains(&signal) {
                return Err(Errno::EINVAL);
            }
            let mut file = descriptor.file.borrow_mut();
            file.owner.get_or_insert_default().signal = signal;
            Ok(0)
        }
        // Only pipes have a size (`pipe_fcntl`).
        F_GETPIPE_SZ | F_SETPIPE_SZ if !path_only => {
            let node = descriptor.file.borrow().node.clone();
            let may_exceed = kernel.credentials(tid).has(CAP_SYS_RESOURCE);
            let mut inode = node.borrow_mut();
            let Content::Pipe(pipe) = &mut inode.content else {
                return Err(Errno::EBADF);
            };
            if command == F_GETPIPE_SZ {
                return Ok(pipe.capacity as u64 * PAGE_SIZE);
            }
            // `pipe_set_size`: rounded up to pages, within the largest size, not below what it
            // holds.
            let size = crate::pipe::round_size(u64::from(argument as u32)).ok_or(Errno::EINVAL)?;
            if size > crate::pipe::MAX_SIZE && !may_exceed {
                return Err(Errno::EPERM);
            }
            let slots = (size / PAGE_SIZE) as usize;
            if slots < pipe.occupancy() {
                return Err(Errno::EBUSY);
            }
            pipe.capacity = slots;
            drop(inode);
            kernel.wake(super::pipe::channel(&node));
            Ok(size)
        }
        F_GET_RW_HINT if !path_only => {
            let hint = descriptor.file.borrow().node.borrow().write_hint;
            guest(kernel, cx, tid).write(argument, &hint.to_le_bytes())?;
            Ok(0)
        }
        F_SET_RW_HINT if !path_only => {
            let node = descriptor.file.borrow().node.clone();
            let credentials = kernel.credentials(tid);
            if !credentials.owns_or_capable(node.borrow().uid) {
                return Err(Errno::EPERM);
            }
            let hint: u64 = guest(kernel, cx, tid).read_value(argument)?;
            if hint > u64::from(RWH_WRITE_LIFE_EXTREME) {
                return Err(Errno::EINVAL);
            }
            node.borrow_mut().write_hint = hint;
            Ok(0)
        }
        F_ADD_SEALS if !path_only => {
            super::memfd::add_seals(kernel, &descriptor.file.borrow(), argument)
        }
        F_GET_SEALS if !path_only => super::memfd::get_seals(&descriptor.file.borrow()),
        _ if path_only => Err(Errno::EBADF),
        // Leases, directory notification, the owner's ids (`CONFIG_CHECKPOINT_RESTORE`) and
        // delegations are not built.
        F_SETLEASE
        | F_NOTIFY
        | F_GETOWNER_UIDS
        | crate::linux_extra::F_GETDELEG
        | crate::linux_extra::F_SETDELEG => Err(kernel.counters.refuse(
            cx.host,
            reason::FCNTL_NOT_BUILT,
            command.into(),
            Errno::EINVAL,
        )),
        _ => Err(Errno::EINVAL),
    }
}

/// Whether a number is in use by a thread, process, group or session (Linux's `find_vpid`).
fn pid_in_use(kernel: &Kernel, pid: i32) -> bool {
    kernel.threads.contains_key(&pid)
        || kernel
            .processes
            .values()
            .any(|process| process.pid == pid || process.group == pid || process.session == pid)
}

/// Whether a task of the owner's kind lives under its number (`pid_task`).
fn owner_lives(kernel: &Kernel, owner: &Owner) -> bool {
    match owner.kind {
        OwnerKind::Thread => kernel.threads.contains_key(&owner.pid),
        OwnerKind::Process => kernel.processes.contains_key(&owner.pid),
        OwnerKind::Group => kernel
            .processes
            .values()
            .any(|process| process.group == owner.pid),
    }
}

/// `f_setown` and `f_setown_ex`: a number in use, or 0 for none, becomes the file's owner; the signal
/// stays.
fn set_owner(
    kernel: &Kernel,
    tid: Tid,
    file: &FileRef,
    pid: i32,
    kind: OwnerKind,
) -> Result<u64, Errno> {
    if pid != 0 && !pid_in_use(kernel, pid) {
        return Err(Errno::ESRCH);
    }
    set_owner_of(kernel, tid, file, pid, kind);
    Ok(0)
}

/// Linux's `f_modown`: the owner, with the caller's users when it names one.
pub(super) fn set_owner_of(kernel: &Kernel, tid: Tid, file: &FileRef, pid: i32, kind: OwnerKind) {
    let credentials = kernel.credentials(tid);
    let mut file = file.borrow_mut();
    let owner = file.owner.get_or_insert_default();
    owner.pid = pid;
    owner.kind = kind;
    if pid != 0 {
        owner.uid = credentials.uid;
        owner.euid = credentials.euid;
    }
}

/// `fsync` and `fdatasync`: everything is in memory, so there is nothing to wait for. Files with no
/// way to be synced refuse.
pub(super) fn fsync(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, ..]: Args,
) -> Result<u64, Errno> {
    sync_file(kernel, cx, tid, fd, false)
}

pub(super) fn fdatasync(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, ..]: Args,
) -> Result<u64, Errno> {
    sync_file(kernel, cx, tid, fd, true)
}

/// `vfs_fsync`: a file on the backend is synced there, its mapped pages written back first
/// (`fuse_fsync`); tmpfs has nothing to write.
fn sync_file(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    fd: u64,
    data_only: bool,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let handle = file.borrow().backend.clone();
    if let Some(handle) = &handle {
        let node = file.borrow().node.clone();
        crate::shared::backend_write_back(kernel, cx, &node, 0, u64::MAX, true)?;
        crate::fs::backend::fsync(handle, data_only)?;
        return Ok(0);
    }
    // Only tmpfs's files and directories have a sync operation; pipes, devices and the process
    // information tree have none.
    let file = file.borrow();
    let inode = file.node.borrow();
    match inode.content {
        Content::Pipe(_)
        | Content::Special
        | Content::Proc(_)
        | Content::Pidfd(..)
        | Content::Anon
        | Content::Mqueue(_) => Err(Errno::EINVAL),
        _ if inode.is_secret() => Err(Errno::EINVAL),
        _ => Ok(0),
    }
}

/// `syncfs`: any open file names a file system, and there is nothing to wait for.
pub(super) fn syncfs(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, ..]: Args,
) -> Result<u64, Errno> {
    if file(kernel, tid, fd)?.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    Ok(0)
}

pub(super) fn sync(_: &mut Kernel, _: &mut Context<'_>, _: Tid, _: Args) -> Result<u64, Errno> {
    Ok(0)
}

/// What `poll` reports for an open file: Linux's `DEFAULT_POLLMASK` for files without a poll operation,
/// or what the file's own says. The host's pipes always have room.
fn poll_mask(kernel: &Kernel, tid: Tid, file: &FileRef) -> u16 {
    use crate::fs::proc::{Entry, ProcessFile};
    if super::tty::end(file).is_some() {
        return super::tty::poll(file);
    }
    let file = file.borrow();
    if file.is_path() {
        return POLLNVAL as u16;
    }
    let node = file.node.borrow();
    match &node.content {
        Content::Pipe(pipe) => crate::pipe::poll(pipe, &file),
        // `random_poll`: the generator is ready from the start here. `/dev/urandom` has no poll.
        Content::Special if Device::from_number(node.rdev) == Some(Device::Random) => {
            (POLLIN | POLLRDNORM) as u16
        }
        // `mounts_poll`: the mount table cannot change within the container.
        Content::Proc(Entry::ProcessFile(_, ProcessFile::Mounts)) => (POLLIN | POLLRDNORM) as u16,
        Content::Pidfd(..) => {
            drop(node);
            super::pidfd::poll(kernel, &file)
        }
        Content::Mqueue(queue) => queue.poll(),
        Content::Anon => match (super::signalfd::mask_of(&file), super::timer::of(&file)) {
            (Some(mask), _) => super::signalfd::poll(kernel, tid, mask),
            (_, Some(timer)) => super::timer::poll(kernel, timer),
            _ => 0,
        },
        _ => (POLLIN | POLLOUT | POLLRDNORM | POLLWRNORM) as u16,
    }
}

/// A wait's timeout as `poll_select_set_timeout` checks it: its length.
fn read_timeout(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    timeout: u64,
) -> Result<Option<u64>, Errno> {
    if timeout == 0 {
        return Ok(None);
    }
    let value: __kernel_timespec = guest(kernel, cx, tid).read_value(timeout)?;
    if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
        return Err(Errno::EINVAL);
    }
    Ok(Some(
        (value.tv_sec as u64)
            .saturating_mul(1_000_000_000)
            .saturating_add(value.tv_nsec as u64),
    ))
}

/// The first run of `ppoll` or `pselect6`: when the wait ends (kept in the call's slot as the time
/// asked for, and with `select_estimate_accuracy`'s slack as when the thread is woken), then the
/// temporary signal mask (`set_user_sigmask`).
fn start_wait(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    duration: Option<u64>,
    mask: u64,
    mask_size: u64,
) -> Result<(), Errno> {
    let now = cx.monotonic();
    let new_mask = if mask != 0 {
        if mask_size != 8 {
            return Err(Errno::EINVAL);
        }
        Some(guest(kernel, cx, tid).read_value::<u64>(mask)?)
    } else {
        None
    };
    let thread = kernel.thread_mut(tid);
    if let Some(duration) = duration {
        let until = now.saturating_add(duration);
        let slack = if thread.timer_slack == 0 {
            0
        } else {
            // 0.1% of the time left (0.5% for a nice thread), at most 100 ms, at least the slack.
            const MAX_SLACK: u64 = 100_000_000;
            let divisor = if thread.schedule.nice > 0 { 200 } else { 1000 };
            (duration / divisor).min(MAX_SLACK).max(thread.timer_slack)
        };
        thread.call.progress = until;
        thread.call.deadline = Some(until.saturating_add(slack));
    }
    if let Some(new_mask) = new_mask {
        thread.saved_mask = Some(thread.signal_mask);
        thread.signal_mask = new_mask & !crate::signal::UNBLOCKABLE;
    }
    thread.call.started = true;
    Ok(())
}

/// After a run of `ppoll` or `pselect6` found nothing ready: it ends with none when its time is up,
/// is interrupted by a signal, or parks until a file's readiness changes. A call watching no file
/// (musl's `pause`) parks until its time or a signal only, as no wait queue holds it in Linux.
fn nothing_ready(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    watching: bool,
) -> Option<Result<u64, Errno>> {
    let call = kernel.threads[&tid].call;
    if call
        .deadline
        .is_some_and(|_| call.progress <= cx.monotonic())
    {
        return Some(Ok(0));
    }
    if kernel.signal_pending(tid) {
        return Some(Err(Errno::ERESTARTNOHAND));
    }
    let channel = if watching {
        crate::process::Channel::Readiness
    } else {
        crate::process::Channel::Nothing
    };
    kernel.park(tid, channel);
    None
}

/// As Linux's `poll_select_finish`: the time left goes back into a non-zero timeout, whatever the
/// result (so a restarted call waits only what is left), a failure to write it is ignored, and the
/// signal mask goes back unless a handler is to run first.
fn finish_wait(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    timeout: u64,
    result: Option<Result<u64, Errno>>,
) -> super::Outcome {
    let Some(result) = result else {
        return super::Outcome::Parked;
    };
    let call = kernel.threads[&tid].call;
    if timeout != 0 && call.deadline.is_some() {
        let left = call.progress.saturating_sub(cx.monotonic());
        let value = __kernel_timespec {
            tv_sec: (left / 1_000_000_000) as i64,
            tv_nsec: (left % 1_000_000_000) as i64,
        };
        let _ = guest(kernel, cx, tid).write_value(timeout, &value);
    }
    if result != Err(Errno::ERESTARTNOHAND)
        && let Some(saved) = kernel.thread_mut(tid).saved_mask.take()
    {
        kernel.thread_mut(tid).signal_mask = saved;
    }
    result.into()
}

pub(super) fn ppoll(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fds, count, timeout, mask, mask_size, _]: Args,
) -> super::Outcome {
    if !kernel.threads[&tid].call.started {
        // The timeout first; a bad one ends the call before anything else. A zero timeout never
        // waits.
        let duration = match read_timeout(kernel, cx, tid, timeout) {
            Ok(duration) => duration,
            Err(error) => return Err(error).into(),
        };
        if let Err(error) = start_wait(kernel, cx, tid, duration, mask, mask_size) {
            return Err(error).into();
        }
    }
    let result = match ppoll_typed(kernel, cx, tid, fds, count) {
        Ok((0, watching)) => nothing_ready(kernel, cx, tid, watching),
        result => Some(result.map(|(ready, _)| ready)),
    };
    finish_wait(kernel, cx, tid, timeout, result)
}

pub(super) fn pselect6(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [count, read, write, except, timeout, mask_pack]: Args,
) -> super::Outcome {
    if !kernel.threads[&tid].call.started {
        // The signal mask's address and size first, then the timeout.
        let (mask, mask_size) = if mask_pack != 0 {
            match guest(kernel, cx, tid).read(mask_pack, 16) {
                Ok(bytes) => (
                    u64::from_le_bytes(bytes[..8].try_into().expect("8 bytes")),
                    u64::from_le_bytes(bytes[8..].try_into().expect("8 bytes")),
                ),
                Err(error) => return Err(error).into(),
            }
        } else {
            (0, 0)
        };
        let duration = match read_timeout(kernel, cx, tid, timeout) {
            Ok(duration) => duration,
            Err(error) => return Err(error).into(),
        };
        if let Err(error) = start_wait(kernel, cx, tid, duration, mask, mask_size) {
            return Err(error).into();
        }
    }
    let result = match select_typed(kernel, cx, tid, count, [read, write, except]) {
        Ok((None, watching)) => nothing_ready(kernel, cx, tid, watching).map(|result| {
            // A timeout writes back the sets, empty.
            if result == Ok(0) {
                select_typed(kernel, cx, tid, count, [read, write, except]).and(Ok(0))
            } else {
                result
            }
        }),
        Ok((Some(ready), _)) => Some(Ok(ready)),
        Err(error) => Some(Err(error)),
    };
    finish_wait(kernel, cx, tid, timeout, result)
}

/// One look at `pselect6`'s sets, as Linux's `core_sys_select`: Some(count) and the sets written
/// back when something is ready, None when nothing is (the sets are then written back empty only
/// when the call returns); and whether any descriptor is asked about.
fn select_typed(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    count: u64,
    sets: [u64; 3],
) -> Result<(Option<u64>, bool), Errno> {
    let count = count as i32;
    if count < 0 {
        return Err(Errno::EINVAL);
    }
    let count = (count as u32).min(kernel.files(tid).capacity()) as u64;
    let bytes = count.div_ceil(64) * 8;
    let mut wanted = [const { Vec::new() }; 3];
    for (index, address) in sets.iter().enumerate() {
        wanted[index] = if *address == 0 {
            vec![0; bytes as usize]
        } else {
            guest(kernel, cx, tid).read(*address, bytes)?
        };
    }
    let asked = |set: &[u8], fd: u64| set[(fd / 8) as usize] & (1 << (fd % 8)) != 0;
    let files = kernel.files(tid);
    let mut watching = false;
    for fd in 0..count {
        if wanted.iter().any(|set| asked(set, fd)) {
            if files.get(fd as i32).is_err() {
                return Err(Errno::EBADF);
            }
            watching = true;
        }
    }
    // Linux's `POLLIN_SET`, `POLLOUT_SET` and `POLLEX_SET`.
    let kinds = [
        POLLRDNORM | POLLRDBAND | POLLIN | POLLHUP | POLLERR | POLLNVAL,
        POLLWRBAND | POLLWRNORM | POLLOUT | POLLERR | POLLNVAL,
        POLLPRI | POLLNVAL,
    ];
    let mut results = [const { Vec::new() }; 3];
    let mut ready = 0;
    for (index, set) in wanted.iter().enumerate() {
        let mut result = vec![0u8; bytes as usize];
        for fd in 0..count {
            if !asked(set, fd) {
                continue;
            }
            // `select_poll_one`: an `O_PATH` descriptor is not found, so it is invalid, which every
            // set reports.
            let mask = poll_mask(kernel, tid, &files.file(fd as i32)?);
            if u32::from(mask) & kinds[index] != 0 {
                result[(fd / 8) as usize] |= 1 << (fd % 8);
                ready += 1;
            }
        }
        results[index] = result;
    }
    drop(files);
    if ready == 0 {
        let call = kernel.threads[&tid].call;
        let over = call.deadline.is_some() && call.progress <= cx.monotonic();
        if !over {
            return Ok((None, watching));
        }
    }
    for (address, result) in sets.iter().zip(&results) {
        if *address != 0 {
            guest(kernel, cx, tid).write(*address, result)?;
        }
    }
    Ok((Some(ready), watching))
}

/// One look at `ppoll`'s descriptors: how many are ready, each one's events written back; and
/// whether any descriptor is watched (a negative one is not).
fn ppoll_typed(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    fds: u64,
    count: u64,
) -> Result<(u64, bool), Errno> {
    if count > nofile_limit(kernel, tid) {
        return Err(Errno::EINVAL);
    }
    let mut entries = guest(kernel, cx, tid).read(fds, count * 8)?;
    let mut ready = 0;
    let mut watching = false;
    for entry in entries.as_chunks_mut::<8>().0 {
        let fd = i32::from_le_bytes(entry[..4].try_into().expect("4 bytes"));
        watching |= fd >= 0;
        let events = u16::from_le_bytes([entry[4], entry[5]]);
        let revents = if fd < 0 {
            0
        } else {
            match kernel.files(tid).file(fd) {
                Ok(file) => {
                    poll_mask(kernel, tid, &file) & (events | (POLLERR | POLLHUP | POLLNVAL) as u16)
                }
                Err(_) => POLLNVAL as u16,
            }
        };
        entry[6..8].copy_from_slice(&revents.to_le_bytes());
        if revents != 0 {
            ready += 1;
        }
    }
    // Only each entry's `revents` is stored, one store each.
    let stores: Vec<(u64, &[u8])> = entries
        .as_chunks::<8>()
        .0
        .iter()
        .enumerate()
        .map(|(index, entry)| (8 * index as u64 + 6, &entry[6..]))
        .collect();
    if guest(kernel, cx, tid).put_each(fds, &stores) < stores.len() {
        return Err(Errno::EFAULT);
    }
    Ok((ready, watching))
}

/// `cachestat`: how many of a file's pages are in memory. Only tmpfs files hold pages here (as in
/// Linux, where devices, `/proc` and directories keep none); none is ever dirty, under writeback or
/// evicted, as tmpfs without swap (measured).
pub(super) fn cachestat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, range, out, flags, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, u64::from(fd as u32))?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    let range: cachestat_range = guest(kernel, cx, tid).read_value(range)?;
    let (offset, length) = (range.off, range.len);
    // Linux's `can_do_cachestat`: opened for writing, the owner, or allowed to write.
    let node = file.borrow().node.clone();
    let allowed = file.borrow().writable() || {
        let credentials = kernel.credentials(tid);
        let inode = node.borrow();
        credentials.owns_or_capable(inode.uid)
            || !inode.fs.read_only() && inode.permits(credentials, W_OK, true)
    };
    if !allowed {
        return Err(Errno::EPERM);
    }
    if flags as u32 != 0 {
        return Err(Errno::EINVAL);
    }
    let first = offset / PAGE_SIZE;
    let last = if length == 0 {
        u64::MAX
    } else {
        offset.wrapping_add(length).wrapping_sub(1) / PAGE_SIZE
    };
    let mut cached = 0;
    if let Content::Regular(regular) = &node.borrow().content
        && first <= last
    {
        if regular.is_mapped() {
            kernel
                .counters
                .count(cx.host, reason::RESIDENCY_NOT_TRACKED, 0);
        }
        cached = (regular.pages.range(first..=last).count()
            + regular.allocated.range(first..=last).count()) as u64;
    }
    let stat = cachestat {
        nr_cache: cached,
        nr_dirty: 0,
        nr_writeback: 0,
        nr_evicted: 0,
        nr_recently_evicted: 0,
    };
    guest(kernel, cx, tid).write_value(out, &stat)?;
    Ok(0)
}
