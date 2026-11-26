//! `MemorySnapshot::diff` with its proof: the owner's code, changed only where a `// VERUS:` line says why.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::ops::Range;

use rangemap::RangeMap;
#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::slice::{cmp_le, sorted_by_le};

use crate::trusted::*;
use crate::*;

// The owner's diff as he wrote it, which the one here must agree with.
#[cfg(test)]
#[path = "diff_equivalence_tests.rs"]
mod diff_equivalence_tests;

verus! {

impl<Architecture: ProcessorArchitecture> MemorySnapshot<'_, '_, Architecture> {
    /// Calls the hook with each mutation that turns this snapshot into the new one, bytes before permissions.
    #[verifier::spinoff_prover]
    pub fn diff(&self, new: &Self, mutation_hook: &mut impl MemorySink<Architecture>)
        ensures
            exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(memory_of(self.mutations@), changes)]
                receives(*old(mutation_hook), *final(mutation_hook), changes)
                    && apply_all(memory_of(self.mutations@), changes) == memory_of(new.mutations@),
            memory_of(self.mutations@) == memory_of(new.mutations@) ==> *final(mutation_hook) == *old(mutation_hook),
    {
        let (old, new) = (self.mutations, new.mutations);
        /// Returns whether the two mutations assign the same thing to the same addresses, the same slice of bytes first.
        fn is_same_mutation<Architecture: ProcessorArchitecture>(
            first: &MemoryDelta<'_, Architecture>,
            second: &MemoryDelta<'_, Architecture>,
        ) -> (same: bool)
            requires
                obeys_concrete_eq::<Architecture::PagePermission>(),
            ensures
                same ==> first@ == second@,
        {
            proof {
                reveal(obeys_concrete_eq);
            }
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
                ) => {
                    // VERUS: no `==` on ranges, so their bounds are compared.
                    first_addresses.start == second_addresses.start && first_addresses.end == second_addresses.end
                        && first_permission == second_permission
                }
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
                    // VERUS: no deref of a `Cow`, so each one's two cases are matched.
                    let first_bytes: &[u8] = match first_bytes { Cow::Borrowed(borrowed) => borrowed, Cow::Owned(owned) => owned.as_slice() };
                    let second_bytes: &[u8] = match second_bytes { Cow::Borrowed(borrowed) => borrowed, Cow::Owned(owned) => owned.as_slice() };
                    // VERUS: no `ptr::eq`, so it is trusted.
                    let same = first_start == second_start && (is_same_slice(first_bytes, second_bytes) || first_bytes == second_bytes);
                    proof {
                        if same {
                            assert(first_bytes@ =~= second_bytes@);
                        }
                    }
                    same
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
        )
            ensures
                records_unpaired(mutation@, vstd::prelude::old(written)@, *vstd::prelude::old(has_unpaired_permission), final(written)@, *final(has_unpaired_permission)),
        {
            match mutation {
                MemoryDelta::ValueAssignment { start, bytes } => {
                    proof {
                        assert(ADDRESSES_END == 0x1_0000_0000_0000_0000) by (compute_only);
                    }
                    // VERUS: no deref of a `Cow`, so its two cases are matched.
                    let bytes: &[u8] = match bytes { Cow::Borrowed(borrowed) => borrowed, Cow::Owned(owned) => owned.as_slice() };
                    let ghost before = written@;
                    let end = u128::from(*start) + bytes.len() as u128;
                    // A write that runs past the last address goes on from zero, in a second range.
                    let ranges_around_wrap = [
                        u128::from(*start)..end.min(ADDRESSES_END),
                        0..end.saturating_sub(ADDRESSES_END),
                    ];
                    // VERUS: no `Vec::extend`, no array `into_iter` and no `clone` of a range that says what it gives (each
                    // measured), so each of the two ranges that is not empty is pushed by its bounds.
                    if !ranges_around_wrap[0].is_empty() {
                        written.push(ranges_around_wrap[0].start..ranges_around_wrap[0].end);
                    }
                    let ghost below_wrap = written@;
                    if !ranges_around_wrap[1].is_empty() {
                        written.push(ranges_around_wrap[1].start..ranges_around_wrap[1].end);
                    }
                    proof {
                        // Recording the one or two ranges around the last address records every byte the write reaches.
                        let after = written@;
                        let bytes = bytes@;
                        let start = *start;
                        let end = start + bytes.len();
                        let below_end = if end < address_count() { end } else { address_count() };
                        let change = Assignment::<Architecture::PagePermission>::ValueAssignment { start, bytes };
                        assert forall|address: u64| #[trigger] writes_byte(change, address) implies in_ranges(after, address) by {
                            if start <= address {
                                let at = before.len() as int;
                                assert(after[at] == below_wrap[at]);
                            } else {
                                let at = below_wrap.len() as int;
                                assert(after[at] == (0u128..((end - address_count()) as u128)));
                                assert(after[at].start <= address < after[at].end);
                            }
                        }
                    }
                }
                MemoryDelta::PermissionAssignment { addresses, .. } => {
                    *has_unpaired_permission |= !addresses.is_empty();
                    proof {
                        assert(written@.is_prefix_of(written@)) by {
                            assert(written@.take(written@.len() as int) =~= written@);
                        }
                    }
                }
            }
        }

        /// Writes into `rendered` what the mutations leave at its addresses from `start` on, which is zero where none wrote.
        fn resolve_bytes<Architecture: ProcessorArchitecture>(
            mutations: &[MemoryDelta<'_, Architecture>],
            start: u64,
            length: usize,
            rendered: &mut Vec<u8>,
        )
            requires
                start as int + length as int <= address_count(),
            ensures
                final(rendered)@.len() == length,
                bytes_resolved(memory_of(mutations@), final(rendered)@, start),
        {
            /// Copies `count` bytes from `from` on into the buffer from `at` on.
            #[inline]
            fn copy_into(buffer: &mut Vec<u8>, at: usize, bytes: &[u8], from: usize, count: usize)
                requires
                    at + count <= vstd::prelude::old(buffer)@.len(),
                    from + count <= bytes@.len(),
                    at + count <= usize::MAX,
                    from + count <= usize::MAX,
                ensures
                    final(buffer)@.len() == vstd::prelude::old(buffer)@.len(),
                    forall|index: int| 0 <= index < vstd::prelude::old(buffer)@.len() ==> #[trigger] final(buffer)@[index]
                        == if at <= index < at + count { bytes@[from + index - at] } else { vstd::prelude::old(buffer)@[index] },
            {
                buffer.as_mut_slice()[at..at + count].copy_from_slice(&bytes[from..from + count]);
            }

            rendered.clear();
            rendered.resize(length, u8::MIN);
            for mutation in walking: mutations
                invariant
                    walking.seq() == mutations@.as_ref(),
                    rendered@.len() == length,
                    start as int + length as int <= address_count(),
                    bytes_resolved(memory_of(mutations@.take(walking.index())), rendered@, start),
            {
                let ghost before = rendered@;
                let ghost index = walking.index();
                if let MemoryDelta::ValueAssignment {
                    start: mutation_start,
                    bytes,
                } = mutation
                {
                    // VERUS: no deref of a `Cow`, so its two cases are matched.
                    let bytes: &[u8] = match bytes { Cow::Borrowed(borrowed) => borrowed, Cow::Owned(owned) => owned.as_slice() };
                    // Addresses wrap after the last one, so the offsets are taken modulo 2^64.
                    let rendered_start_in_bytes = start.wrapping_sub(*mutation_start);
                    let bytes_start_in_rendered = mutation_start.wrapping_sub(start);
                    if rendered_start_in_bytes < bytes.len() as u64 {
                        let copied = (bytes.len() - rendered_start_in_bytes as usize).min(length);
                        // VERUS: a copy into a range of a `Vec` is more than the verifier checks reliably in this loop (it ran out
                        // of its limit under two of three seeds), so each copy is a function of its own.
                        copy_into(rendered, 0, bytes, rendered_start_in_bytes as usize, copied);
                    }
                    let ghost after_first = rendered@;
                    // VERUS: proven only with this: a write that wraps past the last address can reach the end of the window too.
                    if bytes_start_in_rendered != 0 && bytes_start_in_rendered < length as u64 {
                        let copied = bytes.len().min(length - bytes_start_in_rendered as usize);
                        // VERUS: a copy into a range of a `Vec` is more than the verifier checks reliably in this loop, so each
                        // copy is a function of its own.
                        copy_into(rendered, bytes_start_in_rendered as usize, bytes, 0, copied);
                    }
                    proof {
                        // What the one or two copies leave: the write's byte wherever it reaches, the window's byte before
                        // everywhere else.
                        let (after, written_start, bytes, length) = (rendered@, *mutation_start, bytes@, length as int);
                        let window_in_bytes = offset(written_start, start);
                        let bytes_in_window = offset(start, written_start);
                    }
                }
                proof {
                    // The window, rewritten where the next mutation writes, is resolved for one more mutation.
                    let after = rendered@;
                    lemma_memory_of_take(mutations@, index);
                    let memory = memory_of(mutations@.take(index));
                    let change = mutations@[index]@;
                }
            }
            proof {
                assert(mutations@.take(mutations@.len() as int) =~= mutations@);
            }
        }

        /// Returns the permission of every range of addresses once the mutations are applied, keyed by address range.
        fn resolve_permissions<Architecture: ProcessorArchitecture>(
            mutations: &[MemoryDelta<'_, Architecture>],
        ) -> (rendered: RangeMap<u64, Architecture::PagePermission>)
            ensures
                permissions_resolved::<Architecture>(memory_of(mutations@), rendered),
        {
            proof {
                Architecture::equality_is_identity();
            }
            // VERUS: no `RangeMap`, so its `new` and `insert` are trusted.
            let mut rendered = new_range_map();
            for mutation in walking: mutations
                invariant
                    walking.seq() == mutations@.as_ref(),
                    obeys_concrete_eq::<Architecture::PagePermission>(),
                    permissions_resolved::<Architecture>(memory_of(mutations@.take(walking.index())), rendered),
            {
                let ghost before = rendered;
                let ghost index = walking.index();
                // VERUS: no `let` in a condition joined by `&&`, so the empty check is a guard.
                match mutation {
                    MemoryDelta::PermissionAssignment {
                        addresses,
                        permission,
                    } if !addresses.is_empty() => {
                        insert_range(&mut rendered, addresses, *permission);
                    },
                    _ => {},
                }
                proof {
                    lemma_memory_of_take(mutations@, index);
                    let memory = memory_of(mutations@.take(index));
                }
            }
            proof {
                assert(mutations@.take(mutations@.len() as int) =~= mutations@);
            }
            rendered
        }

        // Mutations both lists make in the same order leave the same memory wherever no other mutation reaches, so only
        // what the unpaired mutations reach can differ.
        const ADDRESSES_END: u128 = 1 << u64::BITS;
        let ghost hook_before = *mutation_hook;
        let ghost memory_old = memory_of(old@);
        let ghost memory_new = memory_of(new@);
        proof {
            Architecture::equality_is_identity();
        }
        let (mut written, mut has_unpaired_permission) = (Vec::new(), false);
        let mut rest = new;
        let ghost mut rest_start: int = 0;
        for mutation in pairing: old
            invariant
                0 <= rest_start <= new@.len(),
                // `rest` is the new list from `rest_start` on.
                rest@ == new@.skip(rest_start),
                pairing.seq() == old@.as_ref(),
                obeys_concrete_eq::<Architecture::PagePermission>(),
                well_formed(written@),
                same_outside(memory_of(old@.take(pairing.index())), memory_of(new@.take(rest_start)), written@, has_unpaired_permission),
        {
            let ghost unpaired_written = written@;
            let ghost unpaired_flag = has_unpaired_permission;
            let ghost mut paired = false;
            match rest
                .iter()
                .position(|other| -> (same: bool)
                    requires
                        obeys_concrete_eq::<Architecture::PagePermission>(),
                    ensures
                        same ==> mutation@ == other@,
                { is_same_mutation(mutation, other) })
            {
                Some(index) => {
                    let ghost candidate = rest_start + index;
                    let ghost paired_written = written@;
                    let ghost paired_flag = has_unpaired_permission;
                    for skipped in skipping: &rest[..index]
                        invariant
                            0 <= rest_start,
                            rest_start + index < new@.len(),
                            skipping.seq().len() == index,
                            forall|at: int| 0 <= at < index ==> *#[trigger] skipping.seq()[at] == new@[rest_start + at],
                            well_formed(written@),
                            paired_written.is_prefix_of(written@),
                            paired_flag ==> has_unpaired_permission,
                            same_outside(memory_of(new@.take(rest_start + skipping.index())), memory_of(new@.take(rest_start)), written@,
                                has_unpaired_permission),
                    {
                        let ghost skipped_written = written@;
                        let ghost skipped_flag = has_unpaired_permission;
                        let ghost at = rest_start + skipping.index();
                        proof {
                            // Empty, but the proof of this function fails without it (measured).
                        }
                        add_unpaired(skipped, &mut written, &mut has_unpaired_permission);
                        proof {
                            lemma_unpaired_change(new@, at, memory_of(new@.take(rest_start)), skipped_written,
                                skipped_flag, written@, has_unpaired_permission);
                            // The ranges when paired are the start of those before this one, which are the start of those now.
                        }
                    }
                    proof {
                        // The two lists made the same change at their next places, so they stay alike outside the ranges.
                        let (first, second, index, rest, ranges_before, flag_before, ranges_after, flag_after) = (old@, new@,
                            pairing.index(), rest_start, paired_written, paired_flag, written@, has_unpaired_permission);
                        let first_before = memory_of(first.take(index));
                        let second_rest = memory_of(second.take(rest));
                        let second_before = memory_of(second.take(candidate));
                        let change = first[index]@;
                        lemma_memory_of_take(first, index);
                        lemma_memory_of_take(second, candidate);
                    }
                    rest = &rest[index + 1..];
                    proof {
                        rest_start = candidate + 1;
                        paired = true;
                    }
                }
                None => add_unpaired(mutation, &mut written, &mut has_unpaired_permission),
            }
            proof {
                if !paired {
                    lemma_unpaired_change(old@, pairing.index(), memory_of(new@.take(rest_start)), unpaired_written,
                        unpaired_flag, written@, has_unpaired_permission);
                }
            }
        }
        proof {
            assert(old@.take(old@.len() as int) =~= old@);
        }
        for unpaired in remaining: rest
            invariant
                0 <= rest_start <= new@.len(),
                remaining.seq() == new@.skip(rest_start).as_ref(),
                well_formed(written@),
                same_outside(memory_of(new@.take(rest_start + remaining.index())), memory_old, written@, has_unpaired_permission),
                memory_old == memory_of(old@),
        {
            let ghost rest_written = written@;
            let ghost rest_flag = has_unpaired_permission;
            let ghost at = rest_start + remaining.index();
            proof {
                // Empty, but the proof of this function fails without it (measured).
            }
            add_unpaired(unpaired, &mut written, &mut has_unpaired_permission);
            proof {
                lemma_unpaired_change(new@, at, memory_old, rest_written, rest_flag, written@, has_unpaired_permission);
            }
        }
        proof {
            // Empty, but the proof of this function fails without it (measured).
        }
        let ghost unsorted = written@;
        // VERUS: vstd's `sort_unstable_by_key` promises an order only of keys the key function is known to return, which no
        // proof can show of a closure or a function (measured), so his sort is a trusted function that states the order.
        sort_by_start(written.as_mut_slice());
        proof {
            // Sorting keeps each range well formed and the addresses the ranges hold.
            broadcast use vstd::seq_lib::group_to_multiset_ensures;
            let (before, after) = (unsorted, written@);
            assert forall|index: int| 0 <= index < after.len() implies #[trigger] after[index].start < after[index].end <= address_count() by {
                assert(before.to_multiset().count(after[index]) > 0);
                let at = choose|at: int| 0 <= at < before.len() && before[at] == after[index];
            }
            assert forall|address: u64| #[trigger] in_ranges(after, address) == in_ranges(before, address) by {
                if in_ranges(after, address) {
                    let index = choose|index: int| #![trigger after[index]] 0 <= index < after.len() && after[index].start <= address < after[index].end;
                    assert(before.to_multiset().count(after[index]) > 0);
                    let at = choose|at: int| 0 <= at < before.len() && before[at] == after[index];
                }
                if in_ranges(before, address) {
                    let index = choose|index: int| #![trigger before[index]] 0 <= index < before.len() && before[index].start <= address < before[index].end;
                    assert(after.to_multiset().count(before[index]) > 0);
                    let at = choose|at: int| 0 <= at < after.len() && after[at] == before[index];
                }
            }
        }
        let ghost sorted = written@;
        // Overlapping and touching ranges become one, so that each address is compared once.
        // VERUS: no `dedup_by` with a closure, so touching ranges are merged by index.
        let mut kept: usize = 0;
        let mut next: usize = 0;
        while next < written.len()
            invariant
                kept <= next <= written@.len(),
                written@.len() == sorted.len(),
                well_formed(sorted),
                starts_ascending(sorted),
                forall|at: int| next <= at < sorted.len() ==> #[trigger] written@[at] == sorted[at],
                well_formed(written@.take(kept as int)),
                separated(written@.take(kept as int)),
                kept > 0 ==> forall|at: int| next <= at < sorted.len() ==> written@[kept - 1].start <= #[trigger] sorted[at].start,
                forall|address: u64| #[trigger] in_ranges(written@.take(kept as int), address) == in_ranges(sorted.take(next as int), address),
            decreases sorted.len() - next,
        {
            let ghost merged_before = written@.take(kept as int);
            let (start, end) = (written[next].start, written[next].end);
            if kept > 0 && start <= written[kept - 1].end {
                let (merged_start, merged_end) = (written[kept - 1].start, written[kept - 1].end);
                written[kept - 1] = merged_start..merged_end.max(end);
                proof {
                    let merged = merged_start..(if end > merged_end { end } else { merged_end });
                    assert(written@.take(kept as int) =~= merged_before.update(kept - 1, merged));
                    lemma_merge_into_last(merged_before, sorted, next as int, merged);
                }
            } else {
                written[kept] = start..end;
                proof {
                    assert(written@.take(kept + 1) =~= merged_before.push(sorted[next as int]));
                    // Appending a range that starts after the last merged one ends keeps the merged ranges apart and holding
                    // what the sorted ones up to it hold.
                    assert(well_formed(merged_before.push(sorted[next as int])) && separated(merged_before.push(sorted[next as int]))
                        && forall|address: u64| #[trigger] in_ranges(merged_before.push(sorted[next as int]), address)
                            == in_ranges(sorted.take(next as int + 1), address)) by {
                        let next = next as int;
                        let merged_after = merged_before.push(sorted[next]);
                        assert forall|address: u64| #[trigger] in_ranges(merged_after, address) == in_ranges(sorted.take(next + 1), address) by {
                            if in_ranges(sorted.take(next + 1), address) {
                                let at = choose|at: int| #![trigger sorted.take(next + 1)[at]] 0 <= at < next + 1
                                    && sorted.take(next + 1)[at].start <= address < sorted.take(next + 1)[at].end;
                                if at < next {
                                    assert(in_ranges(merged_before, address));
                                    let merged_at = choose|merged_at: int| #![trigger merged_before[merged_at]] 0 <= merged_at < merged_before.len()
                                        && merged_before[merged_at].start <= address < merged_before[merged_at].end;
                                    assert(merged_after[merged_at] == merged_before[merged_at]);
                                } else {
                                    assert(merged_after[merged_before.len() as int] == sorted[next]);
                                }
                            }
                            if in_ranges(merged_after, address) {
                                let at = choose|at: int| #![trigger merged_after[at]] 0 <= at < merged_after.len()
                                    && merged_after[at].start <= address < merged_after[at].end;
                                if at < merged_before.len() {
                                    assert(in_ranges(merged_before, address));
                                    let sorted_at = choose|sorted_at: int| #![trigger sorted.take(next)[sorted_at]] 0 <= sorted_at < next
                                        && sorted.take(next)[sorted_at].start <= address < sorted.take(next)[sorted_at].end;
                                } else {
                                    assert(sorted.take(next + 1)[next] == sorted[next]);
                                }
                            }
                        }
                    }
                }
                kept += 1;
            }
            next += 1;
        }
        written.truncate(kept);
        proof {
            assert(sorted.take(sorted.len() as int) =~= sorted);
        }
        proof {
            // Empty, but the proof of this function fails without it (measured).
        }
        let ghost ranges = written@;
        proof {
            // Before the first range, or anywhere when there is none, the cursor has passed no address of the ranges.
        }
        let ghost mut changes: Seq<Assignment<Architecture::PagePermission>> = Seq::empty();
        let (mut old_bytes, mut new_bytes) = (Vec::new(), Vec::new());
        for addresses in walking: written
            invariant
                walking.seq() == ranges,
                well_formed(ranges),
                separated(ranges),
                forall|address: u64| !in_ranges(ranges, address) ==> #[trigger] memory_old(address).0 == memory_new(address).0,
                values_up_to(memory_old, memory_new, apply_all(memory_old, changes), ranges, if walking.index() < ranges.len() {
                    ranges[walking.index()].start as int } else { address_count() }),
                receives(hook_before, *mutation_hook, changes),
                memory_old == memory_new ==> changes.len() == 0 && *mutation_hook == hook_before,
                memory_old == memory_of(old@),
                memory_new == memory_of(new@),
        {
            let ghost range_index = walking.index();
            proof {
                // Empty, but the proof of this function fails without it (measured).
            }
            // VERUS: a range of every address has no length a `usize` holds, so a range is compared in pieces of at most `usize::MAX` bytes.
            let mut piece_start = addresses.start;
            while piece_start < addresses.end
                invariant
                    0 <= range_index < ranges.len(),
                    ranges[range_index] == addresses,
                    addresses.start <= piece_start <= addresses.end <= address_count(),
                    well_formed(ranges),
                    separated(ranges),
                    forall|address: u64| !in_ranges(ranges, address) ==> #[trigger] memory_old(address).0 == memory_new(address).0,
                    values_up_to(memory_old, memory_new, apply_all(memory_old, changes), ranges, piece_start as int),
                    receives(hook_before, *mutation_hook, changes),
                    memory_old == memory_new ==> changes.len() == 0 && *mutation_hook == hook_before,
                    memory_old == memory_of(old@),
                    memory_new == memory_of(new@),
                decreases addresses.end - piece_start,
            {
                let piece_end = if addresses.end - piece_start > usize::MAX as u128 { piece_start + usize::MAX as u128 } else { addresses.end };
                let start = piece_start as u64;
                let length = (piece_end - piece_start) as usize;
                resolve_bytes(old, start, length, &mut old_bytes);
                resolve_bytes(new, start, length, &mut new_bytes);
                let is_same_at = |offset: &usize| -> (same: bool)
                    requires
                        *offset < old_bytes@.len(),
                        *offset < new_bytes@.len(),
                    ensures
                        same == (old_bytes@[*offset as int] == new_bytes@[*offset as int]),
                { old_bytes[*offset] == new_bytes[*offset] };
                // Each run of bytes the two lists leave different is one mutation.
                let mut run_end = usize::MIN;
                while let Some(run_start) = (run_end..length).find(|offset| -> (different: bool)
                    requires
                        *offset < old_bytes@.len(),
                        *offset < new_bytes@.len(),
                    ensures
                        different == (old_bytes@[*offset as int] != new_bytes@[*offset as int]),
                { !is_same_at(offset) })
                    invariant
                        run_end <= length,
                        start as int + length as int <= addresses.end as int,
                        addresses.end as int <= address_count(),
                        start as int == piece_start as int,
                        addresses.start <= piece_start,
                        0 <= range_index < ranges.len(),
                        ranges[range_index] == addresses,
                        old_bytes@.len() == length,
                        new_bytes@.len() == length,
                        bytes_resolved(memory_old, old_bytes@, start),
                        bytes_resolved(memory_new, new_bytes@, start),
                        values_up_to(memory_old, memory_new, apply_all(memory_old, changes), ranges, start + run_end),
                        receives(hook_before, *mutation_hook, changes),
                        memory_old == memory_new ==> changes.len() == 0 && *mutation_hook == hook_before,
                        forall|offset: &usize| *offset < length ==> #[trigger] is_same_at.requires((offset,)),
                        forall|offset: &usize, same: bool| #[trigger] is_same_at.ensures((offset,), same)
                            ==> same == (old_bytes@[*offset as int] == new_bytes@[*offset as int]),
                    ensures
                        forall|step: int| 0 <= step < length - run_end ==> #[trigger] IteratorSpec::remaining(&(run_end..length))[step]
                            == run_end + step && old_bytes@[run_end + step] == new_bytes@[run_end + step],
                    decreases length - run_end,
                {
                    proof {
                        assert forall|at: int| run_end <= at < run_start implies
                            #[trigger] memory_old((start + at) as u64).0 == memory_new((start + at) as u64).0 by {
                            let searched = IteratorSpec::remaining(&(run_end..length));
                            assert(searched[at - run_end] == at);
                        }
                        lemma_same_bytes_passed(memory_old, memory_new, apply_all(memory_old, changes), ranges, range_index,
                            start, run_end as int, run_start as int);
                    }
                    let ghost run_start_value = run_start;
                    run_end = (run_start..length).find(is_same_at).unwrap_or(length);
                    let ghost hook_now = *mutation_hook;
                    let ghost change = Assignment::<Architecture::PagePermission>::ValueAssignment {
                        start: (start + run_start) as u64,
                        bytes: new_bytes@.subrange(run_start as int, run_end as int),
                    };
                    mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                        start: start + run_start as u64,
                        bytes: Cow::Borrowed(&new_bytes[run_start..run_end]),
                    });
                    proof {
                        // The run, sent as the new list's bytes, moves the cursor past the run and shows the memories differ.
                        let (first, second, memory, index, first_bytes, second_bytes, run_start, run_end) = (memory_old, memory_new,
                            apply_all(memory_old, changes), range_index, old_bytes@, new_bytes@, run_start as int, run_end as int);
                        let written_start = (start + run_start) as u64;
                        lemma_receives_one(hook_before, hook_now, *mutation_hook, changes, change);
                        lemma_apply_all_push(memory_old, changes, change);
                        changes = changes.push(change);
                    }
                }
                proof {
                    assert forall|at: int| run_end <= at < length implies
                        #[trigger] memory_old((start + at) as u64).0 == memory_new((start + at) as u64).0 by {
                        assert(IteratorSpec::remaining(&(run_end..length))[at - run_end] == at);
                    }
                    lemma_same_bytes_passed(memory_old, memory_new, apply_all(memory_old, changes), ranges, range_index,
                        start, run_end as int, length as int);
                }
                piece_start = piece_end;
            }
            proof {
                // From the end of one range the cursor moves to the start of the next, or past the last address after the last.
                let (first, second, memory, index) = (memory_old, memory_new, apply_all(memory_old, changes), range_index);
            }
        }
        proof {
            // Past the last address, the memory holds the new bytes with the old permissions everywhere.
            let (first, second) = (memory_old, memory_new);
            assert(apply_all(first, changes) =~= bytes_of_with_permissions_of(second, first));
        }
        if !has_unpaired_permission {
            proof {
                // The bytes sent are all the diff sends, since the two memories hold the same permissions.
                let (first, second, before, after) = (memory_of(old@), memory_of(new@), hook_before, *mutation_hook);
            }
            return;
        }
        let ghost hook_after_bytes = *mutation_hook;
        let ghost after_values = bytes_of_with_permissions_of(memory_new, memory_old);
        let ghost mut permission_changes: Seq<Assignment<Architecture::PagePermission>> = Seq::empty();
        let old_permissions = resolve_permissions(old);
        let new_permissions = resolve_permissions(new);
        // VERUS: no iterator chain over the range maps, so each map's bounds are appended to one list made to hold them, as
        // `collect` would size it.
        let mut bounds: Vec<u64> =
            Vec::with_capacity(range_map_len(&old_permissions).saturating_add(range_map_len(&new_permissions)).saturating_mul(2));
        append_bounds(&old_permissions, &mut bounds);
        let ghost old_bounds = bounds@;
        append_bounds(&new_permissions, &mut bounds);
        let ghost unsorted_bounds = bounds@;
        bounds.sort_unstable();
        let ghost sorted_bounds = bounds@;
        // VERUS: no `dedup`, so repeated bounds are dropped by index.
        let ghost sorted = bounds@;
        let mut kept: usize = 0;
        let mut next: usize = 0;
        while next < bounds.len()
            invariant
                kept <= next <= bounds@.len(),
                bounds@.len() == sorted.len(),
                sorted_by_le(sorted, |first: u64, second: u64| cmp_le(first, second)),
                forall|at: int| next <= at < sorted.len() ==> #[trigger] bounds@[at] == sorted[at],
                strictly_ascending(bounds@.take(kept as int)),
                kept > 0 ==> forall|at: int| next <= at < sorted.len() ==> bounds@[kept - 1] <= #[trigger] sorted[at],
                forall|bound: u64| #[trigger] bounds@.take(kept as int).contains(bound) == sorted.take(next as int).contains(bound),
            decreases sorted.len() - next,
        {
            let ghost kept_before = bounds@.take(kept as int);
            let bound = bounds[next];
            let ghost mut appended = false;
            if kept == 0 || bounds[kept - 1] != bound {
                bounds[kept] = bound;
                proof {
                    assert(bounds@.take(kept + 1) =~= kept_before.push(sorted[next as int]));
                    // A bound greater than the last kept one keeps the kept bounds rising and holding the sorted ones up to it.
                    let next = next as int;
                    let kept_after = kept_before.push(sorted[next]);
                    assert forall|bound: u64| #[trigger] kept_after.contains(bound) == sorted.take(next + 1).contains(bound) by {
                        if kept_after.contains(bound) && bound != sorted[next] {
                            let at = choose|at: int| 0 <= at < kept_after.len() && #[trigger] kept_after[at] == bound;
                            assert(kept_before.contains(bound));
                        }
                        if sorted.take(next + 1).contains(bound) && bound != sorted[next] {
                            let at = choose|at: int| 0 <= at < next + 1 && #[trigger] sorted.take(next + 1)[at] == bound;
                            assert(kept_before.contains(bound));
                            let kept_at = choose|kept_at: int| 0 <= kept_at < kept_before.len() && #[trigger] kept_before[kept_at] == bound;
                            assert(kept_after[kept_at] == bound);
                        }
                        if bound == sorted[next] {
                            assert(kept_after[kept_before.len() as int] == bound);
                            assert(sorted.take(next + 1)[next] == bound);
                        }
                    }
                }
                proof {
                    appended = true;
                }
                kept += 1;
            }
            proof {
                if !appended {
                    // A bound equal to the last kept one adds nothing.
                    let next = next as int;
                    assert(kept_before.contains(sorted[next])) by {
                        assert(kept_before[kept_before.len() - 1] == sorted[next]);
                    }
                }
            }
            next += 1;
        }
        bounds.truncate(kept);
        proof {
            assert(sorted.take(sorted.len() as int) =~= sorted);
        }
        proof {
            // The bounds walked after sorting and dropping repeats separate each map's values.
            let (first_values, second_values, first_bounds, unsorted, sorted, walked) = (range_map_values(&old_permissions),
                range_map_values(&new_permissions), old_bounds, unsorted_bounds, sorted_bounds, bounds@);
            let second_bounds = unsorted.skip(first_bounds.len() as int);
            lemma_bounds_kept(first_bounds, 0, unsorted, sorted, walked);
            lemma_bounds_kept(second_bounds, first_bounds.len() as int, unsorted, sorted, walked);
            lemma_values_between(first_values, first_bounds, walked);
            lemma_values_between(second_values, second_bounds, walked);
        }
        let read_permission = |permissions: &RangeMap<u64, Architecture::PagePermission>, address: u64|
            -> (permission: Architecture::PagePermission)
            ensures
                permission == permission_or_default::<Architecture>(range_map_values(permissions)(address)),
        {
            // VERUS: no `RangeMap`, so its `get` is trusted.
            range_map_get(permissions, address).unwrap_or(Architecture::DEFAULT_PERMISSION)
        };
        // Between two neighboring bounds each list leaves one permission.
        let ghost all = bounds@;
        let mut bounds = bounds.into_iter();
        let Some(mut start) = bounds.next() else {
            proof {
                lemma_permissions_outside::<Architecture>(memory_old, memory_new, after_values, old_permissions, new_permissions,
                    all, 0, 0);
                lemma_permissions_sent(memory_old, memory_new, hook_after_bytes, *mutation_hook, permission_changes);
            }
            return;
        };
        let ghost low = all[0] as int;
        for end in walking: bounds
            invariant
                1 <= all.len(),
                walking.seq() == all.drop_first(),
                start == all[walking.index()],
                low == all[0] as int,
                strictly_ascending(all),
                permissions_resolved::<Architecture>(memory_old, old_permissions),
                permissions_resolved::<Architecture>(memory_new, new_permissions),
                values_by_bounds(range_map_values(&old_permissions), all),
                values_by_bounds(range_map_values(&new_permissions), all),
                permissions_up_to(memory_old, memory_new, apply_all(after_values, permission_changes), low, start as int),
                receives(hook_after_bytes, *mutation_hook, permission_changes),
                memory_old == memory_new ==> permission_changes.len() == 0 && *mutation_hook == hook_after_bytes,
                memory_old == memory_of(old@),
                memory_new == memory_of(new@),
                after_values == bytes_of_with_permissions_of(memory_new, memory_old),
                obeys_concrete_eq::<Architecture::PagePermission>(),
                forall|permissions: &RangeMap<u64, Architecture::PagePermission>, address: u64|
                    #[trigger] read_permission.requires((permissions, address)),
                forall|permissions: &RangeMap<u64, Architecture::PagePermission>, address: u64, permission: Architecture::PagePermission|
                    #[trigger] read_permission.ensures((permissions, address), permission)
                        ==> permission == permission_or_default::<Architecture>(range_map_values(permissions)(address)),
        {
            proof {
                reveal(obeys_concrete_eq);
            }
            let ghost end_index = walking.index() + 1;
            let addresses = start..end;
            start = end;
            let permission = read_permission(&new_permissions, addresses.start);
            let ghost changes_before = permission_changes;
            let ghost mut sent = false;
            if read_permission(&old_permissions, addresses.start) != permission {
                proof {
                    assert(memory_old(addresses.start).1 != memory_new(addresses.start).1);
                }
                let ghost hook_now = *mutation_hook;
                let ghost change = Assignment::<Architecture::PagePermission>::PermissionAssignment { addresses, permission };
                mutation_hook.on_memory_mutated(MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                });
                proof {
                    lemma_receives_one(hook_after_bytes, hook_now, *mutation_hook, permission_changes, change);
                    lemma_apply_all_push(after_values, permission_changes, change);
                    lemma_permission_interval::<Architecture>(memory_old, memory_new, apply_all(after_values, permission_changes),
                        apply_all(after_values, permission_changes.push(change)), old_permissions, new_permissions, all,
                        end_index, low, true);
                    permission_changes = permission_changes.push(change);
                    sent = true;
                }
            }
            proof {
                if !sent {
                    lemma_permission_interval::<Architecture>(memory_old, memory_new, apply_all(after_values, changes_before),
                        apply_all(after_values, changes_before), old_permissions, new_permissions, all, end_index, low, false);
                }
            }
        }
        proof {
            lemma_permissions_outside::<Architecture>(memory_old, memory_new, apply_all(after_values, permission_changes), old_permissions,
                new_permissions, all, low, all[all.len() - 1] as int);
            lemma_bytes_then_permissions(memory_old, memory_new, hook_before, hook_after_bytes, *mutation_hook);
        }
    }
}

