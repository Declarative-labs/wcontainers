//! What the Direct3D 12 front reads from the declarations of every real shader container the
//! crates hold (the shaders crate's own corpus and the graphics test's shaders): typed views and
//! multisampled arrays, one line per container and element, against the answers recorded in
//! declared.expected before views were kept as ranges. `WC_RECORD_DECLARED=1` writes the file.

use std::fmt::Write;
use std::ops::Range;
use std::path::PathBuf;
use wcontainers_direct3d_12::dxbc::{self, TypedView};
use wcontainers_gpu::SlotKind;

/// The folders of containers, relative to the crate.
const CORPORA: [&str; 3] = [
    "../wcontainers-shaders/tests/own",
    "../wcontainers-graphics-test/shaders/dxbc",
    "../wcontainers-graphics-test/shaders/dxil",
];

/// Root signature ranges for the arrays declared without a length, in spaces 0 to 3.
fn ranges() -> Vec<(SlotKind, u32, Range<u32>)> {
    let kinds = [SlotKind::Texture, SlotKind::Storage];
    (0..4)
        .flat_map(|space| kinds.map(|k| (k, space, 0..64)))
        .collect()
}

/// Each element of `views` as (storage, buffer, slot, kind).
fn elements(views: &[TypedView]) -> Vec<String> {
    views
        .iter()
        .flat_map(|v| v.slots.clone().map(move |s| (v, s)))
        .map(|(v, s)| format!("{} {} {s} {:?}", v.storage as u8, v.buffer as u8, v.kind))
        .collect()
}

/// Each slot of `arrays`.
fn slots(arrays: &[Range<u32>]) -> Vec<u32> {
    arrays.iter().flat_map(|r| r.clone()).collect()
}

fn containers() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut all: Vec<PathBuf> = CORPORA
        .iter()
        .flat_map(|dir| std::fs::read_dir(root.join(dir)).unwrap())
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "dxbc" || e == "dxil"))
        .collect();
    all.sort();
    all
}

fn answers() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ranges = ranges();
    let mut out = String::new();
    for path in containers() {
        let code = std::fs::read(&path).unwrap();
        // Named with `/` on every host, as declared.expected holds them.
        let name = path
            .strip_prefix(&root)
            .unwrap()
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        for (label, r) in [("", &[][..]), ("ranged ", &ranges[..])] {
            let views = dxbc::typed_views(&code, r).map(|v| elements(&v));
            let arrays = dxbc::multisampled_arrays(&code, r).map(|a| slots(&a));
            writeln!(out, "{name} {label}dxbc {views:?} {arrays:?}").unwrap();
        }
        let unbounded = dxbc::declares_unbounded_array(&code);
        let (views, arrays) = wcontainers_direct3d_12::dxil::declared(&code);
        let views = views.map(|v| elements(&v));
        let arrays = arrays.map(|a| slots(&a));
        writeln!(out, "{name} dxil {unbounded} {views:?} {arrays:?}").unwrap();
    }
    out
}

#[test]
fn real_containers_declare_what_they_did_before_ranges() {
    let expected = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/declared.expected");
    let got = answers();
    if std::env::var_os("WC_RECORD_DECLARED").is_some() {
        std::fs::write(expected, &got).unwrap();
    }
    let want = std::fs::read_to_string(expected).unwrap();
    assert!(containers().len() > 300);
    for (line, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        assert_eq!(g, w, "line {}", line + 1);
    }
    assert_eq!(got.lines().count(), want.lines().count());
}
