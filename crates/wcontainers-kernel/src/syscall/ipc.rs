//! System V IPC as Linux's `ipc/` builds it for a container's own IPC namespace: identifiers,
//! keys and permissions shared by every kind of object, and shared memory segments, which are files on
//! the internal tmpfs mapped shared (`ipc/shm.c`).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use wcontainers_abi::PAGE_SIZE;
use zerocopy::IntoBytes;

use super::Args;
use super::guest;
use crate::counters::reason;
use crate::credentials::Credentials;
use crate::fs::{Content, Node};
use crate::linux::general::*;
use crate::linux_extra::*;
use crate::process::{Pid, Tid};
use crate::{Context, Errno, Kernel};

/// An object's key, owners, mode and identifier (Linux's `kern_ipc_perm`).
#[derive(Clone)]
struct Perm {
    pub key: i32,
    pub uid: u32,
    pub gid: u32,
    pub cuid: u32,
    pub cgid: u32,
    pub mode: u32,
    pub seq: u32,
    pub id: i32,
}

impl Perm {
    fn shown(&self) -> ipc64_perm {
        ipc64_perm {
            key: self.key,
            uid: self.uid,
            gid: self.gid,
            cuid: self.cuid,
            cgid: self.cgid,
            mode: self.mode,
            seq: self.seq as u16,
            ..Default::default()
        }
    }

    /// Linux's `ipcperms`: whether the caller is granted every access `flag`'s permission bits ask.
    fn permits(&self, credentials: &Credentials, flag: u32) -> bool {
        let requested = (flag >> 6) | (flag >> 3) | flag;
        let mut granted = self.mode;
        if credentials.euid == self.cuid || credentials.euid == self.uid {
            granted >>= 6;
        } else if credentials.in_group(self.cgid, true) || credentials.in_group(self.gid, true) {
            granted >>= 3;
        }
        requested & !granted & 0o7 == 0 || credentials.has(CAP_IPC_OWNER)
    }

    /// Whether the caller may change or remove the object (`ipcctl_obtain_check`).
    fn may_control(&self, credentials: &Credentials) -> bool {
        credentials.euid == self.cuid
            || credentials.euid == self.uid
            || credentials.has(CAP_SYS_ADMIN)
    }
}

/// One kind of object's identifiers (Linux's `ipc_ids`): slots handed out in a cycle, with a sequence
/// number that grows each time a slot at or below the last one is handed out.
#[derive(Clone)]
struct Ids<T> {
    slots: BTreeMap<u32, T>,
    seq: u32,
    last_index: Option<u32>,
    next: u32,
}

impl<T> Default for Ids<T> {
    fn default() -> Self {
        Self {
            slots: BTreeMap::new(),
            seq: 0,
            last_index: None,
            next: 0,
        }
    }
}

impl<T> Ids<T> {
    /// The slot a new object gets and its sequence number (`ipc_idr_alloc`'s cyclic allocation),
    /// within `limit` objects; ENOSPC when full.
    fn allocate(&mut self, limit: u64) -> Result<(u32, u32), Errno> {
        let in_use = self.slots.len() as u32;
        if u64::from(in_use) >= limit {
            return Err(Errno::ENOSPC);
        }
        let end = (in_use * 3 / 2).clamp(IPC_MIN_CYCLE, IPCMNI);
        let free = |from: u32| (from..end).find(|index| !self.slots.contains_key(index));
        let index = free(self.next).or_else(|| free(0)).ok_or(Errno::ENOSPC)?;
        self.next = index + 1;
        if self.last_index.is_some_and(|last| index <= last) {
            self.seq += 1;
            if self.seq >= (i32::MAX as u32) >> IPCMNI_SHIFT {
                self.seq = 0;
            }
        }
        self.last_index = Some(index);
        Ok((index, self.seq))
    }

    /// `ipc_get_maxidx`: the highest slot in use, or 0 when none is.
    fn max_index(&self) -> u64 {
        self.slots
            .keys()
            .next_back()
            .map_or(0, |index| (*index).into())
    }
}

trait Object {
    fn perm(&self) -> &Perm;
}

impl<T: Object> Ids<T> {
    /// The object with identifier `id`, its sequence number checked (`ipc_obtain_object_check`).
    fn checked(&self, id: i32) -> Result<u32, Errno> {
        let index = id as u32 & (IPCMNI - 1);
        match self.slots.get(&index) {
            Some(object) if object.perm().seq == id as u32 >> IPCMNI_SHIFT => Ok(index),
            _ => Err(Errno::EINVAL),
        }
    }

