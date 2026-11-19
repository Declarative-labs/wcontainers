//! The monitor's answer to the inside's translate requests: the native output's piece of the guest code the
//! link carries.

use wcontainers_arm64::Arm64;
use wcontainers_inside::link::Link;
use wcontainers_inside::translation::{self, Piece, TooLarge, Translate};
use wcontainers_translator::{LookAhead, translate_native};

/// Answers the translate request in `link` at look-ahead none.
pub fn translate(link: &mut Link) -> Result<(), TooLarge> {
    let request: Translate = link.get();
    let piece = translate_native::<Arm64>(
        link.data(),
        request.address,
        LookAhead::None,
        request.exact != 0,
    )
    .map(|code| Piece {
        code: code.bytes,
        spans: code.spans,
        sites: code.sites,
    });
    translation::answer(link, piece.as_ref())
}

#[cfg(test)]
mod tests {
    use wcontainers_arm64::Arm64;
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
                super::translate(&mut link).expect("a piece fits the link");
                let got = translation::piece(&link).expect("a piece");
                let made =
                    translate_native::<Arm64>(&word.to_le_bytes(), at, LookAhead::None, exact)
                        .expect("a piece");
                assert_eq!(got.code, made.bytes, "{word:#010x}");
                assert_eq!(got.spans, made.spans, "{word:#010x}");
                assert_eq!(got.sites, made.sites, "{word:#010x}");
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
        super::translate(&mut link).expect("an empty answer fits");
        assert_eq!(
            translation::piece(&link),
            None,
            "no instruction in three bytes"
        );
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
        super::translate(&mut link).expect("a piece fits the link");
        let answer: translation::Translated = link.get();
        let code = usize::try_from(answer.code).expect("fits");
        link.bytes[code..].fill(u8::MAX);
        translation::piece(&link);
    }
}