/// A list holding the bytes.
// VERUS: no `<[u8]>::to_vec`, so the bytes are copied into an empty list.
#[inline]
pub(crate) fn bytes_to_vec(bytes: &[u8]) -> (owned: Vec<u8>)
    ensures
        owned@ == bytes@,
{
    let mut owned = Vec::new();
    owned.extend_from_slice(bytes);
    owned
}

// The proof of the diff.
/// The number of addresses, one past the last.
pub open spec fn address_count() -> int {
    0x1_0000_0000_0000_0000
}

/// The memory of a list's first `index + 1` mutations is that of its first `index` with the next one applied.
pub proof fn lemma_memory_of_take<'memory, Architecture: ProcessorArchitecture>(mutations: Seq<MemoryDelta<'memory, Architecture>>, index: int)
    requires
        0 <= index < mutations.len(),
    ensures
        memory_of(mutations.take(index + 1)) == apply(memory_of(mutations.take(index)), mutations[index]@),
{
    assert(assignments(mutations.take(index + 1)).drop_last() =~= assignments(mutations.take(index)));
}

/// A sink that received changes and then one more received the changes with that one appended.
pub proof fn lemma_receives_one<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    before: Sink,
    middle: Sink,
    after: Sink,
    changes: Seq<Assignment<Architecture::PagePermission>>,
    change: Assignment<Architecture::PagePermission>,
)
    requires
        receives(before, middle, changes),
        forall|memory: Memory<Architecture::PagePermission>|
            #[trigger] after.applied_to(memory) == apply(middle.applied_to(memory), change),
    ensures
        receives(before, after, changes.push(change)),
{
    assert forall|memory: Memory<Architecture::PagePermission>|
        #[trigger] after.applied_to(memory) == apply_all(before.applied_to(memory), changes.push(change)) by {
        assert(changes.push(change).drop_last() =~= changes);
    }
}

