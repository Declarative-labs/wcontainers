//! `MemorySnapshot::diff` with its proof: the owner's code, changed only where a `// VERUS:` line says why.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use rangemap::RangeMap;
#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::slice::{cmp_le, sorted_by_le};
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

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
                },
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
                },
                _ => false,
            }
        }

        /// Records the addresses an unpaired mutation reaches: the bytes it writes, or that it assigns a permission.
        // VERUS: no closure that captures a mutable reference, so what is recorded is passed in.
        fn add_unpaired<Architecture: ProcessorArchitecture>(
            mutation: &MemoryDelta<'_, Architecture>,
            written: &mut Vec<(u128, u128)>,
            has_unpaired_permission: &mut bool,
        )
            ensures
                records_unpaired(mutation@, vstd::prelude::old(written)@, *vstd::prelude::old(has_unpaired_permission), final(written)@, *final(has_unpaired_permission)),
        {
            proof {
                broadcast use vstd::laws_cmp::group_laws_cmp;
            }
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
                        (u128::from(*start), end.min(ADDRESSES_END)),
                        (0, end.saturating_sub(ADDRESSES_END)),
                    ];
                    // VERUS: no `filter` with a closure, so each of the two ranges is kept if it is not empty.
                    if ranges_around_wrap[0].0 < ranges_around_wrap[0].1 {
                        written.push(ranges_around_wrap[0]);
                    }
                    let ghost below_wrap = written@;
                    if ranges_around_wrap[1].0 < ranges_around_wrap[1].1 {
                        written.push(ranges_around_wrap[1]);
                    }
                    proof {
                        lemma_value_reach_recorded::<Architecture::PagePermission>(*start, bytes@, before, below_wrap, written@);
                    }
                },
                MemoryDelta::PermissionAssignment { addresses, .. } => {
                    *has_unpaired_permission |= !addresses.is_empty();
                    proof {
                        assert(written@.is_prefix_of(written@)) by {
                            assert(written@.take(written@.len() as int) =~= written@);
                        }
                    }
                },
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
            proof {
                assert(mutations@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
                assert(bytes_resolved(memory_of(mutations@.take(0)), rendered@, start));
            }
            for mutation in walking: mutations
                invariant
                    walking.seq() == mutations@.as_ref(),
                    rendered@.len() == length,
                    start as int + length as int <= address_count(),
                    bytes_resolved(memory_of(mutations@.take(walking.index())), rendered@, start),
            {
                let ghost before = rendered@;
                let ghost index = walking.index();
                proof {
                    assert(*mutation == mutations@[index]);
                }
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
                        lemma_copy_window(before, after_first, rendered@, start, *mutation_start, bytes@, length as int);
                    }
                }
                proof {
                    lemma_resolved_after_change(mutations@, index, before, rendered@, start);
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
                broadcast use vstd::laws_cmp::group_laws_cmp;
            }
            // VERUS: no `RangeMap`, so its `new` and `insert` are trusted.
            let mut rendered = new_range_map();
            proof {
                assert(mutations@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
                assert(assignments(mutations@.take(0)) =~= Seq::empty());
            }
            for mutation in walking: mutations
                invariant
                    walking.seq() == mutations@.as_ref(),
                    obeys_concrete_eq::<Architecture::PagePermission>(),
                    permissions_resolved::<Architecture>(memory_of(mutations@.take(walking.index())), rendered),
            {
                let ghost before = rendered;
                let ghost index = walking.index();
                proof {
                    assert(*mutation == mutations@[index]);
                }
                // VERUS: no `let` in a condition joined by `&&`, so the empty check is an `if` of its own.
                if let MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } = mutation
                {
                    if !addresses.is_empty() {
                        insert_range(&mut rendered, addresses, *permission);
                    }
                }
                proof {
                    lemma_memory_of_take(mutations@, index);
                    let memory = memory_of(mutations@.take(index));
                    assert forall|address: u64| #[trigger] apply(memory, mutations@[index]@)(address).1
                        == permission_or_default::<Architecture>(range_map_values(&rendered)(address)) by {
                        assert(memory(address).1 == permission_or_default::<Architecture>(range_map_values(&before)(address)));
                    }
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
        proof {
            Architecture::equality_is_identity();
        }
        let ghost hook_before = *mutation_hook;
        let ghost memory_old = memory_of(old@);
        let ghost memory_new = memory_of(new@);
        proof {
            Architecture::equality_is_identity();
            assert(old@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
            assert(new@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
            assert(memory_of(old@.take(0)) =~= memory_of(new@.take(0)));
        }
        let (mut written, mut has_unpaired_permission) = (Vec::new(), false);
        let mut rest = new;
        // Where `rest` starts in the new list.
        let ghost mut rest_start: int = 0;
        for mutation in pairing: old
            invariant
                0 <= rest_start <= new@.len(),
                rest@ == new@.skip(rest_start),
                pairing.seq() == old@.as_ref(),
                obeys_concrete_eq::<Architecture::PagePermission>(),
                well_formed(written@),
                same_outside(memory_of(old@.take(pairing.index())), memory_of(new@.take(rest_start)), written@, has_unpaired_permission),
        {
            proof {
                assert(*mutation == old@[pairing.index()]);
            }
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
                    proof {
                        assert forall|at: int| 0 <= at < index implies #[trigger] rest@[at] == new@[rest_start + at] by {
                            assert(rest@[at] == new@.skip(rest_start)[at]);
                        }
                    }
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
                            assert(*skipped == new@[at]);
                        }
                        add_unpaired(skipped, &mut written, &mut has_unpaired_permission);
                        proof {
                            lemma_unpaired_change(new@, at, memory_of(new@.take(rest_start)), skipped_written,
                                skipped_flag, written@, has_unpaired_permission);
                            lemma_prefix_transitive(paired_written, skipped_written, written@);
                        }
                    }
                    proof {
                        lemma_paired_change(old@, new@, pairing.index(), rest_start, candidate, paired_written, paired_flag,
                            written@, has_unpaired_permission);
                    }
                    rest = &rest[index + 1..];
                    proof {
                        rest_start = candidate + 1;
                        assert(rest@ =~= new@.skip(rest_start));
                    }
                }
                None => {
                    let ghost unpaired_written = written@;
                    let ghost unpaired_flag = has_unpaired_permission;
                    add_unpaired(mutation, &mut written, &mut has_unpaired_permission);
                    proof {
                        lemma_unpaired_change(old@, pairing.index(), memory_of(new@.take(rest_start)), unpaired_written,
                            unpaired_flag, written@, has_unpaired_permission);
                    }
                }
            }
        }
        proof {
            assert(old@.take(old@.len() as int) =~= old@);
            lemma_same_outside_symmetric(memory_old, memory_of(new@.take(rest_start)), written@, has_unpaired_permission);
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
                assert(*unpaired == new@[at]);
            }
            add_unpaired(unpaired, &mut written, &mut has_unpaired_permission);
            proof {
                lemma_unpaired_change(new@, at, memory_old, rest_written, rest_flag, written@, has_unpaired_permission);
            }
        }
        proof {
            assert(new@.take(new@.len() as int) =~= new@);
            lemma_same_outside_symmetric(memory_new, memory_old, written@, has_unpaired_permission);
        }
        let ghost unsorted = written@;
        proof {
            broadcast use vstd::laws_cmp::group_laws_cmp;
        }
        // VERUS: no sort by a key a specification can read, so the ranges are (start, end) pairs, which sort by start first.
        written.as_mut_slice().sort_unstable();
        proof {
            lemma_sorted_ranges(unsorted, written@);
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
                kept > 0 ==> forall|at: int| next <= at < sorted.len() ==> written@[kept - 1].0 <= #[trigger] sorted[at].0,
                forall|address: u64| #[trigger] in_ranges(written@.take(kept as int), address) == in_ranges(sorted.take(next as int), address),
            decreases sorted.len() - next,
        {
            let ghost merged_before = written@.take(kept as int);
            let (start, end) = written[next];
            if kept > 0 && start <= written[kept - 1].1 {
                let (merged_start, merged_end) = written[kept - 1];
                written[kept - 1] = (merged_start, merged_end.max(end));
                proof {
                    let merged = (merged_start, if end > merged_end { end } else { merged_end });
                    assert(written@[kept - 1] == merged);
                    assert(written@.take(kept as int) =~= merged_before.update(kept - 1, merged));
                    lemma_merge_into_last(merged_before, sorted, next as int, merged);
                }
            } else {
                written[kept] = (start, end);
                proof {
                    assert(written@.take(kept + 1) =~= merged_before.push(sorted[next as int]));
                    lemma_append_range(merged_before, sorted, next as int);
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
            assert forall|address: u64| !in_ranges(written@, address)
                implies #[trigger] memory_of(old@)(address).0 == memory_of(new@)(address).0 by {
                assert(in_ranges(sorted, address) == in_ranges(unsorted, address));
            }
        }
        proof {
            lemma_cursor_at_first_range(memory_old, memory_new, written@);
        }
        let ghost mut changes: Seq<Assignment<Architecture::PagePermission>> = Seq::empty();
        let (mut old_bytes, mut new_bytes) = (Vec::new(), Vec::new());
        // VERUS: no `for` over ranges a proof can count, so the ranges are walked by index.
        let written = written.as_slice();
        let mut range_index: usize = 0;
        while range_index < written.len()
            invariant
                range_index <= written@.len(),
                well_formed(written@),
                separated(written@),
                forall|address: u64| !in_ranges(written@, address) ==> #[trigger] memory_old(address).0 == memory_new(address).0,
                values_up_to(memory_old, memory_new, apply_all(memory_old, changes), written@,
                    if range_index < written@.len() { written@[range_index as int].0 as int } else { address_count() }),
                receives(hook_before, *mutation_hook, changes),
                memory_old == memory_new ==> changes.len() == 0 && *mutation_hook == hook_before,
                memory_old == memory_of(old@),
                memory_new == memory_of(new@),
            decreases written@.len() - range_index,
        {
            let (range_start, range_end) = written[range_index];
            // VERUS: a range of every address has no length a `usize` holds, so a range is compared in pieces of at most `usize::MAX` bytes.
            let mut piece_start = range_start;
            while piece_start < range_end
                invariant
                    range_index < written@.len(),
                    written@[range_index as int] == (range_start, range_end),
                    range_start <= piece_start <= range_end <= address_count(),
                    well_formed(written@),
                    separated(written@),
                    forall|address: u64| !in_ranges(written@, address) ==> #[trigger] memory_old(address).0 == memory_new(address).0,
                    values_up_to(memory_old, memory_new, apply_all(memory_old, changes), written@, piece_start as int),
                    receives(hook_before, *mutation_hook, changes),
                    memory_old == memory_new ==> changes.len() == 0 && *mutation_hook == hook_before,
                    memory_old == memory_of(old@),
                    memory_new == memory_of(new@),
                decreases range_end - piece_start,
            {
                let piece_end = if range_end - piece_start > usize::MAX as u128 { piece_start + usize::MAX as u128 } else { range_end };
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
                        start as int + length as int <= range_end as int,
                        range_end as int <= address_count(),
                        start as int == piece_start as int,
                        range_start <= piece_start,
                        range_index < written@.len(),
                        written@[range_index as int] == (range_start, range_end),
                        old_bytes@.len() == length,
                        new_bytes@.len() == length,
                        bytes_resolved(memory_old, old_bytes@, start),
                        bytes_resolved(memory_new, new_bytes@, start),
                        values_up_to(memory_old, memory_new, apply_all(memory_old, changes), written@, start + run_end),
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
                            assert(old_bytes@[at] == new_bytes@[at]);
                        }
                        lemma_same_bytes_passed(memory_old, memory_new, apply_all(memory_old, changes), written@, range_index as int,
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
                        lemma_run_sent(memory_old, memory_new, apply_all(memory_old, changes), written@, range_index as int,
                            start, old_bytes@, new_bytes@, run_start as int, run_end as int, change);
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
                    lemma_same_bytes_passed(memory_old, memory_new, apply_all(memory_old, changes), written@, range_index as int,
                        start, run_end as int, length as int);
                }
                piece_start = piece_end;
            }
            proof {
                lemma_cursor_to_next_range(memory_old, memory_new, apply_all(memory_old, changes), written@, range_index as int);
            }
            range_index += 1;
        }
        proof {
            lemma_bytes_sent(memory_old, memory_new, hook_before, *mutation_hook, changes, written@);
        }
        if !has_unpaired_permission {
            proof {
                lemma_bytes_were_all(memory_of(old@), memory_of(new@), hook_before, *mutation_hook);
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
        proof {
            broadcast use vstd::laws_cmp::group_laws_cmp;
        }
        bounds.as_mut_slice().sort_unstable();
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
            if kept == 0 || bounds[kept - 1] != bound {
                bounds[kept] = bound;
                proof {
                    assert(bounds@.take(kept + 1) =~= kept_before.push(sorted[next as int]));
                    lemma_append_bound(kept_before, sorted, next as int);
                }
                kept += 1;
            } else {
                proof {
                    lemma_repeat_bound(kept_before, sorted, next as int);
                }
            }
            next += 1;
        }
        bounds.truncate(kept);
        proof {
            assert(sorted.take(sorted.len() as int) =~= sorted);
        }
        proof {
            lemma_walked_bounds(range_map_values(&old_permissions), range_map_values(&new_permissions), old_bounds,
                unsorted_bounds, sorted_bounds, bounds@);
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
                lemma_bytes_then_permissions(memory_old, memory_new, hook_before, hook_after_bytes, *mutation_hook);
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
            proof {
                assert(end == all[end_index]);
            }
            let addresses = start..end;
            start = end;
            let permission = read_permission(&new_permissions, addresses.start);
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
                }
            } else {
                proof {
                    lemma_permission_interval::<Architecture>(memory_old, memory_new, apply_all(after_values, permission_changes),
                        apply_all(after_values, permission_changes), old_permissions, new_permissions, all, end_index, low, false);
                }
            }
        }
        proof {
            lemma_permissions_outside::<Architecture>(memory_old, memory_new, apply_all(after_values, permission_changes), old_permissions,
                new_permissions, all, low, all[all.len() - 1] as int);
            lemma_permissions_sent(memory_old, memory_new, hook_after_bytes, *mutation_hook, permission_changes);
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
    proof {
        assert(owned@ =~= bytes@);
    }
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
        assert(middle.applied_to(memory) == apply_all(before.applied_to(memory), changes));
    }
}

