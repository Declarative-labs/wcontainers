//! Core dumps (`do_coredump` with `elf_core_dump`): a process killed by a signal whose default
//! action dumps core writes an ELF core file named `core` in its working directory, as the
//! reference's `core_pattern` "core" with `core_uses_pid` 0 names it, and its wait status gets
//! the core bit. The writing thread runs the dump in stages that may wait for the backend, as a
//! call does; the process's other threads wait meanwhile and end with it.

use alloc::vec;
use alloc::vec::Vec;
use wcontainers_abi::NANOSECONDS_PER_SECOND;

use wcontainers_abi::{PAGE_SIZE, Permissions, RegisterBlock};
use zerocopy::IntoBytes;

use crate::counters::reason;
use crate::file::FileRef;
use crate::linux::elf_uapi::{
    EM_AARCH64, ET_CORE, NT_ARM_HW_BREAK, NT_ARM_HW_WATCH, NT_ARM_SYSTEM_CALL,
    NT_ARM_TAGGED_ADDR_CTRL, NT_ARM_TLS, NT_AUXV, NT_FILE, NT_PRFPREG, NT_PRPSINFO, NT_PRSTATUS,
    NT_SIGINFO, PT_LOAD, PT_NOTE, elf64_hdr, elf64_note, elf64_phdr,
};
use crate::linux::general::{
    AT_FDCWD, O_CREAT, O_EXCL, O_LARGEFILE, O_NOFOLLOW, O_WRONLY, RLIMIT_CORE, S_IFREG,
};
use crate::linux::ptrace::{user_fpsimd_state, user_hwdebug_state};
use crate::linux_extra::*;
use crate::memory::{Backing, Mapping};
use crate::process::{Channel, Pid, Tid};
use crate::{Context, Errno, Kernel};

/// The largest NT_FILE note written (`core_file_note_size_limit`); a larger one is left out.
pub(crate) const FILE_NOTE_LIMIT: usize = 4 << 20;

/// The core file's name.
const CORE: &[u8] = b"core";

/// The smallest core size limit a core is written under (`elf_format.min_coredump`).
const MIN_COREDUMP: u64 = PAGE_SIZE;

/// The most of the file one stage writes.
const CHUNK: u64 = 64 * PAGE_SIZE;

/// The wait status bit of a process that dumped core.
const CORE_DUMPED: i32 = 0x80;

/// A dump in progress.
#[derive(Clone)]
pub(crate) struct Dump {
    signal: u32,
    /// The signal's information, for the dump's NT_SIGINFO note.
    info: [u8; 128],
    stage: Stage,
    /// What the file holds, in order of offset; the gaps between are zero.
    emits: Vec<Emit>,
    /// The file's size once written.
    end: u64,
    /// How much of the file is written.
    written: u64,
    /// The file, once made and checked.
    file: Option<FileRef>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Layout,
    Unlink,
    Open,
    Write,
}

/// Bytes `dump_emit` writes at an offset of the file.
#[derive(Clone)]
struct Emit {
    at: u64,
    source: Source,
}

#[derive(Clone)]
enum Source {
    Bytes(Vec<u8>),
    /// A page of the process's memory.
    Page(u64),
}

impl Source {
    fn length(&self) -> u64 {
        match self {
            Source::Bytes(bytes) => bytes.len() as u64,
            Source::Page(_) => PAGE_SIZE,
        }
    }
}

/// The thread writing process `pid`'s core, if one is.
pub(crate) fn writer(kernel: &Kernel, pid: Pid) -> Option<Tid> {
    kernel.dumps.keys().copied().find(|tid| {
        kernel
            .threads
            .get(tid)
            .is_some_and(|thread| thread.process == pid)
    })
}

impl Dump {
    /// The signal the process dies of.
    pub(crate) fn signal(&self) -> u32 {
        self.signal
    }
}

