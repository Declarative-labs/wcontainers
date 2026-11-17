//! Starting a program, as Linux's `execve` does: scripts that name their interpreter, ELF programs and
//! the loader they name, and the starting stack.

use alloc::vec;
use alloc::vec::Vec;

use object::LittleEndian;
use object::elf;
use object::read::elf::{FileHeader, ProgramHeader};
use wcontainers_abi::{
    Cpu, HWCAP, HWCAP2, LOWEST_ADDRESS, MemoryId, PAGE_SIZE, Permissions, RegisterBlock,
};

use crate::fs::{Content, Node};
use crate::linux::auxvec::*;
use crate::linux::general::{ARG_MAX, PATH_MAX, R_OK, S_IFLNK, S_IFMT, S_IFREG, X_OK};
use crate::memory::{AddressMap, Backing, Mapping, STACK_LIMIT, page_down, page_up};
use crate::process::{StartingStack, Tid};
use crate::{Context, Errno, Kernel, errno};

/// Linux's `AT_PLATFORM` string on ARM64.
const PLATFORM: &[u8] = b"aarch64\0";

/// Linux's `USER_HZ`, reported as `AT_CLKTCK`.
const CLOCK_TICKS: u64 = 100;

/// How many interpreters a script may name in a chain (`BINPRM_MAX_RECURSION`).
const MAX_INTERPRETERS: usize = 4;

/// The most a `#!` line may hold (`BINPRM_BUF_SIZE`).
const SCRIPT_LINE: usize = 256;

/// What the stack is given beyond the arguments when a program starts (Linux's `stack_expand`).
const STACK_EXPAND: u64 = 128 << 10;

/// An ELF file header and program header as Linux reads them.
type Header = elf::FileHeader64<LittleEndian>;
type Segment = elf::ProgramHeader64<LittleEndian>;

/// Why a program could not be started.
pub(crate) enum Failed {
    /// Before the point of no return: the caller's program carries on and gets the error.
    Refused(Errno),
    /// After it (`begin_new_exec`): there is nothing to go back to, and the process dies of SIGSEGV
    /// (`bprm_execve`).
    Fatal,
}

impl From<Errno> for Failed {
    fn from(error: Errno) -> Self {
        Self::Refused(error)
    }
}

fn fatal(_: Errno) -> Failed {
    Failed::Fatal
}

/// The room Linux gives the strings a program starts with (`bprm_stack_limits`): a quarter of the
/// stack limit, at most three quarters of 8 MiB and at least `ARG_MAX`, less a pointer to each
/// argument (at least one) and variable.
pub(crate) struct ArgumentRoom(u64);

impl ArgumentRoom {
    pub(crate) fn new(
        stack_limit: u64,
        arguments: usize,
        environment: usize,
    ) -> Result<Self, Errno> {
        let limit = (STACK_LIMIT / 4 * 3)
            .min(stack_limit / 4)
            .max(u64::from(ARG_MAX));
        let pointers = (arguments.max(1) as u64 + environment as u64) * 8;
        if limit <= pointers {
            return Err(Errno::E2BIG);
        }
        Ok(Self(limit - pointers))
    }

    /// Takes room for one string and its terminating zero (`bprm_hit_stack_limit`).
    pub(crate) fn take(&mut self, string: &[u8]) -> Result<(), Errno> {
        self.0 = self
            .0
            .checked_sub(string.len() as u64 + 1)
            .ok_or(Errno::E2BIG)?;
        Ok(())
    }

    /// With no arguments, Linux gives the program an empty first one (`do_execveat_common`).
    pub(crate) fn first_argument(&mut self, arguments: &mut Vec<Vec<u8>>) -> Result<(), Errno> {
        if arguments.is_empty() {
            self.take(b"")?;
            arguments.push(Vec::new());
        }
        Ok(())
    }
}

/// A program placed in a new memory, ready to replace the process's old one.
pub(crate) struct Image {
    pub memory: MemoryId,
    pub map: AddressMap,
    /// Where `[vvar]` starts, with the vDSO's code after it.
    pub vdso: u64,
    pub registers: RegisterBlock,
    pub executable: (Node, Vec<u8>),
    pub starting_stack: StartingStack,
    /// The last part of the file name, for the process's name.
    pub name: Vec<u8>,
    /// Who the process is once the program runs.
    pub after: crate::credentials::AfterExec,
    /// Every file the start read (scripts, the program, its loader) was readable by the caller, which
    /// keeps the process dumpable.
    pub readable: bool,
}

