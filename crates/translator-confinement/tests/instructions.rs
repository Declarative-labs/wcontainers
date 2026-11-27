//! The translator's `TranslatedInstructions` for form I: the bytes are the native output's, padding is `udf`, the way
//! is the one asked for, and every piece of words drawn from every form is confined under the entry rule once the
//! 64 KiB below its window are reserved.

use wcontainers_arm64::way::{Kept, SLOT_BITS};
use wcontainers_arm64::{Arm64, Way, forms};
use wcontainers_translator::{
    LookAhead, NativeRequest, translate_instructions, translate_native_request,
};
use wcontainers_translator_confinement::{Place, State, check};

const SLOT: u16 = 3;
const PAGE: usize = 4096;

fn request() -> NativeRequest {
    NativeRequest {
        look_ahead: LookAhead::None,
        exact_replay: true,
        stop: false,
        step: false,
    }
}

fn way(displaced: bool) -> Way {
    Way::InProcess {
        slot: SLOT,
        displaced,
        kept: Kept::USUAL,
    }
}

fn words(per_form: usize) -> Vec<u32> {
    let mut rng = fastrand::Rng::with_seed(0x1257_ac7e);
    forms::all()
        .iter()
        .flat_map(|form| {
            (0..per_form)
                .map(|_| form.word(rng.u32(..)))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn the_bytes_are_the_native_output_s_and_the_way_the_one_asked_for() {
    let guest = 0x40_1000;
    for word in words(2) {
        for displaced in [false, true] {
            let made = translate_instructions::<Arm64>(
                &word.to_le_bytes(),
                guest,
                &request(),
                &way(displaced),
                &(),
            );
            let plain = translate_native_request::<Arm64>(
                &word.to_le_bytes(),
                guest,
                &request(),
                &way(displaced),
                &(),
            );
            assert_eq!(made.is_some(), plain.is_some(), "{word:#x}");
            let (Some(made), Some(plain)) = (made, plain) else {
                continue;
            };
            assert_eq!(made.bytes(), plain.bytes.as_slice(), "{word:#x}");
            assert_eq!(made.code().spans, plain.spans, "{word:#x}");
            assert_eq!(made.way(), way(displaced));
        }
    }
}

#[test]
fn padding_is_zero_words_to_a_whole_page() {
    let made = translate_instructions::<Arm64>(
        &0x8b02_0020u32.to_le_bytes(),
        0x40_1000,
        &request(),
        &way(false),
        &(),
    )
    .expect("add x0, x1, x2 translates");
    let padded = made.padded(PAGE);
    assert_eq!(padded.len(), PAGE);
    assert_eq!(&padded[..made.bytes().len()], made.bytes());
    assert!(padded[made.bytes().len()..].iter().all(|&b| b == 0));
    // A length already whole stays as it is.
    let whole = made.padded(4);
    assert_eq!(whole, made.bytes());
}

#[test]
fn every_piece_of_every_form_is_confined_under_the_entry_rule() {
    let guest = 0x40_1000;
    let mut checked = 0;
    for word in words(4) {
        for displaced in [false, true] {
            let Some(made) = translate_instructions::<Arm64>(
                &word.to_le_bytes(),
                guest,
                &request(),
                &way(displaced),
                &(),
            ) else {
                continue;
            };
            let place = Place {
                base: u64::from(SLOT) << SLOT_BITS,
                frame: 0x7fff_0000_1000,
                code: (1 << 45) + (1 << 34),
                below: 1 << 16,
                displaced,
            };
            let (found, _) = check(made.bytes(), &place, &[(0, State::entry(&place))]);
            assert_eq!(found, [], "{word:#010x} displaced {displaced}");
            checked += 1;
        }
    }
    assert!(checked > 10_000, "{checked} pieces");
}
