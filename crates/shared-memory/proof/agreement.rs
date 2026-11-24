//! Tests that the copy Verus proves (arithmetic.rs) and the crate's own arithmetic (../arithmetic.rs) give the same
//! answers at the edges and on random inputs, so that the proof speaks for the code.
#![cfg(test)]

#[path = "arithmetic.rs"]
mod copy;
#[path = "../arithmetic.rs"]
#[allow(dead_code)]
mod real;

/// The random inputs drawn for each function.
const DRAWS: u32 = 4_000_000;

/// Returns the next number of a SplitMix64 sequence.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Returns a count, offset or length near one of the edges, or anywhere.
fn near(state: &mut u64, edges: &[u64]) -> u64 {
    let pick = next(state);
    match pick % 4 {
        0 => next(state),
        1 => next(state) % 64,
        _ => edges[(pick >> 8) as usize % edges.len()]
            .wrapping_add(next(state) % 17)
            .wrapping_sub(8),
    }
}

/// The largest count of words a mapping can hold.
const MOST: usize = usize::MAX / 8;

fn same_plan(count: usize, at: u64, length: usize) {
    let copy = copy::plan(count, at, length).map(|p| (p.low, p.index, p.head));
    let real = real::plan(count, at, length).map(|p| (p.low, p.index, p.head));
    assert_eq!(copy, real, "plan({count}, {at}, {length})");
}

fn same_part(count: usize, start: u64, end: u64) {
    assert_eq!(
        copy::part(count, start, end),
        real::part(count, start, end),
        "part({count}, {start}, {end})"
    );
}

#[test]
fn agree_at_the_edges() {
    let counts = [0, 1, 2, 3, 4, 5, 7, 8, 9, 511, 512, 513, MOST - 1, MOST];
    for count in counts {
        let bytes = count as u64 * 8;
        let mut offsets: Vec<u64> = (0..=40).collect();
        offsets.extend((0..=40).map(|k| bytes.wrapping_sub(k)));
        offsets.extend((0..=40).map(|k| bytes.saturating_add(k)));
        offsets.extend((0..=40).map(|k| u64::MAX - k));
        for &at in &offsets {
            for &length in &offsets {
                same_plan(count, at, length as usize);
                same_part(count, at, length);
            }
        }
    }
}

#[test]
fn agree_on_random_inputs() {
    let mut state = 0x5eed;
    let edges = [0, 8, 4096, u64::MAX, MOST as u64, MOST as u64 * 8];
    for _ in 0..DRAWS {
        let count = (near(&mut state, &edges) as usize).min(MOST);
        let bytes = count as u64 * 8;
        let edges = [0, 8, bytes, u64::MAX, bytes / 2];
        let (at, length) = (near(&mut state, &edges), near(&mut state, &edges));
        same_plan(count, at, length as usize);
        same_part(count, at, length);
    }
}