/// One program file found by path.
pub(crate) struct Program {
    node: Node,
    /// Its path from the root, with links followed.
    path: Vec<u8>,
    bytes: Vec<u8>,
    /// A program on the backend: the open file it was read through, which its pages are kept by.
    backend: Option<alloc::rc::Rc<crate::fs::backend::Handle>>,
    /// The caller may read it (Linux's `would_dump`).
    readable: bool,
    /// The new program could open it again by the name it was run by, as a script's interpreter
    /// must: not so through a descriptor closed at exec.
    pub reopenable: bool,
}

/// Finds a file to run by path from the process's point of view, with Linux's checks.
pub(crate) fn open_program(
    kernel: &Kernel,
    cpu: &mut dyn Cpu,
    tid: Tid,
    path: &[u8],
) -> Result<Program, Errno> {
    let cwd = kernel.fs(tid).cwd.clone();
    open_program_at(kernel, cpu, tid, &cwd, path, true)
}

/// Finds a file to run by a name the kernel made (`open_exec`): one that may be empty
/// (`getname_kernel`), which names the working directory and so no program.
fn open_named_program(
    kernel: &Kernel,
    cpu: &mut dyn Cpu,
    tid: Tid,
    path: &[u8],
) -> Result<Program, Errno> {
    if path.is_empty() {
        let cwd = kernel.fs(tid).cwd.clone();
        return program_of(kernel, cpu, tid, cwd, Vec::new());
    }
    open_program(kernel, cpu, tid, path)
}

/// The same from `start`; not following a link as the last part makes one ELOOP.
pub(crate) fn open_program_at(
    kernel: &Kernel,
    cpu: &mut dyn Cpu,
    tid: Tid,
    start: &Node,
    path: &[u8],
    follow: bool,
) -> Result<Program, Errno> {
    // A trailing slash follows a last link whatever the flags say and asks for a directory, which
    // no program is (`open_last_lookups`, `do_open`).
    let trailing_slash = path.ends_with(b"/");
    let (node, full) = kernel.with_walk(tid, |walk, _| {
        if !follow
            && !trailing_slash
            && walk.resolve(start, path, false)?.borrow().file_type() == S_IFLNK
        {
            return Err(Errno::ELOOP);
        }
        walk.locate(start, path)
    })?;
    if trailing_slash && !node.borrow().is_directory() {
        return Err(Errno::ENOTDIR);
    }
    program_of(kernel, cpu, tid, node, full)
}

/// Linux's checks on a file to run (`do_open_execat`), for a file already found.
pub(crate) fn program_of(
    kernel: &Kernel,
    cpu: &mut dyn Cpu,
    tid: Tid,
    node: Node,
    path: Vec<u8>,
) -> Result<Program, Errno> {
    let credentials = kernel.credentials(tid);
    let inode = node.borrow();
    if inode.mode & S_IFMT != S_IFREG || inode.fs.no_execute() {
        return Err(Errno::EACCES);
    }
    if !inode.permits(credentials, X_OK, true) {
        return Err(Errno::EACCES);
    }
    let readable = inode.permits(credentials, R_OK, true);
    let size = match &inode.content {
        Content::Backend(remote) => Some(remote.size.get()),
        _ => None,
    };
    let (bytes, backend) = match (&inode.content, size) {
        (Content::Regular(file), _) => {
            let bytes = file.bytes(cpu);
            drop(inode);
            (bytes, None)
        }
        // A program on the backend is opened there and read whole (`do_open_execat`).
        (Content::Backend(_), Some(size)) => {
            drop(inode);
            let handle = crate::fs::backend::open(
                &node,
                crate::linux::general::O_RDONLY | crate::linux::general::O_LARGEFILE,
            )?;
            (crate::fs::backend::read(&handle, 0, size)?, Some(handle))
        }
        _ => return Err(Errno::EACCES),
    };
    Ok(Program {
        node,
        path,
        bytes,
        backend,
        readable,
        reopenable: true,
    })
}

