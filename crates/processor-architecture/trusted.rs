//! What the memory diff trusts of code Verus cannot read, none of it callable from a proof: each function states what that code does, and
//! `trusted_tests.rs` checks each statement on the real code.

use alloc::vec::Vec;
use core::ops::Range;
use core::ptr;

use rangemap::RangeMap;
#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::cmp::PartialOrdSpec;
use vstd::prelude::*;

verus! {

#[cfg(verus_keep_ghost)]
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_type_specification]
#[verifier::external_body]
#[verifier::reject_recursive_types(K)]
#[verifier::reject_recursive_types(V)]
pub struct ExRangeMap<K, V>(RangeMap<K, V>);

/// The value a range map gives each address, if any.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
pub uninterp spec fn range_map_values<V>(map: &RangeMap<u64, V>) -> spec_fn(u64) -> Option<V>;

/// Between two addresses with no bound after the first up to the second the values are the same, and an address
/// with a value lies at or after one bound and before another.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
pub open spec fn bounds_separate<V>(values: spec_fn(u64) -> Option<V>, bounds: Seq<u64>) -> bool {
    &&& forall|first: u64, second: u64|
        #![trigger values(first), values(second)]
        first <= second && (forall|index: int| 0 <= index < bounds.len() ==> !(first < #[trigger] bounds[index] <= second))
            ==> values(first) == values(second)
    &&& forall|address: u64| #[trigger] values(address) is Some ==> exists|below: int, above: int|
        #![trigger bounds[below], bounds[above]]
        0 <= below < bounds.len() && 0 <= above < bounds.len() && bounds[below] <= address < bounds[above]
}

/// A map that gives no address a value.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
#[inline]
pub fn new_range_map<V: Eq + Clone>() -> (map: RangeMap<u64, V>)
    ensures
        forall|address: u64| #[trigger] range_map_values(&map)(address) is None,
{
    RangeMap::new()
}

/// Gives the value to each of the addresses, which are not empty; the value's `==` must be equality, since the map
/// joins neighbors that are `==`.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
#[inline]
pub fn insert_range<V: Eq + Clone>(map: &mut RangeMap<u64, V>, addresses: &Range<u64>, value: V)
    requires
        addresses.start < addresses.end,
        obeys_concrete_eq::<V>(),
    ensures
        forall|address: u64| #[trigger] range_map_values(&*final(map))(address)
            == if addresses.start <= address < addresses.end { Some(value) } else { range_map_values(&*old(map))(address) },
{
    map.insert(addresses.clone(), value)
}

/// The value the map gives the address.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
#[inline]
pub fn range_map_get<V: Eq + Copy>(map: &RangeMap<u64, V>, address: u64) -> (value: Option<V>)
    ensures
        value == range_map_values(map)(address),
{
    map.get(&address).copied()
}

/// The number of ranges the map holds, which says nothing a proof uses.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
#[inline]
pub fn range_map_len<V: Eq + Clone>(map: &RangeMap<u64, V>) -> usize {
    map.len()
}

/// Appends the start and the end of each range of the map, in order.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
#[inline]
pub fn append_bounds<V: Eq + Clone>(map: &RangeMap<u64, V>, bounds: &mut Vec<u64>)
    ensures
        old(bounds)@.is_prefix_of(final(bounds)@),
        bounds_separate(range_map_values(map), final(bounds)@.skip(old(bounds)@.len() as int)),
{
    for (addresses, _) in map.iter() {
        bounds.push(addresses.start);
        bounds.push(addresses.end);
    }
}

/// `Range::is_empty` as std defines it: a range is empty when its start is not below its end.
#[cfg(verus_keep_ghost)]
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_fn_specification]
pub fn ex_range_is_empty<Idx: PartialOrd<Idx>>(range: &Range<Idx>) -> (empty: bool)
    where
        Idx: PartialOrd<Idx>,
    ensures
        Idx::obeys_partial_cmp_spec() ==> empty == !(range.start.partial_cmp_spec(&range.end) == Some(core::cmp::Ordering::Less)),
{
    range.is_empty()
}

/// The owner's sort of the written ranges by their starts: the same ranges, each start at most the next one's.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
#[inline]
pub fn sort_by_start(written: &mut [Range<u128>])
    ensures
        final(written)@.len() == old(written)@.len(),
        final(written)@.to_multiset() == old(written)@.to_multiset(),
        forall|first: int, second: int| 0 <= first < second < final(written)@.len()
            ==> #[trigger] final(written)@[first].start <= #[trigger] final(written)@[second].start,
{
    written.sort_unstable_by_key(|addresses| addresses.start);
}

/// Whether the two slices are the same slice, which then holds the same bytes.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
#[inline]
pub fn is_same_slice(first: &[u8], second: &[u8]) -> (same: bool)
    ensures
        same ==> first@ == second@,
{
    ptr::eq(first, second)
}

} // verus!
