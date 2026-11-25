//! The body of `diff_memory` with its proof: the owner's code, changed only where a `// VERUS:` line says why.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use rangemap::RangeMap;
#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::slice::{cmp_le, sorted_by_le};

#[cfg(verus_keep_ghost)]
use crate::memory_proof::*;
use crate::trusted::*;
use crate::*;

verus! {

/// Calls the hook with each mutation that turns the memory of the first list into the memory of the second, bytes before permissions.
#[verifier::spinoff_prover]
pub fn diff_memory<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
    mutation_hook: &mut impl MemorySink<Architecture>,
)
    ensures
        exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(memory_of(first@), changes)]
            receives(*old(mutation_hook), *final(mutation_hook), changes)
                && apply_all(memory_of(first@), changes) == memory_of(second@),
        memory_of(first@) == memory_of(second@) ==> *final(mutation_hook) == *old(mutation_hook),
{
    let ghost hook_before = *mutation_hook;
    // Mutations both lists make in the same order leave the same memory wherever no other mutation reaches, so only
    // what the unpaired mutations reach can differ.
    // VERUS: a function this long is more than the verifier checks at once, so each step is a function of its own.
    let (mut written, has_unpaired_permission) = record_unpaired_reach(first, second);
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
    merge_ranges(&mut written);
    proof {
        assert forall|address: u64| !in_ranges(written@, address)
            implies #[trigger] memory_of(first@)(address).0 == memory_of(second@)(address).0 by {
            assert(in_ranges(sorted, address) == in_ranges(unsorted, address));
        }
    }
    send_changed_bytes(first, second, written.as_slice(), mutation_hook);
    if !has_unpaired_permission {
        proof {
            lemma_bytes_were_all(memory_of(first@), memory_of(second@), hook_before, *mutation_hook);
        }
        return;
    }
    let ghost hook_after_bytes = *mutation_hook;
    send_changed_permissions(first, second, mutation_hook);
    proof {
        lemma_bytes_then_permissions(memory_of(first@), memory_of(second@), hook_before, hook_after_bytes, *mutation_hook);
    }
}

