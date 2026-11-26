//! The owner's `MemorySnapshot::diff` exactly as he wrote it on his main, kept here only as the reference the proven
//! `diff` is checked against: on many random pairs of snapshots both must send the same mutations in the same order.

extern crate std;

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::ops::Range;
use core::ptr;

use rangemap::RangeMap;

use crate::{MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture};

/// An architecture with nothing but memory, to build snapshots of.
#[derive(Clone, PartialEq, Eq)]
struct MemoryOnly;

impl ProcessorArchitecture for MemoryOnly {
    type ProcessorState = ();
    type ProcessorStateExpectation = ();
    type PagePermission = u8;
    type Instruction = ();
    type PreciseInstruction = ();
    type Instructions = core::iter::Empty<()>;
    type KnownBehaviors = core::iter::Empty<()>;

    const ADDRESSABLE: Range<u64> = 0..u64::MAX;
    const DEFAULT_PERMISSION: u8 = 0;

    fn list_instructions() -> Self::Instructions {
        core::iter::empty()
    }

    fn list_known_behaviors((): ()) -> Self::KnownBehaviors {
        core::iter::empty()
    }

    fn meets_expectations((): &(), (): &()) -> bool {
        true
    }
}

/// The owner's method under another name, so that both can be called on the same snapshots.
trait OwnersDiff<Architecture: ProcessorArchitecture> {
    fn diff(&self, new: &Self, mutation_hook: &mut impl MemorySink<Architecture>);
}