/// A sink that received two lists of changes one after the other received the joined list.
pub proof fn lemma_receives_append<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    before: Sink,
    middle: Sink,
    after: Sink,
    first: Seq<Assignment<Architecture::PagePermission>>,
    second: Seq<Assignment<Architecture::PagePermission>>,
)
    requires
        receives(before, middle, first),
        receives(middle, after, second),
    ensures
        receives(before, after, first + second),
{
    assert forall|memory: Memory<Architecture::PagePermission>|
        #[trigger] after.applied_to(memory) == apply_all(before.applied_to(memory), first + second) by {
        assert(after.applied_to(memory) == apply_all(middle.applied_to(memory), second));
        assert(middle.applied_to(memory) == apply_all(before.applied_to(memory), first));
        lemma_apply_all_append(before.applied_to(memory), first, second);
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
pub open spec fn in_ranges(ranges: Seq<(u128, u128)>, address: u64) -> bool {
    exists|index: int| #![trigger ranges[index]] 0 <= index < ranges.len() && ranges[index].0 <= address < ranges[index].1
}

/// Each range is not empty and ends at most one past the last address.
pub open spec fn well_formed(ranges: Seq<(u128, u128)>) -> bool {
    forall|index: int| 0 <= index < ranges.len() ==> #[trigger] ranges[index].0 < ranges[index].1 <= address_count()
}

/// The ranges start in ascending order.
pub open spec fn starts_ascending(ranges: Seq<(u128, u128)>) -> bool {
    forall|first: int, second: int| 0 <= first < second < ranges.len() ==> #[trigger] ranges[first].0 <= #[trigger] ranges[second].0
}

/// Each range ends before the next starts.
pub open spec fn separated(ranges: Seq<(u128, u128)>) -> bool {
    forall|first: int, second: int| 0 <= first < second < ranges.len() ==> #[trigger] ranges[first].1 < #[trigger] ranges[second].0
}

/// The two memories hold the same byte at every address outside the ranges, and the same permission everywhere
/// unless permissions may differ.
pub open spec fn same_outside<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    permissions_may_differ: bool,
) -> bool {
    &&& forall|address: u64| !in_ranges(ranges, address) ==> #[trigger] first(address).0 == second(address).0
    &&& !permissions_may_differ ==> forall|address: u64| #[trigger] first(address).1 == second(address).1
}