/// Whether a process killed by `signal` would dump core: the signal's default action is to, the
/// process is dumpable and its core size limit is at least `MIN_COREDUMP`.
pub(crate) fn wanted(kernel: &Kernel, pid: Pid) -> bool {
    let process = &kernel.processes[&pid];
    process.limits[RLIMIT_CORE as usize].current >= MIN_COREDUMP && process.map.borrow().dumpable
}

/// Starts the dump of thread `tid`'s process for `signal` (`coredump_wait` stops the other
/// threads where they are, then the dump is written), and runs it as far as it goes now.
pub(crate) fn start<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    signal: u32,
    info: [u8; 128],
) {
    let pid = kernel.threads[&tid].process;
    for other in kernel.processes[&pid].threads.clone() {
        if let Some(client) = &kernel.files.backend {
            client.drop_call(other);
        }
        kernel.unschedule(other);
        kernel.thread_mut(other).call = Default::default();
        if other != tid {
            kernel.park(other, Channel::CoreDump);
        }
    }
    kernel.dumps.insert(
        tid,
        Dump {
            signal,
            info,
            stage: Stage::Layout,
            emits: Vec::new(),
            end: 0,
            written: 0,
            file: None,
        },
    );
    run(kernel, cx, tid);
}

/// Runs thread `tid`'s dump, the first time or again once the backend has answered; when it is
/// done, the process ends with the signal, and the core bit if a core file was made.
pub(crate) fn run<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, tid: Tid) {
    loop {
        let Some(mut dump) = kernel.dumps.remove(&tid) else {
            return;
        };
        kernel.threads[&tid].taken.borrow_mut().rewind();
        kernel.begin_asking(tid);
        let result = step(kernel, cx, tid, &mut dump);
        let parked = result == Err(Errno::PARKED);
        kernel.end_asking(tid, parked);
        if parked {
            kernel.dumps.insert(tid, dump);
            return;
        }
        kernel.finish_call(cx, tid);
        if result == Ok(true) {
            kernel.dumps.insert(tid, dump);
            continue;
        }
        let pid = kernel.threads[&tid].process;
        let status = dump.signal as i32 | if dump.file.is_some() { CORE_DUMPED } else { 0 };
        kernel.end_process(cx, pid, status);
        return;
    }
}

/// Does the dump's next stage: whether another follows.
fn step<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    dump: &mut Dump,
) -> Result<bool, Errno> {
    match dump.stage {
        Stage::Layout => {
            let limit = kernel.processes[&kernel.threads[&tid].process].limits
                [RLIMIT_CORE as usize]
                .current;
            let notes = notes(kernel, cx, tid, dump.signal, &dump.info)?;
            let (emits, end) = layout(kernel, cx, tid, &notes, limit)?;
            dump.emits = emits;
            dump.end = end;
            kernel
                .counters
                .count(cx.host, reason::PROC_FIELDS_NOT_TRACKED, PROC_FIELDS_CORE);
            kernel
                .counters
                .count(cx.host, reason::RESIDENCY_NOT_TRACKED, 0);
            dump.stage = Stage::Unlink;
            Ok(true)
        }
        Stage::Unlink => {
            // A file of that name goes first, whatever it is; what stops this stops the open.
            match unlink(kernel, cx, tid) {
                Err(Errno::PARKED) => return Err(Errno::PARKED),
                _ => dump.stage = Stage::Open,
            }
            Ok(true)
        }
        Stage::Open => {
            let flags = O_CREAT | O_WRONLY | O_NOFOLLOW | O_LARGEFILE | O_EXCL;
            let fd = match super::files::open(kernel, cx, tid, AT_FDCWD as u64, CORE, flags, 0o600)
            {
                Ok(fd) => fd,
                Err(Errno::PARKED) => return Err(Errno::PARKED),
                Err(_) => return Ok(false),
            };
            let file = super::io::file(kernel, tid, fd)?;
            // The file must be the one made, as the dumping user made it.
            let fsuid = kernel.credentials(tid).fsuid;
            let node = file.borrow().node.clone();
            let inode = node.borrow();
            if inode.nlink > 1
                || inode.file_type() != S_IFREG
                || inode.uid != fsuid
                || inode.mode & 0o677 != 0o600
                || !file.borrow().writable()
            {
                return Ok(false);
            }
            drop(inode);
            dump.file = Some(file);
            dump.stage = Stage::Write;
            Ok(dump.end > 0)
        }
        Stage::Write => {
            let file = dump.file.clone().expect("the core file");
            let bytes = dump.bytes_at(kernel, cx, tid)?;
            match super::io::write_bytes(
                kernel,
                cx,
                tid,
                &file,
                &bytes,
                bytes.len() as u64,
                Some(dump.written),
                false,
            ) {
                Ok(written) if written > 0 => {
                    dump.written += written;
                    Ok(dump.written < dump.end)
                }
                Err(Errno::PARKED) => Err(Errno::PARKED),
                // A write that fails ends the dump; the core counts as dumped, as in Linux.
                _ => Ok(false),
            }
        }
    }
}

