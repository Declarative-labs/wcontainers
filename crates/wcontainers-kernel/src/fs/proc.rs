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
use crate::linux_extra::{
    MQ_MSG_DEFAULT, MQ_MSG_MAX, MQ_MSGSIZE_DEFAULT, MQ_MSGSIZE_MAX, MQ_QUEUES_MAX, MSGMAX, MSGMNB,
    MSGMNI, SEMMNI, SEMMNS, SEMMSL, SEMOPM, SHMALL, SHMMAX, SHMMNI,
};
use crate::memory::Backing;
use crate::process::{Channel, Pid, Process, Thread, Tid};

/// What a walk through `/proc` needs to know about the kernel.
pub(crate) struct View<'a> {
    pub processes: &'a BTreeMap<Pid, Process>,
    pub threads: &'a BTreeMap<Tid, Thread>,
    /// The process walking, which `/proc/self` names, and its thread, which `/proc/thread-self`
    /// names.
    pub caller: Pid,
    pub caller_thread: Tid,
    pub fs: FileSystem,
    /// The file system namespaces' inodes are on.
    pub nsfs: FileSystem,
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
    ThreadSelfLink,
    /// A process's directory or a thread's, and what is in it.
    Process(Task),
    ProcessFile(Task, ProcessFile),
    ProcessLink(Task, ProcessLink),
    Fds(Task),
    Fd(Task, i32),
    /// A process's `fdinfo` directory, and a descriptor's file in it.
    FdInfos(Task),
    FdInfo(Task, i32),
    /// A process's `ns` directory, and the namespace links in it.
    Namespaces(Task),
    Namespace(Task, Namespace),
    /// A process's `net` directory (its network's), and the files in it.
    Net(Task),
    NetFile(Task, NetFile),
    /// A process's `task` directory, which holds a directory for each of its threads.
    Tasks(Pid),
    /// A directory or file outside the process directories, by its index in [`GLOBAL`].
    Global(usize),
}

/// Whose directory: the thread whose details its files show, and its process. A thread's own
/// directory (under `task`) holds Linux's `tid_base_stuff`; a process's, found by its number or by
/// one of its threads' numbers, `tgid_base_stuff`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Task {
    pub pid: Pid,
    pub tid: Tid,
    pub own: bool,
}

impl Task {
    fn path(&self) -> String {
        if self.own {
            format!("/{}/task/{}", self.pid, self.tid)
        } else {
            format!("/{}", self.tid)
        }
    }

    /// Its entries' list in [`LINUX_ENTRIES`].
    fn listing(&self) -> &'static str {
        if self.own { "<tid>" } else { "<pid>" }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessFile {
    Cmdline,
    Comm,
    CoredumpFilter,
    Environ,
    Auxv,
    OomAdj,
    OomScore,
    OomScoreAdj,
    Maps,
    Stat,
    Statm,
    Status,
    Mounts,
    Mountinfo,
    Limits,
    Cgroup,
}

/// The files of `/proc/net` built here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NetFile {
    Dev,
    IfInet6,
    Route,
    Tcp,
    Tcp6,
    Udp,
    Udp6,
    Unix,
}

/// This kernel's configuration as Linux's `CONFIG_IKCONFIG_PROC` gives it, compressed by the
/// build from `config` (the reference's with what is not built left out, made by `config.py`).
const CONFIG_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/config.gz"));

const NET_FILES: &[(&[u8], NetFile)] = &[
    (b"dev", NetFile::Dev),
    (b"if_inet6", NetFile::IfInet6),
    (b"route", NetFile::Route),
    (b"tcp", NetFile::Tcp),
    (b"tcp6", NetFile::Tcp6),
    (b"udp", NetFile::Udp),
    (b"udp6", NetFile::Udp6),
    (b"unix", NetFile::Unix),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessLink {
    Exe,
    Cwd,
    Root,
}

/// The links of a process's `ns` directory, in Linux's order (`ns_entries`). The container has one of
/// each namespace, shared by every process in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Namespace {
    Net,
    Uts,
    Ipc,
    Pid,
    PidForChildren,
    User,
    Mnt,
    Cgroup,
    Time,
    TimeForChildren,
}

impl Namespace {
    const ALL: [Namespace; 10] = [
        Namespace::Net,
        Namespace::Uts,
        Namespace::Ipc,
        Namespace::Pid,
        Namespace::PidForChildren,
        Namespace::User,
        Namespace::Mnt,
        Namespace::Cgroup,
        Namespace::Time,
        Namespace::TimeForChildren,
    ];

    fn name(self) -> &'static [u8] {
        match self {
            Namespace::Net => b"net",
            Namespace::Uts => b"uts",
            Namespace::Ipc => b"ipc",
            Namespace::Pid => b"pid",
            Namespace::PidForChildren => b"pid_for_children",
            Namespace::User => b"user",
            Namespace::Mnt => b"mnt",
            Namespace::Cgroup => b"cgroup",
            Namespace::Time => b"time",
            Namespace::TimeForChildren => b"time_for_children",
        }
    }

    /// The namespace's type, which its link names (`proc_ns_operations.name`).
    fn kind(self) -> &'static str {
        match self {
            Namespace::PidForChildren => "pid",
            Namespace::TimeForChildren => "time",
            other => core::str::from_utf8(other.name()).unwrap_or_default(),
        }
    }

    /// The namespace's inode number on nsfs. The user and time namespaces are the machine's first,
    /// which have Linux's fixed numbers (`PROC_USER_INIT_INO`, `PROC_TIME_INIT_INO`); the others are
    /// the container's own, numbered as Linux numbers new ones.
    pub fn ino(self) -> u64 {
        match self {
            Namespace::User => 0xEFFF_FFFD,
            Namespace::Time | Namespace::TimeForChildren => 0xEFFF_FFFA,
            Namespace::Mnt => 0xF000_0100,
            Namespace::Uts => 0xF000_0101,
            Namespace::Ipc => 0xF000_0102,
            Namespace::Pid | Namespace::PidForChildren => 0xF000_0103,
            Namespace::Net => 0xF000_0105,
            Namespace::Cgroup => 0xF000_0106,
        }
    }

    /// The `CLONE_NEW*` flag of its type, which `NS_GET_NSTYPE` answers.
    pub fn clone_flag(self) -> u32 {
        match self {
            Namespace::Net => CLONE_NEWNET,
            Namespace::Uts => CLONE_NEWUTS,
            Namespace::Ipc => CLONE_NEWIPC,
            Namespace::Pid | Namespace::PidForChildren => CLONE_NEWPID,
            Namespace::User => CLONE_NEWUSER,
            Namespace::Mnt => CLONE_NEWNS,
            Namespace::Cgroup => CLONE_NEWCGROUP,
            Namespace::Time | Namespace::TimeForChildren => CLONE_NEWTIME,
        }
    }

    /// What its link reads and a descriptor of it shows (`ns_get_name`).
    pub fn link(self) -> Vec<u8> {
        format!("{}:[{}]", self.kind(), self.ino()).into_bytes()
    }
}

