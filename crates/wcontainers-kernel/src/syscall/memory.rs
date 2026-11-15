//! The memory calls: the program break, mappings and their protection.

use alloc::vec;

use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE, Permissions};

use super::Args;
use crate::counters::reason;
use crate::fs::Content;
use crate::fs::dev::Device;
use crate::linux::general::*;
use crate::memory::{Backing, Mapping, STACK_GUARD_GAP, page_down, page_up};
use crate::process::Tid;
use crate::{Context, Errno, Kernel, errno};

/// What a protection argument allows. `mprotect` refuses bits ARM64 does not have; `mmap` ignores them.
fn permissions(prot: u64, strict: bool) -> Result<Permissions, Errno> {
    let prot = prot as u32;
    if strict && prot & !(PROT_READ | PROT_WRITE | PROT_EXEC | PROT_SEM) != 0 {
        return Err(Errno::EINVAL);
    }
    let mut permissions = Permissions::empty();
    if prot & PROT_READ != 0 {
        permissions |= Permissions::READ;
    }
    if prot & PROT_WRITE != 0 {
        permissions |= Permissions::WRITE;
    }
    if prot & PROT_EXEC != 0 {
        permissions |= Permissions::EXECUTE;
    }
    Ok(permissions)
}

pub(super) fn brk(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, ..]: Args,
) -> Result<u64, Errno> {
    let process = kernel.process_of_mut(tid);
    let map = &mut process.map;
    let old = map.brk;
    if address < map.brk_start {
        return Ok(old);
    }
    let old_end = page_up(old);
    let new_end = page_up(address);
    if new_end == old_end {
        map.brk = address;
        return Ok(address);
    }
    if new_end < old_end {
        map.unmap(cx.cpu, process.memory, new_end, old_end)?;
        map.brk = address;
        return Ok(address);
    }
    // Growing: the new part must not touch the next mapping, nor a stack's guard gap.
    if let Some(next) = map.next_after(old_end) {
        let gap = if matches!(next.backing, Backing::Stack) {
            STACK_GUARD_GAP
        } else {
            0
        };
        if next.start < new_end.saturating_add(gap) {
            return Ok(old);
        }
    }
    if new_end > map.end {
        return Ok(old);
    }
    let heap = Mapping {
        start: old_end,
        end: new_end,
        permissions: Permissions::READ | Permissions::WRITE,
        shared: false,
        backing: Backing::Heap,
    };
    if map.insert(cx.cpu, process.memory, heap).is_err() {
        return Ok(old);
    }
    map.brk = address;
    Ok(address)
}