/// Loads the program at `path` into a new memory for thread `tid`'s process. Nothing about the process changes;
/// on failure nothing is left behind.
#[allow(
    clippy::too_many_arguments,
    reason = "the parts of one exec, each distinct"
)]
pub(crate) fn load(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    path: &[u8],
    program: Program,
    arguments: Vec<Vec<u8>>,
    environment: Vec<Vec<u8>>,
    room: ArgumentRoom,
) -> Result<Image, Failed> {
    let mut arguments = arguments;
    let mut room = room;
    let mut file_name = path.to_vec();
    let mut program = program;
    let mut readable = program.readable;
    // Scripts: the interpreter runs with the script's path in place of the first argument.
    for depth in 0..=MAX_INTERPRETERS {
        if !program.bytes.starts_with(b"#!") {
            break;
        }
        if depth == MAX_INTERPRETERS {
            return Err(Errno::ELOOP.into());
        }
        let (interpreter, argument) = parse_script_line(&program.bytes)?;
        // The interpreter would open the script by a path the new program cannot reach.
        if !program.reopenable {
            return Err(Errno::ENOENT.into());
        }
        // `remove_arg_zero`, then the file's name, the argument and the interpreter's name.
        if let Some(first) = arguments.first() {
            room.0 += first.len() as u64 + 1;
        }
        room.take(&file_name)?;
        if let Some(argument) = &argument {
            room.take(argument)?;
        }
        room.take(&interpreter)?;
        let mut new_arguments = vec![interpreter.clone()];
        new_arguments.extend(argument);
        new_arguments.push(file_name.clone());
        new_arguments.extend(arguments.into_iter().skip(1));
        arguments = new_arguments;
        program = open_named_program(kernel, cx.cpu, tid, &interpreter)?;
        readable &= program.readable;
        file_name = interpreter;
    }
    let memory = cx.cpu.memory_new().map_err(|error| cx.cpu_error(error))?;
    let result = place(
        kernel,
        cx,
        tid,
        memory,
        program,
        path,
        &arguments,
        &environment,
        readable,
    );
    if result.is_err() {
        cx.cpu.memory_drop(memory);
    }
    result
}

/// The interpreter and its one optional argument from a `#!` line, as Linux's `load_script` splits
/// it: in the file's first bytes padded with zeros, a zero ends the name, and a line of nothing but
/// blanks and zeros names the empty interpreter.
fn parse_script_line(bytes: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>), Errno> {
    let mut buffer = [0u8; SCRIPT_LINE];
    let length = bytes.len().min(SCRIPT_LINE);
    buffer[..length].copy_from_slice(&bytes[..length]);
    let last = SCRIPT_LINE - 1;
    let blank = |byte: u8| byte == b' ' || byte == b'\t';
    let non_blank = |from: usize, to: usize| (from..=to).find(|&at| !blank(buffer[at]));
    let terminator =
        |from: usize, to: usize| (from..=to).find(|&at| blank(buffer[at]) || buffer[at] == 0);
    let mut end = match buffer.iter().position(|byte| *byte == b'\n') {
        Some(end) => end,
        None => {
            // Without a newline the name must end inside the buffer, or it may be cut short.
            let first = non_blank(2, last).ok_or(Errno::ENOEXEC)?;
            terminator(first, last).ok_or(Errno::ENOEXEC)?;
            last
        }
    };
    while blank(buffer[end - 1]) {
        end -= 1;
    }
    let name = non_blank(2, end)
        .filter(|&name| name != end)
        .ok_or(Errno::ENOEXEC)?;
    let separator = terminator(name, end);
    let argument = separator
        .filter(|&at| buffer[at] != 0)
        .and_then(|at| non_blank(at, end));
    buffer[end] = 0;
    if let (Some(at), Some(_)) = (separator, argument) {
        buffer[at] = 0;
    }
    let string = |from: usize| -> Vec<u8> {
        buffer[from..]
            .iter()
            .take_while(|byte| **byte != 0)
            .copied()
            .collect()
    };
    Ok((string(name), argument.map(string)))
}

/// Bytes of a file from `offset`, as many as it has up to `length`, as Linux's `kernel_read`: an
/// offset a file position cannot hold is EINVAL (`rw_verify_area`).
fn file_bytes(bytes: &[u8], offset: u64, length: u64) -> Result<&[u8], Errno> {
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= i64::MAX as u64)
        .ok_or(Errno::EINVAL)?;
    let clamp = |at: u64| usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
    Ok(&bytes[clamp(offset).min(clamp(end))..clamp(end)])
}

/// The same, all of it or EIO (`elf_read`).
fn elf_read(bytes: &[u8], offset: u64, length: u64) -> Result<&[u8], Errno> {
    let read = file_bytes(bytes, offset, length)?;
    if read.len() as u64 != length {
        return Err(Errno::EIO);
    }
    Ok(read)
}

