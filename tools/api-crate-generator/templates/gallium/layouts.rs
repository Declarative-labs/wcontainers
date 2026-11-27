//! The flat structures of the glue's C have the sizes of their Rust mirrors (built against the pinned Mesa only).

#[cfg(mesa)]
#[test]
fn layouts_agree() {
    let checked = wcontainers_gallium_direct::layouts_agree();
    println!("LAYOUTS {checked:?}");
    assert!(checked.is_ok(), "a flat structure's size differs between C and Rust: {checked:?}");
}