pub(super) fn mmap(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, length, prot, flags, fd, offset]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `sys_mmap`, `ksys_mmap_pgoff` and `do_mmap`.
    let flags = flags as u32;
    let fd = fd as i32;
    if offset % PAGE_SIZE != 0 {
        return Err(Errno::EINVAL);
    }
    let file = if flags & MAP_ANONYMOUS != 0 {
        None
    } else {
        let file = kernel.process_of(tid).files.file(fd)?;
        if file.borrow().is_path() {
            return Err(Errno::EBADF);
        }
        Some(file)
    };
    if flags & MAP_HUGETLB != 0 {
        kernel.counters.count(
            cx.host,
            reason::MEMORY_FLAG_NOT_IMPLEMENTED,
            MAP_HUGETLB.into(),
        );
        return Err(Errno::ENOMEM);
    }
    if length == 0 {
        return Err(Errno::EINVAL);
    }
    let permissions = permissions(prot, false)?;
    let length = page_up(length);
    if length == 0 {
        return Err(Errno::ENOMEM);
    }
    if (offset / PAGE_SIZE)
        .checked_add(length / PAGE_SIZE)
        .is_none()
    {
        return Err(Errno::EOVERFLOW);
    }
    let map = &kernel.process_of(tid).map;
    let fixed = flags & (MAP_FIXED | MAP_FIXED_NOREPLACE) != 0;
    let start = if fixed {
        if length > map.end || address > map.end - length {
            return Err(Errno::ENOMEM);
        }
        if address % PAGE_SIZE != 0 {
            return Err(Errno::EINVAL);
        }
        if address < LOWEST_ADDRESS {
            return Err(Errno::EPERM);
        }
        address
    } else {
        let hint = page_down(address);
        let hint_fits = hint >= LOWEST_ADDRESS
            && hint
                .checked_add(length)
                .is_some_and(|end| end <= map.end && !map.any_mapped(hint, end));
        if hint != 0 && hint_fits {
            hint
        } else {
            map.find_free(length).ok_or(Errno::ENOMEM)?
        }
    };
    if flags & MAP_FIXED_NOREPLACE != 0
        && flags & MAP_FIXED == 0
        && map.any_mapped(start, start + length)
    {
        return Err(Errno::EEXIST);
    }
    // Validated sharing exists only for files.
    let shared = match (flags & MAP_TYPE, file.is_some()) {
        (MAP_PRIVATE, _) => false,
        (MAP_SHARED, _) | (MAP_SHARED_VALIDATE, true) => true,
        _ => return Err(Errno::EINVAL),
    };
    // What backs the mapping.
    let file = match file {
        None => None,
        Some(file) => {
            let file = file.borrow();
            let node = file.node.clone();
            let inode = node.borrow();
            if shared && permissions.contains(Permissions::WRITE) && !file.writable() {
                return Err(Errno::EACCES);
            }
            if !file.readable() {
                return Err(Errno::EACCES);
            }
            let zero = matches!(inode.content, Content::Special)
                && Device::from_number(inode.rdev) == Some(Device::Zero);
            if !zero && !matches!(inode.content, Content::Regular(_)) {
                return Err(Errno::ENODEV);
            }
            if zero {
                None
            } else {
                drop(inode);
                if shared {
                    kernel
                        .counters
                        .count(cx.host, reason::SHARED_FILE_MAPPING, 0);
                    return Err(Errno::ENODEV);
                }
                Some((node, file.path.clone()))
            }
        }
    };
    let process = kernel.process_of_mut(tid);
    let map = &mut process.map;
    let backing = match &file {
        None => Backing::Anonymous,
        Some((node, path)) => {
            let size = match &node.borrow().content {
                Content::Regular(regular) => regular.data.len() as u64,
                _ => 0,
            };
            let file_pages = page_up(size).saturating_sub(offset);
            Backing::File {
                node: node.clone(),
                path: path.clone(),
                offset,
                file_end: start + file_pages.min(length),
            }
        }
    };
    let mapping = Mapping {
        start,
        end: start + length,
        permissions,
        shared,
        backing,
    };
    map.insert(cx.cpu, process.memory, mapping)?;
    if let Some((node, _)) = &file {
        copy_file_pages(cx, process.memory, node, offset, start, length)?;
    }
    Ok(start)
}

/// Copies a file's bytes into a private mapping of it: the bytes from `offset` up to the mapping's
/// length or the file's end. The rest is zero already.
fn copy_file_pages(
    cx: &mut Context<'_>,
    memory: wcontainers_abi::MemoryId,
    node: &crate::fs::Node,
    offset: u64,
    start: u64,
    length: u64,
) -> Result<(), Errno> {
    let inode = node.borrow();
    let Content::Regular(regular) = &inode.content else {
        return Ok(());
    };
    let size = regular.data.len() as u64;
    if offset < size {
        let end = size.min(offset + length);
        cx.cpu
            .write(memory, start, &regular.data[offset as usize..end as usize])
            .map_err(errno::from_cpu)?;
    }
    Ok(())
}

pub(super) fn munmap(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, length, ..]: Args,
) -> Result<u64, Errno> {
    if address % PAGE_SIZE != 0 || length == 0 {
        return Err(Errno::EINVAL);
    }
    let length = page_up(length);
    let process = kernel.process_of_mut(tid);
    let end = address.checked_add(length).ok_or(Errno::EINVAL)?;
    if end > crate::user::ADDRESS_LIMIT {
        return Err(Errno::EINVAL);
    }
    let end = end.min(process.map.end);
    if address < end {
        process.map.unmap(cx.cpu, process.memory, address, end)?;
    }
    Ok(0)
}

