//! The memory calls: the program break, mappings and their protection.

use alloc::vec;
use alloc::vec::Vec;

use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE, Permissions};

use super::Args;
use crate::counters::reason;
use crate::fs::Content;
use crate::fs::dev::Device;
use crate::linux::general::*;
use crate::linux_extra::LEGACY_MAP_MASK;
use crate::memory::{Backing, Lock, Mapping, page_align, page_down, page_up};
use crate::process::Tid;
use crate::{Context, Errno, Kernel, errno};

/// What a protection argument allows. `mprotect` refuses bits ARM64 does not have; `mmap` ignores them.
fn permissions(prot: u64, strict: bool) -> Result<Permissions, Errno> {
    if strict && prot & !u64::from(PROT_READ | PROT_WRITE | PROT_EXEC | PROT_SEM) != 0 {
        return Err(Errno::EINVAL);
    }
    let prot = prot as u32;
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
    if let Some(next) = map.next_after(old_end)
        && next.start_gap() < new_end
    {
        return Ok(old);
    }
    if new_end > map.end {
        return Ok(old);
    }
    // Linux's `check_data_rlimit` on the heap and the program's data, then `may_expand_vm` on the
    // growth.
    let data_limit = process.limits[RLIMIT_DATA as usize].current;
    if (address - map.brk_start).saturating_add(map.data_size) > data_limit
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
        allowed: Permissions::all(),
        shared: false,
        locked: map.future_lock,
        grows_down: false,
        on_fork: crate::memory::OnFork::default(),
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
    // `ksys_mmap_pgoff`: only a hugetlbfs file takes huge pages, and none can be made here.
    if flags & MAP_HUGETLB != 0 && file.is_some() {
        return Err(Errno::EINVAL);
    }
    if flags & MAP_HUGETLB != 0 {
        return Err(kernel.counters.refuse_flag(
            cx.host,
            __NR_mmap,
            MAP_HUGETLB.into(),
            Errno::ENOMEM,
        ));
    }
    if length == 0 {
        return Err(Errno::EINVAL);
    }
    let permissions = permissions(prot, false)?;
    let length = page_align(length);
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
                .is_some_and(|end| end <= map.end && map.free_for_placement(hint, end));
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
    // Validated sharing exists only for files, droppable memory only without one (`do_mmap`).
    let grows_down = flags & MAP_GROWSDOWN != 0;
    let droppable = flags & MAP_TYPE == MAP_DROPPABLE && file.is_none();
    let shared = match (flags & MAP_TYPE, file.is_some()) {
        (MAP_PRIVATE, _) => false,
        (MAP_SHARED, false) if grows_down => return Err(Errno::EINVAL),
        (MAP_SHARED, _) | (MAP_SHARED_VALIDATE, true) => true,
        (MAP_DROPPABLE, false) if flags & MAP_LOCKED != 0 || grows_down => {
            return Err(Errno::EINVAL);
        }
        (MAP_DROPPABLE, false) => false,
        _ => return Err(Errno::EINVAL),
    };
    let mut allowed = Permissions::all();
    // What backs the mapping, with Linux's checks on the file; `/dev/zero` gives zeroed memory.
    let mut zero_device = None;
    let file = match file {
        None => None,
        Some(file) => {
            let file = file.borrow();
            let node = file.node.clone();
            let inode = node.borrow();
            if flags & MAP_TYPE == MAP_SHARED_VALIDATE && flags & !LEGACY_MAP_MASK != 0 {
                return Err(Errno::EOPNOTSUPP);
            }
            if shared {
                if permissions.contains(Permissions::WRITE) && !file.writable() {
                    return Err(Errno::EACCES);
                }
                if !file.writable() {
                    allowed.remove(Permissions::WRITE);
                }
            }
            if !file.readable() {
                return Err(Errno::EACCES);
            }
            // Running code from a `noexec` mount is refused, then and later.
            if inode.fs.no_execute() {
                if permissions.contains(Permissions::EXECUTE) {
                    return Err(Errno::EPERM);
                }
                allowed.remove(Permissions::EXECUTE);
            }
            let zero = matches!(inode.content, Content::Special)
                && Device::from_number(inode.rdev) == Some(Device::Zero);
            if !zero && !matches!(inode.content, Content::Regular(_)) {
                return Err(Errno::ENODEV);
            }
            if grows_down {
                return Err(Errno::EINVAL);
            }
            // A sealed file is not written through a mapping, then or later (`memfd_check_seals_mmap`).
            if let Content::Regular(regular) = &inode.content
                && shared
                && regular.seals & (F_SEAL_WRITE | F_SEAL_FUTURE_WRITE) != 0
            {
                if permissions.contains(Permissions::WRITE) {
                    return Err(Errno::EPERM);
                }
                allowed.remove(Permissions::WRITE);
            }
            if zero {
                zero_device = Some((inode.fs.device, inode.ino, file.path.clone()));
                None
            } else {
                drop(inode);
                // `shmem_mmap` counts mapping as reading the file.
                super::io::accessed(kernel, cx, &node, file.flags);
                Some((node, file.path.clone()))
            }
        }
    };
    // Linux's `shmem_zero_setup`: shared anonymous memory and shared `/dev/zero` are a shared
    // mapping of a removed file on its internal tmpfs, from offset 0 for anonymous memory and where
    // asked for `/dev/zero`, so a child sees the same pages.
    // Shared anonymous memory starts at offset 0 of its file, whatever offset was given (`do_mmap`).
    let offset = if shared && file.is_none() && zero_device.is_none() {
        0
    } else {
        offset
    };
    let file = match file {
        None if shared => Some((
            shared_zero_file(kernel, cx, offset + length)?,
            b"/dev/zero".to_vec(),
        )),
        file => file,
    };
    let backing = match (&file, zero_device) {
        // A private mapping of `/dev/zero` is anonymous memory that keeps the file
        // (`mmap_zero_private_success`).
        (None, Some((device, ino, path))) => Backing::Zero {
            device,
            ino,
            offset,
            name: path,
        },
        (None, None) if droppable => Backing::Droppable,
        (None, None) => Backing::Anonymous,
        (Some((node, path)), _) => {
            let size = match &mut node.borrow_mut().content {
                Content::Regular(regular) => {
                    regular.set_mapped();
                    regular.size()
                }
                _ => 0,
            };
            let file_pages = page_up(size).saturating_sub(offset);
            Backing::File {
                node: node.clone(),
                path: path.clone(),
                offset,
                file_end: start + file_pages.min(length),
                copied: alloc::collections::BTreeSet::new(),
            }
        }
    };
    let process = kernel.process_of_mut(tid);
    let map = &mut process.map;
    let mut mapping = Mapping {
        start,
        end: start + length,
        permissions,
        allowed,
        shared,
        locked,
        grows_down,
        on_fork: crate::memory::OnFork::default(),
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
    // A secret memory file is mapped only shared, and locked, within the limit on locked memory
    // (`secretmem_mmap`).
    if mapping.is_secret() {
        if !shared {
            return Err(Errno::EINVAL);
        }
        if !map.may_lock(
            &process.limits,
            Lock::Now,
            length / PAGE_SIZE,
            process.credentials.has(CAP_IPC_LOCK),
        ) {
            return Err(Errno::EAGAIN);
        }
        mapping.locked = Lock::Now;
    }
    let memory = process.memory;
    map.insert(cx.cpu, memory, mapping.clone())?;
    crate::shared::show(kernel, cx, memory, &mapping)?;
    // `do_mmap`'s populate: a locked mapping, or one asked to be filled at once, is faulted in, and
    // writing a private one copies its file pages.
    if mapping.locked == Lock::Now || flags & (MAP_POPULATE | MAP_NONBLOCK) == MAP_POPULATE {
        populate(kernel, cx, tid, start, start + length, true);
    }
    crate::shared::release(kernel, cx);
    Ok(start)
}

/// Linux's `__mm_populate` over `[start, end)`, mapping by mapping in order until a page cannot be
/// brought in (then on at the next mapping if `ignore_errors`): a writable private file mapping is
/// faulted in for writing, which copies its file pages. Which pages are in memory is not tracked
/// otherwise.
fn populate(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    start: u64,
    end: u64,
    ignore_errors: bool,
) {
    let process = kernel.process_of_mut(tid);
    let mut at = start;
    while at < end {
        let Some(mapping) = process.map.next_after(at).cloned() else {
            return;
        };
        if mapping.start >= end {
            return;
        }
        let from = mapping.start.max(at);
        let to = mapping.end.min(end);
        if mapping.permissions.is_empty() {
            if !ignore_errors {
                return;
            }
            at = to;
            continue;
        }
        let reach = match &mapping.backing {
            Backing::File { file_end, .. } => to.min(*file_end).max(from),
            _ => to,
        };
        if mapping.permissions.contains(Permissions::WRITE)
            && process
                .map
                .copy_on_write(cx.cpu, process.memory, from, reach)
                .is_err()
        {
            return;
        }
        if reach < to && !ignore_errors {
            return;
        }
        at = to;
    }
}

/// The removed file on the internal tmpfs that shared anonymous memory is (`shmem_kernel_file_setup`).
fn shared_zero_file(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    size: u64,
) -> Result<crate::fs::Node, Errno> {
    let now = kernel.now(cx);
    let ino = kernel.files.numbers.ino();
    let mut regular = crate::fs::Regular::default();
    regular.truncate(cx.cpu, size);
    let node = crate::fs::new_inode(
        kernel.files.memfd_fs.clone(),
        ino,
        crate::fs::New {
            mode: S_IFREG | 0o777,
            uid: 0,
            gid: 0,
            rdev: 0,
            now,
            content: Content::Regular(regular),
            parent: None,
            name: b"dev/zero".to_vec(),
        },
    )?;
    node.borrow_mut().nlink = 0;
    Ok(node)
}

pub(super) fn munmap(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, length, ..]: Args,
) -> Result<u64, Errno> {
    let address = crate::user::untagged(address);
    let process = kernel.process_of_mut(tid);
    unmap(&mut process.map, cx, process.memory, address, length)?;
    crate::shared::release(kernel, cx);
    Ok(0)
}

