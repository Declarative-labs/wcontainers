//! Setting up the container: its root file system, the mounts a default Docker container has, and its
//! first process.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;

use wcontainers_abi::{Cpu, End, Host, PAGE_SIZE, Resume};

use crate::credentials::Credentials;
use crate::ends::Standard;
use crate::file::{FdTable, OpenFile};
use crate::fs::dev::DOCKER_DEVICES;
use crate::fs::mounts::Mount;
use crate::fs::{
    self, Content, Directory, FileSystem, Kind, New, Node, Numbers, Regular, Time, encode_device,
};
use crate::linux::general::*;
use crate::linux_extra::{ST_NODEV, ST_NOEXEC, ST_NOSUID, ST_RDONLY, ST_RELATIME};
use crate::loader::Image;
use crate::process::{Pid, Process, SIGNALS, Thread, Tid, docker_limits};
use crate::{Context, Errno, Kernel};

/// The permission bits, owner and time of a file added before the container starts.
#[derive(Clone, Copy, Debug)]
pub struct Attributes {
    /// Permission bits, with set-user, set-group and sticky bits.
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub mtime_seconds: i64,
}

/// A file added to the root file system before the container starts, as an image's layers hold it.
#[derive(Clone, Copy, Debug)]
pub enum NewFile<'a> {
    Directory,
    Regular(&'a [u8]),
    Symlink(&'a [u8]),
    /// Another name for the file at this path, which must already exist.
    HardLink(&'a [u8]),
    CharacterDevice {
        major: u32,
        minor: u32,
    },
    BlockDevice {
        major: u32,
        minor: u32,
    },
    Fifo,
}

/// How the container's first process starts, as `docker run` gives it.
#[derive(Clone, Copy, Debug)]
pub struct Start<'a> {
    pub path: &'a [u8],
    pub arguments: &'a [&'a [u8]],
    pub environment: &'a [&'a [u8]],
    pub working_directory: &'a [u8],
    pub hostname: &'a [u8],
    /// Supplementary groups of the first process.
    pub groups: &'a [u32],
    /// Where the file backend is mounted: `/` makes it the root file system, any other path a
    /// volume as `docker run -v` mounts one; none for a container without one, whose root is in
    /// memory.
    pub backend: Option<&'a [u8]>,
    /// What descriptors 0 to 2 are.
    pub standard: Standard,
}

/// Where the first process's descriptor 0 opens when it has no input.
const NULL_PATH: &[u8] = b"/dev/null";

/// The wait status of a container whose first program could not be started, with the exit status
/// `docker run` reports (measured): 127 for a program not found, 126 for one it may not run, 125
/// for anything else.
fn start_failed(error: Errno) -> i32 {
    let status = match error {
        Errno::ENOENT => 127,
        Errno::EACCES => 126,
        _ => 125,
    };
    status << 8
}

/// The container's start while it runs: what the first process starts with, and how far it got.
/// It runs again from where it got each time the backend answers, as a parked call does.
pub(crate) struct Starting {
    pid: Pid,
    path: Vec<u8>,
    arguments: Vec<Vec<u8>>,
    environment: Vec<Vec<u8>>,
    working_directory: Vec<u8>,
    standard: Standard,
    /// Whether the mounts are made.
    mounted: bool,
}

/// One file of the root file system as it is now, for comparing what a container left behind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRecord {
    pub path: Vec<u8>,
    /// Type and permission bits, as `st_mode`.
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    /// A regular file's bytes or a symbolic link's target.
    pub contents: Vec<u8>,
    /// For devices, the device number.
    pub rdev: u64,
}

/// The root of the container's file system: an empty in-memory directory.
pub(crate) fn new_root(numbers: &mut Numbers) -> Node {
    let mut fs = numbers.file_system(Kind::Tmpfs, ST_RELATIME);
    // Its limits are tmpfs's defaults for the machine, set when the container starts.
    fs.space = Some(fs::Space::new(0, 0));
    let ino = numbers.ino();
    let root = fs::new_inode(
        fs,
        ino,
        New {
            mode: S_IFDIR | 0o755,
            uid: 0,
            gid: 0,
            rdev: 0,
            now: Time::default(),
            content: Content::Directory(Directory::new()),
            parent: None,
            name: Vec::new(),
        },
    )
    .expect("a new container's file systems have room");
    root.borrow_mut().place.mount_root = true;
    root
}

/// Adds a name for `node` in `directory`, keeping the link counts right.
fn link(directory: &Node, name: &[u8], node: Node) {
    let is_directory = node.borrow().is_directory();
    if is_directory {
        directory.borrow_mut().nlink += 1;
    }
    directory
        .borrow_mut()
        .directory_mut()
        .expect("directory")
        .insert(name, node);
}

