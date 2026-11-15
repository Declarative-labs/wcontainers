//! Starting a program: placing an ELF file's parts in a new memory and building the starting stack.

use alloc::vec;
use alloc::vec::Vec;

use object::LittleEndian;
use object::elf;
use object::read::elf::{FileHeader, ProgramHeader};
use wcontainers_abi::{Cpu, HWCAP, HWCAP2, MemoryId, PAGE_SIZE, Permissions, RegisterBlock};

use crate::counters::reason;
use crate::linux::auxvec::*;
use crate::process::Mapping;
use crate::{Context, Errno, Kernel, errno};

/// The stack's size, Linux's default `RLIMIT_STACK`.
const STACK_SIZE: u64 = 8 << 20;

/// Linux's `AT_PLATFORM` string on ARM64.
const PLATFORM: &[u8] = b"aarch64\0";

/// Linux's `USER_HZ`, reported as `AT_CLKTCK`.
const CLOCK_TICKS: u64 = 100;

/// A program placed in a new memory, ready for its first thread.
pub(crate) struct Image {
    pub memory: MemoryId,
    pub mappings: Vec<Mapping>,
    pub registers: RegisterBlock,
}

fn page_down(address: u64) -> u64 {
    address & !(PAGE_SIZE - 1)
}

fn page_up(address: u64) -> u64 {
    page_down(address + PAGE_SIZE - 1)
}

/// Places a program in a new memory. On failure nothing is left behind.
pub(crate) fn load(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    program: &[u8],
    arguments: &[&[u8]],
    environment: &[&[u8]],
) -> Result<Image, Errno> {
    let memory = cx.cpu.memory_new().map_err(errno::from_cpu)?;
    let result = place(kernel, cx, memory, program, arguments, environment);
    if result.is_err() {
        cx.cpu.memory_drop(memory);
    }
    result
}

fn place(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    memory: MemoryId,
    program: &[u8],
    arguments: &[&[u8]],
    environment: &[&[u8]],
) -> Result<Image, Errno> {
    let header = elf::FileHeader64::<LittleEndian>::parse(program).map_err(|_| Errno::ENOEXEC)?;
    let endian = header.endian().map_err(|_| Errno::ENOEXEC)?;
    if header.e_machine(endian) != elf::EM_AARCH64 {
        return Err(Errno::ENOEXEC);
    }
    let segments = header
        .program_headers(endian, program)
        .map_err(|_| Errno::ENOEXEC)?;
    if segments
        .iter()
        .any(|segment| segment.p_type(endian) == elf::PT_INTERP)
    {
        kernel.counters.count(cx.host, reason::DYNAMIC_PROGRAM, 0);
        return Err(Errno::ENOEXEC);
    }
    let end = cx.cpu.address_space().end;
    let base = match header.e_type(endian) {
        elf::ET_EXEC => 0,
        // Where Linux puts a program that can be placed anywhere: two thirds of the way up.
        elf::ET_DYN => page_down(end / 3 * 2),
        _ => return Err(Errno::ENOEXEC),
    };

    let mut mappings = Vec::new();
    let mut program_headers = None;
    let phoff = header.e_phoff(endian);
    for segment in segments {
        if segment.p_type(endian) != elf::PT_LOAD {
            continue;
        }
        let address = base + segment.p_vaddr(endian);
        let offset = segment.p_offset(endian);
        let file_size = segment.p_filesz(endian);
        let memory_size = segment.p_memsz(endian);
        if address % PAGE_SIZE != offset % PAGE_SIZE || file_size > memory_size {
            return Err(Errno::ENOEXEC);
        }
        let start = page_down(address);
        let stop = page_up(address.checked_add(memory_size).ok_or(Errno::ENOEXEC)?);
        if start < wcontainers_abi::LOWEST_ADDRESS || stop > end - STACK_SIZE {
            return Err(Errno::ENOEXEC);
        }
        // As Linux maps the file: from the start of the page, up to the end of the file's part.
        let from = page_down(offset) as usize;
        let to = (offset + file_size) as usize;
        let bytes = program.get(from..to).ok_or(Errno::ENOEXEC)?;
        cx.cpu
            .write(memory, start, bytes)
            .map_err(errno::from_cpu)?;
        // Linux clears the rest of the last file page, which may hold bytes from an earlier segment.
        let tail = page_up(start + bytes.len() as u64).min(stop) - (start + bytes.len() as u64);
        if tail > 0 {
            cx.cpu
                .write(memory, start + bytes.len() as u64, &vec![0; tail as usize])
                .map_err(errno::from_cpu)?;
        }
        let permissions = segment_permissions(segment.p_flags(endian));
        cx.cpu
            .set_permissions(memory, start, stop - start, permissions)
            .map_err(errno::from_cpu)?;
        insert(
            &mut mappings,
            Mapping {
                start,
                end: stop,
                permissions,
            },
        );
        if (offset..offset + file_size).contains(&phoff) {
            program_headers = Some(address + (phoff - offset));
        }
    }
    let entry = base + header.e_entry(endian);
    for mapping in &mappings {
        if mapping.permissions.contains(Permissions::EXECUTE) {
            let starts: &[u64] = if (mapping.start..mapping.end).contains(&entry) {
                &[entry]
            } else {
                &[]
            };
            cx.cpu
                .code_changed(memory, mapping.start, mapping.end - mapping.start, starts);
        }
    }

    let stack = Mapping {
        start: end - STACK_SIZE,
        end,
        permissions: Permissions::READ | Permissions::WRITE,
    };
    cx.cpu
        .set_permissions(memory, stack.start, STACK_SIZE, stack.permissions)
        .map_err(errno::from_cpu)?;
    insert(&mut mappings, stack);

    let auxiliary = Auxiliary {
        program_headers: program_headers.unwrap_or(0),
        program_header_size: u64::from(header.e_phentsize(endian)),
        program_header_count: u64::from(header.e_phnum(endian)),
        entry,
    };
    let sp = build_stack(cx, memory, end, arguments, environment, &auxiliary)?;
    let mut registers = RegisterBlock::ZERO;
    registers.sp = sp;
    registers.pc = entry;
    Ok(Image {
        memory,
        mappings,
        registers,
    })
}