/// Linux's `do_vmi_munmap`: the range must lie below `TASK_SIZE` before its length is rounded up.
fn unmap(
    map: &mut crate::memory::AddressMap,
    cx: &mut Context<'_>,
    memory: wcontainers_abi::MemoryId,
    start: u64,
    length: u64,
) -> Result<(), Errno> {
    if !start.is_multiple_of(PAGE_SIZE) || start > TASK_SIZE || length > TASK_SIZE - start {
        return Err(Errno::EINVAL);
    }
    let end = start + page_align(length);
    if end == start {
        return Err(Errno::EINVAL);
    }
    let end = end.min(map.end);
    if start < end {
        map.unmap(cx.cpu, memory, start, end)?;
    }
    Ok(())
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
    let address = crate::user::untagged(address);
    // A change may reach down to the start of a mapping that grows down (or up, which ARM64 has none
    // of), taken off the protection first.
    let grows = prot & u64::from(PROT_GROWSDOWN | PROT_GROWSUP);
    let prot = prot & !grows;
    if grows == u64::from(PROT_GROWSDOWN | PROT_GROWSUP) {
        return Err(Errno::EINVAL);
    }
    if !address.is_multiple_of(PAGE_SIZE) {
        return Err(Errno::EINVAL);
    }
    if length == 0 {
        return Ok(0);
    }
    let length = page_align(length);
    let end = address
        .checked_add(length)
        .filter(|end| *end > address)
        .ok_or(Errno::ENOMEM)?;
    let permissions = permissions(prot, true)?;
    let process = kernel.process_of_mut(tid);
    if key != -1 && !key_allocated(&process.map, key) {
        return Err(Errno::EINVAL);
    }
    let first = process
        .map
        .next_after(address)
        .filter(|mapping| mapping.start < end)
        .ok_or(Errno::ENOMEM)?;
    let address = if grows == u64::from(PROT_GROWSDOWN) {
        if !first.grows_down {
            return Err(Errno::EINVAL);
        }
        first.start
    } else if first.start > address {
        return Err(Errno::ENOMEM);
    } else if grows != 0 {
        return Err(Errno::EINVAL);
    } else {
        address
    };
    // `do_mprotect_pkey`: the start must be mapped; then mapping by mapping, keeping what it changed
    // before, the change stops at a hole (ENOMEM), at a mapping that may not have it (EACCES), or at
    // private pages that turning writable would take past the data limit (ENOMEM).
    let mut at = address;
    while at < end {
        let Some((stop, allowed, becomes_data)) = process
            .map
            .iter()
            .find(|mapping| mapping.start <= at && mapping.end > at)
            .map(|mapping| {
                (
                    mapping.end.min(end),
                    mapping.allowed,
                    permissions.contains(Permissions::WRITE)
                        && !mapping.is_data()
                        && crate::memory::is_data(permissions, mapping.shared, mapping.grows_down),
                )
            })
        else {
            return Err(Errno::ENOMEM);
        };
        if !allowed.contains(permissions) {
            return Err(Errno::EACCES);
        }
        let pages = (stop - at) / PAGE_SIZE;
        if becomes_data
            && !process
                .map
                .may_expand(&process.limits, true, pages, (pages, 0))
        {
            return Err(Errno::ENOMEM);
        }
        process
            .map
            .protect(cx.cpu, process.memory, at, stop, permissions)?;
        at = stop;
    }
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
            | MADV_DONTNEED_LOCKED
    );
    // Making huge pages now (MADV_COLLAPSE) and guard markers that make pages fault (MADV_GUARD_*)
    // are not built: refused as by a Linux without them. Poisoning pages (MADV_HWPOISON,
    // MADV_SOFT_OFFLINE) is not in the reference's kernel either.
    if matches!(
        advice,
        MADV_COLLAPSE | MADV_GUARD_INSTALL | MADV_GUARD_REMOVE
    ) {
        return Err(kernel.counters.refuse(
            cx.host,
            reason::ADVICE_NOT_BUILT,
            advice.into(),
            Errno::EINVAL,
        ));
    }
    if !known {
        return Err(Errno::EINVAL);
    }
    if !address.is_multiple_of(PAGE_SIZE) {
        return Err(Errno::EINVAL);
    }
    // `is_valid_madvise`: a length rounded up to 0 from the last page is refused, 0 itself is not.
    let aligned = page_align(length);
    if length != 0 && aligned == 0 {
        return Err(Errno::EINVAL);
    }
    let length = aligned;
    address.checked_add(length).ok_or(Errno::EINVAL)?;
    if length == 0 {
        return Ok(0);
    }
    // `madvise_do_behavior` strips the tag only after the checks.
    let address = crate::user::untagged(address);
    let end = address + length;
    let now = kernel.now(cx);
    let process = kernel.process_of_mut(tid);
    // Removing pages punches a hole in what a shared mapping is backed by: its file, or its shared
    // anonymous memory. Linux's `madvise_remove`, mapping by mapping.
    if advice == MADV_REMOVE {
        let mut at = address;
        while let Some(mapping) = process.map.next_after(at).cloned() {
            if mapping.start >= end {
                break;
            }
            if mapping.locked != Lock::None {
                return Err(Errno::EINVAL);
            }
            match &mapping.backing {
                Backing::File { node, offset, .. }
                    if mapping.shared && mapping.allowed.contains(Permissions::WRITE) =>
                {
                    // `vfs_fallocate` punching a hole, as tmpfs does it.
                    let from = offset + (mapping.start.max(at) - mapping.start);
                    let to = offset + (mapping.end.min(end) - mapping.start);
                    let mut inode = node.borrow_mut();
                    let Content::Regular(regular) = &mut inode.content else {
                        return Err(Errno::EINVAL);
                    };
                    if regular.seals & (F_SEAL_WRITE | F_SEAL_FUTURE_WRITE) != 0 {
                        return Err(Errno::EPERM);
                    }
                    regular.punch(cx.cpu, from, to);
                    inode.mtime = now;
                    inode.ctime = now;
                }
                Backing::File { .. } => return Err(Errno::EACCES),
                // A private mapping of `/dev/zero` has a file but may not write it.
                Backing::Zero { .. } if !mapping.shared => return Err(Errno::EACCES),
                Backing::Zero { .. } => {
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
    // Locked memory is neither dropped nor aged (`madvise_dontneed_free_valid_vma`,
    // `can_madv_lru_vma`), whatever gaps the range has.
    if matches!(advice, MADV_DONTNEED | MADV_FREE | MADV_COLD | MADV_PAGEOUT)
        && process.map.iter().any(|mapping| {
            mapping.start < end && mapping.end > address && mapping.locked != Lock::None
        })
    {
        return Err(Errno::EINVAL);
    }
    if matches!(advice, MADV_DONTNEED | MADV_DONTNEED_LOCKED) {
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
            drop_pages(cx, process.memory, &mut process.map, &mapping, from, to)?;
            at = to;
        }
    }
    // Populating faults the pages in mapping by mapping (`madvise_populate`): a mapping that does not
    // allow the access is EINVAL, a gap before it ENOMEM.
    if matches!(advice, MADV_POPULATE_READ | MADV_POPULATE_WRITE) {
        let wanted = if advice == MADV_POPULATE_WRITE {
            Permissions::WRITE
        } else {
            Permissions::READ
        };
        // Writing a private file mapping's pages copies them; a page wholly past its file's end
        // faults with SIGBUS, which is EFAULT here.
        let mut at = address;
        while at < end {
            let Some(mapping) = process.map.find(at).cloned() else {
                return Err(Errno::ENOMEM);
            };
            if !mapping.permissions.contains(wanted) {
                return Err(Errno::EINVAL);
            }
            let to = mapping.end.min(end);
            let reach = match &mapping.backing {
                Backing::File { file_end, .. } => to.min(*file_end).max(at),
                _ => to,
            };
            if advice == MADV_POPULATE_WRITE {
                process
                    .map
                    .copy_on_write(cx.cpu, process.memory, at, reach)?;
            }
            if reach < to {
                return Err(Errno::EFAULT);
            }
            at = mapping.end;
        }
    }
    // What a fork gives the child, set on what is mapped (`madvise_vma_behavior`).
    let change: Option<fn(&mut crate::memory::OnFork)> = match advice {
        MADV_DONTFORK => Some(|on_fork| on_fork.skip = true),
        MADV_DOFORK => Some(|on_fork| on_fork.skip = false),
        MADV_WIPEONFORK => Some(|on_fork| on_fork.wipe = true),
        MADV_KEEPONFORK => Some(|on_fork| on_fork.wipe = false),
        _ => None,
    };
    if let Some(change) = change {
        let mapped: Vec<(u64, u64)> = process
            .map
            .iter()
            .filter(|mapping| mapping.start < end && mapping.end > address)
            .map(|mapping| (mapping.start.max(address), mapping.end.min(end)))
            .collect();
        for (from, to) in mapped {
            process.map.set_on_fork(from, to, change);
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
    let old = crate::user::untagged(old);
    // In the order of Linux's `do_mremap` and `check_mremap_params`.
    let flags = flags as u32;
    let old_length = page_align(old_length);
    let new_length = page_align(new_length);
    if flags & !(MREMAP_MAYMOVE | MREMAP_FIXED | MREMAP_DONTUNMAP) != 0
        || !old.is_multiple_of(PAGE_SIZE)
    {
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
            || (old.wrapping_add(old_length) > new_address && new_address + new_length > old))
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
        unmap(map, cx, memory, old + new_length, old_length - new_length)?;
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
        crate::shared::show(kernel, cx, memory, &extra)?;
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
        unmap(
            map,
            cx,
            memory,
            request.old + request.new_length,
            request.old_length - request.new_length,
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
    // A file mapping shows its file wherever it goes; its own pages, and anything else, take their
    // bytes along.
    let kept = if mapping.shared {
        0
    } else {
        old_length.min(new_length)
    };
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
        drop_pages(cx, memory, map, mapping, old, old_end)?;
    } else {
        map.unmap(cx.cpu, memory, old, old_end)?;
    }
    map.insert(cx.cpu, memory, moved.clone())?;
    let kept_end = destination + kept;
    for (from, to, own) in moved.runs() {
        let to = to.min(kept_end);
        if own && !moved.shared && from < to {
            let at = (from - destination) as usize;
            cx.cpu
                .write(memory, from, &bytes[at..at + (to - from) as usize])
                .map_err(errno::from_cpu)?;
        }
    }
    crate::shared::show(kernel, cx, memory, &moved)?;
    crate::shared::release(kernel, cx);
    Ok(destination)
}

/// Puts a private range back to what it held before it was touched: zero, or the file's bytes. A
/// shared mapping's pages are what it shares and stay.
fn drop_pages(
    cx: &mut Context<'_>,
    memory: wcontainers_abi::MemoryId,
    map: &mut crate::memory::AddressMap,
    mapping: &Mapping,
    from: u64,
    to: u64,
) -> Result<(), Errno> {
    if mapping.shared {
        return Ok(());
    }
    match mapping.backing {
        Backing::File { .. } => map.drop_copies(cx.cpu, memory, from, to),
        _ => cx
            .cpu
            .zero(memory, from, to - from)
            .map_err(errno::from_cpu),
    }
}

pub(super) fn msync(
    kernel: &mut Kernel,
    _: &mut Context<'_>,
    tid: Tid,
    [address, length, flags, ..]: Args,
) -> Result<u64, Errno> {
    let address = crate::user::untagged(address);
    let flags = flags as u32;
    if !address.is_multiple_of(PAGE_SIZE)
        || flags & !(MS_ASYNC | MS_INVALIDATE | MS_SYNC) != 0
        || flags & (MS_ASYNC | MS_SYNC) == MS_ASYNC | MS_SYNC
    {
        return Err(Errno::EINVAL);
    }
    let end = address
        .checked_add(page_align(length))
        .ok_or(Errno::ENOMEM)?;
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
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    length: u64,
    lock: Lock,
) -> Result<u64, Errno> {
    let address = crate::user::untagged(address);
    if lock != Lock::None && !can_lock(kernel, tid) {
        return Err(Errno::EPERM);
    }
    let length = page_align(length.wrapping_add(address % PAGE_SIZE));
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
    if lock == Lock::Now {
        populate(kernel, cx, tid, start, end, false);
        if unreadable {
            return Err(Errno::ENOMEM);
        }
    }
    Ok(0)
}

pub(super) fn mlock(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, length, ..]: Args,
) -> Result<u64, Errno> {
    lock_range(kernel, cx, tid, address, length, Lock::Now)
}

pub(super) fn mlock2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
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
    lock_range(kernel, cx, tid, address, length, lock)
}

pub(super) fn munlock(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, length, ..]: Args,
) -> Result<u64, Errno> {
    let address = crate::user::untagged(address);
    lock_range(kernel, cx, tid, address, length, Lock::None)
}

pub(super) fn mlockall(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
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
    if flags & MCL_CURRENT != 0 && lock == Lock::Now {
        populate(kernel, cx, tid, 0, u64::MAX, true);
    }
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
    let address = crate::user::untagged(address);
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

pub(super) fn process_vm_readv(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    arguments: Args,
) -> Result<u64, Errno> {
    process_vm(kernel, cx, tid, arguments, false)
}

pub(super) fn process_vm_writev(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    arguments: Args,
) -> Result<u64, Errno> {
    process_vm(kernel, cx, tid, arguments, true)
}

/// Linux's `process_vm_rw`: copies between the caller's segments and another task's memory, which
/// is reached as `get_user_pages` reaches it. Returns what was copied, or the error if nothing was.
fn process_vm(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [pid, local, local_count, remote, remote_count, flags]: Args,
    write: bool,
) -> Result<u64, Errno> {
    use super::io::{MAX_SEGMENTS, segments};
    if flags != 0 {
        return Err(Errno::EINVAL);
    }
    let local = segments(&mut super::guest(kernel, cx, tid), local, local_count)?;
    let mut remaining: u64 = local.iter().map(|(_, length)| length).sum();
    if remaining == 0 {
        return Ok(0);
    }
    // The other task's segments are only read and checked for a negative length (`iovec_from_user`).
    if remote_count > MAX_SEGMENTS {
        return Err(Errno::EINVAL);
    }
    let bytes = super::guest(kernel, cx, tid).read(remote, remote_count * 16)?;
    let mut targets = Vec::with_capacity(remote_count as usize);
    for chunk in bytes.as_chunks::<16>().0 {
        let base = u64::from_le_bytes(chunk[..8].try_into().expect("8 bytes"));
        let length = u64::from_le_bytes(chunk[8..].try_into().expect("8 bytes"));
        if (length as i64) < 0 {
            return Err(Errno::EINVAL);
        }
        // The other side's pages are found as `get_user_pages` finds them, with the tag stripped.
        targets.push((crate::user::untagged(base), length));
    }
    if targets.iter().all(|(_, length)| *length == 0) {
        return Ok(0);
    }
    let Some(target) = kernel.threads.get(&(pid as i32)) else {
        return Err(Errno::ESRCH);
    };
    if target.process != kernel.threads[&tid].process {
        // Another process's memory needs Linux's `ptrace_may_access`, which comes with processes.
        let number = if write {
            __NR_process_vm_writev
        } else {
            __NR_process_vm_readv
        };
        return Err(kernel.counters.refuse(
            cx.host,
            reason::CROSS_PROCESS_NOT_BUILT,
            number.into(),
            Errno::ENOSYS,
        ));
    }
    let mut guest = super::guest(kernel, cx, tid);
    let mut local = local.into_iter().filter(|(_, length)| *length > 0);
    let mut current = local.next();
    let mut copied = 0;
    for (base, length) in targets {
        if remaining == 0 {
            break;
        }
        if length == 0 {
            continue;
        }
        let wanted = length.min(remaining);
        let reach = guest.map.pinnable(base, wanted, write);
        // Linux copies a page of the other side at a time, into as many local segments as it takes.
        let mut at = base;
        while at < base + reach {
            let Some((local_base, local_length)) = current else {
                break;
            };
            let page_end = (at / PAGE_SIZE + 1) * PAGE_SIZE;
            let size = local_length.min(base + reach - at).min(page_end - at);
            let local_right = if write {
                Permissions::READ
            } else {
                Permissions::WRITE
            };
            let moved = guest.accessible(local_base, size, local_right);
            let (from, to) = if write {
                (local_base, at)
            } else {
                (at, local_base)
            };
            guest.copy_within(from, to, moved)?;
            copied += moved;
            remaining -= moved;
            at += moved;
            if moved < size {
                return finish(copied);
            }
            current = if moved == local_length {
                local.next()
            } else {
                Some((local_base + moved, local_length - moved))
            };
        }
        if remaining > 0 && reach < wanted {
            return finish(copied);
        }
    }
    Ok(copied)
}

/// The end of a copy that faulted: what was copied, or `EFAULT` if nothing was.
fn finish(copied: u64) -> Result<u64, Errno> {
    if copied > 0 {
        Ok(copied)
    } else {
        Err(Errno::EFAULT)
    }
}
