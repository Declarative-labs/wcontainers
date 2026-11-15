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
const MAX_SEGMENTS: u64 = 1024;

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
    write_at(
        kernel,
        cx,
        tid,
        &file,
        buffer,
        count.min(MAX_RW_COUNT),
        None,
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
    write_at(
        kernel,
        cx,
        tid,
        &file,
        buffer,
        count.min(MAX_RW_COUNT),
        Some(offset),
    )
}

/// The checks for a read or write at a given offset: a negative offset is invalid, and a pipe has none.
fn positioned(file: &FileRef, _offset: u64) -> Result<(), Errno> {
    if matches!(file.borrow().node.borrow().content, Content::HostStream(_)) {
        return Err(Errno::ESPIPE);
    }
    Ok(())
}

/// The segments of a `readv` or `writev`, checked as Linux's `import_iovec` does.
fn segments(guest: &mut Guest<'_>, address: u64, count: u64) -> Result<Vec<(u64, u64)>, Errno> {
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
    vectored(kernel, cx, tid, &file, &parts, None, false)
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
    vectored(kernel, cx, tid, &file, &parts, None, true)
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
    vectored(kernel, cx, tid, &file, &parts, Some(offset), false)
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
    vectored(kernel, cx, tid, &file, &parts, Some(offset), true)
}

fn vectored(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
    mut offset: Option<u64>,
    writing: bool,
) -> Result<u64, Errno> {
    let mut done = 0;
    for &(base, length) in parts {
        if length == 0 {
            continue;
        }
        let result = if writing {
            write_at(kernel, cx, tid, file, base, length, offset)
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

/// Writes up to `count` bytes from the guest at the file's position, or at `at` without moving it.
pub(super) fn write_at(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    buffer: u64,
    count: u64,
    at: Option<u64>,
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
        // As much of the buffer as can be read.
        let readable = guest(kernel, cx, tid).accessible(buffer, count, Permissions::READ);
        if readable == 0 && count > 0 {
            return Err(Errno::EFAULT);
        }
        guest(kernel, cx, tid).read(buffer, readable)?
    } else if matches!(device, Some(Device::Random | Device::Urandom)) {
        // The random devices take the bytes in; the rest only count them.
        guest(kernel, cx, tid).read(buffer, count.min(PAGE_SIZE))?;
        return write_bytes(kernel, cx, tid, file, &[], at).map(|_| count);
    } else {
        return write_bytes(kernel, cx, tid, file, &[], at).map(|_| count);
    };
    write_bytes(kernel, cx, tid, file, &bytes, at)
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
    let append = file.borrow().flags & O_APPEND != 0;
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
    if !bytes.is_empty() {
        regular.write(offset as usize, bytes);
        inode.mtime = now;
        inode.ctime = now;
        // Writing clears the set-user and set-group bits, as Linux's `file_remove_privs` does for a
        // writer without `CAP_FSETID`.
        if !may_keep_privileges {
            inode.mode &= !crate::fs::privileges_to_drop(inode.mode);
        }
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
    let input = file(kernel, tid, in_fd)?;
    let output = file(kernel, tid, out_fd)?;
    if !input.borrow().readable() || !output.borrow().writable() {
        return Err(Errno::EBADF);
    }
    if output.borrow().flags & O_APPEND != 0 {
        return Err(Errno::EINVAL);
    }
    if matches!(input.borrow().node.borrow().content, Content::Directory(_)) {
        return Err(Errno::EINVAL);
    }
    let explicit = if offset_address != 0 {
        let offset: i64 = guest(kernel, cx, tid).read_value(offset_address)?;
        if offset < 0 {
            return Err(Errno::EINVAL);
        }
        Some(offset as u64)
    } else {
        None
    };
    let offset = explicit.unwrap_or(input.borrow().offset);
    let bytes = file_bytes(cx, &input, count.min(MAX_RW_COUNT), offset)?;
    let written = write_bytes(kernel, cx, tid, &output, &bytes, None)?;
    match explicit {
        Some(_) => {
            guest(kernel, cx, tid).write(offset_address, &(offset + written).to_le_bytes())?
        }
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
    let count = count as u32 as u64;
    if !access_ok(buffer, count) {
        return Err(Errno::EFAULT);
    }
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

/// `fsync`, `fdatasync` and `syncfs`: everything is in memory, so there is nothing to wait for.
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
    match file.borrow().node.borrow().content {
        Content::HostStream(_) | Content::Special => Err(Errno::EINVAL),
        _ => Ok(0),
    }
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

pub(super) fn ppoll(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fds, count, timeout, mask, mask_size, _]: Args,
) -> super::Outcome {
    match ppoll_typed(kernel, cx, tid, fds, count, timeout, mask, mask_size) {
        Ok(Some(ready)) => super::Outcome::Return(ready),
        Ok(None) => super::Outcome::Parked,
        Err(error) => Err(error).into(),
    }
}

/// `ppoll`: Some(count) when it returns now, None when it waits.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
fn ppoll_typed(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    fds: u64,
    count: u64,
    timeout: u64,
    mask: u64,
    mask_size: u64,
) -> Result<Option<u64>, Errno> {
    // In the order of Linux's `ppoll`: the timeout, the signal mask, then the descriptors.
    let until = if timeout != 0 {
        let value: __kernel_timespec = guest(kernel, cx, tid).read_value(timeout)?;
        if value.tv_sec < 0 || !(0..1_000_000_000).contains(&value.tv_nsec) {
            return Err(Errno::EINVAL);
        }
        let duration = (value.tv_sec as u64)
            .saturating_mul(1_000_000_000)
            .saturating_add(value.tv_nsec as u64);
        Some(cx.host.monotonic_nanoseconds().saturating_add(duration))
    } else {
        None
    };
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
    kernel.thread_mut(tid).parked = Some(match until {
        Some(nanoseconds) => crate::process::Parked::Until { nanoseconds },
        None => crate::process::Parked::Forever,
    });
    Ok(None)
}