/// A program's file header as Linux sees it: its first bytes, padded with zeros.
fn file_header(bytes: &[u8]) -> Header {
    let mut buffer = [0u8; size_of::<Header>()];
    let length = bytes.len().min(buffer.len());
    buffer[..length].copy_from_slice(&bytes[..length]);
    *object::pod::from_bytes::<Header>(&buffer)
        .expect("a header's bytes")
        .0
}

/// The program headers, as Linux's `load_elf_phdrs`: entries of Linux's size, at least one and at
/// most 64 KiB of them, all in the file.
fn program_headers(bytes: &[u8], header: &Header) -> Option<Vec<Segment>> {
    let endian = LittleEndian;
    if usize::from(header.e_phentsize(endian)) != size_of::<Segment>() {
        return None;
    }
    let count = usize::from(header.e_phnum(endian));
    let size = (size_of::<Segment>() * count) as u64;
    if size == 0 || size > 65536 {
        return None;
    }
    let raw = elf_read(bytes, header.e_phoff(endian), size).ok()?;
    let (segments, _) = object::pod::slice_from_bytes::<Segment>(raw, count).ok()?;
    Some(segments.to_vec())
}

/// Linux's `parse_elf_properties` and arm64's `arch_parse_elf_property` on the file whose properties
/// count (the loader's when there is one): one GNU property note of at most 1 KiB whose properties
/// are whole, sorted and unique. The CPU has no branch target identification, so what they ask for
/// changes nothing.
fn check_properties(bytes: &[u8], segment: Option<&Segment>) -> Result<(), Errno> {
    let Some(segment) = segment else {
        return Ok(());
    };
    let endian = LittleEndian;
    // `struct elf_note`, the name "GNU" and the alignment of properties on a 64 bit machine.
    const NOTE_HEADER: usize = 12;
    const ALIGN: usize = 8;
    if segment.p_filesz(endian) > 1024 {
        return Err(Errno::ENOEXEC);
    }
    let note = file_bytes(bytes, segment.p_offset(endian), segment.p_filesz(endian))?;
    if note.len() < NOTE_HEADER + 4 {
        return Err(Errno::EIO);
    }
    let word = |at: usize| u32::from_le_bytes(note[at..at + 4].try_into().expect("four bytes"));
    if word(8) != elf::NT_GNU_PROPERTY_TYPE_0.0
        || word(0) != 4
        || &note[NOTE_HEADER..NOTE_HEADER + 4] != b"GNU\0"
    {
        return Err(Errno::ENOEXEC);
    }
    let mut at = (NOTE_HEADER + 4).next_multiple_of(ALIGN);
    if word(4) as usize > note.len() - at {
        return Err(Errno::ENOEXEC);
    }
    let end = at + word(4) as usize;
    let mut previous = None;
    while at != end {
        let left = end - at;
        if left < 8 {
            return Err(Errno::ENOEXEC);
        }
        let (kind, size) = (word(at), word(at + 4) as usize);
        let step = size.next_multiple_of(ALIGN);
        if size > left - 8 || step > left - 8 || previous.is_some_and(|previous| kind <= previous) {
            return Err(Errno::ENOEXEC);
        }
        previous = Some(kind);
        if kind == elf::GNU_PROPERTY_AARCH64_FEATURE_1_AND.0 && size != 4 {
            return Err(Errno::ENOEXEC);
        }
        at += 8 + step;
    }
    Ok(())
}