/// What recording an unpaired change does: the ranges grow to hold every byte it writes, and the flag is raised
/// when it gives a permission.
pub open spec fn records_unpaired<Permission>(
    change: Assignment<Permission>,
    ranges_before: Seq<(u128, u128)>,
    flag_before: bool,
    ranges_after: Seq<(u128, u128)>,
    flag_after: bool,
) -> bool {
    &&& ranges_before.is_prefix_of(ranges_after)
    &&& (well_formed(ranges_before) ==> well_formed(ranges_after))
    &&& forall|address: u64| #[trigger] writes_byte(change, address) ==> in_ranges(ranges_after, address)
    &&& flag_after == (flag_before || assigns_permission(change))
}

pub proof fn lemma_in_prefix(before: Seq<(u128, u128)>, after: Seq<(u128, u128)>, address: u64)
    requires
        before.is_prefix_of(after),
        in_ranges(before, address),
    ensures
        in_ranges(after, address),
{
    let index = choose|index: int| #![trigger before[index]] 0 <= index < before.len() && before[index].0 <= address < before[index].1;
    assert(after[index] == before[index]);
}

/// Memories alike outside some ranges are alike outside more ranges.
pub proof fn lemma_same_outside_widen<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    ranges_before: Seq<(u128, u128)>,
    flag_before: bool,
    ranges_after: Seq<(u128, u128)>,
    flag_after: bool,
)
    requires
        same_outside(first, second, ranges_before, flag_before),
        ranges_before.is_prefix_of(ranges_after),
        flag_before ==> flag_after,
    ensures
        same_outside(first, second, ranges_after, flag_after),
{
    assert forall|address: u64| !in_ranges(ranges_after, address) implies #[trigger] first(address).0 == second(address).0 by {
        if in_ranges(ranges_before, address) {
            lemma_in_prefix(ranges_before, ranges_after, address);
        }
    }
}

pub proof fn lemma_same_outside_symmetric<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    flag: bool,
)
    requires
        same_outside(first, second, ranges, flag),
    ensures
        same_outside(second, first, ranges, flag),
{
    assert forall|address: u64| !in_ranges(ranges, address) implies #[trigger] second(address).0 == first(address).0 by {
        assert(first(address).0 == second(address).0);
    }
    if !flag {
        assert forall|address: u64| #[trigger] second(address).1 == first(address).1 by {
            assert(first(address).1 == second(address).1);
        }
    }
}

pub proof fn lemma_same_outside_transitive<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    third: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    flag: bool,
)
    requires
        same_outside(first, second, ranges, flag),
        same_outside(second, third, ranges, flag),
    ensures
        same_outside(first, third, ranges, flag),
{
    assert forall|address: u64| !in_ranges(ranges, address) implies #[trigger] first(address).0 == third(address).0 by {
        assert(first(address).0 == second(address).0);
        assert(second(address).0 == third(address).0);
    }
    if !flag {
        assert forall|address: u64| #[trigger] first(address).1 == third(address).1 by {
            assert(first(address).1 == second(address).1);
            assert(second(address).1 == third(address).1);
        }
    }
}

/// The same change applied to two memories alike outside the ranges leaves them alike outside the ranges.
pub proof fn lemma_same_outside_apply<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    flag: bool,
    change: Assignment<Permission>,
)
    requires
        same_outside(first, second, ranges, flag),
    ensures
        same_outside(apply(first, change), apply(second, change), ranges, flag),
{
    assert forall|address: u64| !in_ranges(ranges, address) implies #[trigger] apply(first, change)(address).0 == apply(second, change)(address).0 by {
        assert(first(address).0 == second(address).0);
    }
    if !flag {
        assert forall|address: u64| #[trigger] apply(first, change)(address).1 == apply(second, change)(address).1 by {
            assert(first(address).1 == second(address).1);
        }
    }
}

/// An unpaired change applied to a list's memory leaves it alike, outside what was recorded, to what it was alike to.
pub proof fn lemma_unpaired_change<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
    index: int,
    other: Memory<Architecture::PagePermission>,
    ranges_before: Seq<(u128, u128)>,
    flag_before: bool,
    ranges_after: Seq<(u128, u128)>,
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
    lemma_same_outside_widen(before, other, ranges_before, flag_before, ranges_after, flag_after);
    assert forall|address: u64| !in_ranges(ranges_after, address) implies #[trigger] apply(before, change)(address).0 == other(address).0 by {
        assert(!writes_byte(change, address));
        assert(before(address).0 == other(address).0);
    }
    if !flag_after {
        assert forall|address: u64| #[trigger] apply(before, change)(address).1 == other(address).1 by {
            assert(before(address).1 == other(address).1);
        }
    }
}

/// The two lists made the same change at their next places, so they stay alike outside the recorded ranges.
pub proof fn lemma_paired_change<'first, 'second, Architecture: ProcessorArchitecture>(
    first: Seq<MemoryDelta<'first, Architecture>>,
    second: Seq<MemoryDelta<'second, Architecture>>,
    index: int,
    rest: int,
    candidate: int,
    ranges_before: Seq<(u128, u128)>,
    flag_before: bool,
    ranges_after: Seq<(u128, u128)>,
    flag_after: bool,
)
    requires
        0 <= index < first.len(),
        0 <= rest <= candidate < second.len(),
        first[index]@ == second[candidate]@,
        same_outside(memory_of(first.take(index)), memory_of(second.take(rest)), ranges_before, flag_before),
        same_outside(memory_of(second.take(candidate)), memory_of(second.take(rest)), ranges_after, flag_after),
        ranges_before.is_prefix_of(ranges_after),
        flag_before ==> flag_after,
    ensures
        same_outside(memory_of(first.take(index + 1)), memory_of(second.take(candidate + 1)), ranges_after, flag_after),
{
    let first_before = memory_of(first.take(index));
    let second_rest = memory_of(second.take(rest));
    let second_before = memory_of(second.take(candidate));
    lemma_same_outside_widen(first_before, second_rest, ranges_before, flag_before, ranges_after, flag_after);
    lemma_same_outside_symmetric(second_before, second_rest, ranges_after, flag_after);
    lemma_same_outside_transitive(first_before, second_rest, second_before, ranges_after, flag_after);
    lemma_same_outside_apply(first_before, second_before, ranges_after, flag_after, first[index]@);
    lemma_memory_of_take(first, index);
    lemma_memory_of_take(second, candidate);
}

