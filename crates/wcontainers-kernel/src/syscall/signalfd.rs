//! Signal descriptors, as Linux's `fs/signalfd.c`: a file on the shared anonymous inode whose reads
//! take the reader's own pending signals of a mask, each as a `signalfd_siginfo`.

use super::{Args, guest};
use crate::file::{Anon, FileRef, OpenFile};
use crate::linux::general::*;
use crate::linux_extra::{SFD_CLOEXEC, SFD_NONBLOCK};
use crate::process::{Channel, Tid};
use crate::signal::bit;
use crate::{Context, Errno, Kernel};

/// What `/proc/<pid>/fd` shows.
const LINK: &[u8] = b"anon_inode:[signalfd]";

/// The size of a `signalfd_siginfo`, each read's unit.
const RECORD: u64 = 128;

/// The signals a descriptor reads, if it is one.
pub(super) fn mask_of(file: &OpenFile) -> Option<u64> {
    match file.anon {
        Some(Anon::Signals(mask)) => Some(mask),
        _ => None,
    }
}

pub(super) fn signalfd4(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [fd, mask_address, size, flags, ..]: Args,
) -> Result<u64, Errno> {
    // The size, the mask, then the flags; SIGKILL and SIGSTOP are never read.
    if size != 8 {
        return Err(Errno::EINVAL);
    }
    let mask: u64 = guest(kernel, cx, tid).read_value(mask_address)?;
    let flags = flags as u32;
    if flags & !(SFD_CLOEXEC | SFD_NONBLOCK) != 0 {
        return Err(Errno::EINVAL);
    }
    let mask = mask & !(bit(SIGKILL) | bit(SIGSTOP));
    if fd as i32 != -1 {
        // An existing descriptor takes the new mask; its flags stay.
        let file = super::io::open_file(kernel, tid, fd)?;
        let mut file = file.borrow_mut();
        if mask_of(&file).is_none() {
            return Err(Errno::EINVAL);
        }
        file.anon = Some(Anon::Signals(mask));
        drop(file);
        kernel.wake_readiness_of(kernel.threads[&tid].process);
        return Ok(fd);
    }
    let node = super::anon_node(kernel, cx);
    let opener = kernel.credentials(tid).serial;
    let file = OpenFile::new(node, O_RDWR | (flags & O_NONBLOCK), LINK.to_vec(), opener);
    file.borrow_mut().anon = Some(Anon::Signals(mask));
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let fd = kernel
        .files_mut(tid)
        .add(file, flags & SFD_CLOEXEC != 0, 0, limit)?;
    Ok(fd as u64)
}

/// `signalfd_poll`: readable while the polling thread has a signal of the mask pending.
pub(super) fn poll(kernel: &Kernel, tid: Tid, mask: u64) -> u16 {
    let thread = &kernel.threads[&tid];
    let process = &kernel.processes[&thread.process];
    if (thread.pending.set | process.pending.set) & mask != 0 {
        (POLLIN | POLLRDNORM) as u16
    } else {
        0
    }
}

/// `signalfd_read_iter`: as many whole records as fit, the first waited for unless nonblocking, the
/// rest only if already pending. A record that cannot be stored is lost, as on Linux.
pub(super) fn read(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let mask = mask_of(&file.borrow()).expect("a signal descriptor");
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    let records = total / RECORD;
    if records == 0 {
        return Err(Errno::EINVAL);
    }
    let mut done = 0;
    while done < records {
        let Some(queued) = kernel.dequeue_signal(tid, mask) else {
            if done > 0 {
                break;
            }
            if file.borrow().flags & O_NONBLOCK != 0 {
                return Err(Errno::EAGAIN);
            }
            if kernel.signal_pending(tid) {
                return Err(Errno::ERESTARTSYS);
            }
            kernel.park(tid, Channel::Signals);
            return Err(Errno::PARKED);
        };
        let record = record(&queued.info);
        if let Err(error) = store(kernel, cx, tid, parts, done * RECORD, &record) {
            if done == 0 {
                return Err(error);
            }
            break;
        }
        done += 1;
    }
    Ok(done * RECORD)
}

/// Stores bytes at `offset` into a reader's buffers, across them as `copy_to_iter` does.
pub(super) fn store(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    parts: &[(u64, u64)],
    offset: u64,
    record: &[u8],
) -> Result<(), Errno> {
    let mut skip = offset;
    let mut from = 0usize;
    let mut guest = guest(kernel, cx, tid);
    for &(base, length) in parts {
        if skip >= length {
            skip -= length;
            continue;
        }
        let take = ((length - skip) as usize).min(record.len() - from);
        guest.write(base + skip, &record[from..from + take])?;
        from += take;
        skip = 0;
        if from == record.len() {
            break;
        }
    }
    Ok(())
}

