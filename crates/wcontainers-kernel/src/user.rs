//! Copying to and from guest memory with the checks Linux makes.

use alloc::vec;
use alloc::vec::Vec;

use wcontainers_abi::{Cpu, MemoryId, PAGE_SIZE, Permissions};
use zerocopy::{FromBytes, Immutable, IntoBytes};

use crate::memory::{AddressMap, Backing, page_down, page_up};
use crate::process::{LIMITS, Limit};
use crate::{Errno, errno};

/// One past the highest user address on ARM64 Linux with 48 bit virtual addresses (`TASK_SIZE_MAX`).
pub(crate) const ADDRESS_LIMIT: u64 = 1 << 48;

/// arm64 Linux's `untagged_addr`: the memory calls ignore a user address's top byte, a tag the CPU's
/// top byte ignore also skips (kernel addresses, bit 55 set, are kept whole).
pub(crate) fn untagged(address: u64) -> u64 {
    address & (((address << 8) as i64 >> 8) as u64)
}

/// Linux's `access_ok`: the whole range lies below the user address limit. Whether it is mapped is
/// found out only when it is copied.
pub(crate) fn access_ok(address: u64, length: u64) -> bool {
    length <= ADDRESS_LIMIT && address <= ADDRESS_LIMIT - length
}

/// One process's memory as a system call sees it.
pub(crate) struct Guest<'a> {
    pub cpu: &'a mut dyn Cpu,
    pub memory: MemoryId,
    pub map: &'a core::cell::RefCell<AddressMap>,
    pub limits: &'a [Limit; LIMITS],
    /// Whether the process may lock memory past its limit (`CAP_IPC_LOCK`), for a locked stack's growth.
    pub may_lock_any: bool,
}