impl<Architecture: ProcessorArchitecture> OwnersDiff<Architecture>
    for MemorySnapshot<'_, '_, Architecture>
{
    fn diff(&self, new: &Self, mutation_hook: &mut impl MemorySink<Architecture>) {
        let (old, new) = (self.mutations, new.mutations);
        /// Returns whether the two mutations assign the same thing to the same addresses, the same slice of bytes first.
        fn is_same_mutation<Architecture: ProcessorArchitecture>(
            first: &MemoryDelta<'_, Architecture>,
            second: &MemoryDelta<'_, Architecture>,
        ) -> bool {
            match (first, second) {
                (
                    MemoryDelta::PermissionAssignment {
                        addresses: first_addresses,
                        permission: first_permission,
                    },
                    MemoryDelta::PermissionAssignment {
                        addresses: second_addresses,
                        permission: second_permission,
                    },
                ) => first_addresses == second_addresses && first_permission == second_permission,
                (
                    MemoryDelta::ValueAssignment {
                        start: first_start,
                        bytes: first_bytes,
                    },
                    MemoryDelta::ValueAssignment {
                        start: second_start,
                        bytes: second_bytes,
                    },
                ) => {
                    first_start == second_start
                        && (ptr::eq(&**first_bytes, &**second_bytes) || first_bytes == second_bytes)
                }
                _ => false,
            }
        }

        /// Records the addresses an unpaired mutation reaches: the bytes it writes, or that it assigns a permission.
        // VERUS: no closure that captures a mutable reference, so what is recorded is passed in.
        fn add_unpaired<Architecture: ProcessorArchitecture>(
            mutation: &MemoryDelta<'_, Architecture>,
            written: &mut Vec<Range<u128>>,
            has_unpaired_permission: &mut bool,
        ) {
            match mutation {
                MemoryDelta::ValueAssignment { start, bytes } => {
                    let end = u128::from(*start) + bytes.len() as u128;
                    // A write that runs past the last address goes on from zero, in a second range.
                    let ranges_around_wrap = [
                        u128::from(*start)..end.min(ADDRESSES_END),
                        0..end.saturating_sub(ADDRESSES_END),
                    ];
                    written.extend(
                        ranges_around_wrap
                            .into_iter()
                            .filter(|addresses| !addresses.is_empty()),
                    );
                }
                MemoryDelta::PermissionAssignment { addresses, .. } => {
                    *has_unpaired_permission |= !addresses.is_empty();
                }
            }
        }

        /// Writes into `rendered` what the mutations leave at its addresses from `start` on, which is zero where none wrote.
        fn resolve_bytes<Architecture: ProcessorArchitecture>(
            mutations: &[MemoryDelta<'_, Architecture>],
            start: u64,
            length: usize,
            rendered: &mut Vec<u8>,
        ) {
            rendered.clear();
            rendered.resize(length, u8::MIN);
            for mutation in mutations {
                if let MemoryDelta::ValueAssignment {
                    start: mutation_start,
                    bytes,
                } = mutation
                {
                    // Addresses wrap after the last one, so the offsets are taken modulo 2^64.
                    let rendered_start_in_bytes = start.wrapping_sub(*mutation_start);
                    let bytes_start_in_rendered = mutation_start.wrapping_sub(start);
                    if rendered_start_in_bytes < bytes.len() as u64 {
                        let copied = (bytes.len() - rendered_start_in_bytes as usize).min(length);
                        rendered[..copied]
                            .copy_from_slice(&bytes[rendered_start_in_bytes as usize..][..copied]);
                    } else if bytes_start_in_rendered < length as u64 {
                        let copied = bytes.len().min(length - bytes_start_in_rendered as usize);
                        rendered[bytes_start_in_rendered as usize..][..copied]
                            .copy_from_slice(&bytes[..copied]);
                    }
                }
            }
        }

        /// Returns the permission of every range of addresses once the mutations are applied, keyed by address range.
        fn resolve_permissions<Architecture: ProcessorArchitecture>(
            mutations: &[MemoryDelta<'_, Architecture>],
        ) -> RangeMap<u64, Architecture::PagePermission> {
            let mut rendered = RangeMap::new();
            for mutation in mutations {
                if let MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } = mutation
                    && !addresses.is_empty()
                {
                    rendered.insert(addresses.clone(), *permission);
                }
            }
            rendered
        }

        // Mutations both lists make in the same order leave the same memory wherever no other mutation reaches, so only
        // what the unpaired mutations reach can differ.
        const ADDRESSES_END: u128 = 1 << u64::BITS;
        let (mut written, mut has_unpaired_permission) = (Vec::new(), false);
        let mut rest = new;
        for mutation in old {
            match rest
                .iter()
                .position(|other| is_same_mutation(mutation, other))
            {
                Some(index) => {
                    for skipped in &rest[..index] {
                        add_unpaired(skipped, &mut written, &mut has_unpaired_permission);
                    }
                    rest = &rest[index + 1..];
                }
                None => add_unpaired(mutation, &mut written, &mut has_unpaired_permission),
            }
        }
        for unpaired in rest {
            add_unpaired(unpaired, &mut written, &mut has_unpaired_permission);
        }
        written.sort_unstable_by_key(|addresses| addresses.start);
        // Overlapping and touching ranges become one, so that each address is compared once.
        written.dedup_by(|addresses, merged| {
            let touching = addresses.start <= merged.end;
            if touching {
                merged.end = merged.end.max(addresses.end);
            }
            touching
        });
        let (mut old_bytes, mut new_bytes) = (Vec::new(), Vec::new());
        for addresses in written {
            let start = addresses.start as u64;
            let length = (addresses.end - addresses.start) as usize;
            resolve_bytes(old, start, length, &mut old_bytes);
            resolve_bytes(new, start, length, &mut new_bytes);
            let is_same_at = |offset: &usize| old_bytes[*offset] == new_bytes[*offset];
            // Each run of bytes the two lists leave different is one mutation.
            let mut run_end = usize::MIN;
            while let Some(run_start) = (run_end..length).find(|offset| !is_same_at(offset)) {
                run_end = (run_start..length).find(is_same_at).unwrap_or(length);
                mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                    start: start + run_start as u64,
                    bytes: Cow::Borrowed(&new_bytes[run_start..run_end]),
                });
            }
        }
        if !has_unpaired_permission {
            return;
        }
        let old_permissions = resolve_permissions(old);
        let new_permissions = resolve_permissions(new);
        let mut bounds: Vec<u64> = old_permissions
            .iter()
            .chain(new_permissions.iter())
            .flat_map(|(addresses, _)| [addresses.start, addresses.end])
            .collect();
        bounds.sort_unstable();
        bounds.dedup();
        let read_permission = |permissions: &RangeMap<u64, Architecture::PagePermission>,
                               address| {
            *permissions
                .get(&address)
                .unwrap_or(&Architecture::DEFAULT_PERMISSION)
        };
        // Between two neighboring bounds each list leaves one permission.
        let mut bounds = bounds.into_iter();
        let Some(mut start) = bounds.next() else {
            return;
        };
        for end in bounds {
            let addresses = start..end;
            start = end;
            let permission = read_permission(&new_permissions, addresses.start);
            if read_permission(&old_permissions, addresses.start) != permission {
                mutation_hook.on_memory_mutated(MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                });
            }
        }
    }
}

/// A seeded xorshift, so a failing case can be rerun.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    /// An address near zero, near the last address, or in a small window in between.
    fn address(&mut self) -> u64 {
        let near = self.below(24);
        match self.below(3) {
            0 => near,
            1 => u64::MAX - near,
            _ => (1 << 40) + near,
        }
    }
}

/// A random list of mutations whose bytes are owned or borrowed from the pool.
fn random_mutations<'pool>(
    random: &mut Random,
    pool: &'pool [Vec<u8>],
) -> Vec<MemoryDelta<'pool, MemoryOnly>> {
    (0..random.below(8))
        .map(|_| random_mutation(random, pool))
        .collect()
}