impl Dump {
    /// The file's next bytes from `written` on.
    fn bytes_at<S: crate::System>(
        &self,
        kernel: &mut Kernel,
        cx: &mut Context<'_, S>,
        tid: Tid,
    ) -> Result<Vec<u8>, Errno> {
        let start = self.written;
        let end = self.end.min(start + CHUNK);
        let mut bytes = vec![0u8; (end - start) as usize];
        let first = self
            .emits
            .partition_point(|emit| emit.at + emit.source.length() <= start);
        for emit in &self.emits[first..] {
            if emit.at >= end {
                break;
            }
            let from = start.max(emit.at);
            let to = end.min(emit.at + emit.source.length());
            let into = &mut bytes[(from - start) as usize..(to - start) as usize];
            match &emit.source {
                Source::Bytes(source) => into
                    .copy_from_slice(&source[(from - emit.at) as usize..(to - emit.at) as usize]),
                Source::Page(address) => {
                    read_memory(kernel, cx, tid, address + (from - emit.at), into)?;
                }
            }
        }
        Ok(bytes)
    }
}

/// Reads the dying process's memory as the dump does (`get_dump_page` forces reading whatever the
/// page allows), once pages of mapped backend files are read in.
fn read_memory<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    address: u64,
    into: &mut [u8],
) -> Result<(), Errno> {
    let guest = super::guest(kernel, cx, tid);
    if !guest.read_in(address, address + into.len() as u64) {
        return Err(Errno::PARKED);
    }
    if guest.cpu.read(guest.memory, address, into).is_err() {
        into.fill(0);
    }
    Ok(())
}

/// `do_unlinkat` of the core's name, as Linux removes an old core before making the new one.
fn unlink<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
) -> Result<(), Errno> {
    let at = super::files::start(kernel, tid, AT_FDCWD as u64, CORE)?;
    let parent = kernel.with_walk(tid, |walk, _| walk.parent(&at, CORE))?;
    if parent.directory.borrow().fs.read_only() {
        return Err(Errno::EROFS);
    }
    let node = kernel.with_walk(tid, |walk, _| walk.step(&parent.directory, &parent.name))?;
    if node.borrow().is_directory() {
        return Err(Errno::EISDIR);
    }
    super::files::remove_from(kernel, cx, tid, &parent.directory, &parent.name, &node)
}

/// One note: its owner's name, its type and its contents.
struct Note {
    name: &'static [u8],
    kind: u32,
    contents: Vec<u8>,
}

impl Note {
    fn core(kind: u32, contents: Vec<u8>) -> Self {
        Note {
            name: b"CORE\0",
            kind,
            contents,
        }
    }

    fn linux(kind: u32, contents: Vec<u8>) -> Self {
        Note {
            name: b"LINUX\0",
            kind,
            contents,
        }
    }
}

