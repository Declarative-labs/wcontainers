//! The process information tree mounted at `/proc`. Its inodes are made when they are looked up, and a
//! file's bytes are made when it is opened.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use wcontainers_abi::Permissions;

use super::{Content, FileSystem, New, Node, Time, Walk, new_inode};
use crate::Errno;
use crate::linux::general::*;
use crate::memory::Backing;
use crate::process::{Channel, Pid, Process, Thread, Tid};

/// What a walk through `/proc` needs to know about the kernel.
pub(crate) struct View<'a> {
    pub processes: &'a BTreeMap<Pid, Process>,
    pub threads: &'a BTreeMap<Tid, Thread>,
    /// The process walking, which `/proc/self` names.
    pub caller: Pid,
    pub fs: FileSystem,
    pub boot: Time,
    /// Attributes changed on entries that keep them, by inode number.
    pub kept: &'a BTreeMap<u64, Kept>,
    pub counters: &'a crate::counters::Counters,
}

/// The attributes of an entry outside the process directories after a change. Linux keeps them: the
/// root's and `self`'s inodes live as long as the mount, and the others' `proc_dir_entry` holds them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Kept {
    mode: u32,
    uid: u32,
    gid: u32,
    atime: Time,
    mtime: Time,
    ctime: Time,
}

impl Kept {
    pub fn of(inode: &super::Inode) -> Self {
        Self {
            mode: inode.mode,
            uid: inode.uid,
            gid: inode.gid,
            atime: inode.atime,
            mtime: inode.mtime,
            ctime: inode.ctime,
        }
    }
}