impl Guest<'_> {
    /// How many bytes from `address` on can be accessed as `wanted`, growing the stack as a fault there
    /// would. Pages of backend files in the range that are not read yet are asked for, and while
    /// they are awaited nothing is accessible: the call sees a fault, and runs again once they came
    /// (`Kernel::system_call`).
    pub fn accessible(&mut self, address: u64, length: u64, wanted: Permissions) -> u64 {
        let mut done = self.map.borrow().accessible(address, length, wanted);
        if done < length
            && self.map.borrow().find(address + done).is_none()
            && self
                .map
                .borrow_mut()
                .grow_stack(
                    self.cpu,
                    self.memory,
                    address + done,
                    self.limits,
                    self.may_lock_any,
                )
                .unwrap_or(false)
        {
            done = self.map.borrow().accessible(address, length, wanted);
        }
        if done > 0 && !self.read_in(address, address + done) {
            return 0;
        }
        done
    }

    /// Asks for the pages of mapped backend files in `[start, end)` not read yet; whether all are
    /// there.
    pub(crate) fn read_in(&self, start: u64, end: u64) -> bool {
        let map = self.map.borrow();
        let mut there = true;
        let mut at = start;
        while at < end {
            let Some(mapping) = map.find(at) else { break };
            if let Backing::File { node, offset, .. } = &mapping.backing {
                let to = mapping.end.min(end);
                let page = |address: u64| (offset + (address - mapping.start)) / PAGE_SIZE;
                let pages = page(page_down(at))..page(page_up(to));
                if !crate::shared::unfilled(node, pages.clone()).is_empty() {
                    // Read ahead as a fault would, so a copy in pieces waits once.
                    let ahead = pages.start + crate::shared::readahead_pages(node);
                    let reach = page(mapping.end).min(ahead).max(pages.end);
                    there &= crate::shared::fill(node, pages.start..reach).is_ok();
                }
            }
            at = mapping.end;
        }
        there
    }

    /// `copy_from_user` of the whole range, or `EFAULT`.
    pub fn read(&mut self, address: u64, length: u64) -> Result<Vec<u8>, Errno> {
        if length == 0 {
            return Ok(Vec::new());
        }
        if !access_ok(address, length) {
            return Err(Errno::EFAULT);
        }
        if self.accessible(address, length, Permissions::READ) != length {
            return Err(Errno::EFAULT);
        }
        let mut bytes = vec![0; length as usize];
        self.cpu
            .read(self.memory, address, &mut bytes)
            .map_err(errno::from_cpu)?;
        Ok(bytes)
    }

    /// `copy_to_user` of the whole range from a kernel buffer aligned to 8 bytes, or `EFAULT` after
    /// storing what arm64's copy routine stores before the fault.
    pub fn write(&mut self, address: u64, bytes: &[u8]) -> Result<(), Errno> {
        self.write_from(address, bytes, 0)
    }

    /// `copy_to_user` from a kernel buffer at `source` (only its place within 16 bytes matters, as
    /// the copy routine first aligns its loads): on a fault the stores made before it stay, and when
    /// none was counted one byte is tried (`copy_to_user.S`'s fixup).
    pub fn write_from(&mut self, address: u64, bytes: &[u8], source: u64) -> Result<(), Errno> {
        if self.copy_out(address, bytes, source) == bytes.len() as u64 {
            Ok(())
        } else {
            Err(Errno::EFAULT)
        }
    }

    /// `write_from`'s copy, returning how many bytes it reports copied (`copy_to_iter`'s answer).
    pub fn copy_out(&mut self, address: u64, bytes: &[u8], source: u64) -> u64 {
        let length = bytes.len() as u64;
        // Copying nothing succeeds wherever it points, as `copy_to_user` reports nothing uncopied.
        if length == 0 {
            return 0;
        }
        if !access_ok(address, length) {
            return 0;
        }
        let writable = self.accessible(address, length, Permissions::WRITE);
        let (stored, reported) = match stored_before_fault(length, writable, source) {
            (stored, 0) => (stored.max(writable.min(1)), writable.min(1)),
            counts => counts,
        };
        if stored > 0
            && (self.copy_on_write(address, stored).is_err()
                || self
                    .cpu
                    .write(self.memory, address, &bytes[..stored as usize])
                    .is_err())
        {
            return 0;
        }
        reported
    }

    /// `put_user`: one store, made whole or not at all.
    pub fn put(&mut self, address: u64, bytes: &[u8]) -> Result<(), Errno> {
        let length = bytes.len() as u64;
        if !access_ok(address, length)
            || self.accessible(address, length, Permissions::WRITE) != length
        {
            return Err(Errno::EFAULT);
        }
        self.copy_on_write(address, length)?;
        self.cpu
            .write(self.memory, address, bytes)
            .map_err(errno::from_cpu)
    }

    /// `unsafe_put_user` stores in order, each at an offset from `address`, ending at the first that
    /// faults. Returns how many were made.
    pub fn put_each(&mut self, address: u64, stores: &[(u64, &[u8])]) -> usize {
        let end = stores
            .iter()
            .map(|(offset, bytes)| offset + bytes.len() as u64)
            .max()
            .unwrap_or(0);
        if !access_ok(address, end) {
            return 0;
        }
        let writable = self.accessible(address, end, Permissions::WRITE);
        let made = stores
            .iter()
            .position(|(offset, bytes)| offset + bytes.len() as u64 > writable)
            .unwrap_or(stores.len());
        for (offset, bytes) in &stores[..made] {
            if self
                .copy_on_write(address + offset, bytes.len() as u64)
                .and_then(|()| {
                    self.cpu
                        .write(self.memory, address + offset, bytes)
                        .map_err(errno::from_cpu)
                })
                .is_err()
            {
                return 0;
            }
        }
        made
    }

    /// Makes the private file pages of a range about to be written the process's own copies, as a
    /// write fault would.
    fn copy_on_write(&mut self, address: u64, length: u64) -> Result<(), Errno> {
        self.map
            .borrow_mut()
            .copy_on_write(self.cpu, self.memory, address, address + length)
    }

    /// Copies as much of `bytes` as can be written, a page at a time, as a partial `copy_to_user`.
    pub fn write_partial(&mut self, address: u64, bytes: &[u8]) -> usize {
        if !access_ok(address, bytes.len() as u64) {
            return 0;
        }
        let length = self.accessible(address, bytes.len() as u64, Permissions::WRITE) as usize;
        if self.copy_on_write(address, length as u64).is_err()
            || self
                .cpu
                .write(self.memory, address, &bytes[..length])
                .is_err()
        {
            return 0;
        }
        length
    }

    /// Zeroes as much of a range as ARM64 Linux's `clear_user` does before a fault, and returns how
    /// much: it stores eight bytes at a time from the start (and overlapping words at the end), so a
    /// store that would cross into a page it may not write ends the clearing at that store's start.
    pub fn clear_partial(&mut self, address: u64, length: u64) -> u64 {
        if !access_ok(address, length) {
            return 0;
        }
        let writable = self.accessible(address, length, Permissions::WRITE);
        let fits = |from: u64, size: u64| from + size <= address + writable;
        let end = address + length;
        let cleared = if length >= 8 {
            // The main loop, then one store of the last eight bytes.
            let mut at = address;
            loop {
                if !fits(at, 8) {
                    break at - address;
                }
                at += 8;
                if at + 8 >= end {
                    break if fits(end - 8, 8) {
                        length
                    } else {
                        at - address
                    };
                }
            }
        } else if length >= 4 {
            match (fits(address, 4), fits(end - 4, 4)) {
                (true, true) => length,
                (true, false) => 4,
                _ => 0,
            }
        } else {
            let half = length & 2 != 0;
            if half && !fits(address, 2) {
                0
            } else if length & 1 != 0 && !fits(end - 1, 1) {
                length - 1
            } else {
                length
            }
        };
        if cleared > 0
            && (self.copy_on_write(address, cleared).is_err()
                || self.cpu.zero(self.memory, address, cleared).is_err())
        {
            return 0;
        }
        cleared
    }

    /// Copies `length` bytes from `from` to `to` within this memory as ARM64 Linux's user copy routine
    /// (`copy_template.S`) orders its loads and stores, which decides what overlapping ranges end
    /// up holding. Both ranges must be accessible.
    pub fn copy_within(&mut self, from: u64, to: u64, length: u64) -> Result<(), Errno> {
        if length == 0 || from == to {
            return Ok(());
        }
        self.copy_on_write(to, length)?;
        if from + length <= to || to + length <= from {
            let mut bytes = vec![0; length as usize];
            self.cpu
                .read(self.memory, from, &mut bytes)
                .map_err(errno::from_cpu)?;
            return self
                .cpu
                .write(self.memory, to, &bytes)
                .map_err(errno::from_cpu);
        }
        let low = from.min(to);
        let mut bytes = vec![0; (from.max(to) + length - low) as usize];
        self.cpu
            .read(self.memory, low, &mut bytes)
            .map_err(errno::from_cpu)?;
        template_copy(
            &mut bytes,
            (from - low) as usize,
            (to - low) as usize,
            length as usize,
            from as usize,
        );
        let start = (to - low) as usize;
        self.cpu
            .write(self.memory, to, &bytes[start..start + length as usize])
            .map_err(errno::from_cpu)
    }

    pub fn read_value<T: FromBytes>(&mut self, address: u64) -> Result<T, Errno> {
        let bytes = self.read(address, size_of::<T>() as u64)?;
        Ok(T::read_from_bytes(&bytes).expect("size matches"))
    }

    pub fn write_value<T: IntoBytes + Immutable>(
        &mut self,
        address: u64,
        value: &T,
    ) -> Result<(), Errno> {
        self.write(address, value.as_bytes())
    }

    /// A string ending in a zero byte, as `strncpy_from_user` reads it: `ENAMETOOLONG` if there is no
    /// zero within `limit` bytes.
    pub fn read_string(&mut self, address: u64, limit: u64) -> Result<Vec<u8>, Errno> {
        let mut out = Vec::new();
        let mut at = address;
        loop {
            if at >= ADDRESS_LIMIT {
                return Err(Errno::EFAULT);
            }
            let page_end = (at / PAGE_SIZE + 1) * PAGE_SIZE;
            let wanted = (page_end - at).min(limit - out.len() as u64);
            if !access_ok(at, wanted) || self.accessible(at, wanted, Permissions::READ) != wanted {
                return Err(Errno::EFAULT);
            }
            let mut chunk = vec![0; wanted as usize];
            self.cpu
                .read(self.memory, at, &mut chunk)
                .map_err(errno::from_cpu)?;
            if let Some(zero) = chunk.iter().position(|byte| *byte == 0) {
                out.extend_from_slice(&chunk[..zero]);
                return Ok(out);
            }
            out.extend_from_slice(&chunk);
            if out.len() as u64 >= limit {
                return Err(Errno::ENAMETOOLONG);
            }
            at = page_end;
        }
    }

    /// A path argument that may be empty (for `AT_EMPTY_PATH`).
    pub fn read_path_maybe_empty(&mut self, address: u64) -> Result<Vec<u8>, Errno> {
        self.read_string(address, crate::linux::general::PATH_MAX.into())
    }
}