fn random_mutation<'pool>(
    random: &mut Random,
    pool: &'pool [Vec<u8>],
) -> MemoryDelta<'pool, MemoryOnly> {
    if random.below(5) < 3 {
        let bytes = if random.below(2) == 0 {
            Cow::Borrowed(pool[random.below(pool.len() as u64) as usize].as_slice())
        } else {
            Cow::Owned(
                (0..random.below(7))
                    .map(|_| random.below(3) as u8)
                    .collect(),
            )
        };
        MemoryDelta::ValueAssignment {
            start: random.address(),
            bytes,
        }
    } else {
        let start = random.address();
        // Mostly short ranges from the start; sometimes another address, which may lie before it.
        let end = if random.below(4) == 0 {
            random.address()
        } else {
            start.saturating_add(random.below(9))
        };
        MemoryDelta::PermissionAssignment {
            addresses: start..end,
            permission: random.below(3) as u8,
        }
    }
}

/// A list made from the old one by a few edits, so that many mutations pair; or a list of its own.
fn random_successor<'pool>(
    random: &mut Random,
    pool: &'pool [Vec<u8>],
    old: &[MemoryDelta<'pool, MemoryOnly>],
) -> Vec<MemoryDelta<'pool, MemoryOnly>> {
    if random.below(6) == 0 {
        return random_mutations(random, pool);
    }
    let mut new: Vec<MemoryDelta<'pool, MemoryOnly>> = old.to_vec();
    for _ in 0..random.below(4) {
        let at = random.below(new.len() as u64 + 1) as usize;
        match random.below(5) {
            0 => new.insert(at, random_mutation(random, pool)),
            1 if at < new.len() => drop(new.remove(at)),
            2 if at < new.len() => new[at] = random_mutation(random, pool),
            3 if at + 1 < new.len() => new.swap(at, at + 1),
            // The same mutation with its bytes copied, so that it pairs by value, not by pointer.
            _ if at < new.len() => {
                let copied = new[at].clone().into_owned();
                new[at] = copied;
            }
            _ => {}
        }
    }
    new
}

/// What the mutations sent hash to, for the record of a run.
fn fold_hash(hash: &mut u64, mutations: &[MemoryDelta<'static, MemoryOnly>]) {
    let mut add = |value: u64| *hash = (*hash ^ value).wrapping_mul(0x0000_0100_0000_01b3);
    for mutation in mutations {
        match mutation {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => {
                add(1);
                add(addresses.start);
                add(addresses.end);
                add(u64::from(*permission));
            }
            MemoryDelta::ValueAssignment { start, bytes } => {
                add(2);
                add(*start);
                add(bytes.len() as u64);
                bytes.iter().for_each(|&byte| add(u64::from(byte)));
            }
        }
    }
}

/// The proven `diff` sends exactly what the owner's sends, on every random pair.
#[test]
fn diff_sends_what_the_owners_diff_sends() {
    let pairs: u64 = std::env::var("DIFF_EQUIVALENCE_PAIRS")
        .ok()
        .and_then(|pairs| pairs.parse().ok())
        .unwrap_or(500_000);
    let seed: u64 = std::env::var("DIFF_EQUIVALENCE_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or(0x5eed_d1ff);
    let mut random = Random(seed);
    let pool: Vec<Vec<u8>> = (0..6)
        .map(|_| {
            (0..random.below(7))
                .map(|_| random.below(3) as u8)
                .collect()
        })
        .collect();
    let (mut sent, mut pairs_with_changes, mut hash) = (0_usize, 0_u64, 0xcbf2_9ce4_8422_2325_u64);
    for pair in 0..pairs {
        let old = random_mutations(&mut random, &pool);
        let new = random_successor(&mut random, &pool, &old);
        let (old_snapshot, new_snapshot) = (
            MemorySnapshot { mutations: &old },
            MemorySnapshot { mutations: &new },
        );
        let mut proven: Vec<MemoryDelta<'static, MemoryOnly>> = Vec::new();
        old_snapshot.diff(&new_snapshot, &mut proven);
        let mut owners: Vec<MemoryDelta<'static, MemoryOnly>> = Vec::new();
        OwnersDiff::diff(&old_snapshot, &new_snapshot, &mut owners);
        assert!(
            proven == owners,
            "pair {pair} (seed {seed:#x}): the two diffs sent different mutations"
        );
        sent += proven.len();
        pairs_with_changes += u64::from(!proven.is_empty());
        fold_hash(&mut hash, &proven);
    }
    std::println!(
        "diff equivalence: {pairs} pairs (seed {seed:#x}), {pairs_with_changes} with changes, {sent} mutations sent, hash {hash:016x}"
    );
}