/// Returns ranges that hold every byte an unpaired mutation of either list writes, and whether an unpaired one gives
/// a permission: outside those the two lists leave the same memory.
#[verifier::spinoff_prover]
fn record_unpaired_reach<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
) -> (reach: (Vec<(u128, u128)>, bool))
    ensures
        well_formed(reach.0@),
        same_outside(memory_of(first@), memory_of(second@), reach.0@, reach.1),
{
    proof {
        Architecture::equality_is_identity();
        assert(first@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
        assert(second@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
        assert(memory_of(first@.take(0)) =~= memory_of(second@.take(0)));
    }
    let ghost memory_first = memory_of(first@);
    let ghost memory_second = memory_of(second@);
    let (mut written, mut has_unpaired_permission) = (Vec::new(), false);
    // VERUS: no `position` with a closure, so both lists are walked by index.
    let mut rest_start: usize = 0;
    let mut index: usize = 0;
    while index < first.len()
        invariant
            index <= first@.len(),
            rest_start <= second@.len(),
            obeys_concrete_eq::<Architecture::PagePermission>(),
            well_formed(written@),
            same_outside(memory_of(first@.take(index as int)), memory_of(second@.take(rest_start as int)), written@, has_unpaired_permission),
        decreases first@.len() - index,
    {
        let mutation = &first[index];
        let mut candidate = rest_start;
        let mut found = false;
        while !found && candidate < second.len()
            invariant
                index < first@.len(),
                    rest_start <= candidate <= second@.len(),
                *mutation == first@[index as int],
                found ==> candidate < second@.len() && first@[index as int]@ == second@[candidate as int]@,
                obeys_concrete_eq::<Architecture::PagePermission>(),
            decreases second@.len() - candidate + if found { 0int } else { 1int },
        {
            if is_same_mutation(mutation, &second[candidate]) {
                found = true;
            } else {
                candidate += 1;
            }
        }
        if found {
            let ghost paired_written = written@;
            let ghost paired_flag = has_unpaired_permission;
            let mut skipped = rest_start;
            while skipped < candidate
                invariant
                            rest_start <= skipped <= candidate < second@.len(),
                    well_formed(written@),
                    paired_written.is_prefix_of(written@),
                    paired_flag ==> has_unpaired_permission,
                    same_outside(memory_of(second@.take(skipped as int)), memory_of(second@.take(rest_start as int)), written@,
                        has_unpaired_permission),
                decreases candidate - skipped,
            {
                let ghost skipped_written = written@;
                let ghost skipped_flag = has_unpaired_permission;
                add_unpaired(&second[skipped], &mut written, &mut has_unpaired_permission);
                proof {
                    lemma_unpaired_change(second@, skipped as int, memory_of(second@.take(rest_start as int)), skipped_written,
                        skipped_flag, written@, has_unpaired_permission);
                    lemma_prefix_transitive(paired_written, skipped_written, written@);
                }
                skipped += 1;
            }
            proof {
                if rest_start == candidate {
                    assert(same_outside(memory_of(second@.take(skipped as int)), memory_of(second@.take(rest_start as int)), written@,
                        has_unpaired_permission));
                }
                lemma_paired_change(first@, second@, index as int, rest_start as int, candidate as int, paired_written, paired_flag,
                    written@, has_unpaired_permission);
            }
            rest_start = candidate + 1;
        } else {
            let ghost unpaired_written = written@;
            let ghost unpaired_flag = has_unpaired_permission;
            add_unpaired(mutation, &mut written, &mut has_unpaired_permission);
            proof {
                lemma_unpaired_change(first@, index as int, memory_of(second@.take(rest_start as int)), unpaired_written,
                    unpaired_flag, written@, has_unpaired_permission);
            }
        }
        index += 1;
    }
    proof {
        assert(first@.take(first@.len() as int) =~= first@);
        lemma_same_outside_symmetric(memory_first, memory_of(second@.take(rest_start as int)), written@, has_unpaired_permission);
    }
    while rest_start < second.len()
        invariant
            rest_start <= second@.len(),
            well_formed(written@),
            same_outside(memory_of(second@.take(rest_start as int)), memory_first, written@, has_unpaired_permission),
            memory_first == memory_of(first@),
        decreases second@.len() - rest_start,
    {
        let ghost rest_written = written@;
        let ghost rest_flag = has_unpaired_permission;
        add_unpaired(&second[rest_start], &mut written, &mut has_unpaired_permission);
        proof {
            lemma_unpaired_change(second@, rest_start as int, memory_first, rest_written, rest_flag, written@, has_unpaired_permission);
        }
        rest_start += 1;
    }
    proof {
        assert(second@.take(second@.len() as int) =~= second@);
        lemma_same_outside_symmetric(memory_second, memory_first, written@, has_unpaired_permission);
    }
    (written, has_unpaired_permission)
}

/// Sends a mutation for each run of bytes the two lists leave different within the ranges, which hold every byte
/// they leave different.
#[verifier::spinoff_prover]
fn send_changed_bytes<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
    written: &[(u128, u128)],
    mutation_hook: &mut impl MemorySink<Architecture>,
)
    requires
        well_formed(written@),
        separated(written@),
        forall|address: u64| !in_ranges(written@, address) ==> #[trigger] memory_of(first@)(address).0 == memory_of(second@)(address).0,
    ensures
        bytes_sent(memory_of(first@), memory_of(second@), *old(mutation_hook), *final(mutation_hook)),
{
    let ghost memory_first = memory_of(first@);
    let ghost memory_second = memory_of(second@);
    let ghost hook_before = *mutation_hook;
    proof {
        lemma_cursor_at_first_range(memory_first, memory_second, written@);
    }
    let ghost mut changes: Seq<Assignment<Architecture::PagePermission>> = Seq::empty();
    let (mut first_bytes, mut second_bytes) = (Vec::new(), Vec::new());
    // VERUS: no `for` over ranges a proof can count, so the ranges are walked by index.
    let mut range_index: usize = 0;
    while range_index < written.len()
        invariant
            range_index <= written@.len(),
            well_formed(written@),
            separated(written@),
            forall|address: u64| !in_ranges(written@, address) ==> #[trigger] memory_first(address).0 == memory_second(address).0,
            values_up_to(memory_first, memory_second, apply_all(memory_first, changes), written@,
                if range_index < written@.len() { written@[range_index as int].0 as int } else { address_count() }),
            receives(hook_before, *mutation_hook, changes),
            memory_first == memory_second ==> changes.len() == 0 && *mutation_hook == hook_before,
            memory_first == memory_of(first@),
            memory_second == memory_of(second@),
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
                forall|address: u64| !in_ranges(written@, address) ==> #[trigger] memory_first(address).0 == memory_second(address).0,
                values_up_to(memory_first, memory_second, apply_all(memory_first, changes), written@, piece_start as int),
                receives(hook_before, *mutation_hook, changes),
                memory_first == memory_second ==> changes.len() == 0 && *mutation_hook == hook_before,
                memory_first == memory_of(first@),
                memory_second == memory_of(second@),
            decreases range_end - piece_start,
        {
            let piece_end = if range_end - piece_start > usize::MAX as u128 { piece_start + usize::MAX as u128 } else { range_end };
            let start = piece_start as u64;
            let length = (piece_end - piece_start) as usize;
            resolve_bytes(first, start, length, &mut first_bytes);
            resolve_bytes(second, start, length, &mut second_bytes);
            // Each run of bytes the two lists leave different is one mutation.
            // VERUS: no `find` with a closure, so each run's start and end are found by index.
            let mut run_end = usize::MIN;
            while run_end < length
                invariant
                    run_end <= length,
                    start as int + length as int <= range_end as int,
                    range_end as int <= address_count(),
                    start as int == piece_start as int,
                    range_start <= piece_start,
                    range_index < written@.len(),
                    written@[range_index as int] == (range_start, range_end),
                    first_bytes@.len() == length,
                    second_bytes@.len() == length,
                    bytes_resolved(memory_first, first_bytes@, start),
                    bytes_resolved(memory_second, second_bytes@, start),
                    values_up_to(memory_first, memory_second, apply_all(memory_first, changes), written@, start + run_end),
                    receives(hook_before, *mutation_hook, changes),
                    memory_first == memory_second ==> changes.len() == 0 && *mutation_hook == hook_before,
                decreases length - run_end,
            {
                let mut run_start = run_end;
                while run_start < length && first_bytes[run_start] == second_bytes[run_start]
                    invariant
                        run_end <= run_start <= length,
                        start as int + length as int <= range_end as int,
                        range_end as int <= address_count(),
                        range_start <= start,
                        range_index < written@.len(),
                        written@[range_index as int] == (range_start, range_end),
                        first_bytes@.len() == length,
                        second_bytes@.len() == length,
                        bytes_resolved(memory_first, first_bytes@, start),
                        bytes_resolved(memory_second, second_bytes@, start),
                        values_up_to(memory_first, memory_second, apply_all(memory_first, changes), written@, start + run_start),
                    decreases length - run_start,
                {
                    proof {
                        lemma_same_resolved_byte_passed(memory_first, memory_second, apply_all(memory_first, changes), written@,
                            range_index as int, start, first_bytes@, second_bytes@, run_start as int);
                    }
                    run_start += 1;
                }
                run_end = run_start;
                if run_start < length {
                    while run_end < length && first_bytes[run_end] != second_bytes[run_end]
                        invariant
                            run_start <= run_end <= length,
                            first_bytes@.len() == length,
                            second_bytes@.len() == length,
                            run_start < length,
                            first_bytes@[run_start as int] != second_bytes@[run_start as int],
                        decreases length - run_end,
                    {
                        run_end += 1;
                    }
                    let ghost hook_now = *mutation_hook;
                    let mutation = MemoryDelta::ValueAssignment {
                        start: start + run_start as u64,
                        // VERUS: no range index of a `Vec` it reads, so the run is cut from its slice.
                        bytes: Cow::Borrowed(&second_bytes.as_slice()[run_start..run_end]),
                    };
                    let ghost change = mutation@;
                    mutation_hook.on_memory_mutated(mutation);
                    proof {
                        lemma_run_sent(memory_first, memory_second, apply_all(memory_first, changes), written@, range_index as int,
                            start, first_bytes@, second_bytes@, run_start as int, run_end as int, change);
                        lemma_receives_one(hook_before, hook_now, *mutation_hook, changes, change);
                        lemma_apply_all_push(memory_first, changes, change);
                        changes = changes.push(change);
                    }
                }
            }
            piece_start = piece_end;
        }
        proof {
            lemma_cursor_to_next_range(memory_first, memory_second, apply_all(memory_first, changes), written@, range_index as int);
        }
        range_index += 1;
    }
    proof {
        lemma_bytes_sent(memory_first, memory_second, hook_before, *mutation_hook, changes, written@);
    }
}

