//! Compute shaders with atomic operations, view counters and typed buffers: what each declares and
//! that the written WGSL reads back. Run against real Direct3D 11 in `tests/compute_draw.rs`.

use std::path::Path;
use wcontainers_gpu::{Format, HelperValue, SlotKind, TexelBufferRoute, VariantKey};
use wcontainers_shaders::ConvertError;
use wcontainers_shaders::{Source, convert};

fn own(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/own")
            .join(name),
    )
    .unwrap()
}

#[test]
fn atomics_and_counters_declare_their_bindings() {
    // (shader, storage slots, counter slots)
    let expected: [(&str, &[u32], &[u32]); 2] = [
        ("cs_atomics", &[0, 1, 2, 3, 4], &[2, 3, 4]),
        ("cs_decrement", &[0], &[0]),
    ];
    for (name, storage, counters) in expected {
        let file = format!("{name}.dxbc");
        let bytes = own(&file);
        let c = match convert(Source::Dxbc(&bytes), &VariantKey::default(), None) {
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
        reads_back(&file, &c.modules[0]);
    }
}

/// A DXBC program handed to the DXIL route is refused by a reason naming the input, in every
/// round, and the routes give the same bytes afterwards as before.
#[test]
fn a_dxbc_program_through_the_dxil_route_is_refused_by_name() {
    let (dxbc, dxil) = (own("cs_atomics.dxbc"), own("cs_atomics.dxil"));
    let encoded = |source| {
        let c = convert(source, &VariantKey::default(), None).expect("converts");
        format!("{:?}", c.modules[0].module)
    };
    let before = [encoded(Source::Dxbc(&dxbc)), encoded(Source::Dxil(&dxil))];
    for round in 0..3 {
        match convert(Source::Dxil(&dxbc), &VariantKey::default(), None) {
            Err(ConvertError::Rejected(reason)) if reason.contains("no DXIL program") => {}
            other => panic!("round {round}: {other:?}"),
        }
    }
    let after = [encoded(Source::Dxbc(&dxbc)), encoded(Source::Dxil(&dxil))];
    assert_eq!(before, after);
}

fn reads_back(file: &str, m: &wcontainers_gpu::ShaderModule) {
    let text =
        naga::back::wgsl::write_string(&m.module, &m.info, naga::back::wgsl::WriterFlags::empty())
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

/// Formats `cs_texels` can bind at each slot: t0 and t3 float, t1 unsigned, t2 signed, u0 float
/// (stored), u1 32-bit unsigned (atomics).
const FLOAT_FORMATS: [Format; 25] = [
    Format::R8Unorm,
    Format::R8Snorm,
    Format::R16Unorm,
    Format::R16Snorm,
    Format::R16Float,
    Format::Rg8Unorm,
    Format::Rg8Snorm,
    Format::R32Float,
    Format::Rg16Unorm,
    Format::Rg16Snorm,
    Format::Rg16Float,
    Format::Rgba8Unorm,
    Format::Rgba8Snorm,
    Format::Bgra8Unorm,
    Format::Bgrx8Unorm,
    Format::Rgb10a2Unorm,
    Format::Rg11b10Float,
    Format::Rg32Float,
    Format::Rgba16Unorm,
    Format::Rgba16Snorm,
    Format::Rgba16Float,
    Format::Rgb32Float,
    Format::Rgba32Float,
    Format::Rgba8UnormSrgb,
    Format::Rgb9e5Float,
];

#[test]
fn typed_buffers_convert_for_every_format_through_storage() {
    let bytes = own("cs_texels.dxbc");
    let key = |t0: Format, u0: Format| VariantKey {
        texel_buffers: TexelBufferRoute::Storage,
        texel_formats: vec![
            (0, t0),
            (1, Format::Rgba16Uint),
            (2, Format::Rg8Sint),
            (3, Format::Rg32Float),
        ],
        storage_formats: vec![(0, u0), (1, Format::R32Uint)],
        ..Default::default()
    };
    // The last two have no typed buffer form here; the others convert, loaded at t0 and stored at u0.
    for (n, &f) in FLOAT_FORMATS.iter().enumerate() {
        let c = convert(Source::Dxbc(&bytes), &key(f, f), None);
        if n >= FLOAT_FORMATS.len() - 2 {
            assert!(
                matches!(
                    c,
                    Err(ConvertError::Unsupported(
                        wcontainers_shaders::refusal::TYPED_BUFFER_FORMAT_ABSENT
                    ))
                ),
                "{f:?}"
            );
            continue;
        }
        let c = c.unwrap_or_else(|e| panic!("{f:?}: {e}"));
        // Typed buffers keep their slots' bindings; each has its view in the helper constants.
        let views: Vec<_> = c
            .reflection
            .helpers
            .iter()
            .filter_map(|h| match h.value {
                HelperValue::TexelBufferView { set, binding } => Some((set, binding)),
                _ => None,
            })
            .collect();
        assert_eq!(views.len(), 6, "{f:?}");
        for b in &c.reflection.bindings {
            let typed = !(b.kind == SlotKind::Storage && b.register == 2);
            assert_eq!(views.contains(&(b.set, b.binding)), typed, "{f:?} {b:?}");
        }
        reads_back(&format!("cs_texels {f:?}"), &c.modules[0]);
    }
    // Refused by name: no route, no format, atomics on a format without them.
    let refused = |key: &VariantKey| match convert(Source::Dxbc(&bytes), key, None) {
        Err(ConvertError::Unsupported(name)) => name,
        other => panic!("{:?}", other.map(|_| ())),
    };
    let plain = key(Format::Rgba32Float, Format::Rgba32Float);
    assert_eq!(
        refused(&VariantKey::default()),
        wcontainers_shaders::refusal::TYPED_BUFFER
    );
    let mut k = plain.clone();
    k.texel_formats.remove(0);
    assert_eq!(
        refused(&k),
        wcontainers_shaders::refusal::TYPED_BUFFER_FORMAT
    );
    let mut k = plain.clone();
    k.storage_formats[1].1 = Format::Rg16Uint;
    assert_eq!(
        refused(&k),
        wcontainers_shaders::refusal::TYPED_BUFFER_ATOMIC_FORMAT
    );
    let mut k = plain;
    k.texel_buffers = TexelBufferRoute::Texture;
    assert_eq!(
        refused(&k),
        wcontainers_shaders::refusal::TYPED_BUFFER_STORAGE_AS_TEXTURE
    );
}

#[test]
fn views_of_another_kind_than_declared_convert_on_both_routes() {
    // cs_texels' float, unsigned and signed typed buffers bound as views of the other kinds, and
    // ps_texels' through textures; cs_images' storage textures likewise, natively and through
    // buffers.
    let texels = VariantKey {
        texel_buffers: TexelBufferRoute::Storage,
        texel_formats: vec![
            (0, Format::Rgba8Uint),
            (1, Format::Rgba16Float),
            (2, Format::R32Uint),
            (3, Format::Rg16Sint),
        ],
        storage_formats: vec![(0, Format::Rgba16Sint), (1, Format::R32Sint)],
        ..Default::default()
    };
    let images = VariantKey {
        storage_formats: vec![
            (0, Format::Rgba32Uint),
            (1, Format::R32Uint),
            (2, Format::Rgba16Sint),
            (3, Format::Rg32Uint),
        ],
        ..Default::default()
    };
    let cases = [
        ("cs_texels", texels),
        (
            "ps_texels",
            VariantKey {
                texel_buffers: TexelBufferRoute::Texture,
                texel_formats: vec![(0, Format::Rgba8Uint), (1, Format::Rg16Float)],
                ..Default::default()
            },
        ),
        ("cs_images", images.clone()),
        (
            "cs_images",
            VariantKey {
                texels_through_buffer: u64::MAX,
                ..images
            },
        ),
    ];
    for (name, key) in cases {
        for dxil in [false, true] {
            let file = format!("{name}.{}", if dxil { "dxil" } else { "dxbc" });
            let bytes = own(&file);
            let source = match dxil {
                true => Source::Dxil(&bytes),
                false => Source::Dxbc(&bytes),
            };
            let c = convert(source, &key, None).unwrap_or_else(|e| panic!("{file} {key:?}: {e}"));
            reads_back(&format!("{file} {key:?}"), &c.modules[0]);
        }
    }
}

/// Each value stored through `global` in `block` and the blocks inside it.
fn stored_values(
    f: &naga::Function,
    block: &naga::Block,
    global: naga::Handle<naga::GlobalVariable>,
    out: &mut Vec<naga::Handle<naga::Expression>>,
) {
    for s in block.iter() {
        match s {
            naga::Statement::ImageStore { image, value, .. }
                if f.expressions[*image] == naga::Expression::GlobalVariable(global) =>
            {
                out.push(*value)
            }
            naga::Statement::Block(b) | naga::Statement::Loop { body: b, .. } => {
                stored_values(f, b, global, out)
            }
            naga::Statement::If { accept, reject, .. } => {
                stored_values(f, accept, global, out);
                stored_values(f, reject, global, out);
            }
            _ => {}
        }
    }
}

#[test]
fn alpha_storage_views_store_their_alpha_alone_on_both_routes() {
    // cs_images' u0 bound as an `A8Unorm` view, which the back ends hold as rgba8unorm with the
    // value in alpha: declared rgba8unorm, every store into it is (0, 0, 0, a) of its value.
    let key = VariantKey {
        storage_formats: vec![
            (0, Format::A8Unorm),
            (1, Format::R32Float),
            (2, Format::Rgba16Uint),
            (3, Format::Rg32Sint),
        ],
        ..Default::default()
    };
    let binding = SlotKind::Storage.binding(0).unwrap();
    for dxil in [false, true] {
        let file = format!("cs_images.{}", if dxil { "dxil" } else { "dxbc" });
        let bytes = own(&file);
        let source = match dxil {
            true => Source::Dxil(&bytes),
            false => Source::Dxbc(&bytes),
        };
        let c = convert(source, &key, None).unwrap_or_else(|e| panic!("{file}: {e}"));
        let m = &c.modules[0].module;
        let (global, _) = m
            .global_variables
            .iter()
            .find(|(_, g)| g.binding.is_some_and(|b| b.binding == binding))
            .unwrap();
        assert!(
            matches!(
                m.types[m.global_variables[global].ty].inner,
                naga::TypeInner::Image {
                    class: naga::ImageClass::Storage {
                        format: naga::StorageFormat::Rgba8Unorm,
                        ..
                    },
                    ..
                }
            ),
            "{file}"
        );
        let mut stores = 0;
        for f in m
            .functions
            .iter()
            .map(|(_, f)| f)
            .chain(m.entry_points.iter().map(|e| &e.function))
        {
            let mut values = Vec::new();
            stored_values(f, &f.body, global, &mut values);
            for v in values {
                let naga::Expression::Compose { components, .. } = &f.expressions[v] else {
                    panic!("{file}: {:?}", f.expressions[v]);
                };
                for &c in &components[..3] {
                    assert_eq!(
                        f.expressions[c],
                        naga::Expression::Literal(naga::Literal::F32(0.0)),
                        "{file}"
                    );
                }
                assert!(
                    matches!(
                        f.expressions[components[3]],
                        naga::Expression::AccessIndex { index: 3, .. }
                    ),
                    "{file}"
                );
                stores += 1;
            }
        }
        assert_eq!(stores, 1, "{file}");
        reads_back(&file, &c.modules[0]);
        // Through a buffer of its one-byte texels, laid out as the helper constants say.
        let through = VariantKey {
            texels_through_buffer: 1,
            ..key.clone()
        };
        let source = match dxil {
            true => Source::Dxil(&bytes),
            false => Source::Dxbc(&bytes),
        };
        let c = convert(source, &through, None)
            .unwrap_or_else(|e| panic!("{file} through a buffer: {e}"));
        assert!(
            c.reflection
                .helpers
                .iter()
                .any(|h| matches!(h.value, HelperValue::TexelLayout { slot: 0 })),
            "{file}"
        );
        reads_back(&file, &c.modules[0]);
    }
}

#[test]
fn storage_textures_convert_through_buffers() {
    let bytes = own("cs_through.dxbc");
    let formats = [
        (3, Format::R32Uint),
        (4, Format::R32Float),
        (5, Format::Rgba16Float),
        (6, Format::Rg8Uint),
    ];
    let key = VariantKey {
        texels_through_buffer: 0b111_1000,
        storage_formats: formats.to_vec(),
        ..Default::default()
    };
    let c = convert(Source::Dxbc(&bytes), &key, None).unwrap_or_else(|e| panic!("{e}"));
    // Each texture's layout is in the helper constants; the bindings stay the slots'.
    let mut layouts: Vec<u32> = c
        .reflection
        .helpers
        .iter()
        .filter_map(|h| match h.value {
            HelperValue::TexelLayout { slot } => Some(slot),
            _ => None,
        })
        .collect();
    layouts.sort();
    assert_eq!(layouts, [3, 4, 5, 6]);
    for b in &c.reflection.bindings {
        assert_eq!(b.binding, SlotKind::Storage.binding(b.register).unwrap());
    }
    reads_back("cs_through", &c.modules[0]);
    // Without the variant, the atomics are refused; without a format, the texture is.
    let plain = VariantKey {
        storage_formats: formats.to_vec(),
        ..Default::default()
    };
    assert!(matches!(
        convert(Source::Dxbc(&bytes), &plain, None),
        Err(ConvertError::Unsupported(
            wcontainers_shaders::refusal::STORAGE_IMAGE_ATOMICS
        ))
    ));
    let mut k = key;
    k.storage_formats.pop();
    assert!(matches!(
        convert(Source::Dxbc(&bytes), &k, None),
        Err(ConvertError::Unsupported(
            wcontainers_shaders::refusal::STORAGE_FORMAT
        ))
    ));
}