    /// The object in slot `id`'s slot, whatever its sequence (`ipc_obtain_object_idr`).
    fn by_index(&self, id: i32) -> Result<u32, Errno> {
        let index = id as u32 & (IPCMNI - 1);
        if self.slots.contains_key(&index) {
            Ok(index)
        } else {
            Err(Errno::EINVAL)
        }
    }

    /// The slot of the object with `key` (`ipc_findkey`); private ones have none.
    fn find_key(&self, key: i32) -> Option<u32> {
        self.slots
            .iter()
            .find(|(_, object)| object.perm().key == key)
            .map(|(index, _)| *index)
    }
}

/// A shared memory segment (Linux's `shmid_kernel`).
#[derive(Clone)]
struct Segment {
    perm: Perm,
    /// The key it was made with, which names its file.
    created_key: i32,
    size: u64,
    file: Node,
    atime: i64,
    dtime: i64,
    ctime: i64,
    creator: Pid,
    last: Pid,
    /// How many mappings show it, in every process (`shm_nattch`), as last counted.
    attached: u64,
    /// The real user whose count of locked pages holds it, while locked (`mlock_ucounts`).
    locked_by: Option<u32>,
}

impl Object for Segment {
    fn perm(&self) -> &Perm {
        &self.perm
    }
}

impl Segment {
    fn pages(&self) -> u64 {
        self.size.div_ceil(PAGE_SIZE)
    }
}

/// The container's IPC namespace.
#[derive(Clone, Default)]
pub(crate) struct Ipc {
    shm: Ids<Segment>,
    /// Pages of every segment (`shm_tot`).
    shm_pages: u64,
    /// Pages of segments locked by `SHM_LOCK`, by the real user that locked them
    /// (`UCOUNT_RLIMIT_MEMLOCK`).
    locked_pages: BTreeMap<u32, u64>,
}

/// Linux's `ipcget`: a new object for `IPC_PRIVATE` or a key with none and `IPC_CREAT`, else the key's
/// object after `associate`'s checks and the permissions.
fn get<T: Object>(
    ids: &Ids<T>,
    credentials: &Credentials,
    key: i32,
    flags: u32,
    associate: impl Fn(&T) -> Result<(), Errno>,
) -> Result<Option<i32>, Errno> {
    if key == IPC_PRIVATE {
        return Ok(None);
    }
    match ids.find_key(key) {
        None if flags & IPC_CREAT == 0 => Err(Errno::ENOENT),
        None => Ok(None),
        Some(_) if flags & IPC_CREAT != 0 && flags & IPC_EXCL != 0 => Err(Errno::EEXIST),
        Some(index) => {
            let object = &ids.slots[&index];
            associate(object)?;
            if !object.perm().permits(credentials, flags) {
                return Err(Errno::EACCES);
            }
            Ok(Some(object.perm().id))
        }
    }
}

/// A new object's `Perm` in slot `index` with sequence `seq` (`ipc_addid`).
fn new_perm(credentials: &Credentials, key: i32, flags: u32, (index, seq): (u32, u32)) -> Perm {
    Perm {
        key,
        uid: credentials.euid,
        gid: credentials.egid,
        cuid: credentials.euid,
        cgid: credentials.egid,
        mode: flags & 0o777,
        seq,
        id: ((seq << IPCMNI_SHIFT) + index) as i32,
    }
}