/// One file or directory of the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    Root,
    SelfLink,
    Process(Pid),
    ProcessFile(Pid, ProcessFile),
    ProcessLink(Pid, ProcessLink),
    Fds(Pid),
    Fd(Pid, i32),
    /// A directory or file outside the process directories, by its index in [`GLOBAL`].
    Global(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessFile {
    Cmdline,
    Comm,
    Environ,
    Auxv,
    Maps,
    Stat,
    Statm,
    Status,
    Mounts,
    Limits,
    Cgroup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessLink {
    Exe,
    Cwd,
    Root,
}

const PROCESS_FILES: &[(&[u8], ProcessFile)] = &[
    (b"auxv", ProcessFile::Auxv),
    (b"cgroup", ProcessFile::Cgroup),
    (b"cmdline", ProcessFile::Cmdline),
    (b"comm", ProcessFile::Comm),
    (b"environ", ProcessFile::Environ),
    (b"limits", ProcessFile::Limits),
    (b"maps", ProcessFile::Maps),
    (b"mounts", ProcessFile::Mounts),
    (b"stat", ProcessFile::Stat),
    (b"statm", ProcessFile::Statm),
    (b"status", ProcessFile::Status),
];

const PROCESS_LINKS: &[(&[u8], ProcessLink)] = &[
    (b"cwd", ProcessLink::Cwd),
    (b"exe", ProcessLink::Exe),
    (b"root", ProcessLink::Root),
];

/// The files and directories outside the process directories, by path below `/proc`. A directory's
/// entries are the paths one part longer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Global {
    Directory,
    /// A file with these permissions (for sysctls, those of Linux's tables).
    File(u32),
    /// A symbolic link to a path below `/proc`.
    Link(&'static str),
}

pub(crate) const GLOBAL: &[(&str, Global)] = &[
    ("cpuinfo", Global::File(0o444)),
    ("filesystems", Global::File(0o444)),
    ("loadavg", Global::File(0o444)),
    ("meminfo", Global::File(0o444)),
    ("mounts", Global::Link("self/mounts")),
    ("stat", Global::File(0o444)),
    ("sys", Global::Directory),
    ("sys/abi", Global::Directory),
    ("sys/debug", Global::Directory),
    ("sys/dev", Global::Directory),
    ("sys/dev/tty", Global::Directory),
    ("sys/dev/tty/ldisc_autoload", Global::File(0o644)),
    ("sys/dev/tty/legacy_tiocsti", Global::File(0o644)),
    ("sys/fs", Global::Directory),
    ("sys/fs/nr_open", Global::File(0o644)),
    ("sys/kernel", Global::Directory),
    ("sys/kernel/cap_last_cap", Global::File(0o444)),
    ("sys/kernel/hostname", Global::File(0o644)),
    ("sys/kernel/ngroups_max", Global::File(0o444)),
    ("sys/kernel/osrelease", Global::File(0o444)),
    ("sys/kernel/ostype", Global::File(0o444)),
    ("sys/kernel/pid_max", Global::File(0o644)),
    ("sys/kernel/random", Global::Directory),
    ("sys/kernel/random/boot_id", Global::File(0o444)),
    ("sys/kernel/random/uuid", Global::File(0o444)),
    ("sys/net", Global::Directory),
    ("sys/sunrpc", Global::Directory),
    ("sys/user", Global::Directory),
    ("sys/vm", Global::Directory),
    ("sys/vm/overcommit_memory", Global::File(0o644)),
    ("uptime", Global::File(0o444)),
    ("version", Global::File(0o444)),
];

/// The entries Linux 7.0 has in the directories built here, in the order the reference's `getdents`
/// lists them, each directory's names separated by spaces; `<pid>` stands for a process directory.
/// An entry's position in its directory is 2 plus its index here, except in `/proc` itself, where
/// `self` and `thread-self` come at `FIRST_PROCESS_ENTRY`. Looking up one that is not built is
/// counted and warned before it answers ENOENT.
const LINUX_ENTRIES: &[(&str, &str)] = &[
    (
        "",
        concat!(
            "fs bus irq net sys tty keys kmsg misc scsi stat iomem kcore locks swaps .reset ",
            "crypto docker driver mounts uptime vmstat cgroups cmdline cpuinfo devices ",
            "ioports loadavg meminfo modules sysvipc version consoles kallsyms pressure ",
            "slabinfo softirqs zoneinfo buddyinfo config.gz diskstats key-users interrupts ",
            "kpagecount kpageflags partitions timer_list device-tree execdomains filesystems ",
            "kpagecgroup vmallocinfo pagetypeinfo sysrq-trigger self thread-self",
        ),
    ),
    (
        "<pid>",
        concat!(
            "task fd map_files fdinfo ns net environ auxv status personality limits sched ",
            "autogroup timens_offsets comm syscall cmdline stat statm maps mem cwd root exe ",
            "mounts mountinfo mountstats clear_refs smaps smaps_rollup pagemap attr wchan ",
            "stack schedstat cpuset cgroup oom_score oom_adj oom_score_adj loginuid ",
            "sessionid coredump_filter io uid_map gid_map projid_map setgroups timers ",
            "timerslack_ns ksm_merging_pages ksm_stat",
        ),
    ),
    ("sys", "abi debug dev fs kernel net sunrpc user vm"),
    ("sys/abi", "tagged_addr_disabled"),
    ("sys/debug", "exception-trace"),
    ("sys/dev", "scsi tty"),
    ("sys/dev/tty", "ldisc_autoload legacy_tiocsti"),
    (
        "sys/fs",
        concat!(
            "aio-max-nr aio-nr binfmt_misc dentry-negative dentry-state dir-notify-enable ",
            "epoll fanotify file-max file-nr fuse inode-nr inode-state inotify ",
            "lease-break-time leases-enable mount-max mqueue nfs nr_open overflowgid ",
            "overflowuid pipe-max-size pipe-user-pages-hard pipe-user-pages-soft ",
            "protected_fifos protected_hardlinks protected_regular protected_symlinks quota ",
            "suid_dumpable xfs",
        ),
    ),
    (
        "sys/kernel",
        concat!(
            "acct arch auto_msgmni bpf_stats_enabled cad_pid cap_last_cap ",
            "core_file_note_size_limit core_modes core_pattern core_pipe_limit core_sort_vma ",
            "core_uses_pid ctrl-alt-del dmesg_restrict domainname ftrace_dump_on_oops ",
            "ftrace_enabled hostname io_uring_disabled io_uring_group kernel_sys_info keys ",
            "kptr_restrict max_lock_depth max_rcu_stall_to_panic modprobe modules_disabled ",
            "msg_next_id msgmax msgmnb msgmni ngroups_max ns_last_pid oops_all_cpu_backtrace ",
            "oops_limit osrelease ostype overflowgid overflowuid panic panic_on_oops ",
            "panic_on_rcu_stall panic_on_warn panic_print panic_sys_info ",
            "perf_cpu_time_max_percent perf_event_max_contexts_per_stack ",
            "perf_event_max_sample_rate perf_event_max_stack perf_event_mlock_kb ",
            "perf_event_paranoid pid_max poweroff_cmd print-fatal-signals printk printk_delay ",
            "printk_devkmsg printk_ratelimit printk_ratelimit_burst pty random ",
            "randomize_va_space sched_autogroup_enabled sched_cfs_bandwidth_slice_us ",
            "sched_deadline_period_max_us sched_deadline_period_min_us sched_rr_timeslice_ms ",
            "sched_rt_period_us sched_rt_runtime_us seccomp sem sem_next_id sg-big-buff ",
            "shm_next_id shm_rmid_forced shmall shmmax shmmni sysctl_writes_strict sysrq ",
            "tainted task_delayacct threads-max timer_migration traceoff_on_warning ",
            "tracepoint_printk unprivileged_bpf_disabled unprivileged_userns_clone ",
            "usermodehelper version warn_limit",
        ),
    ),
    (
        "sys/kernel/random",
        concat!(
            "boot_id entropy_avail poolsize urandom_min_reseed_secs uuid ",
            "write_wakeup_threshold",
        ),
    ),
    (
        "sys/net",
        "bridge core ipv4 ipv6 mpls netfilter sctp unix vsock",
    ),
    (
        "sys/sunrpc",
        concat!(
            "max_resvport min_resvport tcp_fin_timeout tcp_max_slot_table_entries ",
            "tcp_slot_table_entries udp_slot_table_entries",
        ),
    ),
    (
        "sys/user",
        concat!(
            "max_cgroup_namespaces max_fanotify_groups max_fanotify_marks ",
            "max_inotify_instances max_inotify_watches max_ipc_namespaces max_mnt_namespaces ",
            "max_net_namespaces max_pid_namespaces max_time_namespaces max_user_namespaces ",
            "max_uts_namespaces",
        ),
    ),
    (
        "sys/vm",
        concat!(
            "admin_reserve_kbytes compact_memory compact_unevictable_allowed ",
            "compaction_proactiveness defrag_mode dirty_background_bytes ",
            "dirty_background_ratio dirty_bytes dirty_expire_centisecs dirty_ratio ",
            "dirty_writeback_centisecs dirtytime_expire_seconds drop_caches extfrag_threshold ",
            "hugetlb_shm_group laptop_mode legacy_va_layout lowmem_reserve_ratio ",
            "max_map_count memfd_noexec min_free_kbytes mmap_min_addr mmap_rnd_bits ",
            "movable_gigantic_pages nr_hugepages nr_overcommit_hugepages oom_dump_tasks ",
            "oom_kill_allocating_task overcommit_kbytes overcommit_memory overcommit_ratio ",
            "page-cluster page_lock_unfairness panic_on_oom percpu_pagelist_high_fraction ",
            "stat_interval stat_refresh swappiness user_reserve_kbytes vfs_cache_pressure ",
            "vfs_cache_pressure_denom watermark_boost_factor watermark_scale_factor",
        ),
    ),
];

/// How a file of the tree makes its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Style {
    /// `cmdline`, `environ` and `auxv`: made at each read, from the process's memory or its saved
    /// auxiliary vector; they seek as `generic_file_llseek` with a size of zero.
    Memory,
    /// A sysctl: made at each read (`proc_sys_call_handler`); seeks as `default_llseek`.
    Sysctl,
    /// Linux's `seq_file`: made by a read from the start or from anywhere but where the last read
    /// stopped, and by a seek elsewhere, and kept between; no positioned writes (`seq_open` clears
    /// `FMODE_PWRITE`); seeks as `seq_lseek`.
    Sequence,
}

/// What a write to a file of the tree does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Writing {
    /// `comm_write`: names the process.
    Comm,
    /// A sysctl's own write; `/proc/sys` is mounted read-only, so no file is open for it.
    Sysctl,
    /// `EIO`, as `pde_write` for an entry with no write of its own.
    Io,
    /// The file has no write (`FMODE_CAN_WRITE` unset): `EINVAL`.
    None,
}