/// What sorting the ranges gives: ascending starts, each still well formed, holding the same addresses.
pub proof fn lemma_sorted_ranges(before: Seq<(u128, u128)>, after: Seq<(u128, u128)>)
    requires
        well_formed(before),
        after.to_multiset() == before.to_multiset(),
        sorted_by_le(after, |first: (u128, u128), second: (u128, u128)| cmp_le(first, second)),
    ensures
        well_formed(after),
        starts_ascending(after),
        forall|address: u64| #[trigger] in_ranges(after, address) == in_ranges(before, address),
{
    broadcast use vstd::seq_lib::group_to_multiset_ensures;
    assert forall|index: int| 0 <= index < after.len() implies #[trigger] after[index].0 < after[index].1 <= address_count() by {
        assert(after.contains(after[index]));
        assert(after.to_multiset().count(after[index]) > 0);
        assert(before.to_multiset().count(after[index]) > 0);
        assert(before.contains(after[index]));
        let at = choose|at: int| 0 <= at < before.len() && before[at] == after[index];
        assert(before[at].0 < before[at].1 <= address_count());
    }
    assert forall|first: int, second: int| 0 <= first < second < after.len() implies #[trigger] after[first].0 <= #[trigger] after[second].0 by {
        assert(cmp_le(after[first], after[second]));
    }
    assert forall|address: u64| #[trigger] in_ranges(after, address) == in_ranges(before, address) by {
        if in_ranges(after, address) {
            let index = choose|index: int| #![trigger after[index]] 0 <= index < after.len() && after[index].0 <= address < after[index].1;
            assert(after.contains(after[index]));
            assert(after.to_multiset().count(after[index]) > 0);
            assert(before.to_multiset().count(after[index]) > 0);
            assert(before.contains(after[index]));
            let at = choose|at: int| 0 <= at < before.len() && before[at] == after[index];
            assert(before[at].0 <= address < before[at].1);
        }
        if in_ranges(before, address) {
            let index = choose|index: int| #![trigger before[index]] 0 <= index < before.len() && before[index].0 <= address < before[index].1;
            assert(before.contains(before[index]));
            assert(before.to_multiset().count(before[index]) > 0);
            assert(after.to_multiset().count(before[index]) > 0);
            assert(after.contains(before[index]));
            let at = choose|at: int| 0 <= at < after.len() && after[at] == before[index];
            assert(after[at].0 <= address < after[at].1);
        }
    }
}

/// The memory holds the second memory's bytes at each address of the ranges below the cursor, with the first
/// memory's permissions, and the first memory everywhere else.
pub open spec fn values_up_to<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    cursor: int,
) -> bool {
    forall|address: u64| #[trigger] memory(address)
        == if in_ranges(ranges, address) && address < cursor { (second(address).0, first(address).1) } else { first(address) }
}

/// No address of the separated ranges lies between the end of one and the start of the next.
pub proof fn lemma_between_ranges(ranges: Seq<(u128, u128)>, index: int, address: u64)
    requires
        well_formed(ranges),
        separated(ranges),
        0 <= index < ranges.len(),
        ranges[index].1 <= address,
        index + 1 < ranges.len() ==> address < ranges[index + 1].0,
    ensures
        !in_ranges(ranges, address),
{
    if in_ranges(ranges, address) {
        let at = choose|at: int| #![trigger ranges[at]] 0 <= at < ranges.len() && ranges[at].0 <= address < ranges[at].1;
        if at < index {
            assert(ranges[at].1 < ranges[index].0);
        } else if at > index + 1 {
            assert(ranges[index + 1].1 < ranges[at].0);
        }
    }
}

/// Before the start of the first range the cursor has passed no address of the ranges.
pub proof fn lemma_before_ranges(ranges: Seq<(u128, u128)>, address: u64)
    requires
        well_formed(ranges),
        separated(ranges),
        ranges.len() > 0,
        address < ranges[0].0,
    ensures
        !in_ranges(ranges, address),
{
    if in_ranges(ranges, address) {
        let at = choose|at: int| #![trigger ranges[at]] 0 <= at < ranges.len() && ranges[at].0 <= address < ranges[at].1;
        if at > 0 {
            assert(ranges[0].1 < ranges[at].0);
        }
    }
}

/// Moving the cursor over addresses that no range holds changes nothing.
pub proof fn lemma_cursor_skip<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
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
    assert forall|address: u64| #[trigger] memory(address)
        == if in_ranges(ranges, address) && address < to { (second(address).0, first(address).1) } else { first(address) } by {
        assert(memory(address)
            == if in_ranges(ranges, address) && address < from { (second(address).0, first(address).1) } else { first(address) });
    }
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

/// Between a walked bound and the next, a map whose bounds are among the walked ones gives one value.
#[verifier::spinoff_prover]
pub proof fn lemma_value_between<V>(values: spec_fn(u64) -> Option<V>, bounds: Seq<u64>, walked: Seq<u64>, index: int, address: u64)
    requires
        bounds_separate(values, bounds),
        bounds_within(bounds, walked),
        strictly_ascending(walked),
        0 <= index,
        index + 1 < walked.len(),
        walked[index] <= address < walked[index + 1],
    ensures
        values(address) == values(walked[index]),
{
    assert forall|at: int| 0 <= at < bounds.len() implies !(walked[index] < #[trigger] bounds[at] <= address) by {
        assert(walked.contains(bounds[at]));
        let place = choose|place: int| 0 <= place < walked.len() && walked[place] == bounds[at];
        if walked[index] < bounds[at] <= address {
            if place < index {
                assert(walked[place] < walked[index]);
            } else if place > index + 1 {
                assert(walked[index + 1] < walked[place]);
            }
        }
    }
    assert(values(walked[index]) == values(address));
}

/// Before the first walked bound and from the last on, a map whose bounds are among the walked ones gives no value.
#[verifier::spinoff_prover]
pub proof fn lemma_value_outside<V>(values: spec_fn(u64) -> Option<V>, bounds: Seq<u64>, walked: Seq<u64>, address: u64)
    requires
        bounds_separate(values, bounds),
        bounds_within(bounds, walked),
        strictly_ascending(walked),
        walked.len() == 0 || address < walked[0] || walked[walked.len() - 1] <= address,
    ensures
        values(address) is None,
{
    if values(address) is Some {
        let (below, above) = choose|below: int, above: int|
            #![trigger bounds[below], bounds[above]]
            0 <= below < bounds.len() && 0 <= above < bounds.len() && bounds[below] <= address < bounds[above];
        assert(walked.contains(bounds[below]));
        assert(walked.contains(bounds[above]));
        let low = choose|low: int| 0 <= low < walked.len() && walked[low] == bounds[below];
        let high = choose|high: int| 0 <= high < walked.len() && walked[high] == bounds[above];
        if low > 0 {
            assert(walked[0] < walked[low]);
        }
        if high < walked.len() - 1 {
            assert(walked[high] < walked[walked.len() - 1]);
        }
    }
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
    assert forall|index: int, address: u64| 0 <= index && index + 1 < walked.len() && walked[index] <= address < walked[index + 1]
        implies #[trigger] values(address) == values(#[trigger] walked[index]) by {
        lemma_value_between(values, bounds, walked, index, address);
    }
    assert forall|address: u64| (walked.len() == 0 || address < walked[0] || walked[walked.len() - 1] <= address)
        implies #[trigger] values(address) is None by {
        lemma_value_outside(values, bounds, walked, address);
    }
}

