//! Reading, writing and moving around in open files, and what descriptors refer to.

use alloc::vec;
use alloc::vec::Vec;

use wcontainers_abi::{PAGE_SIZE, Permissions, Stream};

use super::{Args, guest};
use crate::counters::reason;
use crate::file::{Descriptor, FileRef};
use crate::fs::dev::Device;
use crate::fs::{Content, FIRST_POSITION};
use crate::linux::general::*;
use crate::linux::ioctl::{FIOCLEX, FIONBIO, FIONCLEX, TCGETS, TIOCGWINSZ};
use crate::process::Tid;
use crate::user::{Guest, access_ok};
use crate::{Context, Errno, Kernel};

/// The most one read or write moves, as Linux's `MAX_RW_COUNT`.
pub(super) const MAX_RW_COUNT: u64 = 0x7fff_f000;

/// The most segments `readv` and `writev` take (`UIO_MAXIOV`).
pub(super) const MAX_SEGMENTS: u64 = 1024;

/// The end-of-directory position tmpfs gives (`DIR_OFFSET_EOD`).
const END_OF_DIRECTORY: u64 = i32::MAX as u64;

fn file(kernel: &Kernel, tid: Tid, fd: u64) -> Result<FileRef, Errno> {
    kernel.process_of(tid).files.file(fd as i32)
}