pub(super) fn shmget(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [key, size, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (key, flags) = (key as i32, flags as u32);
    let credentials = kernel.process_of(tid).credentials.clone();
    let existing = get(&kernel.ipc.shm, &credentials, key, flags, |segment| {
        if segment.size < size {
            Err(Errno::EINVAL)
        } else {
            Ok(())
        }
    })?;
    if let Some(id) = existing {
        return Ok(id as u64);
    }
    // `newseg`.
    if !(SHMMIN..=SHMMAX).contains(&size) {
        return Err(Errno::EINVAL);
    }
    let pages = size.div_ceil(PAGE_SIZE);
    if kernel
        .ipc
        .shm_pages
        .checked_add(pages)
        .is_none_or(|total| total > SHMALL)
    {
        return Err(Errno::ENOSPC);
    }
    if flags & SHM_HUGETLB != 0 {
        return Err(kernel.counters.refuse_flag(
            cx.host,
            __NR_shmget,
            SHM_HUGETLB.into(),
            Errno::ENOMEM,
        ));
    }
    // `shmem_file_setup` takes no file larger than `MAX_LFS_FILESIZE`; memory is overcommitted, as
    // the made-up machine's `overcommit_memory` says.
    if size > i64::MAX as u64 {
        return Err(Errno::EINVAL);
    }
    let ino = kernel.files.numbers.ino();
    let slot = kernel.ipc.shm.allocate(SHMMNI)?;
    let perm = new_perm(&credentials, key, flags, slot);
    let now = kernel.now(cx);
    let mut regular = crate::fs::Regular::default();
    regular.truncate(cx.cpu, size);
    regular.segment = true;
    let file = crate::fs::new_inode(
        kernel.files.memfd_fs.clone(),
        ino,
        crate::fs::New {
            mode: S_IFREG | (flags & 0o777),
            uid: credentials.euid,
            gid: credentials.egid,
            rdev: 0,
            now,
            content: Content::Regular(regular),
            parent: None,
            name: segment_name(key),
        },
    )?;
    {
        let mut inode = file.borrow_mut();
        inode.nlink = 0;
        inode.ino = perm.id as u64;
    }
    let id = perm.id;
    let pid = kernel.threads[&tid].process;
    kernel.ipc.shm.slots.insert(
        slot.0,
        Segment {
            perm,
            created_key: key,
            size,
            file,
            atime: 0,
            dtime: 0,
            ctime: now.seconds,
            creator: pid,
            last: 0,
            attached: 0,
            locked_by: None,
        },
    );
    kernel.ipc.shm_pages += pages;
    Ok(id as u64)
}

/// The name of a segment's file (`SYSV%08x`).
fn segment_name(key: i32) -> Vec<u8> {
    alloc::format!("SYSV{:08x}", key as u32).into_bytes()
}

pub(super) fn shmat(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, address, flags, ..]: Args,
) -> Result<u64, Errno> {
    // `do_shmat`.
    let (id, flags) = (id as i32, flags as u32);
    // Tags are not taken off: the tagged address ABI leaves shmat and shmdt out.
    let mut address = address;
    if id < 0 {
        return Err(Errno::EINVAL);
    }
    let mut map_flags = MAP_SHARED;
    if address != 0 {
        if !address.is_multiple_of(PAGE_SIZE) {
            if flags & SHM_RND == 0 {
                return Err(Errno::EINVAL);
            }
            address &= !(PAGE_SIZE - 1);
            if address == 0 && flags & SHM_REMAP != 0 {
                return Err(Errno::EINVAL);
            }
        }
        map_flags |= MAP_FIXED;
    } else if flags & SHM_REMAP != 0 {
        return Err(Errno::EINVAL);
    }
    let (mut prot, mut access, open_flags) = if flags & SHM_RDONLY != 0 {
        (PROT_READ, 0o444, O_RDONLY)
    } else {
        (PROT_READ | PROT_WRITE, 0o666, O_RDWR)
    };
    if flags & SHM_EXEC != 0 {
        prot |= PROT_EXEC;
        access |= 0o111;
    }
    let index = kernel.ipc.shm.checked(id)?;
    let segment = &kernel.ipc.shm.slots[&index];
    if !segment
        .perm
        .permits(&kernel.process_of(tid).credentials, access)
    {
        return Err(Errno::EACCES);
    }
    let size = segment.size;
    let mut path = alloc::vec![b'/'];
    path.extend(segment_name(segment.created_key));
    let file = crate::file::OpenFile::new(segment.file.clone(), open_flags, path, 0);
    if address != 0 && flags & SHM_REMAP == 0 {
        let end = address.checked_add(size).ok_or(Errno::EINVAL)?;
        if kernel.process_of(tid).map.any_mapped(address, end) {
            return Err(Errno::EINVAL);
        }
    }
    super::memory::do_mmap(
        kernel,
        cx,
        tid,
        address,
        size,
        prot.into(),
        map_flags,
        Some(file),
        0,
    )
}

pub(super) fn shmdt(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [address, ..]: Args,
) -> Result<u64, Errno> {
    // `ksys_shmdt`: the first segment mapping at or above `address` that starts where its segment
    // would if attached there goes, then every piece of the same segment within its size.
    if !address.is_multiple_of(PAGE_SIZE) {
        return Err(Errno::EINVAL);
    }
    let process = kernel.process_of_mut(tid);
    let starts_there = |mapping: &crate::memory::Mapping| match &mapping.backing {
        crate::memory::Backing::File { node, offset, .. } if node.borrow().is_segment() => {
            mapping.start.wrapping_sub(address) / PAGE_SIZE == offset / PAGE_SIZE
        }
        _ => false,
    };
    let mut pieces = Vec::new();
    let mut found: Option<(Node, u64)> = None;
    for mapping in process.map.iter().filter(|mapping| mapping.end > address) {
        let crate::memory::Backing::File { node, .. } = &mapping.backing else {
            continue;
        };
        match &found {
            None if starts_there(mapping) => {
                let size = match &node.borrow().content {
                    Content::Regular(regular) => regular.size(),
                    _ => 0,
                };
                found = Some((node.clone(), crate::memory::page_align(size)));
                pieces.push((mapping.start, mapping.end));
            }
            None => {}
            Some((segment, size)) => {
                if mapping.end - address > *size {
                    break;
                }
                if starts_there(mapping) && alloc::rc::Rc::ptr_eq(node, segment) {
                    pieces.push((mapping.start, mapping.end));
                }
            }
        }
    }
    if found.is_none() {
        return Err(Errno::EINVAL);
    }
    for (start, end) in pieces {
        super::memory::unmap(&mut process.map, cx, process.memory, start, end - start)?;
    }
    crate::shared::release(kernel, cx);
    Ok(0)
}