/// How many of `count` bytes arm64's `copy_to_user` stores, and how many it reports copied, from a
/// source at `source` when only the first `writable` bytes of the target can be written. Its stores
/// go up through the target in order: pieces of 1, 2, 4 and 8 bytes that align the source (only when
/// 16 or more are copied), pairs of 8 (`user_stp` is two `sttr`, and the target moves on only after
/// both), then 8, 4, 2 and 1 for the rest. A store that would reach past `writable` faults whole
/// (measured), and nothing after it is stored; the first half of a pair that faults stays stored but
/// is not counted (measured).
fn stored_before_fault(count: u64, writable: u64, source: u64) -> (u64, u64) {
    let leading = if count < 16 {
        0
    } else {
        source.wrapping_neg() & 15
    };
    let middle = (count - leading) & !15;
    let pieces = [1, 2, 4, 8]
        .into_iter()
        .filter(|size| leading & size != 0)
        .chain(core::iter::repeat_n(16, (middle / 16) as usize))
        .chain(
            [8, 4, 2, 1]
                .into_iter()
                .filter(|size| (count - leading) & 15 & size != 0),
        );
    let mut stored = 0;
    for size in pieces {
        if stored + size > writable {
            if size == 16 && stored + 8 <= writable {
                return (stored + 8, stored);
            }
            break;
        }
        stored += size;
    }
    (stored, stored)
}