/// The link count of `/proc` itself, as Linux's `proc_root_getattr` gives it: two, one for each
/// directory below it, and one for each process on the machine.
pub(crate) fn root_links(processes: usize) -> u32 {
    let directories = global_children("")
        .filter(|(index, _)| GLOBAL[*index].1 == Global::Directory)
        .count();
    (2 + directories + processes) as u32
}

/// Whether a global entry is the `/proc/sys` directory itself.
pub(crate) fn is_sysctl_root(index: usize) -> bool {
    GLOBAL[index].0 == "sys"
}

impl Entry {
    /// A directory's path below the tree's root, for finding it again when the inodes above it, which
    /// are made at each lookup, are gone.
    pub fn directory_path(&self) -> Option<String> {
        match *self {
            Entry::Root => Some(String::new()),
            Entry::Process(pid) => Some(format!("/{pid}")),
            Entry::Fds(pid) => Some(format!("/{pid}/fd")),
            Entry::Global(index) => Some(format!("/{}", GLOBAL[index].0)),
            _ => None,
        }
    }

    /// The process an entry belongs to, for the entries inside a process's directory and the
    /// directory itself.
    pub fn of_process(&self) -> Option<Pid> {
        match *self {
            Entry::Process(pid)
            | Entry::ProcessFile(pid, _)
            | Entry::ProcessLink(pid, _)
            | Entry::Fds(pid)
            | Entry::Fd(pid, _) => Some(pid),
            Entry::Root | Entry::SelfLink | Entry::Global(_) => None,
        }
    }

    /// Whether a change to the entry's owner, mode or times lasts (see [`Kept`]).
    pub fn keeps_attributes(&self) -> bool {
        matches!(self, Entry::Root | Entry::SelfLink | Entry::Global(_))
    }

    /// The size `stat` reports: none, except for descriptor links.
    pub fn size(&self) -> u64 {
        match self {
            // Linux's `proc_fd_instantiate` gives descriptor links a size of 64 (measured too).
            Entry::Fd(..) => 64,
            _ => 0,
        }
    }

    /// How a file of the tree makes its bytes, which also sets how it seeks and what it takes.
    pub fn style(&self) -> Style {
        match self {
            Entry::ProcessFile(
                _,
                ProcessFile::Cmdline | ProcessFile::Environ | ProcessFile::Auxv,
            ) => Style::Memory,
            Entry::Global(index) if GLOBAL[*index].0.starts_with("sys/") => Style::Sysctl,
            _ => Style::Sequence,
        }
    }

    /// What a write to the file does.
    pub fn writing(&self) -> Writing {
        match self {
            Entry::ProcessFile(_, ProcessFile::Comm) => Writing::Comm,
            Entry::Global(_) if self.style() == Style::Sysctl => Writing::Sysctl,
            // `proc_reg_write` with no `proc_write` of the entry's own.
            Entry::Global(_) => Writing::Io,
            _ => Writing::None,
        }
    }

    /// Whether reads and writes go a segment at a time (Linux's `do_loop_readv_writev`, which takes
    /// no flag but RWF_HIPRI): the files with a plain `read` or `write` and no iterating one.
    pub fn loops(&self, writing: bool) -> bool {
        match self.style() {
            Style::Sysctl => false,
            // `proc_reg_read_iter` reads the entries outside the process directories; their writes
            // are `proc_reg_write`.
            _ if matches!(self, Entry::Global(_)) => writing,
            _ => true,
        }
    }

    fn mode(&self) -> u32 {
        match self {
            Entry::Root | Entry::Process(_) => S_IFDIR | 0o555,
            Entry::Fds(_) => S_IFDIR | 0o500,
            Entry::Global(index) => match GLOBAL[*index].1 {
                Global::Directory => S_IFDIR | 0o555,
                Global::File(permissions) => S_IFREG | permissions,
                Global::Link(_) => S_IFLNK | 0o777,
            },
            Entry::ProcessFile(_, ProcessFile::Comm) => S_IFREG | 0o644,
            Entry::ProcessFile(_, ProcessFile::Environ | ProcessFile::Auxv) => S_IFREG | 0o400,
            Entry::ProcessFile(..) => S_IFREG | 0o444,
            Entry::SelfLink | Entry::ProcessLink(..) => S_IFLNK | 0o777,
            Entry::Fd(..) => S_IFLNK | 0o700,
        }
    }