/// Where a change that starts at `start` reaches the address, counted from its start and past the last address.
pub open spec fn offset(start: u64, address: u64) -> int {
    if start <= address { address - start } else { address + address_count() - start }
}

/// Whether the change writes the byte at the address.
pub open spec fn writes_byte<Permission>(change: Assignment<Permission>, address: u64) -> bool {
    match change {
        Assignment::ValueAssignment { start, bytes } => offset(start, address) < bytes.len(),
        Assignment::PermissionAssignment { .. } => false,
    }
}

/// Whether the change gives some address a permission.
pub open spec fn assigns_permission<Permission>(change: Assignment<Permission>) -> bool {
    change matches Assignment::PermissionAssignment { addresses, .. } && addresses.start < addresses.end
}

/// Whether one of the ranges holds the address.
pub open spec fn in_ranges(ranges: Seq<Range<u128>>, address: u64) -> bool {
    exists|index: int| #![trigger ranges[index]] 0 <= index < ranges.len() && ranges[index].start <= address < ranges[index].end
}

/// Each range is not empty and ends at most one past the last address.
pub open spec fn well_formed(ranges: Seq<Range<u128>>) -> bool {
    forall|index: int| 0 <= index < ranges.len() ==> #[trigger] ranges[index].start < ranges[index].end <= address_count()
}

