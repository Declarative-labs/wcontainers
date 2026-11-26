//! The statement `trusted.rs` makes about a `map` and itertools' cartesian product, checked on itertools: every shape of up to
//! four vectors of up to four elements, the empty ones included.

extern crate std;

use alloc::vec;
use alloc::vec::Vec;

use vstd::prelude::Ghost;

use crate::trusted::map_multi_cartesian_product;

/// The most vectors, and the most elements of each, the check combines.
const LARGEST: usize = 4;

/// `product` of `trusted.rs`, as it reads.
fn product(sequences: &[Vec<usize>]) -> Vec<Vec<usize>> {
    match sequences.split_first() {
        None => vec![Vec::new()],
        Some((first, rest)) => {
            let rest = product(rest);
            first
                .iter()
                .flat_map(|element| {
                    rest.iter().map(move |tail| {
                        let mut combination = vec![*element];
                        combination.extend_from_slice(tail);
                        combination
                    })
                })
                .collect()
        }
    }
}

#[test]
fn the_product_is_every_combination_in_order() {
    let mut shape = Vec::new();
    loop {
        // Distinct elements, so that the order of the combinations is checked too.
        let mut next = 0;
        let sequences: Vec<Vec<usize>> = shape
            .iter()
            .map(|&length| {
                next += length;
                (next - length..next).collect()
            })
            .collect();
        let combinations: Vec<Vec<usize>> =
            map_multi_cartesian_product(sequences.iter(), Clone::clone, Ghost::assume_new())
                .collect();
        assert_eq!(combinations, product(&sequences), "shape {shape:?}");
        // The next shape: count in base LARGEST + 1, with one more vector after the last of each length.
        match shape.iter().position(|&length| length < LARGEST) {
            Some(position) => {
                shape[position] += 1;
                shape[..position].fill(0);
            }
            None if shape.len() < LARGEST => {
                shape.fill(0);
                shape.push(0);
            }
            None => break,
        }
    }
}