/// An ELF file placed in memory.
struct Placed {
    /// Where the file's first byte would be (the load bias plus the lowest address).
    base: u64,
    entry: u64,
    program_headers: u64,
    program_header_size: u64,
    program_header_count: u64,
    /// One past the highest byte of any segment, for the program break.
    end: u64,
    /// Linux's `end_data - start_data`: from the highest segment start to the highest end of a
    /// segment's file bytes.
    data: u64,
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site, each value distinct"
)]
fn place(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    memory: MemoryId,
    program: Program,
    exec_path: &[u8],
    arguments: &[Vec<u8>],
    environment: &[Vec<u8>],
    readable: bool,
) -> Result<Image, Failed> {
    let end = cx.cpu.address_space().end;
    let mut map = AddressMap::new(end);
    let endian = LittleEndian;
    // In the order of Linux's `load_elf_binary`: the program's header and program headers, its
    // loader's name and header, the loader's program headers, then the properties.
    let header = file_header(&program.bytes);
    if header.e_ident.magic != elf::ELFMAG
        || !matches!(header.e_type(endian), elf::ET_EXEC | elf::ET_DYN)
        || header.e_machine(endian) != elf::EM_AARCH64
    {
        return Err(Errno::ENOEXEC.into());
    }
    let segments = program_headers(&program.bytes, &header).ok_or(Errno::ENOEXEC)?;
    let mut property = None;
    let mut interpreter = None;
    for segment in &segments {
        match segment.p_type(endian) {
            elf::PT_GNU_PROPERTY => property = Some(*segment),
            elf::PT_INTERP => {
                let size = segment.p_filesz(endian);
                if !(2..=u64::from(PATH_MAX)).contains(&size) {
                    return Err(Errno::ENOEXEC.into());
                }
                let name = elf_read(&program.bytes, segment.p_offset(endian), size)?;
                let (last, name) = name.split_last().expect("two bytes at least");
                if *last != 0 {
                    return Err(Errno::ENOEXEC.into());
                }
                let name = name.split(|byte| *byte == 0).next().unwrap_or_default();
                let found = open_named_program(kernel, cx.cpu, tid, name)?;
                elf_read(&found.bytes, 0, size_of::<Header>() as u64)?;
                interpreter = Some(found);
                break;
            }
            _ => {}
        }
    }
    let interpreter = match interpreter {
        Some(found) => {
            let header = file_header(&found.bytes);
            if header.e_ident.magic != elf::ELFMAG || header.e_machine(endian) != elf::EM_AARCH64 {
                return Err(Errno::ELIBBAD.into());
            }
            let segments = program_headers(&found.bytes, &header).ok_or(Errno::ELIBBAD)?;
            property = segments
                .iter()
                .rfind(|segment| segment.p_type(endian) == elf::PT_GNU_PROPERTY)
                .copied();
            Some((found, header, segments))
        }
        None => None,
    };
    let properties_of = interpreter
        .as_ref()
        .map_or(&program.bytes, |(found, ..)| &found.bytes);
    check_properties(properties_of, property.as_ref())?;
    // Where Linux puts a program that can be placed anywhere: two thirds of the way up.
    let bias = if header.e_type(endian) == elf::ET_EXEC {
        0
    } else {
        page_down(end / 3 * 2)
    };
    let readable = readable
        && interpreter
            .as_ref()
            .is_none_or(|(program, ..)| program.readable);
    let nosuid = program.node.borrow().fs.flags & crate::linux_extra::ST_NOSUID != 0;
    // A program on the backend has its file capabilities read there (`get_vfs_caps_from_disk`,
    // into a buffer of the larger form's size), unless its mount ignores them.
    let backend_capabilities = if crate::fs::backend::remote(&program.node).is_some() && !nosuid {
        match crate::fs::backend::getxattr(
            &program.node,
            crate::syscall::CAPABILITY,
            size_of::<crate::linux::general::vfs_ns_cap_data>() as u32,
        ) {
            Ok(value) => Some(value),
            Err(Errno::ENODATA | Errno::EOPNOTSUPP) => None,
            Err(error) => return Err(error.into()),
        }
    } else {
        None
    };
    let after = {
        let inode = program.node.borrow();
        let capabilities = match &backend_capabilities {
            Some(value) => Some(value.as_slice()),
            None => inode
                .xattrs
                .get(crate::syscall::CAPABILITY)
                .map(Vec::as_slice),
        };
        kernel
            .credentials(tid)
            .for_program(&crate::credentials::ProgramFile {
                mode: inode.mode,
                uid: inode.uid,
                gid: inode.gid,
                nosuid,
                capabilities,
            })?
    };
    // The point of no return: from here a failure kills the process.
    let main_path = program.path.clone();
    let main = place_elf(
        cx,
        memory,
        &mut map,
        (&program, &header, &segments),
        &main_path,
        Some(bias),
    )
    .map_err(fatal)?;
    map.brk_start = page_up(main.end);
    map.brk = map.brk_start;
    map.data_size = main.data;

    let (entry, interpreter_base) = match &interpreter {
        Some((found, header, segments)) => {
            let placed = place_elf(
                cx,
                memory,
                &mut map,
                (found, header, segments),
                &found.path,
                None,
            )
            .map_err(fatal)?;
            (placed.entry, placed.base)
        }
        None => (main.entry, 0),
    };
    // A program on the backend shows the pages it was read into, as Linux's page cache.
    for placed in core::iter::once(&program).chain(interpreter.as_ref().map(|(found, ..)| found)) {
        if let Some(handle) = &placed.backend {
            crate::shared::backend_block_of(kernel, cx, &placed.node, handle, &placed.bytes)
                .map_err(fatal)?;
        }
    }
    crate::shared::show_all(kernel, cx, memory, &map).map_err(fatal)?;
    let vdso = place_vdso(cx, memory, &mut map).map_err(fatal)?;

    let credentials = &after.credentials;
    let auxiliary = [
        (AT_HWCAP, HWCAP),
        (AT_PAGESZ, PAGE_SIZE),
        (AT_CLKTCK, CLOCK_TICKS),
        (AT_PHDR, main.program_headers),
        (AT_PHENT, main.program_header_size),
        (AT_PHNUM, main.program_header_count),
        (AT_BASE, interpreter_base),
        (AT_FLAGS, 0),
        (AT_ENTRY, main.entry),
        (AT_UID, credentials.uid.into()),
        (AT_EUID, credentials.euid.into()),
        (AT_GID, credentials.gid.into()),
        (AT_EGID, credentials.egid.into()),
        (AT_SECURE, after.secure.into()),
        (AT_SYSINFO_EHDR, vdso + crate::vdso::VVAR_PAGES * PAGE_SIZE),
        (AT_MINSIGSTKSZ, crate::linux_extra::SIGNAL_MINSIGSTKSZ),
    ];
    let (sp, starting_stack) = build_stack(
        cx,
        memory,
        end,
        exec_path,
        arguments,
        environment,
        &auxiliary,
    )
    .map_err(fatal)?;
    let stack_start = page_down(sp).saturating_sub(STACK_EXPAND);
    map.insert(
        cx.cpu,
        memory,
        Mapping {
            start: stack_start,
            end,
            permissions: Permissions::READ | Permissions::WRITE,
            allowed: Permissions::all(),
            shared: false,
            locked: crate::memory::Lock::None,
            grows_down: true,
            on_fork: crate::memory::OnFork::default(),
            backing: Backing::Stack,
        },
    )
    .map_err(fatal)?;
    // The stack's range was never used before, so making its mapping keeps the bytes just written.
    let mut registers = RegisterBlock::ZERO;
    registers.sp = sp;
    registers.pc = entry;
    // Linux names the process after the file asked for, not a script's interpreter.
    let name = exec_path
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default()
        .to_vec();
    Ok(Image {
        memory,
        map,
        vdso,
        registers,
        executable: (program.node, main_path),
        starting_stack,
        name,
        after,
        readable,
    })
}