    /// A stable inode number for the entry.
    fn ino(&self) -> u64 {
        let process = |pid: Pid, index: u64| ((pid as u64) << 16) | index;
        match self {
            Entry::Root => 1,
            Entry::SelfLink => 2,
            Entry::Global(index) => 16 + *index as u64,
            Entry::Process(pid) => process(*pid, 1),
            Entry::Fds(pid) => process(*pid, 2),
            Entry::ProcessFile(pid, file) => process(*pid, 16 + *file as u64),
            Entry::ProcessLink(pid, link) => process(*pid, 64 + *link as u64),
            Entry::Fd(pid, fd) => process(*pid, 0x1000 + *fd as u64 % 0xf000),
        }
    }

    fn owner(&self, view: &View<'_>) -> (u32, u32) {
        let pid = match self {
            Entry::Process(pid)
            | Entry::Fds(pid)
            | Entry::ProcessFile(pid, _)
            | Entry::ProcessLink(pid, _)
            | Entry::Fd(pid, _) => *pid,
            _ => return (0, 0),
        };
        dump_owner(view.processes, pid, self.mode())
    }
}

/// Linux's `task_dump_owner`: the owner of a process's entry with `mode`, the process's effective ids,
/// except that what is not a world readable directory belongs to root while the process may not be
/// dumped.
pub(crate) fn dump_owner(processes: &BTreeMap<Pid, Process>, pid: Pid, mode: u32) -> (u32, u32) {
    processes.get(&pid).map_or((0, 0), |process| {
        if mode != S_IFDIR | 0o555 && !process.dumpable {
            (0, 0)
        } else {
            (process.credentials.euid, process.credentials.egid)
        }
    })
}

/// Makes the inode for an entry found in `parent` under `name`.
/// A process's state as its first thread's (`get_task_state`): ended and not reaped, stopped, parked
/// (unkillably only for a `vfork` parent's wait), or running.
fn state(view: &View<'_>, process: &Process) -> (char, &'static str) {
    let wait = process
        .threads
        .first()
        .and_then(|tid| view.threads.get(tid))
        .and_then(|thread| thread.call.wait);
    if process.ended.is_some() {
        ('Z', "zombie")
    } else if process.stopped {
        ('T', "stopped")
    } else if matches!(wait, Some(Channel::Vfork(_))) {
        ('D', "disk sleep")
    } else if wait.is_some() {
        ('S', "sleeping")
    } else {
        ('R', "running")
    }
}

pub(crate) fn node(view: &View<'_>, parent: Option<&Node>, name: &[u8], entry: Entry) -> Node {
    let (uid, gid) = entry.owner(view);
    let mut mode = entry.mode();
    // A descriptor's link shows how the file was opened: reading gives read and execute, writing
    // write and execute, and an `O_PATH` descriptor nothing (Linux's `tid_fd_update_inode`).
    if let Entry::Fd(pid, fd) = entry
        && let Some(file) = view
            .processes
            .get(&pid)
            .and_then(|process| process.files.file(fd).ok())
    {
        let file = file.borrow();
        mode = S_IFLNK
            | if file.readable() { 0o500 } else { 0 }
            | if file.writable() { 0o300 } else { 0 };
    }
    let is_directory = mode & S_IFMT == S_IFDIR;
    // Docker mounts `/proc/sys` again over itself, read-only.
    let fs = match entry {
        Entry::Global(index) if GLOBAL[index].0 == "sys" || GLOBAL[index].0.starts_with("sys/") => {
            FileSystem {
                flags: view.fs.flags | crate::linux_extra::ST_RDONLY,
                ..view.fs.clone()
            }
        }
        _ => view.fs.clone(),
    };
    let node = new_inode(
        fs.clone(),
        entry.ino(),
        New {
            mode,
            uid,
            gid,
            rdev: 0,
            now: view.boot,
            content: Content::Proc(entry),
            parent: parent.filter(|_| is_directory).cloned(),
            name: name.to_vec(),
        },
    )
    .expect("proc counts no space");
    if let Some(kept) = view.kept.get(&entry.ino()) {
        let mut inode = node.borrow_mut();
        inode.mode = kept.mode;
        inode.uid = kept.uid;
        inode.gid = kept.gid;
        inode.atime = kept.atime;
        inode.mtime = kept.mtime;
        inode.ctime = kept.ctime;
    }
    if is_directory {
        node.borrow_mut().place.held_parent = parent.cloned();
    }
    // Link counts as the reference reports them: a process directory counts Linux's seven
    // subdirectories, and the sysctl directories count none.
    match entry {
        Entry::Process(_) => node.borrow_mut().nlink = 9,
        // The root of Docker's read-only mount of `/proc/sys`.
        Entry::Global(index) if is_sysctl_root(index) => {
            let mut inode = node.borrow_mut();
            inode.nlink = 1;
            inode.place.mount_root = true;
        }
        Entry::Global(_) if fs.read_only() && is_directory => node.borrow_mut().nlink = 1,
        _ => {}
    }
    node
}

fn parse_number(name: &[u8]) -> Option<i64> {
    if name.is_empty() || name.len() > 10 || (name.len() > 1 && name[0] == b'0') {
        return None;
    }
    let mut value: i64 = 0;
    for byte in name {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + i64::from(byte - b'0');
    }
    Some(value)
}

