//! Each statement `trusted.rs` makes about code Verus cannot read, checked on that code: every `ensures` clause is
//! transcribed into a check over addresses near both ends of the address space and the bounds themselves, after each
//! of many random calls. The clauses quantify over every address, which the generated checkers cannot express.

extern crate std;

use alloc::vec::Vec;

use rangemap::RangeMap;

use crate::trusted::{append_bounds, bytes_to_vec, insert_range, is_same_slice, new_range_map, range_map_get};

/// A permission-like value whose `==` is equality, as `insert_range` requires.
type Value = u8;

/// The addresses each check reads: both ends of the address space, the middle, and each bound inserted.
fn sampled_addresses(extra: &[u64]) -> Vec<u64> {
    let mut addresses: Vec<u64> = (0..80).chain(u64::MAX - 80..=u64::MAX).chain([1 << 47, (1 << 48) - 1, 1 << 48]).collect();
    addresses.extend_from_slice(extra);
    for &address in extra {
        addresses.push(address.wrapping_sub(1));
        addresses.push(address.wrapping_add(1));
    }
    addresses.sort_unstable();
    addresses.dedup();
    addresses
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

    /// An address near one of the ends of the address space.
    fn address(&mut self) -> u64 {
        let near = self.next() % 70;
        if self.next().is_multiple_of(2) { near } else { u64::MAX - near }
    }
}

/// The ensures of `append_bounds`, read as two checks over the sampled addresses.
fn check_bounds_separate(map: &RangeMap<u64, Value>, bounds: &[u64], addresses: &[u64]) {
    let value = |address: u64| range_map_get(map, address);
    for (index, &first) in addresses.iter().enumerate() {
        for &second in &addresses[index..] {
            let no_bound_between = bounds.iter().all(|&bound| !(first < bound && bound <= second));
            if no_bound_between {
                assert_eq!(value(first), value(second), "no bound in ({first}, {second}] but the values differ");
            }
        }
        if value(first).is_some() {
            assert!(
                bounds.iter().any(|&below| below <= first) && bounds.iter().any(|&above| first < above),
                "{first} has a value but no bound at or below it and above it"
            );
        }
    }
}

#[test]
fn trusted_range_map_statements_hold_on_the_real_crate() {
    for seed in 1..=300u64 {
        let mut random = Random(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut map: RangeMap<u64, Value> = new_range_map();
        // `new_range_map`: no address has a value.
        let mut inserted = Vec::new();
        for &address in &sampled_addresses(&[]) {
            assert_eq!(range_map_get(&map, address), None, "seed {seed}: a new map gives {address} a value");
        }
        for step in 0..12 {
            let (first, second) = (random.address(), random.address());
            let addresses = first.min(second)..first.max(second);
            if addresses.is_empty() {
                continue;
            }
            let value = (random.next() % 3) as Value;
            let before_map = map.clone();
            insert_range(&mut map, addresses.clone(), value);
            inserted.push(addresses.start);
            inserted.push(addresses.end);
            // `insert_range`: the value inside the addresses, the value before everywhere else.
            for &address in &sampled_addresses(&inserted) {
                let expected = if addresses.contains(&address) { Some(value) } else { range_map_get(&before_map, address) };
                assert_eq!(range_map_get(&map, address), expected, "seed {seed} step {step}: insert {addresses:?} = {value} at {address}");
                // `range_map_get`: what the crate's own `get` gives.
                assert_eq!(range_map_get(&map, address), map.get(&address).copied());
            }
            // `append_bounds`: the old list is kept as a prefix, and the appended bounds separate the values.
            let mut bounds = alloc::vec![random.next(), random.next()];
            let kept = bounds.clone();
            append_bounds(&map, &mut bounds);
            assert_eq!(&bounds[..kept.len()], &kept[..], "seed {seed} step {step}: the bounds before were not kept");
            check_bounds_separate(&map, &bounds[kept.len()..], &sampled_addresses(&bounds));
        }
    }
}

#[test]
fn the_same_slice_holds_the_same_bytes() {
    let buffer: Vec<u8> = (0..=255).collect();
    let copy = buffer.clone();
    for start in 0..buffer.len() {
        for end in start..buffer.len().min(start + 9) {
            let (first, second) = (&buffer[start..end], &buffer[start..end]);
            if is_same_slice(first, second) {
                assert_eq!(first, second);
            }
            assert!(is_same_slice(first, second), "a slice is the same slice as itself");
            // A copy is another slice: `is_same_slice` says nothing then, and must not claim it is the same slice.
            if !first.is_empty() {
                assert!(!is_same_slice(first, &copy[start..end]));
            }
        }
    }
}

#[test]
fn a_copy_of_bytes_holds_them() {
    let mut random = Random(0x5eed);
    for length in (0..64).chain([4095, 4096, 8192]) {
        let bytes: Vec<u8> = (0..length).map(|_| random.next() as u8).collect();
        // `bytes_to_vec`: the list holds the bytes, in order.
        assert_eq!(bytes_to_vec(&bytes)[..], bytes[..]);
    }
}
