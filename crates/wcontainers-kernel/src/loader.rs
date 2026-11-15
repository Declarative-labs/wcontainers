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
use crate::linux::general::{S_IFMT, S_IFREG, X_OK};
use crate::memory::{AddressMap, Backing, Mapping, STACK_LIMIT, page_down, page_up};
use crate::process::{Pid, StartingStack};
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

/// A program placed in a new memory, ready to replace the process's old one.
pub(crate) struct Image {
    pub memory: MemoryId,
    pub map: AddressMap,
    pub registers: RegisterBlock,
    pub executable: (Node, Vec<u8>),
    pub starting_stack: StartingStack,
    /// The last part of the file name, for the process's name.
    pub name: Vec<u8>,
}

/// One program file found by path.
struct Program {
    node: Node,
    /// Its path from the root, with links followed.
    path: Vec<u8>,
    bytes: Vec<u8>,
}

/// Finds a file to run by path from the process's point of view, with Linux's checks.
fn open_program(kernel: &Kernel, pid: Pid, path: &[u8]) -> Result<Program, Errno> {
    kernel.with_walk(pid, |walk, process| {
        let (node, full) = walk.locate(&process.cwd, path)?;
        let inode = node.borrow();
        if inode.mode & S_IFMT != S_IFREG || inode.fs.no_execute() {
            return Err(Errno::EACCES);
        }
        if !inode.permits(&process.credentials, X_OK, true) {
            return Err(Errno::EACCES);
        }
        let bytes = match &inode.content {
            Content::Regular(file) => file.data.clone(),
            _ => return Err(Errno::EACCES),
        };
        drop(inode);
        Ok(Program {
            node,
            path: full,
            bytes,
        })
    })
}

/// Loads the program at `path` into a new memory for process `pid`. Nothing about the process changes;
/// on failure nothing is left behind.
pub(crate) fn load(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    pid: Pid,
    path: &[u8],
    arguments: Vec<Vec<u8>>,
    environment: Vec<Vec<u8>>,
) -> Result<Image, Errno> {
    let mut arguments = arguments;
    let mut file_name = path.to_vec();
    let mut program = open_program(kernel, pid, path)?;
    // Scripts: the interpreter runs with the script's path in place of the first argument.
    for depth in 0..=MAX_INTERPRETERS {
        if !program.bytes.starts_with(b"#!") {
            break;
        }
        if depth == MAX_INTERPRETERS {
            return Err(Errno::ELOOP);
        }
        let (interpreter, argument) = parse_script_line(&program.bytes)?;
        let mut new_arguments = vec![interpreter.clone()];
        new_arguments.extend(argument);
        new_arguments.push(file_name.clone());
        new_arguments.extend(arguments.into_iter().skip(1));
        arguments = new_arguments;
        program = open_program(kernel, pid, &interpreter)?;
        file_name = interpreter;
    }
    let memory = cx.cpu.memory_new().map_err(errno::from_cpu)?;
    let result = place(
        kernel,
        cx,
        pid,
        memory,
        program,
        &file_name,
        path,
        &arguments,
        &environment,
    );
    if result.is_err() {
        cx.cpu.memory_drop(memory);
    }
    result
}

/// The interpreter and its one optional argument from a `#!` line, as Linux's `load_script` splits it.
fn parse_script_line(bytes: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>), Errno> {
    let window = &bytes[2..bytes.len().min(SCRIPT_LINE)];
    let line = match window.iter().position(|byte| *byte == b'\n') {
        Some(end) => &window[..end],
        None => {
            // A line longer than the buffer is accepted only if the interpreter's name ends inside it.
            let trimmed = trim_start(window);
            if trimmed.iter().all(|byte| !is_blank(*byte)) && bytes.len() > SCRIPT_LINE {
                return Err(Errno::ENOEXEC);
            }
            window
        }
    };
    let line = trim_start(line);
    let line = trim_end(line);
    if line.is_empty() {
        return Err(Errno::ENOEXEC);
    }
    let name_end = line
        .iter()
        .position(|byte| is_blank(*byte))
        .unwrap_or(line.len());
    let interpreter = line[..name_end].to_vec();
    let rest = trim_start(&line[name_end..]);
    let argument = (!rest.is_empty()).then(|| rest.to_vec());
    Ok((interpreter, argument))
}

fn is_blank(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}