/// The ranges start in ascending order.
pub open spec fn starts_ascending(ranges: Seq<Range<u128>>) -> bool {
    forall|first: int, second: int| 0 <= first < second < ranges.len() ==> #[trigger] ranges[first].start <= #[trigger] ranges[second].start
}

/// Each range ends before the next starts.
pub open spec fn separated(ranges: Seq<Range<u128>>) -> bool {
    forall|first: int, second: int| 0 <= first < second < ranges.len() ==> #[trigger] ranges[first].end < #[trigger] ranges[second].start
}

/// The two memories hold the same byte at every address outside the ranges, and the same permission everywhere
/// unless permissions may differ.
pub open spec fn same_outside<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    ranges: Seq<Range<u128>>,
    permissions_may_differ: bool,
) -> bool {
    &&& forall|address: u64| !in_ranges(ranges, address) ==> #[trigger] first(address).0 == second(address).0
    &&& !permissions_may_differ ==> forall|address: u64| #[trigger] first(address).1 == second(address).1
}

/// What recording an unpaired change does: the ranges grow to hold every byte it writes, and the flag is raised
/// when it gives a permission.
pub open spec fn records_unpaired<Permission>(
    change: Assignment<Permission>,
    ranges_before: Seq<Range<u128>>,
    flag_before: bool,
    ranges_after: Seq<Range<u128>>,
    flag_after: bool,
) -> bool {
    &&& ranges_before.is_prefix_of(ranges_after)
    &&& (well_formed(ranges_before) ==> well_formed(ranges_after))
    &&& forall|address: u64| #[trigger] writes_byte(change, address) ==> in_ranges(ranges_after, address)
    &&& flag_after == (flag_before || assigns_permission(change))
}

