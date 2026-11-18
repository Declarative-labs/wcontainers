//! Compute shaders with atomic operations and view counters: what each declares and that the written
//! WGSL reads back. Run against real Direct3D 11 in `tests/compute_draw.rs`.

use std::path::Path;
use wcontainers_gpu::{SlotKind, VariantKey};
use wcontainers_shaders::{Source, convert};

fn own(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/own")
            .join(name),
    )
    .unwrap()
}

fn source<'a>(bytes: &'a [u8], name: &str) -> Source<'a> {
    if name.ends_with(".dxil") {
        Source::Dxil(bytes)
    } else {
        Source::Dxbc(bytes)
    }
}

#[test]
fn atomics_and_counters_declare_their_bindings() {
    // (shader, storage slots, counter slots)
    let expected: [(&str, &[u32], &[u32]); 2] = [
        ("cs_atomics", &[0, 1, 2, 3, 4], &[2, 3, 4]),
        ("cs_decrement", &[0], &[0]),
    ];
    for (name, storage, counters) in expected {
        for file in [format!("{name}.dxbc")] {
            let bytes = own(&file);
            let c = match convert(source(&bytes, &file), &VariantKey::default(), None) {
                Ok(c) => c,
                Err(e) => panic!("{file}: {e}"),
            };
            let of = |kind| {
                let mut v: Vec<u32> = c
                    .reflection
                    .bindings
                    .iter()
                    .filter(|b| b.kind == kind)
                    .map(|b| {
                        assert_eq!(b.binding, kind.binding(b.register).unwrap(), "{file}");
                        b.register
                    })
                    .collect();
                v.sort();
                v
            };
            assert_eq!(of(SlotKind::Storage), storage, "{file}");
            assert_eq!(of(SlotKind::Counter), counters, "{file}");
            let m = &c.modules[0];
            let text = naga::back::wgsl::write_string(
                &m.module,
                &m.info,
                naga::back::wgsl::WriterFlags::empty(),
            )
            .unwrap();
            let back = naga::front::wgsl::parse_str(&text)
                .unwrap_or_else(|e| panic!("{file}: {}\n{text}", e.emit_to_string(&text)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                wcontainers_shaders::WEBGPU_CAPABILITIES,
            )
            .validate(&back)
            .unwrap_or_else(|e| panic!("{file}: {e:?}\n{text}"));
            if std::env::var_os("WCONTAINERS_PRINT").is_some() {
                println!("// {file}\n{text}");
            }
        }
    }
}
