//! What the input domain trusts of itertools, which Verus cannot read: its cartesian product, stated here and checked
//! against itertools by `trusted_tests.rs`.

use alloc::vec::{self, Vec};

use itertools::{Itertools, MultiProduct};
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

verus! {

#[cfg(verus_keep_ghost)]
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_type_specification]
#[verifier::external_body]
#[verifier::reject_recursive_types(I)]
pub struct ExMultiProduct<I: Iterator + Clone>(MultiProduct<I>) where I::Item: Clone;

/// Every sequence with one element of each of the sequences, in order, the last one varying fastest.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
pub open spec fn product<T>(sequences: Seq<Seq<T>>) -> Seq<Seq<T>>
    decreases sequences.len(),
{
    if sequences.len() == 0 {
        seq![Seq::empty()]
    } else {
        let rest = product(sequences.drop_first());
        sequences[0].map_values(|first: T| rest.map_values(|tail: Seq<T>| seq![first] + tail)).flatten()
    }
}

/// A `map` and itertools' `multi_cartesian_product`: every value of the iterator is mapped before it returns, and then
/// each combination of the vectors is given once, in the order of `product`; `values_of` says what the map gives.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
#[inline]
pub fn map_multi_cartesian_product<I, F, T>(
    iterator: I,
    to_values: F,
    Ghost(values_of): Ghost<spec_fn(I::Item) -> Seq<T>>,
) -> (combinations: MultiProduct<vec::IntoIter<T>>)
    where
        I: Iterator,
        F: FnMut(I::Item) -> Vec<T>,
        T: Clone,
    requires
        forall|item: I::Item| #[trigger] to_values.requires((item,)),
        forall|item: I::Item, values: Vec<T>| #[trigger] to_values.ensures((item,), values) ==> values@ == values_of(item),
    ensures
        iterator.obeys_prophetic_iter_laws() ==> {
            &&& combinations.obeys_prophetic_iter_laws()
            &&& combinations.remaining().map_values(|combination: Vec<T>| combination@)
                == product(iterator.remaining().map_values(values_of))
        },
{
    iterator.map(to_values).multi_cartesian_product()
}

} // verus!