/// Maps `[vvar]` and the vDSO where `mmap` would put them, and writes the vDSO's code
/// (`arch_setup_additional_pages`). The data pages hold nothing a program reads here.
fn place_vdso(cx: &mut Context<'_>, memory: MemoryId, map: &mut AddressMap) -> Result<u64, Errno> {
    use crate::vdso::{CODE_PAGES, VVAR_PAGES};
    let length = (VVAR_PAGES + CODE_PAGES) * PAGE_SIZE;
    let start = map.find_free(length).ok_or(Errno::ENOMEM)?;
    let code = start + VVAR_PAGES * PAGE_SIZE;
    let special = |start: u64, end: u64, permissions: Permissions, name: &'static str| Mapping {
        start,
        end,
        permissions,
        allowed: permissions | Permissions::READ,
        shared: false,
        locked: crate::memory::Lock::None,
        grows_down: false,
        on_fork: crate::memory::OnFork::default(),
        backing: Backing::Special(name),
    };
    map.insert(
        cx.cpu,
        memory,
        special(start, code, Permissions::READ, "[vvar]"),
    )?;
    map.insert(
        cx.cpu,
        memory,
        special(
            code,
            start + length,
            Permissions::READ | Permissions::EXECUTE,
            "[vdso]",
        ),
    )?;
    let image = crate::vdso::image();
    cx.cpu
        .write(memory, code, &image)
        .map_err(errno::from_cpu)?;
    cx.cpu.code_changed(
        memory,
        code,
        CODE_PAGES * PAGE_SIZE,
        &[code + crate::vdso::SIGTRAMP],
    );
    Ok(start)
}

