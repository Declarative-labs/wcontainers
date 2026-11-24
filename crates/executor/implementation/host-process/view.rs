//! A stub's host mappings of its guest area as its queued commands leave them, counted as the host counts them, so a
//! change that would pass the host's limit is refused before the host refuses it (plan 4.2).

use rangemap::RangeMap;
use rustix::mm::ProtFlags;
use wcontainers_abi::SharedId;

use crate::pages::Command;

/// What one host mapping shows; neighbours with equal values are one mapping, as the host merges them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mapping {
    protection: ProtFlags,
    /// The shared block shown and its offset less the address; none for the memory's own memfd at the address.
    block: Option<(SharedId, u64)>,
}

/// A stub's guest-area mappings, its code area's and its slot table's.
#[derive(Debug, Default)]
pub struct View {
    mappings: RangeMap<u64, Mapping>,
    count: usize,
    code_mapped: bool,
    slots_mapped: bool,
}

impl View {
    /// The host mappings of the guest area, the code area and the slot table.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Records what a command does to the mappings; `block` names a `MapShared`'s block.
    pub fn apply(&mut self, command: &Command, block: Option<SharedId>) {
        let (address, length) = match *command {
            Command::Map {
                address, length, ..
            }
            | Command::Protect {
                address, length, ..
            }
            | Command::Unmap { address, length }
            | Command::MapShared {
                address, length, ..
            } => (address as u64, length as u64),
            Command::Place { .. } | Command::NewThread { .. } => return,
            Command::MapCode => {
                self.count += usize::from(!self.code_mapped);
                self.code_mapped = true;
                return;
            }
            Command::MapSlots => {
                self.count += usize::from(!self.slots_mapped);
                self.slots_mapped = true;
                return;
            }
        };
        if length == 0 {
            return;
        }
        let range = address..address + length;
        // The neighbours a change may merge with or split from.
        let around = range.start.saturating_sub(1)..range.end.saturating_add(1);
        let before = self.mappings.overlapping(around.clone()).count();
        match *command {
            Command::Map { permissions, .. } => self.mappings.insert(
                range,
                Mapping {
                    protection: permissions,
                    block: None,
                },
            ),
            Command::MapShared {
                permissions,
                offset,
                ..
            } => self.mappings.insert(
                range,
                Mapping {
                    protection: permissions,
                    block: Some((
                        block.expect("a block for every shared mapping"),
                        (offset as u64).wrapping_sub(address),
                    )),
                },
            ),
            Command::Protect { permissions, .. } => {
                let changed: Vec<_> = self
                    .mappings
                    .overlapping(range.clone())
                    .map(|(held, mapping)| {
                        (
                            held.start.max(range.start)..held.end.min(range.end),
                            mapping.block,
                        )
                    })
                    .collect();
                for (piece, block) in changed {
                    self.mappings.insert(
                        piece,
                        Mapping {
                            protection: permissions,
                            block,
                        },
                    );
                }
            }
            Command::Unmap { .. } => self.mappings.remove(range),
            Command::Place { .. }
            | Command::NewThread { .. }
            | Command::MapCode
            | Command::MapSlots => {}
        }
        self.count = self.count + self.mappings.overlapping(around).count() - before;
    }
}
