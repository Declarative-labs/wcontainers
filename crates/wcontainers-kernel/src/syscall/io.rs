//! Reading and writing file descriptors.

use wcontainers_abi::{Permissions, Syscall};

use super::Outcome;
use crate::process::{File, Tid};
use crate::{Context, Errno, Kernel, errno};

/// The most one read or write moves, as Linux's `MAX_RW_COUNT`.
const MAX_RW_COUNT: u64 = 0x7fff_f000;

/// How many bytes are copied out of guest memory at a time.
const CHUNK: usize = 4096;

pub(super) fn write(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid, call: Syscall) -> Outcome {
    let [fd, buffer, count, ..] = call.arguments;
    write_typed(kernel, cx, tid, fd as u32, buffer, count).into()
}

/// `write(fd, buffer, count)`.
fn write_typed(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    fd: u32,
    buffer: u64,
    count: u64,
) -> Result<u64, Errno> {
    let process = &kernel.processes[&kernel.threads[&tid].process];
    let file = *process.files.get(&fd).ok_or(Errno::EBADF)?;
    let count = count.min(MAX_RW_COUNT);
    if count == 0 {
        return Ok(0);
    }
    let readable = process.accessible(buffer, count, Permissions::READ);
    if readable == 0 {
        return Err(Errno::EFAULT);
    }
    let memory = process.memory;
    let mut chunk = [0u8; CHUNK];
    let mut done = 0;
    while done < readable {
        let length = (readable - done).min(CHUNK as u64) as usize;
        cx.cpu
            .read(memory, buffer + done, &mut chunk[..length])
            .map_err(errno::from_cpu)?;
        match file {
            File::HostStream(stream) => cx.host.output(stream, &chunk[..length]),
        }
        done += length as u64;
    }
    Ok(done)
}
