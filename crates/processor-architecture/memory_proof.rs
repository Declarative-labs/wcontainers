//! The proofs about the memory model that the methods of this crate use; nothing here is part of the specification.

use vstd::prelude::*;
use vstd::std_specs::slice::{cmp_le, sorted_by_le};

use crate::*;

verus! {

/// The number of addresses, one past the last.
pub open spec fn address_count() -> int {
    0x1_0000_0000_0000_0000
}

/// Applying two lists of changes one after the other is applying the joined list.
pub proof fn lemma_apply_all_append<Permission>(memory: Memory<Permission>, first: Seq<Assignment<Permission>>, second: Seq<Assignment<Permission>>)
    ensures
        apply_all(memory, first + second) == apply_all(apply_all(memory, first), second),
    decreases second.len(),
{
    if second.len() == 0 {
        assert(first + second =~= first);
    } else {
        lemma_apply_all_append(memory, first, second.drop_last());
        assert((first + second).drop_last() =~= first + second.drop_last());
    }
}

/// The memory of a list with mutations appended is the memory of the list with their changes applied.
pub proof fn lemma_appended_memory<'memory, Architecture: ProcessorArchitecture>(
    memory: Seq<MemoryDelta<'memory, Architecture>>,
    added: Seq<MemoryDelta<'memory, Architecture>>,
)
    ensures
        memory_of(memory + added) == apply_all(memory_of(memory), assignments(added)),
{
    assert(assignments(memory + added) =~= assignments(memory) + assignments(added));
    lemma_apply_all_append(zeroed::<Architecture>(), assignments(memory), assignments(added));
}

/// No change applied leaves the memory as it was.
pub proof fn lemma_no_assignments<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
    memory: Memory<Architecture::PagePermission>,
)
    requires
        mutations.len() == 0,
    ensures
        apply_all(memory, assignments(mutations)) == memory,
{
    assert(assignments(mutations) =~= Seq::empty());
}

/// A list sink that starts empty and is sent changes that turn one memory into another stays empty exactly when the
/// two memories are the same, given that equal memories send it nothing.
pub proof fn lemma_diff_into_empty<'memory, Architecture: ProcessorArchitecture>(
    first: Memory<Architecture::PagePermission>,
    second: Memory<Architecture::PagePermission>,
    before: Vec<MemoryDelta<'memory, Architecture>>,
    after: Vec<MemoryDelta<'memory, Architecture>>,
)
    requires
        before@.len() == 0,
        exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(first, changes)]
            receives(before, after, changes) && apply_all(first, changes) == second,
        first == second ==> after == before,
    ensures
        after@.len() == 0 <==> first == second,
{
    if after@.len() == 0 && first != second {
        let changes = choose|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(first, changes)]
            receives(before, after, changes) && apply_all(first, changes) == second;
        assert(after.applied_to(first) == apply_all(before.applied_to(first), changes));
        lemma_no_assignments(after@, first);
        lemma_no_assignments(before@, first);
    }
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
        Sink::obeys_sink_spec() ==> forall|memory: Memory<Architecture::PagePermission>|
            #[trigger] after.applied_to(memory) == apply(middle.applied_to(memory), change),
    ensures
        receives(before, after, changes.push(change)),
{
    if Sink::obeys_sink_spec() {
        assert forall|memory: Memory<Architecture::PagePermission>|
            #[trigger] after.applied_to(memory) == apply_all(before.applied_to(memory), changes.push(change)) by {
            assert(changes.push(change).drop_last() =~= changes);
            assert(middle.applied_to(memory) == apply_all(before.applied_to(memory), changes));
        }
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
    if Sink::obeys_sink_spec() {
        assert forall|memory: Memory<Architecture::PagePermission>|
            #[trigger] after.applied_to(memory) == apply_all(before.applied_to(memory), first + second) by {
            assert(after.applied_to(memory) == apply_all(middle.applied_to(memory), second));
            assert(middle.applied_to(memory) == apply_all(before.applied_to(memory), first));
            lemma_apply_all_append(before.applied_to(memory), first, second);
        }
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

/// A map whose bounds are among strictly rising ones gives one value from each of them up to the next, and none
/// before the first or from the last on.
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
        assert forall|at: int| 0 <= at < bounds.len() implies !(walked[index] < #[trigger] bounds[at] <= address) by {
            assert(walked.contains(bounds[at]));
            let place = choose|place: int| 0 <= place < walked.len() && walked[place] == bounds[at];
            if walked[index] < bounds[at] <= address {
                if place <= index {
                    if place < index {
                        assert(walked[place] < walked[index]);
                    }
                } else if place >= index + 1 {
                    if place > index + 1 {
                        assert(walked[index + 1] < walked[place]);
                    }
                }
            }
        }
        assert(values(walked[index]) == values(address));
    }
    assert forall|address: u64| (walked.len() == 0 || address < walked[0] || walked[walked.len() - 1] <= address)
        implies #[trigger] values(address) is None by {
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

} // verus!
