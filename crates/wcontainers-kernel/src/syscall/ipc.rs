//! System V IPC as Linux's `ipc/` builds it for a container's own IPC namespace: identifiers,
//! keys and permissions shared by every kind of object, shared memory segments, which are files on
//! the internal tmpfs mapped shared (`ipc/shm.c`), and message queues (`ipc/msg.c`). Semaphores
//! are in sem.rs.

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
use crate::process::{Channel, Pid, Tid};
use crate::{Context, Errno, Kernel};

/// An object's key, owners, mode and identifier (Linux's `kern_ipc_perm`).
#[derive(Clone)]
pub(super) struct Perm {
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
    pub(super) fn shown(&self) -> ipc64_perm {
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
    pub(super) fn permits(&self, credentials: &Credentials, flag: u32) -> bool {
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
    pub(super) fn may_control(&self, credentials: &Credentials) -> bool {
        credentials.euid == self.cuid
            || credentials.euid == self.uid
            || credentials.has(CAP_SYS_ADMIN)
    }
}

/// One kind of object's identifiers (Linux's `ipc_ids`): slots handed out in a cycle, with a sequence
/// number that grows each time a slot at or below the last one is handed out.
#[derive(Clone)]
pub(super) struct Ids<T> {
    pub(super) slots: BTreeMap<u32, T>,
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
    pub(super) fn allocate(&mut self, limit: u64) -> Result<(u32, u32), Errno> {
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
    pub(super) fn max_index(&self) -> u64 {
        self.slots
            .keys()
            .next_back()
            .map_or(0, |index| (*index).into())
    }
}

pub(super) trait Object {
    fn perm(&self) -> &Perm;
}

impl<T: Object> Ids<T> {
    /// The object with identifier `id`, its sequence number checked (`ipc_obtain_object_check`).
    pub(super) fn checked(&self, id: i32) -> Result<u32, Errno> {
        let index = id as u32 & (IPCMNI - 1);
        match self.slots.get(&index) {
            Some(object) if object.perm().seq == id as u32 >> IPCMNI_SHIFT => Ok(index),
            _ => Err(Errno::EINVAL),
        }
    }

    /// The object in slot `id`'s slot, whatever its sequence (`ipc_obtain_object_idr`).
    pub(super) fn by_index(&self, id: i32) -> Result<u32, Errno> {
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
    msg: Ids<Queue>,
    pub(super) sem: super::sem::Semaphores,
    /// The root of the namespace's mqueue file system (Linux's `mq_mnt`).
    pub(crate) mqueue_root: Option<Node>,
    pub(super) mq_usage: alloc::rc::Rc<super::mqueue::Usage>,
    /// The queue each waiting sender or receiver waits in.
    pub(super) mq_waiting: BTreeMap<Tid, alloc::rc::Weak<core::cell::RefCell<crate::fs::Inode>>>,
    /// What a waiting receiver (a message) or sender (nothing) was handed, taken when its call runs
    /// again.
    pub(super) mq_ready: BTreeMap<Tid, Option<(u32, Vec<u8>)>>,
    /// What a receiver waiting in `msgrcv` was handed while parked: a message, or why its wait ended
    /// (Linux's `r_msg`), taken when its call runs again.
    handed: BTreeMap<Tid, Result<Message, Errno>>,
    /// Pages of every segment (`shm_tot`).
    shm_pages: u64,
    /// Pages of segments locked by `SHM_LOCK`, by the real user that locked them
    /// (`UCOUNT_RLIMIT_MEMLOCK`).
    locked_pages: BTreeMap<u32, u64>,
}

/// Linux's `ipcget`: a new object for `IPC_PRIVATE` or a key with none and `IPC_CREAT`, else the key's
/// object after `associate`'s checks and the permissions.
pub(super) fn get<T: Object>(
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
pub(super) fn new_perm(
    credentials: &Credentials,
    key: i32,
    flags: u32,
    (index, seq): (u32, u32),
) -> Perm {
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
    let credentials = kernel.credentials(tid).clone();
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
        // `newseg`: the size its flags name (all of Linux's huge page flags share
        // `HUGETLB_FLAG_ENCODE_SHIFT`), then `hugetlb_file_setup`'s checks.
        crate::hugetlb::page_shift((flags >> MAP_HUGE_SHIFT) & MAP_HUGE_MASK)
            .map_err(|_| Errno::EINVAL)?;
        // `can_do_hugetlb_shm`: CAP_IPC_LOCK or the group `vm.hugetlb_shm_group` names (0).
        if !credentials.has(CAP_IPC_LOCK) && !credentials.in_group(0, true) {
            return Err(Errno::EPERM);
        }
        // No huge page is free to reserve; a segment that reserves none, whose every touch
        // would fault, is not built.
        if flags & SHM_NORESERVE == 0 {
            return Err(Errno::ENOMEM);
        }
        return Err(kernel.counters.refuse_flag(
            cx.host,
            __NR_shmget,
            SHM_NORESERVE.into(),
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
    if !segment.perm.permits(kernel.credentials(tid), access) {
        return Err(Errno::EACCES);
    }
    let size = segment.size;
    let mut path = alloc::vec![b'/'];
    path.extend(segment_name(segment.created_key));
    let node = segment.file.clone();
    let file = crate::file::OpenFile::new(kernel.next_object(), node, open_flags, path, 0);
    if address != 0 && flags & SHM_REMAP == 0 {
        let end = address.checked_add(size).ok_or(Errno::EINVAL)?;
        if kernel.process_of(tid).map.borrow().any_mapped(address, end) {
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
    for mapping in process
        .map
        .borrow()
        .iter()
        .filter(|mapping| mapping.end > address)
    {
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
        super::memory::unmap(
            &mut process.map.borrow_mut(),
            cx,
            process.memory,
            start,
            end - start,
        )?;
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
    let credentials = kernel.credentials(tid).clone();
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

/// Whether a segment's file is still its segment's, which a mapping of it reopens by (`__shm_open`).
pub(super) fn segment_exists(kernel: &Kernel, file: &Node) -> bool {
    kernel
        .ipc
        .shm
        .slots
        .values()
        .any(|segment| alloc::rc::Rc::ptr_eq(&segment.file, file))
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
        for mapping in process.map.borrow().iter() {
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

/// A message (Linux's `msg_msg`): its type and text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Message {
    kind: i64,
    text: Vec<u8>,
}

/// How a receiver chooses a message (`convert_mode`), with the type it names.
#[derive(Clone, Copy)]
enum Search {
    Any,
    /// The message at this position, for `MSG_COPY`.
    Number(i64),
    Equal(i64),
    NotEqual(i64),
    /// The lowest type at or below this one.
    LessEqual(i64),
}

impl Search {
    fn new(kind: i64, flags: u32) -> Self {
        if flags & MSG_COPY != 0 {
            Search::Number(kind)
        } else if kind == 0 {
            Search::Any
        } else if kind < 0 {
            Search::LessEqual(kind.checked_neg().unwrap_or(i64::MAX))
        } else if flags & MSG_EXCEPT != 0 {
            Search::NotEqual(kind)
        } else {
            Search::Equal(kind)
        }
    }

    /// `testmsg`.
    fn takes(self, kind: i64) -> bool {
        match self {
            Search::Any | Search::Number(_) => true,
            Search::Equal(wanted) => kind == wanted,
            Search::NotEqual(wanted) => kind != wanted,
            Search::LessEqual(most) => kind <= most,
        }
    }
}

/// A receiver parked in `msgrcv`, in the order receivers came (`msg_receiver`).
#[derive(Clone, Copy)]
struct Receiver {
    tid: Tid,
    search: Search,
    /// The largest text it takes; any with `MSG_NOERROR`.
    largest: u64,
}

/// A message queue (Linux's `msg_queue`).
#[derive(Clone)]
struct Queue {
    perm: Perm,
    messages: alloc::collections::VecDeque<Message>,
    bytes: u64,
    limit: u64,
    stime: i64,
    rtime: i64,
    ctime: i64,
    last_sender: Pid,
    last_receiver: Pid,
    receivers: Vec<Receiver>,
    /// What waiting senders and receivers wait on: numbers from `Kernel::next_object`.
    senders: u64,
    receivers_waiting: u64,
}

impl Object for Queue {
    fn perm(&self) -> &Perm {
        &self.perm
    }
}

impl Queue {
    fn senders(&self) -> Channel {
        Channel::Object(crate::process::ObjectId::Numbered(self.senders))
    }

    fn receivers_channel(&self) -> Channel {
        Channel::Object(crate::process::ObjectId::Numbered(self.receivers_waiting))
    }

    /// `msg_fits_inqueue`: the bytes and the count of messages are both held to the byte limit.
    fn fits(&self, size: u64) -> bool {
        size + self.bytes <= self.limit && (self.messages.len() as u64) < self.limit
    }

    /// `find_msg`: the position of the first message the search takes; for `LessEqual` the first
    /// of the lowest type.
    fn find(&self, search: Search) -> Option<usize> {
        let mut found = None;
        let mut most = match search {
            Search::LessEqual(most) => most,
            _ => 0,
        };
        let mut count = 0;
        for (index, message) in self.messages.iter().enumerate() {
            let search = match search {
                Search::LessEqual(_) => Search::LessEqual(most),
                other => other,
            };
            if !search.takes(message.kind) {
                continue;
            }
            match search {
                Search::LessEqual(_) if message.kind != 1 => {
                    most = message.kind - 1;
                    found = Some(index);
                }
                Search::Number(wanted) => {
                    if wanted == count {
                        return Some(index);
                    }
                }
                _ => return Some(index),
            }
            count += 1;
        }
        found
    }
}

pub(super) fn msgget(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [key, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (key, flags) = (key as i32, flags as u32);
    let credentials = kernel.credentials(tid).clone();
    if let Some(id) = get(&kernel.ipc.msg, &credentials, key, flags, |_| Ok(()))? {
        return Ok(id as u64);
    }
    // `newque`.
    let slot = kernel.ipc.msg.allocate(MSGMNI)?;
    let perm = new_perm(&credentials, key, flags, slot);
    let id = perm.id;
    let now = kernel.now(cx).seconds;
    let (senders, receivers_waiting) = (kernel.next_object(), kernel.next_object());
    kernel.ipc.msg.slots.insert(
        slot.0,
        Queue {
            perm,
            messages: Default::default(),
            bytes: 0,
            limit: MSGMNB,
            stime: 0,
            rtime: 0,
            ctime: now,
            last_sender: 0,
            last_receiver: 0,
            receivers: Vec::new(),
            senders,
            receivers_waiting,
        },
    );
    Ok(id as u64)
}

pub(super) fn msgsnd(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, buffer, size, flags, ..]: Args,
) -> Result<u64, Errno> {
    // `ksys_msgsnd` and `do_msgsnd`, in their order. A sender that waited runs again from here; its
    // queue gone since is EIDRM.
    let waited = kernel.threads[&tid].call.started;
    let kind: i64 = guest(kernel, cx, tid).read_value(buffer)?;
    let (id, flags) = (id as i32, flags as u32);
    if size > MSGMAX || (size as i64) < 0 || id < 0 || kind < 1 {
        return Err(Errno::EINVAL);
    }
    let text = guest(kernel, cx, tid).read(buffer + 8, size)?;
    let index = kernel
        .ipc
        .msg
        .checked(id)
        .map_err(|error| if waited { Errno::EIDRM } else { error })?;
    let credentials = kernel.credentials(tid);
    let queue = &kernel.ipc.msg.slots[&index];
    if !queue.perm.permits(credentials, 0o222) {
        return Err(Errno::EACCES);
    }
    if !queue.fits(size) {
        if flags & IPC_NOWAIT != 0 {
            return Err(Errno::EAGAIN);
        }
        if waited && kernel.signal_pending(tid) {
            return Err(Errno::ERESTARTNOHAND);
        }
        let channel = queue.senders();
        kernel.thread_mut(tid).call.started = true;
        kernel.park(tid, channel);
        return Err(Errno::PARKED);
    }
    let pid = kernel.threads[&tid].process;
    let now = kernel.now(cx).seconds;
    let message = Message { kind, text };
    let queue = kernel.ipc.msg.slots.get_mut(&index).expect("checked");
    queue.last_sender = pid;
    queue.stime = now;
    // `pipelined_send`: the first receiver in line the message suits takes it at once; one whose
    // buffer is too small is woken with E2BIG and the next is tried.
    let receivers = core::mem::take(&mut queue.receivers);
    let mut kept = Vec::new();
    let mut message = Some(message);
    let channel = queue.receivers_channel();
    for receiver in receivers {
        let waiting = kernel
            .threads
            .get(&receiver.tid)
            .is_some_and(|thread| thread.call.wait == Some(channel));
        let Some(sent) = message.as_ref().filter(|_| waiting) else {
            if waiting {
                kept.push(receiver);
            }
            continue;
        };
        if !receiver.search.takes(sent.kind) {
            kept.push(receiver);
            continue;
        }
        if receiver.largest < sent.text.len() as u64 {
            kernel.ipc.handed.insert(receiver.tid, Err(Errno::E2BIG));
        } else {
            let queue = kernel.ipc.msg.slots.get_mut(&index).expect("checked");
            queue.last_receiver = receiver.tid;
            queue.rtime = now;
            let sent = message.take().expect("a message");
            kernel.ipc.handed.insert(receiver.tid, Ok(sent));
        }
        kernel.wake_thread(receiver.tid);
    }
    let queue = kernel.ipc.msg.slots.get_mut(&index).expect("checked");
    queue.receivers = kept;
    if let Some(message) = message {
        queue.bytes += message.text.len() as u64;
        queue.messages.push_back(message);
    }
    Ok(0)
}

pub(super) fn msgrcv(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, buffer, size, kind, flags, ..]: Args,
) -> Result<u64, Errno> {
    let (id, flags, kind) = (id as i32, flags as u32, kind as i64);
    let waited = kernel.threads[&tid].call.started;
    // What a send or a removal handed it while it waited comes first.
    let handed = kernel.ipc.handed.remove(&tid);
    if waited && let Some(handed) = handed {
        return fill(kernel, cx, tid, buffer, size, handed?);
    }
    if id < 0 || (size as i64) < 0 {
        return Err(Errno::EINVAL);
    }
    if flags & MSG_COPY != 0 {
        if flags & MSG_EXCEPT != 0 || flags & IPC_NOWAIT == 0 {
            return Err(Errno::EINVAL);
        }
        // `prepare_copy` loads the caller's buffer as a message first.
        guest(kernel, cx, tid).read(buffer, size.min(MSGMAX))?;
    }
    let search = Search::new(kind, flags);
    let index = kernel
        .ipc
        .msg
        .checked(id)
        .map_err(|error| if waited { Errno::EIDRM } else { error })?;
    let credentials = kernel.credentials(tid).clone();
    let now = kernel.now(cx).seconds;
    let queue = kernel.ipc.msg.slots.get_mut(&index).expect("checked");
    queue.receivers.retain(|receiver| receiver.tid != tid);
    if !queue.perm.permits(&credentials, 0o444) {
        return Err(Errno::EACCES);
    }
    if let Some(position) = queue.find(search) {
        let found = &queue.messages[position];
        if size < found.text.len() as u64 && flags & MSG_NOERROR == 0 {
            return Err(Errno::E2BIG);
        }
        if flags & MSG_COPY != 0 {
            // `copy_msg`: the copy is made at the size asked for, at most the largest message.
            if size.min(MSGMAX) < found.text.len() as u64 {
                return Err(Errno::EINVAL);
            }
            let message = found.clone();
            return fill(kernel, cx, tid, buffer, size, message);
        }
        let message = queue.messages.remove(position).expect("found");
        queue.bytes -= message.text.len() as u64;
        queue.rtime = now;
        let queue = kernel.ipc.msg.slots.get_mut(&index).expect("checked");
        queue.last_receiver = kernel.threads[&tid].process;
        let senders = queue.senders();
        kernel.wake(senders);
        return fill(kernel, cx, tid, buffer, size, message);
    }
    if flags & IPC_NOWAIT != 0 {
        return Err(Errno::ENOMSG);
    }
    if kernel.signal_pending(tid) {
        return Err(Errno::ERESTARTNOHAND);
    }
    let largest = if flags & MSG_NOERROR != 0 {
        i32::MAX as u64
    } else {
        size
    };
    let queue = kernel.ipc.msg.slots.get_mut(&index).expect("checked");
    queue.receivers.push(Receiver {
        tid,
        search,
        largest,
    });
    let channel = queue.receivers_channel();
    kernel.thread_mut(tid).call.started = true;
    kernel.park(tid, channel);
    Err(Errno::PARKED)
}

/// `do_msg_fill`: the type, then as much of the text as the buffer takes. A message that cannot be
/// stored is lost, as on Linux.
fn fill(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    buffer: u64,
    size: u64,
    message: Message,
) -> Result<u64, Errno> {
    let mut guest = guest(kernel, cx, tid);
    guest.put(buffer, &message.kind.to_le_bytes())?;
    let length = size.min(message.text.len() as u64);
    guest
        .write(buffer + 8, &message.text[..length as usize])
        .map_err(|_| Errno::EFAULT)?;
    Ok(length)
}

pub(super) fn msgctl(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [id, command, buffer, ..]: Args,
) -> Result<u64, Errno> {
    let (id, command) = (id as i32, command as i32);
    if id < 0 || command < 0 {
        return Err(Errno::EINVAL);
    }
    let credentials = kernel.credentials(tid).clone();
    let msg = &kernel.ipc.msg;
    match command {
        IPC_INFO | MSG_INFO => {
            let mut info = msginfo {
                msgpool: MSGPOOL,
                msgmap: MSGMNB as i32,
                msgmax: MSGMAX as i32,
                msgmnb: MSGMNB as i32,
                msgmni: MSGMNI as i32,
                msgssz: MSGSSZ,
                msgtql: MSGMNB as i32,
                msgseg: MSGSEG,
                pad: 0,
            };
            if command == MSG_INFO {
                info.msgpool = msg.slots.len() as i32;
                info.msgmap = msg
                    .slots
                    .values()
                    .map(|queue| queue.messages.len() as i32)
                    .sum();
                info.msgtql = msg.slots.values().map(|queue| queue.bytes as i32).sum();
            }
            let max = msg.max_index();
            guest(kernel, cx, tid).write(buffer, info.as_bytes())?;
            Ok(max)
        }
        IPC_STAT | MSG_STAT | MSG_STAT_ANY => {
            let index = if command == IPC_STAT {
                msg.checked(id)?
            } else {
                msg.by_index(id)?
            };
            let queue = &msg.slots[&index];
            if command != MSG_STAT_ANY && !queue.perm.permits(&credentials, 0o444) {
                return Err(Errno::EACCES);
            }
            let status = msqid64_ds {
                msg_perm: queue.perm.shown(),
                msg_stime: queue.stime,
                msg_rtime: queue.rtime,
                msg_ctime: queue.ctime,
                msg_cbytes: queue.bytes,
                msg_qnum: queue.messages.len() as u64,
                msg_qbytes: queue.limit,
                msg_lspid: queue.last_sender,
                msg_lrpid: queue.last_receiver,
                ..Default::default()
            };
            let answer = if command == IPC_STAT {
                0
            } else {
                queue.perm.id as u64
            };
            guest(kernel, cx, tid).write(buffer, status.as_bytes())?;
            Ok(answer)
        }
        IPC_SET | IPC_RMID => {
            let wanted = if command == IPC_SET {
                Some(guest(kernel, cx, tid).read_value::<msqid64_ds>(buffer)?)
            } else {
                None
            };
            let index = kernel.ipc.msg.checked(id)?;
            let now = kernel.now(cx).seconds;
            let queue = kernel.ipc.msg.slots.get_mut(&index).expect("checked");
            if !queue.perm.may_control(&credentials) {
                return Err(Errno::EPERM);
            }
            let (senders, receivers) = (queue.senders(), queue.receivers_channel());
            match wanted {
                Some(wanted) => {
                    if wanted.msg_qbytes > MSGMNB && !credentials.has(CAP_SYS_RESOURCE) {
                        return Err(Errno::EPERM);
                    }
                    let perm = wanted.msg_perm;
                    if perm.uid == u32::MAX || perm.gid == u32::MAX {
                        return Err(Errno::EINVAL);
                    }
                    queue.perm.uid = perm.uid;
                    queue.perm.gid = perm.gid;
                    queue.perm.mode = (queue.perm.mode & !0o777) | (perm.mode & 0o777);
                    queue.limit = wanted.msg_qbytes;
                    queue.ctime = now;
                    // Receivers look again under the new permissions, senders under the new size.
                    queue.receivers.clear();
                }
                None => {
                    // `freeque`: receivers are told the queue is gone; senders find it gone.
                    let queue = kernel.ipc.msg.slots.remove(&index).expect("checked");
                    for receiver in queue.receivers {
                        kernel.ipc.handed.insert(receiver.tid, Err(Errno::EIDRM));
                    }
                }
            }
            kernel.wake(senders);
            kernel.wake(receivers);
            Ok(0)
        }
        _ => Err(Errno::EINVAL),
    }
}

/// `/proc/sysvipc/shm` (`sysvipc_shm_proc_show`): every segment of the namespace. Resident pages
/// are not tracked, so `rss` shows none (counted).
pub(crate) fn proc_shm(ipc: &Ipc, counters: &crate::counters::Counters) -> alloc::string::String {
    use core::fmt::Write;
    let mut out = alloc::string::String::from(
        "       key      shmid perms                  size  cpid  lpid nattch   uid   gid  cuid  \
         cgid      atime      dtime      ctime                   rss                  swap\n",
    );
    for segment in ipc.shm.slots.values() {
        let perm = &segment.perm;
        let _ = writeln!(
            out,
            "{:10} {:10}  {:4o} {:21} {:5} {:5}  {:5} {:5} {:5} {:5} {:5} {:10} {:10} {:10} {:21} {:21}",
            perm.key,
            perm.id,
            perm.mode,
            segment.size,
            segment.creator,
            segment.last,
            segment.attached,
            perm.uid,
            perm.gid,
            perm.cuid,
            perm.cgid,
            segment.atime,
            segment.dtime,
            segment.ctime,
            0,
            0
        );
    }
    if !ipc.shm.slots.is_empty() {
        counters.note(reason::RESIDENCY_NOT_TRACKED, 0);
    }
    out
}

/// `/proc/sysvipc/msg` (`sysvipc_msg_proc_show`): every queue of the namespace.
pub(crate) fn proc_msg(ipc: &Ipc) -> alloc::string::String {
    use core::fmt::Write;
    let mut out = alloc::string::String::from(
        "       key      msqid perms      cbytes       qnum lspid lrpid   uid   gid  cuid  cgid      \
         stime      rtime      ctime\n",
    );
    for queue in ipc.msg.slots.values() {
        let perm = &queue.perm;
        let _ = writeln!(
            out,
            "{:10} {:10}  {:4o}  {:10} {:10} {:5} {:5} {:5} {:5} {:5} {:5} {:10} {:10} {:10}",
            perm.key,
            perm.id,
            perm.mode,
            queue.bytes,
            queue.messages.len(),
            queue.last_sender,
            queue.last_receiver,
            perm.uid,
            perm.gid,
            perm.cuid,
            perm.cgid,
            queue.stime,
            queue.rtime,
            queue.ctime
        );
    }
    out
}

/// `/proc/sysvipc/sem`.
pub(crate) fn proc_sem(ipc: &Ipc) -> alloc::string::String {
    super::sem::listing(&ipc.sem)
}
