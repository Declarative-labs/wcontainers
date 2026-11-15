//! Copying to and from guest memory with the checks Linux makes.

use alloc::vec;
use alloc::vec::Vec;

use wcontainers_abi::{Cpu, MemoryId, PAGE_SIZE, Permissions};
use zerocopy::{FromBytes, Immutable, IntoBytes};

use crate::memory::AddressMap;
use crate::process::{LIMITS, Limit};
use crate::{Errno, errno};

/// One past the highest user address on ARM64 Linux with 48 bit virtual addresses (`TASK_SIZE_MAX`).
pub(crate) const ADDRESS_LIMIT: u64 = 1 << 48;

/// Linux's `access_ok`: the whole range lies below the user address limit. Whether it is mapped is
/// found out only when it is copied.
pub(crate) fn access_ok(address: u64, length: u64) -> bool {
    length <= ADDRESS_LIMIT && address <= ADDRESS_LIMIT - length
}

/// One process's memory as a system call sees it.
pub(crate) struct Guest<'a> {
    pub cpu: &'a mut dyn Cpu,
    pub memory: MemoryId,
    pub map: &'a mut AddressMap,
    pub limits: &'a [Limit; LIMITS],
}

impl Guest<'_> {
    /// How many bytes from `address` on can be accessed as `wanted`, growing the stack as a fault there
    /// would.
    pub fn accessible(&mut self, address: u64, length: u64, wanted: Permissions) -> u64 {
        let done = self.map.accessible(address, length, wanted);
        if done < length
            && self.map.find(address + done).is_none()
            && self
                .map
                .grow_stack(self.cpu, self.memory, address + done, self.limits)
                .unwrap_or(false)
        {
            return self.map.accessible(address, length, wanted);
        }
        done
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

    /// `copy_to_user` of the whole range, or `EFAULT`.
    pub fn write(&mut self, address: u64, bytes: &[u8]) -> Result<(), Errno> {
        let length = bytes.len() as u64;
        // Copying nothing succeeds wherever it points, as `copy_to_user` reports nothing uncopied.
        if length == 0 {
            return Ok(());
        }
        if !access_ok(address, length) {
            return Err(Errno::EFAULT);
        }
        if self.accessible(address, length, Permissions::WRITE) != length {
            return Err(Errno::EFAULT);
        }
        self.cpu
            .write(self.memory, address, bytes)
            .map_err(errno::from_cpu)
    }

    /// Copies as much of `bytes` as can be written, a page at a time, as a partial `copy_to_user`.
    pub fn write_partial(&mut self, address: u64, bytes: &[u8]) -> usize {
        if !access_ok(address, bytes.len() as u64) {
            return 0;
        }
        let length = self.accessible(address, bytes.len() as u64, Permissions::WRITE) as usize;
        if self
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
        if cleared > 0 && self.cpu.zero(self.memory, address, cleared).is_err() {
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

    /// A path argument, as `getname` reads it: at most `PATH_MAX` bytes with the zero, `ENOENT` if empty.
    pub fn read_path(&mut self, address: u64) -> Result<Vec<u8>, Errno> {
        let path = self.read_string(address, crate::linux::general::PATH_MAX.into())?;
        if path.is_empty() {
            return Err(Errno::ENOENT);
        }
        Ok(path)
    }

    /// A path argument that may be empty (for `AT_EMPTY_PATH`).
    pub fn read_path_maybe_empty(&mut self, address: u64) -> Result<Vec<u8>, Errno> {
        self.read_string(address, crate::linux::general::PATH_MAX.into())
    }
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