/// Memories alike outside some ranges are alike outside more ranges.
pub proof fn lemma_same_outside_widen<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    ranges_before: Seq<Range<u128>>,
    flag_before: bool,
    ranges_after: Seq<Range<u128>>,
    flag_after: bool,
)
    requires
        same_outside(first, second, ranges_before, flag_before),
        ranges_before.is_prefix_of(ranges_after),
        flag_before ==> flag_after,
    ensures
        same_outside(first, second, ranges_after, flag_after),
{
}

pub proof fn lemma_same_outside_symmetric<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    ranges: Seq<Range<u128>>,
    flag: bool,
)
    requires
        same_outside(first, second, ranges, flag),
    ensures
        same_outside(second, first, ranges, flag),
{
}

/// An unpaired change applied to a list's memory leaves it alike, outside what was recorded, to what it was alike to.
pub proof fn lemma_unpaired_change<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
    index: int,
    other: Memory<Architecture::PagePermission>,
    ranges_before: Seq<Range<u128>>,
    flag_before: bool,
    ranges_after: Seq<Range<u128>>,
    flag_after: bool,
)
    requires
        0 <= index < mutations.len(),
        well_formed(ranges_before),
        same_outside(memory_of(mutations.take(index)), other, ranges_before, flag_before),
        records_unpaired(mutations[index]@, ranges_before, flag_before, ranges_after, flag_after),
    ensures
        well_formed(ranges_after),
        same_outside(memory_of(mutations.take(index + 1)), other, ranges_after, flag_after),
{
    lemma_memory_of_take(mutations, index);
    let before = memory_of(mutations.take(index));
    let change = mutations[index]@;
    assert forall|address: u64| !in_ranges(ranges_after, address) implies #[trigger] apply(before, change)(address).0 == other(address).0 by {
        assert(!writes_byte(change, address));
    }
}

