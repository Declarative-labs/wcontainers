// Written by tools/api-crate-generator from Mesa 26.3.1's Gallium headers and the plan in src/gallium.rs. Do not edit.
//! The flat structures of the glue's C have the layouts of their Rust mirrors, field by field, and every field Mesa
//! declares is carried or left out by name, at the offset clang's own record layouts of Mesa's headers give (built
//! against the pinned Mesa only; clang must be on the path, or named by WCONTAINERS_CLANG: the test fails without it).

mod clang_layouts;

#[cfg(mesa)]
#[test]
fn layouts_agree() {
    let checked = wcontainers_gallium_direct::layouts_agree();
    println!("LAYOUTS {checked:?}");
    assert_eq!(checked, Ok(wcontainers_gallium_direct::flat::SIZES.len()), "every flat structure the same size in C and Rust");
}

#[cfg(mesa)]
#[test]
fn fields_agree() {
    use std::collections::BTreeMap;
    // Each field as C measures it: its offset and size in the flat form the Rust mirror's; a bit-field (with no offset
    // in C) one whose width fits the flat field; a field of the same size in Mesa as in the flat form (a number, a flag,
    // an object, client memory or a color) of that size, an enumeration (which Mesa may pack) at most that size.
    let measured = wcontainers_gallium_direct::fields_measured();
    assert_eq!(measured.len(), wcontainers_gallium_direct::flat::FIELDS.len(), "every field measured");
    let mut fields = Vec::new();
    for (layout, field) in &measured {
        let field = field.unwrap_or_else(|| panic!("{}.{}: a field C does not list", layout.structure, layout.field));
        assert_eq!((field.flat_offset, field.flat_size), (layout.flat_offset, layout.flat_size), "{}.{}: its offset and size in C and in Rust", layout.structure, layout.field);
        let bit_field = layout.bits != 0;
        assert_eq!(field.mesa_offset.is_none(), bit_field, "{}.{}: an offset in Mesa only where it is no bit-field", layout.structure, layout.field);
        assert_eq!(field.mesa_size.is_none(), bit_field, "{}.{}: a size in Mesa only where it is no bit-field", layout.structure, layout.field);
        if layout.same_size {
            let size = field.mesa_size.unwrap_or(usize::MAX);
            assert!(if layout.at_least { size <= field.flat_size } else { size == field.flat_size }, "{}.{}: its size in Mesa {size}, in the flat form {}", layout.structure, layout.field, field.flat_size);
        }
        if bit_field {
            assert!(layout.bits <= u64::try_from(field.flat_size).unwrap_or(0) * u64::from(u8::BITS), "{}.{}: a width of {} bits in {} bytes", layout.structure, layout.field, layout.bits, field.flat_size);
        }
        fields.push((layout, field));
    }
    // The generator's fields by Mesa's structure, with Mesa's offsets by offsetof, and the fields it leaves out.
    let mut carried: BTreeMap<String, Vec<clang_layouts::Carried>> = BTreeMap::new();
    for (layout, measured) in &fields {
        let mesa_offset = measured.mesa_offset.map(|offset| u64::try_from(offset).unwrap_or(u64::MAX));
        carried.entry(layout.structure.to_string()).or_default().push(clang_layouts::Carried { field: layout.field.to_string(), mesa_offset, bits: layout.bits });
    }
    let mut left: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (structure, field) in wcontainers_gallium_direct::flat::LEFT_OUT {
        left.entry((*structure).to_string()).or_default().push((*field).to_string());
    }
    let dump = clang_record_layouts(carried.keys().map(|name| name.split('.').next().unwrap_or(name)));
    let report = clang_layouts::compare(&dump, &carried, &left);
    match report {
        Ok(report) => println!("{report}"),
        Err(report) => panic!("Mesa's fields as clang lays them out differ from the generator's:\n{report}"),
    }
}