/// Sends a mutation for each interval between neighboring bounds of the two lists' permissions where they leave
/// different ones, which turns the second list's bytes with the first's permissions into the second list's memory.
#[verifier::spinoff_prover]
fn send_changed_permissions<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
    mutation_hook: &mut impl MemorySink<Architecture>,
)
    ensures
        permissions_sent(memory_of(first@), memory_of(second@), *old(mutation_hook), *final(mutation_hook)),
{
    proof {
        Architecture::equality_is_identity();
    }
    let ghost memory_first = memory_of(first@);
    let ghost memory_second = memory_of(second@);
    let ghost hook_before = *mutation_hook;
    let ghost after_values = bytes_of_with_permissions_of(memory_second, memory_first);
    let ghost mut changes: Seq<Assignment<Architecture::PagePermission>> = Seq::empty();
    let first_permissions = resolve_permissions(first);
    let second_permissions = resolve_permissions(second);
    // VERUS: no iterator chain over the range maps, so each map's bounds are appended to one list made to hold them, as
    // `collect` would size it.
    let mut bounds: Vec<u64> =
        Vec::with_capacity(range_map_len(&first_permissions).saturating_add(range_map_len(&second_permissions)).saturating_mul(2));
    append_bounds(&first_permissions, &mut bounds);
    let ghost first_bounds = bounds@;
    append_bounds(&second_permissions, &mut bounds);
    let ghost unsorted_bounds = bounds@;
    proof {
        broadcast use vstd::laws_cmp::group_laws_cmp;
    }
    bounds.as_mut_slice().sort_unstable();
    let ghost sorted_bounds = bounds@;
    // VERUS: no `dedup`, so repeated bounds are dropped by index.
    dedup_sorted(&mut bounds);
    proof {
        lemma_walked_bounds(range_map_values(&first_permissions), range_map_values(&second_permissions), first_bounds,
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
    // VERUS: no `for` over a consumed iterator whose first item is taken apart, so the bounds are walked by index.
    if bounds.is_empty() {
        proof {
            lemma_permissions_outside::<Architecture>(memory_first, memory_second, after_values, first_permissions, second_permissions,
                bounds@, 0, 0);
            lemma_permissions_sent(memory_first, memory_second, hook_before, *mutation_hook, changes);
        }
        return;
    }
    let ghost low = bounds@[0] as int;
    let mut end_index: usize = 1;
    while end_index < bounds.len()
        invariant
            1 <= end_index <= bounds@.len(),
            low == bounds@[0] as int,
            strictly_ascending(bounds@),
            permissions_resolved::<Architecture>(memory_first, first_permissions),
            permissions_resolved::<Architecture>(memory_second, second_permissions),
            values_by_bounds(range_map_values(&first_permissions), bounds@),
            values_by_bounds(range_map_values(&second_permissions), bounds@),
            permissions_up_to(memory_first, memory_second, apply_all(after_values, changes), low, bounds@[end_index - 1] as int),
            receives(hook_before, *mutation_hook, changes),
            memory_first == memory_second ==> changes.len() == 0 && *mutation_hook == hook_before,
            memory_first == memory_of(first@),
            memory_second == memory_of(second@),
            after_values == bytes_of_with_permissions_of(memory_second, memory_first),
            obeys_concrete_eq::<Architecture::PagePermission>(),
            forall|permissions: &RangeMap<u64, Architecture::PagePermission>, address: u64|
                #[trigger] read_permission.requires((permissions, address)),
            forall|permissions: &RangeMap<u64, Architecture::PagePermission>, address: u64, permission: Architecture::PagePermission|
                #[trigger] read_permission.ensures((permissions, address), permission)
                    ==> permission == permission_or_default::<Architecture>(range_map_values(permissions)(address)),
        decreases bounds@.len() - end_index,
    {
        proof {
            reveal(obeys_concrete_eq);
        }
        let addresses = bounds[end_index - 1]..bounds[end_index];
        let permission = read_permission(&second_permissions, addresses.start);
        if read_permission(&first_permissions, addresses.start) != permission {
            proof {
                assert(memory_first(addresses.start).1 != memory_second(addresses.start).1);
            }
            let ghost hook_now = *mutation_hook;
            let mutation = MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            };
            let ghost change = mutation@;
            mutation_hook.on_memory_mutated(mutation);
            proof {
                lemma_receives_one(hook_before, hook_now, *mutation_hook, changes, change);
                lemma_apply_all_push(after_values, changes, change);
                lemma_permission_interval::<Architecture>(memory_first, memory_second, apply_all(after_values, changes),
                    apply_all(after_values, changes.push(change)), first_permissions, second_permissions, bounds@,
                    end_index as int, low, true);
                changes = changes.push(change);
            }
        } else {
            proof {
                lemma_permission_interval::<Architecture>(memory_first, memory_second, apply_all(after_values, changes),
                    apply_all(after_values, changes), first_permissions, second_permissions, bounds@, end_index as int, low, false);
            }
        }
        end_index += 1;
    }
    proof {
        lemma_permissions_outside::<Architecture>(memory_first, memory_second, apply_all(after_values, changes), first_permissions,
            second_permissions, bounds@, low, bounds@[bounds@.len() - 1] as int);
        lemma_permissions_sent(memory_first, memory_second, hook_before, *mutation_hook, changes);
    }
}

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

/// One past the last address, where a value assignment goes on from zero.
// VERUS: no shift in a constant it evaluates, so the end is the last address plus one.
const ADDRESSES_END: u128 = u64::MAX as u128 + 1;

/// Records the addresses an unpaired mutation reaches: the bytes it writes, or that it assigns a permission.
// VERUS: no closure that captures a mutable reference, so what is recorded is passed in.
#[verifier::spinoff_prover]
fn add_unpaired<Architecture: ProcessorArchitecture>(
    mutation: &MemoryDelta<'_, Architecture>,
    written: &mut Vec<(u128, u128)>,
    has_unpaired_permission: &mut bool,
)
    ensures
        records_unpaired(mutation@, old(written)@, *old(has_unpaired_permission), final(written)@, *final(has_unpaired_permission)),
{
    match mutation {
        MemoryDelta::ValueAssignment { start, bytes } => {
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
            // VERUS: no `Range::is_empty`, so emptiness is its start not below its end.
            *has_unpaired_permission |= addresses.start < addresses.end;
            proof {
                assert(written@.is_prefix_of(written@)) by {
                    assert(written@.take(written@.len() as int) =~= written@);
                }
            }
        },
    }
}

/// Joins each range that overlaps or touches the one before into it.
#[verifier::spinoff_prover]
fn merge_ranges(written: &mut Vec<(u128, u128)>)
    requires
        well_formed(old(written)@),
        starts_ascending(old(written)@),
    ensures
        well_formed(final(written)@),
        separated(final(written)@),
        forall|address: u64| #[trigger] in_ranges(final(written)@, address) == in_ranges(old(written)@, address),
{
    let ghost sorted = written@;
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
}

/// Drops each bound equal to the one before.
#[verifier::spinoff_prover]
fn dedup_sorted(bounds: &mut Vec<u64>)
    requires
        sorted_by_le(old(bounds)@, |first: u64, second: u64| cmp_le(first, second)),
    ensures
        strictly_ascending(final(bounds)@),
        forall|bound: u64| #[trigger] final(bounds)@.contains(bound) == old(bounds)@.contains(bound),
{
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
}

/// Copies `count` bytes from `from` on into the buffer from `at` on.
#[verifier::spinoff_prover]
#[inline]
fn copy_into(buffer: &mut Vec<u8>, at: usize, bytes: &[u8], from: usize, count: usize)
    requires
        at + count <= old(buffer)@.len(),
        from + count <= bytes@.len(),
        at + count <= usize::MAX,
        from + count <= usize::MAX,
    ensures
        final(buffer)@.len() == old(buffer)@.len(),
        forall|index: int| 0 <= index < old(buffer)@.len() ==> #[trigger] final(buffer)@[index]
            == if at <= index < at + count { bytes@[from + index - at] } else { old(buffer)@[index] },
{
    buffer.as_mut_slice()[at..at + count].copy_from_slice(&bytes[from..from + count]);
}

/// Writes into `rendered` what the mutations leave at its addresses from `start` on, which is zero where none wrote.
#[verifier::spinoff_prover]
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
    rendered.clear();
    rendered.resize(length, u8::MIN);
    proof {
        assert(mutations@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
        assert(bytes_resolved(memory_of(mutations@.take(0)), rendered@, start));
    }
    // VERUS: no `for` over a slice whose position a proof can name, so the mutations are walked by index.
    let mut index: usize = 0;
    while index < mutations.len()
        invariant
            index <= mutations@.len(),
            rendered@.len() == length,
            start as int + length as int <= address_count(),
            bytes_resolved(memory_of(mutations@.take(index as int)), rendered@, start),
        decreases mutations@.len() - index,
    {
        let ghost before = rendered@;
        if let MemoryDelta::ValueAssignment {
            start: mutation_start,
            bytes,
        } = &mutations[index]
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
            lemma_resolved_after_change(mutations@, index as int, before, rendered@, start);
        }
        index += 1;
    }
    proof {
        assert(mutations@.take(mutations@.len() as int) =~= mutations@);
    }
}

