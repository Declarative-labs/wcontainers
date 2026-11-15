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
use crate::process::{Pid, Process, Thread, Tid};

/// What a walk through `/proc` needs to know about the kernel.
pub(crate) struct View<'a> {
    pub processes: &'a BTreeMap<Pid, Process>,
    pub threads: &'a BTreeMap<Tid, Thread>,
    /// The process walking, which `/proc/self` names.
    pub caller: Pid,
    pub fs: FileSystem,
    pub boot: Time,
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
    File,
    /// A symbolic link to a path below `/proc`.
    Link(&'static str),
}

pub(crate) const GLOBAL: &[(&str, Global)] = &[
    ("cpuinfo", Global::File),
    ("filesystems", Global::File),
    ("loadavg", Global::File),
    ("meminfo", Global::File),
    ("mounts", Global::Link("self/mounts")),
    ("stat", Global::File),
    ("sys", Global::Directory),
    ("sys/abi", Global::Directory),
    ("sys/debug", Global::Directory),
    ("sys/dev", Global::Directory),
    ("sys/fs", Global::Directory),
    ("sys/fs/nr_open", Global::File),
    ("sys/kernel", Global::Directory),
    ("sys/kernel/cap_last_cap", Global::File),
    ("sys/kernel/hostname", Global::File),
    ("sys/kernel/ngroups_max", Global::File),
    ("sys/kernel/osrelease", Global::File),
    ("sys/kernel/ostype", Global::File),
    ("sys/kernel/pid_max", Global::File),
    ("sys/kernel/random", Global::Directory),
    ("sys/kernel/random/boot_id", Global::File),
    ("sys/kernel/random/uuid", Global::File),
    ("sys/net", Global::Directory),
    ("sys/sunrpc", Global::Directory),
    ("sys/user", Global::Directory),
    ("sys/vm", Global::Directory),
    ("sys/vm/overcommit_memory", Global::File),
    ("uptime", Global::File),
    ("version", Global::File),
];

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

    /// Proc files report no size.
    pub fn size(&self) -> u64 {
        0
    }

    fn mode(&self) -> u32 {
        match self {
            Entry::Root | Entry::Process(_) => S_IFDIR | 0o555,
            Entry::Fds(_) => S_IFDIR | 0o500,
            Entry::Global(index) => match GLOBAL[*index].1 {
                Global::Directory => S_IFDIR | 0o555,
                Global::File => S_IFREG | 0o444,
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
        // Linux's `task_dump_owner`: the process's effective ids, except that what is not a world
        // readable directory belongs to root while the process may not be dumped.
        view.processes.get(&pid).map_or((0, 0), |process| {
            if self.mode() != S_IFDIR | 0o555 && !process.dumpable {
                (0, 0)
            } else {
                (process.credentials.euid, process.credentials.egid)
            }
        })
    }
}

/// Makes the inode for an entry found in `parent` under `name`.
pub(crate) fn node(view: &View<'_>, parent: Option<&Node>, name: &[u8], entry: Entry) -> Node {
    let (uid, gid) = entry.owner(view);
    let mut mode = entry.mode();
    // A descriptor's link shows how the file was opened.
    if let Entry::Fd(pid, fd) = entry
        && let Some(file) = view
            .processes
            .get(&pid)
            .and_then(|process| process.files.file(fd).ok())
    {
        let file = file.borrow();
        mode = S_IFLNK
            | 0o100
            | if file.readable() { 0o400 } else { 0 }
            | if file.writable() { 0o200 } else { 0 };
    }
    let is_directory = mode & S_IFMT == S_IFDIR;
    // Docker mounts `/proc/sys` again over itself, read-only.
    let fs = match entry {
        Entry::Global(index) if GLOBAL[index].0 == "sys" || GLOBAL[index].0.starts_with("sys/") => {
            FileSystem {
                flags: view.fs.flags | crate::linux_extra::ST_RDONLY,
                ..view.fs
            }
        }
        _ => view.fs,
    };
    let node = new_inode(
        fs,
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
    );
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
        .find(|(child, _)| child.as_slice() == name)
        .map(|(_, child)| child);
    // Process directories that are not listed (threads) can still be looked up.
    let found = found.or_else(|| match entry {
        Entry::Root => parse_number(name)
            .and_then(|pid| i32::try_from(pid).ok())
            .filter(|pid| view.processes.contains_key(pid))
            .map(Entry::Process),
        _ => None,
    });
    let child = found.ok_or(Errno::ENOENT)?;
    Ok(node(view, Some(directory), name, child))
}

/// The entries of a directory of the tree, in the order `getdents` lists them.
pub(crate) fn list(view: &View<'_>, entry: &Entry) -> Vec<(Vec<u8>, Entry)> {
    let mut out = Vec::new();
    match entry {
        Entry::Root => {
            for (index, name) in global_children("") {
                out.push((name.as_bytes().to_vec(), Entry::Global(index)));
            }
            out.push((b"self".to_vec(), Entry::SelfLink));
            for pid in view.processes.keys() {
                out.push((format!("{pid}").into_bytes(), Entry::Process(*pid)));
            }
        }
        Entry::Process(pid) => {
            for (name, file) in PROCESS_FILES {
                out.push((name.to_vec(), Entry::ProcessFile(*pid, *file)));
            }
            for (name, link) in PROCESS_LINKS {
                out.push((name.to_vec(), Entry::ProcessLink(*pid, *link)));
            }
            out.push((b"fd".to_vec(), Entry::Fds(*pid)));
            out.sort_by(|a, b| a.0.cmp(&b.0));
        }
        Entry::Fds(pid) => {
            if let Some(process) = view.processes.get(pid) {
                for (fd, _) in process.files.iter() {
                    out.push((format!("{fd}").into_bytes(), Entry::Fd(*pid, fd)));
                }
            }
        }
        Entry::Global(index) => {
            let (path, _) = GLOBAL[*index];
            for (child, name) in global_children(path) {
                out.push((name.as_bytes().to_vec(), Entry::Global(child)));
            }
        }
        _ => {}
    }
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
                Content::HostStream(_) => format!("pipe:[{}]", inode.ino).into_bytes(),
                Content::Directory(_) | Content::Proc(_) if inode.is_directory() => {
                    drop(inode);
                    walk.path_of(&file.node)
                        .unwrap_or_else(|| file.path.clone())
                }
                _ if inode.nlink == 0 => {
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
                    let _ = write!(
                        out,
                        "MemTotal:       {total:8} kB\nMemFree:        {total:8} kB\n\
                         MemAvailable:   {total:8} kB\nBuffers:               0 kB\n\
                         Cached:                0 kB\nSwapTotal:             0 kB\nSwapFree:              0 kB\n"
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
                "sys/vm/overcommit_memory" => out.push_str("0\n"),
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
}

fn process_file(
    source: &mut Source<'_>,
    process: &Process,
    file: ProcessFile,
) -> Result<Vec<u8>, Errno> {
    let mut out = String::new();
    let stack = process.starting_stack;
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
        ProcessFile::Auxv => {
            let (start, length) = range(stack.auxiliary);
            return Ok((source.read_memory)(process, start, length));
        }
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
                let (offset, ino, path) = match &mapping.backing {
                    Backing::File {
                        node, path, offset, ..
                    } => (
                        *offset,
                        node.borrow().ino,
                        String::from_utf8_lossy(path).into_owned(),
                    ),
                    Backing::Heap => (0, 0, String::from("[heap]")),
                    Backing::Stack => (0, 0, String::from("[stack]")),
                    Backing::Anonymous => (0, 0, String::new()),
                };
                let line = format!(
                    "{:08x}-{:08x} {}{}{}{} {:08x} 00:00 {}",
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
            let _ = writeln!(
                out,
                "{} ({}) R {} {} {} 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 {} 0 0 {} 0 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0 0",
                process.pid,
                name,
                process.parent,
                process.group,
                process.session,
                process.threads.len(),
                vsize
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
                "Name:\t{}\nUmask:\t{:04o}\nState:\tR (running)\nTgid:\t{}\nNgid:\t0\nPid:\t{}\nPPid:\t{}\n\
                 TracerPid:\t0\nUid:\t{}\t{}\t{}\t{}\nGid:\t{}\t{}\t{}\t{}\nFDSize:\t64\nGroups:\t",
                String::from_utf8_lossy(&process.name),
                process.umask,
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
                credentials.fsgid
            );
            for group in &credentials.groups {
                let _ = write!(out, "{group} ");
            }
            let _ = write!(
                out,
                "\nVmSize:\t{:8} kB\nThreads:\t{}\nSigBlk:\t0000000000000000\n\
                 CapInh:\t{:016x}\nCapPrm:\t{:016x}\nCapEff:\t{:016x}\nCapBnd:\t{:016x}\nCapAmb:\t{:016x}\n\
                 NoNewPrivs:\t{}\nSeccomp:\t2\nSeccomp_filters:\t{}\n",
                vsize / 1024,
                process.threads.len(),
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