/// Makes a new inode in a directory of a tmpfs.
#[allow(
    clippy::too_many_arguments,
    reason = "each is one attribute of the new inode"
)]
pub(crate) fn make(
    numbers: &mut Numbers,
    directory: &Node,
    name: &[u8],
    mode: u32,
    uid: u32,
    gid: u32,
    rdev: u64,
    now: Time,
    content: Content,
) -> Result<Node, Errno> {
    let fs = directory.borrow().fs.clone();
    let ino = numbers.ino();
    let node = fs::new_inode(
        fs,
        ino,
        New {
            mode,
            uid,
            gid,
            rdev,
            now,
            content,
            parent: Some(directory.clone()),
            name: name.to_vec(),
        },
    )?;
    link(directory, name, node.clone());
    Ok(node)
}

/// A new empty in-memory file system, mounted on a directory.
fn mount_tmpfs(
    numbers: &mut Numbers,
    on: &Node,
    mode: u32,
    now: Time,
    kind: Kind,
    flags: u32,
    space: Option<Rc<fs::Space>>,
) -> Node {
    let mut fs = numbers.file_system(kind, flags);
    fs.space = space;
    let ino = numbers.ino();
    let root = fs::new_inode(
        fs,
        ino,
        New {
            mode: S_IFDIR | mode,
            uid: 0,
            gid: 0,
            rdev: 0,
            now,
            content: Content::Directory(Directory::new()),
            parent: None,
            name: Vec::new(),
        },
    )
    .expect("a new container's file systems have room");
    fs::mount(on, &root);
    root
}

fn fs_of(node: &Node) -> FileSystem {
    node.borrow().fs.clone()
}

/// The file backend's mount at `path`: a FUSE file system that checks permissions itself and lets
/// every user in (`fuse_show_options`).
fn backend_mount(fs: FileSystem, path: &[u8]) -> Mount {
    Mount {
        fs,
        root: "/",
        point: path.to_vec(),
        source: "wcontainers",
        kind: "fuse",
        options: "user_id=0,group_id=0,default_permissions,allow_other",
        read_only_super: false,
    }
}

/// The size Docker gives the tmpfs at `/dev` (runc's `size=65536k`) and at `/dev/shm` (its default
/// `--shm-size` of 64 MiB).
const DOCKER_TMPFS_BYTES: u64 = 64 << 20;

/// The directory at an absolute path in the root file system, made if missing.
fn directory_at(kernel: &mut Kernel, path: &[u8], now: Time) -> Node {
    let mut at = kernel.files.root.clone();
    for part in path
        .split(|byte| *byte == b'/')
        .filter(|part| !part.is_empty())
    {
        let existing = at
            .borrow()
            .directory()
            .and_then(|directory| directory.get(part));
        at = match existing {
            Some(node) => {
                let mounted = node.borrow().place.mounted.clone();
                mounted.unwrap_or(node)
            }
            None => make(
                &mut kernel.files.numbers,
                &at,
                part,
                S_IFDIR | 0o755,
                0,
                0,
                0,
                now,
                Content::Directory(Directory::new()),
            )
            .expect("a new container's file systems have room"),
        };
    }
    at
}