/// The notes in `elf_core_dump`'s order: the dumping thread's status, the process's notes, that
/// thread's other register sets, then each other thread's status and register sets.
fn notes<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    signal: u32,
    info: &[u8; 128],
) -> Result<Vec<Note>, Errno> {
    let pid = kernel.threads[&tid].process;
    let mut threads = vec![tid];
    threads.extend(
        kernel.processes[&pid]
            .threads
            .iter()
            .copied()
            .filter(|other| *other != tid),
    );
    let mut notes = Vec::new();
    for (index, &thread) in threads.iter().enumerate() {
        let mut registers = RegisterBlock::ZERO;
        cx.cpu
            .registers(kernel.threads[&thread].cpu, &mut registers);
        notes.push(Note::core(
            NT_PRSTATUS,
            status(kernel, pid, thread, signal, &registers),
        ));
        if index == 0 {
            notes.push(Note::core(NT_PRPSINFO, process_info(kernel, cx, pid, tid)));
            notes.push(Note::core(NT_SIGINFO, info.to_vec()));
            notes.push(Note::core(
                NT_AUXV,
                kernel.processes[&pid].starting_stack.auxiliary.clone(),
            ));
            let files = mapped_files(kernel, pid);
            if files.len() <= FILE_NOTE_LIMIT {
                notes.push(Note::core(NT_FILE, files));
            }
        }
        let fpsimd = user_fpsimd_state {
            vregs: core::array::from_fn(|index| {
                u128::from(registers.v[index][0]) | (u128::from(registers.v[index][1]) << u64::BITS)
            }),
            fpsr: registers.fpsr,
            fpcr: registers.fpcr,
            __reserved: [0; 2],
        };
        // SAFETY: `user_fpsimd_state` is plain integers with no padding.
        let fpsimd = unsafe {
            core::slice::from_raw_parts(
                (&fpsimd as *const user_fpsimd_state).cast::<u8>(),
                size_of::<user_fpsimd_state>(),
            )
        };
        notes.push(Note::core(NT_PRFPREG, fpsimd.to_vec()));
        // TPIDR_EL0, then TPIDR2_EL0, which this machine does not have (zero, as Linux gives).
        let mut tls = registers.tpidr_el0.to_le_bytes().to_vec();
        tls.extend_from_slice(&[0; 8]);
        notes.push(Note::linux(NT_ARM_TLS, tls));
        // No breakpoint or watchpoint registers: tracing is not built (plan section 15).
        let debug = vec![0u8; size_of::<user_hwdebug_state>()];
        notes.push(Note::linux(NT_ARM_HW_BREAK, debug.clone()));
        notes.push(Note::linux(NT_ARM_HW_WATCH, debug));
        // The call the thread is in, or NO_SYSCALL.
        let call = &kernel.threads[&thread].call;
        let in_call = call.wait.is_some() || kernel.woken.contains(&thread);
        let number = if in_call { registers.x[8] as i32 } else { -1 };
        notes.push(Note::linux(
            NT_ARM_SYSTEM_CALL,
            number.to_le_bytes().to_vec(),
        ));
        notes.push(Note::linux(
            NT_ARM_TAGGED_ADDR_CTRL,
            0u64.to_le_bytes().to_vec(),
        ));
    }
    Ok(notes)
}

/// `PROC_FIELDS_NOT_TRACKED`'s detail for a core's notes: the task flags are those
/// `/proc/<pid>/stat` shows.
const PROC_FIELDS_CORE: u64 = 6;

/// A time in nanoseconds as `struct __kernel_old_timeval`.
fn timeval(nanoseconds: u64) -> [i64; 2] {
    [
        (nanoseconds / NANOSECONDS_PER_SECOND) as i64,
        (nanoseconds % NANOSECONDS_PER_SECOND / NANOSECONDS_PER_MICROSECOND) as i64,
    ]
}