/// The memory holds the second memory's bytes at each address of the ranges below the cursor, with the first
/// memory's permissions, and the first memory everywhere else.
pub open spec fn values_up_to<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<Range<u128>>,
    cursor: int,
) -> bool {
    forall|address: u64| #[trigger] memory(address)
        == if in_ranges(ranges, address) && address < cursor { (second(address).0, first(address).1) } else { first(address) }
}

/// No address of the separated ranges lies between the end of one and the start of the next.
pub proof fn lemma_between_ranges(ranges: Seq<Range<u128>>, index: int, address: u64)
    requires
        well_formed(ranges),
        separated(ranges),
        0 <= index < ranges.len(),
        ranges[index].end <= address,
        index + 1 < ranges.len() ==> address < ranges[index + 1].start,
    ensures
        !in_ranges(ranges, address),
{
    if in_ranges(ranges, address) {
        let at = choose|at: int| #![trigger ranges[at]] 0 <= at < ranges.len() && ranges[at].start <= address < ranges[at].end;
        if at < index {
            assert(ranges[at].end < ranges[index].start);
        }
    }
}

/// Moving the cursor over addresses that no range holds changes nothing.
pub proof fn lemma_cursor_skip<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<Range<u128>>,
    from: int,
    to: int,
)
    requires
        from <= to,
        values_up_to(first, second, memory, ranges, from),
        forall|address: u64| from <= address < to ==> !#[trigger] in_ranges(ranges, address),
    ensures
        values_up_to(first, second, memory, ranges, to),
{
}