pub(super) fn mprotect(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, length, prot, ..]: Args,
) -> Result<u64, Errno> {
    if address % PAGE_SIZE != 0 {
        return Err(Errno::EINVAL);
    }
    if length == 0 {
        return Ok(0);
    }
    let length = page_up(length);
    let end = address
        .checked_add(length)
        .filter(|end| *end > address)
        .ok_or(Errno::ENOMEM)?;
    let permissions = permissions(prot, true)?;
    let process = kernel.process_of_mut(tid);
    if !process.map.fully_mapped(address, end) {
        return Err(Errno::ENOMEM);
    }
    process
        .map
        .protect(cx.cpu, process.memory, address, end, permissions)?;
    Ok(0)
}

pub(super) fn madvise(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, length, advice, ..]: Args,
) -> Result<u64, Errno> {
    let advice = advice as u32;
    let known = matches!(
        advice,
        MADV_NORMAL
            | MADV_RANDOM
            | MADV_SEQUENTIAL
            | MADV_WILLNEED
            | MADV_DONTNEED
            | MADV_FREE
            | MADV_DONTFORK
            | MADV_DOFORK
            | MADV_HUGEPAGE
            | MADV_NOHUGEPAGE
            | MADV_DONTDUMP
            | MADV_DODUMP
            | MADV_COLD
            | MADV_PAGEOUT
            | MADV_WIPEONFORK
            | MADV_KEEPONFORK
            | MADV_REMOVE
            | MADV_MERGEABLE
            | MADV_UNMERGEABLE
            | MADV_POPULATE_READ
            | MADV_POPULATE_WRITE
            | MADV_COLLAPSE
    );
    // Poisoning pages (MADV_HWPOISON, MADV_SOFT_OFFLINE) is not in the reference's kernel either.
    if !known {
        return Err(Errno::EINVAL);
    }
    if address % PAGE_SIZE != 0 {
        return Err(Errno::EINVAL);
    }
    let length = page_up(length);
    let end = address.checked_add(length).ok_or(Errno::EINVAL)?;
    if length == 0 {
        return Ok(0);
    }
    let process = kernel.process_of_mut(tid);
    // Removing pages works only on shared file mappings, which do not exist yet; anything else refuses.
    if advice == MADV_REMOVE {
        if let Some(mapping) = process
            .map
            .next_after(address)
            .filter(|mapping| mapping.start < end)
        {
            return Err(if matches!(mapping.backing, Backing::File { .. }) {
                Errno::EACCES
            } else {
                Errno::EINVAL
            });
        }
        return Err(Errno::ENOMEM);
    }
    if advice == MADV_DONTNEED {
        // Private pages go back to what they were: zero, or the file's bytes.
        let mut at = address;
        while at < end {
            let Some(mapping) = process.map.next_after(at).cloned() else {
                break;
            };
            if mapping.start >= end {
                break;
            }
            let from = mapping.start.max(at);
            let to = mapping.end.min(end);
            cx.cpu
                .zero(process.memory, from, to - from)
                .map_err(errno::from_cpu)?;
            if let Backing::File { node, offset, .. } = &mapping.backing {
                let file_offset = offset + (from - mapping.start);
                copy_file_pages(cx, process.memory, node, file_offset, from, to - from)?;
            }
            at = to;
        }
    }
    if !process.map.fully_mapped(address, end) {
        return Err(Errno::ENOMEM);
    }
    Ok(0)
}

