//! What the native tests share: their knobs and the words they draw of each form.

use wcontainers_arm64::encode::{self, field};
use wcontainers_arm64::{Arm64, decode, forms};
use wcontainers_guest_cpu::{Native, Route};

/// Words drawn per form when the knob is not set.
pub const WORDS_PER_FORM: u64 = 64;

pub fn knob(name: &str, default: u64) -> u64 {
    match std::env::var(name) {
        Ok(text) => text
            .parse()
            .unwrap_or_else(|_| panic!("{name} is not a number: {text}")),
        Err(_) => default,
    }
}

/// A form whose bits other than its register fields are this few is swept through every value of them, so
/// that values the decoder picks out one by one (the system registers of a move) are all seen.
pub const SWEPT_BITS: u32 = 16;

/// The words of a form to try, each with drawn register fields: when its non-register bits are few, every value
/// of them, swept again with other register fields until there are at least `draws`; else `draws` drawn. A word
/// routed for float status (few among a system register move's values) also comes with every register in each
/// of its register fields, withheld ones included.
pub fn words_of(form: &forms::Form, draws: u64, rng: &mut fastrand::Rng) -> Vec<u32> {
    let mut words = drawn_words(form, draws, rng);
    let float_status: Vec<u32> = words
        .iter()
        .copied()
        .filter(|&w| Arm64::route(&(), &decode(w), &()) == Route::FloatStatus)
        .collect();
    for word in float_status {
        for other in every_register(form, word) {
            if !words.contains(&other) {
                words.push(other);
            }
        }
    }
    words
}

/// The word with each of its register fields in turn set to every register, the other fields as they are.
fn every_register(form: &forms::Form, word: u32) -> Vec<u32> {
    form.fields
        .iter()
        .zip(encode::field_names(form))
        .filter(|((name, ..), _)| name.starts_with(field::REGISTER))
        .flat_map(|((_, _, width), name)| {
            (0..1u32 << width).filter_map(move |r| encode::replace(word, name, r).ok())
        })
        .collect()
}

fn drawn_words(form: &forms::Form, draws: u64, rng: &mut fastrand::Rng) -> Vec<u32> {
    let free = !form.mask & !form.should_mask;
    let registers = form
        .fields
        .iter()
        .filter(|(name, ..)| name.starts_with(field::REGISTER))
        .fold(0u32, |m, &(_, low, width)| {
            m | (((1u32 << width) - 1) << low)
        });
    let swept = free & !registers;
    if swept.count_ones() > SWEPT_BITS {
        return (0..draws).map(|_| form.word(rng.u32(..))).collect();
    }
    // Every subset of the swept bits, by counting through them.
    let mut words = Vec::new();
    let mut value = 0u32;
    loop {
        words.push(form.word(value | (rng.u32(..) & registers)));
        value = value.wrapping_sub(swept) & swept;
        if value == 0 && words.len() as u64 >= draws {
            break;
        }
    }
    words
}