/// A thread's `elf_prstatus` (`fill_prstatus`).
fn status(kernel: &Kernel, pid: Pid, tid: Tid, signal: u32, registers: &RegisterBlock) -> Vec<u8> {
    let process = &kernel.processes[&pid];
    let thread = &kernel.threads[&tid];
    // The first thread shows the whole process's time, the others their own.
    let (used, system) = if tid == pid {
        super::time::process_times(kernel, pid)
    } else {
        (thread.runtime, thread.system)
    };
    let mut reg = [0u64; 34];
    reg[..31].copy_from_slice(&registers.x);
    reg[31] = registers.sp;
    reg[32] = registers.pc;
    reg[33] = registers.pstate;
    elf_prstatus {
        si_signo: signal as i32,
        pr_cursig: signal as i16,
        pr_sigpend: thread.pending.set,
        pr_sighold: thread.signal_mask,
        pr_pid: tid,
        pr_ppid: process.parent,
        pr_pgrp: process.group,
        pr_sid: process.session,
        pr_times: [
            timeval(used - system),
            timeval(system),
            timeval(process.children_time - process.children_system),
            timeval(process.children_system),
        ],
        pr_reg: reg,
        pr_fpvalid: 1,
        ..zerocopy::FromZeros::new_zeroed()
    }
    .as_bytes()
    .to_vec()
}

/// The process's `elf_prpsinfo` (`fill_psinfo`), of its first thread.
fn process_info<S: crate::System>(
    kernel: &Kernel,
    cx: &mut Context<'_, S>,
    pid: Pid,
    dumping: Tid,
) -> Vec<u8> {
    let process = &kernel.processes[&pid];
    let leader = kernel.threads.get(&pid);
    let (name, credentials, nice) = match leader {
        Some(thread) => (&thread.name, &thread.credentials, thread.schedule.nice),
        None => (
            &process.ended_leader.name,
            &process.ended_leader.credentials,
            process.ended_leader.schedule.nice,
        ),
    };
    // The dumping thread runs; another first thread waits uninterruptibly, on its way out.
    let (state, flags) = if pid == dumping {
        (0, PF_DUMPCORE | PF_SIGNALED)
    } else {
        (2, PF_SIGNALED | PF_EXITING)
    };
    let mut flag = PF_RANDOMIZE | PF_SUPERPRIV | flags;
    if !process.execed {
        flag |= PF_FORKNOEXEC;
    }
    let mut fname = [0u8; 16];
    let shown = name.len().min(15);
    fname[..shown].copy_from_slice(&name[..shown]);
    // The arguments, at most 79 bytes of them, each NUL a space.
    let (start, end) = process.starting_stack.arguments;
    let mut psargs = [0u8; ELF_PRARGSZ];
    let length = (end.saturating_sub(start) as usize).min(ELF_PRARGSZ - 1);
    if cx
        .cpu
        .read(process.memory, start, &mut psargs[..length])
        .is_err()
    {
        psargs = [0; ELF_PRARGSZ];
    }
    for byte in &mut psargs[..length] {
        if *byte == 0 {
            *byte = b' ';
        }
    }
    elf_prpsinfo {
        pr_state: state,
        pr_sname: b"RSDTZW"[state as usize],
        pr_zomb: 0,
        pr_nice: nice as i8,
        pad0: 0,
        pr_flag: flag,
        pr_uid: credentials.uid,
        pr_gid: credentials.gid,
        pr_pid: pid,
        pr_ppid: process.parent,
        pr_pgrp: process.group,
        pr_sid: process.session,
        pr_fname: fname,
        pr_psargs: psargs,
    }
    .as_bytes()
    .to_vec()
}

/// The NT_FILE note (`fill_files_note`): each mapping of a file, its pages and its file's name.
fn mapped_files(kernel: &Kernel, pid: Pid) -> Vec<u8> {
    let map = kernel.processes[&pid].map.borrow();
    let mut ranges = Vec::new();
    let mut names = Vec::new();
    for mapping in map.iter() {
        let (offset, name) = match &mapping.backing {
            Backing::File {
                node, path, offset, ..
            } => {
                let mut name = path.clone();
                if node.borrow().shown_deleted() {
                    name.extend_from_slice(crate::fs::DELETED.as_bytes());
                }
                (*offset, name)
            }
            Backing::Zero { offset, name, .. } => (*offset, name.clone()),
            _ => continue,
        };
        ranges.push([mapping.start, mapping.end, offset / PAGE_SIZE]);
        names.extend_from_slice(&name);
        names.push(0);
    }
    let mut note = Vec::new();
    note.extend_from_slice(&(ranges.len() as u64).to_le_bytes());
    note.extend_from_slice(&PAGE_SIZE.to_le_bytes());
    for range in &ranges {
        for value in range {
            note.extend_from_slice(&value.to_le_bytes());
        }
    }
    note.extend_from_slice(&names);
    note
}