/// A byte the two memories hold alike is passed by the cursor with nothing to change.
pub proof fn lemma_same_byte_passed<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    index: int,
    address: u64,
)
    requires
        0 <= index < ranges.len(),
        ranges[index].0 <= address < ranges[index].1,
        values_up_to(first, second, memory, ranges, address as int),
        first(address).0 == second(address).0,
    ensures
        values_up_to(first, second, memory, ranges, address + 1),
{
    assert(in_ranges(ranges, address));
    assert forall|other: u64| #[trigger] memory(other)
        == if in_ranges(ranges, other) && other < address + 1 { (second(other).0, first(other).1) } else { first(other) } by {
        assert(memory(other)
            == if in_ranges(ranges, other) && other < address { (second(other).0, first(other).1) } else { first(other) });
    }
}

/// Bytes the two memories hold alike are passed by the cursor with nothing to change.
pub proof fn lemma_same_bytes_passed<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    index: int,
    start: u64,
    from: int,
    to: int,
)
    requires
        0 <= index < ranges.len(),
        0 <= from <= to,
        ranges[index].0 <= start + from,
        start + to <= ranges[index].1 <= address_count(),
        values_up_to(first, second, memory, ranges, start + from),
        forall|at: int| from <= at < to ==> #[trigger] first((start + at) as u64).0 == second((start + at) as u64).0,
    ensures
        values_up_to(first, second, memory, ranges, start + to),
{
    assert forall|address: u64| #[trigger] memory(address)
        == if in_ranges(ranges, address) && address < start + to { (second(address).0, first(address).1) } else { first(address) } by {
        assert(memory(address)
            == if in_ranges(ranges, address) && address < start + from { (second(address).0, first(address).1) } else { first(address) });
        if start + from <= address < start + to {
            assert(ranges[index].0 <= address < ranges[index].1);
            assert(in_ranges(ranges, address));
            let at = address - start;
            assert((start + at) as u64 == address);
            assert(first((start + at) as u64).0 == second((start + at) as u64).0);
        }
    }
}

/// A run of the second memory's bytes written from the cursor on moves the cursor past the run.
pub proof fn lemma_run_written<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    index: int,
    start: u64,
    second_bytes: Seq<u8>,
    run_start: int,
    run_end: int,
    change: Assignment<Permission>,
)
    requires
        0 <= index < ranges.len(),
        ranges[index].0 <= start,
        0 <= run_start < run_end <= second_bytes.len(),
        start + run_end <= ranges[index].1 <= address_count(),
        values_up_to(first, second, memory, ranges, start + run_start),
        bytes_resolved(second, second_bytes, start),
        change == (Assignment::<Permission>::ValueAssignment {
            start: (start + run_start) as u64,
            bytes: second_bytes.subrange(run_start, run_end),
        }),
    ensures
        values_up_to(first, second, apply(memory, change), ranges, start + run_end),
{
    let written_start = (start + run_start) as u64;
    assert forall|address: u64| #[trigger] apply(memory, change)(address)
        == if in_ranges(ranges, address) && address < start + run_end { (second(address).0, first(address).1) } else { first(address) } by {
        assert(memory(address)
            == if in_ranges(ranges, address) && address < start + run_start { (second(address).0, first(address).1) } else { first(address) });
        if written_start <= address < start + run_end {
            assert(ranges[index].0 <= address < ranges[index].1);
            assert(in_ranges(ranges, address));
            let at = address - start;
            assert(second_bytes[at] == second((start + at) as u64).0);
            assert(offset(written_start, address) == address - written_start);
        } else if address < written_start {
            assert(offset(written_start, address) >= run_end - run_start);
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
        assert(unsorted[at + index] == part[index]);
        assert(unsorted.contains(part[index]));
        assert(unsorted.to_multiset().count(part[index]) > 0);
        assert(sorted.to_multiset().count(part[index]) > 0);
        assert(sorted.contains(part[index]));
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
    assert forall|address: u64| #[trigger] memory(address) == second(address) by {
        assert(memory(address) == if low <= address < high { second(address) } else { (second(address).0, first(address).1) });
        if !(low <= address < high) {
            assert(range_map_values(&first_permissions)(address) is None);
            assert(range_map_values(&second_permissions)(address) is None);
            assert(first(address).1 == permission_or_default::<Architecture>(range_map_values(&first_permissions)(address)));
            assert(second(address).1 == permission_or_default::<Architecture>(range_map_values(&second_permissions)(address)));
        }
    }
    assert(memory =~= second);
}

/// What the one or two copies of a value assignment into a window leave: its byte wherever it writes, the window's
/// byte before everywhere else.
pub proof fn lemma_copy_window(
    before: Seq<u8>,
    after_first: Seq<u8>,
    after: Seq<u8>,
    start: u64,
    written_start: u64,
    bytes: Seq<u8>,
    length: int,
)
    requires
        0 <= length,
        start + length <= address_count(),
        bytes.len() < address_count(),
        before.len() == length,
        after_first.len() == length,
        after.len() == length,
        offset(written_start, start) < bytes.len() ==> forall|at: int| 0 <= at < length ==> #[trigger] after_first[at]
            == if at < bytes.len() - offset(written_start, start) { bytes[offset(written_start, start) + at] } else { before[at] },
        !(offset(written_start, start) < bytes.len()) ==> after_first == before,
        (offset(start, written_start) != 0 && offset(start, written_start) < length) ==> forall|at: int| 0 <= at < length ==> #[trigger] after[at]
            == if offset(start, written_start) <= at < offset(start, written_start) + bytes.len() { bytes[at - offset(start, written_start)] } else { after_first[at] },
        !(offset(start, written_start) != 0 && offset(start, written_start) < length) ==> after == after_first,
    ensures
        forall|at: int| 0 <= at < length ==> #[trigger] after[at]
            == if offset(written_start, (start + at) as u64) < bytes.len() {
                bytes[offset(written_start, (start + at) as u64)]
            } else {
                before[at]
            },
{
    let window_in_bytes = offset(written_start, start);
    let bytes_in_window = offset(start, written_start);
    assert forall|at: int| 0 <= at < length implies #[trigger] after[at]
        == if offset(written_start, (start + at) as u64) < bytes.len() {
            bytes[offset(written_start, (start + at) as u64)]
        } else {
            before[at]
        } by {
        let address = (start + at) as u64;
        assert(address == start + at);
        let first_region = window_in_bytes < bytes.len() && at < bytes.len() - window_in_bytes;
        let second_region = bytes_in_window != 0 && bytes_in_window < length && bytes_in_window <= at < bytes_in_window + bytes.len();
        if first_region {
            assert(offset(written_start, address) == window_in_bytes + at);
            assert(!second_region);
        } else if second_region {
            assert(written_start > start);
            assert(offset(written_start, address) == at - bytes_in_window);
        } else {
            assert(offset(written_start, address) >= bytes.len());
        }
    }
}

/// Merging a range that starts within the last merged one keeps the merged ranges apart and holding what the sorted
/// ones up to it hold.
pub proof fn lemma_merge_into_last(merged_before: Seq<(u128, u128)>, sorted: Seq<(u128, u128)>, next: int, new_last: (u128, u128))
    requires
        0 <= next < sorted.len(),
        well_formed(sorted),
        merged_before.len() > 0,
        well_formed(merged_before),
        separated(merged_before),
        merged_before.last().0 <= sorted[next].0 <= merged_before.last().1,
        new_last == (merged_before.last().0,
            if sorted[next].1 > merged_before.last().1 { sorted[next].1 } else { merged_before.last().1 }),
        forall|address: u64| #[trigger] in_ranges(merged_before, address) == in_ranges(sorted.take(next), address),
    ensures
        well_formed(merged_before.update(merged_before.len() - 1, new_last)),
        separated(merged_before.update(merged_before.len() - 1, new_last)),
        forall|address: u64| #[trigger] in_ranges(merged_before.update(merged_before.len() - 1, new_last), address)
            == in_ranges(sorted.take(next + 1), address),
{
    let last = merged_before.len() - 1;
    let merged_after = merged_before.update(last, new_last);
    assert forall|at: int| 0 <= at < last implies #[trigger] merged_after[at] == merged_before[at] by {}
    assert(merged_after[last].0 == merged_before[last].0);
    assert(merged_after[last].1 >= merged_before[last].1 && merged_after[last].1 >= sorted[next].1);
    assert(sorted[next].0 < sorted[next].1 <= address_count());
    assert(merged_before[last].0 < merged_before[last].1 <= address_count());
    assert forall|first: int, second: int| 0 <= first < second < merged_after.len() implies #[trigger] merged_after[first].1 < #[trigger] merged_after[second].0 by {
        assert(merged_before[first].1 < merged_before[second].0);
    }
    assert forall|address: u64| #[trigger] in_ranges(merged_after, address) == in_ranges(sorted.take(next + 1), address) by {
        let in_new = sorted[next].0 <= address < sorted[next].1;
        if in_ranges(sorted.take(next + 1), address) {
            let at = choose|at: int| #![trigger sorted.take(next + 1)[at]] 0 <= at < next + 1 && sorted.take(next + 1)[at].0 <= address < sorted.take(next + 1)[at].1;
            if at < next {
                assert(sorted.take(next)[at] == sorted[at]);
                assert(in_ranges(sorted.take(next), address));
                assert(in_ranges(merged_before, address));
                let merged_at = choose|merged_at: int| #![trigger merged_before[merged_at]] 0 <= merged_at < merged_before.len()
                    && merged_before[merged_at].0 <= address < merged_before[merged_at].1;
                assert(merged_after[merged_at].0 <= address < merged_after[merged_at].1);
            } else {
                assert(merged_after[last].0 <= address < merged_after[last].1);
            }
        }
        if in_ranges(merged_after, address) {
            let at = choose|at: int| #![trigger merged_after[at]] 0 <= at < merged_after.len() && merged_after[at].0 <= address < merged_after[at].1;
            if at < last || (merged_before[last].0 <= address < merged_before[last].1) {
                let merged_at = if at < last { at } else { last };
                assert(merged_before[merged_at].0 <= address < merged_before[merged_at].1);
                assert(in_ranges(merged_before, address));
                assert(in_ranges(sorted.take(next), address));
                let sorted_at = choose|sorted_at: int| #![trigger sorted.take(next)[sorted_at]] 0 <= sorted_at < next
                    && sorted.take(next)[sorted_at].0 <= address < sorted.take(next)[sorted_at].1;
                assert(sorted.take(next + 1)[sorted_at] == sorted.take(next)[sorted_at]);
            } else {
                assert(in_new);
                assert(sorted.take(next + 1)[next] == sorted[next]);
            }
        }
    }
}