/// Adds a mapping, replacing whatever it covers, as a fixed mapping does in Linux.
fn insert(mappings: &mut Vec<Mapping>, new: Mapping) {
    let mut kept = Vec::with_capacity(mappings.len() + 2);
    for old in mappings.drain(..) {
        if old.end <= new.start || old.start >= new.end {
            kept.push(old);
            continue;
        }
        if old.start < new.start {
            kept.push(Mapping {
                end: new.start,
                ..old
            });
        }
        if old.end > new.end {
            kept.push(Mapping {
                start: new.end,
                ..old
            });
        }
    }
    kept.push(new);
    kept.sort_by_key(|mapping| mapping.start);
    *mappings = kept;
}

fn segment_permissions(flags: elf::ProgramFlags) -> Permissions {
    let mut permissions = Permissions::empty();
    if flags.0 & elf::PF_R.0 != 0 {
        permissions |= Permissions::READ;
    }
    if flags.0 & elf::PF_W.0 != 0 {
        permissions |= Permissions::WRITE;
    }
    if flags.0 & elf::PF_X.0 != 0 {
        permissions |= Permissions::EXECUTE;
    }
    permissions
}

struct Auxiliary {
    program_headers: u64,
    program_header_size: u64,
    program_header_count: u64,
    entry: u64,
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
    arguments: &[&[u8]],
    environment: &[&[u8]],
    auxiliary: &Auxiliary,
) -> Result<u64, Errno> {
    let mut strings = Vec::new();
    let mut argument_offsets = Vec::with_capacity(arguments.len());
    for argument in arguments {
        argument_offsets.push(strings.len() as u64);
        strings.extend_from_slice(argument);
        strings.push(0);
    }
    let mut environment_offsets = Vec::with_capacity(environment.len());
    for variable in environment {
        environment_offsets.push(strings.len() as u64);
        strings.extend_from_slice(variable);
        strings.push(0);
    }
    let file_name_offset = strings.len() as u64;
    strings.extend_from_slice(arguments.first().copied().unwrap_or_default());
    strings.push(0);

    // Linux leaves one null pointer's worth of space at the very top.
    let strings_start = top - 8 - strings.len() as u64;
    let platform = strings_start - PLATFORM.len() as u64;
    let random = platform - 16;
    let mut random_bytes = [0u8; 16];
    cx.host.random(&mut random_bytes);

    let vector: Vec<(u32, u64)> = vec![
        (AT_HWCAP, HWCAP),
        (AT_PAGESZ, PAGE_SIZE),
        (AT_CLKTCK, CLOCK_TICKS),
        (AT_PHDR, auxiliary.program_headers),
        (AT_PHENT, auxiliary.program_header_size),
        (AT_PHNUM, auxiliary.program_header_count),
        (AT_BASE, 0),
        (AT_FLAGS, 0),
        (AT_ENTRY, auxiliary.entry),
        (AT_UID, 0),
        (AT_EUID, 0),
        (AT_GID, 0),
        (AT_EGID, 0),
        (AT_SECURE, 0),
        (AT_RANDOM, random),
        (AT_HWCAP2, HWCAP2),
        (AT_EXECFN, strings_start + file_name_offset),
        (AT_PLATFORM, platform),
        (AT_NULL, 0),
    ];
    let words = 1 + arguments.len() + 1 + environment.len() + 1 + 2 * vector.len();
    let sp = (random - 8 * words as u64) & !15;
    // Linux allows arguments and environment a quarter of the stack.
    if top - sp > STACK_SIZE / 4 {
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
    for (key, value) in vector {
        table.extend_from_slice(&u64::from(key).to_le_bytes());
        table.extend_from_slice(&value.to_le_bytes());
    }

    let write = |cpu: &mut dyn Cpu, address: u64, bytes: &[u8]| {
        cpu.write(memory, address, bytes).map_err(errno::from_cpu)
    };
    write(cx.cpu, strings_start, &strings)?;
    write(cx.cpu, platform, PLATFORM)?;
    write(cx.cpu, random, &random_bytes)?;
    write(cx.cpu, sp, &table)?;
    Ok(sp)
}
