//! Reading and writing file descriptors.

use wcontainers_abi::{PAGE_SIZE, Permissions, Stream, Syscall};

use super::Outcome;
use crate::process::{File, Process, Tid};
use crate::{Context, Errno, Kernel, errno, user};

/// The most one read or write moves, as Linux's `MAX_RW_COUNT`.
const MAX_RW_COUNT: u64 = 0x7fff_f000;

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
    if !user::access_ok(buffer, count) {
        return Err(Errno::EFAULT);
    }
    let count = count.min(MAX_RW_COUNT);
    match file {
        File::Null => Ok(count),
        File::HostStream(stream) => write_host_stream(process, cx, stream, buffer, count),
    }
}

/// The host takes a stream's bytes at once, so this is a write to an empty pipe, as the reference's
/// standard streams are: whole pages from the buffer, and the first page that cannot be read in full
/// ends the write and is dropped (`EFAULT` if nothing was written), as Linux's `pipe_write` does.
fn write_host_stream(
    process: &Process,
    cx: &mut Context<'_>,
    stream: Stream,
    buffer: u64,
    count: u64,
) -> Result<u64, Errno> {
    let mut page = [0u8; PAGE_SIZE as usize];
    let mut done = 0;
    while done < count {
        let at = buffer + done;
        let length = (count - done).min(PAGE_SIZE);
        let copied = if process.accessible(at, length, Permissions::READ) == length {
            cx.cpu
                .read(process.memory, at, &mut page[..length as usize])
                .map_err(errno::from_cpu)
        } else {
            Err(Errno::EFAULT)
        };
        if let Err(error) = copied {
            return if done == 0 { Err(error) } else { Ok(done) };
        }
        cx.host.output(stream, &page[..length as usize]);
        done += length;
    }
    Ok(done)
}
