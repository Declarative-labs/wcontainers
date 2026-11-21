//! Which VEX forms have a meaning.

use wcontainers_x86::{decode, written};

#[test]
fn lane_forms_are_written() {
    // vpaddd xmm1, xmm2, xmm3; vpaddd ymm1, ymm2, ymm3; vmovups ymm0, [rax]; vzeroupper.
    for bytes in [
        &[0xc5, 0xe9, 0xfe, 0xcb][..],
        &[0xc5, 0xed, 0xfe, 0xcb],
        &[0xc5, 0xfc, 0x10, 0x00],
        &[0xc5, 0xf8, 0x77],
    ] {
        let (i, _) = decode(bytes).expect("an instruction");
        assert!(written(&i), "{:?} is written", i.iced.code());
    }
}
