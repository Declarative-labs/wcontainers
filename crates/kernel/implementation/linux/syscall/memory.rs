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
use crate::system::AccessKind;
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
    let may_lock_any = kernel.credentials(tid).has(CAP_IPC_LOCK);
    let process = kernel.process_of_mut(tid);
    let mut map = process.map.borrow_mut();
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
    // Growing past the ceiling is outgrowing it, whatever lies in the way (the stack sits just
    // below the ceiling). Otherwise the new part must not touch the next mapping, nor a stack's
    // guard gap.
    if new_end > map.end || map.crosses_ceiling(new_end, new_end - old_end) {
        return Ok(old);
    }
    if let Some(next) = map.next_after(old_end)
        && next.start_gap() < new_end
    {
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
        may_lock_any,
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
        let file = kernel.call_file(tid, fd)?;
        if file.borrow().is_path() {
            return Err(Errno::EBADF);
        }
        Some(file)
    };
    do_mmap(kernel, cx, tid, address, length, prot, flags, file, offset)
}

/// Why a mapping of huge pages fails: none is free to reserve; one that reserves none
/// (`MAP_NORESERVE`), which Linux makes and faults on every touch, is refused and counted.
fn no_huge_pages(kernel: &mut Kernel, cx: &mut Context<'_>, flags: u32) -> Errno {
    if flags & MAP_NORESERVE != 0 {
        return kernel.counters.refuse_flag(
            cx.host,
            __NR_mmap,
            MAP_NORESERVE.into(),
            Errno::ENOMEM,
        );
    }
    Errno::ENOMEM
}