impl Kernel {
    /// Adds a file to the root file system before the container starts, as unpacking an image does.
    /// Directories above it must already exist. Adding a directory that exists sets its attributes.
    pub fn add_file(
        &mut self,
        path: &[u8],
        attributes: Attributes,
        file: NewFile<'_>,
    ) -> Result<(), Errno> {
        assert!(
            self.init.is_none(),
            "files are added before the container starts"
        );
        let mut parent = self.files.root.clone();
        let parts: Vec<&[u8]> = path
            .split(|byte| *byte == b'/')
            .filter(|part| !part.is_empty() && *part != b".")
            .collect();
        let time = Time {
            seconds: attributes.mtime_seconds,
            nanoseconds: 0,
        };
        let Some((name, above)) = parts.split_last() else {
            let mut root = parent.borrow_mut();
            root.mode = S_IFDIR | (attributes.mode & 0o7777);
            root.uid = attributes.uid;
            root.gid = attributes.gid;
            root.mtime = time;
            root.atime = time;
            root.ctime = time;
            return Ok(());
        };
        for part in above {
            let next = parent
                .borrow()
                .directory()
                .and_then(|directory| directory.get(part))
                .ok_or(Errno::ENOENT)?;
            if !next.borrow().is_directory() {
                return Err(Errno::ENOTDIR);
            }
            parent = next;
        }
        let existing = parent.borrow().directory().expect("directory").get(name);
        let permissions = attributes.mode & 0o7777;
        if let Some(existing) = &existing {
            let mut inode = existing.borrow_mut();
            if matches!(file, NewFile::Directory) && inode.is_directory() {
                inode.mode = S_IFDIR | permissions;
                inode.uid = attributes.uid;
                inode.gid = attributes.gid;
                inode.mtime = time;
                inode.atime = time;
                inode.ctime = time;
                return Ok(());
            }
            return Err(Errno::EEXIST);
        }
        let (mode, rdev, content) = match file {
            NewFile::Directory => (S_IFDIR, 0, Content::Directory(Directory::new())),
            NewFile::Regular(bytes) => (
                S_IFREG,
                0,
                Content::Regular(Regular::with_bytes(bytes.to_vec())),
            ),
            NewFile::Symlink(target) => (S_IFLNK, 0, Content::Symlink(target.to_vec())),
            NewFile::HardLink(target) => {
                let node = self.with_root_walk(|walk, root| walk.resolve(root, target, false))?;
                if node.borrow().is_directory() {
                    return Err(Errno::EPERM);
                }
                node.borrow_mut().nlink += 1;
                link(&parent, name, node);
                return Ok(());
            }
            NewFile::CharacterDevice { major, minor } => {
                (S_IFCHR, encode_device(major, minor), Content::Special)
            }
            NewFile::BlockDevice { major, minor } => {
                (S_IFBLK, encode_device(major, minor), Content::Special)
            }
            NewFile::Fifo => (S_IFIFO, 0, Content::Special),
        };
        let permissions = if mode == S_IFLNK { 0o777 } else { permissions };
        make(
            &mut self.files.numbers,
            &parent,
            name,
            mode | permissions,
            attributes.uid,
            attributes.gid,
            rdev,
            time,
            content,
        )
        .expect("a new container's file systems have room");
        Ok(())
    }