/// Appending a range that starts after the last merged one ends keeps the merged ranges apart and holding what the
/// sorted ones up to it hold.
pub proof fn lemma_append_range(merged_before: Seq<(u128, u128)>, sorted: Seq<(u128, u128)>, next: int)
    requires
        0 <= next < sorted.len(),
        well_formed(sorted),
        well_formed(merged_before),
        separated(merged_before),
        merged_before.len() > 0 ==> merged_before.last().1 < sorted[next].0,
        forall|address: u64| #[trigger] in_ranges(merged_before, address) == in_ranges(sorted.take(next), address),
    ensures
        well_formed(merged_before.push(sorted[next])),
        separated(merged_before.push(sorted[next])),
        forall|address: u64| #[trigger] in_ranges(merged_before.push(sorted[next]), address) == in_ranges(sorted.take(next + 1), address),
{
    let merged_after = merged_before.push(sorted[next]);
    assert forall|at: int| 0 <= at < merged_before.len() implies #[trigger] merged_after[at] == merged_before[at] by {}
    assert(merged_after[merged_before.len() as int] == sorted[next]);
    assert(sorted[next].0 < sorted[next].1 <= address_count());
    assert forall|first: int, second: int| 0 <= first < second < merged_after.len() implies #[trigger] merged_after[first].1 < #[trigger] merged_after[second].0 by {
        if second == merged_before.len() {
            let last = merged_before.len() - 1;
            if first < last {
                assert(merged_before[first].1 < merged_before[last].0);
                assert(merged_before[last].0 < merged_before[last].1);
            }
        } else {
            assert(merged_before[first].1 < merged_before[second].0);
        }
    }
    assert forall|address: u64| #[trigger] in_ranges(merged_after, address) == in_ranges(sorted.take(next + 1), address) by {
        if in_ranges(sorted.take(next + 1), address) {
            let at = choose|at: int| #![trigger sorted.take(next + 1)[at]] 0 <= at < next + 1 && sorted.take(next + 1)[at].0 <= address < sorted.take(next + 1)[at].1;
            if at < next {
                assert(sorted.take(next)[at] == sorted[at]);
                assert(in_ranges(sorted.take(next), address));
                assert(in_ranges(merged_before, address));
                let merged_at = choose|merged_at: int| #![trigger merged_before[merged_at]] 0 <= merged_at < merged_before.len()
                    && merged_before[merged_at].0 <= address < merged_before[merged_at].1;
                assert(merged_after[merged_at] == merged_before[merged_at]);
            } else {
                assert(merged_after[merged_before.len() as int] == sorted[next]);
            }
        }
        if in_ranges(merged_after, address) {
            let at = choose|at: int| #![trigger merged_after[at]] 0 <= at < merged_after.len() && merged_after[at].0 <= address < merged_after[at].1;
            if at < merged_before.len() {
                assert(merged_before[at] == merged_after[at]);
                assert(in_ranges(merged_before, address));
                let sorted_at = choose|sorted_at: int| #![trigger sorted.take(next)[sorted_at]] 0 <= sorted_at < next
                    && sorted.take(next)[sorted_at].0 <= address < sorted.take(next)[sorted_at].1;
                assert(sorted.take(next + 1)[sorted_at] == sorted.take(next)[sorted_at]);
            } else {
                assert(sorted.take(next + 1)[next] == sorted[next]);
            }
        }
    }
}

