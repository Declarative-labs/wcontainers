//! Setting up the container: its root file system, the mounts a default Docker container has, and its
//! first process.

use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;

use wcontainers_abi::{Cpu, Host, PAGE_SIZE, Resume, Stream};

use crate::credentials::Credentials;
use crate::file::{FdTable, OpenFile};
use crate::fs::dev::DOCKER_DEVICES;
use crate::fs::{self, Content, Directory, Kind, New, Node, Numbers, Regular, Time, encode_device};
use crate::linux::general::*;
use crate::linux_extra::{ST_NODEV, ST_NOEXEC, ST_NOSUID, ST_RELATIME};
use crate::loader::Image;
use crate::process::{Pid, Process, SIGNALS, Thread, Tid, docker_limits};
use crate::{Context, Errno, Kernel, errno};

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
            fs: self.files.proc_fs.clone(),
            boot: self.boot,
            kept: &self.files.proc_kept,
            counters: &self.counters,
        };
        let walk = fs::Walk {
            root: &self.files.root,
            credentials: &credentials,
            proc: &view,
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
    /// process.
    pub fn start(
        &mut self,
        cpu: &mut dyn Cpu,
        host: &mut dyn Host,
        start: &Start<'_>,
    ) -> Result<(), Errno> {
        assert!(self.init.is_none(), "the container has already started");
        let mut cx = Context { cpu, host };
        self.boot = Time::from_nanoseconds(cx.host.realtime_nanoseconds());
        self.boot_monotonic = cx.host.monotonic_nanoseconds();
        cx.host.random(&mut self.boot_id);
        cx.host.random(&mut self.files.generation_key);
        self.machine.memory_bytes = cx.cpu.address_space().end;
        self.hostname = start.hostname.to_vec();
        self.make_mounts();

        let pid = self.allocate_id();
        let root = self.files.root.clone();
        let credentials = Credentials::container_root(start.groups.to_vec());
        let process = Process {
            pid,
            parent: 0,
            group: pid,
            session: pid,
            memory: wcontainers_abi::MemoryId(0),
            map: crate::memory::AddressMap::new(cx.cpu.address_space().end),
            threads: vec![pid],
            files: FdTable::new(pid as u64),
            root: root.clone(),
            cwd: root.clone(),
            umask: 0o022,
            credentials,
            limits: docker_limits(),
            name: Vec::new(),
            executable: (root.clone(), Vec::new()),
            starting_stack: Default::default(),
            dumpable: true,
            personality: 0,
            child_subreaper: false,
            thp_disable: 0,
            pending: Default::default(),
            exit_signal: SIGCHLD,
            ended: None,
            vfork_parent: None,
            stopped: false,
            report: None,
            ended_time: 0,
            children_time: 0,
            children_maxrss: 0,
            peak_mapped: 0,
            started: cx.host.monotonic_nanoseconds(),
            vdso: 0,
            execed: false,
            terminal: None,
            terminal_old_group: None,
            actions: [kernel_sigaction {
                sa_handler_kernel: crate::linux_extra::SIG_DFL,
                sa_flags: 0,
                sa_restorer: 0,
                sa_mask: kernel_sigset_t { sig: [0] },
            }; SIGNALS],
        };
        self.processes.insert(pid, process);
        let result = self.start_first(&mut cx, pid, start);
        if result.is_err() {
            self.processes.remove(&pid);
        }
        result
    }

    fn start_first(
        &mut self,
        cx: &mut Context<'_>,
        pid: Pid,
        start: &Start<'_>,
    ) -> Result<(), Errno> {
        let cwd = self.with_walk(pid, |walk, process| {
            walk.resolve(&process.cwd, start.working_directory, true)
        })?;
        if !cwd.borrow().is_directory() {
            return Err(Errno::ENOTDIR);
        }
        let null = self.with_walk(pid, |walk, process| {
            walk.resolve(&process.cwd, b"/dev/null", true)
        })?;
        let now = self.now(cx);
        let pipes: Vec<Node> = [Stream::Output, Stream::Error]
            .into_iter()
            .map(|stream| {
                let ino = self.files.numbers.ino();
                fs::new_inode(
                    self.files.pipe_fs.clone(),
                    ino,
                    New {
                        mode: S_IFIFO | 0o600,
                        uid: 0,
                        gid: 0,
                        rdev: 0,
                        now,
                        content: Content::Pipe(crate::pipe::Pipe::new(Some(stream))),
                        parent: None,
                        name: Vec::new(),
                    },
                )
                .expect("a new container's file systems have room")
            })
            .collect();
        let limit = u64::MAX;
        let process = self.processes.get_mut(&pid).expect("process");
        process.cwd = cwd;
        process.files.add(
            OpenFile::new(null, O_RDWR | O_LARGEFILE, b"/dev/null".to_vec(), 0),
            false,
            0,
            limit,
        )?;
        for pipe in pipes {
            process.files.add(
                OpenFile::new(pipe, O_WRONLY, Vec::new(), 0),
                false,
                0,
                limit,
            )?;
        }
        let program = crate::loader::open_program(self, cx.cpu, pid, start.path)?;
        // The strings in the order and room of Linux's `do_execveat_common`.
        let stack_limit = self.processes[&pid].limits[RLIMIT_STACK as usize].current;
        let mut room = crate::loader::ArgumentRoom::new(
            stack_limit,
            start.arguments.len(),
            start.environment.len(),
        )?;
        room.take(start.path)?;
        let environment: Vec<Vec<u8>> = start
            .environment
            .iter()
            .map(|variable| variable.to_vec())
            .collect();
        let mut arguments: Vec<Vec<u8>> = start
            .arguments
            .iter()
            .map(|argument| argument.to_vec())
            .collect();
        for string in environment.iter().chain(&arguments) {
            room.take(string)?;
        }
        room.first_argument(&mut arguments)?;
        let image = match crate::loader::load(
            self,
            cx,
            pid,
            start.path,
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
                self.init = Some(pid);
                self.finished = Some(SIGSEGV as i32);
                self.counters.flush(cx.host);
                return Ok(());
            }
        };
        let thread = cx.cpu.thread_new(image.memory).map_err(errno::from_cpu)?;
        self.threads.insert(pid, Thread::new(pid, thread));
        self.install_image(cx, pid, pid, image);
        self.init = Some(pid);
        self.entry_now = cx.host.monotonic_nanoseconds();
        self.make_runnable(pid, Resume::FromRegisters);
        self.settle(cx);
        Ok(())
    }

    /// Replaces a process's program with a loaded image, as the last part of `execve` does.
    pub(crate) fn install_image(&mut self, cx: &mut Context<'_>, pid: Pid, tid: Tid, image: Image) {
        let process = self.processes.get_mut(&pid).expect("process");
        let old_memory = core::mem::replace(&mut process.memory, image.memory);
        process.map = image.map;
        process.executable = image.executable;
        process.starting_stack = image.starting_stack;
        process.vdso = image.vdso;
        process.execed = true;
        process.set_name(&image.name);
        // Dumpable unless a file could not be read or the ids differ, before or after (Linux's
        // `begin_new_exec` and `commit_creds`).
        let old = &process.credentials;
        let new = &image.after.credentials;
        let lose_parent_death_signal = image.after.secure || new.loses_dumpable(old);
        process.dumpable = image.readable
            && old.euid == old.uid
            && old.egid == old.gid
            && !new.loses_dumpable(old);
        if image.after.gained {
            process.personality &= !crate::linux_extra::PER_CLEAR_ON_SETID;
        }
        // A program in secure mode starts with at most Linux's default stack limit (`_STK_LIM`).
        let stack = &mut process.limits[RLIMIT_STACK as usize];
        if image.after.secure && stack.current > crate::memory::STACK_LIMIT {
            stack.current = crate::memory::STACK_LIMIT;
        }
        process.credentials = image.after.credentials;
        process.files.close_on_exec();
        // New credentials (`commit_creds` in `begin_new_exec`).
        self.last_credentials += 1;
        process.credentials.serial = self.last_credentials;
        for action in &mut process.actions {
            if action.sa_handler_kernel != crate::linux_extra::SIG_IGN {
                action.sa_handler_kernel = crate::linux_extra::SIG_DFL;
            }
            action.sa_flags = 0;
            action.sa_restorer = 0;
            action.sa_mask = kernel_sigset_t { sig: [0] };
        }
        let thread = self.threads.get_mut(&tid).expect("thread");
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
        self.release_vfork(pid);
        let shown_elsewhere = self
            .processes
            .values()
            .any(|other| other.pid != pid && other.ended.is_none() && other.memory == old_memory);
        if old_memory.0 != 0 && !shown_elsewhere {
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

    /// The mounts and device files `docker run` sets up.
    fn make_mounts(&mut self) {
        let now = self.boot;
        // tmpfs's default limits are half the machine's pages, in pages and in inodes
        // (`shmem_default_max_blocks`, `shmem_default_max_inodes`); Docker mounts `/dev` and
        // `/dev/shm` with 64 MiB.
        let half = self.machine.memory_bytes / PAGE_SIZE / 2;
        let docker_size = DOCKER_TMPFS_BYTES / PAGE_SIZE;
        if let Some(space) = &self.files.root.borrow().fs.space {
            space.set_limits(half, half);
        }
        let dev_point = directory_at(self, b"/dev", now);
        let dev_space = self.new_space(docker_size, half);
        let dev = mount_tmpfs(
            &mut self.files.numbers,
            &dev_point,
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
        mount_tmpfs(
            &mut self.files.numbers,
            &shm_point,
            0o1777,
            now,
            Kind::Tmpfs,
            ST_NOSUID | ST_NODEV | ST_NOEXEC | ST_RELATIME,
            Some(shm_space),
        );
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

        let proc_point = directory_at(self, b"/proc", now);
        let view = fs::proc::View {
            processes: &self.processes,
            threads: &self.threads,
            caller: 0,
            fs: self.files.proc_fs.clone(),
            boot: self.boot,
            kept: &self.files.proc_kept,
            counters: &self.counters,
        };
        let proc_root = fs::proc::root(&view);
        fs::mount(&proc_point, &proc_root);
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