/// Linux's `do_mmap`: maps `file` (or anonymous memory) at `offset`, which is page aligned.
#[allow(clippy::too_many_arguments, reason = "the call's own arguments")]
pub(super) fn do_mmap(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    address: u64,
    length: u64,
    prot: u64,
    flags: u32,
    file: Option<crate::file::FileRef>,
    offset: u64,
) -> Result<u64, Errno> {
    // `ksys_mmap_pgoff`: a hugetlbfs file is mapped in whole huge pages; anonymous huge pages are
    // a hugetlbfs file made for the mapping, whose pages are reserved at once unless
    // `MAP_NORESERVE` (`hugetlb_file_setup`). The machine holds no huge pages, so a reservation
    // fails; a mapping that reserves none, whose every touch would fault, is not built.
    let huge = file
        .as_ref()
        .and_then(|file| file.borrow().node.borrow().huge_page_size());
    let length = match huge {
        Some(huge) => length.next_multiple_of(huge),
        None if flags & MAP_HUGETLB == 0 => length,
        None if file.is_some() => return Err(Errno::EINVAL),
        None => {
            crate::hugetlb::page_shift((flags >> MAP_HUGE_SHIFT) & MAP_HUGE_MASK)
                .map_err(|_| Errno::EINVAL)?;
            return Err(no_huge_pages(kernel, cx, flags));
        }
    };
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
    let map = kernel.process_of(tid).map.borrow();
    let fixed = flags & (MAP_FIXED | MAP_FIXED_NOREPLACE) != 0;
    // Past the host's address space and inside Linux's: the plan's small address space.
    let above = address
        .checked_add(length)
        .is_some_and(|end| end > map.end && end <= TASK_SIZE);
    let mut hint_above = false;
    let start = if fixed {
        if length > map.end || address > map.end - length {
            if above {
                drop(map);
                kernel
                    .counters
                    .count(cx.host, crate::reason::ADDRESS_ABOVE_SPACE, address);
            }
            return Err(Errno::ENOMEM);
        }
        if !address.is_multiple_of(PAGE_SIZE) {
            return Err(Errno::EINVAL);
        }
        if address < LOWEST_ADDRESS {
            return Err(Errno::EPERM);
        }
        if map.crosses_ceiling(address + length, length) {
            return Err(Errno::ENOMEM);
        }
        address
    } else {
        let hint = page_down(address);
        let hint_fits = hint >= LOWEST_ADDRESS
            && hint
                .checked_add(length)
                .is_some_and(|end| end <= map.ceiling && map.free_for_placement(hint, end));
        if hint != 0 && hint_fits {
            hint
        } else {
            hint_above = hint != 0 && above;
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
    drop(map);
    if hint_above {
        kernel
            .counters
            .count(cx.host, crate::reason::ADDRESS_ABOVE_SPACE, address);
    }
    if flags & MAP_LOCKED != 0 && !can_lock(kernel, tid) {
        return Err(Errno::EPERM);
    }
    let process = kernel.process_of(tid);
    if !process.map.borrow().may_lock(
        &process.limits,
        locked,
        length / PAGE_SIZE,
        kernel.credentials(tid).has(CAP_IPC_LOCK),
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
    // A mapping of a backend file: the open file its bytes are read through.
    let mut backend = None;
    // A mapping of a socket: whether it is TCP's, whose `mmap` is asked after the limits.
    let mut socket_tcp = None;
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
            // `fuse_file_mmap`: a shared mapping of a file opened for direct reads and writes is
            // refused, as Linux refuses it.
            if matches!(inode.content, Content::Backend(_)) {
                let handle = file.backend.clone().ok_or(Errno::ENODEV)?;
                if shared && !handle.may_map_shared() {
                    return Err(Errno::ENODEV);
                }
                backend = Some(handle);
            }
            // A socket has an `mmap` (`sock_mmap`), so the checks after it come first; the
            // protocol's refuses it later (`sock_no_mmap`).
            let socket = matches!(inode.content, Content::Socket(_));
            if !zero
                && backend.is_none()
                && !socket
                && !matches!(inode.content, Content::Regular(_))
            {
                return Err(Errno::ENODEV);
            }
            if grows_down {
                return Err(Errno::EINVAL);
            }
            if socket {
                socket_tcp = Some(
                    matches!(&inode.content, Content::Socket(socket) if socket.borrow().tcp()),
                );
            }
            // `hugetlbfs_file_mmap`: whole huge pages from the file's start, reserved now.
            if let Some(huge) = huge {
                if !offset.is_multiple_of(huge) {
                    return Err(Errno::EINVAL);
                }
                drop(inode);
                drop(file);
                return Err(no_huge_pages(kernel, cx, flags));
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
                if backend.is_none() && !socket {
                    super::io::accessed(kernel, cx, &node, file.flags);
                }
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
            // The backend says where its file ends now, and its pages are read before anything
            // changes, since the call runs again once they come.
            if let Some(handle) = &backend {
                crate::fs::backend::getattr(node, Some(handle.fh))?;
                let writer = shared && allowed.contains(Permissions::WRITE);
                crate::shared::backend_block(
                    kernel,
                    cx,
                    node,
                    Some(handle),
                    offset + length,
                    writer,
                )?;
            }
            let size = match &mut node.borrow_mut().content {
                Content::Regular(regular) => {
                    regular.set_mapped();
                    regular.size()
                }
                Content::Backend(remote) => remote.size.get(),
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
    let may_lock_any = kernel.credentials(tid).has(CAP_IPC_LOCK);
    let process = kernel.process_of_mut(tid);
    let mut map = process.map.borrow_mut();
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
    // `tcp_mmap`: a TCP socket maps for reading only, and never later for more; its pages come
    // only from TCP_ZEROCOPY_RECEIVE (not built), so touching one is SIGBUS. Every other protocol
    // has `sock_no_mmap`. The file's `mmap` runs after what a fixed mapping replaces was taken
    // away, so a refusal leaves the range unmapped (`vms_abort_munmap_vmas`).
    let refused = match socket_tcp {
        Some(false) => Some(Errno::ENODEV),
        Some(true) if permissions.intersects(Permissions::WRITE | Permissions::EXECUTE) => {
            Some(Errno::EPERM)
        }
        _ => None,
    };
    if let Some(error) = refused {
        if fixed {
            let memory = process.memory;
            unmap(&mut map, cx, memory, start, length)?;
            drop(map);
            crate::shared::release(kernel, cx);
        }
        return Err(error);
    }
    if socket_tcp == Some(true) {
        mapping
            .allowed
            .remove(Permissions::WRITE | Permissions::EXECUTE);
    }
    // A secret memory file is mapped only shared, and locked, within the limit on locked memory
    // (`secretmem_mmap`).
    if mapping.is_secret() {
        if !shared {
            return Err(Errno::EINVAL);
        }
        if !map.may_lock(&process.limits, Lock::Now, length / PAGE_SIZE, may_lock_any) {
            return Err(Errno::EAGAIN);
        }
        mapping.locked = Lock::Now;
    }
    // `mmap_region` takes the lock off a `VM_SPECIAL` mapping.
    if mapping.is_special() {
        mapping.locked = Lock::None;
    }
    let memory = process.memory;
    map.insert(cx.cpu, memory, mapping.clone())?;
    drop(map);
    crate::shared::show(kernel, cx, memory, &mapping)?;
    // `do_mmap`'s populate: a locked mapping, or one asked to be filled at once, is faulted in, and
    // writing a private one copies its file pages.
    if mapping.locked == Lock::Now || flags & (MAP_POPULATE | MAP_NONBLOCK) == MAP_POPULATE {
        populate(kernel, cx, tid, start, start + length, true);
    }
    crate::shared::release(kernel, cx);
    Ok(start)
}

/// `remap_file_pages`, as Linux emulates it: the shared file mapping at `start` is mapped again
/// over `[start, start + size)` from file page `pgoff`, with its own protection, and filled at once
/// unless `MAP_NONBLOCK`. The range must lie in mappings of the same file with the same flags.
pub(super) fn remap_file_pages(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [start, size, prot, pgoff, flags, _]: Args,
) -> Result<u64, Errno> {
    if prot != 0 {
        return Err(Errno::EINVAL);
    }
    let start = page_down(start);
    let size = page_down(size);
    let end = start.checked_add(size).filter(|end| *end > start);
    let end = end.ok_or(Errno::EINVAL)?;
    pgoff.checked_add(size / PAGE_SIZE).ok_or(Errno::EINVAL)?;
    let process = kernel.process_of(tid);
    let map = process.map.borrow();
    let first = map.find(start).filter(|mapping| mapping.shared);
    let first = first.ok_or(Errno::EINVAL)?.clone();
    let Backing::File { node, path, .. } = &first.backing else {
        return Err(Errno::EINVAL);
    };
    let (node, path) = (node.clone(), path.clone());
    let same = |mapping: &Mapping| {
        matches!(&mapping.backing, Backing::File { node: other, .. } if alloc::rc::Rc::ptr_eq(other, &node))
            && mapping.permissions == first.permissions
            && mapping.allowed == first.allowed
            && mapping.shared
            && mapping.locked == first.locked
            && mapping.grows_down == first.grows_down
            && mapping.on_fork == first.on_fork
    };
    let mut reached = first.end;
    while reached < end {
        match map.find(reached) {
            Some(next) if same(next) => reached = next.end,
            _ => return Err(Errno::EINVAL),
        }
    }
    drop(map);
    // `file_mmap_ok`: the window lies within the largest file (`MAX_LFS_FILESIZE`).
    const LARGEST_FILE: u64 = i64::MAX as u64;
    if pgoff > LARGEST_FILE / PAGE_SIZE || size > LARGEST_FILE - pgoff * PAGE_SIZE {
        return Err(Errno::EOVERFLOW);
    }
    let offset = pgoff * PAGE_SIZE;
    // `shm_mmap`: the window's old mappings close first, which destroys a removed segment nothing
    // else shows; a segment its id no longer names fails the open and leaves the window unmapped.
    if node.borrow().is_segment() {
        let pid = kernel.threads[&tid].process;
        let process = kernel.process_of_mut(tid);
        let memory = process.memory;
        process.map.borrow_mut().unmap(cx.cpu, memory, start, end)?;
        super::ipc::settle(kernel, cx, pid);
        if !super::ipc::segment_exists(kernel, &node) {
            crate::shared::release(kernel, cx);
            return Err(Errno::EINVAL);
        }
    }
    if matches!(node.borrow().content, Content::Backend(_)) {
        crate::fs::backend::getattr(&node, None)?;
        let writer = first.allowed.contains(Permissions::WRITE);
        crate::shared::backend_block(kernel, cx, &node, None, offset + size, writer)?;
    }
    let file_size = match &node.borrow().content {
        Content::Regular(regular) => regular.size(),
        Content::Backend(remote) => remote.size.get(),
        _ => 0,
    };
    let file_pages = page_up(file_size).saturating_sub(offset);
    let mapping = Mapping {
        start,
        end,
        backing: Backing::File {
            node,
            path,
            offset,
            file_end: start + file_pages.min(size),
            copied: alloc::collections::BTreeSet::new(),
        },
        ..first
    };
    let process = kernel.process_of(tid);
    let memory = process.memory;
    process
        .map
        .borrow_mut()
        .insert(cx.cpu, memory, mapping.clone())?;
    crate::shared::show(kernel, cx, memory, &mapping)?;
    if flags as u32 & MAP_NONBLOCK == 0 || mapping.locked == Lock::Now {
        populate(kernel, cx, tid, start, end, true);
    }
    crate::shared::release(kernel, cx);
    Ok(0)
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
        let Some(mapping) = process.map.borrow().next_after(at).cloned() else {
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
                .borrow_mut()
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
    regular.anonymous = true;
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
    unmap(
        &mut process.map.borrow_mut(),
        cx,
        process.memory,
        address,
        length,
    )?;
    crate::shared::release(kernel, cx);
    Ok(0)
}

/// Linux's `do_vmi_munmap`: the range must lie below `TASK_SIZE` before its length is rounded up.
pub(super) fn unmap(
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
    if key != -1 && !key_allocated(&process.map.borrow(), key) {
        return Err(Errno::EINVAL);
    }
    let first = process
        .map
        .borrow()
        .next_after(address)
        .filter(|mapping| mapping.start < end)
        .cloned()
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
            .borrow()
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
                .borrow()
                .may_expand(&process.limits, true, pages, (pages, 0))
        {
            return Err(Errno::ENOMEM);
        }
        process
            .map
            .borrow_mut()
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
            | MADV_GUARD_INSTALL
            | MADV_GUARD_REMOVE
    );
    // Making huge pages now (MADV_COLLAPSE) is not built: refused as by a Linux without it.
    // Poisoning pages (MADV_HWPOISON, MADV_SOFT_OFFLINE) is not in the reference's kernel either.
    if advice == MADV_COLLAPSE {
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
    // Guard pages (`madvise_guard_install`, `madvise_guard_remove`), mapping by mapping: refused on
    // special mappings, and installing on locked ones; installing drops what private pages held.
    if matches!(advice, MADV_GUARD_INSTALL | MADV_GUARD_REMOVE) {
        let install = advice == MADV_GUARD_INSTALL;
        let memory = process.memory;
        let mut map = process.map.borrow_mut();
        let mut at = address;
        while let Some(mapping) = map.next_after(at).cloned() {
            if mapping.start >= end {
                break;
            }
            if mapping.is_special()
                || matches!(mapping.backing, Backing::Special(_))
                || (install && (mapping.locked != Lock::None || mapping.is_secret()))
            {
                return Err(Errno::EINVAL);
            }
            let (from, to) = (mapping.start.max(at), mapping.end.min(end));
            if install {
                drop_pages(cx, memory, &mut map, &mapping, from, to)?;
                map.add_guards(cx.cpu, memory, (from..to).step_by(PAGE_SIZE as usize))?;
            } else {
                map.remove_guards(cx.cpu, memory, from, to)?;
            }
            at = mapping.end;
        }
        if !map.fully_mapped(address, end) {
            return Err(Errno::ENOMEM);
        }
        return Ok(0);
    }
    // Removing pages punches a hole in what a shared mapping is backed by: its file, or its shared
    // anonymous memory. Linux's `madvise_remove`, mapping by mapping.
    if advice == MADV_REMOVE {
        let mut at = address;
        loop {
            let next = process.map.borrow().next_after(at).cloned();
            let Some(mapping) = next else {
                break;
            };
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
            process.map.borrow_mut().note_written(
                cx.cpu,
                process.memory,
                mapping.start.max(at),
                mapping.end.min(end),
            );
            at = mapping.end;
        }
        if !process.map.borrow().fully_mapped(address, end) {
            return Err(Errno::ENOMEM);
        }
        return Ok(0);
    }
    // Freeing lazily is for private anonymous memory only.
    if advice == MADV_FREE
        && process
            .map
            .borrow_mut()
            .iter()
            .filter(|mapping| mapping.start < end && mapping.end > address)
            .any(|mapping| mapping.shared || matches!(mapping.backing, Backing::File { .. }))
    {
        return Err(Errno::EINVAL);
    }
    // Locked memory is neither dropped nor aged (`madvise_dontneed_free_valid_vma`,
    // `can_madv_lru_vma`), whatever gaps the range has.
    if matches!(advice, MADV_DONTNEED | MADV_FREE | MADV_COLD | MADV_PAGEOUT)
        && process.map.borrow().iter().any(|mapping| {
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
            let Some(mapping) = process.map.borrow().next_after(at).cloned() else {
                break;
            };
            if mapping.start >= end {
                break;
            }
            let from = mapping.start.max(at);
            let to = mapping.end.min(end);
            drop_pages(
                cx,
                process.memory,
                &mut process.map.borrow_mut(),
                &mapping,
                from,
                to,
            )?;
            at = to;
        }
    }
    let process = kernel.process_of_mut(tid);
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
            let Some(mapping) = process.map.borrow().find(at).cloned() else {
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
            // A guard page faults with SIGSEGV, EFAULT here too.
            let reach = process.map.borrow().guard_from(at, reach);
            if advice == MADV_POPULATE_WRITE {
                process
                    .map
                    .borrow_mut()
                    .copy_on_write(cx.cpu, process.memory, at, reach)?;
            }
            if reach < to {
                return Err(Errno::EFAULT);
            }
            at = mapping.end;
        }
    }
    // What a fork gives the child and whether a core dump holds it, mapping by mapping as
    // `madvise_vma_behavior`: a mapping that refuses the advice ends the walk with the ones before it
    // changed. Wiping is for private anonymous memory; droppable memory is always wiped and never
    // dumped; the kernel's own pages (`VM_SPECIAL`) are never dumped.
    let change: Option<fn(&mut crate::memory::OnFork)> = match advice {
        MADV_DONTFORK => Some(|on_fork| on_fork.skip = true),
        MADV_DOFORK => Some(|on_fork| on_fork.skip = false),
        MADV_WIPEONFORK => Some(|on_fork| on_fork.wipe = true),
        MADV_KEEPONFORK => Some(|on_fork| on_fork.wipe = false),
        _ => None,
    };
    let mapped: Vec<(u64, u64, bool)> = process
        .map
        .borrow_mut()
        .iter()
        .filter(|mapping| mapping.start < end && mapping.end > address)
        .map(|mapping| {
            let refused = match advice {
                MADV_WIPEONFORK => {
                    mapping.shared || matches!(mapping.backing, Backing::File { .. })
                }
                MADV_KEEPONFORK => matches!(mapping.backing, Backing::Droppable),
                MADV_DODUMP => matches!(mapping.backing, Backing::Droppable | Backing::Special(_)),
                _ => false,
            };
            (mapping.start.max(address), mapping.end.min(end), refused)
        })
        .collect();
    for (from, to, refused) in mapped {
        if refused {
            return Err(Errno::EINVAL);
        }
        if let Some(change) = change {
            process.map.borrow_mut().set_on_fork(from, to, change);
        }
    }
    if !process.map.borrow().fully_mapped(address, end) {
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
        .borrow()
        .find(old)
        .cloned()
        .ok_or(Errno::EFAULT)?;
    prepare(kernel, tid, &mapping, &request)?;
    // A backend file's pages for the grown range are read before anything changes.
    if let Backing::File { node, offset, .. } = &mapping.backing
        && crate::fs::backend::remote(node).is_some()
        && new_length > old_length
    {
        let end = offset + (old - mapping.start) + new_length;
        crate::shared::backend_block(kernel, cx, node, None, end, false)?;
    }
    let moved = if to_new_address {
        remap_to(kernel, cx, tid, &mapping, request)?
    } else {
        remap_at(kernel, cx, tid, &mapping, request)?
    };
    // A file mapping that grew shows the file as far as it reaches now.
    if let Backing::File { node, .. } = &mapping.backing {
        crate::shared::changed(kernel, cx, node)?;
    }
    Ok(moved)
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
    // `VM_DONTEXPAND`: a socket's mapping never grows.
    if mapping.is_special() {
        return Err(Errno::EFAULT);
    }
    let process = kernel.process_of(tid);
    let growth = (new_length - old_length) / PAGE_SIZE;
    if !process.map.borrow().may_lock(
        &process.limits,
        mapping.locked,
        growth,
        kernel.credentials(tid).has(CAP_IPC_LOCK),
    ) {
        return Err(Errno::EAGAIN);
    }
    if !process
        .map
        .borrow()
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
    let mut map = process.map.borrow_mut();
    if new_length == old_length {
        return Ok(old);
    }
    if new_length < old_length {
        unmap(
            &mut map,
            cx,
            memory,
            old + new_length,
            old_length - new_length,
        )?;
        return Ok(old);
    }
    // Grow in place when the range is the end of its mapping and the space after is free.
    let old_end = old + old_length;
    let grown_end = old.saturating_add(new_length);
    if mapping.end == old_end && grown_end <= map.ceiling && !map.any_mapped(old_end, grown_end) {
        let mut extra = mapping.cut_public(old_end, grown_end);
        if let Backing::File { file_end, .. } = &mut extra.backing {
            *file_end = (*file_end).min(grown_end).max(old_end);
        }
        map.insert(cx.cpu, memory, extra.clone())?;
        drop(map);
        crate::shared::show(kernel, cx, memory, &extra)?;
        return Ok(old);
    }
    if request.flags & MREMAP_MAYMOVE == 0 {
        return Err(Errno::ENOMEM);
    }
    let destination = map.find_free(new_length).ok_or(Errno::ENOMEM)?;
    drop(map);
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
    let mut map = process.map.borrow_mut();
    // Outgrowing the ceiling is decided before anything is unmapped.
    if request.flags & MREMAP_FIXED != 0
        && request.new_address + request.new_length <= map.end
        && map.crosses_ceiling(request.new_address + request.new_length, request.new_length)
    {
        return Err(Errno::ENOMEM);
    }
    if request.flags & MREMAP_FIXED != 0 {
        let end = (request.new_address + request.new_length).min(map.end);
        if request.new_address < end {
            map.unmap(cx.cpu, memory, request.new_address, end)?;
        }
    }
    if request.new_length < request.old_length {
        unmap(
            &mut map,
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
            && hint + request.new_length <= map.ceiling
            && !map.any_mapped(hint, hint + request.new_length)
        {
            hint
        } else {
            map.find_free(request.new_length).ok_or(Errno::ENOMEM)?
        }
    };
    drop(map);
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
        let Some(mapping) = kernel.process_of(tid).map.borrow().next_after(at).cloned() else {
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
    let mut map = process.map.borrow_mut();
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
    // The embedder's watches go with the pages, which are new where they land.
    let moved_length = old_length.min(new_length);
    let watches = map.take_watches(old, old + moved_length);
    let mut moved = mapping.cut_public(old, old + new_length);
    let shift = |address: u64| address - old + destination;
    moved.start = destination;
    moved.end = destination + new_length;
    if let Backing::File { file_end, .. } = &mut moved.backing {
        *file_end = shift((*file_end).clamp(old, old + new_length));
    }
    // Guard pages go with the pages (`move_ptes`).
    let guards = map.take_guards(old, old + old_length.min(new_length));
    if request.flags & MREMAP_DONTUNMAP != 0 {
        drop_pages(cx, memory, &mut map, mapping, old, old_end)?;
        map.refresh(cx.cpu, memory, old, old_end)?;
    } else {
        map.unmap(cx.cpu, memory, old, old_end)?;
    }
    map.insert(cx.cpu, memory, moved.clone())?;
    map.add_guards(cx.cpu, memory, guards.into_iter().map(shift))?;
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
    map.moved_here(
        cx.cpu,
        memory,
        destination,
        destination + moved_length,
        watches,
    );
    drop(map);
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
    match &mapping.backing {
        Backing::File { .. } => map.drop_copies(cx.cpu, memory, from, to)?,
        _ => cx
            .cpu
            .zero(memory, from, to - from)
            .map_err(errno::from_cpu)?,
    }
    map.note_written(cx.cpu, memory, from, to);
    Ok(())
}

pub(super) fn msync(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
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
    let map = kernel.process_of(tid).map.borrow();
    if flags & MS_INVALIDATE != 0
        && map.iter().any(|mapping| {
            mapping.start < end && mapping.end > address && mapping.locked != Lock::None
        })
    {
        return Err(Errno::EBUSY);
    }
    // `MS_SYNC` syncs each shared mapping of a backend file in the range (`vfs_fsync_range`):
    // its pages written back, then FSYNC for the data, before a gap in the range is reported. A
    // socket's file has no fsync: EINVAL at its mapping.
    let fully = map.fully_mapped(address, end);
    let mut refused = false;
    let synced: Vec<(crate::fs::Node, u64, u64)> = map
        .iter()
        .filter(|mapping| {
            flags & MS_SYNC != 0 && mapping.shared && mapping.start < end && mapping.end > address
        })
        .take_while(|mapping| {
            refused = mapping.is_special();
            !refused
        })
        .filter_map(|mapping| match &mapping.backing {
            Backing::File { node, offset, .. } if crate::fs::backend::remote(node).is_some() => {
                let from = offset + (mapping.start.max(address) - mapping.start);
                let to = offset + (mapping.end.min(end) - mapping.start);
                Some((node.clone(), from, to))
            }
            _ => None,
        })
        .collect();
    drop(map);
    for (node, from, to) in synced {
        crate::shared::backend_write_back(kernel, cx, &node, from, to, true)?;
        let handle = match &node.borrow().content {
            Content::Backend(remote) => remote.pages.as_ref().map(|pages| pages.handle.clone()),
            _ => None,
        };
        if let Some(handle) = handle {
            crate::fs::backend::fsync(&handle, true)?;
        }
    }
    if refused {
        return Err(Errno::EINVAL);
    }
    if !fully {
        return Err(Errno::ENOMEM);
    }
    Ok(0)
}

/// Whether the caller may lock memory at all (Linux's `can_do_mlock`).
fn can_lock(kernel: &Kernel, tid: Tid) -> bool {
    let process = kernel.process_of(tid);
    process.limits[RLIMIT_MEMLOCK as usize].current != 0
        || kernel.credentials(tid).has(CAP_IPC_LOCK)
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
    let may_lock_any = kernel.credentials(tid).has(CAP_IPC_LOCK);
    let process = kernel.process_of_mut(tid);
    if lock != Lock::None {
        let (locked, within) = process
            .map
            .borrow()
            .locked_pages(start, start.saturating_add(length));
        let limit = process.limits[RLIMIT_MEMLOCK as usize].current / PAGE_SIZE;
        if locked + length / PAGE_SIZE - within > limit && !may_lock_any {
            return Err(Errno::ENOMEM);
        }
    }
    let end = start.checked_add(length).ok_or(Errno::EINVAL)?;
    if end == start {
        return Ok(0);
    }
    if !process.map.borrow().fully_mapped(start, end) {
        return Err(Errno::ENOMEM);
    }
    process.map.borrow_mut().set_lock(start, end, lock);
    // Linux's `__mm_populate`: bringing the pages in fails at one that cannot be read (no access, or
    // wholly past its file's end), and the call is ENOMEM though the range stays locked.
    let unreadable = process
        .map
        .borrow_mut()
        .iter()
        .filter(|mapping| mapping.start < end && mapping.end > start)
        .any(|mapping| {
            mapping.permissions.is_empty()
                || matches!(mapping.backing, Backing::File { file_end, .. } if file_end < end.min(mapping.end))
        });
    // Droppable memory that allows nothing is ENOMEM to either kind of lock (measured).
    let droppable_closed = process.map.borrow().iter().any(|mapping| {
        mapping.start < end
            && mapping.end > start
            && matches!(mapping.backing, Backing::Droppable)
            && mapping.permissions.is_empty()
    });
    if lock == Lock::Now {
        populate(kernel, cx, tid, start, end, false);
        if unreadable {
            return Err(Errno::ENOMEM);
        }
    }
    if lock != Lock::None && droppable_closed {
        return Err(Errno::ENOMEM);
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
    let may_lock_any = kernel.credentials(tid).has(CAP_IPC_LOCK);
    let process = kernel.process_of_mut(tid);
    let lock = if flags & MCL_ONFAULT != 0 {
        Lock::OnFault
    } else {
        Lock::Now
    };
    if flags & MCL_CURRENT != 0 {
        let total = process.map.borrow().pages_in(0, u64::MAX).0;
        if total > process.limits[RLIMIT_MEMLOCK as usize].current / PAGE_SIZE && !may_lock_any {
            return Err(Errno::ENOMEM);
        }
        process.map.borrow_mut().set_lock_all(lock);
    }
    process.map.borrow_mut().future_lock = if flags & MCL_FUTURE != 0 {
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
    let mut map = kernel.process_of(tid).map.borrow_mut();
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
        let Some(mapping) = kernel.process_of(tid).map.borrow().find(at).cloned() else {
            return Err(Errno::ENOMEM);
        };
        let to = mapping.end.min(end);
        let count = (to - at) / PAGE_SIZE;
        // A socket's mapping never holds a page here (none come without TCP_ZEROCOPY_RECEIVE).
        let resident = u8::from(!mapping.is_special());
        super::guest(kernel, cx, tid).write(vector + written, &vec![resident; count as usize])?;
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
    let mut map = kernel.process_of(tid).map.borrow_mut();
    if !key_allocated(&map, key) {
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
    // `mm_access` with PTRACE_MODE_ATTACH_REALCREDS: another process's memory only for a caller
    // that may trace it, judged by its real ids (EACCES, answered as EPERM).
    let remote = target.process;
    if !crate::fs::proc::ptrace_may_read_as(&kernel.processes, &kernel.threads, tid, remote, true) {
        return Err(Errno::EPERM);
    }
    let local_process = kernel.threads[&tid].process;
    let mut local = local.into_iter().filter(|(_, length)| *length > 0);
    let mut current = local.next();
    let mut copied = 0;
    let remote_access = if write {
        AccessKind::Write
    } else {
        AccessKind::Read
    };
    for (base, length) in targets {
        if remaining == 0 {
            break;
        }
        if length == 0 {
            continue;
        }
        let wanted = length.min(remaining);
        // The other side's watches are met as a call's own copies meet them.
        let reach = {
            let mut map = kernel.processes[&remote].map.borrow_mut();
            let pinnable = map.pinnable(base, wanted, write);
            map.watches
                .copy_reach(base, wanted, pinnable, remote_access)
        };
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
            let moved = super::guest(kernel, cx, tid).accessible(local_base, size, local_right);
            let (from, to) = if write {
                ((local_process, local_base), (remote, at))
            } else {
                ((remote, at), (local_process, local_base))
            };
            if remote == local_process {
                super::guest(kernel, cx, tid).copy_within(from.1, to.1, moved)?;
            } else {
                let bytes = process_guest(kernel, cx, from.0, tid).read(from.1, moved)?;
                process_guest(kernel, cx, to.0, tid).write(to.1, &bytes)?;
            }
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

/// `mem_rw`: reads or writes the memory `/proc/<pid>/mem` was opened on, from the position, a page's
/// worth at a time through a kernel page, as `access_remote_vm` with FOLL_FORCE reaches it (the
/// reference's PROC_MEM_ALWAYS_FORCE). Nothing moves once that memory is gone (its process ended or
/// ran another program). Returns what moved, or EIO if nothing could, or EFAULT at a fault on the
/// caller's side; the position moves to where it stopped.
#[allow(clippy::too_many_arguments, reason = "a read or write's own arguments")]
pub(super) fn proc_mem(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &crate::file::FileRef,
    buffer: u64,
    count: u64,
    at: Option<u64>,
    write: bool,
) -> Result<u64, Errno> {
    let (pid, memory, position) = {
        let file = file.borrow();
        let Content::Proc(crate::fs::proc::Entry::ProcessFile(task, _)) =
            file.node.borrow().content
        else {
            return Err(Errno::EINVAL);
        };
        (task.pid, file.memory, at.unwrap_or(file.offset))
    };
    let live = kernel
        .processes
        .get(&pid)
        .is_some_and(|process| process.ended.is_none() && Some(process.memory) == memory);
    if !live {
        return Ok(0);
    }
    let mut address = position;
    let mut copied = 0;
    let mut result = Ok(());
    while copied < count {
        let length = (count - copied).min(PAGE_SIZE);
        let moved = if write {
            let Ok(bytes) = super::guest(kernel, cx, tid).read(buffer + copied, length) else {
                result = Err(Errno::EFAULT);
                break;
            };
            process_guest(kernel, cx, pid, tid).forced_write(address, &bytes)
        } else {
            let bytes = process_guest(kernel, cx, pid, tid).forced_read(address, length);
            if !bytes.is_empty()
                && super::guest(kernel, cx, tid)
                    .write(buffer + copied, &bytes)
                    .is_err()
            {
                result = Err(Errno::EFAULT);
                break;
            }
            bytes.len() as u64
        };
        if moved == 0 {
            if copied == 0 {
                result = Err(Errno::EIO);
            }
            break;
        }
        address = address.wrapping_add(moved);
        copied += moved;
    }
    if at.is_none() {
        file.borrow_mut().offset = address;
    }
    result.map(|()| copied)
}

/// A process's memory, as another process's call (by `tid`) reaches it.
fn process_guest<'a>(
    kernel: &'a mut Kernel,
    cx: &'a mut Context<'_>,
    pid: crate::process::Pid,
    tid: Tid,
) -> crate::user::Guest<'a> {
    let process = &kernel.processes[&pid];
    crate::user::Guest {
        cpu: cx.cpu,
        memory: process.memory,
        map: &process.map,
        limits: &process.limits,
        may_lock_any: false,
        pid,
        tid,
        written: &kernel.kernel_writes,
    }
}

/// The end of a copy that faulted: what was copied, or `EFAULT` if nothing was.
fn finish(copied: u64) -> Result<u64, Errno> {
    if copied > 0 {
        Ok(copied)
    } else {
        Err(Errno::EFAULT)
    }
}