/// Appending a bound greater than the last kept one keeps the kept bounds rising and holding the sorted ones up to it.
pub proof fn lemma_append_bound(kept_before: Seq<u64>, sorted: Seq<u64>, next: int)
    requires
        0 <= next < sorted.len(),
        strictly_ascending(kept_before),
        kept_before.len() > 0 ==> kept_before.last() < sorted[next],
        forall|bound: u64| #[trigger] kept_before.contains(bound) == sorted.take(next).contains(bound),
    ensures
        strictly_ascending(kept_before.push(sorted[next])),
        forall|bound: u64| #[trigger] kept_before.push(sorted[next]).contains(bound) == sorted.take(next + 1).contains(bound),
{
    let kept_after = kept_before.push(sorted[next]);
    assert forall|at: int| 0 <= at < kept_before.len() implies #[trigger] kept_after[at] == kept_before[at] by {}
    assert(kept_after[kept_before.len() as int] == sorted[next]);
    assert forall|first: int, second: int| 0 <= first < second < kept_after.len() implies #[trigger] kept_after[first] < #[trigger] kept_after[second] by {
        if second == kept_before.len() && first < kept_before.len() - 1 {
            assert(kept_before[first] < kept_before[kept_before.len() - 1]);
        }
    }
    assert(sorted.take(next + 1) =~= sorted.take(next).push(sorted[next]));
    assert forall|bound: u64| #[trigger] kept_after.contains(bound) == sorted.take(next + 1).contains(bound) by {
        if kept_after.contains(bound) && bound != sorted[next] {
            let at = choose|at: int| 0 <= at < kept_after.len() && #[trigger] kept_after[at] == bound;
            assert(kept_before[at] == bound);
            assert(kept_before.contains(bound));
        }
        if sorted.take(next + 1).contains(bound) && bound != sorted[next] {
            let at = choose|at: int| 0 <= at < next + 1 && #[trigger] sorted.take(next + 1)[at] == bound;
            assert(sorted.take(next)[at] == bound);
            assert(sorted.take(next).contains(bound));
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

/// A bound equal to the last kept one adds nothing.
pub proof fn lemma_repeat_bound(kept_before: Seq<u64>, sorted: Seq<u64>, next: int)
    requires
        0 <= next < sorted.len(),
        kept_before.len() > 0,
        kept_before.last() == sorted[next],
        forall|bound: u64| #[trigger] kept_before.contains(bound) == sorted.take(next).contains(bound),
    ensures
        forall|bound: u64| #[trigger] kept_before.contains(bound) == sorted.take(next + 1).contains(bound),
{
    assert(sorted.take(next + 1) =~= sorted.take(next).push(sorted[next]));
    assert(kept_before.contains(sorted[next])) by {
        assert(kept_before[kept_before.len() - 1] == sorted[next]);
    }
    assert forall|bound: u64| #[trigger] kept_before.contains(bound) == sorted.take(next + 1).contains(bound) by {
        if sorted.take(next + 1).contains(bound) && bound != sorted[next] {
            let at = choose|at: int| 0 <= at < next + 1 && #[trigger] sorted.take(next + 1)[at] == bound;
            assert(sorted.take(next)[at] == bound);
        }
        if kept_before.contains(bound) {
            assert(sorted.take(next).contains(bound));
            let at = choose|at: int| 0 <= at < next && #[trigger] sorted.take(next)[at] == bound;
            assert(sorted.take(next + 1)[at] == bound);
        }
    }
}

/// Applying one more change applies it last.
pub proof fn lemma_apply_all_push<Permission>(memory: Memory<Permission>, changes: Seq<Assignment<Permission>>, change: Assignment<Permission>)
    ensures
        apply_all(memory, changes.push(change)) == apply(apply_all(memory, changes), change),
{
    assert(changes.push(change).drop_last() =~= changes);
}

/// One list of addresses is the start of another, which is the start of a third.
pub proof fn lemma_prefix_transitive(first: Seq<(u128, u128)>, second: Seq<(u128, u128)>, third: Seq<(u128, u128)>)
    requires
        first.is_prefix_of(second),
        second.is_prefix_of(third),
    ensures
        first.is_prefix_of(third),
{
    assert(third.take(second.len() as int) == second);
    assert(second.take(first.len() as int) == first);
    assert(third.take(first.len() as int) =~= first);
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

/// The bytes sent are all the diff sends where the two memories hold the same permissions.
pub proof fn lemma_bytes_were_all<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    before: Sink,
    after: Sink,
)
    requires
        bytes_sent(first, second, before, after),
        forall|address: u64| #[trigger] first(address).1 == second(address).1,
    ensures
        exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(first, changes)]
            receives(before, after, changes) && apply_all(first, changes) == second,
        first == second ==> after == before,
{
    let changes = choose|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(first, changes)]
        receives(before, after, changes) && apply_all(first, changes) == bytes_of_with_permissions_of(second, first);
    assert(bytes_of_with_permissions_of(second, first) =~= second) by {
        assert forall|address: u64| #[trigger] bytes_of_with_permissions_of(second, first)(address) == second(address) by {
            assert(first(address).1 == second(address).1);
        }
    }
    assert(apply_all(first, changes) == second);
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
    lemma_receives_append(before, middle, after, byte_changes, permission_changes);
    lemma_apply_all_append(first, byte_changes, permission_changes);
    assert(apply_all(first, byte_changes + permission_changes) == second);
}

/// Before the first range, or anywhere when there is none, the cursor has passed no address of the ranges.
pub proof fn lemma_cursor_at_first_range<Permission>(first: Memory<Permission>, second: Memory<Permission>, ranges: Seq<(u128, u128)>)
    requires
        well_formed(ranges),
        separated(ranges),
    ensures
        values_up_to(first, second, apply_all(first, Seq::empty()), ranges,
            if 0 < ranges.len() { ranges[0].0 as int } else { address_count() }),
{
    if ranges.len() > 0 {
        assert forall|address: u64| address < ranges[0].0 implies !#[trigger] in_ranges(ranges, address) by {
            lemma_before_ranges(ranges, address);
        }
    }
}

/// From the end of one range the cursor moves to the start of the next, or past the last address after the last.
pub proof fn lemma_cursor_to_next_range<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    index: int,
)
    requires
        well_formed(ranges),
        separated(ranges),
        0 <= index < ranges.len(),
        values_up_to(first, second, memory, ranges, ranges[index].1 as int),
    ensures
        values_up_to(first, second, memory, ranges, if index + 1 < ranges.len() { ranges[index + 1].0 as int } else { address_count() }),
{
    if index + 1 < ranges.len() {
        assert(ranges[index].1 < ranges[index + 1].0);
        assert forall|address: u64| ranges[index].1 <= address < ranges[index + 1].0 implies !#[trigger] in_ranges(ranges, address) by {
            lemma_between_ranges(ranges, index, address);
        }
        lemma_cursor_skip(first, second, memory, ranges, ranges[index].1 as int, ranges[index + 1].0 as int);
    } else {
        assert(ranges[index].0 < ranges[index].1 <= address_count());
        assert forall|address: u64| ranges[index].1 <= address < address_count() implies !#[trigger] in_ranges(ranges, address) by {
            lemma_between_ranges(ranges, index, address);
        }
        lemma_cursor_skip(first, second, memory, ranges, ranges[index].1 as int, address_count());
    }
}

/// A byte the two windows hold alike is passed by the cursor with nothing to change.
pub proof fn lemma_same_resolved_byte_passed<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    index: int,
    start: u64,
    first_bytes: Seq<u8>,
    second_bytes: Seq<u8>,
    at: int,
)
    requires
        0 <= index < ranges.len(),
        ranges[index].0 <= start,
        0 <= at < first_bytes.len(),
        first_bytes.len() == second_bytes.len(),
        start + first_bytes.len() <= ranges[index].1 <= address_count(),
        bytes_resolved(first, first_bytes, start),
        bytes_resolved(second, second_bytes, start),
        first_bytes[at] == second_bytes[at],
        values_up_to(first, second, memory, ranges, start + at),
    ensures
        values_up_to(first, second, memory, ranges, start + at + 1),
{
    assert(first_bytes[at] == first((start + at) as u64).0);
    assert(second_bytes[at] == second((start + at) as u64).0);
    assert((start + at) as u64 == start + at);
    lemma_same_byte_passed(first, second, memory, ranges, index, (start + at) as u64);
}