pub(super) fn shmctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, command, buffer, ..]: Args,
) -> Result<u64, Errno> {
    let (id, command) = (id as i32, command as i32);
    if command < 0 || id < 0 {
        return Err(Errno::EINVAL);
    }
    let credentials = kernel.process_of(tid).credentials.clone();
    let shm = &kernel.ipc.shm;
    match command {
        IPC_INFO => {
            let info = shminfo64 {
                shmmax: SHMMAX,
                shmmin: SHMMIN,
                shmmni: SHMMNI,
                shmseg: SHMMNI,
                shmall: SHMALL,
                unused: [0; 4],
            };
            let max = shm.max_index();
            guest(kernel, cx, tid).write(buffer, info.as_bytes())?;
            Ok(max)
        }
        SHM_INFO => {
            // Resident pages are those the kernel has written; a mapping's faults are not known.
            kernel
                .counters
                .count(cx.host, reason::RESIDENCY_NOT_TRACKED, __NR_shmctl.into());
            let resident = shm
                .slots
                .values()
                .map(|segment| match &segment.file.borrow().content {
                    Content::Regular(regular) => regular.pages.len() as u64,
                    _ => 0,
                })
                .sum();
            let info = shm_info {
                used_ids: shm.slots.len() as i32,
                shm_tot: kernel.ipc.shm_pages,
                shm_rss: resident,
                ..Default::default()
            };
            let max = shm.max_index();
            guest(kernel, cx, tid).write(buffer, info.as_bytes())?;
            Ok(max)
        }
        IPC_STAT | SHM_STAT | SHM_STAT_ANY => {
            let index = if command == IPC_STAT {
                shm.checked(id)?
            } else {
                shm.by_index(id)?
            };
            let segment = &shm.slots[&index];
            if command != SHM_STAT_ANY && !segment.perm.permits(&credentials, 0o444) {
                return Err(Errno::EACCES);
            }
            let status = shmid64_ds {
                shm_perm: segment.perm.shown(),
                shm_segsz: segment.size,
                shm_atime: segment.atime,
                shm_dtime: segment.dtime,
                shm_ctime: segment.ctime,
                shm_cpid: segment.creator,
                shm_lpid: segment.last,
                shm_nattch: segment.attached,
                ..Default::default()
            };
            let answer = if command == IPC_STAT {
                0
            } else {
                segment.perm.id as u64
            };
            guest(kernel, cx, tid).write(buffer, status.as_bytes())?;
            Ok(answer)
        }
        IPC_SET | IPC_RMID => {
            let wanted = if command == IPC_SET {
                Some(guest(kernel, cx, tid).read_value::<shmid64_ds>(buffer)?)
            } else {
                None
            };
            let index = kernel.ipc.shm.checked(id)?;
            let now = kernel.now(cx).seconds;
            let segment = kernel.ipc.shm.slots.get_mut(&index).expect("checked");
            if !segment.perm.may_control(&credentials) {
                return Err(Errno::EPERM);
            }
            match wanted {
                Some(wanted) => {
                    // `ipc_update_perm`: ids that name no one are refused.
                    let perm = wanted.shm_perm;
                    if perm.uid == u32::MAX || perm.gid == u32::MAX {
                        return Err(Errno::EINVAL);
                    }
                    segment.perm.uid = perm.uid;
                    segment.perm.gid = perm.gid;
                    segment.perm.mode = (segment.perm.mode & !0o777) | (perm.mode & 0o777);
                    segment.ctime = now;
                }
                None => {
                    // `do_shm_rmid`: gone now if nothing is attached, else when the last goes.
                    segment.perm.mode |= SHM_DEST;
                    segment.perm.key = IPC_PRIVATE;
                    settle_segments(kernel);
                }
            }
            Ok(0)
        }
        SHM_LOCK | SHM_UNLOCK => {
            // `shmctl_do_lock`.
            let index = shm.checked(id)?;
            let process = kernel.process_of(tid);
            let limit = process.limits[RLIMIT_MEMLOCK as usize].current;
            let segment = kernel.ipc.shm.slots.get_mut(&index).expect("checked");
            let capable = credentials.has(CAP_IPC_LOCK);
            if !capable {
                if credentials.euid != segment.perm.uid && credentials.euid != segment.perm.cuid {
                    return Err(Errno::EPERM);
                }
                if command == SHM_LOCK && limit == 0 {
                    return Err(Errno::EPERM);
                }
            }
            let pages = segment.pages();
            if command == SHM_LOCK {
                if segment.locked_by.is_none() {
                    // `user_shm_lock`.
                    let user = credentials.uid;
                    let locked = kernel.ipc.locked_pages.get(&user).copied().unwrap_or(0) + pages;
                    let limit_pages = if limit == RLIM_INFINITY as u64 {
                        u64::MAX
                    } else {
                        limit / PAGE_SIZE
                    };
                    if locked > limit_pages && !capable {
                        return Err(Errno::ENOMEM);
                    }
                    kernel.ipc.locked_pages.insert(user, locked);
                    let segment = kernel.ipc.shm.slots.get_mut(&index).expect("checked");
                    segment.locked_by = Some(user);
                    segment.perm.mode |= SHM_LOCKED;
                }
            } else if let Some(user) = segment.locked_by.take() {
                segment.perm.mode &= !SHM_LOCKED;
                unlock_pages(&mut kernel.ipc, user, pages);
            }
            Ok(0)
        }
        _ => Err(Errno::EINVAL),
    }
}

