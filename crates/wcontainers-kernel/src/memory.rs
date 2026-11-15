//! A process's address space as the kernel keeps it: the ranges it has mapped and what each allows.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use wcontainers_abi::{Cpu, LOWEST_ADDRESS, MemoryId, PAGE_SIZE, Permissions};

use crate::fs::Node;
use crate::linux::general::{RLIMIT_AS, RLIMIT_DATA, RLIMIT_MEMLOCK, RLIMIT_STACK};
use crate::process::{LIMITS, Limit};
use crate::{Errno, errno};

pub(crate) fn page_down(address: u64) -> u64 {
    address & !(PAGE_SIZE - 1)
}

pub(crate) fn page_up(address: u64) -> u64 {
    page_down(address.saturating_add(PAGE_SIZE - 1))
}

/// Linux's default stack limit, `RLIMIT_STACK`, which places a new program's mappings.
pub(crate) const STACK_LIMIT: u64 = 8 << 20;

/// The gap Linux keeps below a stack that may grow (`stack_guard_gap`, 256 pages).
pub(crate) const STACK_GUARD_GAP: u64 = 256 * PAGE_SIZE;

/// What a range holds, for `/proc/<pid>/maps` and for what happens past the end of a file.
#[derive(Clone)]
pub(crate) enum Backing {
    Anonymous,
    /// A private copy of a file's pages from `offset`. Pages wholly past the file's end, from
    /// `file_end` on, are kept with no access, and touching one is `SIGBUS` as on Linux.
    File {
        node: Node,
        path: Vec<u8>,
        offset: u64,
        file_end: u64,
    },
    Heap,
    Stack,
}

#[derive(Clone)]
pub(crate) struct Mapping {
    pub start: u64,
    pub end: u64,
    /// What the program asked for.
    pub permissions: Permissions,
    pub shared: bool,
    /// Whether `mlock` holds it in memory (Linux's `VM_LOCKED`).
    pub locked: Lock,
    pub backing: Backing,
}

/// Whether a mapping is locked in memory, and if so whether its pages are brought in at once or as
/// they are touched (`VM_LOCKONFAULT`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Lock {
    #[default]
    None,
    Now,
    OnFault,
}

impl Mapping {
    /// Whether the mapping counts against `RLIMIT_DATA`: private and writable, and not a stack
    /// (Linux's `is_data_mapping`).
    pub fn is_data(&self) -> bool {
        is_data(self.permissions, self.shared, &self.backing)
    }

    /// What the CPU side is told the page at `address` allows.
    fn page_permissions(&self, address: u64) -> Permissions {
        match &self.backing {
            Backing::File { file_end, .. } if address >= *file_end => Permissions::empty(),
            _ => cpu_permissions(self.permissions),
        }
    }

    /// The same mapping cut to `[start, end)`.
    pub fn cut_public(&self, start: u64, end: u64) -> Mapping {
        self.cut(start, end)
    }

    fn cut(&self, start: u64, end: u64) -> Mapping {
        let mut cut = self.clone();
        if let Backing::File { offset, .. } = &mut cut.backing {
            *offset += start - self.start;
        }
        cut.start = start;
        cut.end = end;
        cut
    }
}

/// Whether a mapping with these properties counts against `RLIMIT_DATA`.
pub(crate) fn is_data(permissions: Permissions, shared: bool, backing: &Backing) -> bool {
    permissions.contains(Permissions::WRITE) && !shared && !matches!(backing, Backing::Stack)
}

/// Every mapped range of one process, and where the next ones go.
pub(crate) struct AddressMap {
    /// By start address; never overlapping.
    mappings: BTreeMap<u64, Mapping>,
    /// Ranges that have been mapped at some time, so may hold old bytes. Start to end.
    used: BTreeMap<u64, u64>,
    /// One past the highest address.
    pub end: u64,
    /// New mappings with no address go below this, highest first.
    pub mmap_base: u64,
    pub brk_start: u64,
    pub brk: u64,
    /// How new mappings are locked, as `mlockall(MCL_FUTURE)` sets (Linux's `def_flags`).
    pub future_lock: Lock,
    /// `membarrier` registrations, as Linux's `MEMBARRIER_STATE_*` bits.
    pub membarrier: u32,
    /// Protection keys allocated, one bit each; key 0 from the start.
    pub protection_keys: u32,
}

