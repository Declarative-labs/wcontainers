//! The checks a translation leaves out keep their meaning through the bits a caller outside Rust passes.

use wcontainers_translator::Omit;

#[test]
fn every_set_of_left_out_checks_survives_its_bits() {
    for bits in 0..16 {
        assert_eq!(Omit::from_bits(bits).bits(), bits);
    }
    let one = |omit: Omit| omit.bits();
    let all = Omit::default();
    assert_eq!(one(Omit { stop: true, ..all }), 1);
    assert_eq!(
        one(Omit {
            permissions: true,
            ..all
        }),
        2
    );
    assert_eq!(
        one(Omit {
            returns: true,
            ..all
        }),
        4
    );
    assert_eq!(one(Omit { depth: true, ..all }), 8);
}