fn unlock_pages(ipc: &mut Ipc, user: u32, pages: u64) {
    if let Some(locked) = ipc.locked_pages.get_mut(&user) {
        *locked -= pages;
        if *locked == 0 {
            ipc.locked_pages.remove(&user);
        }
    }
}

/// After a call or a process's end that may have changed which mappings show segments: each
/// segment's count of mappings, with Linux's `shm_open` (attach time and process) for a count that
/// grew and `shm_close` (detach time and process) for one that fell, `pid` being the process that
/// acted; a removed segment nothing shows any more is destroyed.
pub(crate) fn settle(kernel: &mut Kernel, cx: &mut Context<'_>, pid: Pid) {
    if kernel.ipc.shm.slots.is_empty() {
        return;
    }
    let counts = attachments(kernel);
    let changed = kernel
        .ipc
        .shm
        .slots
        .iter()
        .any(|(index, segment)| counts.get(index).copied().unwrap_or(0) != segment.attached);
    if !changed {
        return;
    }
    let now = kernel.now(cx).seconds;
    for (index, segment) in kernel.ipc.shm.slots.iter_mut() {
        let count = counts.get(index).copied().unwrap_or(0);
        if count != segment.attached {
            if count > segment.attached {
                segment.atime = now;
            } else {
                segment.dtime = now;
            }
            segment.last = pid;
            segment.attached = count;
        }
    }
    settle_segments(kernel);
}

/// Destroys removed segments nothing shows (`shm_destroy`).
fn settle_segments(kernel: &mut Kernel) {
    let ipc = &mut kernel.ipc;
    let gone: Vec<u32> = ipc
        .shm
        .slots
        .iter()
        .filter(|(_, segment)| segment.perm.mode & SHM_DEST != 0 && segment.attached == 0)
        .map(|(index, _)| *index)
        .collect();
    for index in gone {
        let segment = ipc.shm.slots.remove(&index).expect("listed");
        ipc.shm_pages -= segment.pages();
        if let Some(user) = segment.locked_by {
            unlock_pages(ipc, user, segment.pages());
        }
    }
}

/// How many mappings show each segment, by slot, over every memory once (a `vfork` child shows its
/// parent's).
fn attachments(kernel: &Kernel) -> BTreeMap<u32, u64> {
    let mut counts = BTreeMap::new();
    let mut memories = Vec::new();
    for process in kernel.processes.values() {
        if process.ended.is_some() || memories.contains(&process.memory) {
            continue;
        }
        memories.push(process.memory);
        for mapping in process.map.iter() {
            if let crate::memory::Backing::File { node, .. } = &mapping.backing
                && node.borrow().is_segment()
                && let Some((index, _)) = kernel
                    .ipc
                    .shm
                    .slots
                    .iter()
                    .find(|(_, segment)| alloc::rc::Rc::ptr_eq(&segment.file, node))
            {
                *counts.entry(*index).or_insert(0) += 1;
            }
        }
    }
    counts
}