fn global_children(prefix: &str) -> impl Iterator<Item = (usize, &'static str)> + '_ {
    GLOBAL
        .iter()
        .enumerate()
        .filter_map(move |(index, (path, _))| {
            let rest = if prefix.is_empty() {
                *path
            } else {
                path.strip_prefix(prefix)?.strip_prefix('/')?
            };
            (!rest.contains('/')).then_some((index, rest))
        })
}

/// Looks a name up in a directory of the tree.
pub(crate) fn lookup(
    view: &View<'_>,
    directory: &Node,
    entry: &Entry,
    name: &[u8],
) -> Result<Node, Errno> {
    let found = list(view, entry)
        .into_iter()
        .find(|(child, _, _)| child.as_slice() == name)
        .map(|(_, child, _)| child);
    // Process directories that are not listed (threads) can still be looked up.
    let found = found.or_else(|| match entry {
        Entry::Root => parse_number(name)
            .and_then(|pid| i32::try_from(pid).ok())
            .filter(|pid| view.processes.contains_key(pid))
            .map(Entry::Process),
        _ => None,
    });
    let Some(child) = found else {
        not_built(view, entry, name);
        return Err(Errno::ENOENT);
    };
    Ok(node(view, Some(directory), name, child))
}

/// Counts a lookup that found nothing when Linux has the name there.
fn not_built(view: &View<'_>, entry: &Entry, name: &[u8]) {
    let directory = match entry {
        Entry::Root => "",
        Entry::Process(_) => "<pid>",
        Entry::Global(index) => GLOBAL[*index].0,
        _ => return,
    };
    let Some(index) = LINUX_ENTRIES
        .iter()
        .position(|(path, _)| *path == directory)
    else {
        return;
    };
    if let Some(position) = LINUX_ENTRIES[index]
        .1
        .split(' ')
        .position(|known| known.as_bytes() == name)
    {
        view.counters.note(
            crate::reason::PROC_ENTRY_NOT_BUILT,
            (index * 1024 + position) as u64,
        );
    }
}

/// Where Linux's `getdents` puts a name in a directory of the tree: 2 plus its index in Linux's list,
/// and `FIRST_PROCESS_ENTRY` (256) on for what `/proc` lists after its own entries.
fn linux_position(directory: &str, name: &str) -> u64 {
    const FIRST_PROCESS_ENTRY: u64 = 256;
    let names = LINUX_ENTRIES
        .iter()
        .find(|(path, _)| *path == directory)
        .map_or("", |(_, names)| *names);
    let index = names
        .split(' ')
        .position(|known| known == name)
        .expect("every entry built is one of Linux's") as u64;
    match name {
        "self" if directory.is_empty() => FIRST_PROCESS_ENTRY,
        "thread-self" if directory.is_empty() => FIRST_PROCESS_ENTRY + 1,
        _ => index + 2,
    }
}

/// Where `/proc` puts a process's directory (`TGID_OFFSET` past the first process entry).
pub(crate) const fn process_position(pid: Pid) -> u64 {
    pid as u64 + 256 + 2
}

/// Where `/proc`'s listing ends: past every process number Linux can give (`PID_MAX_LIMIT`).
pub(crate) const PROCESSES_END: u64 = (4 << 20) + 256 + 2;

/// The entries of a directory of the tree, each with its position, in the order `getdents` lists
/// them.
pub(crate) fn list(view: &View<'_>, entry: &Entry) -> Vec<(Vec<u8>, Entry, u64)> {
    let mut out = Vec::new();
    match entry {
        Entry::Root => {
            for (index, name) in global_children("") {
                out.push((
                    name.as_bytes().to_vec(),
                    Entry::Global(index),
                    linux_position("", name),
                ));
            }
            out.push((
                b"self".to_vec(),
                Entry::SelfLink,
                linux_position("", "self"),
            ));
            for pid in view.processes.keys() {
                out.push((
                    format!("{pid}").into_bytes(),
                    Entry::Process(*pid),
                    process_position(*pid),
                ));
            }
        }
        Entry::Process(pid) => {
            let mut add = |name: &[u8], child: Entry| {
                let position =
                    linux_position("<pid>", core::str::from_utf8(name).unwrap_or_default());
                out.push((name.to_vec(), child, position));
            };
            for (name, file) in PROCESS_FILES {
                add(name, Entry::ProcessFile(*pid, *file));
            }
            for (name, link) in PROCESS_LINKS {
                add(name, Entry::ProcessLink(*pid, *link));
            }
            add(b"fd", Entry::Fds(*pid));
        }
        Entry::Fds(pid) => {
            if let Some(process) = view.processes.get(pid) {
                for (fd, _) in process.files.iter() {
                    out.push((
                        format!("{fd}").into_bytes(),
                        Entry::Fd(*pid, fd),
                        fd as u64 + 2,
                    ));
                }
            }
        }
        Entry::Global(index) => {
            let (path, _) = GLOBAL[*index];
            for (child, name) in global_children(path) {
                out.push((
                    name.as_bytes().to_vec(),
                    Entry::Global(child),
                    linux_position(path, name),
                ));
            }
        }
        _ => {}
    }
    out.sort_by_key(|(_, _, position)| *position);
    out
}

/// Where a symbolic link of the tree points, as `readlink` gives it.
pub(crate) fn link_target(
    view: &View<'_>,
    walk: &Walk<'_>,
    entry: &Entry,
) -> Result<Vec<u8>, Errno> {
    match entry {
        Entry::SelfLink => Ok(format!("{}", view.caller).into_bytes()),
        Entry::Global(index) => match GLOBAL[*index].1 {
            Global::Link(target) => Ok(target.as_bytes().to_vec()),
            _ => Err(Errno::EINVAL),
        },
        Entry::ProcessLink(pid, link) => {
            let process = view.processes.get(pid).ok_or(Errno::ENOENT)?;
            match link {
                ProcessLink::Exe => Ok(process.executable.1.clone()),
                ProcessLink::Cwd => Ok(walk.path_of(&process.cwd).unwrap_or_default()),
                ProcessLink::Root => Ok(walk.path_of(&process.root).unwrap_or_default()),
            }
        }
        Entry::Fd(pid, fd) => {
            let process = view.processes.get(pid).ok_or(Errno::ENOENT)?;
            let file = process.files.file(*fd).map_err(|_| Errno::ENOENT)?;
            let file = file.borrow();
            let inode = file.node.borrow();
            Ok(match &inode.content {
                Content::Pipe(_) if inode.fs.kind == super::Kind::Pipe => {
                    format!("pipe:[{}]", inode.ino).into_bytes()
                }
                Content::Directory(_) | Content::Proc(_) if inode.is_directory() => {
                    drop(inode);
                    walk.path_of(&file.node)
                        .unwrap_or_else(|| file.path.clone())
                }
                _ if inode.shown_deleted() => {
                    let mut path = file.path.clone();
                    path.extend_from_slice(b" (deleted)");
                    path
                }
                _ => file.path.clone(),
            })
        }
        _ => Err(Errno::EINVAL),
    }
}

/// The path a magic link (a process link or a descriptor) leads to, as `d_path` gives it for the file
/// it names; a file opened through one keeps that path.
pub(crate) fn magic_path(view: &View<'_>, walk: &Walk<'_>, entry: &Entry) -> Option<Vec<u8>> {
    match entry {
        Entry::ProcessLink(..) => link_target(view, walk, entry).ok(),
        Entry::Fd(pid, fd) => {
            let file = view.processes.get(pid)?.files.file(*fd).ok()?;
            let file = file.borrow();
            if file.node.borrow().is_directory() {
                return walk.path_of(&file.node).or_else(|| Some(file.path.clone()));
            }
            Some(file.path.clone())
        }
        _ => None,
    }
}

/// Follows a link of the tree. The process links and descriptors lead straight to the inode, not
/// through its path, as Linux's "magic links" do.
pub(crate) fn follow(
    view: &View<'_>,
    walk: &Walk<'_>,
    at: &Node,
    entry: &Entry,
    links: &mut u32,
) -> Result<Node, Errno> {
    match entry {
        Entry::ProcessLink(pid, link) => {
            let process = view.processes.get(pid).ok_or(Errno::ENOENT)?;
            Ok(match link {
                ProcessLink::Exe => process.executable.0.clone(),
                ProcessLink::Cwd => process.cwd.clone(),
                ProcessLink::Root => process.root.clone(),
            })
        }
        Entry::Fd(pid, fd) => {
            let process = view.processes.get(pid).ok_or(Errno::ENOENT)?;
            let file = process.files.file(*fd).map_err(|_| Errno::ENOENT)?;
            let node = file.borrow().node.clone();
            Ok(node)
        }
        _ => {
            let target = link_target(view, walk, entry)?;
            walk.resolve_from(at, &target, links)
        }
    }
}

/// Makes the bytes of a file of the tree.
pub(crate) fn contents(source: &mut Source<'_>, entry: &Entry) -> Result<Vec<u8>, Errno> {
    let mut out = String::new();
    match entry {
        Entry::ProcessFile(pid, file) => {
            let process = source.view.processes.get(pid).ok_or(Errno::ESRCH)?;
            return process_file(source, process, *file);
        }
        Entry::Global(index) => {
            let (path, _) = GLOBAL[*index];
            let machine = source.machine;
            match path {
                "cpuinfo" => {
                    let _ = write!(
                        out,
                        "processor\t: 0\nBogoMIPS\t: 48.00\nFeatures\t: fp asimd crc32 atomics\n\
                         CPU implementer\t: 0x00\nCPU architecture: 8\nCPU variant\t: 0x0\n\
                         CPU part\t: 0x000\nCPU revision\t: 0\n\n"
                    );
                }
                "filesystems" => out.push_str("nodev\ttmpfs\nnodev\tproc\nnodev\tpipefs\n"),
                "loadavg" => {
                    let _ = writeln!(
                        out,
                        "0.00 0.00 0.00 1/{} {}",
                        source.view.processes.len(),
                        source.last_pid
                    );
                }
                "meminfo" => {
                    let total = machine.memory_bytes / 1024;
                    let (in_use, shared) = source.memory_in_use;
                    let free = total - in_use / 1024;
                    let shared = shared / 1024;
                    let _ = write!(
                        out,
                        "MemTotal:       {total:8} kB\nMemFree:        {free:8} kB\n\
                         MemAvailable:   {free:8} kB\nBuffers:               0 kB\n\
                         Cached:                0 kB\nSwapTotal:             0 kB\nSwapFree:              0 kB\n\
                         Shmem:          {shared:8} kB\n"
                    );
                }
                "stat" => {
                    let _ = write!(
                        out,
                        "cpu  0 0 0 0 0 0 0 0 0 0\ncpu0 0 0 0 0 0 0 0 0 0 0\nintr 0\nctxt 0\nbtime {}\n\
                         processes {}\nprocs_running 1\nprocs_blocked 0\nsoftirq 0\n",
                        source.view.boot.seconds, source.last_pid
                    );
                }
                "uptime" => {
                    let up = source.uptime_nanoseconds;
                    let _ = writeln!(
                        out,
                        "{}.{:02} 0.00",
                        up / 1_000_000_000,
                        up % 1_000_000_000 / 10_000_000
                    );
                }
                "version" => {
                    let _ = writeln!(
                        out,
                        "Linux version {} (wcontainers) {}",
                        machine.release, machine.version
                    );
                }
                // Line disciplines load as modules on demand; TIOCSTI is still built (measured).
                "sys/dev/tty/ldisc_autoload" | "sys/dev/tty/legacy_tiocsti" => out.push_str("1\n"),
                "sys/fs/nr_open" => out.push_str("1048576\n"),
                "sys/kernel/cap_last_cap" => {
                    let _ = writeln!(out, "{CAP_LAST_CAP}");
                }
                "sys/kernel/hostname" => {
                    out.push_str(core::str::from_utf8(source.hostname).unwrap_or(""));
                    out.push('\n');
                }
                "sys/kernel/ngroups_max" => out.push_str("65536\n"),
                "sys/kernel/osrelease" => {
                    let _ = writeln!(out, "{}", machine.release);
                }
                "sys/kernel/ostype" => out.push_str("Linux\n"),
                "sys/kernel/pid_max" => out.push_str("4194304\n"),
                "sys/kernel/random/boot_id" => out.push_str(&uuid(source.boot_id)),
                "sys/kernel/random/uuid" => out.push_str(&uuid(source.fresh_random)),
                "sys/vm/overcommit_memory" => out.push_str("1\n"),
                _ => return Err(Errno::EISDIR),
            }
        }
        _ => return Err(Errno::EINVAL),
    }
    Ok(out.into_bytes())
}

fn uuid(bytes: [u8; 16]) -> String {
    let mut bytes = bytes;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut out = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        let _ = write!(out, "{byte:02x}");
    }
    out.push('\n');
    out
}