/// The bytes are what the memory holds from the start on.
pub open spec fn bytes_resolved<Permission>(memory: Memory<Permission>, bytes: Seq<u8>, start: u64) -> bool {
    forall|index: int| 0 <= index < bytes.len() ==> #[trigger] bytes[index] == memory((start + index) as u64).0
}

/// What the permission of an address is when the range map gives it a value, or gives none.
pub open spec fn permission_or_default<Architecture: ProcessorArchitecture>(
    value: Option<Architecture::PagePermission>,
) -> Architecture::PagePermission {
    match value {
        Some(permission) => permission,
        None => Architecture::DEFAULT_PERMISSION,
    }
}

/// The map gives each address the permission the memory holds there, or nothing where that is the default.
pub open spec fn permissions_resolved<Architecture: ProcessorArchitecture>(
    memory: Memory<Architecture::PagePermission>,
    map: RangeMap<u64, Architecture::PagePermission>,
) -> bool {
    forall|address: u64| #[trigger] memory(address).1 == permission_or_default::<Architecture>(range_map_values(&map)(address))
}

/// The memory holds the second memory at each address from the low bound up to the cursor, and the second
/// memory's bytes with the first memory's permissions everywhere else.
pub open spec fn permissions_up_to<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    low: int,
    cursor: int,
) -> bool {
    forall|address: u64| #[trigger] memory(address)
        == if low <= address < cursor { second(address) } else { (second(address).0, first(address).1) }
}

/// The bounds rise strictly.
pub open spec fn strictly_ascending(bounds: Seq<u64>) -> bool {
    forall|first: int, second: int| 0 <= first < second < bounds.len() ==> #[trigger] bounds[first] < #[trigger] bounds[second]
}

/// Each bound of the one list is among the other's.
pub open spec fn bounds_within(bounds: Seq<u64>, within: Seq<u64>) -> bool {
    forall|index: int| 0 <= index < bounds.len() ==> within.contains(#[trigger] bounds[index])
}

/// The values are the same from each walked bound up to the next, and there are none before the first or from the
/// last on.
#[verifier::opaque]
pub open spec fn values_by_bounds<V>(values: spec_fn(u64) -> Option<V>, walked: Seq<u64>) -> bool {
    &&& forall|index: int, address: u64| 0 <= index && index + 1 < walked.len() && walked[index] <= address < walked[index + 1]
        ==> #[trigger] values(address) == values(#[trigger] walked[index])
    &&& forall|address: u64| (walked.len() == 0 || address < walked[0] || walked[walked.len() - 1] <= address)
        ==> #[trigger] values(address) is None
}

/// A map whose bounds are among strictly rising ones gives one value from each of them up to the next, and none
/// before the first or from the last on.
#[verifier::spinoff_prover]
pub proof fn lemma_values_between<V>(values: spec_fn(u64) -> Option<V>, bounds: Seq<u64>, walked: Seq<u64>)
    requires
        bounds_separate(values, bounds),
        bounds_within(bounds, walked),
        strictly_ascending(walked),
    ensures
        values_by_bounds(values, walked),
{
    reveal(values_by_bounds);
}

/// Bytes the two memories hold alike are passed by the cursor with nothing to change.
pub proof fn lemma_same_bytes_passed<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<Range<u128>>,
    index: int,
    start: u64,
    from: int,
    to: int,
)
    requires
        0 <= index < ranges.len(),
        0 <= from <= to,
        ranges[index].start <= start + from,
        start + to <= ranges[index].end <= address_count(),
        values_up_to(first, second, memory, ranges, start + from),
        forall|at: int| from <= at < to ==> #[trigger] first((start + at) as u64).0 == second((start + at) as u64).0,
    ensures
        values_up_to(first, second, memory, ranges, start + to),
{
    assert forall|address: u64| #[trigger] memory(address)
        == if in_ranges(ranges, address) && address < start + to { (second(address).0, first(address).1) } else { first(address) } by {
        if start + from <= address < start + to {
            let at = address - start;
            assert(first((start + at) as u64).0 == second((start + at) as u64).0);
        }
    }
}

/// The bounds of one map are among those walked, which were sorted and deduplicated from a list that held them.
pub proof fn lemma_bounds_kept(part: Seq<u64>, at: int, unsorted: Seq<u64>, sorted: Seq<u64>, walked: Seq<u64>)
    requires
        0 <= at,
        at + part.len() <= unsorted.len(),
        forall|index: int| 0 <= index < part.len() ==> #[trigger] part[index] == unsorted[at + index],
        sorted.to_multiset() == unsorted.to_multiset(),
        forall|bound: u64| #[trigger] walked.contains(bound) == sorted.contains(bound),
    ensures
        bounds_within(part, walked),
{
    broadcast use vstd::seq_lib::group_to_multiset_ensures;
    assert forall|index: int| 0 <= index < part.len() implies walked.contains(#[trigger] part[index]) by {
        assert(sorted.to_multiset().count(part[index]) > 0);
    }
}

/// The permission of each address in the interval from one bound to the next is now the second memory's.
pub proof fn lemma_permission_interval<Architecture: ProcessorArchitecture>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    memory: Memory<Architecture::PagePermission>,
    memory_after: Memory<Architecture::PagePermission>,
    first_permissions: RangeMap<u64, Architecture::PagePermission>,
    second_permissions: RangeMap<u64, Architecture::PagePermission>,
    bounds: Seq<u64>,
    index: int,
    low: int,
    emitted: bool,
)
    requires
        1 <= index < bounds.len(),
        low == bounds[0],
        strictly_ascending(bounds),
        permissions_resolved::<Architecture>(first, first_permissions),
        permissions_resolved::<Architecture>(second, second_permissions),
        values_by_bounds(range_map_values(&first_permissions), bounds),
        values_by_bounds(range_map_values(&second_permissions), bounds),
        permissions_up_to(first, second, memory, low, bounds[index - 1] as int),
        emitted ==> memory_after == apply(memory, Assignment::PermissionAssignment {
            addresses: bounds[index - 1]..bounds[index],
            permission: permission_or_default::<Architecture>(range_map_values(&second_permissions)(bounds[index - 1])),
        }),
        !emitted ==> memory_after == memory,
        !emitted ==> permission_or_default::<Architecture>(range_map_values(&first_permissions)(bounds[index - 1]))
            == permission_or_default::<Architecture>(range_map_values(&second_permissions)(bounds[index - 1])),
    ensures
        permissions_up_to(first, second, memory_after, low, bounds[index] as int),
{
    reveal(values_by_bounds);
    if index > 1 {
        assert(bounds[0] < bounds[index - 1]);
    }
    assert(bounds[index - 1] < bounds[index]);
    assert forall|address: u64| #[trigger] memory_after(address)
        == if low <= address < bounds[index] { second(address) } else { (second(address).0, first(address).1) } by {
        assert(memory(address)
            == if low <= address < bounds[index - 1] { second(address) } else { (second(address).0, first(address).1) });
        if bounds[index - 1] <= address < bounds[index] {
            assert(range_map_values(&first_permissions)(address) == range_map_values(&first_permissions)(bounds[index - 1]));
            assert(range_map_values(&second_permissions)(address) == range_map_values(&second_permissions)(bounds[index - 1]));
            assert(first(address).1 == permission_or_default::<Architecture>(range_map_values(&first_permissions)(address)));
            assert(second(address).1 == permission_or_default::<Architecture>(range_map_values(&second_permissions)(address)));
        }
    }
}

