//! The monitor's answer to the inside's translation requests: the native output's piece of the guest code the
//! link carries, and the words the inside fills to link pieces.

use wcontainers_arm64::Arm64;
use wcontainers_arm64::carry::link_words;
use wcontainers_inside::link::Link;
use wcontainers_inside::translation::{
    self, Field, LinkWords, Piece, Request, TooLarge, Translate,
};
use wcontainers_translator::{LookAhead, translate_native};

/// Answers the translation request in `link`, translating at `look_ahead`.
pub fn answer(link: &mut Link, look_ahead: LookAhead) {
    match Request::from_word(link.head.request).expect("a translation request") {
        Request::Translate => translate(link, look_ahead),
        Request::Linking => {
            let words = link_words();
            let field = |(clear, mask)| Field { clear, mask };
            link.put(&LinkWords {
                moves: words.moves.map(field),
                jump: words.jump,
                branch: field(words.branch),
                empty: words.empty,
            });
        }
    }
}

/// Answers a translate request with the piece at `look_ahead`, cut shorter where its host code would not fit the
/// link.
fn translate(link: &mut Link, look_ahead: LookAhead) {
    let request: Translate = link.get();
    let code = link.data().to_vec();
    let mut look_ahead = look_ahead;
    loop {
        let piece =
            translate_native::<Arm64>(&code, request.address, look_ahead, request.exact != 0).map(
                |made| Piece {
                    code: made.bytes,
                    spans: made.spans,
                    exits: made.exits,
                    sites: made.sites,
                },
            );
        let instructions = piece.as_ref().map_or(0, |p| {
            let mut guests: Vec<u64> = p.spans.iter().map(|span| span.guest).collect();
            guests.sort_unstable();
            guests.dedup();
            guests.len()
        });
        match translation::answer(link, piece.as_ref()) {
            Ok(()) => return,
            Err(TooLarge) if instructions > 1 => {
                let half = u32::try_from(instructions / 2).expect("a piece's instructions fit u32");
                look_ahead = LookAhead::Bounded(half);
            }
            Err(TooLarge) => panic!("one instruction's piece does not fit the link"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_arm64::{Arm64, encode};
    use wcontainers_guest_cpu::Guest;
    use wcontainers_inside::link::Link;
    use wcontainers_inside::translation::{self, Translate};
    use wcontainers_translator::{LookAhead, translate_native};

    /// A link page on the heap, as the inside writes one.
    fn link() -> Box<Link> {
        // SAFETY: a link is plain words and bytes, for which all zeros is valid.
        unsafe { Box::new_zeroed().assume_init() }
    }

    /// Every word of a few routes comes back from the link as the emitter made it.
    #[test]
    fn a_piece_comes_back_whole() {
        let at = 0x40_1000;
        let words = [
            0xd400_0001u32, // svc #0
            0xf940_0020,    // ldr x0, [x1]
            0xa9bf_7bfd,    // stp x29, x30, [sp, #-16]!
            0x9400_0000,    // bl .
            0xd61f_0360,    // br x27
            0xd53b_4400,    // mrs x0, fpcr
        ];
        for word in words {
            for exact in [false, true] {
                let mut link = link();
                link.ask(
                    &Translate {
                        address: at,
                        exact: u64::from(exact),
                    },
                    &word.to_le_bytes(),
                );
                super::answer(&mut link, LookAhead::None);
                let got = translation::piece(&link).expect("a piece");
                let made =
                    translate_native::<Arm64>(&word.to_le_bytes(), at, LookAhead::None, exact)
                        .expect("a piece");
                assert_eq!(got.code, made.bytes, "{word:#010x}");
                assert_eq!(got.spans, made.spans, "{word:#010x}");
                assert_eq!(got.sites, made.sites, "{word:#010x}");
                assert_eq!(got.exits, made.exits, "{word:#010x}");
            }
        }
        let mut link = link();
        link.ask(
            &Translate {
                address: at,
                exact: 0,
            },
            &[0; Arm64::MAX_LENGTH as usize - 1],
        );
        super::answer(&mut link, LookAhead::None);
        assert_eq!(
            translation::piece(&link),
            None,
            "no instruction in three bytes"
        );
    }

    /// The guest addresses a piece holds code for, and the targets of its exits.
    fn shape(words: &[u32], at: u64) -> (BTreeSet<u64>, BTreeSet<u64>) {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let made = translate_native::<Arm64>(&bytes, at, LookAhead::Bounded(u32::MAX), true)
            .expect("a piece");
        (
            made.spans.iter().map(|span| span.guest).collect(),
            made.exits.iter().map(|exit| exit.target).collect(),
        )
    }

    /// At bounded look-ahead a loop is held whole, its branch back going straight to its head, and the code after it
    /// is reached through an exit; a call's next instruction is left to the return.
    #[test]
    fn pieces_hold_loops_and_leave_calls() {
        let at = 0x40_1000;
        let step = Arm64::MAX_LENGTH;
        let looping = [
            0xd100_0400u32, // sub x0, x0, #1
            0xb5ff_ffe0,    // cbnz x0, . - 4
            0xd65f_03c0,    // ret
        ];
        let (held, exits) = shape(&looping, at);
        assert_eq!(held, BTreeSet::from([at, at + step]));
        assert_eq!(exits, BTreeSet::from([at + 2 * step]));
        let page = i64::try_from(PAGE_SIZE).expect("a page fits i64");
        let call = encode::branch(true, page).expect("a call to the next page");
        let (held, exits) = shape(&[call, looping[2]], at);
        assert_eq!(held, BTreeSet::from([at]));
        assert_eq!(exits, BTreeSet::from([at + PAGE_SIZE]));
    }

    /// An answer whose position map does not decode fails loudly, never as the guest's illegal instruction.
    #[test]
    #[should_panic(expected = "the monitor's translate answer is broken: position map")]
    fn a_broken_answer_panics() {
        let mut link = link();
        link.ask(
            &Translate {
                address: 0x40_1000,
                exact: 0,
            },
            &0xd400_0001u32.to_le_bytes(),
        );
        super::answer(&mut link, LookAhead::None);
        let answer: translation::Translated = link.get();
        let code = usize::try_from(answer.code).expect("fits");
        link.bytes[code..].fill(u8::MAX);
        translation::piece(&link);
    }
}