/// What making a file's bytes may read besides the tree itself.
pub(crate) struct Source<'a> {
    pub view: &'a View<'a>,
    pub machine: &'a crate::Machine,
    pub hostname: &'a [u8],
    pub uptime_nanoseconds: u64,
    pub last_pid: i32,
    pub boot_id: [u8; 16],
    pub fresh_random: [u8; 16],
    /// Reads a range of a process's memory; bytes it cannot read end the result.
    pub read_memory: &'a mut dyn FnMut(&Process, u64, u64) -> Vec<u8>,
    /// The mount table as `/proc/<pid>/mounts` shows it.
    pub mounts: &'a str,
    /// The memory in use and what of it tmpfs holds, in bytes.
    pub memory_in_use: (u64, u64),
}

fn process_file(
    source: &mut Source<'_>,
    process: &Process,
    file: ProcessFile,
) -> Result<Vec<u8>, Errno> {
    let mut out = String::new();
    let stack = &process.starting_stack;
    let range = |(start, end): (u64, u64)| (start, end - start);
    match file {
        ProcessFile::Cmdline => {
            let (start, length) = range(stack.arguments);
            return Ok((source.read_memory)(process, start, length));
        }
        ProcessFile::Environ => {
            let (start, length) = range(stack.environment);
            return Ok((source.read_memory)(process, start, length));
        }
        ProcessFile::Auxv => return Ok(stack.auxiliary.clone()),
        ProcessFile::Comm => {
            let mut bytes = process.name.clone();
            bytes.push(b'\n');
            return Ok(bytes);
        }
        ProcessFile::Maps => {
            for mapping in process.map.iter() {
                let flag = |permission: Permissions, letter: char| {
                    if mapping.permissions.contains(permission) {
                        letter
                    } else {
                        '-'
                    }
                };
                let (offset, device, ino, path) = match &mapping.backing {
                    Backing::File {
                        node, path, offset, ..
                    } => {
                        let inode = node.borrow();
                        let deleted = if inode.shown_deleted() {
                            " (deleted)"
                        } else {
                            ""
                        };
                        (
                            *offset,
                            inode.fs.device,
                            inode.ino,
                            alloc::format!("{}{deleted}", String::from_utf8_lossy(path)),
                        )
                    }
                    Backing::Zero {
                        device,
                        ino,
                        offset,
                        name,
                    } => (
                        *offset,
                        *device,
                        *ino,
                        String::from_utf8_lossy(name).into_owned(),
                    ),
                    Backing::Heap => (0, 0, 0, String::from("[heap]")),
                    Backing::Stack => (0, 0, 0, String::from("[stack]")),
                    Backing::Special(name) => (0, 0, 0, String::from(*name)),
                    Backing::Anonymous | Backing::Droppable => (0, 0, 0, String::new()),
                };
                let (major, minor) = super::decode_device(device);
                let line = format!(
                    "{:08x}-{:08x} {}{}{}{} {:08x} {major:02x}:{minor:02x} {}",
                    mapping.start,
                    mapping.end,
                    flag(Permissions::READ, 'r'),
                    flag(Permissions::WRITE, 'w'),
                    flag(Permissions::EXECUTE, 'x'),
                    if mapping.shared { 's' } else { 'p' },
                    offset,
                    ino
                );
                if path.is_empty() {
                    let _ = writeln!(out, "{line} ");
                } else {
                    let _ = writeln!(out, "{line:<72} {path}");
                }
            }
        }
        ProcessFile::Stat => {
            let name = String::from_utf8_lossy(&process.name);
            let (vsize, _) = sizes(process);
            let schedule = source.view.threads[&process.threads[0]].schedule;
            // Linux's `task_prio`: 20 plus nice for the fair policies, below 0 for real time.
            let priority = match schedule.policy {
                SCHED_FIFO | SCHED_RR => -1 - schedule.priority as i32,
                SCHED_DEADLINE => -101,
                _ => 20 + schedule.nice,
            };
            let (ignored, caught) = ignored_and_caught(process);
            let blocked = source.view.threads[&process.threads[0]].signal_mask;
            // The controlling terminal's device number as `new_encode_dev` packs it, and its
            // foreground group.
            let (terminal, foreground) = process.terminal.as_ref().map_or((0, -1), |pty| {
                let pty = pty.borrow();
                let index = pty.index;
                let number = (index & 0xff)
                    | (crate::linux_extra::UNIX98_PTY_SLAVE_MAJOR << 8)
                    | ((index & !0xff) << 12);
                (number, pty.sides[crate::tty::SLAVE].pgrp.unwrap_or(0))
            });
            let _ = writeln!(
                out,
                "{} ({}) {} {} {} {} {} {} 4194560 0 0 0 0 0 0 0 0 {} {} {} 0 0 {} 0 0 0 0 0 0 0 0 {} {} {} 0 0 0 17 0 {} {} 0 0 0 0 0 0 0 0 0 0 0",
                process.pid,
                name,
                state(source.view, process).0,
                process.parent,
                process.group,
                process.session,
                terminal,
                foreground,
                priority,
                schedule.nice,
                process.threads.len(),
                vsize,
                // Only the first 31 signals, as Linux's `do_task_stat` shows them.
                blocked & 0x7fff_ffff,
                ignored & 0x7fff_ffff,
                caught & 0x7fff_ffff,
                schedule.priority,
                schedule.policy
            );
        }
        ProcessFile::Statm => {
            let (vsize, _) = sizes(process);
            let _ = writeln!(out, "{} 0 0 0 0 0 0", vsize / 4096);
        }
        ProcessFile::Status => {
            let credentials = &process.credentials;
            let filters = source.view.threads[&process.threads[0]].filters.len();
            let (vsize, _) = sizes(process);
            let _ = write!(
                out,
                "Name:\t{}\nUmask:\t{:04o}\nState:\t{} ({})\nTgid:\t{}\nNgid:\t0\nPid:\t{}\nPPid:\t{}\n\
                 TracerPid:\t0\nUid:\t{}\t{}\t{}\t{}\nGid:\t{}\t{}\t{}\t{}\nFDSize:\t{}\nGroups:\t",
                String::from_utf8_lossy(&process.name),
                process.umask,
                state(source.view, process).0,
                state(source.view, process).1,
                process.pid,
                process.pid,
                process.parent,
                credentials.uid,
                credentials.euid,
                credentials.suid,
                credentials.fsuid,
                credentials.gid,
                credentials.egid,
                credentials.sgid,
                credentials.fsgid,
                process.files.capacity()
            );
            // Separated by spaces, then always one more (Linux's `task_state`).
            for (index, group) in credentials.groups.iter().enumerate() {
                let _ = write!(out, "{}{group}", if index > 0 { " " } else { "" });
            }
            out.push(' ');
            let (ignored, caught) = ignored_and_caught(process);
            let _ = write!(
                out,
                "\nVmSize:\t{:8} kB\nThreads:\t{}\nSigBlk:\t{:016x}\nSigIgn:\t{:016x}\nSigCgt:\t{:016x}\n\
                 CapInh:\t{:016x}\nCapPrm:\t{:016x}\nCapEff:\t{:016x}\nCapBnd:\t{:016x}\nCapAmb:\t{:016x}\n\
                 NoNewPrivs:\t{}\nSeccomp:\t2\nSeccomp_filters:\t{}\n",
                vsize / 1024,
                process.threads.len(),
                source.view.threads[&process.threads[0]].signal_mask,
                ignored,
                caught,
                credentials.inheritable,
                credentials.permitted,
                credentials.effective,
                credentials.bounding,
                credentials.ambient,
                u8::from(credentials.no_new_privileges),
                // Docker's profile, then the first thread's own.
                1 + filters,
            );
        }
        ProcessFile::Mounts => out.push_str(source.mounts),
        ProcessFile::Limits => {
            const NAMES: [(&str, &str); 16] = [
                ("Max cpu time", "seconds"),
                ("Max file size", "bytes"),
                ("Max data size", "bytes"),
                ("Max stack size", "bytes"),
                ("Max core file size", "bytes"),
                ("Max resident set", "bytes"),
                ("Max processes", "processes"),
                ("Max open files", "files"),
                ("Max locked memory", "bytes"),
                ("Max address space", "bytes"),
                ("Max file locks", "locks"),
                ("Max pending signals", "signals"),
                ("Max msgqueue size", "bytes"),
                ("Max nice priority", ""),
                ("Max realtime priority", ""),
                ("Max realtime timeout", "us"),
            ];
            let _ = writeln!(
                out,
                "{:<25} {:<20} {:<20} {:<10}",
                "Limit", "Soft Limit", "Hard Limit", "Units"
            );
            for (index, (name, unit)) in NAMES.iter().enumerate() {
                let show = |value: u64| {
                    if value == RLIM_INFINITY as u64 {
                        String::from("unlimited")
                    } else {
                        format!("{value}")
                    }
                };
                let limit = process.limits[index];
                // As Linux's `proc_pid_limits` prints them: the unit is padded, and left out when there
                // is none.
                let _ = write!(
                    out,
                    "{:<25} {:<20} {:<20} ",
                    name,
                    show(limit.current),
                    show(limit.maximum)
                );
                if unit.is_empty() {
                    out.push('\n');
                } else {
                    let _ = writeln!(out, "{unit:<10}");
                }
            }
        }
        ProcessFile::Cgroup => out.push_str("0::/\n"),
    }
    Ok(out.into_bytes())
}

/// The total mapped size of a process, and the part that is writable.
/// The signals a process ignores and those it has handlers for, one bit each from bit 0 for signal 1
/// (Linux's `collect_sigign_sigcatch`).
fn ignored_and_caught(process: &Process) -> (u64, u64) {
    let (mut ignored, mut caught) = (0, 0);
    for (index, action) in process.actions.iter().enumerate() {
        match action.sa_handler_kernel {
            crate::linux_extra::SIG_IGN => ignored |= 1 << index,
            crate::linux_extra::SIG_DFL => {}
            _ => caught |= 1 << index,
        }
    }
    (ignored, caught)
}

fn sizes(process: &Process) -> (u64, u64) {
    let mut total = 0;
    let mut writable = 0;
    for mapping in process.map.iter() {
        total += mapping.end - mapping.start;
        if mapping.permissions.contains(Permissions::WRITE) {
            writable += mapping.end - mapping.start;
        }
    }
    (total, writable)
}

/// The inode of `/proc`'s root.
pub(crate) fn root(view: &View<'_>) -> Node {
    node(view, None, b"proc", Entry::Root)
}