fn trim_start(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !is_blank(*byte))
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn trim_end(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|byte| !is_blank(*byte))
        .map_or(0, |end| end + 1);
    &bytes[..end]
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
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site, each value distinct"
)]
fn place(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    pid: Pid,
    memory: MemoryId,
    program: Program,
    file_name: &[u8],
    exec_path: &[u8],
    arguments: &[Vec<u8>],
    environment: &[Vec<u8>],
) -> Result<Image, Errno> {
    let end = cx.cpu.address_space().end;
    let mut map = AddressMap::new(end);
    let header = elf::FileHeader64::<LittleEndian>::parse(program.bytes.as_slice())
        .map_err(|_| Errno::ENOEXEC)?;
    let endian = header.endian().map_err(|_| Errno::ENOEXEC)?;
    if header.e_machine(endian) != elf::EM_AARCH64 {
        return Err(Errno::ENOEXEC);
    }
    let segments = header
        .program_headers(endian, program.bytes.as_slice())
        .map_err(|_| Errno::ENOEXEC)?;
    let mut interpreter_path = None;
    for segment in segments {
        if segment.p_type(endian) == elf::PT_INTERP {
            let bytes = segment
                .data(endian, program.bytes.as_slice())
                .map_err(|_| Errno::ENOEXEC)?;
            let path = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
            interpreter_path = Some(path.to_vec());
        }
    }
    let interpreter = match &interpreter_path {
        Some(path) => Some(open_program(kernel, pid, path)?),
        None => None,
    };
    let bias = match header.e_type(endian) {
        elf::ET_EXEC => 0,
        // Where Linux puts a program that can be placed anywhere: two thirds of the way up.
        elf::ET_DYN => page_down(end / 3 * 2),
        _ => return Err(Errno::ENOEXEC),
    };
    let main_path = program.path.clone();
    let main = place_elf(cx, memory, &mut map, &program, &main_path, Some(bias))?;
    map.brk_start = page_up(main.end);
    map.brk = map.brk_start;

    let (entry, interpreter_base) = match &interpreter {
        Some(interpreter) => {
            let placed = place_elf(cx, memory, &mut map, interpreter, &interpreter.path, None)?;
            (placed.entry, placed.base)
        }
        None => (main.entry, 0),
    };

    let credentials = kernel.processes[&pid].credentials.clone();
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
        (AT_SECURE, 0),
    ];
    let (sp, starting_stack) = build_stack(
        cx,
        memory,
        end,
        exec_path,
        arguments,
        environment,
        &auxiliary,
    )?;
    let stack_start = page_down(sp).saturating_sub(STACK_EXPAND);
    map.insert(
        cx.cpu,
        memory,
        Mapping {
            start: stack_start,
            end,
            permissions: Permissions::READ | Permissions::WRITE,
            shared: false,
            backing: Backing::Stack,
        },
    )?;
    // The stack's range was never used before, so making its mapping keeps the bytes just written.
    // No vDSO yet (its page holds the signal return path), and no signal frame size to report.
    for key in [AT_SYSINFO_EHDR, AT_MINSIGSTKSZ] {
        kernel
            .counters
            .count(cx.host, crate::reason::AUXILIARY_MISSING, key.into());
    }
    let mut registers = RegisterBlock::ZERO;
    registers.sp = sp;
    registers.pc = entry;
    let name = file_name
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default()
        .to_vec();
    Ok(Image {
        memory,
        map,
        registers,
        executable: (program.node, main_path),
        starting_stack,
        name,
    })
}

/// Places one ELF file's loadable segments. With no bias given, the file goes wherever free space is,
/// as `mmap` would put it.
fn place_elf(
    cx: &mut Context<'_>,
    memory: MemoryId,
    map: &mut AddressMap,
    program: &Program,
    path: &[u8],
    bias: Option<u64>,
) -> Result<Placed, Errno> {
    let bytes = program.bytes.as_slice();
    let header = elf::FileHeader64::<LittleEndian>::parse(bytes).map_err(|_| Errno::ENOEXEC)?;
    let endian = header.endian().map_err(|_| Errno::ENOEXEC)?;
    if header.e_machine(endian) != elf::EM_AARCH64 {
        return Err(Errno::ENOEXEC);
    }
    let segments = header
        .program_headers(endian, bytes)
        .map_err(|_| Errno::ENOEXEC)?;
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
            let available = (page_up(to).min(file_size) - from) as usize;
            map.insert(
                cx.cpu,
                memory,
                Mapping {
                    start,
                    end: file_pages_end,
                    permissions,
                    shared: false,
                    backing: Backing::File {
                        node: program.node.clone(),
                        path: path.to_vec(),
                        offset: from,
                        file_end: start + page_up(available as u64),
                    },
                },
            )?;
            cx.cpu
                .write(
                    memory,
                    start,
                    &bytes[from as usize..from as usize + available],
                )
                .map_err(errno::from_cpu)?;
            // Linux clears the rest of the last file page only when zeroed data follows and the segment
            // is writable (`padzero`).
            if segment_memory > segment_file && permissions.contains(Permissions::WRITE) {
                cx.cpu
                    .zero(memory, file_end, file_pages_end - file_end)
                    .map_err(errno::from_cpu)?;
            }
        }
        if stop > file_pages_end {
            map.insert(
                cx.cpu,
                memory,
                Mapping {
                    start: file_pages_end,
                    end: stop,
                    permissions,
                    shared: false,
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
    // Linux allows arguments and environment a quarter of the stack limit.
    if top - sp > STACK_LIMIT / 4 {
        return Err(Errno::E2BIG);
    }

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
    let auxiliary_start = sp + table.len() as u64;
    for (key, value) in vector {
        table.extend_from_slice(&u64::from(key).to_le_bytes());
        table.extend_from_slice(&value.to_le_bytes());
    }
    let auxiliary_end = sp + table.len() as u64;

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
            auxiliary: (auxiliary_start, auxiliary_end),
        },
    ))
}
