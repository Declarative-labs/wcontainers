//! The memory calls: the program break, mappings and their protection.

use alloc::vec;

use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE, Permissions};

use super::Args;
use crate::counters::reason;
use crate::fs::Content;
use crate::fs::dev::Device;
use crate::linux::general::*;
use crate::memory::{Backing, Lock, Mapping, STACK_GUARD_GAP, page_down, page_up};
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
    // Linux's `check_data_rlimit` on the heap's size, then `may_expand_vm` on the growth.
    let data_limit = process.limits[RLIMIT_DATA as usize].current;
    if address - map.brk_start > data_limit
        || !map.may_expand(
            &process.limits,
            true,
            (new_end - old_end) / PAGE_SIZE,
            (0, 0),
        )
    {
        return Ok(old);
    }
    if !map.may_lock(
        &process.limits,
        map.future_lock,
        (new_end - old_end) / PAGE_SIZE,
        process.credentials.has(CAP_IPC_LOCK),
    ) {
        return Ok(old);
    }
    let heap = Mapping {
        start: old_end,
        end: new_end,
        permissions: Permissions::READ | Permissions::WRITE,
        shared: false,
        locked: map.future_lock,
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
    // Locked if asked, or if every new mapping is to be; within the limit on locked memory.
    let locked = if flags & MAP_LOCKED != 0 {
        Lock::Now
    } else {
        map.future_lock
    };
    if flags & MAP_LOCKED != 0 && !can_lock(kernel, tid) {
        return Err(Errno::EPERM);
    }
    let process = kernel.process_of(tid);
    if !process.map.may_lock(
        &process.limits,
        locked,
        length / PAGE_SIZE,
        process.credentials.has(CAP_IPC_LOCK),
    ) {
        return Err(Errno::EAGAIN);
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
            // Running code from a `noexec` mount is refused.
            if permissions.contains(Permissions::EXECUTE) && inode.fs.no_execute() {
                return Err(Errno::EPERM);
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
        locked,
        backing,
    };
    // Within the limits, counting what a fixed mapping replaces.
    let replaced = map.pages_in(start, start + length);
    if !map.may_expand(
        &process.limits,
        mapping.is_data(),
        length / PAGE_SIZE,
        replaced,
    ) {
        return Err(Errno::ENOMEM);
    }
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
    protect(kernel, cx, tid, address, length, prot, -1)
}

/// `pkey_mprotect`: `mprotect` with a protection key, which must be -1 (none) or allocated.
pub(super) fn pkey_mprotect(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, length, prot, key, ..]: Args,
) -> Result<u64, Errno> {
    protect(kernel, cx, tid, address, length, prot, key as i32)
}