/// How much of a mapping the dump writes (`vma_dump_size`) under `filter`. Which pages a program
/// touched is not known here: anonymous memory counts as written to, as Linux's `anon_vma` shows
/// once a page was touched, and a private file mapping as written to once a page was copied.
fn dump_size<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    mapping: &Mapping,
    filter: u32,
) -> Result<u64, Errno> {
    let whole = mapping.end - mapping.start;
    let wants = |bit: u32| filter & bit != 0;
    if mapping.shared {
        let unlinked = match &mapping.backing {
            Backing::File { node, .. } => node.borrow().nlink == 0,
            _ => true,
        };
        let bit = if unlinked {
            DUMP_ANON_SHARED
        } else {
            DUMP_MAPPED_SHARED
        };
        return Ok(if wants(bit) { whole } else { 0 });
    }
    let (node, offset, written) = match &mapping.backing {
        // The vDSO and the like are always written whole.
        Backing::Special(_) => return Ok(whole),
        Backing::File {
            node,
            offset,
            copied,
            ..
        } => (node.clone(), *offset, !copied.is_empty()),
        _ => return Ok(if wants(DUMP_ANON_PRIVATE) { whole } else { 0 }),
    };
    if written && wants(DUMP_ANON_PRIVATE) || wants(DUMP_MAPPED_PRIVATE) {
        return Ok(whole);
    }
    if wants(DUMP_ELF_HEADERS) && offset == 0 && mapping.permissions.contains(Permissions::READ) {
        if node.borrow().mode & 0o111 != 0 {
            return Ok(PAGE_SIZE);
        }
        // A file not executable is written from if it starts as ELF files do.
        let mut magic = [0u8; 4];
        read_memory(kernel, cx, tid, mapping.start, &mut magic)?;
        return Ok(if magic == *b"\x7fELF" { PAGE_SIZE } else { 0 });
    }
    Ok(0)
}

/// Where `elf_core_dump`'s writes land: emits past the core size limit are not made, and the
/// dump ends at the first such.
struct Writer {
    limit: u64,
    position: u64,
    written: u64,
    skip: u64,
    stopped: bool,
    emits: Vec<Emit>,
}

impl Writer {
    /// `dump_emit`: a pending skip moves the position first.
    fn emit(&mut self, source: Source) -> bool {
        if self.stopped {
            return false;
        }
        self.position += core::mem::take(&mut self.skip);
        let length = source.length();
        if self.written + length > self.limit {
            self.stopped = true;
            return false;
        }
        self.emits.push(Emit {
            at: self.position,
            source,
        });
        self.position += length;
        self.written += length;
        true
    }

    /// `dump_align`.
    fn align(&mut self, alignment: u64) {
        let over = (self.position + self.skip) % alignment;
        if over != 0 {
            self.skip += alignment - over;
        }
    }
}