/// Places one ELF file's loadable segments. With no bias given, the file goes wherever free space is,
/// as `mmap` would put it.
fn place_elf(
    cx: &mut Context<'_>,
    memory: MemoryId,
    map: &mut AddressMap,
    (program, header, segments): (&Program, &Header, &[Segment]),
    path: &[u8],
    bias: Option<u64>,
) -> Result<Placed, Errno> {
    let bytes = program.bytes.as_slice();
    let endian = LittleEndian;
    let loads: Vec<_> = segments
        .iter()
        .filter(|segment| segment.p_type(endian) == elf::PT_LOAD)
        .collect();
    let lowest = loads
        .iter()
        .map(|segment| page_down(segment.p_vaddr(endian)))
        .min()
        .ok_or(Errno::ENOEXEC)?;
    let highest = loads
        .iter()
        .map(|segment| {
            segment
                .p_vaddr(endian)
                .saturating_add(segment.p_memsz(endian))
        })
        .max()
        .ok_or(Errno::ENOEXEC)?;
    let span = page_up(highest) - lowest;
    let bias = match (bias, header.e_type(endian)) {
        (_, elf::ET_EXEC) => 0,
        (Some(bias), elf::ET_DYN) => bias,
        (None, elf::ET_DYN) => map.find_free(span).ok_or(Errno::ENOMEM)? - lowest,
        _ => return Err(Errno::ENOEXEC),
    };
    let file_size = bytes.len() as u64;
    let mut program_headers = 0;
    let phoff = header.e_phoff(endian);
    let mut end = 0;
    let (mut start_data, mut end_data) = (0, 0);
    let mut code = Vec::new();
    for segment in &loads {
        let address = bias + segment.p_vaddr(endian);
        let offset = segment.p_offset(endian);
        let segment_file = segment.p_filesz(endian);
        let segment_memory = segment.p_memsz(endian);
        if address % PAGE_SIZE != offset % PAGE_SIZE || segment_file > segment_memory {
            return Err(Errno::ENOEXEC);
        }
        let start = page_down(address);
        let stop = page_up(address.checked_add(segment_memory).ok_or(Errno::ENOEXEC)?);
        if start < LOWEST_ADDRESS || stop > map.end - STACK_LIMIT {
            return Err(Errno::ENOMEM);
        }
        let permissions = segment_permissions(segment.p_flags(endian).0);
        let file_end = address + segment_file;
        let file_pages_end = page_up(file_end);
        if segment_file > 0 {
            // As Linux maps the file: whole pages, so the rest of the last page shows the file's following
            // bytes.
            let from = page_down(offset);
            let to = offset.checked_add(segment_file).ok_or(Errno::ENOEXEC)?;
            if to > file_size {
                return Err(Errno::ENOEXEC);
            }
            let available = page_up(to).min(file_size) - from;
            // Linux clears the rest of the last file page only when zeroed data follows and the segment
            // is writable (`padzero`), which makes that page the process's own copy.
            let mut copied = alloc::collections::BTreeSet::new();
            let last_page = page_down(file_end);
            if segment_memory > segment_file
                && permissions.contains(Permissions::WRITE)
                && file_end != file_pages_end
            {
                let page_offset = from + (last_page - start);
                copied.insert(page_offset / PAGE_SIZE);
                cx.cpu
                    .write(memory, last_page, &bytes[page_offset as usize..to as usize])
                    .map_err(errno::from_cpu)?;
            }
            // The file's pages are shown once every segment is placed.
            map.insert(
                cx.cpu,
                memory,
                Mapping {
                    start,
                    end: file_pages_end,
                    permissions,
                    allowed: Permissions::all(),
                    shared: false,
                    locked: crate::memory::Lock::None,
                    grows_down: false,
                    on_fork: crate::memory::OnFork::default(),
                    backing: Backing::File {
                        node: program.node.clone(),
                        path: path.to_vec(),
                        offset: from,
                        file_end: start + page_up(available),
                        copied,
                    },
                },
            )?;
        }
        if stop > file_pages_end {
            map.insert(
                cx.cpu,
                memory,
                Mapping {
                    start: file_pages_end,
                    end: stop,
                    permissions,
                    allowed: Permissions::all(),
                    shared: false,
                    locked: crate::memory::Lock::None,
                    grows_down: false,
                    on_fork: crate::memory::OnFork::default(),
                    backing: Backing::Anonymous,
                },
            )?;
        }
        if permissions.contains(Permissions::EXECUTE) {
            code.push((start, stop));
        }
        if (offset..offset + segment_file).contains(&phoff) {
            program_headers = address + (phoff - offset);
        }
        end = end.max(address + segment_memory);
        start_data = start_data.max(address);
        end_data = end_data.max(file_end);
    }
    let entry = bias + header.e_entry(endian);
    for (start, stop) in code {
        let starts: &[u64] = if (start..stop).contains(&entry) {
            &[entry]
        } else {
            &[]
        };
        cx.cpu.code_changed(memory, start, stop - start, starts);
    }
    Ok(Placed {
        base: bias + lowest,
        entry,
        program_headers,
        program_header_size: u64::from(header.e_phentsize(endian)),
        program_header_count: u64::from(header.e_phnum(endian)),
        end,
        data: end_data.saturating_sub(start_data),
    })
}