impl AddressMap {
    pub fn new(end: u64) -> Self {
        Self {
            mappings: BTreeMap::new(),
            used: BTreeMap::new(),
            end,
            // Linux leaves at least 128 MiB below the stack's top for the stack and its gap.
            mmap_base: page_down(end - (128 << 20)),
            brk_start: 0,
            brk: 0,
            future_lock: Lock::None,
            membarrier: 0,
            protection_keys: 1,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mapping> {
        self.mappings.values()
    }

    /// Whether `pages` more pages, data pages if `data`, fit the limits on the address space and on
    /// data, as Linux's `may_expand_vm`. `freed` pages of the same kind are given up at the same time.
    pub fn may_expand(
        &self,
        limits: &[Limit; LIMITS],
        data: bool,
        pages: u64,
        freed: (u64, u64),
    ) -> bool {
        let (total, data_pages) = self.iter().fold((0, 0), |(total, data), mapping| {
            let pages = (mapping.end - mapping.start) / PAGE_SIZE;
            (
                total + pages,
                data + if mapping.is_data() { pages } else { 0 },
            )
        });
        let total = total - freed.0;
        let data_pages = data_pages - freed.1;
        if total + pages > limits[RLIMIT_AS as usize].current / PAGE_SIZE {
            return false;
        }
        let data_limit = limits[RLIMIT_DATA as usize];
        if data && data_pages + pages > data_limit.current / PAGE_SIZE {
            // Linux lets a zero limit through up to the hard limit, for Valgrind.
            return data_limit.current == 0 && data_pages + pages <= data_limit.maximum / PAGE_SIZE;
        }
        true
    }

    /// The pages of the range that are mapped, and how many of them are data.
    pub fn pages_in(&self, start: u64, end: u64) -> (u64, u64) {
        let mut total = 0;
        let mut data = 0;
        for mapping in self.mappings.values() {
            let from = mapping.start.max(start);
            let to = mapping.end.min(end);
            if from < to {
                total += (to - from) / PAGE_SIZE;
                if mapping.is_data() {
                    data += (to - from) / PAGE_SIZE;
                }
            }
        }
        (total, data)
    }

    /// The mapping holding `address`.
    pub fn find(&self, address: u64) -> Option<&Mapping> {
        let (_, mapping) = self.mappings.range(..=address).next_back()?;
        (address < mapping.end).then_some(mapping)
    }

    /// The first mapping that ends above `address`.
    pub fn next_after(&self, address: u64) -> Option<&Mapping> {
        if let Some(mapping) = self.find(address) {
            return Some(mapping);
        }
        self.mappings
            .range(address..)
            .next()
            .map(|(_, mapping)| mapping)
    }

    /// How many bytes from `address` on, up to `length`, the program may access as `wanted`.
    pub fn accessible(&self, address: u64, length: u64, wanted: Permissions) -> u64 {
        let end = address.saturating_add(length);
        let mut at = address;
        while at < end {
            let Some(mapping) = self.find(at) else { break };
            if !mapping.page_permissions(at).contains(wanted) {
                break;
            }
            let reach = match &mapping.backing {
                Backing::File { file_end, .. } if at < *file_end => (*file_end).min(mapping.end),
                _ => mapping.end,
            };
            at = reach.min(end);
        }
        at - address
    }

    /// Whether every page of the range is mapped.
    pub fn fully_mapped(&self, start: u64, end: u64) -> bool {
        let mut at = start;
        while at < end {
            match self.find(at) {
                Some(mapping) => at = mapping.end,
                None => return false,
            }
        }
        true
    }

    /// Whether any page of the range is mapped.
    pub fn any_mapped(&self, start: u64, end: u64) -> bool {
        self.next_after(start)
            .is_some_and(|mapping| mapping.start < end)
    }

    /// Free space for `length` bytes, the highest below the mmap base, as Linux's top-down search.
    pub fn find_free(&self, length: u64) -> Option<u64> {
        let mut ceiling = self.mmap_base;
        let floor = self.brk.max(LOWEST_ADDRESS);
        loop {
            let start = ceiling.checked_sub(length)?;
            if start < floor {
                return None;
            }
            // The highest mapping (or stack gap) that overlaps [start, ceiling).
            let blocking = self
                .mappings
                .range(..ceiling)
                .next_back()
                .map(|(_, mapping)| mapping)
                .filter(|mapping| mapping.end > start);
            match blocking {
                Some(mapping) => ceiling = mapping.start,
                None => return Some(start),
            }
        }
    }

    /// Takes a range out of the table, splitting mappings at its ends, and returns what was there.
    pub fn remove(&mut self, start: u64, end: u64) -> Vec<Mapping> {
        let overlapping: Vec<u64> = self
            .mappings
            .range(..end)
            .filter(|(_, mapping)| mapping.end > start)
            .map(|(key, _)| *key)
            .collect();
        let mut removed = Vec::new();
        for key in overlapping {
            let mapping = self.mappings.remove(&key).expect("mapping");
            if mapping.start < start {
                let before = mapping.cut(mapping.start, start);
                self.mappings.insert(before.start, before);
            }
            if mapping.end > end {
                let after = mapping.cut(end, mapping.end);
                self.mappings.insert(after.start, after);
            }
            removed.push(mapping.cut(mapping.start.max(start), mapping.end.min(end)));
        }
        removed
    }

    /// Puts a mapping in the table, replacing whatever it covers, and tells the CPU side. New anonymous
    /// memory is zero: a range that has been used before is cleared first.
    pub fn insert(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        mapping: Mapping,
    ) -> Result<(), Errno> {
        let removed = self.remove(mapping.start, mapping.end);
        let had_code = removed
            .iter()
            .any(|old| old.permissions.contains(Permissions::EXECUTE));
        self.clear_used(cpu, memory, mapping.start, mapping.end)?;
        self.apply(cpu, memory, &mapping)?;
        if had_code && !mapping.permissions.contains(Permissions::EXECUTE) {
            cpu.code_changed(memory, mapping.start, mapping.end - mapping.start, &[]);
        }
        self.mappings.insert(mapping.start, mapping);
        self.merge_around();
        Ok(())
    }

    /// Unmaps a range, as `munmap` does.
    pub fn unmap(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        let removed = self.remove(start, end);
        for old in &removed {
            cpu.set_permissions(memory, old.start, old.end - old.start, Permissions::empty())
                .map_err(errno::from_cpu)?;
            if old.permissions.contains(Permissions::EXECUTE) {
                cpu.code_changed(memory, old.start, old.end - old.start, &[]);
            }
        }
        Ok(())
    }

    /// Changes what a fully mapped range allows, as `mprotect` does.
    pub fn protect(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
        permissions: Permissions,
    ) -> Result<(), Errno> {
        let removed = self.remove(start, end);
        for mut mapping in removed {
            let had_code = mapping.permissions.contains(Permissions::EXECUTE);
            mapping.permissions = permissions;
            self.apply(cpu, memory, &mapping)?;
            if had_code && !permissions.contains(Permissions::EXECUTE) {
                cpu.code_changed(memory, mapping.start, mapping.end - mapping.start, &[]);
            }
            self.mappings.insert(mapping.start, mapping);
        }
        self.merge_around();
        Ok(())
    }

    /// Locks or unlocks a range, every page of which is mapped.
    pub fn set_lock(&mut self, start: u64, end: u64, lock: Lock) {
        for mut mapping in self.remove(start, end) {
            mapping.locked = lock;
            self.mappings.insert(mapping.start, mapping);
        }
        self.merge_around();
    }

    /// Locks or unlocks every mapping.
    pub fn set_lock_all(&mut self, lock: Lock) {
        for mapping in self.mappings.values_mut() {
            mapping.locked = lock;
        }
        self.merge_around();
    }

    /// The pages locked in memory (Linux's `locked_vm`), and those of them within a range.
    pub fn locked_pages(&self, start: u64, end: u64) -> (u64, u64) {
        self.mappings
            .values()
            .filter(|mapping| mapping.locked != Lock::None)
            .fold((0, 0), |(all, within), mapping| {
                let from = mapping.start.max(start);
                let to = mapping.end.min(end);
                let inside = if from < to {
                    (to - from) / PAGE_SIZE
                } else {
                    0
                };
                (
                    all + (mapping.end - mapping.start) / PAGE_SIZE,
                    within + inside,
                )
            })
    }

    /// Whether `pages` more locked pages fit `RLIMIT_MEMLOCK` (Linux's `mlock_future_ok`).
    pub fn may_lock(
        &self,
        limits: &[Limit; LIMITS],
        lock: Lock,
        pages: u64,
        powerful: bool,
    ) -> bool {
        lock == Lock::None
            || powerful
            || self.locked_pages(0, 0).0 + pages
                <= limits[RLIMIT_MEMLOCK as usize].current / PAGE_SIZE
    }

    /// Tells the CPU side what a mapping's pages allow, and that its code may run.
    fn apply(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        mapping: &Mapping,
    ) -> Result<(), Errno> {
        let split = match &mapping.backing {
            Backing::File { file_end, .. } => (*file_end).clamp(mapping.start, mapping.end),
            _ => mapping.end,
        };
        let cpu_permissions = cpu_permissions(mapping.permissions);
        if split > mapping.start {
            cpu.set_permissions(
                memory,
                mapping.start,
                split - mapping.start,
                cpu_permissions,
            )
            .map_err(errno::from_cpu)?;
        }
        if mapping.end > split {
            cpu.set_permissions(memory, split, mapping.end - split, Permissions::empty())
                .map_err(errno::from_cpu)?;
        }
        if mapping.permissions.contains(Permissions::EXECUTE) {
            cpu.code_changed(memory, mapping.start, mapping.end - mapping.start, &[]);
        }
        Ok(())
    }

    /// Zeroes the parts of a range that held something before, and marks it all as used.
    fn clear_used(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        start: u64,
        end: u64,
    ) -> Result<(), Errno> {
        let overlapping: Vec<(u64, u64)> = self
            .used
            .range(..end)
            .filter(|(_, used_end)| **used_end > start)
            .map(|(used_start, used_end)| (*used_start, *used_end))
            .collect();
        let mut low = start;
        let mut high = end;
        for (used_start, used_end) in overlapping {
            let from = used_start.max(start);
            let to = used_end.min(end);
            cpu.zero(memory, from, to - from).map_err(errno::from_cpu)?;
            self.used.remove(&used_start);
            low = low.min(used_start);
            high = high.max(used_end);
        }
        self.used.insert(low, high);
        Ok(())
    }

    /// Joins neighbouring anonymous mappings that are the same, as Linux merges areas.
    fn merge_around(&mut self) {
        let mut merged: BTreeMap<u64, Mapping> = BTreeMap::new();
        for (_, mapping) in core::mem::take(&mut self.mappings) {
            if let Some((_, last)) = merged.iter_mut().next_back()
                && last.end == mapping.start
                && last.permissions == mapping.permissions
                && last.shared == mapping.shared
                && last.locked == mapping.locked
                && matches!(
                    (&last.backing, &mapping.backing),
                    (Backing::Anonymous, Backing::Anonymous)
                        | (Backing::Heap, Backing::Heap)
                        | (Backing::Stack, Backing::Stack)
                )
            {
                last.end = mapping.end;
                continue;
            }
            merged.insert(mapping.start, mapping);
        }
        self.mappings = merged;
    }

    /// Grows a stack mapping down to cover `address` if it lies in the stack's reach, as Linux does on
    /// a fault below the stack. Returns whether it grew.
    pub fn grow_stack(
        &mut self,
        cpu: &mut dyn Cpu,
        memory: MemoryId,
        address: u64,
        limits: &[Limit; LIMITS],
    ) -> Result<bool, Errno> {
        let Some(stack) = self
            .mappings
            .range(address..)
            .next()
            .map(|(_, mapping)| mapping.clone())
            .filter(|mapping| matches!(mapping.backing, Backing::Stack))
        else {
            return Ok(false);
        };
        let new_start = page_down(address);
        // Linux's `acct_stack_growth`: the stack's size within its limit, the growth within the
        // address space's.
        if stack.end - new_start > limits[RLIMIT_STACK as usize].current
            || !self.may_expand(limits, false, (stack.start - new_start) / PAGE_SIZE, (0, 0))
        {
            return Ok(false);
        }
        // Linux keeps the guard gap free below a growing stack.
        let below = self
            .mappings
            .range(..stack.start)
            .next_back()
            .map(|(_, mapping)| mapping.end)
            .unwrap_or(0);
        if below.saturating_add(STACK_GUARD_GAP) > new_start {
            return Ok(false);
        }
        let grown = Mapping {
            start: new_start,
            end: stack.start,
            ..stack
        };
        self.insert(cpu, memory, grown)?;
        Ok(true)
    }
}

/// What the CPU side enforces for what a program asked: on ARM64 writing implies reading, and so does
/// running (no execute-only pages).
pub(crate) fn cpu_permissions(asked: Permissions) -> Permissions {
    if asked.is_empty() {
        asked
    } else {
        asked | Permissions::READ
    }
}