pub(super) fn mremap(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [old, old_length, new_length, flags, new_address, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(MREMAP_MAYMOVE | MREMAP_FIXED | MREMAP_DONTUNMAP) != 0 || old % PAGE_SIZE != 0 {
        return Err(Errno::EINVAL);
    }
    if flags & MREMAP_FIXED != 0 && flags & MREMAP_MAYMOVE == 0 {
        return Err(Errno::EINVAL);
    }
    if flags & MREMAP_DONTUNMAP != 0 {
        kernel.counters.count(
            cx.host,
            reason::MEMORY_FLAG_NOT_IMPLEMENTED,
            MREMAP_DONTUNMAP.into(),
        );
        return Err(Errno::EINVAL);
    }
    let old_length = page_up(old_length);
    let new_length = page_up(new_length);
    if new_length == 0 || old_length == 0 {
        return Err(Errno::EINVAL);
    }
    let process = kernel.process_of_mut(tid);
    let memory = process.memory;
    let map = &mut process.map;
    let old_end = old.checked_add(old_length).ok_or(Errno::EFAULT)?;
    let mapping = map.find(old).cloned().ok_or(Errno::EFAULT)?;
    if mapping.end < old_end {
        return Err(Errno::EFAULT);
    }
    if flags & MREMAP_FIXED == 0 {
        if new_length <= old_length {
            if new_length < old_length {
                map.unmap(cx.cpu, memory, old + new_length, old_end)?;
            }
            return Ok(old);
        }
        // Grow in place when the space after is free and the range is the end of its mapping.
        let grown_end = old.checked_add(new_length).ok_or(Errno::ENOMEM)?;
        if mapping.end == old_end && grown_end <= map.end && !map.any_mapped(old_end, grown_end) {
            let mut extra = mapping.cut_public(old_end, grown_end);
            if let Backing::File { file_end, .. } = &mut extra.backing {
                *file_end = (*file_end).min(grown_end).max(old_end);
            }
            map.insert(cx.cpu, memory, extra.clone())?;
            if let Backing::File { node, offset, .. } = &extra.backing {
                copy_file_pages(cx, memory, node, *offset, old_end, grown_end - old_end)?;
            }
            return Ok(old);
        }
        if flags & MREMAP_MAYMOVE == 0 {
            return Err(Errno::ENOMEM);
        }
    }
    let destination = if flags & MREMAP_FIXED != 0 {
        if new_address % PAGE_SIZE != 0 {
            return Err(Errno::EINVAL);
        }
        let destination_end = new_address.checked_add(new_length).ok_or(Errno::EINVAL)?;
        if destination_end > map.end || (new_address < old_end && old < destination_end) {
            return Err(Errno::EINVAL);
        }
        new_address
    } else {
        map.find_free(new_length).ok_or(Errno::ENOMEM)?
    };
    let kept = old_length.min(new_length);
    let mut bytes = vec![0; kept as usize];
    cx.cpu
        .read(memory, old, &mut bytes)
        .map_err(errno::from_cpu)?;
    let mut moved = mapping.cut_public(old, old + new_length);
    let shift = |address: u64| address - old + destination;
    moved.start = destination;
    moved.end = destination + new_length;
    if let Backing::File { file_end, .. } = &mut moved.backing {
        *file_end = shift((*file_end).clamp(old, old + new_length));
    }
    map.unmap(cx.cpu, memory, old, old_end)?;
    map.insert(cx.cpu, memory, moved.clone())?;
    cx.cpu
        .write(memory, destination, &bytes)
        .map_err(errno::from_cpu)?;
    if new_length > old_length
        && let Backing::File { node, offset, .. } = &moved.backing
    {
        copy_file_pages(
            cx,
            memory,
            node,
            offset + old_length,
            destination + old_length,
            new_length - old_length,
        )?;
    }
    Ok(destination)
}

pub(super) fn msync(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [address, length, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if address % PAGE_SIZE != 0
        || flags & !(MS_ASYNC | MS_INVALIDATE | MS_SYNC) != 0
        || flags & (MS_ASYNC | MS_SYNC) == MS_ASYNC | MS_SYNC
    {
        return Err(Errno::EINVAL);
    }
    let end = address.checked_add(page_up(length)).ok_or(Errno::ENOMEM)?;
    if !kernel.process_of(tid).map.fully_mapped(address, end) {
        return Err(Errno::ENOMEM);
    }
    Ok(0)
}