fn segment_permissions(flags: u32) -> Permissions {
    let mut permissions = Permissions::empty();
    if flags & elf::PF_R.0 != 0 {
        permissions |= Permissions::READ;
    }
    if flags & elf::PF_W.0 != 0 {
        permissions |= Permissions::WRITE;
    }
    if flags & elf::PF_X.0 != 0 {
        permissions |= Permissions::EXECUTE;
    }
    permissions
}

/// Writes the starting stack as Linux lays it out and returns the stack pointer, which points at `argc`.
///
/// From the top down: the program's file name, the environment strings, the argument strings, the
/// platform name, 16 random bytes, then, 16 byte aligned, `argc`, the argument pointers, the environment
/// pointers and the auxiliary vector.
fn build_stack(
    cx: &mut Context<'_>,
    memory: MemoryId,
    top: u64,
    file_name: &[u8],
    arguments: &[Vec<u8>],
    environment: &[Vec<u8>],
    auxiliary: &[(u32, u64)],
) -> Result<(u64, StartingStack), Errno> {
    let mut strings = Vec::new();
    let mut argument_offsets = Vec::with_capacity(arguments.len());
    for argument in arguments {
        argument_offsets.push(strings.len() as u64);
        strings.extend_from_slice(argument);
        strings.push(0);
    }
    let arguments_end = strings.len() as u64;
    let mut environment_offsets = Vec::with_capacity(environment.len());
    for variable in environment {
        environment_offsets.push(strings.len() as u64);
        strings.extend_from_slice(variable);
        strings.push(0);
    }
    let environment_end = strings.len() as u64;
    let file_name_offset = strings.len() as u64;
    strings.extend_from_slice(file_name);
    strings.push(0);

    // Linux leaves one null pointer's worth of space at the very top.
    let strings_start = top - 8 - strings.len() as u64;
    let platform = strings_start - PLATFORM.len() as u64;
    let random = platform - 16;
    let mut random_bytes = [0u8; 16];
    cx.host.random(&mut random_bytes);

    let mut vector: Vec<(u32, u64)> = auxiliary.to_vec();
    vector.extend([
        (AT_RANDOM, random),
        (AT_HWCAP2, HWCAP2),
        (AT_EXECFN, strings_start + file_name_offset),
        (AT_PLATFORM, platform),
        (AT_NULL, 0),
    ]);
    let words = 1 + arguments.len() + 1 + environment.len() + 1 + 2 * vector.len();
    let sp = (random - 8 * words as u64) & !15;

    let mut table = Vec::with_capacity(words * 8);
    table.extend_from_slice(&(arguments.len() as u64).to_le_bytes());
    for offset in argument_offsets {
        table.extend_from_slice(&(strings_start + offset).to_le_bytes());
    }
    table.extend_from_slice(&0u64.to_le_bytes());
    for offset in environment_offsets {
        table.extend_from_slice(&(strings_start + offset).to_le_bytes());
    }
    table.extend_from_slice(&0u64.to_le_bytes());
    let auxiliary_start = table.len();
    for (key, value) in vector {
        table.extend_from_slice(&u64::from(key).to_le_bytes());
        table.extend_from_slice(&value.to_le_bytes());
    }
    let auxiliary = table[auxiliary_start..].to_vec();

    let write = |cpu: &mut dyn Cpu, address: u64, bytes: &[u8]| {
        cpu.write(memory, address, bytes).map_err(errno::from_cpu)
    };
    write(cx.cpu, strings_start, &strings)?;
    write(cx.cpu, platform, PLATFORM)?;
    write(cx.cpu, random, &random_bytes)?;
    write(cx.cpu, sp, &table)?;
    Ok((
        sp,
        StartingStack {
            arguments: (strings_start, strings_start + arguments_end),
            environment: (
                strings_start + arguments_end,
                strings_start + environment_end,
            ),
            auxiliary,
        },
    ))
}