pub(super) fn read(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, count, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
    if !access_ok(buffer, count) {
        return Err(Errno::EFAULT);
    }
    if !matches!(file.borrow().node.borrow().content, Content::HostStream(_)) {
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
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
    if !access_ok(buffer, count) {
        return Err(Errno::EFAULT);
    }
    if !matches!(file.borrow().node.borrow().content, Content::HostStream(_)) {
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
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
    positioned(&file, offset)?;
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
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
    positioned(&file, offset)?;
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
    use crate::linux_extra::{RWF_ATOMIC, RWF_DONTCACHE, RWF_NOAPPEND};
    let flags = flags as u32;
    let current = offset as i64 == -1;
    if !current && (offset as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    let file = file(kernel, tid, fd)?;
    if !current {
        positioned(&file, offset)?;
    }
    let allowed = if writing {
        file.borrow().writable()
    } else {
        file.borrow().readable()
    };
    if !allowed {
        return Err(Errno::EBADF);
    }
    let parts = segments(&mut guest(kernel, cx, tid), vector, count)?;
    if parts.iter().all(|(_, length)| *length == 0) {
        return Ok(0);
    }
    let known = RWF_HIPRI | RWF_DSYNC | RWF_SYNC | RWF_NOWAIT | RWF_APPEND | RWF_NOAPPEND;
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
    if position_in.checked_add(length).is_none() || position_out.checked_add(length).is_none() {
        return Err(Errno::EOVERFLOW);
    }
    let size_in = node_in.borrow().size();
    let mut count = if position_in >= size_in {
        0
    } else {
        length.min(size_in - position_in)
    };
    count = write_limit(kernel, cx, tid, position_out, count)?;
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
    let written = write_bytes(kernel, cx, tid, &output, &bytes, Some(position_out), false)?;
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
    if file.borrow().node.borrow().file_type() != S_IFREG || (length as i64) < 0 {
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

/// The checks for a read or write at a given offset: a negative offset is invalid, and a pipe has none.
fn positioned(file: &FileRef, _offset: u64) -> Result<(), Errno> {
    if matches!(file.borrow().node.borrow().content, Content::HostStream(_)) {
        return Err(Errno::ESPIPE);
    }
    Ok(())
}

/// Linux's `generic_write_check_limits` for a write of `count` bytes to a regular file at `offset`:
/// starting at or past the file size limit is `SIGXFSZ` and `EFBIG`, and a write reaching it is
/// shortened. Nothing is checked for a write of nothing.
pub(super) fn write_limit(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    offset: u64,
    count: u64,
) -> Result<u64, Errno> {
    if count == 0 {
        return Ok(0);
    }
    let limit = kernel.process_of(tid).limits[RLIMIT_FSIZE as usize].current;
    if offset >= limit {
        // A signal that cannot be acted on is counted where it is sent; the write fails either way.
        let _ = kernel.send_to_self(cx, tid, SIGXFSZ);
        return Err(Errno::EFBIG);
    }
    let largest = i64::MAX as u64;
    if offset >= largest {
        return Err(Errno::EFBIG);
    }
    Ok(count.min(limit - offset).min(largest - offset))
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
            let _ = kernel.send_to_self(cx, tid, SIGXFSZ);
            return Err(Errno::EFBIG);
        }
        if new_size > i64::MAX as u64 {
            return Err(Errno::EFBIG);
        }
    }
    Ok(())
}

/// Linux's `rw_verify_area`: the range must end within the largest file offset.
fn verify_area(offset: u64, count: u64) -> Result<(), Errno> {
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
    let mut out = Vec::with_capacity(count as usize);
    let mut total: u64 = 0;
    for chunk in bytes.as_chunks::<16>().0 {
        let base = u64::from_le_bytes(chunk[..8].try_into().expect("8 bytes"));
        let length = u64::from_le_bytes(chunk[8..].try_into().expect("8 bytes"));
        if (length as i64) < 0 {
            return Err(Errno::EINVAL);
        }
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
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
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
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
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
    positioned(&file, offset)?;
    if !file.borrow().readable() {
        return Err(Errno::EBADF);
    }
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
    positioned(&file, offset)?;
    if !file.borrow().writable() {
        return Err(Errno::EBADF);
    }
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
    if !matches!(file.borrow().node.borrow().content, Content::HostStream(_)) {
        verify_area(offset.unwrap_or(file.borrow().offset), total)?;
    }
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
        let mut allowed = write_limit(kernel, cx, tid, start, total)?;
        for (_, length) in &mut parts {
            *length = (*length).min(allowed);
            allowed -= *length;
        }
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
fn file_bytes(
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
        Content::Proc(_) if inode.is_directory() => return Err(Errno::EISDIR),
        Content::Regular(regular) => window(&regular.data),
        Content::Proc(_) => window(file.borrow().contents.as_deref().unwrap_or_default()),
        Content::Special => match Device::from_number(inode.rdev) {
            Some(Device::Null) => Vec::new(),
            Some(Device::Zero | Device::Full) => vec![0; count.min(PAGE_SIZE * 16) as usize],
            Some(Device::Random | Device::Urandom) => {
                let mut bytes = vec![0; count.min(PAGE_SIZE * 16) as usize];
                cx.host.random(&mut bytes);
                bytes
            }
            _ => return Err(Errno::ENXIO),
        },
        Content::HostStream(_) | Content::Symlink(_) => return Err(Errno::EINVAL),
    })
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
    // The zero devices clear the buffer in place and keep no position. `/dev/zero` stops at the first
    // short clear (Linux's `read_zero`); `/dev/full` clears again from where it stopped until nothing
    // more is cleared (`read_iter_zero`).
    let zero_device = {
        let file = file.borrow();
        let inode = file.node.borrow();
        match inode.content {
            Content::Special => Device::from_number(inode.rdev)
                .filter(|device| matches!(device, Device::Zero | Device::Full)),
            _ => None,
        }
    };
    if let Some(device) = zero_device {
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
    let offset = at.unwrap_or(file.borrow().offset);
    let bytes = file_bytes(cx, file, count, offset)?;
    let written = guest(kernel, cx, tid).write_partial(buffer, &bytes);
    if written == 0 && !bytes.is_empty() {
        return Err(Errno::EFAULT);
    }
    if at.is_none() {
        file.borrow_mut().offset = offset + written as u64;
    }
    Ok(written as u64)
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
    let stream = match &node.borrow().content {
        Content::HostStream(stream) => Some(*stream),
        _ => None,
    };
    if let Some(stream) = stream {
        return write_host_stream(kernel, cx, tid, stream, buffer, count);
    }
    let (regular, device) = {
        let inode = node.borrow();
        (
            matches!(inode.content, Content::Regular(_)),
            Device::from_number(inode.rdev),
        )
    };
    let bytes = if regular {
        let offset = if append {
            node.borrow().size()
        } else {
            at.unwrap_or(file.borrow().offset)
        };
        let count = write_limit(kernel, cx, tid, offset, count)?;
        // As much of the buffer as can be read.
        let readable = guest(kernel, cx, tid).accessible(buffer, count, Permissions::READ);
        if readable == 0 && count > 0 {
            return Err(Errno::EFAULT);
        }
        guest(kernel, cx, tid).read(buffer, readable)?
    } else if matches!(device, Some(Device::Random | Device::Urandom)) {
        // The random devices take the bytes in; the rest only count them.
        guest(kernel, cx, tid).read(buffer, count.min(PAGE_SIZE))?;
        return write_bytes(kernel, cx, tid, file, &[], at, append).map(|_| count);
    } else {
        return write_bytes(kernel, cx, tid, file, &[], at, append).map(|_| count);
    };
    write_bytes(kernel, cx, tid, file, &bytes, at, append)
}

/// Writes bytes the kernel holds to a file, at its position or at `at`. Devices that take anything
/// report nothing written here; the caller counts what it offered.
fn write_bytes(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    bytes: &[u8],
    at: Option<u64>,
    append: bool,
) -> Result<u64, Errno> {
    let node = file.borrow().node.clone();
    let now = kernel.now(cx);
    let may_keep_privileges = kernel
        .process_of(tid)
        .credentials
        .has(crate::credentials::CAP_FSETID);
    let mut inode = node.borrow_mut();
    let rdev = inode.rdev;
    let regular = match &mut inode.content {
        Content::Regular(regular) => regular,
        Content::HostStream(stream) => {
            cx.host.output(*stream, bytes);
            return Ok(bytes.len() as u64);
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
        Content::Directory(_) | Content::Proc(_) | Content::Symlink(_) => {
            return Err(Errno::EINVAL);
        }
    };
    let offset = if append {
        regular.data.len() as u64
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
    if !bytes.is_empty() {
        // The times change and privileges go before the seals are looked at, as in tmpfs.
        let allowed = regular.writable(offset, bytes.len() as u64);
        if let Ok(allowed) = allowed {
            bytes = &bytes[..allowed as usize];
            regular.write(offset as usize, bytes);
        }
        inode.mtime = now;
        inode.ctime = now;
        // Writing clears the set-user and set-group bits for a writer without `CAP_FSETID`, and file
        // capabilities always, as Linux's `file_remove_privs` does.
        inode.drop_privileges(!may_keep_privileges);
        allowed?;
    }
    drop(inode);
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
    guest(kernel, cx, tid).write(offset_address, &offset.to_le_bytes())?;
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
        positioned(&input, 0)?;
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
    if (output.borrow().offset as i64)
        .checked_add(count as i64)
        .is_none()
    {
        return Err(Errno::EINVAL);
    }
    if output.borrow().flags & O_APPEND != 0 {
        return Err(Errno::EINVAL);
    }
    if input.borrow().node.borrow().is_directory() {
        return Err(Errno::EINVAL);
    }
    let offset = position as u64;
    let mut bytes = file_bytes(cx, &input, count, offset)?;
    if matches!(output.borrow().node.borrow().content, Content::Regular(_)) {
        let at = output.borrow().offset;
        let allowed = write_limit(kernel, cx, tid, at, bytes.len() as u64)?;
        bytes.truncate(allowed as usize);
    }
    let written = write_bytes(kernel, cx, tid, &output, &bytes, None, false)?;
    match explicit {
        Some(offset) => *offset = (position as u64 + written) as i64,
        None => input.borrow_mut().offset = offset + written,
    }
    Ok(written)
}

/// The host takes a stream's bytes at once, so this is a write to an empty pipe, as the reference's
/// standard streams are: whole pages from the buffer, and the first page that cannot be read in full
/// ends the write and is dropped (`EFAULT` if nothing was written), as Linux's `pipe_write` does.
fn write_host_stream(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    stream: Stream,
    buffer: u64,
    count: u64,
) -> Result<u64, Errno> {
    let mut done = 0;
    while done < count {
        let at = buffer + done;
        let length = (count - done).min(PAGE_SIZE);
        match guest(kernel, cx, tid).read(at, length) {
            Ok(bytes) => cx.host.output(stream, &bytes),
            Err(error) => return if done == 0 { Err(error) } else { Ok(done) },
        }
        done += length;
    }
    Ok(done)
}

pub(super) fn lseek(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, offset, whence, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    let mut file = file.borrow_mut();
    if file.is_path() {
        return Err(Errno::EBADF);
    }
    let offset = offset as i64;
    let whence = whence as u32;
    // An unknown origin is refused before the file has its say.
    if whence > SEEK_MAX {
        return Err(Errno::EINVAL);
    }
    let inode = file.node.borrow();
    let new = match &inode.content {
        Content::HostStream(_) => return Err(Errno::ESPIPE),
        Content::Special => match Device::from_number(inode.rdev) {
            // `/dev/null` and `/dev/zero` always seek to 0; the others keep their position.
            Some(Device::Null | Device::Zero) => 0,
            _ => file.offset as i64,
        },
        Content::Regular(regular) => {
            let size = regular.data.len() as i64;
            match whence {
                SEEK_SET => offset,
                SEEK_CUR => (file.offset as i64)
                    .checked_add(offset)
                    .ok_or(Errno::EINVAL)?,
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
        // The process information tree's directories and `/proc/sys`'s files seek as Linux's
        // `generic_file_llseek` does with a size of zero.
        Content::Proc(entry)
            if inode.is_directory()
                || matches!(entry, crate::fs::proc::Entry::Global(index)
                    if crate::fs::proc::GLOBAL[*index].0.starts_with("sys/")) =>
        {
            match whence {
                SEEK_SET | SEEK_END => offset,
                SEEK_CUR => (file.offset as i64)
                    .checked_add(offset)
                    .ok_or(Errno::EINVAL)?,
                _ => return Err(Errno::ENXIO),
            }
        }
        Content::Directory(_) | Content::Proc(_) | Content::Symlink(_) => match whence {
            SEEK_SET => offset,
            SEEK_CUR => (file.offset as i64)
                .checked_add(offset)
                .ok_or(Errno::EINVAL)?,
            _ => return Err(Errno::EINVAL),
        },
    };
    if new < 0 {
        return Err(Errno::EINVAL);
    }
    drop(inode);
    file.offset = new as u64;
    Ok(new as u64)
}

pub(super) fn getdents64(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, buffer, count, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    // The buffer is checked only as each entry is written, after the file is found to be a
    // directory, as Linux's `filldir64` does.
    let count = count as u32 as u64;
    let node = file.borrow().node.clone();
    let position = file.borrow().offset;
    // Every entry from the position on: name, inode number, type, and the position after it.
    let mut entries: Vec<(Vec<u8>, u64, u8, u64)> = Vec::new();
    {
        let inode = node.borrow();
        if !inode.is_directory() {
            return Err(Errno::ENOTDIR);
        }
        let parent_ino = inode
            .place
            .parent
            .as_ref()
            .and_then(|parent| parent.upgrade())
            .map_or(inode.ino, |parent| parent.borrow().ino);
        match &inode.content {
            Content::Directory(directory) => {
                // `.` at 0, `..` at 1, then the entries newest first, each giving the next one's position.
                let newest = directory.at_or_below(u64::MAX).next().map(|(at, _, _)| at);
                if position == 0 {
                    entries.push((b".".to_vec(), inode.ino, DT_DIR as u8, 1));
                }
                if position <= 1 {
                    entries.push((
                        b"..".to_vec(),
                        parent_ino,
                        DT_DIR as u8,
                        newest.unwrap_or(END_OF_DIRECTORY),
                    ));
                }
                let from = if position <= 1 { u64::MAX } else { position };
                if from >= FIRST_POSITION && position != END_OF_DIRECTORY {
                    let listed: Vec<_> = directory.at_or_below(from).collect();
                    for (index, (_, name, child)) in listed.iter().enumerate() {
                        let next = listed
                            .get(index + 1)
                            .map_or(END_OF_DIRECTORY, |(at, _, _)| *at);
                        let child = child.borrow();
                        entries.push((name.to_vec(), child.ino, child.dirent_type(), next));
                    }
                }
            }
            Content::Proc(entry) => {
                let entry = *entry;
                drop(inode);
                let listed = kernel.with_walk(kernel.threads[&tid].process, |walk, _| {
                    let mut out = Vec::new();
                    for (index, (name, child)) in crate::fs::proc::list(walk.proc, &entry)
                        .into_iter()
                        .enumerate()
                    {
                        let child_node = crate::fs::proc::node(walk.proc, None, &name, child);
                        let child_inode = child_node.borrow();
                        out.push((
                            name,
                            child_inode.ino,
                            child_inode.dirent_type(),
                            index as u64 + 3,
                        ));
                    }
                    Ok(out)
                })?;
                let ino = node.borrow().ino;
                if position == 0 {
                    entries.push((b".".to_vec(), ino, DT_DIR as u8, 1));
                }
                if position <= 1 {
                    entries.push((b"..".to_vec(), parent_ino, DT_DIR as u8, 2));
                }
                let skip = position.saturating_sub(2) as usize;
                entries.extend(listed.into_iter().skip(skip));
            }
            _ => return Err(Errno::ENOTDIR),
        }
    }
    let mut out = Vec::new();
    let mut new_position = position;
    for (name, ino, kind, next) in entries {
        let length = (19 + name.len() + 1).next_multiple_of(8);
        if (out.len() + length) as u64 > count {
            if out.is_empty() {
                return Err(Errno::EINVAL);
            }
            break;
        }
        out.extend_from_slice(&ino.to_le_bytes());
        out.extend_from_slice(&next.to_le_bytes());
        out.extend_from_slice(&(length as u16).to_le_bytes());
        out.push(kind);
        out.extend_from_slice(&name);
        out.resize(out.len() + length - 19 - name.len(), 0);
        new_position = next;
    }
    if !out.is_empty() {
        guest(kernel, cx, tid).write(buffer, &out)?;
    }
    if !out.is_empty() || position <= 1 {
        file.borrow_mut().offset = if out.is_empty() {
            position
        } else {
            new_position
        };
    }
    Ok(out.len() as u64)
}

pub(super) fn close(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, ..]: Args,
) -> Result<u64, Errno> {
    kernel.process_of_mut(tid).files.remove(fd as i32)?;
    Ok(0)
}

fn nofile_limit(kernel: &Kernel, tid: Tid) -> u64 {
    kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current
}

pub(super) fn close_range(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [first, last, flags, ..]: Args,
) -> Result<u64, Errno> {
    use crate::linux_extra::{CLOSE_RANGE_CLOEXEC, CLOSE_RANGE_UNSHARE};
    let (first, last, flags) = (first as u32, last as u32, flags as u32);
    if flags & !(CLOSE_RANGE_UNSHARE | CLOSE_RANGE_CLOEXEC) != 0 || first > last {
        return Err(Errno::EINVAL);
    }
    // The table is never shared between processes yet, so unsharing it changes nothing.
    let files = &mut kernel.process_of_mut(tid).files;
    let fds: Vec<i32> = files
        .iter()
        .map(|(fd, _)| fd)
        .filter(|fd| (first..=last).contains(&(*fd as u32)))
        .collect();
    for fd in fds {
        if flags & CLOSE_RANGE_CLOEXEC != 0 {
            files.get_mut(fd)?.close_on_exec = true;
        } else {
            files.remove(fd)?;
        }
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
    let files = &mut kernel.process_of_mut(tid).files;
    let file = files.file(fd as i32)?;
    Ok(files.add(file, false, 0, limit)? as u64)
}

pub(super) fn dup3(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
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
    let files = &mut kernel.process_of_mut(tid).files;
    let file = files.file(old)?;
    files.set(
        new,
        Descriptor {
            file,
            close_on_exec: flags & O_CLOEXEC != 0,
        },
    );
    Ok(new as u64)
}

/// The status flags `F_SETFL` may change.
const SETTABLE_FLAGS: u32 = O_APPEND | O_NONBLOCK | O_NDELAY | O_DIRECT | O_NOATIME;

pub(super) fn fcntl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, command, argument, ..]: Args,
) -> Result<u64, Errno> {
    let fd = fd as i32;
    let limit = nofile_limit(kernel, tid);
    let files = &mut kernel.process_of_mut(tid).files;
    let descriptor = files.get(fd)?.clone();
    let path_only = descriptor.file.borrow().is_path();
    match command as u32 {
        F_DUPFD | F_DUPFD_CLOEXEC => {
            let minimum = argument as i32;
            if minimum < 0 || minimum as u64 >= limit {
                return Err(Errno::EINVAL);
            }
            let close_on_exec = command as u32 == F_DUPFD_CLOEXEC;
            Ok(files.add(descriptor.file, close_on_exec, minimum, limit)? as u64)
        }
        F_GETFD => Ok(u64::from(descriptor.close_on_exec)),
        F_SETFD => {
            files.get_mut(fd)?.close_on_exec = argument & u64::from(FD_CLOEXEC) != 0;
            Ok(0)
        }
        F_GETFL => {
            let file = descriptor.file.borrow();
            Ok(u64::from(file.flags | O_LARGEFILE))
        }
        F_SETFL if !path_only => {
            let mut file = descriptor.file.borrow_mut();
            let wanted = argument as u32;
            // In the order of Linux's `setfl`. Only the owner or a holder of CAP_FOWNER may stop
            // access times; only tmpfs's regular files take direct I/O (pipes read the flag as packet
            // mode).
            {
                let inode = file.node.borrow();
                let credentials = &kernel.processes[&kernel.threads[&tid].process].credentials;
                if wanted & O_NOATIME != 0
                    && file.flags & O_NOATIME == 0
                    && !credentials.owns_or_capable(inode.uid)
                {
                    return Err(Errno::EPERM);
                }
                if wanted & O_DIRECT != 0
                    && inode.file_type() != S_IFIFO
                    && !inode.takes_direct_io()
                {
                    return Err(Errno::EINVAL);
                }
            }
            // `O_APPEND` cannot be taken off an append-only file; nothing here is append-only.
            file.flags = (file.flags & !SETTABLE_FLAGS) | (wanted & SETTABLE_FLAGS);
            Ok(0)
        }
        F_GETLK | F_SETLK | F_SETLKW | F_OFD_GETLK | F_OFD_SETLK | F_OFD_SETLKW if !path_only => {
            kernel
                .counters
                .count(cx.host, reason::LOCKS_NOT_IMPLEMENTED, command);
            Err(Errno::ENOSYS)
        }
        F_ADD_SEALS if !path_only => super::memfd::add_seals(&descriptor.file.borrow(), argument),
        F_GET_SEALS if !path_only => super::memfd::get_seals(&descriptor.file.borrow()),
        _ if path_only => Err(Errno::EBADF),
        _ => {
            kernel
                .counters
                .count(cx.host, reason::FCNTL_UNKNOWN, command);
            Err(Errno::EINVAL)
        }
    }
}

pub(super) fn ioctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, request, argument, ..]: Args,
) -> Result<u64, Errno> {
    let fd = fd as i32;
    let descriptor = kernel.process_of(tid).files.get(fd)?.clone();
    if descriptor.file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    match request as u32 {
        FIOCLEX | FIONCLEX => {
            kernel.process_of_mut(tid).files.get_mut(fd)?.close_on_exec = request as u32 == FIOCLEX;
            Ok(0)
        }
        FIONBIO => {
            let value: i32 = guest(kernel, cx, tid).read_value(argument)?;
            let mut file = descriptor.file.borrow_mut();
            if value != 0 {
                file.flags |= O_NONBLOCK;
            } else {
                file.flags &= !O_NONBLOCK;
            }
            Ok(0)
        }
        // Nothing open is a terminal yet: terminal requests fail as on a file or pipe.
        TCGETS | TIOCGWINSZ => Err(Errno::ENOTTY),
        _ => {
            let is_regular_or_pipe = matches!(
                descriptor.file.borrow().node.borrow().content,
                Content::Regular(_)
                    | Content::HostStream(_)
                    | Content::Directory(_)
                    | Content::Proc(_)
            );
            if !is_regular_or_pipe {
                kernel
                    .counters
                    .count(cx.host, reason::IOCTL_UNKNOWN, request);
            }
            Err(Errno::ENOTTY)
        }
    }
}

/// `fsync` and `fdatasync`: everything is in memory, so there is nothing to wait for. Files with no
/// way to be synced refuse.
pub(super) fn fsync(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [fd, ..]: Args,
) -> Result<u64, Errno> {
    let file = file(kernel, tid, fd)?;
    if file.borrow().is_path() {
        return Err(Errno::EBADF);
    }
    // Only tmpfs's files and directories have a sync operation; pipes, devices and the process
    // information tree have none.
    match file.borrow().node.borrow().content {
        Content::HostStream(_) | Content::Special | Content::Proc(_) => Err(Errno::EINVAL),
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

/// What `poll` reports for an open file. Everything that exists so far is always ready, as Linux's
/// `DEFAULT_POLLMASK` says for files without a poll operation; the host's pipes always have room.
fn poll_mask(file: &FileRef) -> u16 {
    let file = file.borrow();
    if file.is_path() {
        return POLLNVAL as u16;
    }
    match file.node.borrow().content {
        Content::HostStream(_) => (POLLOUT | POLLWRNORM) as u16,
        _ => (POLLIN | POLLOUT | POLLRDNORM | POLLWRNORM) as u16,
    }
}

/// A wait's timeout as `poll_select_set_timeout` checks it: its length and when it ends.
fn read_timeout(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    timeout: u64,
) -> Result<Option<(u64, u64)>, Errno> {
    if timeout == 0 {
        return Ok(None);
    }
    let value: __kernel_timespec = guest(kernel, cx, tid).read_value(timeout)?;
    if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
        return Err(Errno::EINVAL);
    }
    let duration = (value.tv_sec as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(value.tv_nsec as u64);
    Ok(Some((
        duration,
        cx.host.monotonic_nanoseconds().saturating_add(duration),
    )))
}

/// As Linux's `poll_select_finish`: the time left goes back into a non-zero timeout, whatever the
/// result, and a failure to write it is ignored. A wait that parks will time out with none left.
fn finish_wait(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    timeout: u64,
    until: Option<(u64, u64)>,
    result: Result<Option<u64>, Errno>,
) -> super::Outcome {
    if let Some((duration, until)) = until
        && duration != 0
    {
        let left = match result {
            Ok(Some(_)) | Err(_) => until.saturating_sub(cx.host.monotonic_nanoseconds()),
            Ok(None) => 0,
        };
        let value = __kernel_timespec {
            tv_sec: (left / 1_000_000_000) as i64,
            tv_nsec: (left % 1_000_000_000) as i64,
        };
        let _ = guest(kernel, cx, tid).write_value(timeout, &value);
    }
    match result {
        Ok(Some(ready)) => super::Outcome::Return(ready),
        Ok(None) => super::Outcome::Parked,
        Err(error) => Err(error).into(),
    }
}

/// Parks a waiting thread until its timeout, or for ever (until a signal, K2).
fn park_until(kernel: &mut Kernel, tid: Tid, until: Option<u64>) {
    kernel.thread_mut(tid).parked = Some(match until {
        Some(nanoseconds) => crate::process::Parked::Until { nanoseconds },
        None => crate::process::Parked::Forever,
    });
}

pub(super) fn ppoll(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fds, count, timeout, mask, mask_size, _]: Args,
) -> super::Outcome {
    // The timeout first; a bad one ends the call before anything else.
    let until = match read_timeout(kernel, cx, tid, timeout) {
        Ok(until) => until,
        Err(error) => return Err(error).into(),
    };
    let result = ppoll_typed(
        kernel,
        cx,
        tid,
        fds,
        count,
        until.map(|(_, until)| until),
        mask,
        mask_size,
    );
    finish_wait(kernel, cx, tid, timeout, until, result)
}

pub(super) fn pselect6(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [count, read, write, except, timeout, mask_pack]: Args,
) -> super::Outcome {
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
    let until = match read_timeout(kernel, cx, tid, timeout) {
        Ok(until) => until,
        Err(error) => return Err(error).into(),
    };
    let result = select_typed(
        kernel,
        cx,
        tid,
        count,
        [read, write, except],
        until.map(|(_, until)| until),
        mask,
        mask_size,
    );
    finish_wait(kernel, cx, tid, timeout, until, result)
}

/// `pselect6` after its timeout is read, as Linux's `core_sys_select`: Some(count) when it returns now,
/// None when it waits.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn select_typed(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    count: u64,
    sets: [u64; 3],
    until: Option<u64>,
    mask: u64,
    mask_size: u64,
) -> Result<Option<u64>, Errno> {
    if mask != 0 {
        if mask_size != 8 {
            return Err(Errno::EINVAL);
        }
        guest(kernel, cx, tid).read(mask, 8)?;
    }
    let count = count as i32;
    if count < 0 {
        return Err(Errno::EINVAL);
    }
    let files = &kernel.process_of(tid).files;
    let count = (count as u32).min(files.capacity()) as u64;
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
    let files = &kernel.process_of(tid).files;
    for fd in 0..count {
        if wanted.iter().any(|set| asked(set, fd)) && files.get(fd as i32).is_err() {
            return Err(Errno::EBADF);
        }
    }
    // Linux's `POLLIN_SET`, `POLLOUT_SET` and `POLLEX_SET`.
    let kinds = [
        POLLRDNORM | POLLRDBAND | POLLIN | POLLHUP | POLLERR,
        POLLWRBAND | POLLWRNORM | POLLOUT | POLLERR,
        POLLPRI,
    ];
    let mut results = [const { Vec::new() }; 3];
    let mut ready = 0;
    for (index, set) in wanted.iter().enumerate() {
        let mut result = vec![0u8; bytes as usize];
        for fd in 0..count {
            if !asked(set, fd) {
                continue;
            }
            let file = files.file(fd as i32)?;
            // An `O_PATH` descriptor is never ready, as `fdget` does not see it.
            let mask = if file.borrow().is_path() {
                0
            } else {
                poll_mask(&file)
            };
            if u32::from(mask) & kinds[index] != 0 {
                result[(fd / 8) as usize] |= 1 << (fd % 8);
                ready += 1;
            }
        }
        results[index] = result;
    }
    let returns = ready > 0 || until.is_some_and(|until| until <= cx.host.monotonic_nanoseconds());
    // The sets are written back when the call returns; a timeout returns them empty.
    if returns || until.is_some() {
        for (address, result) in sets.iter().zip(&results) {
            if *address != 0 {
                guest(kernel, cx, tid).write(*address, result)?;
            }
        }
    }
    if returns {
        return Ok(Some(ready));
    }
    park_until(kernel, tid, until);
    Ok(None)
}

/// `ppoll` after its timeout is read: Some(count) when it returns now, None when it waits.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn ppoll_typed(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    fds: u64,
    count: u64,
    until: Option<u64>,
    mask: u64,
    mask_size: u64,
) -> Result<Option<u64>, Errno> {
    if mask != 0 {
        if mask_size != 8 {
            return Err(Errno::EINVAL);
        }
        guest(kernel, cx, tid).read(mask, 8)?;
    }
    if count > nofile_limit(kernel, tid) {
        return Err(Errno::EINVAL);
    }
    let mut entries = guest(kernel, cx, tid).read(fds, count * 8)?;
    let mut ready = 0;
    for entry in entries.as_chunks_mut::<8>().0 {
        let fd = i32::from_le_bytes(entry[..4].try_into().expect("4 bytes"));
        let events = u16::from_le_bytes([entry[4], entry[5]]);
        let revents = if fd < 0 {
            0
        } else {
            match kernel.process_of(tid).files.file(fd) {
                Ok(file) => poll_mask(&file) & (events | (POLLERR | POLLHUP | POLLNVAL) as u16),
                Err(_) => POLLNVAL as u16,
            }
        };
        entry[6..8].copy_from_slice(&revents.to_le_bytes());
        if revents != 0 {
            ready += 1;
        }
    }
    guest(kernel, cx, tid).write(fds, &entries)?;
    if ready > 0 || until.is_some_and(|until| until <= cx.host.monotonic_nanoseconds()) {
        return Ok(Some(ready));
    }
    park_until(kernel, tid, until);
    Ok(None)
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
    let range = guest(kernel, cx, tid).read(range, 16)?;
    let offset = u64::from_le_bytes(range[..8].try_into().expect("8 bytes"));
    let length = u64::from_le_bytes(range[8..].try_into().expect("8 bytes"));
    // Linux's `can_do_cachestat`: opened for writing, the owner, or allowed to write.
    let node = file.borrow().node.clone();
    let allowed = file.borrow().writable() || {
        let credentials = &kernel.process_of(tid).credentials;
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
        if regular.mapped {
            kernel
                .counters
                .count(cx.host, reason::RESIDENCY_NOT_TRACKED, 0);
        }
        cached = (regular.pages.range(first..=last).count()
            + regular.allocated.range(first..=last).count()) as u64;
    }
    let mut bytes = [0u8; 40];
    bytes[..8].copy_from_slice(&cached.to_le_bytes());
    guest(kernel, cx, tid).write(out, &bytes)?;
    Ok(0)
}