/// The file's writes in order (`elf_core_dump`) and its size.
fn layout<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    notes: &[Note],
    limit: u64,
) -> Result<(Vec<Emit>, u64), Errno> {
    let pid = kernel.threads[&tid].process;
    let filter = kernel.processes[&pid].coredump_filter;
    let mappings: Vec<Mapping> = kernel.processes[&pid]
        .map
        .borrow()
        .iter()
        .cloned()
        .collect();
    let mut sizes = Vec::with_capacity(mappings.len());
    for mapping in &mappings {
        sizes.push(dump_size(kernel, cx, tid, mapping, filter)?);
    }
    let segments = mappings.len() as u64 + 1;
    let header_size = size_of::<elf64_hdr>() as u64;
    let program_header_size = size_of::<elf64_phdr>() as u64;
    let notes_at = header_size + segments * program_header_size;
    let notes_size: u64 = notes
        .iter()
        .map(|note| {
            size_of::<elf64_note>() as u64
                + (note.name.len() as u64).next_multiple_of(4)
                + (note.contents.len() as u64).next_multiple_of(4)
        })
        .sum();
    let data_at = (notes_at + notes_size).next_multiple_of(PAGE_SIZE);
    let mut writer = Writer {
        limit,
        position: 0,
        written: 0,
        skip: 0,
        stopped: false,
        emits: Vec::new(),
    };
    let mut ident = [0u8; 16];
    ident[..4].copy_from_slice(b"\x7fELF");
    // ELFCLASS64, ELFDATA2LSB, EV_CURRENT, ELFOSABI_NONE.
    ident[4..7].copy_from_slice(&[2, 1, 1]);
    let header = elf64_hdr {
        e_ident: ident,
        e_type: ET_CORE as u16,
        e_machine: EM_AARCH64 as u16,
        e_version: 1,
        e_entry: 0,
        e_phoff: header_size,
        e_shoff: 0,
        e_flags: 0,
        e_ehsize: header_size as u16,
        e_phentsize: program_header_size as u16,
        e_phnum: segments as u16,
        e_shentsize: 0,
        e_shnum: 0,
        e_shstrndx: 0,
    };
    writer.emit(Source::Bytes(header.as_bytes().to_vec()));
    let note_header = elf64_phdr {
        p_type: PT_NOTE,
        p_flags: 0,
        p_offset: notes_at,
        p_vaddr: 0,
        p_paddr: 0,
        p_filesz: notes_size,
        p_memsz: 0,
        p_align: 4,
    };
    writer.emit(Source::Bytes(note_header.as_bytes().to_vec()));
    let mut offset = data_at;
    for (mapping, size) in mappings.iter().zip(&sizes) {
        let flag = |permission: Permissions, bit: u32| {
            if mapping.permissions.contains(permission) {
                bit
            } else {
                0
            }
        };
        let header = elf64_phdr {
            p_type: PT_LOAD,
            p_flags: flag(Permissions::READ, 4)
                | flag(Permissions::WRITE, 2)
                | flag(Permissions::EXECUTE, 1),
            p_offset: offset,
            p_vaddr: mapping.start,
            p_paddr: 0,
            p_filesz: *size,
            p_memsz: mapping.end - mapping.start,
            p_align: PAGE_SIZE,
        };
        writer.emit(Source::Bytes(header.as_bytes().to_vec()));
        offset += size;
    }
    for note in notes {
        let header = elf64_note {
            n_namesz: note.name.len() as u32,
            n_descsz: note.contents.len() as u32,
            n_type: note.kind,
        };
        writer.emit(Source::Bytes(header.as_bytes().to_vec()));
        writer.emit(Source::Bytes(note.name.to_vec()));
        writer.align(4);
        writer.emit(Source::Bytes(note.contents.clone()));
        writer.align(4);
    }
    writer.skip = data_at - writer.position;
    // A page never touched is a hole Linux skips (`get_dump_page` finds none); which pages were
    // touched is not known here, so a page that holds only zeros is taken for one.
    let mut page_bytes = vec![0u8; PAGE_SIZE as usize];
    for (mapping, size) in mappings.iter().zip(&sizes) {
        let mut page = mapping.start;
        while page < mapping.start + size && !writer.stopped {
            read_memory(kernel, cx, tid, page, &mut page_bytes)?;
            if page_bytes.iter().all(|byte| *byte == 0) {
                writer.skip += PAGE_SIZE;
            } else {
                writer.emit(Source::Page(page));
            }
            page += PAGE_SIZE;
        }
    }
    // A dump that ends in a skip writes one byte at its end, so the file reaches it.
    if !writer.stopped && writer.skip > 0 {
        writer.skip -= 1;
        writer.emit(Source::Bytes(vec![0]));
    }
    let end = writer
        .emits
        .last()
        .map_or(0, |emit| emit.at + emit.source.length());
    Ok((writer.emits, end))
}