/// ARM64 Linux's `copy_template.S` run on `bytes`: copies `count` bytes from offset `source` to
/// offset `target`, with loads and stores in its order and sizes. `source_address` is where the source
/// lies in memory, whose alignment decides the first steps.
fn template_copy(
    bytes: &mut [u8],
    source: usize,
    target: usize,
    count: usize,
    source_address: usize,
) {
    struct Copy<'a> {
        bytes: &'a mut [u8],
        source: usize,
        target: usize,
    }
    impl Copy<'_> {
        fn load(&mut self, size: usize) -> [u8; 16] {
            let mut value = [0; 16];
            value[..size].copy_from_slice(&self.bytes[self.source..self.source + size]);
            self.source += size;
            value
        }
        fn store(&mut self, value: [u8; 16], size: usize) {
            self.bytes[self.target..self.target + size].copy_from_slice(&value[..size]);
            self.target += size;
        }
        fn step(&mut self, size: usize) {
            let value = self.load(size);
            self.store(value, size);
        }
        /// Up to 15 bytes, the largest piece first.
        fn tiny(&mut self, count: usize) {
            for size in [8, 4, 2, 1] {
                if count & size != 0 {
                    self.step(size);
                }
            }
        }
        /// Up to 63 bytes: 16 at a time, then the rest.
        fn tail(&mut self, count: usize) {
            for _ in 0..(count & 0x30) / 16 {
                self.step(16);
            }
            self.tiny(count);
        }
    }
    let mut copy = Copy {
        bytes,
        source,
        target,
    };
    if count < 16 {
        copy.tiny(count);
        return;
    }
    // Align the source to 16 bytes, smallest piece first.
    let leading = source_address.wrapping_neg() & 15;
    let mut count = count - leading;
    for size in [1, 2, 4, 8] {
        if leading & size != 0 {
            copy.step(size);
        }
    }
    if count < 64 {
        copy.tail(count);
        return;
    }
    if count < 128 {
        let a = copy.load(16);
        copy.store(a, 16);
        let b = copy.load(16);
        let c = copy.load(16);
        copy.store(b, 16);
        copy.store(c, 16);
        let d = copy.load(16);
        copy.store(d, 16);
        copy.tail(count);
        return;
    }
    // 64 bytes loaded ahead of the stores, then each block stored as the next is loaded.
    let mut held = [copy.load(16), copy.load(16), copy.load(16), copy.load(16)];
    count -= 128;
    loop {
        for value in &mut held {
            copy.store(*value, 16);
            *value = copy.load(16);
        }
        if count < 64 {
            break;
        }
        count -= 64;
    }
    for value in held {
        copy.store(value, 16);
    }
    copy.tail(count);
}