const PROCESS_FILES: &[(&[u8], ProcessFile)] = &[
    (b"auxv", ProcessFile::Auxv),
    (b"cgroup", ProcessFile::Cgroup),
    (b"cmdline", ProcessFile::Cmdline),
    (b"comm", ProcessFile::Comm),
    (b"coredump_filter", ProcessFile::CoredumpFilter),
    (b"environ", ProcessFile::Environ),
    (b"limits", ProcessFile::Limits),
    (b"maps", ProcessFile::Maps),
    (b"mountinfo", ProcessFile::Mountinfo),
    (b"mounts", ProcessFile::Mounts),
    (b"oom_adj", ProcessFile::OomAdj),
    (b"oom_score", ProcessFile::OomScore),
    (b"oom_score_adj", ProcessFile::OomScoreAdj),
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
    ("config.gz", Global::File(0o444)),
    ("cpuinfo", Global::File(0o444)),
    ("filesystems", Global::File(0o444)),
    ("loadavg", Global::File(0o444)),
    ("meminfo", Global::File(0o444)),
    ("mounts", Global::Link("self/mounts")),
    ("net", Global::Link("self/net")),
    ("stat", Global::File(0o444)),
    ("sys", Global::Directory),
    ("sys/abi", Global::Directory),
    ("sys/debug", Global::Directory),
    ("sys/dev", Global::Directory),
    ("sys/dev/tty", Global::Directory),
    ("sys/dev/tty/ldisc_autoload", Global::File(0o644)),
    ("sys/dev/tty/legacy_tiocsti", Global::File(0o644)),
    ("sys/fs", Global::Directory),
    ("sys/fs/aio-max-nr", Global::File(0o644)),
    ("sys/fs/aio-nr", Global::File(0o444)),
    ("sys/fs/dir-notify-enable", Global::File(0o644)),
    ("sys/fs/lease-break-time", Global::File(0o644)),
    ("sys/fs/leases-enable", Global::File(0o644)),
    ("sys/fs/mqueue", Global::Directory),
    ("sys/fs/mqueue/msg_default", Global::File(0o644)),
    ("sys/fs/mqueue/msg_max", Global::File(0o644)),
    ("sys/fs/mqueue/msgsize_default", Global::File(0o644)),
    ("sys/fs/mqueue/msgsize_max", Global::File(0o644)),
    ("sys/fs/mqueue/queues_max", Global::File(0o644)),
    ("sys/fs/nr_open", Global::File(0o644)),
    ("sys/fs/pipe-max-size", Global::File(0o644)),
    ("sys/kernel", Global::Directory),
    ("sys/kernel/cap_last_cap", Global::File(0o444)),
    ("sys/kernel/hostname", Global::File(0o644)),
    ("sys/kernel/msgmax", Global::File(0o644)),
    ("sys/kernel/msgmnb", Global::File(0o644)),
    ("sys/kernel/msgmni", Global::File(0o644)),
    ("sys/kernel/ngroups_max", Global::File(0o444)),
    ("sys/kernel/osrelease", Global::File(0o444)),
    ("sys/kernel/ostype", Global::File(0o444)),
    ("sys/kernel/pid_max", Global::File(0o644)),
    ("sys/kernel/random", Global::Directory),
    ("sys/kernel/random/boot_id", Global::File(0o444)),
    ("sys/kernel/random/entropy_avail", Global::File(0o444)),
    ("sys/kernel/random/poolsize", Global::File(0o444)),
    ("sys/kernel/random/uuid", Global::File(0o444)),
    ("sys/kernel/sem", Global::File(0o644)),
    ("sys/kernel/shmall", Global::File(0o644)),
    ("sys/kernel/shmmax", Global::File(0o644)),
    ("sys/kernel/shmmni", Global::File(0o644)),
    ("sys/kernel/tainted", Global::File(0o644)),
    ("sys/kernel/version", Global::File(0o444)),
    ("sys/net", Global::Directory),
    ("sys/net/core", Global::Directory),
    ("sys/net/core/somaxconn", Global::File(0o644)),
    ("sys/sunrpc", Global::Directory),
    ("sys/user", Global::Directory),
    ("sys/vm", Global::Directory),
    ("sys/vm/drop_caches", Global::File(0o200)),
    ("sys/vm/hugetlb_shm_group", Global::File(0o644)),
    ("sys/vm/nr_hugepages", Global::File(0o644)),
    ("sys/vm/nr_overcommit_hugepages", Global::File(0o644)),
    ("sys/vm/overcommit_memory", Global::File(0o644)),
    ("sysvipc", Global::Directory),
    ("sysvipc/msg", Global::File(0o444)),
    ("sysvipc/sem", Global::File(0o444)),
    ("sysvipc/shm", Global::File(0o444)),
    ("uptime", Global::File(0o444)),
    ("version", Global::File(0o444)),
];