/// A run of bytes the two windows hold differently, sent as the second's, moves the cursor past the run, and shows
/// the two memories differ.
pub proof fn lemma_run_sent<Permission>(
    first: Memory<Permission>,
    second: Memory<Permission>,
    memory: Memory<Permission>,
    ranges: Seq<(u128, u128)>,
    index: int,
    start: u64,
    first_bytes: Seq<u8>,
    second_bytes: Seq<u8>,
    run_start: int,
    run_end: int,
    change: Assignment<Permission>,
)
    requires
        0 <= index < ranges.len(),
        ranges[index].0 <= start,
        0 <= run_start < run_end <= second_bytes.len(),
        first_bytes.len() == second_bytes.len(),
        start + second_bytes.len() <= ranges[index].1 <= address_count(),
        values_up_to(first, second, memory, ranges, start + run_start),
        bytes_resolved(first, first_bytes, start),
        bytes_resolved(second, second_bytes, start),
        first_bytes[run_start] != second_bytes[run_start],
        change == (Assignment::<Permission>::ValueAssignment {
            start: (start + run_start) as u64,
            bytes: second_bytes.subrange(run_start, run_end),
        }),
    ensures
        values_up_to(first, second, apply(memory, change), ranges, start + run_end),
        first != second,
{
    lemma_run_written(first, second, memory, ranges, index, start, second_bytes, run_start, run_end, change);
    assert(first_bytes[run_start] == first((start + run_start) as u64).0);
    assert(second_bytes[run_start] == second((start + run_start) as u64).0);
}

/// Past the last address, the memory holds the second's bytes with the first's permissions everywhere.
pub proof fn lemma_bytes_sent<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    before: Sink,
    after: Sink,
    changes: Seq<Assignment<Architecture::PagePermission>>,
    ranges: Seq<(u128, u128)>,
)
    requires
        receives(before, after, changes),
        values_up_to(first, second, apply_all(first, changes), ranges, address_count()),
        forall|address: u64| !in_ranges(ranges, address) ==> #[trigger] first(address).0 == second(address).0,
        first == second ==> changes.len() == 0 && after == before,
    ensures
        bytes_sent(first, second, before, after),
{
    assert(apply_all(first, changes) =~= bytes_of_with_permissions_of(second, first)) by {
        assert forall|address: u64| #[trigger] apply_all(first, changes)(address) == bytes_of_with_permissions_of(second, first)(address) by {
            assert(apply_all(first, changes)(address)
                == if in_ranges(ranges, address) && address < address_count() { (second(address).0, first(address).1) } else { first(address) });
        }
    }
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

/// The bounds walked after sorting and dropping repeats separate each map's values.
pub proof fn lemma_walked_bounds<V>(
    first_values: spec_fn(u64) -> Option<V>,
    second_values: spec_fn(u64) -> Option<V>,
    first_bounds: Seq<u64>,
    unsorted: Seq<u64>,
    sorted: Seq<u64>,
    walked: Seq<u64>,
)
    requires
        bounds_separate(first_values, first_bounds.skip(0)),
        first_bounds.is_prefix_of(unsorted),
        bounds_separate(second_values, unsorted.skip(first_bounds.len() as int)),
        sorted.to_multiset() == unsorted.to_multiset(),
        forall|bound: u64| #[trigger] walked.contains(bound) == sorted.contains(bound),
        strictly_ascending(walked),
    ensures
        values_by_bounds(first_values, walked),
        values_by_bounds(second_values, walked),
{
    let second_bounds = unsorted.skip(first_bounds.len() as int);
    assert(first_bounds.skip(0) =~= first_bounds);
    assert forall|at: int| 0 <= at < first_bounds.len() implies #[trigger] first_bounds[at] == unsorted[0 + at] by {
        assert(unsorted.take(first_bounds.len() as int) == first_bounds);
        assert(unsorted.take(first_bounds.len() as int)[at] == unsorted[at]);
    }
    lemma_bounds_kept(first_bounds, 0, unsorted, sorted, walked);
    lemma_bounds_kept(second_bounds, first_bounds.len() as int, unsorted, sorted, walked);
    lemma_values_between(first_values, first_bounds, walked);
    lemma_values_between(second_values, second_bounds, walked);
}

/// A window resolved for a list's first mutations, rewritten where the next one writes, is resolved for one more.
#[verifier::spinoff_prover]
pub proof fn lemma_resolved_after_change<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
    index: int,
    before: Seq<u8>,
    after: Seq<u8>,
    start: u64,
)
    requires
        0 <= index < mutations.len(),
        before.len() == after.len(),
        start + before.len() <= address_count(),
        bytes_resolved(memory_of(mutations.take(index)), before, start),
        match mutations[index]@ {
            Assignment::ValueAssignment { start: written_start, bytes } => forall|at: int| 0 <= at < before.len() ==> #[trigger] after[at]
                == if offset(written_start, (start + at) as u64) < bytes.len() { bytes[offset(written_start, (start + at) as u64)] } else { before[at] },
            Assignment::PermissionAssignment { .. } => after == before,
        },
    ensures
        bytes_resolved(memory_of(mutations.take(index + 1)), after, start),
{
    lemma_memory_of_take(mutations, index);
    let memory = memory_of(mutations.take(index));
    let change = mutations[index]@;
    assert forall|at: int| 0 <= at < after.len() implies #[trigger] after[at] == apply(memory, change)((start + at) as u64).0 by {
        assert(before[at] == memory((start + at) as u64).0);
    }
}

/// Recording the one or two ranges a value assignment writes, around the last address, records every byte it writes.
#[verifier::spinoff_prover]
pub proof fn lemma_value_reach_recorded<Permission>(
    start: u64,
    bytes: Seq<u8>,
    before: Seq<(u128, u128)>,
    below_wrap: Seq<(u128, u128)>,
    after: Seq<(u128, u128)>,
)
    requires
        bytes.len() <= usize::MAX,
        ({
            let end = start + bytes.len();
            let below_end = if end < address_count() { end } else { address_count() };
            &&& below_wrap == if start < below_end { before.push((start as u128, below_end as u128)) } else { before }
            &&& after == if end > address_count() { below_wrap.push((0u128, (end - address_count()) as u128)) } else { below_wrap }
        }),
    ensures
        forall|flag: bool| records_unpaired(
            Assignment::<Permission>::ValueAssignment { start, bytes }, before, flag, after, flag),
{
    let end = start + bytes.len();
    let below_end = if end < address_count() { end } else { address_count() };
    assert(after.take(before.len() as int) =~= before);
    assert(before.is_prefix_of(after));
    if well_formed(before) {
        assert forall|index: int| 0 <= index < after.len()
            implies #[trigger] after[index].0 < after[index].1 <= address_count() by {
            if index < before.len() {
                assert(below_wrap[index] == before[index]);
                assert(after[index] == below_wrap[index]);
            } else if index < below_wrap.len() {
                assert(after[index] == below_wrap[index]);
            }
        }
    }
    let change = Assignment::<Permission>::ValueAssignment { start, bytes };
    assert forall|address: u64| #[trigger] writes_byte(change, address) implies in_ranges(after, address) by {
        if start <= address {
            let at = before.len() as int;
            assert(below_wrap[at] == (start as u128, below_end as u128));
            assert(after[at] == below_wrap[at]);
            assert(after[at].0 <= address < after[at].1);
        } else {
            let at = below_wrap.len() as int;
            assert(after[at] == (0u128, (end - address_count()) as u128));
            assert(after[at].0 <= address < after[at].1);
        }
    }
}

} // verus!