/// Returns the permission of every range of addresses once the mutations are applied, keyed by address range.
#[verifier::spinoff_prover]
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
    proof {
        assert(mutations@.take(0) =~= Seq::<MemoryDelta<'_, Architecture>>::empty());
        assert(assignments(mutations@.take(0)) =~= Seq::empty());
    }
    // VERUS: no `for` over a slice whose position a proof can name, so the mutations are walked by index.
    let mut index: usize = 0;
    while index < mutations.len()
        invariant
            index <= mutations@.len(),
            obeys_concrete_eq::<Architecture::PagePermission>(),
            permissions_resolved::<Architecture>(memory_of(mutations@.take(index as int)), rendered),
        decreases mutations@.len() - index,
    {
        let ghost before = rendered;
        // VERUS: no `let` chain in a condition and no `Range::is_empty`, so the empty check is a guard.
        match &mutations[index] {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } if addresses.start < addresses.end => {
                insert_range(&mut rendered, addresses, *permission);
            },
            _ => {},
        }
        proof {
            lemma_memory_of_take(mutations@, index as int);
            let memory = memory_of(mutations@.take(index as int));
            assert forall|address: u64| #[trigger] apply(memory, mutations@[index as int]@)(address).1
                == permission_or_default::<Architecture>(range_map_values(&rendered)(address)) by {
                assert(memory(address).1 == permission_or_default::<Architecture>(range_map_values(&before)(address)));
            }
        }
        index += 1;
    }
    proof {
        assert(mutations@.take(mutations@.len() as int) =~= mutations@);
    }
    rendered
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

} // verus!
