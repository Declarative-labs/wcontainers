// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! The flat structures of the glue's C have the layouts of their Rust mirrors, field by field, and every field Mesa
//! declares is carried or left out by name (built against the pinned Mesa only). The rows printed (`FIELD`, `LEFT`)
//! are compared with clang's own record layouts of Mesa's headers by `api-crate-generator gallium-layouts`, an
//! independent count of each structure's fields.

#[cfg(mesa)]
#[test]
fn layouts_agree() {
    let checked = wcontainers_gallium_direct::layouts_agree();
    println!("LAYOUTS {checked:?}");
    assert!(checked.is_ok(), "a flat structure's size differs between C and Rust: {checked:?}");
}

#[cfg(mesa)]
#[test]
fn fields_agree() {
    let checked = wcontainers_gallium_direct::fields_agree();
    let fields = match checked {
        Ok(fields) => fields,
        Err((layout, measured)) => panic!("a field differs between C, Rust and Mesa: {layout:?} {measured:?}"),
    };
    // The rows start on a line of their own (the test harness's name is printed before, without its end of line).
    println!();
    for (layout, measured) in &fields {
        let mesa_offset = measured.mesa_offset.map_or_else(|| "bits".to_string(), |offset| offset.to_string());
        let mesa_size = measured.mesa_size.map_or_else(|| "bits".to_string(), |size| size.to_string());
        println!("FIELD {} {} mesa_offset {mesa_offset} mesa_size {mesa_size} flat_offset {} flat_size {} bits {}", layout.structure, layout.field, measured.flat_offset, measured.flat_size, layout.bits);
    }
    for (structure, field) in wcontainers_gallium_direct::flat::LEFT_OUT {
        println!("LEFT {structure} {field}");
    }
    println!("FIELDS AGREE {}", fields.len());
    assert_eq!(fields.len(), wcontainers_gallium_direct::flat::FIELDS.len());
}

#[cfg(mesa)]
#[test]
fn values_agree() {
    let checked = wcontainers_gallium_direct::values_agree();
    println!("VALUES {checked:?}");
    assert!(checked.is_ok(), "a declared value differs between the trait and C: {checked:?}");
}

#[cfg(mesa)]
#[test]
fn table_agrees() {
    let (c_size, rust_size) = wcontainers_gallium_direct::table_agrees();
    println!("TABLE C {c_size} Rust {rust_size}");
    assert_eq!(c_size, rust_size, "serve.c's table and serve.rs's differ");
}