    /// A walk from the root as root, for setting up.
    fn with_root_walk<R>(
        &self,
        f: impl FnOnce(&fs::Walk<'_>, &Node) -> Result<R, Errno>,
    ) -> Result<R, Errno> {
        let credentials = Credentials::container_root(Vec::new());
        let view = fs::proc::View {
            processes: &self.processes,
            threads: &self.threads,
            caller: 0,
            caller_thread: 0,
            fs: self.files.proc_fs.clone(),
            nsfs: self.files.nsfs.clone(),
            boot: self.boot,
            kept: &self.files.proc_kept,
            counters: &self.counters,
        };
        let walk = fs::Walk {
            root: &self.files.root,
            credentials: &credentials,
            proc: &view,
            internal: core::cell::Cell::new(false),
        };
        f(&walk, &self.files.root)
    }

    /// Every file of the root file system, not crossing into other mounts, in path order.
    pub fn root_files(&self, cpu: &mut dyn Cpu) -> Vec<FileRecord> {
        let mut out = Vec::new();
        collect(cpu, &self.files.root, b"", &mut out);
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    /// Starts the container: makes the mounts a default Docker container has and runs its first
    /// process. With the root on the backend this waits for the backend's answers like any call,
    /// so it may still be going on when this returns; a start that fails ends the container, and
    /// [`Kernel::start_failure`] says why.
    pub fn start(&mut self, cpu: &mut dyn Cpu, host: &mut dyn Host, start: &Start<'_>) {
        assert!(self.init.is_none(), "the container has already started");
        let mut cx = Context {
            cpu,
            host,
            monotonic_offset: self.monotonic_offset,
            refused: 0,
        };
        let wall = cx.host.realtime_nanoseconds();
        self.boot = Time::from_nanoseconds(wall);
        self.boot_monotonic = cx.monotonic();
        self.entry_now = self.boot_monotonic;
        self.wall_offset = wall.wrapping_sub(self.boot_monotonic);
        cx.host.random(&mut self.boot_id);
        cx.host.random(&mut self.files.generation_key);
        self.machine.memory_bytes = cx.cpu.address_space().end;
        self.hostname = start.hostname.to_vec();
        for (end, sends) in start.standard.ends() {
            self.ends.name(end, sends);
        }
        if let Some(path) = start.backend {
            self.mount_backend(path);
        }

        let pid = self.allocate_id();
        let root = self.files.root.clone();
        let credentials = Credentials::container_root(start.groups.to_vec());
        let process = Process {
            pid,
            parent: 0,
            parent_thread: 0,
            group: pid,
            session: pid,
            memory: wcontainers_abi::MemoryId(0),
            map: Rc::new(core::cell::RefCell::new(crate::memory::AddressMap::new(
                cx.cpu.address_space().end,
            ))),
            threads: vec![pid],
            limits: docker_limits(),
            executable: (root.clone(), Vec::new()),
            starting_stack: Default::default(),
            child_subreaper: false,
            thp_disable: 0,
            pending: Default::default(),
            exit_signal: SIGCHLD,
            ended: None,
            ended_leader: Default::default(),
            nproc_exceeded: false,
            pidfd: None,
            stopped: false,
            report: None,
            continued_unsent: false,
            next_timer: 0,
            ended_time: 0,
            children_time: 0,
            children_maxrss: 0,
            peak_mapped: 0,
            started: cx.monotonic(),
            vdso: 0,
            execed: false,
            self_exec_id: 0,
            // What Docker's first process inherits (measured).
            coredump_filter: 0x23,
            terminal: None,
            terminal_old_group: None,
            actions: Rc::new(core::cell::RefCell::new(
                [kernel_sigaction {
                    sa_handler_kernel: crate::linux_extra::SIG_DFL,
                    sa_flags: 0,
                    sa_restorer: 0,
                    sa_mask: kernel_sigset_t { sig: [0] },
                }; SIGNALS],
            )),
        };
        self.processes.insert(pid, process);
        // Its CPU side thread is made once its program is loaded.
        let files = Rc::new(core::cell::RefCell::new(FdTable::new(pid as u64)));
        let fs = Rc::new(core::cell::RefCell::new(crate::process::FsContext {
            root: root.clone(),
            cwd: root.clone(),
            umask: 0o022,
        }));
        let thread = Thread::new(pid, wcontainers_abi::ThreadId(0), files, fs, credentials);
        self.threads.insert(pid, thread);
        let owned = |strings: &[&[u8]]| strings.iter().map(|string| string.to_vec()).collect();
        self.starting = Some(Box::new(Starting {
            pid,
            path: start.path.to_vec(),
            arguments: owned(start.arguments),
            environment: owned(start.environment),
            working_directory: start.working_directory.to_vec(),
            standard: start.standard,
            mounted: false,
        }));
        self.run_start(&mut cx);
        self.settle(&mut cx);
    }

    /// Why the container's start failed, once it has.
    pub fn start_failure(&self) -> Option<Errno> {
        self.start_failure
    }

    /// Whether `tid` is the first process's thread while the container starts.
    pub(crate) fn is_starting(&self, tid: Tid) -> bool {
        self.starting
            .as_ref()
            .is_some_and(|starting| starting.pid == tid)
    }

    /// Runs the container's start, the first time or again once the backend has answered: each
    /// part asks the backend as a call does, and a part done is not done again.
    pub(crate) fn run_start(&mut self, cx: &mut Context<'_>) {
        let Some(mut starting) = self.starting.take() else {
            return;
        };
        let pid = starting.pid;
        loop {
            self.begin_asking(pid);
            let result = if starting.mounted {
                self.start_first(cx, &starting).map(|()| true)
            } else {
                let points = self.directories(pid, b"/dev").and_then(|dev| {
                    let proc = self.directories(pid, b"/proc")?;
                    Ok((dev, proc, self.directories(pid, b"/sys")?))
                });
                points.map(|(dev, proc, sys)| {
                    self.make_mounts(&dev, &proc, &sys);
                    starting.mounted = true;
                    false
                })
            };
            self.end_asking(pid, result == Err(Errno::PARKED));
            match result {
                // The mounts are made: the first program's part asks afresh.
                Ok(false) => continue,
                Ok(true) => {}
                Err(Errno::PARKED) => self.starting = Some(starting),
                Err(error) => {
                    self.processes.remove(&pid);
                    self.threads.remove(&pid);
                    self.start_failure = Some(error);
                    self.finished = Some(start_failed(error));
                    self.close_unmade(cx, &starting.standard);
                }
            }
            return;
        }
    }

    /// The directory at `path`, with each missing part made as runc makes mount points and the
    /// working directory (`MkdirAll` with mode 0755).
    fn directories(&mut self, pid: Pid, path: &[u8]) -> Result<Node, Errno> {
        let mut at = self.fs(pid).cwd.clone();
        if path.starts_with(b"/") {
            at = self.fs(pid).root.clone();
        }
        for part in path
            .split(|byte| *byte == b'/')
            .filter(|part| !part.is_empty())
        {
            at = match self.with_walk(pid, |walk, _| walk.resolve(&at, part, true)) {
                Err(Errno::ENOENT) if crate::fs::backend::remote(&at).is_some() => {
                    crate::fs::backend::mkdir(&at, part, 0o755, 0)?
                }
                Err(Errno::ENOENT) => make(
                    &mut self.files.numbers,
                    &at,
                    part,
                    S_IFDIR | 0o755,
                    0,
                    0,
                    0,
                    self.boot,
                    Content::Directory(Directory::new()),
                )?,
                found => found?,
            };
        }
        Ok(at)
    }

    /// Starts the first process: its working directory, its program loaded, then its standard
    /// descriptors on the ends named at start, as runc sets them up.
    fn start_first(&mut self, cx: &mut Context<'_>, starting: &Starting) -> Result<(), Errno> {
        let pid = starting.pid;
        let cwd = self.directories(pid, &starting.working_directory)?;
        if !cwd.borrow().is_directory() {
            return Err(Errno::ENOTDIR);
        }
        self.fs_mut(pid).cwd = cwd;
        let null = match starting.standard {
            Standard::Pipes { input: None, .. } => Some(self.with_walk(pid, |walk, process| {
                walk.resolve(&process.cwd, NULL_PATH, true)
            })?),
            _ => None,
        };
        let program = crate::loader::open_program(self, cx.cpu, pid, &starting.path)?;
        // The strings in the order and room of Linux's `do_execveat_common`.
        let stack_limit = self.processes[&pid].limits[RLIMIT_STACK as usize].current;
        let mut room = crate::loader::ArgumentRoom::new(
            stack_limit,
            starting.arguments.len(),
            starting.environment.len(),
        )?;
        room.take(&starting.path)?;
        let environment = starting.environment.clone();
        let mut arguments = starting.arguments.clone();
        for string in environment.iter().chain(&arguments) {
            room.take(string)?;
        }
        room.first_argument(&mut arguments)?;
        let image = match crate::loader::load(
            self,
            cx,
            pid,
            &starting.path,
            program,
            arguments,
            environment,
            room,
        ) {
            Ok(image) => image,
            Err(crate::loader::Failed::Refused(error)) => return Err(error),
            // Past the point of no return the first process dies of SIGSEGV before it runs.
            Err(crate::loader::Failed::Fatal) => {
                self.processes.remove(&pid);
                self.threads.remove(&pid);
                self.init = Some(pid);
                self.finished = Some(SIGSEGV as i32);
                self.close_unmade(cx, &starting.standard);
                self.counters.flush(cx.host);
                return Ok(());
            }
        };
        self.standard_descriptors(cx, pid, starting.standard, null)?;
        let thread = match cx.cpu.thread_new(image.memory) {
            Ok(thread) => thread,
            Err(error) => {
                cx.cpu.memory_drop(image.memory);
                return Err(cx.cpu_error(error));
            }
        };
        self.thread_mut(pid).cpu = thread;
        self.install_image(cx, pid, pid, image);
        self.init = Some(pid);
        self.entry_now = cx.monotonic();
        self.make_runnable(pid, Resume::FromRegisters);
        Ok(())
    }

    /// Descriptors 0 to 2 of the first process on the ends named at start: pipes (with `/dev/null`
    /// as 0 when there is no input end) or a terminal's slave, the kernel holding the other sides.
    fn standard_descriptors(
        &mut self,
        cx: &mut Context<'_>,
        pid: Pid,
        standard: Standard,
        null: Option<Node>,
    ) -> Result<(), Errno> {
        let limit = u64::MAX;
        let mut descriptors = Vec::new();
        match standard {
            Standard::Pipes {
                input,
                output,
                error,
            } => {
                descriptors.push(match (input, null) {
                    (Some(end), _) => self.end_pipe(cx, end, O_RDONLY),
                    (None, Some(null)) => {
                        OpenFile::new(null, O_RDWR | O_LARGEFILE, NULL_PATH.to_vec(), 0)
                    }
                    (None, None) => {
                        unreachable!("the null device is found for a start with no input")
                    }
                });
                descriptors.push(self.end_pipe(cx, output, O_WRONLY));
                descriptors.push(self.end_pipe(cx, error, O_WRONLY));
            }
            Standard::Terminal { end, columns, rows } => {
                let size = winsize {
                    ws_row: rows,
                    ws_col: columns,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };
                let (master, slave) = crate::syscall::tty::start_terminal(self, cx, pid, size)?;
                self.attach_end(cx, end, master);
                descriptors.extend([slave.clone(), slave.clone(), slave]);
            }
        }
        let mut files = self.files_mut(pid);
        for file in descriptors {
            files
                .add(file, false, 0, limit)
                .expect("space for the first descriptors");
        }
        Ok(())
    }

    /// A pipe for one standard descriptor: the side the guest gets (opened with `flags`), the end
    /// holding the other.
    fn end_pipe(&mut self, cx: &mut Context<'_>, end: End, flags: u32) -> crate::file::FileRef {
        let now = self.now(cx);
        let ino = self.files.numbers.ino();
        let pipe = fs::new_inode(
            self.files.pipe_fs.clone(),
            ino,
            New {
                mode: crate::syscall::pipe::MODE,
                uid: crate::credentials::ROOT_UID,
                gid: crate::credentials::ROOT_GID,
                rdev: 0,
                now,
                content: Content::Pipe(crate::pipe::Pipe::new()),
                parent: None,
                name: Vec::new(),
            },
        )
        .expect("a new container's file systems have space");
        let other = if flags == O_RDONLY {
            O_WRONLY
        } else {
            O_RDONLY
        };
        let guest = OpenFile::new(pipe.clone(), flags, Vec::new(), 0);
        let held = OpenFile::new(pipe.clone(), other, Vec::new(), 0);
        crate::syscall::pipe::attach(&pipe, &guest);
        crate::syscall::pipe::attach(&pipe, &held);
        self.attach_end(cx, end, held);
        guest
    }

    /// Replaces a process's program with a loaded image, as the last part of `execve` does.
    pub(crate) fn install_image(&mut self, cx: &mut Context<'_>, pid: Pid, tid: Tid, image: Image) {
        let thread = self.threads.get_mut(&tid).expect("thread");
        let process = self.processes.get_mut(&pid).expect("process");
        let old_memory = core::mem::replace(&mut process.memory, image.memory);
        process.map = Rc::new(core::cell::RefCell::new(image.map));
        process.executable = image.executable;
        process.starting_stack = image.starting_stack;
        process.vdso = image.vdso;
        process.execed = true;
        process.self_exec_id += 1;
        // Dumpable unless a file could not be read or the ids differ, before or after (Linux's
        // `begin_new_exec` and `commit_creds`).
        let old = &thread.credentials;
        let new = &image.after.credentials;
        let lose_parent_death_signal = image.after.secure || new.loses_dumpable(old);
        process.map.borrow_mut().dumpable = image.readable
            && old.euid == old.uid
            && old.egid == old.gid
            && !new.loses_dumpable(old);
        if image.after.gained {
            thread.personality &= !crate::linux_extra::PER_CLEAR_ON_SETID;
        }
        // A program in secure mode starts with at most Linux's default stack limit (`_STK_LIM`).
        let stack = &mut process.limits[RLIMIT_STACK as usize];
        if image.after.secure && stack.current > crate::memory::STACK_LIMIT {
            stack.current = crate::memory::STACK_LIMIT;
        }
        thread.credentials = image.after.credentials;
        // New credentials (`commit_creds` in `begin_new_exec`).
        self.last_credentials += 1;
        thread.credentials.serial = self.last_credentials;
        // Handlers shared with another process are copied first (`unshare_sighand`).
        let actions = *process.actions.borrow();
        process.actions = Rc::new(core::cell::RefCell::new(actions));
        for action in process.actions.borrow_mut().iter_mut() {
            if action.sa_handler_kernel != crate::linux_extra::SIG_IGN {
                action.sa_handler_kernel = crate::linux_extra::SIG_DFL;
            }
            action.sa_flags = 0;
            action.sa_restorer = 0;
            action.sa_mask = kernel_sigset_t { sig: [0] };
        }
        // The program's descriptor table is its own (`unshare_files`), less what closes on exec.
        self.unshare_files(tid);
        self.files_mut(tid).close_on_exec(pid);
        let thread = self.threads.get_mut(&tid).expect("thread");
        thread.set_name(&image.name);
        // The new program has not registered its area. A program in secure mode, or one whose ids
        // change, cannot be signalled by its parent's end.
        thread.rseq = None;
        if lose_parent_death_signal {
            thread.parent_death_signal = 0;
        }
        if old_memory.0 != 0 {
            cx.cpu.thread_drop(thread.cpu);
            thread.cpu = cx
                .cpu
                .thread_new(image.memory)
                .expect("a thread on the new memory");
        }
        thread.clear_child_tid = 0;
        thread.robust_list = (0, 0);
        thread.alternate_stack = (0, 0, 0);
        cx.cpu.set_registers(thread.cpu, &image.registers);
        thread.resume = Resume::FromRegisters;
        // A `vfork` child gives its parent's memory back; any other old memory goes.
        self.release_vfork(tid);
        if old_memory.0 != 0 && !self.memory_in_use_elsewhere(pid, old_memory) {
            cx.cpu.memory_drop(old_memory);
        }
        self.files_released(cx);
        crate::shared::release(self, cx);
    }

    /// A tmpfs's count, kept for the machine's memory in use.
    fn new_space(&mut self, max_blocks: u64, max_inodes: u64) -> Rc<fs::Space> {
        let space = fs::Space::new(max_blocks, max_inodes);
        self.files.spaces.push(space.clone());
        space
    }

    /// Mounts the file backend at `path`: as the root file system at `/`, with the root's options
    /// in Docker (`relatime`); elsewhere as Linux mounts a FUSE file system, with no set-user bits,
    /// devices or access times (the reference's mount options).
    fn mount_backend(&mut self, path: &[u8]) {
        use crate::linux_extra::{ST_NOATIME, ST_NODEV, ST_NOSUID};
        let is_root = path.iter().all(|byte| *byte == b'/');
        let flags = if is_root {
            ST_RELATIME
        } else {
            ST_NOSUID | ST_NODEV | ST_NOATIME
        };
        let fs = self.files.numbers.file_system(Kind::Fuse, flags);
        let client = crate::fs::backend::Client::new(fs);
        let root = client.root();
        if is_root {
            root.borrow_mut().place.mount_root = true;
            self.files.root = root;
        } else {
            let point = directory_at(self, path, self.boot);
            fs::mount(&point, &root);
            self.files.mounts.push(backend_mount(fs_of(&root), path));
        }
        self.files.backend = Some(client);
    }

    /// The mounts and device files `docker run` sets up, on the directories `/dev`, `/proc` and
    /// `/sys`.
    fn make_mounts(&mut self, dev_point: &Node, proc_point: &Node, sys_point: &Node) {
        let now = self.boot;
        // tmpfs's default limits are half the machine's pages, in pages and in inodes
        // (`shmem_default_max_blocks`, `shmem_default_max_inodes`); Docker mounts `/dev` and
        // `/dev/shm` with 64 MiB.
        let half = self.machine.memory_bytes / PAGE_SIZE / 2;
        let docker_size = DOCKER_TMPFS_BYTES / PAGE_SIZE;
        if let Some(space) = &self.files.root.borrow().fs.space {
            space.set_limits(half, half);
        }
        // Mount points are held while mounted, as Linux holds their dentries.
        self.files
            .mount_points
            .extend([dev_point.clone(), proc_point.clone(), sys_point.clone()]);
        let dev_space = self.new_space(docker_size, half);
        let dev = mount_tmpfs(
            &mut self.files.numbers,
            dev_point,
            0o755,
            now,
            Kind::Tmpfs,
            ST_NOSUID,
            Some(dev_space),
        );
        // In runc's order, which a listing of `/dev` shows (newest first): the mount points in the
        // order of Docker's mounts, the devices, the `ptmx` link, then the standard links.
        for (name, mode) in [(&b"pts"[..], 0o755), (b"mqueue", 0o1777), (b"shm", 0o1777)] {
            make(
                &mut self.files.numbers,
                &dev,
                name,
                S_IFDIR | mode,
                0,
                0,
                0,
                now,
                Content::Directory(Directory::new()),
            )
            .expect("a new container's file systems have room");
        }
        for (name, major, minor) in DOCKER_DEVICES {
            make(
                &mut self.files.numbers,
                &dev,
                name,
                S_IFCHR | 0o666,
                0,
                0,
                encode_device(*major, *minor),
                now,
                Content::Special,
            )
            .expect("a new container's file systems have room");
        }
        let links: [(&[u8], &[u8]); 6] = [
            (b"ptmx", b"pts/ptmx"),
            (b"fd", b"/proc/self/fd"),
            (b"stdin", b"/proc/self/fd/0"),
            (b"stdout", b"/proc/self/fd/1"),
            (b"stderr", b"/proc/self/fd/2"),
            (b"core", b"/proc/kcore"),
        ];
        for (name, target) in links {
            make(
                &mut self.files.numbers,
                &dev,
                name,
                S_IFLNK | 0o777,
                0,
                0,
                0,
                now,
                Content::Symlink(target.to_vec()),
            )
            .expect("a new container's file systems have room");
        }
        let shm_point = dev
            .borrow()
            .directory()
            .expect("dev")
            .get(b"shm")
            .expect("shm");
        let shm_space = self.new_space(docker_size, half);
        let shm = mount_tmpfs(
            &mut self.files.numbers,
            &shm_point,
            0o1777,
            now,
            Kind::Tmpfs,
            ST_NOSUID | ST_NODEV | ST_NOEXEC | ST_RELATIME,
            Some(shm_space),
        );
        let mqueue_point = dev
            .borrow()
            .directory()
            .expect("dev")
            .get(b"mqueue")
            .expect("mqueue");
        let mqueue = mount_tmpfs(
            &mut self.files.numbers,
            &mqueue_point,
            0o1777,
            now,
            Kind::Mqueue,
            ST_NOSUID | ST_NODEV | ST_NOEXEC | ST_RELATIME,
            None,
        );
        let mqueue_fs = fs_of(&mqueue);
        self.ipc.mqueue_root = Some(mqueue);
        let pts_point = dev
            .borrow()
            .directory()
            .expect("dev")
            .get(b"pts")
            .expect("pts");
        let pts = mount_tmpfs(
            &mut self.files.numbers,
            &pts_point,
            0o755,
            now,
            Kind::Devpts,
            ST_NOSUID | ST_NOEXEC | ST_RELATIME,
            None,
        );
        let ptmx = make(
            &mut self.files.numbers,
            &pts,
            b"ptmx",
            S_IFCHR | 0o666,
            0,
            0,
            encode_device(5, 2),
            now,
            Content::Special,
        )
        .expect("a new container's file systems have room");
        // devpts numbers its root 1 and `ptmx` 2 (`devpts_fill_super`), each on its own device.
        pts.borrow_mut().ino = 1;
        ptmx.borrow_mut().ino = 2;

        // sysfs, read-only as in a container without CAP_SYS_ADMIN, its root numbered 1.
        let sys = mount_tmpfs(
            &mut self.files.numbers,
            sys_point,
            0o555,
            now,
            Kind::Sysfs,
            ST_RDONLY | ST_NOSUID | ST_NODEV | ST_NOEXEC | ST_RELATIME,
            None,
        );
        sys.borrow_mut().ino = 1;

        let view = fs::proc::View {
            processes: &self.processes,
            threads: &self.threads,
            caller: 0,
            caller_thread: 0,
            fs: self.files.proc_fs.clone(),
            nsfs: self.files.nsfs.clone(),
            boot: self.boot,
            kept: &self.files.proc_kept,
            counters: &self.counters,
        };
        let proc_root = fs::proc::root(&view);
        fs::mount(proc_point, &proc_root);

        // Docker's order: the root, `/proc`, `/dev` and the mounts in it, the volumes (mounted
        // already), then `/proc/sys` again, read-only.
        let mount = |fs: FileSystem, point: &str, source, kind, options| Mount {
            fs,
            root: "/",
            point: point.as_bytes().to_vec(),
            source,
            kind,
            options,
            read_only_super: false,
        };
        let root_fs = fs_of(&self.files.root);
        let root = if root_fs.kind == Kind::Fuse {
            backend_mount(root_fs, b"/")
        } else {
            mount(root_fs, "/", "tmpfs", "tmpfs", "")
        };
        let volumes = core::mem::take(&mut self.files.mounts);
        self.files.mounts = alloc::vec![
            root,
            mount(proc_root.borrow().fs.clone(), "/proc", "proc", "proc", ""),
            mount(
                fs_of(&dev),
                "/dev",
                "tmpfs",
                "tmpfs",
                "size=65536k,mode=755"
            ),
            mount(
                fs_of(&pts),
                "/dev/pts",
                "devpts",
                "devpts",
                "gid=5,mode=620,ptmxmode=666",
            ),
            Mount {
                read_only_super: true,
                ..mount(fs_of(&sys), "/sys", "sysfs", "sysfs", "")
            },
            mount(mqueue_fs, "/dev/mqueue", "mqueue", "mqueue", ""),
            mount(fs_of(&shm), "/dev/shm", "shm", "tmpfs", "size=65536k"),
        ];
        self.files.mounts.extend(volumes);
        self.files.mounts.push(Mount {
            root: "/sys",
            ..mount(
                FileSystem {
                    flags: self.files.proc_fs.flags | crate::linux_extra::ST_RDONLY,
                    ..self.files.proc_fs.clone()
                },
                "/proc/sys",
                "proc",
                "proc",
                "",
            )
        });
    }
}

fn collect(cpu: &mut dyn Cpu, directory: &Node, path: &[u8], out: &mut Vec<FileRecord>) {
    let inode = directory.borrow();
    let Some(entries) = inode.directory() else {
        return;
    };
    for (name, node) in entries.iter() {
        let mut child_path = path.to_vec();
        child_path.push(b'/');
        child_path.extend_from_slice(name);
        let child = node.borrow();
        let contents = match &child.content {
            Content::Regular(file) => file.bytes(cpu),
            Content::Symlink(target) => target.clone(),
            _ => Vec::new(),
        };
        out.push(FileRecord {
            path: child_path.clone(),
            mode: child.mode,
            uid: child.uid,
            gid: child.gid,
            contents,
            rdev: child.rdev,
        });
        let is_mount_point = child.place.mounted.is_some();
        let is_directory = child.is_directory();
        drop(child);
        if is_directory && !is_mount_point && !Rc::ptr_eq(node, directory) {
            collect(cpu, node, &child_path, out);
        }
    }
}