/// Linux's `do_mprotect_pkey`.
fn protect(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    length: u64,
    prot: u64,
    key: i32,
) -> Result<u64, Errno> {
    if !address.is_multiple_of(PAGE_SIZE) {
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
    if key != -1 && !key_allocated(&process.map, key) {
        return Err(Errno::EINVAL);
    }
    if !process.map.fully_mapped(address, end) {
        return Err(Errno::ENOMEM);
    }
    // Private pages turning writable become data, within its limit.
    if permissions.contains(Permissions::WRITE) {
        let becoming: u64 = process
            .map
            .iter()
            .filter(|mapping| mapping.start < end && mapping.end > address)
            .filter(|mapping| {
                !mapping.is_data()
                    && crate::memory::is_data(permissions, mapping.shared, &mapping.backing)
            })
            .map(|mapping| (mapping.end.min(end) - mapping.start.max(address)) / PAGE_SIZE)
            .sum();
        if becoming > 0
            && !process
                .map
                .may_expand(&process.limits, true, becoming, (becoming, 0))
        {
            return Err(Errno::ENOMEM);
        }
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
    if !address.is_multiple_of(PAGE_SIZE) {
        return Err(Errno::EINVAL);
    }
    let length = page_up(length);
    let end = address.checked_add(length).ok_or(Errno::EINVAL)?;
    if length == 0 {
        return Ok(0);
    }
    let process = kernel.process_of_mut(tid);
    // Removing pages punches a hole in what a shared mapping is backed by; shared anonymous memory is
    // the only such mapping so far. Linux's `madvise_remove`, mapping by mapping.
    if advice == MADV_REMOVE {
        let mut at = address;
        while let Some(mapping) = process.map.next_after(at).cloned() {
            if mapping.start >= end {
                break;
            }
            if mapping.locked != Lock::None {
                return Err(Errno::EINVAL);
            }
            match mapping.backing {
                Backing::File { .. } => return Err(Errno::EACCES),
                Backing::Anonymous if mapping.shared => {
                    let from = mapping.start.max(at);
                    let to = mapping.end.min(end);
                    cx.cpu
                        .zero(process.memory, from, to - from)
                        .map_err(errno::from_cpu)?;
                }
                _ => return Err(Errno::EINVAL),
            }
            at = mapping.end;
        }
        if !process.map.fully_mapped(address, end) {
            return Err(Errno::ENOMEM);
        }
        return Ok(0);
    }
    // Freeing lazily and wiping on fork are for private anonymous memory only.
    if matches!(advice, MADV_FREE | MADV_WIPEONFORK)
        && process
            .map
            .iter()
            .filter(|mapping| mapping.start < end && mapping.end > address)
            .any(|mapping| mapping.shared || matches!(mapping.backing, Backing::File { .. }))
    {
        return Err(Errno::EINVAL);
    }
    if matches!(advice, MADV_DONTNEED | MADV_FREE)
        && process.map.iter().any(|mapping| {
            mapping.start < end && mapping.end > address && mapping.locked != Lock::None
        })
    {
        return Err(Errno::EINVAL);
    }
    if advice == MADV_DONTNEED {
        // Private pages go back to what they were: zero, or the file's bytes. Shared pages keep their
        // bytes in what they share.
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
            if !mapping.shared {
                drop_pages(cx, process.memory, &mapping, from, to)?;
            }
            at = to;
        }
    }
    if !process.map.fully_mapped(address, end) {
        return Err(Errno::ENOMEM);
    }
    Ok(0)
}

/// Linux's `TASK_SIZE` for 48 bit addresses, which `mremap` checks lengths and addresses against.
const TASK_SIZE: u64 = crate::user::ADDRESS_LIMIT;

pub(super) fn mremap(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [old, old_length, new_length, flags, new_address, ..]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `do_mremap` and `check_mremap_params`.
    let flags = flags as u32;
    let old_length = page_up(old_length);
    let new_length = page_up(new_length);
    if flags & !(MREMAP_MAYMOVE | MREMAP_FIXED | MREMAP_DONTUNMAP) != 0 || old % PAGE_SIZE != 0 {
        return Err(Errno::EINVAL);
    }
    if new_length == 0 || new_length > TASK_SIZE {
        return Err(Errno::EINVAL);
    }
    let to_new_address = flags & (MREMAP_FIXED | MREMAP_DONTUNMAP) != 0;
    if to_new_address
        && (new_address > TASK_SIZE - new_length
            || new_address % PAGE_SIZE != 0
            || flags & MREMAP_MAYMOVE == 0
            || (flags & MREMAP_DONTUNMAP != 0 && old_length != new_length)
            || (old.saturating_add(old_length) > new_address && new_address + new_length > old))
    {
        return Err(Errno::EINVAL);
    }
    let request = Remap {
        old,
        old_length,
        new_length,
        new_address,
        flags,
    };
    if flags & MREMAP_FIXED != 0 && old_length == new_length {
        return move_ranges(kernel, cx, tid, request);
    }
    let mapping = kernel
        .process_of(tid)
        .map
        .find(old)
        .cloned()
        .ok_or(Errno::EFAULT)?;
    prepare(kernel, tid, &mapping, &request)?;
    if to_new_address {
        remap_to(kernel, cx, tid, &mapping, request)
    } else {
        remap_at(kernel, cx, tid, &mapping, request)
    }
}

#[derive(Clone, Copy)]
struct Remap {
    old: u64,
    old_length: u64,
    new_length: u64,
    new_address: u64,
    flags: u32,
}

/// Linux's `check_prep_vma`: what a range of `mapping` must allow before it is resized or moved.
fn prepare(kernel: &Kernel, tid: Tid, mapping: &Mapping, request: &Remap) -> Result<(), Errno> {
    let Remap {
        old,
        old_length,
        new_length,
        ..
    } = *request;
    let maps_new =
        new_length > old_length || request.flags & (MREMAP_FIXED | MREMAP_DONTUNMAP) != 0;
    if !maps_new {
        return Ok(());
    }
    // Duplicating a private mapping with a zero old length is refused.
    if old_length == 0 && !mapping.shared {
        return Err(Errno::EINVAL);
    }
    // A shrink may cross the end of the mapping; anything else must lie within it.
    if old_length.min(new_length) > mapping.end - old {
        return Err(Errno::EFAULT);
    }
    // A shrink counts as its new length, so only growth goes on to the limits.
    if new_length <= old_length {
        return Ok(());
    }
    let process = kernel.process_of(tid);
    let growth = (new_length - old_length) / PAGE_SIZE;
    if !process.map.may_lock(
        &process.limits,
        mapping.locked,
        growth,
        process.credentials.has(CAP_IPC_LOCK),
    ) {
        return Err(Errno::EAGAIN);
    }
    if !process
        .map
        .may_expand(&process.limits, mapping.is_data(), growth, (0, 0))
    {
        return Err(Errno::ENOMEM);
    }
    Ok(())
}

/// Linux's `mremap_at`: resizing where the range is, or moving it if it must grow and may move.
fn remap_at(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    mapping: &Mapping,
    request: Remap,
) -> Result<u64, Errno> {
    let Remap {
        old,
        old_length,
        new_length,
        ..
    } = request;
    let process = kernel.process_of_mut(tid);
    let memory = process.memory;
    let map = &mut process.map;
    if new_length == old_length {
        return Ok(old);
    }
    if new_length < old_length {
        map.unmap(
            cx.cpu,
            memory,
            old + new_length,
            old.saturating_add(old_length),
        )?;
        return Ok(old);
    }
    // Grow in place when the range is the end of its mapping and the space after is free.
    let old_end = old + old_length;
    let grown_end = old.saturating_add(new_length);
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
    if request.flags & MREMAP_MAYMOVE == 0 {
        return Err(Errno::ENOMEM);
    }
    let destination = map.find_free(new_length).ok_or(Errno::ENOMEM)?;
    move_range(cx, kernel, tid, mapping, request, destination)
}

/// Linux's `mremap_to`: moving the range to a new address, given or chosen.
fn remap_to(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    mapping: &Mapping,
    mut request: Remap,
) -> Result<u64, Errno> {
    let process = kernel.process_of_mut(tid);
    let memory = process.memory;
    let map = &mut process.map;
    if request.flags & MREMAP_FIXED != 0 {
        let end = (request.new_address + request.new_length).min(map.end);
        if request.new_address < end {
            map.unmap(cx.cpu, memory, request.new_address, end)?;
        }
    }
    if request.new_length < request.old_length {
        map.unmap(
            cx.cpu,
            memory,
            request.old + request.new_length,
            request.old.saturating_add(request.old_length),
        )?;
        request.old_length = request.new_length;
    }
    // Leaving the old range mapped takes as much again.
    if request.flags & MREMAP_DONTUNMAP != 0
        && !map.may_expand(
            &process.limits,
            mapping.is_data(),
            request.old_length / PAGE_SIZE,
            (0, 0),
        )
    {
        return Err(Errno::ENOMEM);
    }
    let destination = if request.flags & MREMAP_FIXED != 0 {
        if request.new_address + request.new_length > map.end {
            return Err(Errno::ENOMEM);
        }
        request.new_address
    } else {
        // The address is a hint, taken when the space there is free.
        let hint = request.new_address;
        if hint >= LOWEST_ADDRESS
            && hint + request.new_length <= map.end
            && !map.any_mapped(hint, hint + request.new_length)
        {
            hint
        } else {
            map.find_free(request.new_length).ok_or(Errno::ENOMEM)?
        }
    };
    move_range(cx, kernel, tid, mapping, request, destination)
}

/// The newer form of a move, Linux's `remap_move`: a fixed move with no change of size takes every
/// mapping in the range along, keeping the gaps between them.
fn move_ranges(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    request: Remap,
) -> Result<u64, Errno> {
    let start = request.old;
    let end = request.old.saturating_add(request.old_length);
    let mut target = request.new_address;
    let mut result = Err(Errno::EFAULT);
    let mut last_end = None;
    let mut at = start;
    while at < end {
        let Some(mapping) = kernel.process_of(tid).map.next_after(at).cloned() else {
            break;
        };
        if mapping.start >= end {
            break;
        }
        // No gap is allowed at the start of the range.
        if last_end.is_none() && start < mapping.start {
            return Err(Errno::EFAULT);
        }
        let from = mapping.start.max(start);
        let length = mapping.end.min(end) - from;
        let gap = last_end.map_or(0, |last_end| mapping.start - last_end);
        last_end = Some(mapping.end);
        let piece = Remap {
            old: from,
            old_length: length,
            new_length: length,
            new_address: target + gap,
            flags: request.flags,
        };
        prepare(kernel, tid, &mapping, &piece)?;
        let moved = remap_to(kernel, cx, tid, &mapping, piece)?;
        if result.is_err() {
            result = Ok(moved);
        }
        target = moved + length;
        at = from + length;
    }
    result
}

/// Linux's `move_vma`: the range's pages and what it maps go to `destination`, the old range is
/// unmapped (or, with `MREMAP_DONTUNMAP`, left mapped with its pages dropped), and growth past the old
/// length is fresh.
fn move_range(
    cx: &mut Context<'_>,
    kernel: &mut Kernel,
    tid: Tid,
    mapping: &Mapping,
    request: Remap,
    destination: u64,
) -> Result<u64, Errno> {
    let Remap {
        old,
        old_length,
        new_length,
        ..
    } = request;
    let process = kernel.process_of_mut(tid);
    let memory = process.memory;
    let map = &mut process.map;
    let old_end = old + old_length;
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
    if request.flags & MREMAP_DONTUNMAP != 0 {
        drop_pages(cx, memory, mapping, old, old_end)?;
    } else {
        map.unmap(cx.cpu, memory, old, old_end)?;
    }
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

/// Puts a private range back to what it held before it was touched: zero, or the file's bytes.
fn drop_pages(
    cx: &mut Context<'_>,
    memory: wcontainers_abi::MemoryId,
    mapping: &Mapping,
    from: u64,
    to: u64,
) -> Result<(), Errno> {
    cx.cpu
        .zero(memory, from, to - from)
        .map_err(errno::from_cpu)?;
    if let Backing::File { node, offset, .. } = &mapping.backing {
        copy_file_pages(
            cx,
            memory,
            node,
            offset + (from - mapping.start),
            from,
            to - from,
        )?;
    }
    Ok(())
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
    let map = &kernel.process_of(tid).map;
    if flags & MS_INVALIDATE != 0
        && map.iter().any(|mapping| {
            mapping.start < end && mapping.end > address && mapping.locked != Lock::None
        })
    {
        return Err(Errno::EBUSY);
    }
    if !map.fully_mapped(address, end) {
        return Err(Errno::ENOMEM);
    }
    Ok(0)
}

/// Whether the caller may lock memory at all (Linux's `can_do_mlock`).
fn can_lock(kernel: &Kernel, tid: Tid) -> bool {
    let process = kernel.process_of(tid);
    process.limits[RLIMIT_MEMLOCK as usize].current != 0 || process.credentials.has(CAP_IPC_LOCK)
}

/// Linux's `do_mlock` and `apply_vma_lock_flags`: the whole range must be mapped and fit the limit on
/// locked memory.
fn lock_range(
    kernel: &mut Kernel,
    tid: Tid,
    address: u64,
    length: u64,
    lock: Lock,
) -> Result<u64, Errno> {
    if lock != Lock::None && !can_lock(kernel, tid) {
        return Err(Errno::EPERM);
    }
    let length = page_up(length.saturating_add(address % PAGE_SIZE));
    let start = page_down(address);
    let process = kernel.process_of_mut(tid);
    if lock != Lock::None {
        let (locked, within) = process
            .map
            .locked_pages(start, start.saturating_add(length));
        let limit = process.limits[RLIMIT_MEMLOCK as usize].current / PAGE_SIZE;
        if locked + length / PAGE_SIZE - within > limit && !process.credentials.has(CAP_IPC_LOCK) {
            return Err(Errno::ENOMEM);
        }
    }
    let end = start.checked_add(length).ok_or(Errno::EINVAL)?;
    if end == start {
        return Ok(0);
    }
    if !process.map.fully_mapped(start, end) {
        return Err(Errno::ENOMEM);
    }
    process.map.set_lock(start, end, lock);
    // Linux's `__mm_populate`: bringing the pages in fails at one that cannot be read (no access, or
    // wholly past its file's end), and the call is ENOMEM though the range stays locked.
    let unreadable = process
        .map
        .iter()
        .filter(|mapping| mapping.start < end && mapping.end > start)
        .any(|mapping| {
            mapping.permissions.is_empty()
                || matches!(mapping.backing, Backing::File { file_end, .. } if file_end < end.min(mapping.end))
        });
    if lock == Lock::Now && unreadable {
        return Err(Errno::ENOMEM);
    }
    Ok(0)
}

pub(super) fn mlock(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [address, length, ..]: Args,
) -> Result<u64, Errno> {
    lock_range(kernel, tid, address, length, Lock::Now)
}

pub(super) fn mlock2(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [address, length, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !MLOCK_ONFAULT != 0 {
        return Err(Errno::EINVAL);
    }
    let lock = if flags & MLOCK_ONFAULT != 0 {
        Lock::OnFault
    } else {
        Lock::Now
    };
    lock_range(kernel, tid, address, length, lock)
}

pub(super) fn munlock(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [address, length, ..]: Args,
) -> Result<u64, Errno> {
    lock_range(kernel, tid, address, length, Lock::None)
}

pub(super) fn mlockall(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags == 0 || flags & !(MCL_CURRENT | MCL_FUTURE | MCL_ONFAULT) != 0 || flags == MCL_ONFAULT
    {
        return Err(Errno::EINVAL);
    }
    if !can_lock(kernel, tid) {
        return Err(Errno::EPERM);
    }
    let process = kernel.process_of_mut(tid);
    let lock = if flags & MCL_ONFAULT != 0 {
        Lock::OnFault
    } else {
        Lock::Now
    };
    if flags & MCL_CURRENT != 0 {
        let total = process.map.pages_in(0, u64::MAX).0;
        if total > process.limits[RLIMIT_MEMLOCK as usize].current / PAGE_SIZE
            && !process.credentials.has(CAP_IPC_LOCK)
        {
            return Err(Errno::ENOMEM);
        }
        process.map.set_lock_all(lock);
    }
    process.map.future_lock = if flags & MCL_FUTURE != 0 {
        lock
    } else {
        Lock::None
    };
    Ok(0)
}

pub(super) fn munlockall(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    _: Args,
) -> Result<u64, Errno> {
    let map = &mut kernel.process_of_mut(tid).map;
    map.set_lock_all(Lock::None);
    map.future_lock = Lock::None;
    Ok(0)
}

pub(super) fn mincore(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, length, vector, ..]: Args,
) -> Result<u64, Errno> {
    // In the order of Linux's `mincore`: the range, the vector, then mapping by mapping.
    if !address.is_multiple_of(PAGE_SIZE) {
        return Err(Errno::EINVAL);
    }
    if !crate::user::access_ok(address, length) {
        return Err(Errno::ENOMEM);
    }
    let pages = length.div_ceil(PAGE_SIZE);
    if !crate::user::access_ok(vector, pages) {
        return Err(Errno::EFAULT);
    }
    kernel
        .counters
        .count(cx.host, reason::RESIDENCY_NOT_TRACKED, 0);
    let end = address + pages * PAGE_SIZE;
    let mut at = address;
    let mut written = 0;
    while at < end {
        let Some(mapping) = kernel.process_of(tid).map.find(at) else {
            return Err(Errno::ENOMEM);
        };
        let to = mapping.end.min(end);
        let count = (to - at) / PAGE_SIZE;
        super::guest(kernel, cx, tid).write(vector + written, &vec![1u8; count as usize])?;
        written += count;
        at = to;
    }
    Ok(0)
}

/// The protection keys a process can hold: ARM64 Linux allows eight with the CPU's permission overlays
/// and only key 0 without them, as on the reference (which lacks them).
const PROTECTION_KEYS: i32 = 1;

fn key_allocated(map: &crate::memory::AddressMap, key: i32) -> bool {
    (0..PROTECTION_KEYS).contains(&key) && map.protection_keys & (1 << key) != 0
}

/// `pkey_alloc`: with no permission overlays there is never a key to give (ENOSPC).
pub(super) fn pkey_alloc(
    _: &mut Kernel,
    _: &mut Context<'_>,
    _: Tid,
    [flags, access, ..]: Args,
) -> Result<u64, Errno> {
    // ARM64's `PKEY_ACCESS_MASK`: no access, no write, no execute, no read.
    const ACCESS_MASK: u64 = 0xf;
    if flags != 0 || access & !ACCESS_MASK != 0 {
        return Err(Errno::EINVAL);
    }
    Err(Errno::ENOSPC)
}

pub(super) fn pkey_free(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [key, ..]: Args,
) -> Result<u64, Errno> {
    let key = key as i32;
    let map = &mut kernel.process_of_mut(tid).map;
    if !key_allocated(map, key) {
        return Err(Errno::EINVAL);
    }
    map.protection_keys &= !(1 << key);
    Ok(0)
}

/// `map_shadow_stack`: the CPU has no guarded control stacks (as the reference's), so EOPNOTSUPP
/// before anything else.
pub(super) fn map_shadow_stack(
    _: &mut Kernel,
    _: &mut Context<'_>,
    _: Tid,
    _: Args,
) -> Result<u64, Errno> {
    Err(Errno::EOPNOTSUPP)
}