/// clang's record layouts of every named structure (each laid out by a static assertion of its size), compiled with
/// the flags build.rs compiles the glue's C with. Fails where clang cannot be run.
#[cfg(mesa)]
fn clang_record_layouts<'a>(structures: impl Iterator<Item = &'a str>) -> String {
    let mut names: Vec<&str> = structures.collect();
    names.sort_unstable();
    names.dedup();
    let mut probe = String::from("#include \"pipe/p_context.h\"\n#include \"pipe/p_defines.h\"\n#include \"pipe/p_screen.h\"\n#include \"pipe/p_state.h\"\n#include \"util/u_transfer_helper.h\"\n");
    for name in &names {
        // Every structure the flat forms carry is a C structure (the unions are nested ones, laid out with it).
        probe.push_str(&format!("_Static_assert(sizeof(struct {name}) > 0, \"{name} laid out\");\n"));
    }
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("gallium_layouts_probe.c");
    std::fs::write(&path, probe).expect("the probe file is written");
    let clang = std::env::var_os("WCONTAINERS_CLANG").unwrap_or_else(|| "clang".into());
    let output = std::process::Command::new(&clang)
        .args(["-fsyntax-only", "-Wno-everything", "-Xclang", "-fdump-record-layouts"])
        .args(env!("WC_GALLIUM_CLANG_ARGS").split('\u{1f}').filter(|arg| !arg.is_empty()))
        .arg(&path)
        .output()
        .unwrap_or_else(|error| panic!("clang ({clang:?}) runs: {error}; the independent count needs it"));
    assert!(output.status.success(), "clang lays out Mesa's structures: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The comparison on a hand-made dump: every field agrees; the first name of `float min_lod, max_lod;` dropped, as the
/// header reader dropped it, fails; so do a moved offset and a field clang does not lay out.
#[test]
fn a_dropped_field_fails() {
    use std::collections::BTreeMap;
    const DUMP: &str = "
*** Dumping AST Record Layout
         0 | struct pipe_sampler_state
     0:0-2 |   unsigned int wrap_s
         4 |   float min_lod
         8 |   float max_lod
        12 |   union pipe_sampler_state::(unnamed at p_state.h:1:1) u
        12 |     unsigned int a
        12 |     float b
        16 |   union pipe_sampler_state::(anonymous at p_state.h:2:2)
        16 |     unsigned int c
        20 |   struct pipe_box box
        20 |     int x
           | [sizeof=24, align=4]
";
    let row = |field: &str, mesa_offset: Option<u64>, bits: u64| clang_layouts::Carried { field: field.to_string(), mesa_offset, bits };
    let all = || {
        let mut carried: BTreeMap<String, Vec<clang_layouts::Carried>> = BTreeMap::new();
        carried.insert("pipe_sampler_state".to_string(), vec![row("wrap_s", None, 3), row("min_lod", Some(4), 0), row("max_lod", Some(8), 0), row("u", Some(12), 0), row("c", Some(16), 0), row("box", Some(20), 0)]);
        carried.insert("pipe_sampler_state.u".to_string(), vec![row("a", Some(0), 0), row("b", Some(0), 0)]);
        carried
    };
    let left = BTreeMap::new();
    assert!(clang_layouts::compare(DUMP, &all(), &left).is_ok(), "{:?}", clang_layouts::compare(DUMP, &all(), &left));
    let mut dropped = all();
    dropped.get_mut("pipe_sampler_state").expect("the structure").retain(|row| row.field != "min_lod");
    let report = clang_layouts::compare(DUMP, &dropped, &left).expect_err("a dropped field fails");
    assert!(report.contains("pipe_sampler_state.min_lod: in clang's layout, neither carried nor left out"), "{report}");
    let mut moved = all();
    moved.get_mut("pipe_sampler_state").expect("the structure")[2].mesa_offset = Some(12);
    assert!(clang_layouts::compare(DUMP, &moved, &left).is_err(), "a moved offset fails");
    let mut extra = all();
    extra.get_mut("pipe_sampler_state").expect("the structure").push(row("absent", Some(24), 0));
    assert!(clang_layouts::compare(DUMP, &extra, &left).is_err(), "a field clang does not lay out fails");
    let mut narrow = all();
    narrow.get_mut("pipe_sampler_state").expect("the structure")[0].bits = 2;
    assert!(clang_layouts::compare(DUMP, &narrow, &left).is_err(), "a bit-field of another width fails");
}

#[cfg(mesa)]
#[test]
fn values_agree() {
    let checked = wcontainers_gallium_direct::values_agree();
    println!("VALUES {checked:?}");
    assert_eq!(checked, Ok(wcontainers_gallium_direct::flat::values().len()), "every declared value of the trait the value C computes");
}

#[cfg(mesa)]
#[test]
fn table_agrees() {
    let (c_size, rust_size) = wcontainers_gallium_direct::table_agrees();
    println!("TABLE C {c_size} Rust {rust_size}");
    assert_eq!((c_size, rust_size), (core::mem::size_of::<wcontainers_gallium_direct::serve::Table>(), core::mem::size_of::<wcontainers_gallium_direct::serve::Table>()), "serve.c's table and serve.rs's the size of serve.rs's");
}