/// The entries Linux 7.0 has in the directories built here, in the order the reference's `getdents`
/// lists them, each directory's names separated by spaces; `<pid>` stands for a process directory.
/// An entry's position in its directory is 2 plus its index here, except in `/proc` itself, where
/// `self` and `thread-self` come at `FIRST_PROCESS_ENTRY`. A `-` is an entry the reference's
/// namespace does not see, which still takes a position. Looking up one that is not built is
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
    (
        "<tid>",
        concat!(
            "fd fdinfo ns net environ auxv status personality limits sched comm syscall ",
            "cmdline stat statm maps children mem cwd root exe mounts mountinfo clear_refs ",
            "smaps smaps_rollup pagemap attr wchan stack schedstat cpuset cgroup oom_score ",
            "oom_adj oom_score_adj loginuid sessionid io uid_map gid_map projid_map ",
            "setgroups ksm_merging_pages ksm_stat",
        ),
    ),
    (
        "<net>",
        concat!(
            "arp dev raw rpc tcp udp icmp igmp raw6 sctp snmp stat tcp6 udp6 unix vlan icmp6 ",
            "igmp6 ip_vs nfsfs pfkey pppoe ptype route snmp6 packet psched bonding netlink ",
            "netstat rt_acct udplite anycast6 fib_trie if_inet6 mcfilter pppol2tp rt_cache ",
            "sockstat udplite6 dev_mcast dev_snmp6 ip_mr_vif ip_vs_app mcfilter6 netfilter ",
            "protocols rt6_stats sockstat6 xfrm_stat xt_recent ip_vs_conn ipv6_route ",
            "ip_mr_cache ip_vs_stats fib_triestat nf_conntrack softnet_stat ip6_flowlabel ",
            "ipt_hashlimit ip6t_hashlimit ip_tables_names ip_vs_conn_sync arp_tables_names ",
            "ip6_tables_names ip_tables_matches ip_tables_targets arp_tables_matches ",
            "arp_tables_targets ip6_tables_matches ip6_tables_targets ip_vs_stats_percpu ",
            "nf_conntrack_expect",
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
    ("sysvipc", "msg sem shm"),
    (
        "sys/fs/mqueue",
        "msg_default msg_max msgsize_default msgsize_max queues_max",
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
        "bridge - core ipv4 ipv6 mpls - netfilter sctp unix vsock",
    ),
    (
        "sys/net/core",
        concat!(
            "bypass_prot_mem optmem_max rmem_default rmem_max rps_default_mask somaxconn ",
            "tstamp_allow_data txq_reselection_ms txrehash wmem_default wmem_max ",
            "xfrm_acq_expires xfrm_aevent_etime xfrm_aevent_rseqth xfrm_larval_drop",
        ),
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
    /// `proc_coredump_filter_write`: chooses what a core dump would hold.
    CoredumpFilter,
    /// `oom_score_adj_write` (false) or the older `oom_adj_write` (true).
    OomScoreAdj(bool),
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
/// Whether a file of the tree has a poll (`file_can_poll`): the mount tables (`mounts_poll`) and
/// sysctls (`proc_sys_poll`).
pub(crate) fn polls(entry: &Entry) -> bool {
    match entry {
        Entry::ProcessFile(_, ProcessFile::Mounts | ProcessFile::Mountinfo) => true,
        Entry::Global(index) => GLOBAL[*index].0.starts_with("sys/"),
        _ => false,
    }
}

pub(crate) fn is_sysctl_root(index: usize) -> bool {
    GLOBAL[index].0 == "sys"
}

impl Entry {
    /// A directory's path below the tree's root, for finding it again when the inodes above it, which
    /// are made at each lookup, are gone.
    pub fn directory_path(&self) -> Option<String> {
        match *self {
            Entry::Root => Some(String::new()),
            Entry::Process(task) => Some(task.path()),
            Entry::Fds(task) => Some(format!("{}/fd", task.path())),
            Entry::FdInfos(task) => Some(format!("{}/fdinfo", task.path())),
            Entry::Net(task) => Some(format!("{}/net", task.path())),
            Entry::Namespaces(task) => Some(format!("{}/ns", task.path())),
            Entry::Tasks(pid) => Some(format!("/{pid}/task")),
            Entry::Global(index) => Some(format!("/{}", GLOBAL[index].0)),
            _ => None,
        }
    }

    /// The process an entry belongs to, for the entries inside a process's directory and the
    /// directory itself.
    pub fn of_process(&self) -> Option<Pid> {
        match *self {
            Entry::Process(task)
            | Entry::ProcessFile(task, _)
            | Entry::ProcessLink(task, _)
            | Entry::Fds(task)
            | Entry::Fd(task, _)
            | Entry::FdInfos(task)
            | Entry::FdInfo(task, _)
            | Entry::Net(task)
            | Entry::NetFile(task, _)
            | Entry::Namespaces(task)
            | Entry::Namespace(task, _) => Some(task.pid),
            Entry::Tasks(pid) => Some(pid),
            Entry::Root | Entry::SelfLink | Entry::ThreadSelfLink | Entry::Global(_) => None,
        }
    }

    /// Whether a change to the entry's owner, mode or times lasts (see [`Kept`]).
    pub fn keeps_attributes(&self) -> bool {
        matches!(
            self,
            Entry::Root | Entry::SelfLink | Entry::ThreadSelfLink | Entry::Global(_)
        )
    }

    /// The size `stat` reports: none, except for descriptor links.
    pub fn size(&self) -> u64 {
        match self {
            // Linux's `proc_fd_instantiate` gives descriptor links a size of 64 (measured too).
            Entry::Fd(..) => 64,
            // `proc_set_size` gives the compressed configuration's.
            Entry::Global(index) if GLOBAL[*index].0 == "config.gz" => CONFIG_GZ.len() as u64,
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
            Entry::ProcessFile(_, ProcessFile::CoredumpFilter) => Writing::CoredumpFilter,
            Entry::ProcessFile(_, ProcessFile::OomScoreAdj) => Writing::OomScoreAdj(false),
            Entry::ProcessFile(_, ProcessFile::OomAdj) => Writing::OomScoreAdj(true),
            Entry::Global(_) if self.style() == Style::Sysctl => Writing::Sysctl,
            // `proc_reg_write` with no `proc_write` of the entry's own.
            Entry::Global(_) | Entry::NetFile(..) => Writing::Io,
            _ => Writing::None,
        }
    }

    /// Whether reads and writes go a segment at a time (Linux's `do_loop_readv_writev`, which takes
    /// no flag but RWF_HIPRI): the files with a plain `read` or `write` and no iterating one.
    pub fn loops(&self, writing: bool) -> bool {
        match self.style() {
            Style::Sysctl => false,
            // `proc_reg_read_iter` reads the entries outside the process directories and the
            // network's; their writes are `proc_reg_write`.
            _ if matches!(self, Entry::Global(_) | Entry::NetFile(..)) => writing,
            _ => true,
        }
    }

    fn mode(&self) -> u32 {
        match self {
            Entry::Root | Entry::Process(_) | Entry::Tasks(_) | Entry::FdInfos(_) => {
                S_IFDIR | 0o555
            }
            Entry::Fds(_) => S_IFDIR | 0o500,
            Entry::Namespaces(_) => S_IFDIR | 0o511,
            Entry::FdInfo(..) => S_IFREG | 0o444,
            Entry::Net(_) => S_IFDIR | 0o555,
            Entry::NetFile(..) => S_IFREG | 0o444,
            Entry::Global(index) => match GLOBAL[*index].1 {
                Global::Directory => S_IFDIR | 0o555,
                Global::File(permissions) => S_IFREG | permissions,
                Global::Link(_) => S_IFLNK | 0o777,
            },
            Entry::ProcessFile(
                _,
                ProcessFile::Comm
                | ProcessFile::CoredumpFilter
                | ProcessFile::OomAdj
                | ProcessFile::OomScoreAdj,
            ) => S_IFREG | 0o644,
            Entry::ProcessFile(_, ProcessFile::Environ | ProcessFile::Auxv) => S_IFREG | 0o400,
            Entry::ProcessFile(..) => S_IFREG | 0o444,
            Entry::SelfLink
            | Entry::ThreadSelfLink
            | Entry::ProcessLink(..)
            | Entry::Namespace(..) => S_IFLNK | 0o777,
            Entry::Fd(..) => S_IFLNK | 0o700,
        }
    }

    /// A stable inode number for the entry.
    fn ino(&self) -> u64 {
        let task = |task: &Task, index: u64| {
            ((task.tid as u64) << 16) | (u64::from(task.own) << 47) | index
        };
        match self {
            Entry::Root => 1,
            Entry::SelfLink => 2,
            Entry::ThreadSelfLink => 3,
            Entry::Global(index) => 16 + *index as u64,
            Entry::Process(t) => task(t, 1),
            Entry::Fds(t) => task(t, 2),
            Entry::Net(t) => task(t, 4),
            Entry::NetFile(t, file) => task(t, 128 + *file as u64),
            Entry::Tasks(pid) => ((*pid as u64) << 16) | 3,
            Entry::ProcessFile(t, file) => task(t, 16 + *file as u64),
            Entry::ProcessLink(t, link) => task(t, 64 + *link as u64),
            Entry::Fd(t, fd) => task(t, 0x1000 + *fd as u64 % 0xf000),
            Entry::FdInfos(t) => task(t, 5),
            Entry::Namespaces(t) => task(t, 6),
            Entry::Namespace(t, namespace) => task(t, 96 + *namespace as u64),
            Entry::FdInfo(t, fd) => task(t, 0x1000 + *fd as u64 % 0xf000) | 1 << 46,
        }
    }

    fn owner(&self, view: &View<'_>) -> (u32, u32) {
        let tid = match *self {
            Entry::Process(task)
            | Entry::ProcessFile(task, _)
            | Entry::ProcessLink(task, _)
            | Entry::Fds(task)
            | Entry::Fd(task, _)
            | Entry::FdInfos(task)
            | Entry::FdInfo(task, _)
            | Entry::Namespaces(task)
            | Entry::Namespace(task, _) => task.tid,
            Entry::Tasks(pid) => pid,
            // The network's entries belong to its owner, root.
            Entry::Root
            | Entry::SelfLink
            | Entry::ThreadSelfLink
            | Entry::Global(_)
            | Entry::Net(_)
            | Entry::NetFile(..) => {
                return (0, 0);
            }
        };
        match self.of_process() {
            Some(pid) => dump_owner(view.processes, view.threads, pid, tid, self.mode()),
            None => (0, 0),
        }
    }
}

/// Linux's `task_dump_owner`: the owner of a process's entry with `mode`, the process's effective ids,
/// except that what is not a world readable directory belongs to root while the process may not be
/// dumped.
pub(crate) fn dump_owner(
    processes: &BTreeMap<Pid, Process>,
    threads: &BTreeMap<Tid, Thread>,
    pid: Pid,
    tid: Tid,
    mode: u32,
) -> (u32, u32) {
    processes.get(&pid).map_or((0, 0), |process| {
        if mode != S_IFDIR | 0o555 && !process.map.borrow().dumpable {
            (0, 0)
        } else {
            let credentials = task_credentials(processes, threads, tid);
            (credentials.euid, credentials.egid)
        }
    })
}

/// The credentials of the task a number names (`__task_cred`): a live thread's, or what a
/// process's first thread kept at its end.
pub(crate) fn task_credentials<'a>(
    processes: &'a BTreeMap<Pid, Process>,
    threads: &'a BTreeMap<Tid, Thread>,
    tid: Tid,
) -> &'a crate::credentials::Credentials {
    match threads.get(&tid) {
        Some(thread) => &thread.credentials,
        None => &processes[&tid].ended_leader.credentials,
    }
}

/// Linux's `ptrace_may_access` with PTRACE_MODE_READ_FSCREDS: the caller's own process, or one
/// whose ids all match the caller's file system ids, that may be dumped and whose permitted
/// powers the caller's effective ones cover (`cap_ptrace_access_check`), unless the caller has
/// CAP_SYS_PTRACE.
pub(crate) fn ptrace_may_read(
    processes: &BTreeMap<Pid, Process>,
    threads: &BTreeMap<Tid, Thread>,
    caller: Tid,
    target: Pid,
) -> bool {
    if threads[&caller].process == target {
        return true;
    }
    let Some(target_process) = processes.get(&target) else {
        return false;
    };
    let (c, t) = (
        &threads[&caller].credentials,
        task_credentials(processes, threads, target),
    );
    let traces = c.has(CAP_SYS_PTRACE);
    let same = [t.uid, t.euid, t.suid].iter().all(|id| *id == c.fsuid)
        && [t.gid, t.egid, t.sgid].iter().all(|id| *id == c.fsgid);
    if !same && !traces {
        return false;
    }
    if target_process.ended.is_none() && !target_process.map.borrow().dumpable && !traces {
        return false;
    }
    t.permitted & !c.effective == 0 || traces
}

/// Whether `caller` may see the descriptors of `entry`'s process through its `fdinfo` directory
/// (`proc_fdinfo_access_allowed`).
pub(crate) fn fdinfo_allowed(view: &View<'_>, entry: &Entry, caller: Tid) -> Result<(), Errno> {
    match entry {
        Entry::FdInfos(task) | Entry::FdInfo(task, _)
            if !ptrace_may_read(view.processes, view.threads, caller, task.pid) =>
        {
            Err(Errno::EACCES)
        }
        _ => Ok(()),
    }
}

/// A thread's state (`get_task_state`): ended and not reaped (a process's first thread stays so
/// while others go on), stopped, parked (unkillably only for a `vfork` parent's wait), or running.
fn state(view: &View<'_>, process: &Process, tid: Tid) -> (char, &'static str) {
    let thread = view.threads.get(&tid);
    let wait = thread.and_then(|thread| thread.call.wait);
    if process.ended.is_some() || thread.is_none() {
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

/// Makes the inode for an entry found in `parent` under `name`.
pub(crate) fn node(view: &View<'_>, parent: Option<&Node>, name: &[u8], entry: Entry) -> Node {
    let (uid, gid) = entry.owner(view);
    let mut mode = entry.mode();
    // A descriptor's link shows how the file was opened: reading gives read and execute, writing
    // write and execute, and an `O_PATH` descriptor nothing (Linux's `tid_fd_update_inode`).
    if let Entry::Fd(task, fd) = entry
        && let Some(file) = view
            .threads
            .get(&task.tid)
            .and_then(|thread| thread.files.borrow().file(fd).ok())
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
    // subdirectories, a thread's five, `task` its threads, and the sysctl directories none.
    match entry {
        Entry::Process(task) => node.borrow_mut().nlink = if task.own { 7 } else { 9 },
        // The network's directory counts Linux's eleven subdirectories.
        Entry::Net(_) => node.borrow_mut().nlink = 13,
        Entry::Tasks(pid) => {
            node.borrow_mut().nlink = 2 + view
                .processes
                .get(&pid)
                .map_or(0, |process| tasks(process).count() as u32);
        }
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
    // Threads other than a process's first are not listed in `/proc`, and are found there too.
    let found = found.or_else(|| match entry {
        Entry::Root => parse_number(name)
            .and_then(|tid| i32::try_from(tid).ok())
            .and_then(|tid| view.threads.get(&tid).map(|thread| (thread.process, tid)))
            .map(|(pid, tid)| {
                Entry::Process(Task {
                    pid,
                    tid,
                    own: false,
                })
            }),
        _ => None,
    });
    let Some(child) = found else {
        not_built(view, entry, name);
        return Err(Errno::ENOENT);
    };
    Ok(node(view, Some(directory), name, child))
}

/// `kstrtouint` in base 0: an optional `+`, then `0x` and hex digits, `0` and octal ones, or
/// decimal ones, then at most a newline before the end or a zero byte.
/// `kstrtoint` in base 0: `parse_unsigned` after an optional minus sign.
pub(crate) fn parse_signed(bytes: &[u8]) -> Result<i32, Errno> {
    match bytes {
        [b'-', rest @ ..] => {
            let magnitude = i64::from(parse_unsigned(rest)?);
            i32::try_from(-magnitude).map_err(|_| Errno::ERANGE)
        }
        _ => i32::try_from(parse_unsigned(bytes)?).map_err(|_| Errno::ERANGE),
    }
}

pub(crate) fn parse_unsigned(bytes: &[u8]) -> Result<u32, Errno> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let mut text = &bytes[..end];
    if let [b'+', rest @ ..] = text {
        text = rest;
    }
    let radix = match text {
        [b'0', b'x' | b'X', digit, ..] if digit.is_ascii_hexdigit() => {
            text = &text[2..];
            16
        }
        [b'0', ..] => 8,
        _ => 10,
    };
    let digits = text
        .iter()
        .take_while(|b| char::from(**b).is_digit(radix))
        .count();
    if digits == 0 {
        return Err(Errno::EINVAL);
    }
    let rest = &text[digits..];
    if !(rest.is_empty() || rest == b"\n") {
        return Err(Errno::EINVAL);
    }
    let mut value: u64 = 0;
    for &b in &text[..digits] {
        let digit = char::from(b).to_digit(radix).expect("a digit");
        value = value
            .checked_mul(u64::from(radix))
            .and_then(|value| value.checked_add(u64::from(digit)))
            .ok_or(Errno::ERANGE)?;
    }
    u32::try_from(value).map_err(|_| Errno::ERANGE)
}

/// Counts a lookup that found nothing when Linux has the name there.
fn not_built(view: &View<'_>, entry: &Entry, name: &[u8]) {
    let directory = match entry {
        Entry::Root => "",
        Entry::Process(task) => task.listing(),
        Entry::Net(_) => "<net>",
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
            out.push((
                b"thread-self".to_vec(),
                Entry::ThreadSelfLink,
                linux_position("", "thread-self"),
            ));
            for pid in view.processes.keys() {
                out.push((
                    format!("{pid}").into_bytes(),
                    Entry::Process(Task {
                        pid: *pid,
                        tid: *pid,
                        own: false,
                    }),
                    process_position(*pid),
                ));
            }
        }
        Entry::Process(task) => {
            let task = *task;
            let mut add = |name: &[u8], child: Entry| {
                let position = linux_position(
                    task.listing(),
                    core::str::from_utf8(name).unwrap_or_default(),
                );
                out.push((name.to_vec(), child, position));
            };
            for (name, file) in PROCESS_FILES {
                // A thread's own directory has no coredump_filter (`tid_base_stuff`).
                if task.own && *file == ProcessFile::CoredumpFilter {
                    continue;
                }
                add(name, Entry::ProcessFile(task, *file));
            }
            for (name, link) in PROCESS_LINKS {
                add(name, Entry::ProcessLink(task, *link));
            }
            add(b"fd", Entry::Fds(task));
            add(b"fdinfo", Entry::FdInfos(task));
            add(b"net", Entry::Net(task));
            add(b"ns", Entry::Namespaces(task));
            if !task.own {
                add(b"task", Entry::Tasks(task.pid));
            }
        }
        // `proc_task_readdir`: the threads from position 2 on, in the order they were made.
        Entry::Tasks(pid) => {
            if let Some(process) = view.processes.get(pid) {
                for (index, tid) in tasks(process).enumerate() {
                    out.push((
                        format!("{tid}").into_bytes(),
                        Entry::Process(Task {
                            pid: *pid,
                            tid,
                            own: true,
                        }),
                        index as u64 + 2,
                    ));
                }
            }
        }
        Entry::Fds(task) | Entry::FdInfos(task) => {
            if let Some(thread) = view.threads.get(&task.tid) {
                for (fd, _) in thread.files.borrow().iter() {
                    let child = if matches!(entry, Entry::Fds(_)) {
                        Entry::Fd(*task, fd)
                    } else {
                        Entry::FdInfo(*task, fd)
                    };
                    out.push((format!("{fd}").into_bytes(), child, fd as u64 + 2));
                }
            }
        }
        // `proc_ns_dir_readdir`: every link, whether or not the process still has its namespaces.
        Entry::Namespaces(task) => {
            for (index, namespace) in Namespace::ALL.into_iter().enumerate() {
                out.push((
                    namespace.name().to_vec(),
                    Entry::Namespace(*task, namespace),
                    index as u64 + 2,
                ));
            }
        }
        Entry::Net(task) => {
            for (name, file) in NET_FILES {
                let text = core::str::from_utf8(name).unwrap_or_default();
                out.push((
                    name.to_vec(),
                    Entry::NetFile(*task, *file),
                    linux_position("<net>", text),
                ));
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

/// A process's threads as its `task` directory lists them: the first, which stays until the process
/// is reaped, then the others in the order they were made.
fn tasks(process: &Process) -> impl Iterator<Item = Tid> + '_ {
    core::iter::once(process.pid).chain(
        process
            .threads
            .iter()
            .copied()
            .filter(move |tid| *tid != process.pid),
    )
}

/// The process and thread a process link names: a thread that has ended has no program, working
/// directory or root (its memory and file system context are gone), so the link leads nowhere.
fn linked<'a>(view: &'a View<'_>, task: &Task) -> Result<(&'a Process, &'a Thread), Errno> {
    let thread = view.threads.get(&task.tid).ok_or(Errno::ENOENT)?;
    let process = view.processes.get(&task.pid).ok_or(Errno::ENOENT)?;
    Ok((process, thread))
}

/// The namespace a process's link names, if the caller may see it (`proc_ns_get_link` and
/// `proc_ns_readlink` ask `ptrace_may_access`) and the process still has it: an ended one keeps its
/// process number's namespace and its credentials' user namespace, and gave the others back
/// (`exit_task_namespaces`).
fn named_namespace(view: &View<'_>, task: &Task, namespace: Namespace) -> Result<(), Errno> {
    if !ptrace_may_read(view.processes, view.threads, view.caller_thread, task.pid) {
        return Err(Errno::EACCES);
    }
    let ended = view
        .processes
        .get(&task.pid)
        .is_none_or(|process| process.ended.is_some())
        || !view.threads.contains_key(&task.tid);
    if ended && !matches!(namespace, Namespace::Pid | Namespace::User) {
        return Err(Errno::ENOENT);
    }
    Ok(())
}

/// The nsfs inode of a namespace: one number for each, read-only and immutable (`__ns_get_path`).
fn namespace_inode(view: &View<'_>, namespace: Namespace) -> Node {
    namespace_node(view.nsfs.clone(), view.boot, namespace)
}

/// The nsfs inode of a namespace, on `nsfs`, made at `boot`.
pub(crate) fn namespace_node(nsfs: FileSystem, boot: Time, namespace: Namespace) -> Node {
    let node = new_inode(
        nsfs,
        namespace.ino(),
        New {
            mode: S_IFREG | 0o444,
            uid: 0,
            gid: 0,
            rdev: 0,
            now: boot,
            content: Content::Namespace(namespace),
            parent: None,
            name: namespace.link(),
        },
    )
    .expect("nsfs counts no space");
    node.borrow_mut().fsflags = FS_IMMUTABLE_FL;
    node
}

/// Where a symbolic link of the tree points, as `readlink` gives it.
pub(crate) fn link_target(
    view: &View<'_>,
    walk: &Walk<'_>,
    entry: &Entry,
) -> Result<Vec<u8>, Errno> {
    match entry {
        Entry::SelfLink => Ok(format!("{}", view.caller).into_bytes()),
        Entry::ThreadSelfLink => {
            Ok(format!("{}/task/{}", view.caller, view.caller_thread).into_bytes())
        }
        Entry::Global(index) => match GLOBAL[*index].1 {
            Global::Link(target) => Ok(target.as_bytes().to_vec()),
            _ => Err(Errno::EINVAL),
        },
        Entry::ProcessLink(task, link) => {
            let (process, thread) = linked(view, task)?;
            let fs = thread.fs.borrow();
            match link {
                ProcessLink::Exe => Ok(process.executable.1.clone()),
                ProcessLink::Cwd => Ok(walk.path_of(&fs.cwd).unwrap_or_default()),
                ProcessLink::Root => Ok(walk.path_of(&fs.root).unwrap_or_default()),
            }
        }
        Entry::Namespace(task, namespace) => {
            named_namespace(view, task, *namespace)?;
            Ok(namespace.link())
        }
        Entry::Fd(task, fd) => {
            let thread = view.threads.get(&task.tid).ok_or(Errno::ENOENT)?;
            let file = thread.files.borrow().file(*fd).map_err(|_| Errno::ENOENT)?;
            let file = file.borrow();
            let inode = file.node.borrow();
            Ok(match &inode.content {
                Content::Pipe(_) if inode.fs.kind == super::Kind::Pipe => {
                    format!("pipe:[{}]", inode.ino).into_bytes()
                }
                // nsfs names its files itself (`ns_dname`).
                Content::Namespace(namespace) => namespace.link(),
                Content::Directory(_) | Content::Proc(_) if inode.is_directory() => {
                    drop(inode);
                    walk.path_of(&file.node)
                        .unwrap_or_else(|| file.path.clone())
                }
                _ if inode.shown_deleted() || file.unnamed() => {
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
        Entry::ProcessLink(..) | Entry::Namespace(..) => link_target(view, walk, entry).ok(),
        Entry::Fd(task, fd) => {
            let file = view.threads.get(&task.tid)?.files.borrow().file(*fd).ok()?;
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
        Entry::ProcessLink(task, link) => {
            let (process, thread) = linked(view, task)?;
            Ok(match link {
                ProcessLink::Exe => process.executable.0.clone(),
                ProcessLink::Cwd => thread.fs.borrow().cwd.clone(),
                ProcessLink::Root => thread.fs.borrow().root.clone(),
            })
        }
        Entry::Namespace(task, namespace) => {
            named_namespace(view, task, *namespace)?;
            Ok(namespace_inode(view, *namespace))
        }
        Entry::Fd(task, fd) => {
            let thread = view.threads.get(&task.tid).ok_or(Errno::ENOENT)?;
            let file = thread.files.borrow().file(*fd).map_err(|_| Errno::ENOENT)?;
            let node = file.borrow().node.clone();
            walk.internal.set(file.borrow().internal_mount);
            Ok(node)
        }
        _ => {
            let target = link_target(view, walk, entry)?;
            walk.resolve_from(at, &target, links)
        }
    }
}

/// What an open of a file of the tree refuses before anything is read: the files of an ended
/// process that need its memory (`__mem_open` finds none) or its mount namespace
/// (`mounts_open_common`).
pub(crate) fn open_check(processes: &BTreeMap<Pid, Process>, entry: &Entry) -> Result<(), Errno> {
    let Entry::ProcessFile(task, file) = entry else {
        return Ok(());
    };
    if processes
        .get(&task.pid)
        .is_some_and(|process| process.ended.is_some())
    {
        match file {
            ProcessFile::Environ | ProcessFile::Auxv => return Err(Errno::ESRCH),
            ProcessFile::Mounts | ProcessFile::Mountinfo => return Err(Errno::EINVAL),
            _ => {}
        }
    }
    Ok(())
}

/// Makes the bytes of a file of the tree.
pub(crate) fn contents(source: &mut Source<'_>, entry: &Entry) -> Result<Vec<u8>, Errno> {
    let mut out = String::new();
    match entry {
        Entry::Global(index) if GLOBAL[*index].0 == "config.gz" => return Ok(CONFIG_GZ.to_vec()),
        Entry::ProcessFile(task, file) => {
            let process = source.view.processes.get(&task.pid).ok_or(Errno::ESRCH)?;
            return process_file(source, process, *task, *file);
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
                // The registered file systems that are built, in Linux's order of registration.
                "filesystems" => out.push_str(
                    "nodev\tsysfs\nnodev\ttmpfs\nnodev\tproc\nnodev\tsockfs\nnodev\tpipefs\n\
                     nodev\tdevpts\nnodev\tfuse\nnodev\tmqueue\n",
                ),
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
                         Shmem:          {shared:8} kB\nHugePages_Total:       0\n\
                         HugePages_Free:        0\nHugePages_Rsvd:        0\n\
                         HugePages_Surp:        0\nHugepagesize:       2048 kB\n\
                         Hugetlb:               0 kB\n"
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
                "sys/fs/aio-max-nr" => {
                    let _ = writeln!(out, "{}", crate::syscall::aio::MAX_NR);
                }
                "sys/fs/aio-nr" => {
                    let _ = writeln!(out, "{}", source.aio_in_use);
                }
                "sys/fs/dir-notify-enable" | "sys/fs/leases-enable" => out.push_str("1\n"),
                "sys/fs/lease-break-time" => {
                    let _ = writeln!(out, "{}", crate::syscall::lease::BREAK_SECONDS);
                }
                "sys/fs/mqueue/msg_default" => {
                    let _ = writeln!(out, "{MQ_MSG_DEFAULT}");
                }
                "sys/fs/mqueue/msg_max" => {
                    let _ = writeln!(out, "{MQ_MSG_MAX}");
                }
                "sys/fs/mqueue/msgsize_default" => {
                    let _ = writeln!(out, "{MQ_MSGSIZE_DEFAULT}");
                }
                "sys/fs/mqueue/msgsize_max" => {
                    let _ = writeln!(out, "{MQ_MSGSIZE_MAX}");
                }
                "sys/fs/mqueue/queues_max" => {
                    let _ = writeln!(out, "{MQ_QUEUES_MAX}");
                }
                "sys/fs/nr_open" => out.push_str("1048576\n"),
                "sys/fs/pipe-max-size" => {
                    let _ = writeln!(out, "{}", crate::pipe::MAX_SIZE);
                }
                "sys/kernel/msgmax" => {
                    let _ = writeln!(out, "{MSGMAX}");
                }
                "sys/kernel/msgmnb" => {
                    let _ = writeln!(out, "{MSGMNB}");
                }
                "sys/kernel/msgmni" => {
                    let _ = writeln!(out, "{MSGMNI}");
                }
                "sys/kernel/sem" => {
                    let _ = writeln!(out, "{SEMMSL}\t{SEMMNS}\t{SEMOPM}\t{SEMMNI}");
                }
                "sys/kernel/shmall" => {
                    let _ = writeln!(out, "{SHMALL}");
                }
                "sys/kernel/shmmax" => {
                    let _ = writeln!(out, "{SHMMAX}");
                }
                "sys/kernel/shmmni" => {
                    let _ = writeln!(out, "{SHMMNI}");
                }
                // Nothing has tainted this kernel.
                "sys/kernel/tainted" => out.push_str("0\n"),
                "sys/kernel/version" => {
                    let _ = writeln!(out, "{}", machine.version);
                }
                // The input pool's size in bits, which Linux 5.18 on always shows full.
                "sys/kernel/random/entropy_avail" | "sys/kernel/random/poolsize" => {
                    out.push_str("256\n")
                }
                // The cap `listen` puts on a backlog.
                "sys/net/core/somaxconn" => out.push_str("4096\n"),
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
                // No huge page is ever reserved, as none can be in a default container.
                "sys/vm/hugetlb_shm_group"
                | "sys/vm/nr_hugepages"
                | "sys/vm/nr_overcommit_hugepages" => out.push_str("0\n"),
                "sys/vm/overcommit_memory" => out.push_str("1\n"),
                "sysvipc/msg" => out = crate::syscall::ipc::proc_msg(source.ipc),
                "sysvipc/sem" => out = crate::syscall::ipc::proc_sem(source.ipc),
                "sysvipc/shm" => {
                    out = crate::syscall::ipc::proc_shm(source.ipc, source.view.counters)
                }
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
    pub mounts: &'a [super::mounts::Mount],
    /// The memory in use and what of it tmpfs holds, in bytes.
    pub memory_in_use: (u64, u64),
    /// System V IPC's objects.
    pub ipc: &'a crate::syscall::ipc::Ipc,
    /// The events asynchronous I/O contexts asked for (`aio_nr`).
    pub aio_in_use: u64,
}

/// A file of a process's or thread's directory, which shows thread `tid` of `process`.
fn process_file(
    source: &mut Source<'_>,
    process: &Process,
    task: Task,
    file: ProcessFile,
) -> Result<Vec<u8>, Errno> {
    let tid = task.tid;
    let shown = shown(source.view, process, tid);
    let mut out = String::new();
    let stack = &process.starting_stack;
    let range = |(start, end): (u64, u64)| (start, end - start);
    match file {
        // An ended process's memory is gone, so its command line and environment read empty
        // (`environ` and `auxv` are refused at the open; `auxv` opened before keeps the saved
        // vector).
        ProcessFile::Cmdline | ProcessFile::Environ if process.ended.is_some() => {
            return Ok(Vec::new());
        }
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
            let mut bytes = shown.name.clone();
            bytes.push(b'\n');
            return Ok(bytes);
        }
        ProcessFile::CoredumpFilter => {
            return Ok(alloc::format!("{:08x}\n", process.coredump_filter).into_bytes());
        }
        ProcessFile::OomScoreAdj => {
            let _ = writeln!(out, "{}", process.oom_score_adj);
        }
        // `oom_adj_read`: the older scale's -17 to 15.
        ProcessFile::OomAdj => {
            use crate::linux_extra::{OOM_ADJUST_MAX, OOM_DISABLE, OOM_SCORE_ADJ_MAX};
            let adj = i32::from(process.oom_score_adj);
            let shown = if adj == OOM_SCORE_ADJ_MAX {
                OOM_ADJUST_MAX
            } else {
                adj * -OOM_DISABLE / OOM_SCORE_ADJ_MAX
            };
            let _ = writeln!(out, "{shown}");
        }
        // `proc_oom_score`: two thirds of 1000 plus the badness `oom_badness` gives per thousand
        // pages of the machine, whose resident part is not tracked here (counted).
        ProcessFile::OomScore => {
            let adj = i64::from(process.oom_score_adj);
            let points = if adj == i64::from(crate::linux_extra::OOM_SCORE_ADJ_MIN) {
                0
            } else {
                let pages = (source.machine.memory_bytes / 4096) as i64;
                let badness = adj * (pages / 1000);
                (1000 + badness * 1000 / pages) * 2 / 3
            };
            let _ = writeln!(out, "{points}");
        }
        ProcessFile::Maps => {
            for mapping in process.map.borrow().iter() {
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
            let name = String::from_utf8_lossy(&shown.name);
            let (vsize, _) = sizes(process);
            let schedule = shown.schedule;
            // Linux's `task_prio`: 20 plus nice for the fair policies, below 0 for real time.
            let priority = match schedule.policy {
                SCHED_FIFO | SCHED_RR => -1 - schedule.priority as i32,
                SCHED_DEADLINE => -101,
                _ => 20 + schedule.nice,
            };
            let (ignored, caught) = ignored_and_caught(process);
            let blocked = shown.signal_mask;
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
            // CPU time in clock ticks (USER_HZ 100), all of it as the user's, as `times`
            // gives it: a thread's own in its task directory, else the process's with its
            // ended threads (`do_task_stat`), and its reaped children's.
            let ticks = |nanoseconds: u64| nanoseconds / 10_000_000;
            let used = if task.own {
                source
                    .view
                    .threads
                    .get(&tid)
                    .map_or(0, |thread| thread.runtime)
            } else {
                process.ended_time
                    + process
                        .threads
                        .iter()
                        .filter_map(|thread| source.view.threads.get(thread))
                        .map(|thread| thread.runtime)
                        .sum::<u64>()
            };
            let children = if task.own { 0 } else { process.children_time };
            let _ = writeln!(
                out,
                "{} ({}) {} {} {} {} {} {} 4194560 0 0 0 0 {} 0 {} 0 {} {} {} 0 0 {} 0 {} 0 0 0 0 0 0 {} {} {} 0 0 0 17 0 {} {} 0 0 0 0 0 0 0 0 0 0 {}",
                tid,
                name,
                state(source.view, process, tid).0,
                process.parent,
                process.group,
                process.session,
                terminal,
                foreground,
                ticks(used),
                ticks(children),
                priority,
                schedule.nice,
                thread_count(process),
                vsize,
                process.limits[RLIMIT_RSS as usize].current,
                // Only the first 31 signals, as Linux's `do_task_stat` shows them.
                blocked & 0x7fff_ffff,
                ignored & 0x7fff_ffff,
                caught & 0x7fff_ffff,
                schedule.priority,
                schedule.policy,
                // The wait status once ended (`exit_code`).
                process.ended.unwrap_or(0)
            );
        }
        ProcessFile::Statm => {
            let (vsize, _) = sizes(process);
            let _ = writeln!(out, "{} 0 0 0 0 0 0", vsize / 4096);
        }
        ProcessFile::Status => {
            let credentials = task_credentials(source.view.processes, source.view.threads, tid);
            let (vsize, _) = sizes(process);
            let state = state(source.view, process, tid);
            let ended = process.ended.is_some() || !source.view.threads.contains_key(&tid);
            // An ended thread has no file system context, descriptor table or memory, so its umask,
            // table size and memory lines are left out or empty (`task_state`, `task_mem`).
            let _ = writeln!(out, "Name:\t{}", String::from_utf8_lossy(&shown.name));
            if !ended && let Some(thread) = source.view.threads.get(&tid) {
                let _ = writeln!(out, "Umask:\t{:04o}", thread.fs.borrow().umask);
            }
            let _ = write!(
                out,
                "State:\t{} ({})\nTgid:\t{}\nNgid:\t0\nPid:\t{}\nPPid:\t{}\n\
                 TracerPid:\t0\nUid:\t{}\t{}\t{}\t{}\nGid:\t{}\t{}\t{}\t{}\nFDSize:\t{}\nGroups:\t",
                state.0,
                state.1,
                process.pid,
                tid,
                process.parent,
                credentials.uid,
                credentials.euid,
                credentials.suid,
                credentials.fsuid,
                credentials.gid,
                credentials.egid,
                credentials.sgid,
                credentials.fsgid,
                source
                    .view
                    .threads
                    .get(&tid)
                    .map_or(0, |thread| thread.files.borrow().capacity())
            );
            // Separated by spaces, then always one more (Linux's `task_state`).
            for (index, group) in credentials.groups.iter().enumerate() {
                let _ = write!(out, "{}{group}", if index > 0 { " " } else { "" });
            }
            out.push(' ');
            let (ignored, caught) = ignored_and_caught(process);
            out.push('\n');
            if !ended {
                let _ = writeln!(out, "VmSize:\t{:8} kB", vsize / 1024);
            }
            let _ = write!(
                out,
                "Threads:\t{}\nSigBlk:\t{:016x}\nSigIgn:\t{:016x}\nSigCgt:\t{:016x}\n\
                 CapInh:\t{:016x}\nCapPrm:\t{:016x}\nCapEff:\t{:016x}\nCapBnd:\t{:016x}\nCapAmb:\t{:016x}\n\
                 NoNewPrivs:\t{}\nSeccomp:\t2\nSeccomp_filters:\t{}\n",
                thread_count(process),
                shown.signal_mask,
                ignored,
                caught,
                credentials.inheritable,
                credentials.permitted,
                credentials.effective,
                credentials.bounding,
                credentials.ambient,
                u8::from(credentials.no_new_privileges),
                // Docker's profile, then the first thread's own.
                1 + shown.filters,
            );
        }
        ProcessFile::Mounts => out.push_str(&super::mounts::mounts(source.mounts)),
        ProcessFile::Mountinfo => out.push_str(&super::mounts::mountinfo(source.mounts)),
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

/// What the files show of thread `tid` of `process`; a first thread that has ended keeps it until
/// the process is reaped.
fn shown(view: &View<'_>, process: &Process, tid: Tid) -> crate::process::Leader {
    match view.threads.get(&tid) {
        Some(thread) => crate::process::Leader {
            schedule: thread.schedule,
            ..crate::process::Leader::of(thread)
        },
        None => process.ended_leader.clone(),
    }
}

/// A process's count of threads, its first among them until the process is reaped.
fn thread_count(process: &Process) -> usize {
    tasks(process).count()
}

/// The signals a process ignores and those it has handlers for, one bit each from bit 0 for signal 1
/// (Linux's `collect_sigign_sigcatch`).
fn ignored_and_caught(process: &Process) -> (u64, u64) {
    let (mut ignored, mut caught) = (0, 0);
    for (index, action) in process.actions.borrow().iter().enumerate() {
        match action.sa_handler_kernel {
            crate::linux_extra::SIG_IGN => ignored |= 1 << index,
            crate::linux_extra::SIG_DFL => {}
            _ => caught |= 1 << index,
        }
    }
    (ignored, caught)
}

/// The total mapped size of a process, and the part that is writable.
fn sizes(process: &Process) -> (u64, u64) {
    let mut total = 0;
    let mut writable = 0;
    for mapping in process.map.borrow().iter() {
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
