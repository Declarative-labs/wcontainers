//! Event descriptors, as Linux's `fs/eventfd.c`: a 64 bit counter on the shared anonymous inode
//! that writes add to and reads take.

use alloc::rc::Rc;
use core::cell::RefCell;

use super::Args;
use crate::file::{Anon, FileRef, OpenFile};
use crate::linux::general::*;
use crate::process::{Channel, Tid};
use crate::{Context, Errno, Kernel};

/// What `/proc/<pid>/fd` shows.
const LINK: &[u8] = b"anon_inode:[eventfd]";

/// An event descriptor's counter, and whether reads take one at a time (EFD_SEMAPHORE).
pub(crate) struct Counter {
    pub count: u64,
    pub semaphore: bool,
    /// Its number, the lowest no other counter has (`eventfd_ida`), which fdinfo shows.
    pub id: u32,
}

/// The counter behind an open file, if it is an event descriptor.
pub(super) fn of(file: &OpenFile) -> Option<Rc<RefCell<Counter>>> {
    match &file.anon {
        Some(Anon::Event(counter)) => Some(counter.clone()),
        _ => None,
    }
}

/// What waits on a counter, and what epoll watches.
pub(super) fn channel(counter: &Rc<RefCell<Counter>>) -> Channel {
    Channel::Object(Rc::as_ptr(counter) as *const () as usize)
}

pub(super) fn eventfd2(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    [count, flags, ..]: Args,
) -> Result<u64, Errno> {
    let flags = flags as u32;
    if flags & !(EFD_CLOEXEC | EFD_NONBLOCK | EFD_SEMAPHORE) != 0 {
        return Err(Errno::EINVAL);
    }
    let node = super::anon_node(kernel, cx);
    let opener = kernel.credentials(tid).serial;
    let file = OpenFile::new(node, O_RDWR | (flags & O_NONBLOCK), LINK.to_vec(), opener);
    let counters = &mut kernel.files.event_counters;
    counters.retain(|counter| counter.strong_count() > 0);
    let mut taken: alloc::vec::Vec<u32> = counters
        .iter()
        .filter_map(|counter| counter.upgrade().map(|counter| counter.borrow().id))
        .collect();
    taken.sort_unstable();
    let id = (0..)
        .zip(taken)
        .find(|(id, used)| id != used)
        .map_or(counters.len() as u32, |(id, _)| id);
    let counter = Rc::new(RefCell::new(Counter {
        count: u64::from(count as u32),
        semaphore: flags & EFD_SEMAPHORE != 0,
        id,
    }));
    counters.push(Rc::downgrade(&counter));
    file.borrow_mut().anon = Some(Anon::Event(counter));
    let limit = kernel.process_of(tid).limits[RLIMIT_NOFILE as usize].current;
    let fd = kernel
        .files_mut(tid)
        .add(file, flags & EFD_CLOEXEC != 0, 0, limit)?;
    Ok(fd as u64)
}

/// `eventfd_poll`.
pub(super) fn poll(counter: &Counter) -> u16 {
    // `eventfd_poll` gives no RDNORM or WRNORM (measured).
    let mut mask = 0;
    if counter.count > 0 {
        mask |= POLLIN;
    }
    if counter.count == u64::MAX {
        mask |= POLLERR;
    }
    if u64::MAX - 1 > counter.count {
        mask |= POLLOUT;
    }
    mask as u16
}

/// `eventfd_read`: the count (or one, as a semaphore) once it is above zero, waited for unless
/// nonblocking.
pub(super) fn read(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let counter = of(&file.borrow()).expect("an event descriptor");
    let total: u64 = parts.iter().map(|(_, length)| length).sum();
    if total < 8 {
        return Err(Errno::EINVAL);
    }
    let count = counter.borrow().count;
    if count == 0 {
        if super::io::nonblocking(kernel, tid, file) {
            return Err(Errno::EAGAIN);
        }
        if kernel.signal_pending(tid) {
            return Err(Errno::ERESTARTSYS);
        }
        kernel.park(tid, channel(&counter));
        return Err(Errno::PARKED);
    }
    let taken = if counter.borrow().semaphore { 1 } else { count };
    counter.borrow_mut().count -= taken;
    kernel.wake_poll(channel(&counter), EPOLLOUT);
    // The count is taken before it is copied out, as Linux's `eventfd_read` does.
    super::signalfd::store(kernel, cx, tid, parts, 0, &taken.to_le_bytes())?;
    Ok(8)
}

/// `eventfd_write`: adds the value once it fits below the largest count, waited for unless
/// nonblocking.
pub(super) fn write(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    parts: &[(u64, u64)],
) -> Result<u64, Errno> {
    let counter = of(&file.borrow()).expect("an event descriptor");
    // `eventfd_write` takes exactly one value, and a vector is written a segment at a time
    // (`do_loop_readv_writev`), stopping at the first that fails (measured).
    let mut written = 0u64;
    for (index, &(base, length)) in parts.iter().enumerate() {
        let failed = |error: Errno| if written > 0 { Ok(written) } else { Err(error) };
        if length != 8 {
            return failed(Errno::EINVAL);
        }
        let bytes: [u8; 8] = match super::guest(kernel, cx, tid).read(base, 8) {
            Ok(bytes) => bytes.try_into().expect("8 bytes"),
            Err(error) => return failed(error),
        };
        // The first is copied once, before any wait.
        let value = u64::from_le_bytes(bytes);
        let value = if index == 0 {
            kernel.settle_once(tid, value)
        } else {
            value
        };
        if value == u64::MAX {
            return failed(Errno::EINVAL);
        }
        if u64::MAX - counter.borrow().count <= value {
            if written > 0 || file.borrow().flags & O_NONBLOCK != 0 {
                return failed(Errno::EAGAIN);
            }
            if kernel.signal_pending(tid) {
                return Err(Errno::ERESTARTSYS);
            }
            kernel.park(tid, channel(&counter));
            return Err(Errno::PARKED);
        }
        counter.borrow_mut().count += value;
        kernel.wake_poll(channel(&counter), EPOLLIN);
        written += 8;
    }
    Ok(written)
}

/// `eventfd_signal`: the kernel counts one, as a completed request does for its descriptor.
pub(super) fn signal(kernel: &mut Kernel, file: &FileRef) {
    if let Some(counter) = of(&file.borrow()) {
        {
            let mut counter = counter.borrow_mut();
            counter.count = counter.count.saturating_add(1).min(u64::MAX - 1);
        }
        kernel.wake_poll(channel(&counter), EPOLLIN);
    }
}