/// Where neither map gives a value both memories hold the default permission, so a memory that holds the second
/// memory between the bounds and its bytes elsewhere is the second memory.
pub proof fn lemma_permissions_outside<Architecture: ProcessorArchitecture>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    memory: Memory<Architecture::PagePermission>,
    first_permissions: RangeMap<u64, Architecture::PagePermission>,
    second_permissions: RangeMap<u64, Architecture::PagePermission>,
    bounds: Seq<u64>,
    low: int,
    high: int,
)
    requires
        permissions_up_to(first, second, memory, low, high),
        permissions_resolved::<Architecture>(first, first_permissions),
        permissions_resolved::<Architecture>(second, second_permissions),
        values_by_bounds(range_map_values(&first_permissions), bounds),
        values_by_bounds(range_map_values(&second_permissions), bounds),
        bounds.len() == 0 ==> low == high,
        bounds.len() > 0 ==> low == bounds[0] && high == bounds[bounds.len() - 1],
    ensures
        memory == second,
{
    reveal(values_by_bounds);
}

/// Applying one more change applies it last.
pub proof fn lemma_apply_all_push<Permission>(memory: Memory<Permission>, changes: Seq<Assignment<Permission>>, change: Assignment<Permission>)
    ensures
        apply_all(memory, changes.push(change)) == apply(apply_all(memory, changes), change),
{
    assert(changes.push(change).drop_last() =~= changes);
}

/// The bytes of one memory with the permissions of another.
pub open spec fn bytes_of_with_permissions_of<Permission>(bytes: Memory<Permission>, permissions: Memory<Permission>) -> Memory<Permission> {
    |address: u64| (bytes(address).0, permissions(address).1)
}

/// The sink received changes that turn the first memory's bytes into the second's, and nothing when the two are the same.
pub open spec fn bytes_sent<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    before: Sink,
    after: Sink,
) -> bool {
    &&& exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(first, changes)]
        receives(before, after, changes) && apply_all(first, changes) == bytes_of_with_permissions_of(second, first)
    &&& first == second ==> after == before
}

/// The sink received changes that turn the second memory's bytes with the first's permissions into the second memory,
/// and nothing when the two are the same.
pub open spec fn permissions_sent<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    before: Sink,
    after: Sink,
) -> bool {
    &&& exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(bytes_of_with_permissions_of(second, first), changes)]
        receives(before, after, changes) && apply_all(bytes_of_with_permissions_of(second, first), changes) == second
    &&& first == second ==> after == before
}

/// The bytes sent and then the permissions sent are what the diff sends.
pub proof fn lemma_bytes_then_permissions<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    before: Sink,
    middle: Sink,
    after: Sink,
)
    requires
        bytes_sent(first, second, before, middle),
        permissions_sent(first, second, middle, after),
    ensures
        exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(first, changes)]
            receives(before, after, changes) && apply_all(first, changes) == second,
        first == second ==> after == before,
{
    let byte_changes = choose|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(first, changes)]
        receives(before, middle, changes) && apply_all(first, changes) == bytes_of_with_permissions_of(second, first);
    let permission_changes = choose|changes: Seq<Assignment<Architecture::PagePermission>>|
        #![trigger apply_all(bytes_of_with_permissions_of(second, first), changes)]
        receives(middle, after, changes) && apply_all(bytes_of_with_permissions_of(second, first), changes) == second;
    // The sink received the byte changes and then the permission changes, so it received both joined.
    assert forall|memory: Memory<Architecture::PagePermission>|
        #[trigger] after.applied_to(memory) == apply_all(before.applied_to(memory), byte_changes + permission_changes) by {
        lemma_apply_all_append(before.applied_to(memory), byte_changes, permission_changes);
    }
    lemma_apply_all_append(first, byte_changes, permission_changes);
}

/// The permission changes sent are what the diff sends after the bytes.
pub proof fn lemma_permissions_sent<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    before: Sink,
    after: Sink,
    changes: Seq<Assignment<Architecture::PagePermission>>,
)
    requires
        receives(before, after, changes),
        apply_all(bytes_of_with_permissions_of(second, first), changes) == second,
        first == second ==> changes.len() == 0 && after == before,
    ensures
        permissions_sent(first, second, before, after),
{
}

/// Merging a range that starts within the last merged one keeps the merged ranges apart and holding what the sorted
/// ones up to it hold.
pub proof fn lemma_merge_into_last(merged_before: Seq<Range<u128>>, sorted: Seq<Range<u128>>, next: int, new_last: Range<u128>)
    requires
        0 <= next < sorted.len(),
        well_formed(sorted),
        merged_before.len() > 0,
        well_formed(merged_before),
        separated(merged_before),
        merged_before.last().start <= sorted[next].start <= merged_before.last().end,
        new_last == (merged_before.last().start..(
            if sorted[next].end > merged_before.last().end { sorted[next].end } else { merged_before.last().end })),
        forall|address: u64| #[trigger] in_ranges(merged_before, address) == in_ranges(sorted.take(next), address),
    ensures
        well_formed(merged_before.update(merged_before.len() - 1, new_last)),
        separated(merged_before.update(merged_before.len() - 1, new_last)),
        forall|address: u64| #[trigger] in_ranges(merged_before.update(merged_before.len() - 1, new_last), address)
            == in_ranges(sorted.take(next + 1), address),
{
    let last = merged_before.len() - 1;
    let merged_after = merged_before.update(last, new_last);
    assert forall|address: u64| #[trigger] in_ranges(merged_after, address) == in_ranges(sorted.take(next + 1), address) by {
        let in_new = sorted[next].start <= address < sorted[next].end;
        if in_ranges(sorted.take(next + 1), address) {
            let at = choose|at: int| #![trigger sorted.take(next + 1)[at]] 0 <= at < next + 1 && sorted.take(next + 1)[at].start <= address < sorted.take(next + 1)[at].end;
            if at < next {
                assert(in_ranges(merged_before, address));
                let merged_at = choose|merged_at: int| #![trigger merged_before[merged_at]] 0 <= merged_at < merged_before.len()
                    && merged_before[merged_at].start <= address < merged_before[merged_at].end;
                assert(merged_after[merged_at].start <= address < merged_after[merged_at].end);
            }
        }
        if in_ranges(merged_after, address) {
            let at = choose|at: int| #![trigger merged_after[at]] 0 <= at < merged_after.len() && merged_after[at].start <= address < merged_after[at].end;
            if at < last || (merged_before[last].start <= address < merged_before[last].end) {
                let merged_at = if at < last { at } else { last };
                assert(merged_before[merged_at].start <= address < merged_before[merged_at].end);
                assert(in_ranges(merged_before, address));
                let sorted_at = choose|sorted_at: int| #![trigger sorted.take(next)[sorted_at]] 0 <= sorted_at < next
                    && sorted.take(next)[sorted_at].start <= address < sorted.take(next)[sorted_at].end;
                assert(sorted.take(next + 1)[sorted_at] == sorted.take(next)[sorted_at]);
            } else {
                assert(in_new);
                assert(sorted.take(next + 1)[next] == sorted[next]);
            }
        }
    }
}

} // verus!
