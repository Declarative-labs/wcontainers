//! The shared memory each process maps, announced to the embedder as it changes.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::ops::Range;

use wcontainers_abi::Permissions;

use crate::memory::{Backing, Mapping};
use crate::process::Pid;
use crate::system::{
    AnonymousMemoryId, DeviceId, InodeNumber, MemoryObject, Process, SystemVSegmentId,
};
use crate::{Context, Kernel};

/// What one process maps of one source, in address order: pieces of its memory, each with where
/// in the source it starts, joined where one continues the other in both.
pub(crate) type Pieces = Vec<(u64, Range<u64>)>;

/// What the embedder has been told each process maps, by source and process.
pub(crate) type Announced = BTreeMap<(MemoryObject, Pid), Pieces>;

/// The source a shared mapping shows, and where in it the mapping starts.
fn source_of(mapping: &Mapping) -> Option<(MemoryObject, u64)> {
    if !mapping.shared {
        return None;
    }
    match &mapping.backing {
        Backing::File { node, offset, .. } => {
            let node = node.borrow();
            let source = if node.is_shared_anonymous() {
                MemoryObject::Anonymous(AnonymousMemoryId(node.ino))
            } else if node.is_segment() {
                // A segment's inode number is its id, as Linux's `newseg` sets it.
                MemoryObject::SystemVSegment(SystemVSegmentId(
                    i32::try_from(node.ino).expect("a segment id"),
                ))
            } else {
                MemoryObject::File {
                    device: DeviceId(node.fs.device),
                    inode: InodeNumber(node.ino),
                }
            };
            Some((source, *offset))
        }
        _ => None,
    }
}

/// The parts of `pieces` that `others` do not map at the same place in the source.
fn missing_from(pieces: &[(u64, Range<u64>)], others: &[(u64, Range<u64>)]) -> Pieces {
    let mut missing = Vec::new();
    for (offset, range) in pieces {
        let shift = range.start.wrapping_sub(*offset);
        let mut left = alloc::vec![range.clone()];
        for (other_offset, other) in others {
            if other.start.wrapping_sub(*other_offset) != shift {
                continue;
            }
            left = left
                .into_iter()
                .flat_map(|part| {
                    let before = part.start..part.end.min(other.start);
                    let after = part.start.max(other.end)..part.end;
                    [before, after]
                })
                .filter(|part| !part.is_empty())
                .collect();
        }
        missing.extend(
            left.into_iter()
                .map(|part| (part.start.wrapping_sub(shift), part)),
        );
    }
    missing
}

impl Kernel {
    /// What every live process maps of the sources to announce: those some mapping may write
    /// through (Linux's `VM_MAYWRITE`), and those announced already while anything maps them.
    fn shared_mapped(&self) -> Announced {
        let mut mapped = Announced::new();
        let mut writable = BTreeSet::new();
        for process in self.processes.values() {
            if process.ended.is_some() || self.sharing_left.contains(&process.pid) {
                continue;
            }
            for mapping in process.map.borrow().iter() {
                let Some((source, offset)) = source_of(mapping) else {
                    continue;
                };
                if mapping.allowed.contains(Permissions::WRITE) {
                    writable.insert(source);
                }
                let pieces = mapped.entry((source, process.pid)).or_default();
                match pieces.last_mut() {
                    Some((start, range))
                        if range.end == mapping.start
                            && start.wrapping_add(range.end - range.start) == offset =>
                    {
                        range.end = mapping.end;
                    }
                    _ => pieces.push((offset, mapping.start..mapping.end)),
                }
            }
        }
        mapped.retain(|(source, _), _| {
            writable.contains(source)
                || self
                    .shared_announced
                    .range((*source, Pid::MIN)..=(*source, Pid::MAX))
                    .next()
                    .is_some()
        });
        mapped
    }

    /// Tells the embedder what changed in the shared memory processes map: every end first,
    /// then every start, each in the process it is about. What it was told is kept in step piece
    /// by piece, so a process a hook takes off the kernel carries exactly that, and is told the
    /// rest when it is back.
    pub(crate) fn announce_sharing(&mut self, cx: &mut Context<'_>) {
        self.map_notes.sharing_changed.set(false);
        let mapped = self.shared_mapped();
        if mapped.is_empty() && self.shared_announced.is_empty() {
            return;
        }
        self.sharing_round = self.sharing_round.wrapping_add(1);
        let round = self.sharing_round;
        let none = Pieces::new();
        let mut changes = Vec::new();
        for (key, pieces) in &self.shared_announced {
            let now = mapped.get(key).unwrap_or(&none);
            changes.extend(
                missing_from(pieces, now)
                    .into_iter()
                    .map(|piece| (false, *key, piece)),
            );
        }
        for (key, pieces) in &mapped {
            let before = self.shared_announced.get(key).unwrap_or(&none);
            changes.extend(
                missing_from(pieces, before)
                    .into_iter()
                    .map(|piece| (true, *key, piece)),
            );
        }
        for (start, key @ (source, pid), (offset, range)) in changes {
            self.announce_kernel_writes(cx);
            // A process taken off the kernel in an earlier hook took what it was told with it.
            if !self.processes.contains_key(&pid) {
                continue;
            }
            let told = self.shared_announced.entry(key).or_default();
            if start {
                let at = told.partition_point(|(_, other)| other.start < range.start);
                told.insert(at, (offset, range.clone()));
            } else {
                *told = missing_from(told, &[(offset, range.clone())]);
                if told.is_empty() {
                    self.shared_announced.remove(&key);
                }
            }
            self.announce_shared(cx, pid, |system, process| {
                if start {
                    system.on_shared_memory_mapping(process, source, offset, range)
                } else {
                    system.on_shared_memory_unmapped(process, source, offset, range)
                }
            });
            // A hook that ended a process had everything announced again then.
            if self.sharing_round != round {
                return;
            }
        }
        let here = &self.processes;
        self.shared_announced = mapped;
        self.shared_announced
            .retain(|(_, pid), _| here.contains_key(pid));
    }

    /// A process about to go: what it maps of shared memory is announced ended while it is
    /// whole, and it is left out from then on.
    pub(crate) fn withdraw_sharing(&mut self, cx: &mut Context<'_>, pid: Pid) {
        if self
            .shared_announced
            .keys()
            .any(|(_, mapper)| *mapper == pid)
        {
            self.sharing_left.insert(pid);
            self.announce_sharing(cx);
        }
    }

    /// Announces one hook of the shared memory a process maps.
    fn announce_shared(
        &mut self,
        cx: &mut Context<'_>,
        pid: Pid,
        hook: impl FnOnce(&dyn crate::System, Process<'_>),
    ) {
        let process = Process {
            kernel: self,
            cpu: &mut *cx.cpu,
            pid,
        };
        hook(cx.host, process);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_missing_keeps_the_place_in_the_source() {
        let page = wcontainers_abi::PAGE_SIZE;
        let whole = [(page, 2 * page..6 * page)];
        // The middle unmapped: two pieces left, the second further into the source.
        let left = [(page, 2 * page..3 * page), (4 * page, 5 * page..6 * page)];
        assert_eq!(
            missing_from(&whole, &left),
            [(2 * page, 3 * page..5 * page)]
        );
        assert_eq!(missing_from(&left, &whole), []);
        // The same addresses at another place in the source are not the same mapping.
        let moved = [(0, 2 * page..6 * page)];
        assert_eq!(missing_from(&whole, &moved), whole);
    }
}
