//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! Every method the serving glue of direct3d-12 refuses by its form is refused by the calling glue first, on its own: the
//! conformance suite's whole drive through both glues reaches no refusal entry of the serving glue.

#[test]
fn no_call_reaches_a_refusal_of_the_serving_glue() {
    let answers = wcontainers_direct3d_12_conformance::answers_of(
        wcontainers_direct3d_12_direct::served(wcontainers_direct3d_12_recorder::Recorder),
    );
    assert!(!answers.is_empty());
    assert_eq!(wcontainers_direct3d_12_direct::refusals_reached(), 0);
}