/// `signalfd_copyinfo`: a `signalfd_siginfo` from a `siginfo`, field by field as the information's
/// layout (`siginfo_layout`) gives them.
fn record(info: &[u8; 128]) -> [u8; RECORD as usize] {
    let word = |at: usize| u32::from_le_bytes(info[at..at + 4].try_into().expect("4 bytes"));
    let long = |at: usize| u64::from_le_bytes(info[at..at + 8].try_into().expect("8 bytes"));
    let signal = word(0);
    let code = word(8) as i32;
    let mut out = [0u8; RECORD as usize];
    let mut put = |at: usize, bytes: &[u8]| out[at..at + bytes.len()].copy_from_slice(bytes);
    put(0, &signal.to_le_bytes());
    put(4, &word(4).to_le_bytes());
    put(8, &code.to_le_bytes());
    // Offsets in `signalfd_siginfo`.
    const PID: usize = 12;
    const UID: usize = 16;
    const FD: usize = 20;
    const TID: usize = 24;
    const BAND: usize = 28;
    const OVERRUN: usize = 32;
    const STATUS: usize = 40;
    const INT: usize = 44;
    const PTR: usize = 48;
    const UTIME: usize = 56;
    const STIME: usize = 64;
    const ADDR: usize = 72;
    const ADDR_LSB: usize = 80;
    const SYSCALL: usize = 84;
    const CALL_ADDR: usize = 88;
    const ARCH: usize = 96;
    match layout(signal, code) {
        Layout::Kill => {
            put(PID, &word(16).to_le_bytes());
            put(UID, &word(20).to_le_bytes());
        }
        Layout::Timer => {
            put(TID, &word(16).to_le_bytes());
            put(OVERRUN, &word(20).to_le_bytes());
            put(PTR, &long(24).to_le_bytes());
            put(INT, &word(24).to_le_bytes());
        }
        Layout::Poll => {
            put(BAND, &word(16).to_le_bytes());
            put(FD, &word(24).to_le_bytes());
        }
        Layout::Fault => put(ADDR, &long(16).to_le_bytes()),
        Layout::FaultMemoryError => {
            put(ADDR, &long(16).to_le_bytes());
            put(ADDR_LSB, &info[24..26]);
        }
        Layout::Child => {
            put(PID, &word(16).to_le_bytes());
            put(UID, &word(20).to_le_bytes());
            put(STATUS, &word(24).to_le_bytes());
            put(UTIME, &long(32).to_le_bytes());
            put(STIME, &long(40).to_le_bytes());
        }
        Layout::Queued => {
            put(PID, &word(16).to_le_bytes());
            put(UID, &word(20).to_le_bytes());
            put(PTR, &long(24).to_le_bytes());
            put(INT, &word(24).to_le_bytes());
        }
        Layout::System => {
            put(CALL_ADDR, &long(16).to_le_bytes());
            put(SYSCALL, &word(24).to_le_bytes());
            put(ARCH, &word(28).to_le_bytes());
        }
    }
    out
}

/// Linux's `siginfo_layout`, as signalfd reads it (the bound and protection key faults read as
/// faults).
enum Layout {
    Kill,
    Timer,
    Poll,
    Fault,
    FaultMemoryError,
    Child,
    Queued,
    System,
}

fn layout(signal: u32, code: i32) -> Layout {
    if code > SI_USER as i32 && code < SI_KERNEL as i32 {
        let own = match signal {
            SIGILL => Some((NSIGILL, Layout::Fault)),
            SIGFPE => Some((NSIGFPE, Layout::Fault)),
            SIGSEGV => Some((NSIGSEGV, Layout::Fault)),
            SIGBUS => Some((NSIGBUS, Layout::Fault)),
            SIGTRAP => Some((NSIGTRAP, Layout::Fault)),
            SIGCHLD => Some((NSIGCHLD, Layout::Child)),
            SIGPOLL => Some((NSIGPOLL, Layout::Poll)),
            SIGSYS => Some((NSIGSYS, Layout::System)),
            _ => None,
        };
        return match own {
            Some((limit, layout)) if code <= limit as i32 => {
                if signal == SIGBUS && (BUS_MCEERR_AR..=BUS_MCEERR_AO).contains(&(code as u32)) {
                    Layout::FaultMemoryError
                } else {
                    layout
                }
            }
            _ if code <= NSIGPOLL as i32 => Layout::Poll,
            _ => Layout::Kill,
        };
    }
    match code {
        SI_TIMER => Layout::Timer,
        SI_SIGIO => Layout::Poll,
        _ if code < 0 => Layout::Queued,
        _ => Layout::Kill,
    }
}
